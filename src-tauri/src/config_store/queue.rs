//! 存储队列：去重 + 串行消费

use crate::config_store::store::{ConfigFile, ConfigStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 存储任务队列
///
/// 去重策略：以配置名称为键，新提交的 StoreFile 若与队列中同名任务冲突，
/// 则替换旧任务（只保留最新版本和时间戳）。消费时从 pending 取该名称下
/// 的最新任务执行，保证只有最新操作实际写入磁盘。
pub struct StoreQueue {
    pending: std::sync::Mutex<HashMap<String, ConfigFile>>,
    sender: mpsc::UnboundedSender<ConfigFile>,
}

impl StoreQueue {
    /// 同步创建队列（不含消费者任务），可在任意上下文调用
    pub fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<ConfigFile>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queue = Arc::new(Self {
            pending: std::sync::Mutex::new(HashMap::new()),
            sender,
        });
        (queue, receiver)
    }

    /// 异步启动消费者任务（必须在 Tokio 上下文中调用）
    pub fn start(self: &Arc<Self>, store: Arc<ConfigStore>, mut receiver: mpsc::UnboundedReceiver<ConfigFile>) {
        let q = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(task) = receiver.recv().await {
                let name = task.name.clone();
                let actual = q.pending.lock().unwrap().remove(&name);
                if let Some(actual) = actual {
                    if let Err(e) = store.store(actual) {
                        crate::log_error!(format!(
                            "[config_store] 存储失败 {}: {}",
                            name, e
                        ));
                    }
                }
            }
        });
    }

    /// 提交一个存储任务（同名旧任务被覆盖）
    pub fn submit(&self, file: ConfigFile) {
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(file.name.clone(), file.clone());
        }
        let _ = self.sender.send(file);
    }

    pub fn pending_len(&self) -> usize {
        self.pending.lock().unwrap().len()
    }
}
