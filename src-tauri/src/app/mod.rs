pub mod daemon;
pub mod exit;
pub mod setup;
pub mod window;

/// 全局代理服务器，用于应用退出时关闭
use std::sync::{Arc, OnceLock};

pub static PROXY_SERVER: OnceLock<Arc<crate::proxy::ProxyServer>> = OnceLock::new();