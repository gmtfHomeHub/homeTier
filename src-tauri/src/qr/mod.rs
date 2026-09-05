//! 通用加密二维码传输层。
//!
//! 对业务 payload 透明：业务方传入 `event`（事件标识，如 `j_s` =
//! join_space）与任意 `data` 字节，本层负责「事件路由 + 加密 + URL 封装」。
//!
//! 链接格式：`homeTier://qr?v=1&d={base64url([nonce 12B][ciphertext+tag 16B])}`
//! 载荷流程：
//!   `[ver][event_len][event][data...]`
//!   → 自适应压缩（zstd level 3；不小于原文则走 raw）
//!   → AES-256-GCM 加密（密钥 = SHA-256("homeTier-qr-v1")）
//!   → base64url（无 padding）
//!
//! 业务编解码（如 `ShareInfo` 二进制）留在各自业务模块，本层不感知。

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use crate::utils::compress::{adaptive_compress, adaptive_decompress};
use crate::utils::encrypt::qr_key;

/// 通用二维码链接前缀。
pub const QR_LINK_PREFIX: &str = "homeTier://qr";
/// 链接版本号。
pub const QR_VERSION: u8 = 1;
/// 加密载荷内部版本标记（首字节）。
const PAYLOAD_VERSION: u8 = 1;

/// 链接参数名。
const PARAM_VERSION: &str = "v";
const PARAM_DATA: &str = "d";

/// 事件：扫码加入空间（join_space）。
pub const EVENT_JOIN_SPACE: &str = "j_s";
/// 事件：扫码导入应用（add_app）。
pub const EVENT_ADD_APP: &str = "a_a";

/// 单字节长度前缀的 event 最大字节数。
const MAX_EVENT_LEN: usize = 255;

/// 生成加密二维码链接。
///
/// `event` 为业务事件标识（≤255 字节 utf-8），`data` 为业务自定义二进制。
pub fn encrypt_qr(event: &str, data: &[u8]) -> Result<String, String> {
    let event_bytes = event.as_bytes();
    if event_bytes.is_empty() {
        return Err("event 不能为空".to_string());
    }
    if event_bytes.len() > MAX_EVENT_LEN {
        return Err(format!("event 长度超过 {} 字节", MAX_EVENT_LEN));
    }

    let mut payload = Vec::with_capacity(2 + event_bytes.len() + data.len());
    payload.push(PAYLOAD_VERSION);
    payload.push(event_bytes.len() as u8);
    payload.extend_from_slice(event_bytes);
    payload.extend_from_slice(data);

    let compressed = adaptive_compress(&payload);
    let enc = qr_key().encrypt(&compressed)?;
    let encoded = URL_SAFE_NO_PAD.encode(&enc);
    Ok(format!(
        "{}?{}={}&{}={}",
        QR_LINK_PREFIX, PARAM_VERSION, QR_VERSION, PARAM_DATA, encoded
    ))
}

/// 解析加密二维码链接，返回 `(event, data)`。
pub fn decrypt_qr(link: &str) -> Result<(String, Vec<u8>), String> {
    let url = url::Url::parse(link.trim()).map_err(|_| "无效的二维码链接".to_string())?;
    // url crate 会把 scheme 规范化为小写（homeTier -> hometier）
    if url.scheme().to_lowercase() != "hometier" || url.host_str() != Some("qr") {
        return Err("无效的二维码链接格式".to_string());
    }

    let expected_version = QR_VERSION.to_string();
    let version = url
        .query_pairs()
        .find(|(k, _)| k == PARAM_VERSION)
        .map(|(_, v)| v.into_owned());
    if version.as_deref() != Some(expected_version.as_str()) {
        return Err("不支持的二维码链接版本".to_string());
    }

    let data = url
        .query_pairs()
        .find(|(k, _)| k == PARAM_DATA)
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| "二维码链接缺少加密数据".to_string())?;

    let raw = URL_SAFE_NO_PAD
        .decode(&data)
        .map_err(|e| format!("二维码数据解码失败: {}", e))?;
    let plaintext = qr_key().decrypt(&raw)?;
    let payload = adaptive_decompress(&plaintext)?;

    let mut cur = payload.as_slice();
    if cur.is_empty() {
        return Err("二维码载荷为空".to_string());
    }
    let ver = cur[0];
    cur = &cur[1..];
    if ver != PAYLOAD_VERSION {
        return Err(format!("不支持的二维码载荷版本: {}", ver));
    }
    if cur.is_empty() {
        return Err("二维码数据截断: 缺少 event 长度".to_string());
    }
    let elen = cur[0] as usize;
    cur = &cur[1..];
    if cur.len() < elen {
        return Err("二维码数据截断: event 不完整".to_string());
    }
    let event = std::str::from_utf8(&cur[..elen])
        .map_err(|_| "二维码数据损坏: event 非 UTF-8".to_string())?
        .to_string();
    cur = &cur[elen..];
    let data = cur.to_vec();

    Ok((event, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_roundtrip_join_space() {
        let event = EVENT_JOIN_SPACE;
        let data = b"\x01\x00\x04test\x04sec!";
        let link = encrypt_qr(event, data).unwrap();
        assert!(link.starts_with("homeTier://qr?v=1&d="));
        let (ev, d) = decrypt_qr(&link).unwrap();
        assert_eq!(ev, event);
        assert_eq!(d, data);
    }

    #[test]
    fn qr_roundtrip_arbitrary_event() {
        let link = encrypt_qr("foo", b"").unwrap();
        let (ev, d) = decrypt_qr(&link).unwrap();
        assert_eq!(ev, "foo");
        assert!(d.is_empty());
    }

    #[test]
    fn rejects_empty_event() {
        assert!(encrypt_qr("", b"x").is_err());
    }

    #[test]
    fn rejects_bad_scheme() {
        assert!(decrypt_qr("https://homeTier/qr?v=1&d=xxx").is_err());
    }

    #[test]
    fn rejects_wrong_host() {
        // 旧 join host 应被拒
        assert!(decrypt_qr("homeTier://join?v=1&d=xxx").is_err());
    }

    #[test]
    fn rejects_missing_data_param() {
        assert!(decrypt_qr("homeTier://qr?v=1").is_err());
    }

    #[test]
    fn rejects_corrupted_payload() {
        let link = "homeTier://qr?v=1&d=AAAAAAAAAAAAAAAAAAAAAAAA";
        assert!(decrypt_qr(link).is_err());
    }

    #[test]
    fn rejects_wrong_version() {
        assert!(decrypt_qr("homeTier://qr?v=3&d=xxx").is_err());
    }
}
