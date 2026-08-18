//! TCP 客户端：连接远端节点，发送请求

use crate::config_store::{ConfigFile, Message};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// 远端配置存储节点客户端
pub struct RemoteStore {
    addr: String,
}

impl RemoteStore {
    pub fn new(ip: &str, port: u16) -> Self {
        Self {
            addr: format!("{}:{}", ip, port),
        }
    }

    async fn roundtrip(&self, msg: &Message) -> Result<Message, String> {
        let mut stream = tokio::time::timeout(
            Duration::from_secs(10),
            TcpStream::connect(&self.addr),
        )
        .await
        .map_err(|_| format!("连接 {} 超时", self.addr))?
        .map_err(|e| format!("连接 {} 失败: {}", self.addr, e))?;

        stream
            .write_all(&msg.to_line())
            .await
            .map_err(|e| e.to_string())?;
        stream.flush().await.map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("远端关闭连接".to_string());
        }
        Message::from_line(line.as_bytes()).map_err(|e| format!("响应解析失败: {}", e))
    }

    /// 查询远端某个配置的最新版本
    pub async fn query_version(
        &self,
        name: &str,
    ) -> Result<Option<crate::config_store::ConfigFileMeta>, String> {
        match self
            .roundtrip(&Message::QueryVersion { name: name.into() })
            .await?
        {
            Message::VersionInfo {
                name: _,
                version,
                timestamp,
                checksum,
            } => {
                if version == 0 && timestamp == 0 {
                    Ok(None)
                } else {
                    Ok(Some(crate::config_store::ConfigFileMeta {
                        name: name.to_string(),
                        version,
                        timestamp,
                        checksum,
                    }))
                }
            }
            other => Err(format!("异常响应: {:?}", other)),
        }
    }

    /// 从远端下载某个配置的最新内容
    pub async fn request_file(&self, name: &str) -> Result<Option<ConfigFile>, String> {
        match self
            .roundtrip(&Message::RequestFile {
                name: name.into(),
                from_version: None,
            })
            .await?
        {
            Message::FileResponse {
                name: _,
                version,
                content,
                checksum,
            } => {
                if version == 0 && content.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(ConfigFile {
                        name: name.to_string(),
                        version,
                        content,
                        timestamp: 0,
                        checksum,
                    }))
                }
            }
            other => Err(format!("异常响应: {:?}", other)),
        }
    }

    /// 推式更新：将配置存储到远端节点
    pub async fn store_file(&self, file: &ConfigFile) -> Result<bool, String> {
        match self
            .roundtrip(&Message::StoreFile {
                name: file.name.clone(),
                version: file.version,
                content: file.content.clone(),
                timestamp: file.timestamp,
                checksum: file.checksum.clone(),
            })
            .await?
        {
            Message::StoreAck {
                name: _,
                success,
                error,
            } => {
                if success {
                    Ok(true)
                } else {
                    Err(error.unwrap_or_else(|| "存储失败".to_string()))
                }
            }
            other => Err(format!("异常响应: {:?}", other)),
        }
    }
}
