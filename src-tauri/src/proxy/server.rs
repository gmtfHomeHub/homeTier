use std::sync::Arc;

use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use http_body_util::Full;
use hyper::body::Bytes;
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
                                        // Peek at first bytes to detect WebSocket upgrade
                                        let mut peek_buf = [0u8; 4096];
                                        let peeked = match stream.peek(&mut peek_buf).await {
                                            Ok(n) if n > 0 => &peek_buf[..n],
                                            _ => {
                                                let io = TokioIo::new(stream);
                                                let service = service_fn(move |req: Request<Incoming>| {
                                                    let chain = chain.clone();
                                                    async move {
                                                        let ctx = RequestContext::new();
                                                        Ok::<_, std::convert::Infallible>(chain.process(req, ctx).await)
                                                    }
                                                });
                                                let conn = http1::Builder::new()
                                                    .serve_connection(io, service);
                                                let _ = conn.await;
                                                return;
                                            }
                                        };

                                        if ws_proxy::is_ws_upgrade(peeked) {
                                            if let Err(e) = ws_proxy::handle_stream(stream).await {
                                                eprintln!("WebSocket proxy error: {}", e);
                                            }
                                        } else {
                                            let io = TokioIo::new(stream);
                                            let service = service_fn(move |req: Request<Incoming>| {
                                                let chain = chain.clone();
                                                async move {
                                                    let ctx = RequestContext::new();
                                                    Ok::<_, std::convert::Infallible>(chain.process(req, ctx).await)
                                                }
                                            });
                                            let conn = http1::Builder::new()
                                                .serve_connection(io, service);
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
