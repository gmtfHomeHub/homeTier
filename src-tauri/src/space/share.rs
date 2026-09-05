//! `ShareInfo` 二进制编解码（join_space 业务载荷）。
//!
//! 本模块只负责 `ShareInfo ↔ 二进制` 的紧凑序列化；加密 / URL 封装 /
//! 事件路由（`e` 字段）由通用传输层 [`crate::qr`] 负责，业务方组合调用：
//! - 生成：`encode_share_binary(&info)` → `qr::encrypt_qr(EVENT_JOIN_SPACE, &bytes)`
//! - 解析：`qr::decrypt_qr(link)` → 校验 event == `j_s` → `decode_share_binary(&bytes)`
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

use crate::types::ShareInfo;

/// 二进制 payload 版本标记（首字节）。
const PAYLOAD_VERSION: u8 = 1;

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

    /// join_space 端到端：ShareInfo → 二进制 → qr 传输层 → 反向还原。
    #[test]
    fn join_space_e2e_roundtrip() {
        let info = sample_full();
        let data = encode_share_binary(&info).unwrap();
        let link = crate::qr::encrypt_qr(crate::qr::EVENT_JOIN_SPACE, &data).unwrap();
        assert!(link.starts_with("homeTier://qr?v=1&d="));
        let (event, decoded_data) = crate::qr::decrypt_qr(&link).unwrap();
        assert_eq!(event, crate::qr::EVENT_JOIN_SPACE);
        let decoded = decode_share_binary(&decoded_data).unwrap();
        assert_eq!(decoded, info);
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
