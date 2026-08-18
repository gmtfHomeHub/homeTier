//! TCP 服务端：监听端口，处理远端请求

use crate::config_store::{ConfigStoreService, Message};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// 启动 TCP 监听（0.0.0.0:{port}），阻塞直到服务关闭
pub async fn serve(service: Arc<ConfigStoreService>, port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    crate::log_info!(format!("[config_store] TCP 服务已启动，监听 0.0.0.0:{}", port));

    loop {
        let (stream, peer) = listener.accept().await?;
        crate::log_debug!("[config_store] 新连接: {}", peer);
        let service = Arc::clone(&service);
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, service).await {
                crate::log_debug!(format!(
                    "[config_store] 连接处理结束({}): {}",
                    peer, e
                ));
            }
        });
    }
}

async fn handle_conn(
    stream: tokio::net::TcpStream,
    service: Arc<ConfigStoreService>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }
        let msg = match Message::from_line(line.as_bytes()) {
            Ok(m) => m,
            Err(e) => {
                let resp = Message::StoreAck {
                    name: String::new(),
                    success: false,
                    error: Some(format!("协议解析失败: {}", e)),
                };
                writer.write_all(&resp.to_line()).await?;
                continue;
            }
        };

        let resp = dispatch(&service, msg).await;
        writer.write_all(&resp.to_line()).await?;
        writer.flush().await?;
    }
    Ok(())
}

async fn dispatch(service: &ConfigStoreService, msg: Message) -> Message {
    match msg {
        Message::QueryVersion { name } => {
            match service.store.get_meta(&name) {
                Some(meta) => Message::VersionInfo {
                    name: meta.name,
                    version: meta.version,
                    timestamp: meta.timestamp,
                    checksum: meta.checksum,
                },
                None => Message::VersionInfo {
                    name,
                    version: 0,
                    timestamp: 0,
                    checksum: None,
                },
            }
        }
        Message::RequestFile {
            name,
            from_version: _,
        } => {
            match service.store.get_file(&name) {
                Ok(Some(file)) => Message::FileResponse {
                    name: file.name,
                    version: file.version,
                    content: file.content,
                    checksum: file.checksum,
                },
                _ => Message::FileResponse {
                    name,
                    version: 0,
                    content: Vec::new(),
                    checksum: None,
                },
            }
        }
        Message::StoreFile {
            name,
            version,
            content,
            timestamp,
            checksum,
        } => {
            let file = crate::config_store::ConfigFile {
                name: name.clone(),
                version,
                content,
                timestamp,
                checksum,
            };
            // 推式更新入队（去重 + 文件锁 + 版本防回退在 store 内部保证）
            service.store_local(file);
            Message::StoreAck {
                name,
                success: true,
                error: None,
            }
        }
        _ => Message::StoreAck {
            name: String::new(),
            success: false,
            error: Some("不支持的请求类型".to_string()),
        },
    }
}
