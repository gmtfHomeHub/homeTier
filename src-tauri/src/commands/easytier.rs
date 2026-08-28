use crate::daemon::{client::IpcClient, ipc::IpcResponse};
use crate::easytier::{EasyTierManager, EasyTierDownloader, BinarySource};
use std::sync::Arc;
use tauri::{State, Emitter};
use futures_util::StreamExt;

/// 获取 EasyTier 版本
#[tauri::command]
pub async fn get_easytier_version(
    #[allow(unused_variables)] easytier: State<'_, Arc<EasyTierManager>>,
) -> Result<String, String> {
    // Mobile: 直接从 EasyTierManager 读取编译期版本号，无需 IPC daemon
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return easytier.get_version().await;

    // Desktop: 通过 daemon IPC 获取版本；IPC 失败时回退到本地读取 current_version.json
    // （binary 可能已由 GUI/daemon 解压，仅版本查询 IPC 不通不应显示“未安装”）
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = IpcClient::get_global();
        match client.get_version().await {
            Ok(IpcResponse::Ok { data }) => {
                data.and_then(|v| v.get("version").and_then(|s| s.as_str().map(|s| s.to_string())))
                    .ok_or_else(|| "无法获取版本".into())
            }
            Ok(IpcResponse::Error { message }) => {
                // daemon 返回错误（如 current_version.json 读取失败），回退本地
                crate::log_warn!(format!("[get_easytier_version] daemon 返回错误，回退本地读取: {}", message));
                easytier.get_version().await.map_err(|e| format!("{}; 本地读取也失败: {}", message, e))
            }
            Err(e) => {
                // daemon IPC 不通，回退本地读取 current_version.json
                crate::log_warn!(format!("[get_easytier_version] daemon IPC 不通，回退本地读取: {}", e));
                easytier.get_version().await.map_err(|local_err| format!("连接 daemon 失败: {}; 本地读取也失败: {}", e, local_err))
            }
        }
    }
}

/// 检查 EasyTier 更新
#[tauri::command]
pub async fn check_easytier_update() -> Result<Vec<String>, String> {
    crate::easytier::github::fetch_available_versions().await
}

/// 升级 EasyTier（通过 daemon，无进度）
#[tauri::command]
pub async fn upgrade_easytier(version: String, source_path: Option<String>) -> Result<(), String> {
    let client = IpcClient::get_global();

    // 先检查 daemon 连通性
    if !client.ping().await {
        return Err("daemon 未运行或无法连接".into());
    }

    match client.upgrade(&version, source_path.as_deref()).await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(format!("连接 daemon 失败: {}", e)),
    }
}

/// 升级 EasyTier（带下载进度，app 侧下载，通知 daemon 切换）
#[tauri::command]
pub async fn upgrade_easytier_with_progress(
    version: String,
    use_proxy: bool,
    app_handle: tauri::AppHandle,
    manager: State<'_, std::sync::Arc<EasyTierManager>>,
) -> Result<(), String> {
    const MAX_RETRIES: u32 = 3;
    const BASE_DELAY_MS: u64 = 1000;

    use tokio::io::AsyncWriteExt;

    let platform = EasyTierDownloader::detect_platform();
    let filename = format!("easytier-{}-v{}.zip", platform, version);
    let direct_url = format!(
        "https://github.com/EasyTier/EasyTier/releases/download/v{}/{}",
        version, filename
    );
    let mirror = crate::config::get_str(crate::config::KEY_GITHUB_MIRROR, crate::config::DEFAULT_GITHUB_MIRROR);
    let proxy_url = format!("{}/{}", mirror, direct_url);

    let urls: Vec<(&str, &str)> = if use_proxy {
        vec![(&proxy_url, "代理"), (&direct_url, "直连")]
    } else {
        vec![(&direct_url, "直连"), (&proxy_url, "代理")]
    };

    let client = reqwest::Client::builder()
        .user_agent("homeTier/0.1.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut last_err = String::new();

    for (url, label) in &urls {
        for attempt in 1..=MAX_RETRIES {
            let _ = app_handle.emit("easytier-download-status", format!("{}({}/{})", label, attempt, MAX_RETRIES));

            let temp_dir = std::env::temp_dir().join(format!("easytier-dl-{}", version));
            let _ = std::fs::remove_dir_all(&temp_dir);

            let resp = match client.get(*url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = format!("{} 下载失败: {}", label, e);
                    if attempt < MAX_RETRIES {
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    }
                    continue;
                }
            };

            if !resp.status().is_success() {
                last_err = format!("{} 返回 HTTP {}", label, resp.status());
                if attempt < MAX_RETRIES {
                    let delay = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
                continue;
            }

            let total = resp.content_length().unwrap_or(0);
            std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
            let temp_path = temp_dir.join(&filename);
            let tmp_path = temp_dir.join(format!("{}.tmp", filename));

            let mut file = tokio::fs::File::create(&tmp_path).await
                .map_err(|e| format!("创建临时文件失败: {}", e))?;
            let mut downloaded: u64 = 0;
            let stream = resp.bytes_stream();
            tokio::pin!(stream);

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| format!("读取下载数据失败: {}", e))?;
                file.write_all(&chunk).await.map_err(|e| format!("写入临时文件失败: {}", e))?;
                downloaded += chunk.len() as u64;
                if total > 0 {
                    let _ = app_handle.emit("easytier-download-progress", downloaded as f64 / total as f64 * 100.0);
                }
            }

            file.flush().await.map_err(|e| format!("刷新文件失败: {}", e))?;
            drop(file);

            tokio::fs::rename(&tmp_path, &temp_path).await.map_err(|e| format!("重命名临时文件失败: {}", e))?;

            if let Err(e) = manager.downloader.install(&version, BinarySource::LocalArchive(temp_path.clone())).await {
                last_err = format!("安装失败: {}", e);
                let _ = std::fs::remove_dir_all(&temp_dir);
                if attempt < MAX_RETRIES {
                    let delay = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
                continue;
            }

            let _ = std::fs::remove_dir_all(&temp_dir);

            let ipc = IpcClient::get_global();
            if !ipc.ping().await {
                return Err("daemon 未运行或无法连接".into());
            }
            return match ipc.switch_binary().await {
                Ok(IpcResponse::Ok { .. }) => Ok(()),
                Ok(IpcResponse::Error { message }) => Err(message),
                Err(e) => Err(format!("连接 daemon 失败: {}", e)),
            };
        }
    }

    Err(format!("下载失败 (重试 {} 次, {} 个源): {}", MAX_RETRIES, urls.len(), last_err))
}
