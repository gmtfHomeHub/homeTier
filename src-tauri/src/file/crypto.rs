use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::Rng;

/// 使用 AES-256-GCM 加密文件数据
pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    // 从密码派生密钥（简化版，生产环境应使用 PBKDF2/Argon2）
    use sha2::{Digest, Sha256};
    let key_hash = Sha256::digest(password.as_bytes());
    let key = aes_gcm::Aes256Gcm::new_from_slice(&key_hash)
        .map_err(|e| format!("Key init error: {}", e))?;

    // 生成随机 nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 加密
    let ciphertext = key
        .encrypt(nonce, data)
        .map_err(|e| format!("Encrypt error: {}", e))?;

    // 返回 nonce + ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// 使用 AES-256-GCM 解密文件数据
pub fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if data.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }

    use sha2::{Digest, Sha256};
    let key_hash = Sha256::digest(password.as_bytes());
    let key = aes_gcm::Aes256Gcm::new_from_slice(&key_hash)
        .map_err(|e| format!("Key init error: {}", e))?;

    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    let plaintext = key
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decrypt error: {}", e))?;

    Ok(plaintext)
}
