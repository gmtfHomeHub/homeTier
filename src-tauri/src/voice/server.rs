use tokio::sync::oneshot;
use tokio::task::spawn;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// WebRTC 信令服务器（带生命周期管理）
pub struct VoiceServer {
    port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl VoiceServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            shutdown_tx: None,
        }
    }

    /// 启动信令服务器
    pub async fn start(&mut self) -> Result<(), String> {
        let socket = tokio::net::TcpSocket::new_v4()
            .map_err(|e| format!("创建 socket 失败: {}", e))?;
        let _ = socket.set_reuseaddr(true);
        socket.bind(format!("0.0.0.0:{}", self.port).parse().unwrap())
            .map_err(|e| format!("绑定信令端口失败: {}", e))?;
        let listener = socket.listen(1024)
            .map_err(|e| format!("监听信令端口失败: {}", e))?;

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _addr)) => {
                                spawn(async move {
                                    handle_connection(stream).await;
                                });
                            }
                            Err(e) => {
                                crate::log_error!(format!("信令服务器接受连接失败: {}", e));
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止信令服务器
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// 获取信令端口
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// 处理 HTTP 连接
async fn handle_connection(stream: tokio::net::TcpStream) {
    let mut buffer = [0u8; 16384];
    let mut stream = tokio::io::BufReader::new(stream);

    let n = match stream.read(&mut buffer).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);

    let response = if let Some(body_start) = request.find("\r\n\r\n") {
        let body = &request[body_start + 4..];
        let path = request.split_whitespace().nth(1).unwrap_or("/");

        match path {
            "/signal/offer" => {
                crate::voice::signal::ingest_offer(body).await;
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string()
            }
            "/signal/answer" => {
                crate::voice::signal::ingest_answer(body).await;
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string()
            }
            "/signal/ice" => {
                crate::voice::signal::ingest_ice(body).await;
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string()
            }
            _ => {
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
            }
        }
    } else {
        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_string()
    };

    let _ = stream.into_inner().write_all(response.as_bytes()).await;
}