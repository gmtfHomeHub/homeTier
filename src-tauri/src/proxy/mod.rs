pub mod hometier_protocol;
pub mod plugin;
pub mod plugins;
pub mod rewriter;
pub mod server;

pub use plugin::{PluginChain, ProxyHandler, ProxyPlugin, RequestContext};
pub use server::ProxyServer;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 代理 key → 源地址 映射
pub type ProxyKeyMap = Arc<RwLock<HashMap<String, String>>>;

/// 兜底缓存的当前活跃源地址（覆盖 fetch('/api') 等绝对路径动态请求）
pub type ActiveOrigin = Arc<RwLock<Option<String>>>;
