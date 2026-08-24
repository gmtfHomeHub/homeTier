use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// 二进制版本元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BinaryMetadata {
    current_version: String,
    binary_dir: PathBuf,
}

/// EasyTier 二进制下载器
pub struct EasyTierDownloader {
    bin_dir: PathBuf,
    current_version_file: PathBuf,
    platform: String,
    /// Tauri 打包资源目录（用于内置二进制兜底）
    resource_dir: Option<PathBuf>,
}

impl EasyTierDownloader {
    pub fn new(app_data_dir: &Path, resource_dir: Option<&Path>) -> Self {
        let bin_dir = app_data_dir.join("bin");
        let current_version_file = bin_dir.join("current_version.json");
        let platform = Self::detect_platform();
        Self { bin_dir, current_version_file, platform, resource_dir: resource_dir.map(|p| p.to_path_buf()) }
    }

    /// 检测当前平台
    pub(crate) fn detect_platform() -> String {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        match (os, arch) {
            ("linux", "x86_64") => "linux-x86_64".into(),
            ("linux", "aarch64") => "linux-aarch64".into(),
            ("macos", "x86_64") => "macos-x86_64".into(),
            ("macos", "aarch64") => "macos-aarch64".into(),
            ("windows", "x86_64") => "windows-x86_64".into(),
            ("windows", "aarch64") => "windows-aarch64".into(),
            _ => format!("{}-{}", os, arch),
        }
    }

    /// 获取二进制文件路径
    pub fn binary_path(&self, version: &str) -> PathBuf {
        let binary_name = if cfg!(target_os = "windows") { "easytier-core.exe" } else { "easytier-core" };
        self.bin_dir.join(format!("easytier-core-{}", version)).join(&self.platform).join(binary_name)
    }

    /// 获取当前已安装的版本
    pub fn current_version(&self) -> Option<String> {
        let content = std::fs::read_to_string(&self.current_version_file).ok()?;
        let metadata: BinaryMetadata = serde_json::from_str(&content).ok()?;
        Some(metadata.current_version)
    }

    /// 获取当前二进制路径
    pub fn current_binary_path(&self) -> Option<PathBuf> {
        let version = self.current_version()?;
        let path = self.binary_path(&version);
        if path.exists() { Some(path) } else { None }
    }

    /// 确保二进制存在，返回路径
    pub async fn ensure_binary(&self) -> Result<PathBuf, String> {
        crate::log_debug!(format!("[EasyTierDownloader] ensure_binary 检查, bin_dir={}", self.bin_dir.display()));
        if let Some(path) = self.current_binary_path() {
            crate::log_info!(format!("[EasyTierDownloader] 找到已安装二进制: {}", path.display()));
            return Ok(path);
        }
        crate::log_warn!("[EasyTierDownloader] 未找到已安装二进制，尝试从打包资源解压内置二进制");
        if let Ok(path) = self.extract_bundled_binary().await {
            crate::log_info!(format!("[EasyTierDownloader] 已从打包资源安装内置二进制: {}", path.display()));
            return Ok(path);
        }
        Err("EasyTier 二进制未安装，请在设置中下载".into())
    }

    /// 从 Tauri 打包资源（resources/bin/）查找当前平台的 easytier-core 归档并安装
    async fn extract_bundled_binary(&self) -> Result<PathBuf, String> {
        let Some(resource_dir) = &self.resource_dir else {
            crate::log_debug!("[EasyTierDownloader] 无打包资源目录，跳过内置二进制解压");
            return Err("无打包资源目录".into());
        };

        let zip_name = format!("easytier-{}-v", self.platform);
        // 打包资源路径：resource_dir/resources/bin/、resource_dir/bin/ 或 resource_dir/（Tauri 可能按文件名扁平化）
        let mut candidates = vec![
            resource_dir.join("resources").join("bin"),
            resource_dir.join("bin"),
            resource_dir.clone(),
        ];

        for dir in candidates.drain(..) {
            crate::log_debug!(format!("[EasyTierDownloader] 扫描内置二进制目录: {}", dir.display()));
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with(&zip_name) || !name.ends_with(".zip") {
                    continue;
                }
                let version = name
                    .trim_start_matches(zip_name.as_str())
                    .trim_end_matches(".zip")
                    .trim_start_matches('v');
                if version.is_empty() {
                    continue;
                }
                crate::log_info!(format!(
                    "[EasyTierDownloader] 找到内置二进制归档: version={}, path={}",
                    version,
                    path.display()
                ));
                return self.install(version, BinarySource::LocalArchive(path)).await;
            }
        }

        Err(format!("打包资源中未找到 {} 平台的内置 easytier-core", self.platform))
    }

    /// 检查是否有新版本可用
    pub async fn check_update(&self, target_version: &str) -> Result<bool, String> {
        let current = self.current_version().unwrap_or_default();
        Ok(current != target_version)
    }

    /// 安装指定版本（从本地路径或远程下载）
    pub async fn install(&self, version: &str, source: BinarySource) -> Result<PathBuf, String> {
        let target_path = self.binary_path(version);

        if target_path.exists() {
            return Ok(target_path);
        }

        // 创建目标目录
        let parent = target_path.parent().ok_or("无效的二进制路径")?;
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;

        match source {
            BinarySource::LocalArchive(archive_path) => {
                let temp_dir = std::env::temp_dir().join(format!("easytier-extract-{}", version));
                let _ = std::fs::remove_dir_all(&temp_dir);
                std::fs::create_dir_all(&temp_dir)
                    .map_err(|e| format!("创建临时目录失败: {}", e))?;

                self.extract_archive(&archive_path, &temp_dir).await?;

                let binary_name = if cfg!(target_os = "windows") { "easytier-core.exe" } else { "easytier-core" };
                let found = Self::find_binary(&temp_dir, binary_name)
                    .ok_or_else(|| format!("在归档中未找到 {}", binary_name))?;

                std::fs::copy(&found, &target_path)
                    .map_err(|e| format!("复制二进制失败: {}", e))?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&target_path, std::fs::Permissions::from_mode(0o755))
                        .map_err(|e| format!("设置权限失败: {}", e))?;
                }

                // Windows: 将 packet.dll/wpcap.dll 复制到 easytier-core.exe 同目录
                // MSI 只把 DLL 放在 homeTier.exe 同目录，而 easytier-core.exe 在 {app_data}/bin/...
                #[cfg(target_os = "windows")]
                {
                    if let Some(resource_dir) = &self.resource_dir {
                        let target_dir = target_path.parent().unwrap_or(&self.bin_dir);
                        for dll_name in &["packet.dll", "wpcap.dll"] {
                            let mut copied = false;
                            for candidate_dir in &[
                                resource_dir.join("resources").join("bin"),
                                resource_dir.join("bin"),
                                resource_dir.clone(),
                            ] {
                                let src = candidate_dir.join(dll_name);
                                if src.exists() {
                                    if let Err(e) = std::fs::copy(&src, target_dir.join(dll_name)) {
                                        crate::log_warn!(format!("[EasyTierDownloader] 复制 {} 失败: {}", dll_name, e));
                                    } else {
                                        crate::log_info!(format!("[EasyTierDownloader] 已复制 {} 到 {}", dll_name, target_dir.display()));
                                    }
                                    copied = true;
                                    break;
                                }
                            }
                            if !copied {
                                crate::log_warn!(format!("[EasyTierDownloader] 未找到 {}，easytier-core 可能启动失败", dll_name));
                            }
                        }
                    }
                }

                let _ = std::fs::remove_dir_all(&temp_dir);
            }
            BinarySource::LocalBinary(binary_path) => {
                std::fs::copy(&binary_path, &target_path)
                    .map_err(|e| format!("复制二进制失败: {}", e))?;
                // Unix: 添加执行权限
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(0o755);
                    std::fs::set_permissions(&target_path, perms)
                        .map_err(|e| format!("设置权限失败: {}", e))?;
                }
            }
        }

        // 更新版本元数据
        self.set_current_version(version)?;

        crate::log_info!(format!("[EasyTierDownloader] 二进制安装完成, version={}, path={}", version, target_path.display()));
        Ok(target_path)
    }

    /// 卸载指定版本
    pub async fn uninstall(&self, version: &str) -> Result<(), String> {
        let dir = self.bin_dir.join(format!("easytier-core-{}", version));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("删除目录失败: {}", e))?;
            crate::log_info!("[EasyTierDownloader] 已卸载版本: {}", version);
        }
        if self.current_version().as_deref() == Some(version) {
            std::fs::remove_file(&self.current_version_file).ok();
        }
        Ok(())
    }

    /// 设置当前版本
    pub(crate) fn set_current_version(&self, version: &str) -> Result<(), String> {
        let metadata = BinaryMetadata {
            current_version: version.to_string(),
            binary_dir: self.binary_path(version).parent().unwrap_or(&self.bin_dir).to_path_buf(),
        };
        let json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("序列化元数据失败: {}", e))?;
        std::fs::write(&self.current_version_file, json)
            .map_err(|e| format!("写入版本文件失败: {}", e))?;
        Ok(())
    }

    /// 解压归档文件
    async fn extract_archive(&self, archive_path: &Path, target_dir: &Path) -> Result<(), String> {
        let archive_path = archive_path.to_path_buf();
        let target_dir = target_dir.to_path_buf();
        tokio::task::spawn_blocking(move || {
            if archive_path.extension().is_some_and(|e| e == "tar" || e == "gz") {
                Self::extract_tar_gz(&archive_path, &target_dir)
            } else if archive_path.extension().is_some_and(|e| e == "zip") {
                Self::extract_zip(&archive_path, &target_dir)
            } else {
                Err(format!("不支持的归档格式: {}", archive_path.display()))
            }
        })
        .await
        .map_err(|e| format!("解压任务失败: {}", e))?
    }

    fn extract_tar_gz(archive: &Path, target: &Path) -> Result<(), String> {
        let file = std::fs::File::open(archive).map_err(|e| format!("打开归档失败: {}", e))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(target).map_err(|e| format!("解压 tar.gz 失败: {}", e))?;
        Ok(())
    }

    fn extract_zip(archive: &Path, target: &Path) -> Result<(), String> {
        let file = std::fs::File::open(archive).map_err(|e| format!("打开归档失败: {}", e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("打开 zip 失败: {}", e))?;
        archive.extract(target).map_err(|e| format!("解压 zip 失败: {}", e))?;
        Ok(())
    }

    fn find_binary(dir: &Path, name: &str) -> Option<PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = Self::find_binary(&path, name) {
                    return Some(found);
                }
            } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
        }
        None
    }

    /// 列出已安装的版本
    pub fn list_installed(&self) -> Vec<String> {
        let mut versions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.bin_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("easytier-core-") {
                        let version = name.strip_prefix("easytier-core-").unwrap_or(name);
                        if entry.path().join(&self.platform).exists() {
                            versions.push(version.to_string());
                        }
                    }
                }
            }
        }
        versions.sort();
        versions
    }

    /// 获取 bin 目录大小（字节）
    pub fn disk_usage(&self) -> u64 {
        Self::dir_size(&self.bin_dir)
    }

    fn dir_size(path: &Path) -> u64 {
        let mut total = 0;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    total += Self::dir_size(&p);
                } else if let Ok(meta) = std::fs::metadata(&p) {
                    total += meta.len();
                }
            }
        }
        total
    }

    /// 从源码编译并安装
    pub async fn build_from_source(&self, source_dir: &std::path::Path) -> Result<String, String> {
        let cargo_toml = source_dir.join("easytier").join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(format!("EasyTier 库 Cargo.toml 未找到: {}", cargo_toml.display()));
        }

        let content = std::fs::read_to_string(&cargo_toml)
            .map_err(|e| format!("读取 Cargo.toml 失败: {}", e))?;
        let version = content
            .lines()
            .find(|l| l.trim().starts_with("version"))
            .and_then(|l| l.split('=').nth(1))
            .map(|s| s.trim().trim_matches('"').trim().to_string())
            .ok_or_else(|| "无法从 Cargo.toml 解析版本号".to_string())?;

        crate::log_info!(format!("[EasyTierDownloader] 开始源码编译: version={}, dir={}", version, source_dir.display()));

        let status = tokio::process::Command::new("cargo")
            .args(["build", "--package", "easytier-core", "--release"])
            .current_dir(source_dir)
            .status()
            .await
            .map_err(|e| format!("执行 cargo build 失败: {}", e))?;

        if !status.success() {
            return Err("cargo build 编译失败，请检查编译错误".into());
        }

        let binary_name = if cfg!(target_os = "windows") { "easytier-core.exe" } else { "easytier-core" };
        let built_binary = source_dir.join("target").join("release").join(binary_name);
        if !built_binary.exists() {
            return Err(format!("编译产物未找到: {}", built_binary.display()));
        }

        let target_dir = self.binary_path(&version).parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&target_dir).map_err(|e| format!("创建目标目录失败: {}", e))?;
        std::fs::copy(&built_binary, self.binary_path(&version))
            .map_err(|e| format!("复制二进制失败: {}", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.binary_path(&version), std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("设置权限失败: {}", e))?;
        }

        self.set_current_version(&version)?;

        crate::log_info!(format!("[EasyTierDownloader] 源码编译完成: version={}", version));
        Ok(version)
    }

    /// 从 GitHub Releases 下载并安装指定版本（带 ghproxy.top 镜像加速、指数退避重试、直连回退、原子写入）
    pub async fn download_from_github(&self, version: &str) -> Result<PathBuf, String> {
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 1000;

        let filename = format!("easytier-{}-v{}.zip", self.platform, version);
        let direct_url = format!(
            "https://github.com/EasyTier/EasyTier/releases/download/v{}/{}",
            version, filename
        );
        let mirror = crate::config::get_str(crate::config::KEY_GITHUB_MIRROR, crate::config::DEFAULT_GITHUB_MIRROR);
        let mirror_url = format!("{}/{}", mirror, direct_url);

        let urls = [(mirror_url.as_str(), "镜像"), (direct_url.as_str(), "直连")];

        crate::log_info!(format!("[EasyTierDownloader] 开始下载: version={}, platform={}", version, self.platform));

        let client = reqwest::Client::builder()
            .user_agent("homeTier/0.1.0")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let mut last_err = String::new();

        for (url, label) in &urls {
            for attempt in 1..=MAX_RETRIES {
                crate::log_info!(format!("[EasyTierDownloader] 尝试 {} 下载 (第 {}/{})", label, attempt, MAX_RETRIES));

                match client.get(*url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let temp_dir = std::env::temp_dir().join(format!("easytier-dl-{}", version));
                        let _ = std::fs::create_dir_all(&temp_dir);
                        let final_path = temp_dir.join(&filename);

                        let tmp_path = temp_dir.join(format!("{}.tmp", filename));
                        let bytes = match resp.bytes().await {
                            Ok(b) => b,
                            Err(e) => {
                                last_err = format!("{} 读取数据失败: {}", label, e);
                                let _ = std::fs::remove_dir_all(&temp_dir);
                                continue;
                            }
                        };
                        if bytes.is_empty() {
                            last_err = format!("{} 下载文件为空", label);
                            let _ = std::fs::remove_dir_all(&temp_dir);
                            continue;
                        }
                        if let Err(e) = tokio::fs::write(&tmp_path, &bytes).await {
                            last_err = format!("{} 写入临时文件失败: {}", label, e);
                            let _ = std::fs::remove_dir_all(&temp_dir);
                            continue;
                        }
                        let _ = tokio::fs::rename(&tmp_path, &final_path).await;

                        crate::log_info!(format!("[EasyTierDownloader] {}下载完成, 大小: {} bytes", label, bytes.len()));

                        match self.install(version, BinarySource::LocalArchive(final_path.clone())).await {
                            Ok(path) => {
                                let _ = std::fs::remove_dir_all(&temp_dir);
                                return Ok(path);
                            }
                            Err(e) => {
                                last_err = format!("{} 安装失败: {}", label, e);
                            }
                        }
                        let _ = std::fs::remove_dir_all(&temp_dir);
                    }
                    Ok(resp) => {
                        last_err = format!("{} 返回 HTTP {}", label, resp.status());
                    }
                    Err(e) => {
                        last_err = format!("{} 下载失败: {}", label, e);
                    }
                }

                if attempt < MAX_RETRIES {
                    let delay = BASE_DELAY_MS * 2_u64.pow(attempt - 1);
                    crate::log_info!(format!("[EasyTierDownloader] {}ms 后重试...", delay));
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
            }
        }

        Err(format!("下载失败 (重试 {} 次, {} 个源): {}", MAX_RETRIES, urls.len(), last_err))
    }
}

/// 二进制来源
pub enum BinarySource {
    /// 本地归档（tar.gz / zip）
    LocalArchive(PathBuf),
    /// 本地二进制文件
    LocalBinary(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_platform() {
        let platform = EasyTierDownloader::detect_platform();
        assert!(!platform.is_empty());
    }
}
