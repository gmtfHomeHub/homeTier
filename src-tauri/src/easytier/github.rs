use serde::Deserialize;

const GITHUB_API: &str = "https://api.github.com/repos/EasyTier/EasyTier/releases";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// 从 GitHub 获取可用版本列表
pub async fn fetch_available_versions() -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("homeTier/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(GITHUB_API)
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

/// 获取指定版本的下载 URL
pub fn download_url(version: &str, platform: &str) -> String {
    format!(
        "https://github.com/EasyTier/EasyTier/releases/download/v{}/easytier-{}-v{}.zip",
        version, platform, version
    )
}
