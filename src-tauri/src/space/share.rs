use crate::crypto::{sha256, NONCE_LEN};
use crate::types::ShareInfo;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::Rng;

/// 分享链接前缀
pub const SHARE_LINK_PREFIX: &str = "homeTier://join";
/// 固定密钥版本标识（无交互分享，密钥由该标识 SHA-256 派生）
pub const SHARE_KEY_VERSION: &str = "homeTier-share-link-v2";
/// 加密载荷参数名
const PARAM_VERSION: &str = "v";
const PARAM_DATA: &str = "d";
/// 链接版本号（v3: 加密+压缩；v2: 仅加密；v1: 明文）
const LINK_VERSION: u8 = 3;
/// zstd 压缩级别（速度优先，分享载荷通常很小）
const ZSTD_LEVEL: i32 = 3;

/// 固定密钥 = SHA-256(版本标识)，直接用作 AES-256 密钥
fn fixed_key() -> [u8; 32] {
    sha256(SHARE_KEY_VERSION.as_bytes())
}

/// 加密分享载荷为 v3 链接（zstd 压缩 + AES-256-GCM）
/// 链接格式: homeTier://join?v=3&d={base64url([nonce 12B][ciphertext of compressed payload])}
pub fn encrypt_share_payload(info: &ShareInfo) -> Result<String, String> {
    let payload = serde_json::to_string(info)
        .map_err(|e| format!("序列化分享信息失败: {}", e))?;
    let compressed = zstd::stream::encode_all(payload.as_bytes(), ZSTD_LEVEL)
        .map_err(|e| format!("压缩分享数据失败: {}", e))?;
    let key = fixed_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {}", e))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, &*compressed)
        .map_err(|e| format!("Encrypt error: {}", e))?;

    let mut data = nonce_bytes.to_vec();
    data.extend(ciphertext);
    let encoded = URL_SAFE_NO_PAD.encode(&data);
    Ok(format!(
        "{}?{}={}&{}={}",
        SHARE_LINK_PREFIX,
        PARAM_VERSION,
        LINK_VERSION,
        PARAM_DATA,
        encoded
    ))
}

/// 解密分享链接（支持 v3 加密+压缩、v2 加密与 v1 明文链接）
pub fn decrypt_share_link(link: &str) -> Result<ShareInfo, String> {
    let url = url::Url::parse(link.trim()).map_err(|_| "无效的分享链接".to_string())?;
    // 注意：url crate 会把 scheme 规范化为小写（homeTier -> hometier）
    if url.scheme().to_lowercase() != "hometier" || url.host_str() != Some("join") {
        return Err("无效的分享链接格式".to_string());
    }

    let version = url
        .query_pairs()
        .find(|(k, _)| k == PARAM_VERSION)
        .map(|(_, v)| v.into_owned());

    match version.as_deref() {
        Some(v) if v == "3" => {
            let data = url
                .query_pairs()
                .find(|(k, _)| k == PARAM_DATA)
                .map(|(_, v)| v.into_owned())
                .ok_or_else(|| "分享链接缺少加密数据".to_string())?;
            decrypt_v3(&data)
        }
        Some(v) if v == "2" => {
            let data = url
                .query_pairs()
                .find(|(k, _)| k == PARAM_DATA)
                .map(|(_, v)| v.into_owned())
                .ok_or_else(|| "分享链接缺少加密数据".to_string())?;
            decrypt_v2(&data)
        }
        _ => decrypt_v1(&url),
    }
}

/// 解密 v3 加密+压缩载荷
fn decrypt_v3(data: &str) -> Result<ShareInfo, String> {
    let raw = URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|e| format!("分享数据解码失败: {}", e))?;
    if raw.len() < NONCE_LEN {
        return Err("无效的加密数据".to_string());
    }

    let key = fixed_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {}", e))?;
    let nonce = Nonce::from_slice(&raw[..NONCE_LEN]);
    let plaintext = cipher
        .decrypt(nonce, &raw[NONCE_LEN..])
        .map_err(|_| "解密失败，链接无效或已损坏".to_string())?;

    let payload = zstd::stream::decode_all(std::io::Cursor::new(plaintext))
        .map_err(|_| "解压分享数据失败，链接无效或已损坏".to_string())?;

    serde_json::from_slice::<ShareInfo>(&payload)
        .map_err(|e| format!("解析分享信息失败: {}", e))
}

/// 解密 v2 加密载荷
fn decrypt_v2(data: &str) -> Result<ShareInfo, String> {
    let raw = URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|e| format!("分享数据解码失败: {}", e))?;
    if raw.len() < NONCE_LEN {
        return Err("无效的加密数据".to_string());
    }

    let key = fixed_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {}", e))?;
    let nonce = Nonce::from_slice(&raw[..NONCE_LEN]);
    let plaintext = cipher
        .decrypt(nonce, &raw[NONCE_LEN..])
        .map_err(|_| "解密失败，链接无效或已损坏".to_string())?;

    serde_json::from_slice::<ShareInfo>(&plaintext)
        .map_err(|e| format!("解析分享信息失败: {}", e))
}

/// 解析 v1 明文链接: homeTier://join?name=X&secret=Y
fn decrypt_v1(url: &url::Url) -> Result<ShareInfo, String> {
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
    let network_name = pairs
        .get("name")
        .map(|v| v.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "分享链接缺少网络名称".to_string())?;
    let network_secret = pairs
        .get("secret")
        .map(|v| v.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "分享链接缺少网络密钥".to_string())?;
    Ok(ShareInfo {
        network_name,
        network_secret,
        host_hint: None,
        virtual_ip: None,
        dhcp: None,
        peer_urls: Vec::new(),
        listener_urls: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_link_roundtrip_v3() {
        let info = ShareInfo {
            network_name: "测试空间".to_string(),
            network_secret: "secret-abc-123".to_string(),
            host_hint: Some("192.168.1.100".to_string()),
            virtual_ip: Some("10.144.144.10".to_string()),
            dhcp: Some(true),
            peer_urls: vec![
                "tcp://public.example.com:11010".to_string(),
                "tcp://public.example.com:11011".to_string(),
                "tcp://public.example.com:11012".to_string(),
            ],
            listener_urls: vec!["tcp://0.0.0.0:11010".to_string()],
        };
        let link = encrypt_share_payload(&info).unwrap();
        assert!(link.starts_with("homeTier://join?v=3&d="));
        let decrypted = decrypt_share_link(&link).unwrap();
        assert_eq!(decrypted.network_name, info.network_name);
        assert_eq!(decrypted.network_secret, info.network_secret);
        assert_eq!(decrypted.peer_urls, info.peer_urls);
        assert_eq!(decrypted.virtual_ip, info.virtual_ip);
    }

    #[test]
    fn share_link_v1_fallback() {
        let link = "homeTier://join?name=legacy&secret=legacy-secret";
        let decrypted = decrypt_share_link(link).unwrap();
        assert_eq!(decrypted.network_name, "legacy");
        assert_eq!(decrypted.network_secret, "legacy-secret");
    }
}
