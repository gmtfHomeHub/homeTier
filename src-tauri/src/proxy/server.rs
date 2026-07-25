use std::pin::Pin;
use std::task::{Context, Poll};
use std::sync::Arc;

use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};

use super::plugin::{PluginChain, ProxyHandler, ProxyPlugin, ProxyResponse, RequestContext};
use super::ws_proxy;

pub struct ProxyServer {
    pub port: u16,
    pub proxy_prefix: String,
    _runtime: tokio::runtime::Runtime,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// 包装 TcpStream，将预先读取的字节「放回」读取流前面，
/// 使得下游（hyper）仍然能读到完整数据。
struct PrependStream {
    stream: TcpStream,
    buf: Vec<u8>,
    pos: usize,
}

impl PrependStream {
    fn new(stream: TcpStream, initial_data: Vec<u8>) -> Self {
        Self { stream, buf: initial_data, pos: 0 }
    }
}

impl AsyncRead for PrependStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // 先提供预先读取的缓存数据
        if self.pos < self.buf.len() {
            let remaining = &self.buf[self.pos..];
            let to_copy = std::cmp::min(remaining.len(), buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.pos += to_copy;
            return Poll::Ready(Ok(()));
        }
        // 缓存耗尽后直接从 TcpStream 读取
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for PrependStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

impl ProxyServer {
    pub fn start(
        plugins: Vec<Arc<dyn ProxyPlugin>>,
        handlers: Vec<Arc<dyn ProxyHandler>>,
    ) -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        let (shutdown_tx, port, proxy_prefix) = rt.block_on(async {
            let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
            let listener = TcpListener::bind(addr)
                .await
                .map_err(|e| format!("Failed to bind proxy port: {}", e))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("Failed to get port: {}", e))?
                .port();
            let proxy_prefix = format!("http://127.0.0.1:{}", port);

            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let shutdown_flag = Arc::new(Mutex::new(false));
            let shutdown_flag_clone = shutdown_flag.clone();

            let chain = Arc::new(PluginChain::new(plugins, handlers));

            tokio::spawn(async move {
                let shutdown_fut = async { shutdown_rx.await.ok() };
                tokio::pin!(shutdown_fut);

                loop {
                    tokio::select! {
                        accept_result = listener.accept() => {
                            match accept_result {
                                Ok((stream, _)) => {
                                    let chain = chain.clone();
                                    let shutdown_flag = shutdown_flag_clone.clone();
                                    tokio::spawn(async move {
                                        // 尝试读取初始字节，判断是否为 WS upgrade
                                        let mut peek_buf = vec![0u8; 8192];
                                        let n = match stream.try_read(&mut peek_buf) {
                                            Ok(n) => n,
                                            Err(_) => {
                                                // 没有立即可读数据，尝试异步读取
                                                match stream.readable().await {
                                                    Ok(()) => match stream.try_read(&mut peek_buf) {
                                                        Ok(n) => n,
                                                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                                            // 仍不可读，按非 WS 处理
                                                            // 将 stream 传给 hyper（空预读数据）
                                                            let prepend = PrependStream::new(stream, vec![]);
                                                            let io = TokioIo::new(prepend);
                                                            serve_http(io, chain, shutdown_flag).await;
                                                            return;
                                                        }
                                                        Err(_) => return,
                                                    },
                                                    Err(_) => return,
                                                }
                                            }
                                        };

                                        let peek_data = &peek_buf[..n];

                                        if ws_proxy::is_raw_ws_upgrade(peek_data) {
                                            // WS 请求：直接使用原始 TcpStream，不经过 hyper
                                            ws_proxy::handle_raw_upgrade(stream, peek_data.to_vec()).await;
                                        } else {
                                            // 非 WS 请求：将预读数据包装回 stream，传给 hyper
                                            let prepend = PrependStream::new(stream, peek_data.to_vec());
                                            let io = TokioIo::new(prepend);
                                            serve_http(io, chain, shutdown_flag).await;
                                        }
                                    });
                                }
                                Err(e) => {
                                    eprintln!("Proxy accept error: {}", e);
                                }
                            }
                        }
                        _ = &mut shutdown_fut => {
                            *shutdown_flag_clone.lock().await = true;
                            break;
                        }
                    }
                }
            });

            Ok::<_, String>((shutdown_tx, port, proxy_prefix))
        })?;

        Ok(Self {
            port,
            proxy_prefix,
            _runtime: rt,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    pub fn proxy_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn proxy_url_for(&self, target_url: &str) -> String {
        format!(
            "http://127.0.0.1:{}/proxy?url={}",
            self.port,
            urlencoding::encode(target_url)
        )
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn serve_http(
    io: TokioIo<PrependStream>,
    chain: Arc<PluginChain>,
    shutdown_flag: Arc<Mutex<bool>>,
) {
    let service = service_fn(move |req: Request<Incoming>| {
        let chain = chain.clone();
        async move {
            let ctx = RequestContext::new();
            Ok::<_, std::convert::Infallible>(
                chain.process(req, ctx).await,
            )
        }
    });
    let conn = http1::Builder::new()
        .serve_connection(io, service)
        .with_upgrades();
    tokio::select! {
        _ = conn => {}
        _ = async {
            loop {
                if *shutdown_flag.lock().await {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        } => {}
    }
}