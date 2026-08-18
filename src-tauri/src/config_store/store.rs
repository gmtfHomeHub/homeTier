//! 本地存储管理：读写、文件锁、版本索引

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

/// 配置文件的版本元数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigFileMeta {
    pub name: String,
    pub version: u32,
    pub timestamp: u64,
    pub checksum: Option<String>,
}

/// 完整的配置文件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigFile {
    pub name: String,
    pub version: u32,
    pub content: Vec<u8>,
    pub timestamp: u64,
    pub checksum: Option<String>,
}

/// 存储错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// 磁盘上的版本不低于待写入版本（避免回退）
    VersionConflict,
    Io(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::VersionConflict => write!(f, "版本冲突：磁盘版本不低于待写入版本"),
            StoreError::Io(e) => write!(f, "IO 错误: {}", e),
        }
    }
}

impl std::error::Error for StoreError {}

/// 本地配置存储
///
/// 目录布局：
///   {root}/configs/{name}.json   — 配置内容
///   {root}/versions.json         — 版本索引（name -> meta）
///   {root}/configs/{name}.lock   — 写入锁（fs2 独占锁）
pub struct ConfigStore {
    root: PathBuf,
    configs_dir: PathBuf,
    versions: RwLock<HashMap<String, ConfigFileMeta>>,
}

impl ConfigStore {
    pub fn new(root: PathBuf) -> Self {
        let configs_dir = root.join("configs");
        let _ = std::fs::create_dir_all(&configs_dir);
        let versions = Self::load_versions(&root);
        Self {
            root,
            configs_dir,
            versions: RwLock::new(versions),
        }
    }

    fn load_versions(root: &PathBuf) -> HashMap<String, ConfigFileMeta> {
        let path = root.join("versions.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<ConfigFileMeta>>(&s).ok())
            .map(|v| v.into_iter().map(|m| (m.name.clone(), m)).collect())
            .unwrap_or_default()
    }

    /// 查询某个配置的最新版本元数据
    pub fn get_meta(&self, name: &str) -> Option<ConfigFileMeta> {
        self.versions.read().unwrap().get(name).cloned()
    }

    /// 列出本地全部配置元数据
    pub fn list_meta(&self) -> Vec<ConfigFileMeta> {
        self.versions
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    /// 读取某个配置的完整内容（本地磁盘）
    pub fn get_file(&self, name: &str) -> Result<Option<ConfigFile>, StoreError> {
        let meta = match self.get_meta(name) {
            Some(m) => m,
            None => return Ok(None),
        };
        let path = self.configs_dir.join(format!("{}.json", name));
        let content = std::fs::read(&path)
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(Some(ConfigFile {
            name: name.to_string(),
            version: meta.version,
            content,
            timestamp: meta.timestamp,
            checksum: meta.checksum,
        }))
    }

    /// 存储一个配置文件：
    /// 1. 对 {name}.lock 加独占锁
    /// 2. 检查磁盘版本（不低于待写入版本则拒绝）
    /// 3. 写入内容 + 更新 versions.json
    pub fn store(&self, file: ConfigFile) -> Result<(), StoreError> {
        let lock_path = self.configs_dir.join(format!("{}.lock", file.name));
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)
            .map_err(|e| StoreError::Io(e.to_string()))?;
        fs2::FileExt::lock_exclusive(&lock_file)
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let result = (|| {
            if let Some(meta) = self.get_meta(&file.name) {
                if meta.version >= file.version {
                    return Err(StoreError::VersionConflict);
                }
            }
            let path = self.configs_dir.join(format!("{}.json", file.name));
            std::fs::write(&path, &file.content)
                .map_err(|e| StoreError::Io(e.to_string()))?;
            self.versions
                .write()
                .unwrap()
                .insert(file.name.clone(), file.to_meta());
            self.persist_versions()?;
            Ok(())
        })();

        let _ = fs2::FileExt::unlock(&lock_file);
        result
    }

    fn persist_versions(&self) -> Result<(), StoreError> {
        let metas: Vec<ConfigFileMeta> = self.list_meta();
        let json = serde_json::to_vec(&metas).map_err(|e| StoreError::Io(e.to_string()))?;
        std::fs::write(self.root.join("versions.json"), json)
            .map_err(|e| StoreError::Io(e.to_string()))
    }

    /// 删除本地配置缓存（设计文档：提供命令清除本地配置缓存）
    pub fn clear(&self) -> Result<(), StoreError> {
        let _ = std::fs::remove_file(self.root.join("versions.json"));
        if let Ok(entries) = std::fs::read_dir(&self.configs_dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let mut versions = self.versions.write().unwrap();
        versions.clear();
        Ok(())
    }
}

impl ConfigFile {
    pub fn to_meta(&self) -> ConfigFileMeta {
        ConfigFileMeta {
            name: self.name.clone(),
            version: self.version,
            timestamp: self.timestamp,
            checksum: self.checksum.clone(),
        }
    }
}
