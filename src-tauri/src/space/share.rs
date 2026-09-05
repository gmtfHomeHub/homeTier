//! 空间分享链接编解码。
//!
//! 链接格式：`homeTier://join?v=1&d={base64url([nonce 12][ciphertext+tag])}`
//! 载荷流程：
//!   ShareInfo → **二进制编码（小端序，单字节长度前缀，可选字段 bitmask）**
//!           → **自适应压缩**（zstd level 3；压缩后不小于原文则走 raw）
//!           → **AES-256-GCM 加密**（密钥 = SHA-256("homeTier-share-link-v1")）
//!           → base64url（无 padding）
//!
//! 二进制布局（相对 `encode_share_binary` 输出）：
//!   offset  size  field
//!   0       1     version       = 1
//!   1       1     flags         bitmask
//!                                0x01=name        0x02=host_hint
//!                                0x04=virtual_ip  0x08=dhcp
//!   2       1+L   network_name  utf-8, L ∈ 0..255
//!   ...     1+L2  network_secret
//!   ...     1+L   name          仅当 flags & 0x01
//!   ...     1+L   host_hint     仅当 flags & 0x02
//!   ...     1+L   virtual_ip    仅当 flags & 0x04
//!   ...     1     dhcp          0/1，仅当 flags & 0x08
//!   ...     1     peer_count    0..255
//!   ...     N×(1+L) peer_urls
//!   ...     1     listener_count
//!   ...     M×(1+L) listener_urls

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use crate::types::ShareInfo;
use crate::utils::compress::{adaptive_compress, adaptive_decompress};
use crate::utils::encrypt::share_key;

/// 分享链接前缀。
pub const SHARE_LINK_PREFIX: &str = "homeTier://join";
/// 链接版本号（仅 v1）。
pub const LINK_VERSION: u8 = 1;
/// 二进制 payload 版本标记（首字节）。
const PAYLOAD_VERSION: u8 = 1;

/// 分享链接参数名。
const PARAM_VERSION: &str = "v";
const PARAM_DATA: &str = "d";

// ---- 二进制可选字段 bitmask ----
const FLAG_NAME: u8 = 0x01;
const FLAG_HOST_HINT: u8 = 0x02;
const FLAG_VIRTUAL_IP: u8 = 0x04;
const FLAG_DHCP: u8 = 0x08;

/// 单字节长度前缀的字符串最大字节数。
const MAX_STRING_LEN: usize = 255;

// ---- 二进制编码 ----

fn write_string(out: &mut Vec<u8>, s: &str, field: &str) -> Result<(), String> {
    let bytes = s.as_bytes();
    if bytes.len() > MAX_STRING_LEN {
        return Err(format!("{} 长度超过 {} 字节: {}", field, MAX_STRING_LEN, s));
    }
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
    Ok(())
}

/// 将 `ShareInfo` 编码为小端序二进制字节流。
pub fn encode_share_binary(info: &ShareInfo) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(256);
    out.push(PAYLOAD_VERSION);

    let mut flags: u8 = 0;
    if info.name.is_some() {
        flags |= FLAG_NAME;
    }
    if info.host_hint.is_some() {
        flags |= FLAG_HOST_HINT;
    }
    if info.virtual_ip.is_some() {
        flags |= FLAG_VIRTUAL_IP;
    }
    if info.dhcp.is_some() {
        flags |= FLAG_DHCP;
    }
    out.push(flags);

    write_string(&mut out, &info.network_name, "network_name")?;
    write_string(&mut out, &info.network_secret, "network_secret")?;
    if let Some(s) = &info.name {
        write_string(&mut out, s, "name")?;
    }
    if let Some(s) = &info.host_hint {
        write_string(&mut out, s, "host_hint")?;
    }
    if let Some(s) = &info.virtual_ip {
        write_string(&mut out, s, "virtual_ip")?;
    }
    if let Some(b) = info.dhcp {
        out.push(if b { 1 } else { 0 });
    }

    if info.peer_urls.len() > 255 {
        return Err("peer_urls 数量超过 255".to_string());
    }
    out.push(info.peer_urls.len() as u8);
    for s in &info.peer_urls {
        write_string(&mut out, s, "peer_url")?;
    }

    if info.listener_urls.len() > 255 {
        return Err("listener_urls 数量超过 255".to_string());
    }
    out.push(info.listener_urls.len() as u8);
    for s in &info.listener_urls {
        write_string(&mut out, s, "listener_url")?;
    }

    Ok(out)
}

// ---- 二进制解码 ----

fn read_string(input: &mut &[u8]) -> Result<String, String> {
    if input.is_empty() {
        return Err("分享数据截断: 缺少长度字节".to_string());
    }
    let len = input[0] as usize;
    *input = &input[1..];
    if input.len() < len {
        return Err("分享数据截断: 字符串长度不匹配".to_string());
    }
    let s = std::str::from_utf8(&input[..len])
        .map_err(|_| "分享数据损坏: 字符串非 UTF-8".to_string())?
        .to_string();
    *input = &input[len..];
    Ok(s)
}

/// 从 `encode_share_binary` 的输出解码回 `ShareInfo`。
pub fn decode_share_binary(data: &[u8]) -> Result<ShareInfo, String> {
    let mut cur = data;

    if cur.is_empty() {
        return Err("分享数据为空".to_string());
    }
    let version = cur[0];
    cur = &cur[1..];
    if version != PAYLOAD_VERSION {
        return Err(format!("不支持的分享载荷版本: {}", version));
    }

    if cur.is_empty() {
        return Err("分享数据截断: 缺少 flags".to_string());
    }
    let flags = cur[0];
    cur = &cur[1..];

    let network_name = read_string(&mut cur)?;
    let network_secret = read_string(&mut cur)?;

    let name = if flags & FLAG_NAME != 0 {
        Some(read_string(&mut cur)?)
    } else {
        None
    };
    let host_hint = if flags & FLAG_HOST_HINT != 0 {
        Some(read_string(&mut cur)?)
    } else {
        None
    };
    let virtual_ip = if flags & FLAG_VIRTUAL_IP != 0 {
        Some(read_string(&mut cur)?)
    } else {
        None
    };
    let dhcp = if flags & FLAG_DHCP != 0 {
        if cur.is_empty() {
            return Err("分享数据截断: 缺少 dhcp 值".to_string());
        }
        let b = cur[0];
        cur = &cur[1..];
        match b {
            0 => Some(false),
            1 => Some(true),
            _ => return Err(format!("非法 dhcp 值: 0x{:02x}", b)),
        }
    } else {
        None
    };

    if cur.is_empty() {
        return Err("分享数据截断: 缺少 peer_count".to_string());
    }
    let peer_count = cur[0] as usize;
    cur = &cur[1..];
    let mut peer_urls = Vec::with_capacity(peer_count);
    for _ in 0..peer_count {
        peer_urls.push(read_string(&mut cur)?);
    }

    if cur.is_empty() {
        return Err("分享数据截断: 缺少 listener_count".to_string());
    }
    let listener_count = cur[0] as usize;
    cur = &cur[1..];
    let mut listener_urls = Vec::with_capacity(listener_count);
    for _ in 0..listener_count {
        listener_urls.push(read_string(&mut cur)?);
    }

    Ok(ShareInfo {
        name,
        network_name,
        network_secret,
        host_hint,
        virtual_ip,
        dhcp,
        peer_urls,
        listener_urls,
    })
}

// ---- 公开 API ----

/// 生成分享链接（v1: 二进制编码 + 自适应压缩 + AES-256-GCM 加密）。
pub fn encrypt_share_payload(info: &ShareInfo) -> Result<String, String> {
    let binary = encode_share_binary(info)?;
    let payload = adaptive_compress(&binary);
    let enc = share_key().encrypt(&payload)?;
    let encoded = URL_SAFE_NO_PAD.encode(&enc);
    Ok(format!(
        "{}?{}={}&{}={}",
        SHARE_LINK_PREFIX,
        PARAM_VERSION,
        LINK_VERSION,
        PARAM_DATA,
        encoded
    ))
}

/// 解析分享链接。仅支持 v1。
pub fn decrypt_share_link(link: &str) -> Result<ShareInfo, String> {
    let url = url::Url::parse(link.trim()).map_err(|_| "无效的分享链接".to_string())?;
    // 注意：url crate 会把 scheme 规范化为小写（homeTier -> hometier）
    if url.scheme().to_lowercase() != "hometier" || url.host_str() != Some("join") {
        return Err("无效的分享链接格式".to_string());
    }

    let expected_version = LINK_VERSION.to_string();
    let version = url
        .query_pairs()
        .find(|(k, _)| k == PARAM_VERSION)
        .map(|(_, v)| v.into_owned());
    if version.as_deref() != Some(expected_version.as_str()) {
        return Err("不支持的分享链接版本".to_string());
    }

    let data = url
        .query_pairs()
        .find(|(k, _)| k == PARAM_DATA)
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| "分享链接缺少加密数据".to_string())?;

    let raw = URL_SAFE_NO_PAD
        .decode(&data)
        .map_err(|e| format!("分享数据解码失败: {}", e))?;
    let plaintext = share_key().decrypt(&raw)?;
    let binary = adaptive_decompress(&plaintext)?;
    decode_share_binary(&binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_full() -> ShareInfo {
        ShareInfo {
            network_name: "home-tier-net".to_string(),
            network_secret: "secret-xyz-456-789".to_string(),
            host_hint: None,
            virtual_ip: Some("10.144.144.10".to_string()),
            dhcp: Some(false),
            name: Some("我的家网络".to_string()),
            peer_urls: vec![
                "tcp://public.example.com:11010".to_string(),
                "tcp://public.example.com:11011".to_string(),
                "tcp://public.example.com:11012".to_string(),
            ],
            listener_urls: vec!["tcp://0.0.0.0:11010".to_string()],
        }
    }

    fn sample_minimal() -> ShareInfo {
        ShareInfo {
            network_name: "small".to_string(),
            network_secret: "sec-123".to_string(),
            host_hint: None,
            virtual_ip: None,
            dhcp: Some(true),
            name: None,
            peer_urls: vec![],
            listener_urls: vec![],
        }
    }

    #[test]
    fn binary_roundtrip_full() {
        let info = sample_full();
        let bin = encode_share_binary(&info).unwrap();
        let decoded = decode_share_binary(&bin).unwrap();
        assert_eq!(decoded.network_name, info.network_name);
        assert_eq!(decoded.network_secret, info.network_secret);
        assert_eq!(decoded.name, info.name);
        assert_eq!(decoded.host_hint, info.host_hint);
        assert_eq!(decoded.virtual_ip, info.virtual_ip);
        assert_eq!(decoded.dhcp, info.dhcp);
        assert_eq!(decoded.peer_urls, info.peer_urls);
        assert_eq!(decoded.listener_urls, info.listener_urls);
    }

    #[test]
    fn binary_roundtrip_minimal() {
        let info = sample_minimal();
        let bin = encode_share_binary(&info).unwrap();
        let decoded = decode_share_binary(&bin).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn link_roundtrip_full() {
        let info = sample_full();
        let link = encrypt_share_payload(&info).unwrap();
        assert!(link.starts_with("homeTier://join?v=1&d="));
        let decoded = decrypt_share_link(&link).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn link_roundtrip_minimal() {
        let info = sample_minimal();
        let link = encrypt_share_payload(&info).unwrap();
        let decoded = decrypt_share_link(&link).unwrap();
        assert_eq!(decoded, info);
    }

    /// 关键体积断言：v1（二进制）链接严格小于"JSON → zstd → AES-GCM → base64"等价流程。
    #[test]
    fn link_size_smaller_than_json_pipeline() {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

        let info = sample_full();

        // 基准：JSON 序列化 + zstd 压缩 + 同一密钥 AES-GCM 加密
        let json_bytes = serde_json::to_vec(&info).unwrap();
        let json_z = zstd::stream::encode_all(json_bytes.as_slice(), 3).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&[0u8; 32]).unwrap();
        let nonce = Nonce::from_slice(&[0u8; 12]);
        let ct = cipher.encrypt(nonce, json_z.as_slice()).unwrap();
        let mut json_blob = vec![0u8; 12];
        json_blob.extend(ct);
        let json_link = format!(
            "homeTier://join?v=1&d={}",
            URL_SAFE_NO_PAD.encode(&json_blob)
        );

        let v1_link = encrypt_share_payload(&info).unwrap();

        println!(
            "JSON pipeline link: {}B, v1 link: {}B (节省 {}B, {:.1}%)",
            json_link.len(),
            v1_link.len(),
            json_link.len() - v1_link.len(),
            100.0 * (json_link.len() - v1_link.len()) as f64 / json_link.len() as f64,
        );
        assert!(
            v1_link.len() < json_link.len(),
            "v1 链接应比 JSON 版本更短"
        );
    }

    #[test]
    fn rejects_v3_link() {
        // 旧 v3 链接应被拒绝（项目尚未上线，不做兼容）
        assert!(decrypt_share_link("homeTier://join?v=3&d=xxx").is_err());
    }

    #[test]
    fn rejects_bad_scheme() {
        assert!(decrypt_share_link("https://homeTier/join?v=1&d=xxx").is_err());
    }

    #[test]
    fn rejects_missing_data_param() {
        assert!(decrypt_share_link("homeTier://join?v=1").is_err());
    }

    #[test]
    fn rejects_corrupted_payload() {
        // 随机字节作为密文，解密应失败
        let link = "homeTier://join?v=1&d=AAAAAAAAAAAAAAAAAAAAAAAA";
        assert!(decrypt_share_link(link).is_err());
    }

    #[test]
    fn rejects_string_too_long() {
        let info = ShareInfo {
            network_name: "a".repeat(300),
            network_secret: "secret".to_string(),
            host_hint: None,
            virtual_ip: None,
            dhcp: None,
            name: None,
            peer_urls: vec![],
            listener_urls: vec![],
        };
        assert!(encode_share_binary(&info).is_err());
    }
}
