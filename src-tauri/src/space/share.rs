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
/// 链接版本号
const LINK_VERSION: u8 = 2;

/// 固定密钥 = SHA-256(版本标识)，直接用作 AES-256 密钥
fn fixed_key() -> [u8; 32] {
    sha256(SHARE_KEY_VERSION.as_bytes())
}

/// 加密分享载荷为 v2 链接
/// 链接格式: homeTier://join?v=2&d={base64url([nonce 12B][ciphertext])}
pub fn encrypt_share_payload(info: &ShareInfo) -> Result<String, String> {
    let payload = serde_json::to_string(info)
        .map_err(|e| format!("序列化分享信息失败: {}", e))?;
    let key = fixed_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {}", e))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, payload.as_bytes())
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

/// 解密分享链接（支持 v2 加密链接与 v1 明文链接）
pub fn decrypt_share_link(link: &str) -> Result<ShareInfo, String> {
    let url = url::Url::parse(link.trim()).map_err(|_| "无效的分享链接".to_string())?;
    if url.scheme() != "homeTier" || url.host_str() != Some("join") {
        return Err("无效的分享链接格式".to_string());
    }

    let version = url
        .query_pairs()
        .find(|(k, _)| k == PARAM_VERSION)
        .map(|(_, v)| v.into_owned());

    match version.as_deref() {
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
