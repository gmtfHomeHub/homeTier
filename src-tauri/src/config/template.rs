use std::path::{Path, PathBuf};

/// 配置文件模板定位
///
/// 按候选路径顺序探测，返回第一个存在的模板路径：
/// 1. 打包资源目录 resource_dir/homeTier.conf.example（生产模式）
/// 2. 仓库根 CARGO_MANIFEST_DIR/../homeTier.conf.example（开发模式）
pub fn locate_template(resource_dir: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = resource_dir {
        candidates.push(dir.join("homeTier.conf.example"));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("../homeTier.conf.example"));
    candidates.into_iter().find(|p| p.is_file())
}

/// 读取模板文件内容
pub fn read_template(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}
