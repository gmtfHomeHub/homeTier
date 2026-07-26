use std::path::PathBuf;
use crate::daemon::{client::IpcClient, ipc::IpcResponse};
use crate::easytier::{EasyTierManager, EasyTierDownloader, BinarySource};
use tauri::{State, Emitter};
use futures_util::StreamExt;

/// 获取 EasyTier 版本
#[tauri::command]
pub async fn get_easytier_version() -> Result<String, String> {
    let client = IpcClient::default_port();
    match client.get_version().await {
        Ok(IpcResponse::Ok { data }) => {
            data.and_then(|v| v.get("version").and_then(|s| s.as_str().map(|s| s.to_string())))
                .ok_or_else(|| "无法获取版本".into())
        }
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(format!("连接 daemon 失败: {}", e)),
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
    let client = IpcClient::default_port();

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
    let platform = EasyTierDownloader::detect_platform();
    let filename = format!("easytier-{}-v{}.zip", platform, version);
    let direct_url = format!(
        "https://github.com/EasyTier/EasyTier/releases/download/v{}/{}",
        version, filename
    );
    let download_url = if use_proxy {
        format!("https://ghproxy.top/{}", direct_url)
    } else {
        direct_url
    };

    let client = reqwest::Client::builder()
        .user_agent("homeTier/0.1.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client.get(&download_url).send().await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("下载返回 HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!("easytier-dl-{}", version));
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    let temp_path = temp_dir.join(&filename);
    let tmp_path = temp_dir.join(format!("{}.tmp", filename));

    let mut file = tokio::fs::File::create(&tmp_path).await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;
    let mut downloaded: u64 = 0;
    let stream = resp.bytes_stream();
    tokio::pin!(stream);

    use tokio::io::AsyncWriteExt;
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

    manager.downloader.install(&version, BinarySource::LocalArchive(temp_path.clone())).await?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    let ipc = IpcClient::default_port();
    if !ipc.ping().await {
        return Err("daemon 未运行或无法连接".into());
    }
    match ipc.switch_binary().await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(format!("连接 daemon 失败: {}", e)),
    }
}

/// 从源码编译 EasyTier 核心
#[tauri::command]
pub async fn build_easytier_from_source(
    manager: State<'_, std::sync::Arc<EasyTierManager>>,
) -> Result<String, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("..").join("third_libs").join("easytier");
    if !source_dir.exists() {
        return Err(format!(
            "EasyTier 源代码未找到: {}。请确保 third_libs/easytier/ 目录存在。",
            source_dir.display()
        ));
    }
    manager.downloader.build_from_source(&source_dir).await
}
