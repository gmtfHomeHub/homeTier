//! 配置存储命令：本地/远端配置查询、下载、上传

use crate::config_store::{ConfigFile, ConfigFileMeta, ConfigStoreService};
use std::sync::Arc;

/// 查询本地某个配置的最新版本信息
#[tauri::command]
pub async fn get_config_version(
    config_store: tauri::State<'_, Arc<ConfigStoreService>>,
    name: String,
) -> Result<Option<ConfigFileMeta>, String> {
    Ok(config_store.store.get_meta(&name))
}

/// 下载本地某个配置的文件内容
#[tauri::command]
pub async fn download_config(
    config_store: tauri::State<'_, Arc<ConfigStoreService>>,
    name: String,
) -> Result<Option<ConfigFile>, String> {
    config_store
        .store
        .get_file(&name)
        .map_err(|e| e.to_string())
}

/// 上传/更新本地某个配置
#[tauri::command]
pub async fn upload_config(
    config_store: tauri::State<'_, Arc<ConfigStoreService>>,
    name: String,
    version: u32,
    content: Vec<u8>,
    timestamp: u64,
) -> Result<(), String> {
    let file = ConfigFile {
        name,
        version,
        content,
        timestamp,
        checksum: None,
    };
    config_store.store_local(file);
    Ok(())
}

/// 列出本地全部配置的版本信息
#[tauri::command]
pub async fn list_config_versions(
    config_store: tauri::State<'_, Arc<ConfigStoreService>>,
) -> Result<Vec<ConfigFileMeta>, String> {
    Ok(config_store.store.list_meta())
}

/// 清除本地配置缓存（测试/重置用）
#[tauri::command]
pub async fn clear_config_store(
    config_store: tauri::State<'_, Arc<ConfigStoreService>>,
) -> Result<(), String> {
    config_store.store.clear().map_err(|e| e.to_string())
}

/// 查询远端节点（虚拟 IP）的某个配置版本
#[tauri::command]
pub async fn get_remote_config_version(
    ip: String,
    name: String,
) -> Result<Option<ConfigFileMeta>, String> {
    let remote = crate::config_store::client::RemoteStore::new(&ip, crate::config_store::DEFAULT_PORT);
    remote.query_version(&name).await
}

/// 从远端节点下载某个配置
#[tauri::command]
pub async fn download_remote_config(
    ip: String,
    name: String,
) -> Result<Option<ConfigFile>, String> {
    let remote = crate::config_store::client::RemoteStore::new(&ip, crate::config_store::DEFAULT_PORT);
    remote.request_file(&name).await
}

/// 推送配置到远端节点
#[tauri::command]
pub async fn store_remote_config(ip: String, file: ConfigFile) -> Result<bool, String> {
    let remote = crate::config_store::client::RemoteStore::new(&ip, crate::config_store::DEFAULT_PORT);
    remote.store_file(&file).await
}
