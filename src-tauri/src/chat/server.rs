use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::chat::message::ChatMessage;

/// P2P 聊天服务器，运行在虚拟网络上
pub struct ChatServer {
    port: u16,
    running: Arc<RwLock<bool>>,
    message_queue: Arc<RwLock<Vec<ChatMessage>>>,
}

impl ChatServer {
    pub fn new() -> Self {
        Self {
            port: 0,
            running: Arc::new(RwLock::new(false)),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 启动 HTTP 服务监听
    pub async fn start(&mut self, port: u16) -> Result<(), String> {
        self.port = port;
        *self.running.write().await = true;

        let running = self.running.clone();
        let queue = self.message_queue.clone();

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .map_err(|e| format!("监听端口 {} 失败: {}", port, e))?;

        tokio::spawn(async move {
            loop {
                if !*running.read().await {
                    break;
                }

                match tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    listener.accept(),
                ).await {
                    Ok(Ok((stream, _addr))) => {
                        let queue = queue.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, queue).await;
                        });
                    }
                    Ok(Err(e)) => {
                        eprintln!("HTTP accept error: {}", e);
                    }
                    Err(_) => {
                        // timeout, check running flag again
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止服务
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// 发送消息到指定节点
    pub async fn send_to(&self, target_ip: &str, target_port: u16, msg: &ChatMessage) -> Result<(), String> {
        let url = format!("http://{}:{}/message", target_ip, target_port);
        let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;

        let client = reqwest::Client::new();
        client.post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Send message error: {}", e))?;

        Ok(())
    }

    /// 获取收到的消息队列
    pub async fn drain_messages(&self) -> Vec<ChatMessage> {
        let mut queue = self.message_queue.write().await;
        queue.drain(..).collect()
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// 处理 HTTP 连接
async fn handle_connection(stream: tokio::net::TcpStream, queue: Arc<RwLock<Vec<ChatMessage>>>) {
    use tokio::io::AsyncReadExt;

    let mut buffer = [0u8; 8192];
    let mut stream = tokio::io::BufReader::new(stream);
    let n = match stream.read(&mut buffer).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);

    // 解析 HTTP 请求路径和 body
    if let Some(body_start) = request.find("\r\n\r\n") {
        let body = &request[body_start + 4..];

        // 尝试解析为 ChatMessage
        if let Ok(msg) = serde_json::from_str::<ChatMessage>(body.trim()) {
            let mut q = queue.write().await;
            q.push(msg);
        }
    }

    // 返回 HTTP 200
    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream.into_inner(), response.as_bytes()).await;
}