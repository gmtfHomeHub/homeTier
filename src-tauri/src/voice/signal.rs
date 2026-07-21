/// WebRTC 信令处理器
/// 通过 EasyTier 虚拟网络内的 HTTP 服务传输信令
pub struct SignalHandler;

impl SignalHandler {
    /// 发送 SDP Offer
    pub async fn send_offer(target_ip: &str, port: u16, sdp: &str) -> Result<(), String> {
        let url = format!("http://{}:{}/signal/offer", target_ip, port);
        let client = reqwest::Client::new();
        client.post(&url)
            .header("Content-Type", "application/json")
            .body(sdp.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Signal error: {}", e))?;
        Ok(())
    }

    /// 发送 SDP Answer
    pub async fn send_answer(target_ip: &str, port: u16, sdp: &str) -> Result<(), String> {
        let url = format!("http://{}:{}/signal/answer", target_ip, port);
        let client = reqwest::Client::new();
        client.post(&url)
            .header("Content-Type", "application/json")
            .body(sdp.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Signal error: {}", e))?;
        Ok(())
    }

    /// 发送 ICE Candidate
    pub async fn send_ice(target_ip: &str, port: u16, candidate: &str) -> Result<(), String> {
        let url = format!("http://{}:{}/signal/ice", target_ip, port);
        let client = reqwest::Client::new();
        client.post(&url)
            .header("Content-Type", "application/json")
            .body(candidate.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Signal error: {}", e))?;
        Ok(())
    }
}