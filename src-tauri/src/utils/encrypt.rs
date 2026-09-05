//! 固定密钥 AES-256-GCM 加密工具库。
//!
//! 面向无交互分享场景：密钥由常量种子 SHA-256 派生，两端一致，
//! 无需密码交换。与 `crate::crypto` 中的密码派生（PBKDF2）方案互补。

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::Rng;

use crate::crypto::{sha256, NONCE_LEN};

/// AES-256-GCM 认证 tag 长度（附加在密文尾部）。
pub const TAG_LEN: usize = 16;

/// 固定密钥 AES-256-GCM 加解密器。
pub struct FixedKeyGcm {
    key: [u8; 32],
}

impl FixedKeyGcm {
    /// 从种子字节派生 AES-256 密钥（SHA-256）。
    ///
    /// 通常传入版本化常量字符串，便于未来轮换密钥。
    pub fn new_from_seed(seed: &[u8]) -> Self {
        Self {
            key: sha256(seed),
        }
    }

    /// 加密：输出 `[nonce 12B][ciphertext][tag 16B]`。
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| format!("密钥初始化失败: {}", e))?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("加密失败: {}", e))?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend(ciphertext);
        Ok(out)
    }

    /// 解密：输入 `[nonce 12B][ciphertext][tag 16B]`。
    pub fn decrypt(&self, blob: &[u8]) -> Result<Vec<u8>, String> {
        if blob.len() < NONCE_LEN + TAG_LEN {
            return Err("加密数据长度不足".to_string());
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| format!("密钥初始化失败: {}", e))?;
        let nonce = Nonce::from_slice(&blob[..NONCE_LEN]);
        cipher
            .decrypt(nonce, &blob[NONCE_LEN..])
            .map_err(|_| "解密失败，数据无效或已损坏".to_string())
    }
}

/// 通用二维码专用密钥（版本化种子，便于未来轮换）。
pub fn qr_key() -> FixedKeyGcm {
    FixedKeyGcm::new_from_seed(b"homeTier-qr-v1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_key_roundtrip() {
        let k = FixedKeyGcm::new_from_seed(b"seed-abc");
        let pt = b"hello, world";
        let ct = k.encrypt(pt).unwrap();
        assert!(ct.len() == pt.len() + NONCE_LEN + TAG_LEN);
        let rt = k.decrypt(&ct).unwrap();
        assert_eq!(rt, pt.to_vec());
    }

    #[test]
    fn fixed_key_rejects_short_input() {
        let k = FixedKeyGcm::new_from_seed(b"seed");
        assert!(k.decrypt(&[]).is_err());
        assert!(k.decrypt(&[0u8; 12]).is_err());
    }

    #[test]
    fn fixed_key_rejects_wrong_key() {
        let k1 = FixedKeyGcm::new_from_seed(b"seed-1");
        let k2 = FixedKeyGcm::new_from_seed(b"seed-2");
        let ct = k1.encrypt(b"secret").unwrap();
        assert!(k2.decrypt(&ct).is_err());
    }

    #[test]
    fn qr_key_is_stable() {
        // 相同种子的密钥应一致，加密结果应可解密
        let k = qr_key();
        let ct = k.encrypt(b"payload").unwrap();
        assert_eq!(qr_key().decrypt(&ct).unwrap(), b"payload".to_vec());
    }
}
