use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;

/// 全局信号消息队列（按路径缓冲）
static SIGNAL_QUEUE: Lazy<Arc<Mutex<VecDeque<SignalMessage>>>> =
    Lazy::new(|| Arc::new(Mutex::new(VecDeque::new())));

pub struct SignalMessage {
    pub path: SignalPath,
    pub body: String,
}

pub enum SignalPath {
    Offer,
    Answer,
    Ice,
}

/// WebRTC 信令处理器 — 收发双向
pub struct SignalHandler;

impl SignalHandler {
    pub async fn send_offer(target_ip: &str, port: u16, sdp: &str) -> Result<(), String> {
        let url = format!("http://{}:{}/signal/offer", target_ip, port);
        reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body(sdp.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Signal error: {}", e))?;
        Ok(())
    }

    pub async fn send_answer(target_ip: &str, port: u16, sdp: &str) -> Result<(), String> {
        let url = format!("http://{}:{}/signal/answer", target_ip, port);
        reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body(sdp.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Signal error: {}", e))?;
        Ok(())
    }

    pub async fn send_ice(target_ip: &str, port: u16, candidate: &str) -> Result<(), String> {
        let url = format!("http://{}:{}/signal/ice", target_ip, port);
        reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body(candidate.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Signal error: {}", e))?;
        Ok(())
    }
}

// 入站信令队列操作

pub async fn ingest_offer(body: &str) {
    let mut q = SIGNAL_QUEUE.lock().await;
    q.push_back(SignalMessage { path: SignalPath::Offer, body: body.to_string() });
}

pub async fn ingest_answer(body: &str) {
    let mut q = SIGNAL_QUEUE.lock().await;
    q.push_back(SignalMessage { path: SignalPath::Answer, body: body.to_string() });
}

pub async fn ingest_ice(body: &str) {
    let mut q = SIGNAL_QUEUE.lock().await;
    q.push_back(SignalMessage { path: SignalPath::Ice, body: body.to_string() });
}

pub async fn poll_signal<F>(pred: F, max_wait_ms: u64, space_id: &str) -> Option<SignalMessage>
where
    F: Fn(&SignalMessage) -> bool,
{
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(max_wait_ms);
    loop {
        let mut q = SIGNAL_QUEUE.lock().await;
        let pos = q.iter().position(&pred);
        if let Some(idx) = pos {
            let msg = q.remove(idx);
            return msg;
        }
        drop(q);

        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}