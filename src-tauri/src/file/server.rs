use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// 文件服务器，运行在虚拟网络上
pub struct FileServer {
    port: u16,
    running: Arc<RwLock<bool>>,
    storage_dir: PathBuf,
}

impl FileServer {
    pub fn new(storage_dir: PathBuf) -> Self {
        Self {
            port: 0,
            running: Arc::new(RwLock::new(false)),
            storage_dir,
        }
    }

    /// 启动 HTTP 服务监听
    pub async fn start(&mut self, port: u16) -> Result<(), String> {
        self.port = port;
        *self.running.write().await = true;

        let running = self.running.clone();
        let storage_dir = self.storage_dir.clone();

        let socket = tokio::net::TcpSocket::new_v4()
            .map_err(|e| format!("创建 socket 失败: {}", e))?;
        let _ = socket.set_reuseaddr(true);
        socket.bind(format!("0.0.0.0:{}", port).parse().unwrap())
            .map_err(|e| format!("绑定端口 {} 失败: {}", port, e))?;
        let listener = socket.listen(1024)
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
                        let storage_dir = storage_dir.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, storage_dir).await;
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

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// 处理 HTTP 连接
async fn handle_connection(stream: tokio::net::TcpStream, storage_dir: PathBuf) {
    use tokio::io::AsyncReadExt;

    let mut buffer = [0u8; 8192];
    let mut stream = tokio::io::BufReader::new(stream);
    let n = match stream.read(&mut buffer).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);

    // 简单解析 HTTP 请求
    let lines: Vec<&str> = request.lines().collect();
    if lines.is_empty() {
        return;
    }

    let request_line = lines[0];
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 3 {
        return;
    }

    let method = parts[0];
    let path = parts[1];

    let response = match (method, path) {
        ("POST", path) if path.starts_with("/files/") => {
            // 接收文件上传
            let file_id = path.trim_start_matches("/files/");
            if let Ok(uuid) = Uuid::parse_str(file_id) {
                match receive_upload(&request, uuid, &storage_dir).await {
                    Ok(_) => "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_string(),
                    Err(e) => format!("HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\n\r\n{}", e.len(), e),
                }
            } else {
                "HTTP/1.1 400 Bad Request\r\nContent-Length: 15\r\n\r\nInvalid file ID".to_string()
            }
        }
        ("GET", path) if path.starts_with("/files/") => {
            // 下载文件
            let file_id = path.trim_start_matches("/files/");
            if let Ok(uuid) = Uuid::parse_str(file_id) {
                match serve_download(uuid, &storage_dir).await {
                    Ok((body, mime)) => {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
                            mime,
                            body.len(),
                            body
                        )
                    }
                    Err(e) => format!("HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n{}", e.len(), e),
                }
            } else {
                "HTTP/1.1 400 Bad Request\r\nContent-Length: 15\r\n\r\nInvalid file ID".to_string()
            }
        }
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_string(),
    };

    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream.into_inner(), response.as_bytes()).await;
}

/// 接收文件上传
async fn receive_upload(request: &str, file_id: Uuid, storage_dir: &PathBuf) -> Result<(), String> {
    // 提取 body
    if let Some(body_start) = request.find("\r\n\r\n") {
        let body = &request[body_start + 4..];

        // 创建存储目录
        let file_path = storage_dir.join(format!("{}.bin", file_id));
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }

        // 保存文件
        std::fs::write(&file_path, body)
            .map_err(|e| format!("保存文件失败: {}", e))?;

        Ok(())
    } else {
        Err("Invalid request format".to_string())
    }
}

/// 提供文件下载
async fn serve_download(file_id: Uuid, storage_dir: &PathBuf) -> Result<(String, String), String> {
    let file_path = storage_dir.join(format!("{}.bin", file_id));

    let data = std::fs::read(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    Ok((String::from_utf8_lossy(&data).to_string(), "application/octet-stream".to_string()))
}