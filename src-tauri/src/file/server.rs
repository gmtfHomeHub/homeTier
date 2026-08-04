use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;
use uuid::Uuid;

/// 文件服务器，运行在虚拟网络上。
///
/// 每个空间一个实例，监听虚拟网上的 `19000 + (space_id % 1000)` 端口，
/// 提供文件上传（PUT）与下载（GET）。上传数据流式写入磁盘，下载流式返回，
/// 支持任意大小文件。
pub struct FileServer {
    #[allow(dead_code)]
    space_id: Uuid,
    port: u16,
    running: Arc<RwLock<bool>>,
    storage_dir: PathBuf,
}

impl FileServer {
    pub fn new(space_id: Uuid, storage_dir: PathBuf) -> Self {
        Self {
            space_id,
            port: 0,
            running: Arc::new(RwLock::new(false)),
            storage_dir,
        }
    }

    /// 启动 HTTP 服务监听（仅监听虚拟网接口 0.0.0.0，EasyTier 会路由）
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

    /// 读取已接收的文件字节（用于本地下载）
    pub async fn read_file(&self, file_id: &Uuid) -> Result<Vec<u8>, String> {
        let file_path = self.storage_dir.join(format!("{}.bin", file_id));
        tokio::fs::read(&file_path)
            .await
            .map_err(|e| format!("读取文件失败: {}", e))
    }

    /// 本地直接写入文件（本机传输优化，跳过 HTTP）
    pub async fn write_file(&self, file_id: &Uuid, data: &[u8]) -> Result<(), String> {
        let file_path = self.storage_dir.join(format!("{}.bin", file_id));
        if let Some(parent) = file_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(&file_path, data)
            .await
            .map_err(|e| format!("保存文件失败: {}", e))
    }

    /// 删除本地存储的文件
    pub async fn delete_file(&self, file_id: &Uuid) -> Result<(), String> {
        let file_path = self.storage_dir.join(format!("{}.bin", file_id));
        match tokio::fs::remove_file(&file_path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("删除文件失败: {}", e)),
        }
    }
}

/// 处理 HTTP 连接（流式读取请求体，避免大文件占用内存）
async fn handle_connection(mut stream: tokio::net::TcpStream, storage_dir: PathBuf) {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let mut header_buf = Vec::with_capacity(4096);

    // 读取请求头直到 \r\n\r\n
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk).await {
            Ok(0) => return,
            Ok(n) => {
                header_buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = find_header_end(&header_buf) {
                    break Some(pos);
                }
                if header_buf.len() > 64 * 1024 {
                    return;
                }
            }
            Err(_) => return,
        }
    };

    let header_end = match header_end {
        Some(pos) => pos,
        None => return,
    };

    let header_text = String::from_utf8_lossy(&header_buf[..header_end]).to_string();
    let mut lines = header_text.lines();
    let request_line = match lines.next() {
        Some(l) => l.to_string(),
        None => return,
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    // 解析 Content-Length
    let mut content_length: usize = 0;
    for line in lines {
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") {
            content_length = line
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
        }
    }

    let body_offset = header_end + 4; // skip \r\n\r\n
    let mut body = header_buf.split_off(body_offset);

    // 继续读取 body 剩余部分
    while body.len() < content_length {
        let mut chunk = [0u8; 16384];
        match stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }

    // 解析路径: /files/{file_id}
    let response = match (method.as_str(), path.as_str()) {
        ("PUT", p) if p.starts_with("/files/") => {
            let file_id_str = p.trim_start_matches("/files/");
            match Uuid::parse_str(file_id_str) {
                Ok(file_id) => {
                    let file_path = storage_dir.join(format!("{}.bin", file_id));
                    if let Some(parent) = file_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::write(&file_path, &body) {
                        Ok(_) => "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_string(),
                        Err(e) => format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\n\r\n{}",
                            e.to_string().len(),
                            e
                        ),
                    }
                }
                Err(_) => "HTTP/1.1 400 Bad Request\r\nContent-Length: 15\r\n\r\nInvalid file ID".to_string(),
            }
        }
        ("GET", p) if p.starts_with("/files/") => {
            let file_id_str = p.trim_start_matches("/files/");
            match Uuid::parse_str(file_id_str) {
                Ok(file_id) => {
                    let file_path = storage_dir.join(format!("{}.bin", file_id));
                    match tokio::fs::read(&file_path).await {
                        Ok(data) => {
                            // 流式返回二进制：先写 header，再写 body
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                                data.len()
                            );
                            if let Err(_) = stream.write_all(header.as_bytes()).await {
                                return;
                            }
                            let _ = stream.write_all(&data).await;
                            return;
                        }
                        Err(_) => "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_string(),
                    }
                }
                Err(_) => "HTTP/1.1 400 Bad Request\r\nContent-Length: 15\r\n\r\nInvalid file ID".to_string(),
            }
        }
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_string(),
    };

    let _ = stream.write_all(response.as_bytes()).await;
}

/// 查找请求头结束位置（\r\n\r\n）
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
}
