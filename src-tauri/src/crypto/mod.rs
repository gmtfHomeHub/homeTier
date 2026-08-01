use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::Rng;

/// PBKDF2 迭代次数（OWASP 2023 建议值）
pub const PBKDF2_ITERATIONS: u32 = 210_000;
/// 盐长度（字节）
pub const SALT_LEN: usize = 16;
/// GCM nonce 长度（字节）
pub const NONCE_LEN: usize = 12;

/// 计算 SHA-256 哈希
pub fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).into()
}

/// 计算 SHA-256 哈希并以十六进制字符串返回
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha256(data))
}

/// 生成随机盐
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill(&mut salt);
    salt
}

/// 使用 PBKDF2-HMAC-SHA256 从密码派生密钥（加盐）
pub fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
        password.as_bytes(),
        salt,
        iterations,
        &mut key,
    );
    key
}

/// 使用 AES-256-GCM 加密数据（加盐派生密钥）
/// 输出格式: [salt 16B][nonce 12B][ciphertext]
pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let salt = generate_salt();
    let key = derive_key(password, &salt, PBKDF2_ITERATIONS);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {}", e))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("Encrypt error: {}", e))?;

    let mut result = salt.to_vec();
    result.extend_from_slice(&nonce_bytes);
    result.extend(ciphertext);
    Ok(result)
}

/// 使用 AES-256-GCM 解密数据（解析盐后派生密钥）
/// 输入格式: [salt 16B][nonce 12B][ciphertext]
pub fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if data.len() < SALT_LEN + NONCE_LEN {
        return Err("Invalid encrypted data".to_string());
    }

    let salt = &data[..SALT_LEN];
    let key = derive_key(password, salt, PBKDF2_ITERATIONS);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {}", e))?;

    let nonce = Nonce::from_slice(&data[SALT_LEN..SALT_LEN + NONCE_LEN]);
    let ciphertext = &data[SALT_LEN + NONCE_LEN..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decrypt error: {}", e))?;

    Ok(plaintext)
}

/// 使用 HMAC-SHA256 计算签名（十六进制字符串）
pub fn hmac_sha256(secret: &str, data: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any size");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

/// 校验 HMAC-SHA256 签名
pub fn verify_hmac(secret: &str, data: &[u8], signature: &str) -> bool {
    let expected = hmac_sha256(secret, data);
    expected == signature
}
