use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::spawn;

use crate::voice::signal::SignalHandler;

/// WebRTC 信令服务器
pub struct VoiceServer {
    port: u16,
    messages: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl VoiceServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 启动信令服务器
    pub async fn start(&mut self) -> Result<(), String> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| format!("监听信令端口失败: {}", e))?;

        let messages = self.messages.clone();
        spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let messages = messages.clone();
                        spawn(async move {
                            handle_connection(stream, messages).await;
                        });
                    }
                    Err(e) => {
                        eprintln!("信令服务器接受连接失败: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// 获取信令端口
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// 处理 HTTP 连接
async fn handle_connection(stream: tokio::net::TcpStream, messages: Arc<RwLock<HashMap<String, Vec<String>>>>) {
    let mut buffer = [0u8; 8192];
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
                let _ = SignalHandler::send_offer("127.0.0.1", 18000, body).await;
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string()
            }
            "/signal/answer" => {
                let _ = SignalHandler::send_answer("127.0.0.1", 18000, body).await;
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string()
            }
            "/signal/ice" => {
                let _ = SignalHandler::send_ice("127.0.0.1", 18000, body).await;
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
