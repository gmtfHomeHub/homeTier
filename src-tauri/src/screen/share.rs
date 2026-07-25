use std::sync::Arc;
use tokio::sync::RwLock;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;

/// 屏幕共享状态
pub struct ScreenShareEngine {
    pub is_sharing: Arc<RwLock<bool>>,
    pub viewers: Arc<RwLock<Vec<String>>>,
    /// WebRTC PeerConnection
    pub peer_connection: Arc<RwLock<Option<RTCPeerConnection>>>,
    /// 信令服务器端口
    pub signal_port: u16,
}

impl ScreenShareEngine {
    pub fn new() -> Self {
        Self {
            is_sharing: Arc::new(RwLock::new(false)),
            viewers: Arc::new(RwLock::new(Vec::new())),
            peer_connection: Arc::new(RwLock::new(None)),
            signal_port: 18200,
        }
    }

    /// 开始屏幕共享
    pub async fn start(&self) -> Result<(), String> {
        // 创建 WebRTC PeerConnection
        let api = APIBuilder::new().build();
        let config = RTCConfiguration::default();
        let peer_connection = api
            .new_peer_connection(config)
            .await
            .map_err(|e| format!("创建 PeerConnection 失败: {}", e))?;

        *self.peer_connection.write().await = Some(peer_connection);

        #[cfg(target_os = "macos")]
        {
            crate::log_info!("macOS 屏幕共享: 初始化 CGDisplayStream");
        }

        #[cfg(target_os = "windows")]
        {
            crate::log_info!("Windows 屏幕共享: 初始化 DXGI");
        }

        #[cfg(target_os = "linux")]
        {
            crate::log_info!("Linux 屏幕共享: 初始化 PipeWire");
        }

        *self.is_sharing.write().await = true;
        crate::log_info!("屏幕共享已启动");
        Ok(())
    }

    /// 停止屏幕共享
    pub async fn stop(&self) -> Result<(), String> {
        // 关闭 PeerConnection
        let mut pc = self.peer_connection.write().await;
        if let Some(conn) = pc.take() {
            let _ = conn.close().await;
        }

        *self.is_sharing.write().await = false;
        self.viewers.write().await.clear();
        crate::log_info!("屏幕共享已停止");
        Ok(())
    }

    /// 邀请成员查看
    pub async fn add_viewer(&self, member_id: String) {
        self.viewers.write().await.push(member_id);
    }

    pub async fn is_active(&self) -> bool {
        *self.is_sharing.read().await
    }
}