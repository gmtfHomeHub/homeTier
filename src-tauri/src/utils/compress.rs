//! 压缩编解码器工具库。
//!
//! 提供统一的压缩/解压抽象，支持不同业务场景（分享链接、文件归档等）
//! 选择不同压缩级别或算法。当前仅实现 zstd，未来可加 gzip/brotli 等。

use std::io::{Read, Write};
use zstd::{Decoder, Encoder};

/// 压缩编解码器 trait：不同业务场景可实现不同算法/级别。
pub trait CompressionCodec: Send + Sync {
    /// 编解码器标识，用于线上格式区分（不同 codec 之间可混用）。
    fn id(&self) -> u8;
    /// 压缩数据。
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, String>;
    /// 解压数据。
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, String>;
}

/// zstd 压缩编解码器。`level` 建议 1..=22，越高越紧凑越慢。
pub struct ZstdCodec {
    pub level: i32,
}

impl ZstdCodec {
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl CompressionCodec for ZstdCodec {
    fn id(&self) -> u8 {
        1
    }
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut encoder = Encoder::new(Vec::new(), self.level)
            .map_err(|e| format!("压缩初始化失败: {}", e))?;
        encoder
            .write_all(data)
            .map_err(|e| format!("压缩写入失败: {}", e))?;
        let out = encoder
            .finish()
            .map_err(|e| format!("压缩完成失败: {}", e))?;
        Ok(out)
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut decoder = Decoder::new(data)
            .map_err(|e| format!("解压初始化失败: {}", e))?;
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|e| format!("解压读取失败: {}", e))?;
        Ok(out)
    }
}

/// 业务场景对应的压缩配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionPurpose {
    /// 分享链接：level 3，速度优先，载荷小。
    ShareLink,
    /// 文件归档：level 15，压缩比优先。
    FileArchive,
}

impl CompressionPurpose {
    pub fn level(&self) -> i32 {
        match self {
            Self::ShareLink => 3,
            Self::FileArchive => 15,
        }
    }
    /// 获取该场景对应的 zstd codec 实例。
    pub fn codec(&self) -> ZstdCodec {
        ZstdCodec::new(self.level())
    }
}

// ---- 自适应压缩（用于分享链接等极小载荷）----

/// 自适应格式标记：`0` = zstd 压缩，`1` = 原文（跳过压缩）。
const FORMAT_ZSTD: u8 = 0;
const FORMAT_RAW: u8 = 1;

/// 自适应压缩：若 zstd 压缩后严格小于原文，走 `[0][zstd 数据]`；
/// 否则（含压缩失败）走 `[1][原文]`。适用于极小载荷——避免 zstd 帧头开销反噬。
///
/// 使用 `adaptive_decompress` 解码。
pub fn adaptive_compress(data: &[u8]) -> Vec<u8> {
    let z = ZstdCodec::new(CompressionPurpose::ShareLink.level());
    match z.compress(data) {
        Ok(compressed) if compressed.len() < data.len() => {
            let mut out = Vec::with_capacity(compressed.len() + 1);
            out.push(FORMAT_ZSTD);
            out.extend_from_slice(&compressed);
            out
        }
        _ => {
            let mut out = Vec::with_capacity(data.len() + 1);
            out.push(FORMAT_RAW);
            out.extend_from_slice(data);
            out
        }
    }
}

/// 解码 `adaptive_compress` 的产物。
pub fn adaptive_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("压缩数据为空".to_string());
    }
    match data[0] {
        FORMAT_ZSTD => {
            let z = ZstdCodec::new(CompressionPurpose::ShareLink.level());
            z.decompress(&data[1..])
        }
        FORMAT_RAW => Ok(data[1..].to_vec()),
        other => Err(format!("未知压缩格式标记: 0x{:02x}", other)),
    }
}

// ---- 兼容旧调用点（file/transfer.rs 等）----

/// zstd 压缩（保留原 `file::compress::compress` 签名，level 由调用方指定）。
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>, String> {
    ZstdCodec::new(level).compress(data)
}

/// zstd 解压（保留原 `file::compress::decompress` 签名）。
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    ZstdCodec::new(CompressionPurpose::ShareLink.level()).decompress(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zstd_roundtrip() {
        let data = b"hello world, hello world, hello world";
        let codec = ZstdCodec::new(3);
        let packed = codec.compress(data).unwrap();
        let unpacked = codec.decompress(&packed).unwrap();
        assert_eq!(unpacked, data.to_vec());
    }

    #[test]
    fn adaptive_switches_on_small_data() {
        // 极小数据：zstd 帧头开销 > 数据，应走 raw
        let small = b"abc";
        let packed = adaptive_compress(small);
        assert_eq!(packed[0], FORMAT_RAW);
        assert_eq!(adaptive_decompress(&packed).unwrap(), small.to_vec());
    }

    #[test]
    fn adaptive_switches_on_repetitive_data() {
        // 大段重复数据：zstd 能显著压缩
        let big = vec![b'a'; 500];
        let packed = adaptive_compress(&big);
        assert_eq!(packed[0], FORMAT_ZSTD);
        assert_eq!(adaptive_decompress(&packed).unwrap(), big);
    }

    #[test]
    fn adaptive_rejects_unknown_flag() {
        assert!(adaptive_decompress(&[0xAA, 0x01]).is_err());
    }

    #[test]
    fn purpose_levels_are_sane() {
        assert!(CompressionPurpose::ShareLink.level() < CompressionPurpose::FileArchive.level());
    }
}
