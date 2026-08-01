use serde::Deserialize;

fn github_api() -> String {
    crate::config::get_str(crate::config::KEY_GITHUB_API, crate::config::DEFAULT_GITHUB_API)
}

fn github_mirror() -> String {
    crate::config::get_str(crate::config::KEY_GITHUB_MIRROR, crate::config::DEFAULT_GITHUB_MIRROR)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// 从 GitHub 获取可用版本列表
pub async fn fetch_available_versions() -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("homeTier/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(github_api())
        .query(&[("per_page", "20")])
        .send()
        .await
        .map_err(|e| format!("获取版本列表失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回错误: {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
    let releases: Vec<Release> = serde_json::from_str(&text)
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let mut versions: Vec<String> = releases
        .into_iter()
        .filter(|r| r.tag_name.starts_with('v'))
        .map(|r| r.tag_name.trim_start_matches('v').to_string())
        .collect();

    versions.sort_by(|a, b| {
        let a_parts: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let b_parts: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        for (x, y) in a_parts.iter().zip(b_parts.iter()) {
            if x != y {
                return y.cmp(x);
            }
        }
        a_parts.len().cmp(&b_parts.len()).reverse()
    });

    Ok(versions)
}

/// 获取指定版本的下载 URL（使用 ghproxy.top 镜像）
pub fn download_url(version: &str, platform: &str) -> String {
    format!(
        "{}/https://github.com/EasyTier/EasyTier/releases/download/v{}/easytier-{}-v{}.zip",
        github_mirror(), version, platform, version
    )
}

/// 获取指定版本的 SHA256 校验和（从 GitHub Release assets 中查找 checksums.txt）
pub async fn fetch_checksum(version: &str, platform: &str) -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("homeTier/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 尝试从 Release assets 获取 checksums.txt
    let url = format!("{}/repos/EasyTier/EasyTier/releases/tags/v{}", github_api(), version);
    let resp = client.get(&url).send().await
        .map_err(|e| format!("获取 Release 信息失败: {}", e))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let text = resp.text().await
        .map_err(|e| format!("读取 Release 响应失败: {}", e))?;
    let release: Release = serde_json::from_str(&text)
        .map_err(|e| format!("解析 Release 失败: {}", e))?;

    // 查找 checksums.txt 或同名 .sha256 文件
    for asset in &release.assets {
        if asset.name == "checksums.txt" || asset.name.ends_with(".sha256") {
            let checksum_url = format!("{}/{}", github_mirror(), asset.browser_download_url);
            let checksum_resp = client.get(&checksum_url).send().await
                .map_err(|e| format!("下载校验文件失败: {}", e))?;
            if checksum_resp.status().is_success() {
                let text = checksum_resp.text().await
                    .map_err(|e| format!("读取校验文件失败: {}", e))?;
                // 解析 checksums.txt，查找对应平台的哈希
                // 格式通常: <hash>  <filename>
                for line in text.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 && parts[1].contains(platform) {
                        return Ok(Some(parts[0].to_string()));
                    }
                }
            }
        }
    }

    Ok(None)
}
