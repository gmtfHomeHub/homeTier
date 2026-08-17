use serde::Serialize;
use tauri::{AppHandle, Emitter};
use futures_util::StreamExt;
use sha2::Digest;
use tokio::io::AsyncWriteExt;

/// 应用更新检查结果
#[derive(Serialize, Clone)]
pub struct CheckAppUpdate {
    pub current: String,
    /// 最新版本号；检查失败（离线等）时为 None，前端静默
    pub latest: Option<String>,
    pub has_update: bool,
}

/// 应用更新执行结果
#[derive(Serialize, Clone)]
pub struct AppUpdateOutcome {
    /// "installed"：已自动下载并替换安装包，需重启生效
    /// "open_release"：当前运行方式不支持自动安装，应打开 Release 页面
    pub action: String,
}

const RELEASE_LATEST_URL: &str = "https://api.github.com/repos/gmtfHomeHub/homeTier/releases/latest";

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize, Clone)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("homeTier/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))
}

/// 语义化版本比较：返回 1 (a>b), 0 (a=b), -1 (a<b)
fn compare_versions(a: &str, b: &str) -> i32 {
    let parse = |v: &str| {
        v.trim_start_matches('v')
            .split('.')
            .map(|s| s.parse::<i32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let pa = parse(a);
    let pb = parse(b);
    for i in 0..pa.len().max(pb.len()) {
        let na = *pa.get(i).unwrap_or(&0);
        let nb = *pb.get(i).unwrap_or(&0);
        if na > nb {
            return 1;
        }
        if na < nb {
            return -1;
        }
    }
    0
}

fn github_mirror() -> String {
    crate::config::get_str(crate::config::KEY_GITHUB_MIRROR, crate::config::DEFAULT_GITHUB_MIRROR)
}

async fn fetch_latest_release() -> Result<GitHubRelease, String> {
    let client = http_client()?;
    let resp = client
        .get(RELEASE_LATEST_URL)
        .send()
        .await
        .map_err(|e| format!("获取最新版本失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回错误: {}", resp.status()));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
}

/// 检查应用更新（启动时调用一次）
pub async fn check_app_update() -> CheckAppUpdate {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let release = match fetch_latest_release().await {
        Ok(r) => r,
        Err(_) => {
            return CheckAppUpdate {
                current,
                latest: None,
                has_update: false,
            };
        }
    };
    let latest = release.tag_name.trim_start_matches('v').to_string();
    CheckAppUpdate {
        has_update: compare_versions(&latest, &current) > 0,
        latest: Some(latest),
        current,
    }
}

/// 当前平台对应的 AppImage 资产名关键字（tauri 产物：homeTier_0.1.0_amd64.AppImage）
fn appimage_asset_keyword() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("_amd64.AppImage"),
        "aarch64" => Some("_aarch64.AppImage"),
        _ => None,
    }
}

/// 下载 AppImage 资产（直连 + 镜像双源，带进度回调）
async fn download_asset(
    url: &str,
    asset_name: &str,
    dest_dir: &std::path::Path,
    use_proxy: bool,
    mut on_progress: impl FnMut(f64),
) -> Result<std::path::PathBuf, String> {
    const MAX_RETRIES: u32 = 3;
    const BASE_DELAY_MS: u64 = 1000;

    let direct_url = url.to_string();
    let proxy_url = format!("{}/{}", github_mirror(), url);
    let urls: Vec<(&str, &str)> = if use_proxy {
        vec![(&proxy_url, "代理"), (&direct_url, "直连")]
    } else {
        vec![(&direct_url, "直连"), (&proxy_url, "代理")]
    };

    let client = reqwest::Client::builder()
        .user_agent(format!("homeTier/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let dest_path = dest_dir.join(asset_name);
    let tmp_path = dest_dir.join(format!("{}.tmp", asset_name));
    let mut last_err = String::new();

    for (url, label) in &urls {
        for attempt in 1..=MAX_RETRIES {
            let resp = match client.get(*url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = format!("{} 下载失败: {}", label, e);
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(std::time::Duration::from_millis(BASE_DELAY_MS * 2u64.pow(attempt - 1))).await;
                    }
                    continue;
                }
            };

            if !resp.status().is_success() {
                last_err = format!("{} 返回 HTTP {}", label, resp.status());
                if attempt < MAX_RETRIES {
                    tokio::time::sleep(std::time::Duration::from_millis(BASE_DELAY_MS * 2u64.pow(attempt - 1))).await;
                }
                continue;
            }

            let total = resp.content_length().unwrap_or(0);
            let mut file = tokio::fs::File::create(&tmp_path)
                .await
                .map_err(|e| format!("创建临时文件失败: {}", e))?;
            let mut downloaded: u64 = 0;
            let stream = resp.bytes_stream();
            tokio::pin!(stream);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| format!("读取下载数据失败: {}", e))?;
                file.write_all(&chunk).await.map_err(|e| format!("写入临时文件失败: {}", e))?;
                downloaded += chunk.len() as u64;
                if total > 0 {
                    on_progress(downloaded as f64 / total as f64 * 100.0);
                }
            }
            file.flush().await.map_err(|e| format!("刷新文件失败: {}", e))?;
            drop(file);

            tokio::fs::rename(&tmp_path, &dest_path)
                .await
                .map_err(|e| format!("重命名临时文件失败: {}", e))?;
            return Ok(dest_path);
        }
    }
    Err(format!("下载失败 (重试 {} 次, {} 个源): {}", MAX_RETRIES, urls.len(), last_err))
}

/// 从 checksums.txt 内容中解析指定资产名的 sha256
fn find_checksum(content: &str, asset_name: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?;
        if name == asset_name {
            Some(hash.to_lowercase())
        } else {
            None
        }
    })
}

/// 下载并校验 release 资产的 sha256 校验和文件
async fn fetch_checksum(assets: &[GitHubAsset], asset_name: &str) -> Result<String, String> {
    let checksum_asset = assets
        .iter()
        .find(|a| a.name == "checksums.txt")
        .ok_or_else(|| "发布缺少 checksums.txt 校验文件".to_string())?;
    let mirror_url = format!("{}/{}", github_mirror(), checksum_asset.browser_download_url);
    let client = http_client()?;
    let resp = client
        .get(&mirror_url)
        .send()
        .await
        .map_err(|e| format!("下载校验文件失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载校验文件返回 HTTP {}", resp.status()));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取校验文件失败: {}", e))?;
    find_checksum(&text, asset_name).ok_or_else(|| format!("校验文件中未找到 {} 的哈希", asset_name))
}

/// 核心升级逻辑（不依赖 tauri 环境，供桌面命令与 Web 路由共用）
pub async fn upgrade_app_inner(
    use_proxy: bool,
    mut on_progress: impl FnMut(f64),
) -> Result<AppUpdateOutcome, String> {
    // 仅支持 Linux AppImage 运行方式下自动替换
    if cfg!(not(target_os = "linux")) {
        return Ok(AppUpdateOutcome { action: "open_release".into() });
    }
    let appimage = std::env::var("APPIMAGE").unwrap_or_default();
    if appimage.is_empty() || !std::path::Path::new(&appimage).exists() {
        return Ok(AppUpdateOutcome { action: "open_release".into() });
    }

    let Some(keyword) = appimage_asset_keyword() else {
        return Ok(AppUpdateOutcome { action: "open_release".into() });
    };

    let release = fetch_latest_release().await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(keyword))
        .ok_or_else(|| format!("最新版本未找到 {} 平台安装包", keyword))?;

    on_progress(0.0);

    let expected = fetch_checksum(&release.assets, &asset.name).await?;

    // 与 AppImage 同目录下载，保证原子替换
    let appimage_path = std::path::PathBuf::from(&appimage);
    let dir = appimage_path
        .parent()
        .ok_or_else(|| "无法确定安装目录".to_string())?
        .to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let downloaded = download_asset(
        &asset.browser_download_url,
        &asset.name,
        &dir,
        use_proxy,
        &mut on_progress,
    )
    .await?;

    // sha256 校验
    let bytes = std::fs::read(&downloaded).map_err(|e| format!("读取安装包失败: {}", e))?;
    let actual = hex::encode(sha2::Sha256::digest(&bytes));
    if actual != expected {
        let _ = std::fs::remove_file(&downloaded);
        return Err(format!("安装包校验失败 (期望 {}，实际 {})", expected, actual));
    }

    // 赋予执行权限后原子替换
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&downloaded, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置执行权限失败: {}", e))?;
    }
    tokio::fs::rename(&downloaded, &appimage_path)
        .await
        .map_err(|e| format!("替换安装包失败: {}", e))?;

    Ok(AppUpdateOutcome { action: "installed".into() })
}

/// 桌面端命令：检查应用更新
#[tauri::command]
pub async fn check_app_update_cmd() -> CheckAppUpdate {
    check_app_update().await
}

/// 桌面端命令：下载并安装应用更新（带进度事件 app-download-progress）
#[tauri::command]
pub async fn upgrade_app_cmd(
    use_proxy: bool,
    app_handle: AppHandle,
) -> Result<AppUpdateOutcome, String> {
    upgrade_app_inner(use_proxy, |pct| {
        let _ = app_handle.emit("app-download-progress", pct);
    })
    .await
}
