use std::io::{Read, Write};
use zstd::{Decoder, Encoder};

/// 使用 Zstd 压缩数据
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>, String> {
    let mut encoder =
        Encoder::new(Vec::new(), level).map_err(|e| format!("Compress init error: {}", e))?;
    encoder
        .write_all(data)
        .map_err(|e| format!("Compress error: {}", e))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("Compress finish error: {}", e))?;
    Ok(compressed)
}

/// 使用 Zstd 解压数据
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = Decoder::new(data).map_err(|e| format!("Decompress init error: {}", e))?;
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Decompress error: {}", e))?;
    Ok(decompressed)
}
