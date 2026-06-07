//! # Encryption Engine
//!
//! 安全加密引擎，提供生产级加密支持。
//!
//! ## 支持的算法
//! - **AES-256-GCM**: 推荐，硬件加速
//! - **ChaCha20-Poly1305**: 软件实现高效
//!
//! ## 密钥派生
//! - Argon2id 密钥派生函数
//!
//! ## 功能
//! - 对称加密/解密
//! - 密钥派生 (KDF)
//! - 密钥轮换
//! - 安全密钥存储

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{password_hash::rand_core::RngCore, Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use zeroize::Zeroizing;

/// 加密错误类型
#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid ciphertext format")]
    InvalidCiphertextFormat,

    #[error("Key rotation failed: {0}")]
    KeyRotationFailed(String),
}

/// 加密算法类型
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM (推荐)
    #[default]
    Aes256Gcm,
    /// ChaCha20-Poly1305
    ChaCha20Poly1305,
}

impl std::fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aes256Gcm => write!(f, "AES-256-GCM"),
            Self::ChaCha20Poly1305 => write!(f, "ChaCha20-Poly1305"),
        }
    }
}

/// 加密密钥（安全存储）
#[derive(Clone)]
pub struct EncryptionKey {
    /// 密钥数据（零化）
    key_data: Arc<Zeroizing<Vec<u8>>>,
    /// 算法类型
    algorithm: EncryptionAlgorithm,
    /// 密钥 ID
    key_id: String,
    /// 创建时间
    created_at: chrono::DateTime<chrono::Utc>,
}

impl EncryptionKey {
    /// 从原始字节创建密钥
    pub fn from_bytes(
        key_bytes: &[u8],
        algorithm: EncryptionAlgorithm,
    ) -> Result<Self, EncryptionError> {
        let expected_len = Self::key_length(algorithm);
        if key_bytes.len() != expected_len {
            return Err(EncryptionError::InvalidKeyLength {
                expected: expected_len,
                actual: key_bytes.len(),
            });
        }

        Ok(Self {
            key_data: Arc::new(Zeroizing::new(key_bytes.to_vec())),
            algorithm,
            key_id: generate_key_id(),
            created_at: chrono::Utc::now(),
        })
    }

    /// 生成随机密钥
    pub fn generate(algorithm: EncryptionAlgorithm) -> Result<Self, EncryptionError> {
        let key_len = Self::key_length(algorithm);
        let mut key_bytes = vec![0u8; key_len];
        OsRng.fill_bytes(&mut key_bytes);

        Self::from_bytes(&key_bytes, algorithm)
    }

    /// 从密码派生密钥
    pub fn derive_from_password(
        password: &str,
        salt: &[u8],
        algorithm: EncryptionAlgorithm,
    ) -> Result<Self, EncryptionError> {
        let key_len = Self::key_length(algorithm);
        let params = Params::new(
            Params::DEFAULT_M_COST,
            Params::DEFAULT_T_COST,
            Params::DEFAULT_P_COST,
            Some(key_len),
        )
        .map_err(|e| EncryptionError::KeyDerivationFailed(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key_bytes = vec![0u8; key_len];

        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| EncryptionError::KeyDerivationFailed(e.to_string()))?;

        Self::from_bytes(&key_bytes, algorithm)
    }

    /// 获取密钥长度
    pub fn key_length(algorithm: EncryptionAlgorithm) -> usize {
        match algorithm {
            EncryptionAlgorithm::Aes256Gcm => 32,
            EncryptionAlgorithm::ChaCha20Poly1305 => 32,
        }
    }

    /// 获取 Nonce 长度
    pub fn nonce_length(algorithm: EncryptionAlgorithm) -> usize {
        match algorithm {
            EncryptionAlgorithm::Aes256Gcm => 12,
            EncryptionAlgorithm::ChaCha20Poly1305 => 12,
        }
    }

    /// 获取密钥数据
    pub fn as_bytes(&self) -> &[u8] {
        &self.key_data
    }

    /// 获取密钥 ID
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// 获取算法
    pub fn algorithm(&self) -> EncryptionAlgorithm {
        self.algorithm
    }

    /// 获取创建时间
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }
}

impl std::fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionKey")
            .field("key_id", &self.key_id)
            .field("algorithm", &self.algorithm)
            .field("created_at", &self.created_at)
            .field("key_data", &"<redacted>")
            .finish()
    }
}

/// 加密结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// 加密算法
    pub algorithm: EncryptionAlgorithm,
    /// Nonce（Base64）
    pub nonce: String,
    /// 密文（Base64）
    pub ciphertext: String,
    /// 密钥 ID
    pub key_id: String,
    /// 关联数据（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_data: Option<String>,
}

impl EncryptedData {
    /// 序列化为字符串
    pub fn to_string(&self) -> Result<String, EncryptionError> {
        serde_json::to_string(self).map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))
    }

    /// 从字符串解析
    pub fn from_string(s: &str) -> Result<Self, EncryptionError> {
        serde_json::from_str(s).map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }
}

/// 加密引擎配置
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// 默认算法
    pub default_algorithm: EncryptionAlgorithm,
    /// 是否自动轮换密钥
    pub auto_key_rotation: bool,
    /// 密钥有效期（秒）
    pub key_validity_secs: u64,
    /// 最大密钥数量
    pub max_keys: usize,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            default_algorithm: EncryptionAlgorithm::Aes256Gcm,
            auto_key_rotation: false,
            key_validity_secs: 86400 * 90, // 90 天
            max_keys: 100,
        }
    }
}

/// 加密引擎
pub struct EncryptionEngine {
    /// 密钥存储
    keys: RwLock<HashMap<String, EncryptionKey>>,
    /// 默认密钥 ID
    default_key_id: RwLock<Option<String>>,
    /// 配置
    config: EncryptionConfig,
}

impl EncryptionEngine {
    /// 创建新的加密引擎
    pub fn new() -> Self {
        Self::with_config(EncryptionConfig::default())
    }

    /// 使用配置创建
    pub fn with_config(config: EncryptionConfig) -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
            default_key_id: RwLock::new(None),
            config,
        }
    }

    /// 生成并添加新密钥
    pub fn generate_key(&self) -> Result<String, EncryptionError> {
        self.generate_key_with_algorithm(self.config.default_algorithm)
    }

    /// 使用指定算法生成密钥
    pub fn generate_key_with_algorithm(
        &self,
        algorithm: EncryptionAlgorithm,
    ) -> Result<String, EncryptionError> {
        let key = EncryptionKey::generate(algorithm)?;
        let key_id = key.key_id().to_string();

        // 检查密钥数量限制
        let mut keys = self.keys.write();
        if keys.len() >= self.config.max_keys {
            // 移除最旧的密钥
            if let Some((oldest_id, _)) = keys
                .iter()
                .min_by_key(|(_, k)| k.created_at())
                .map(|(id, k)| (id.clone(), k.created_at()))
            {
                keys.remove(&oldest_id);
            }
        }

        keys.insert(key_id.clone(), key);

        // 设置为默认密钥
        *self.default_key_id.write() = Some(key_id.clone());

        tracing::info!("Generated new encryption key: {}", key_id);
        Ok(key_id)
    }

    /// 从密码派生并添加密钥
    pub fn derive_key_from_password(
        &self,
        password: &str,
        salt: &[u8],
    ) -> Result<String, EncryptionError> {
        let key =
            EncryptionKey::derive_from_password(password, salt, self.config.default_algorithm)?;
        let key_id = key.key_id().to_string();

        self.keys.write().insert(key_id.clone(), key);
        *self.default_key_id.write() = Some(key_id.clone());

        tracing::info!("Derived encryption key from password: {}", key_id);
        Ok(key_id)
    }

    /// 添加已有密钥
    pub fn add_key(&self, key: EncryptionKey) -> Result<String, EncryptionError> {
        let key_id = key.key_id().to_string();
        self.keys.write().insert(key_id.clone(), key);
        *self.default_key_id.write() = Some(key_id.clone());
        Ok(key_id)
    }

    /// 获取密钥
    pub fn get_key(&self, key_id: &str) -> Result<EncryptionKey, EncryptionError> {
        self.keys
            .read()
            .get(key_id)
            .cloned()
            .ok_or_else(|| EncryptionError::KeyNotFound(key_id.to_string()))
    }

    /// 删除密钥
    pub fn remove_key(&self, key_id: &str) -> Result<bool, EncryptionError> {
        let removed = self.keys.write().remove(key_id).is_some();

        // 如果删除的是默认密钥，清除默认密钥设置
        if removed {
            let mut default_id = self.default_key_id.write();
            if default_id.as_deref() == Some(key_id) {
                *default_id = None;
            }
        }

        Ok(removed)
    }

    /// 列出所有密钥 ID
    pub fn list_keys(&self) -> Vec<String> {
        self.keys.read().keys().cloned().collect()
    }

    /// 获取默认密钥
    pub fn get_default_key(&self) -> Result<EncryptionKey, EncryptionError> {
        let default_id = self
            .default_key_id
            .read()
            .clone()
            .ok_or_else(|| EncryptionError::KeyNotFound("default".to_string()))?;

        self.get_key(&default_id)
    }

    /// 设置默认密钥
    pub fn set_default_key(&self, key_id: &str) -> Result<(), EncryptionError> {
        if !self.keys.read().contains_key(key_id) {
            return Err(EncryptionError::KeyNotFound(key_id.to_string()));
        }

        *self.default_key_id.write() = Some(key_id.to_string());
        Ok(())
    }

    /// 加密数据
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, EncryptionError> {
        let key = self.get_default_key()?;
        self.encrypt_with_key(&key, plaintext)
    }

    /// 使用指定密钥加密
    pub fn encrypt_with_key(
        &self,
        key: &EncryptionKey,
        plaintext: &[u8],
    ) -> Result<EncryptedData, EncryptionError> {
        // 生成随机 Nonce
        let nonce_len = EncryptionKey::nonce_length(key.algorithm);
        let mut nonce_bytes = vec![0u8; nonce_len];
        OsRng.fill_bytes(&mut nonce_bytes);

        let ciphertext = match key.algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                let cipher_key = Key::<Aes256Gcm>::from_slice(key.as_bytes());
                let cipher = Aes256Gcm::new(cipher_key);
                let nonce = Nonce::from_slice(&nonce_bytes);

                cipher
                    .encrypt(nonce, plaintext)
                    .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                let cipher_key = ChaChaKey::from_slice(key.as_bytes());
                let cipher = ChaCha20Poly1305::new(cipher_key);
                let nonce = ChaChaNonce::from_slice(&nonce_bytes);

                cipher
                    .encrypt(nonce, plaintext)
                    .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?
            }
        };

        Ok(EncryptedData {
            algorithm: key.algorithm,
            nonce: BASE64.encode(&nonce_bytes),
            ciphertext: BASE64.encode(&ciphertext),
            key_id: key.key_id().to_string(),
            associated_data: None,
        })
    }

    /// 加密字符串
    pub fn encrypt_string(&self, plaintext: &str) -> Result<EncryptedData, EncryptionError> {
        self.encrypt(plaintext.as_bytes())
    }

    /// 解密数据
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, EncryptionError> {
        let key = self.get_key(&encrypted.key_id)?;
        self.decrypt_with_key(&key, encrypted)
    }

    /// 使用指定密钥解密
    pub fn decrypt_with_key(
        &self,
        key: &EncryptionKey,
        encrypted: &EncryptedData,
    ) -> Result<Vec<u8>, EncryptionError> {
        // 解码 Base64
        let nonce_bytes = BASE64
            .decode(&encrypted.nonce)
            .map_err(|_| EncryptionError::InvalidCiphertextFormat)?;

        let ciphertext = BASE64
            .decode(&encrypted.ciphertext)
            .map_err(|_| EncryptionError::InvalidCiphertextFormat)?;

        // 验证 Nonce 长度
        let expected_nonce_len = EncryptionKey::nonce_length(key.algorithm);
        if nonce_bytes.len() != expected_nonce_len {
            return Err(EncryptionError::InvalidNonceLength {
                expected: expected_nonce_len,
                actual: nonce_bytes.len(),
            });
        }

        let plaintext = match key.algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                let cipher_key = Key::<Aes256Gcm>::from_slice(key.as_bytes());
                let cipher = Aes256Gcm::new(cipher_key);
                let nonce = Nonce::from_slice(&nonce_bytes);

                cipher
                    .decrypt(nonce, ciphertext.as_slice())
                    .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                let cipher_key = ChaChaKey::from_slice(key.as_bytes());
                let cipher = ChaCha20Poly1305::new(cipher_key);
                let nonce = ChaChaNonce::from_slice(&nonce_bytes);

                cipher
                    .decrypt(nonce, ciphertext.as_slice())
                    .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?
            }
        };

        Ok(plaintext)
    }

    /// 解密为字符串
    pub fn decrypt_to_string(&self, encrypted: &EncryptedData) -> Result<String, EncryptionError> {
        let plaintext = self.decrypt(encrypted)?;
        String::from_utf8(plaintext).map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }

    /// 轮换密钥
    pub fn rotate_key(&self, old_key_id: &str) -> Result<String, EncryptionError> {
        // 检查旧密钥是否存在
        if !self.keys.read().contains_key(old_key_id) {
            return Err(EncryptionError::KeyNotFound(old_key_id.to_string()));
        }

        // 生成新密钥
        let new_key_id = self.generate_key()?;

        tracing::info!("Key rotated: {} -> {}", old_key_id, new_key_id);
        Ok(new_key_id)
    }

    /// 获取需要轮换的密钥
    pub fn get_keys_requiring_rotation(&self) -> Vec<String> {
        let now = chrono::Utc::now();
        let validity = chrono::Duration::seconds(self.config.key_validity_secs as i64);

        self.keys
            .read()
            .iter()
            .filter(|(_, key)| {
                let age = now.signed_duration_since(key.created_at());
                age > validity
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 导出密钥（Base64）
    pub fn export_key(&self, key_id: &str) -> Result<String, EncryptionError> {
        let key = self.get_key(key_id)?;
        Ok(BASE64.encode(key.as_bytes()))
    }

    /// 导入密钥（Base64）
    pub fn import_key(
        &self,
        key_b64: &str,
        algorithm: EncryptionAlgorithm,
    ) -> Result<String, EncryptionError> {
        let key_bytes = BASE64
            .decode(key_b64)
            .map_err(|_| EncryptionError::InvalidCiphertextFormat)?;

        let key = EncryptionKey::from_bytes(&key_bytes, algorithm)?;
        self.add_key(key)
    }

    /// 密钥数量
    pub fn key_count(&self) -> usize {
        self.keys.read().len()
    }

    /// 是否有默认密钥
    pub fn has_default_key(&self) -> bool {
        self.default_key_id.read().is_some()
    }
}

impl Default for EncryptionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 生成密钥 ID
fn generate_key_id() -> String {
    use uuid::Uuid;
    format!("key_{}", Uuid::new_v4())
}

/// 生成盐值
pub fn generate_salt() -> Vec<u8> {
    let mut salt = vec![0u8; 16];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// 从密码派生密钥（独立函数）
pub fn derive_key_from_password(
    password: &str,
    salt: &[u8],
    algorithm: EncryptionAlgorithm,
) -> Result<EncryptionKey, EncryptionError> {
    EncryptionKey::derive_from_password(password, salt, algorithm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_key_generation() {
        let key = EncryptionKey::generate(EncryptionAlgorithm::Aes256Gcm).unwrap();
        assert_eq!(key.as_bytes().len(), 32);
        assert!(!key.key_id().is_empty());
    }

    #[test]
    fn test_encryption_key_from_bytes() {
        let key_bytes = vec![0u8; 32];
        let key = EncryptionKey::from_bytes(&key_bytes, EncryptionAlgorithm::Aes256Gcm).unwrap();
        assert_eq!(key.as_bytes().len(), 32);
    }

    #[test]
    fn test_encryption_key_invalid_length() {
        let key_bytes = vec![0u8; 16];
        let result = EncryptionKey::from_bytes(&key_bytes, EncryptionAlgorithm::Aes256Gcm);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_derivation() {
        let salt = generate_salt();
        let key =
            derive_key_from_password("password123", &salt, EncryptionAlgorithm::Aes256Gcm).unwrap();
        assert_eq!(key.as_bytes().len(), 32);
    }

    #[test]
    fn test_aes_gcm_encryption_decryption() {
        let engine = EncryptionEngine::new();
        engine.generate_key().unwrap();

        let plaintext = b"Hello, World!";
        let encrypted = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_chacha_encryption_decryption() {
        let config = EncryptionConfig {
            default_algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            ..Default::default()
        };
        let engine = EncryptionEngine::with_config(config);
        engine.generate_key().unwrap();

        let plaintext = b"Hello, ChaCha20!";
        let encrypted = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_string_encryption() {
        let engine = EncryptionEngine::new();
        engine.generate_key().unwrap();

        let plaintext = "Secret message";
        let encrypted = engine.encrypt_string(plaintext).unwrap();
        let decrypted = engine.decrypt_to_string(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_multiple_keys() {
        let engine = EncryptionEngine::new();
        let key_id1 = engine.generate_key().unwrap();
        let key_id2 = engine
            .generate_key_with_algorithm(EncryptionAlgorithm::ChaCha20Poly1305)
            .unwrap();

        assert_eq!(engine.key_count(), 2);
        assert!(engine.list_keys().contains(&key_id1));
        assert!(engine.list_keys().contains(&key_id2));
    }

    #[test]
    fn test_key_removal() {
        let engine = EncryptionEngine::new();
        let key_id = engine.generate_key().unwrap();

        assert!(engine.remove_key(&key_id).unwrap());
        assert!(!engine.list_keys().contains(&key_id));
    }

    #[test]
    fn test_key_rotation() {
        let engine = EncryptionEngine::new();
        let old_key_id = engine.generate_key().unwrap();

        let new_key_id = engine.rotate_key(&old_key_id).unwrap();

        assert_ne!(old_key_id, new_key_id);
        assert!(engine.list_keys().contains(&new_key_id));
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let engine = EncryptionEngine::new();
        engine.generate_key().unwrap();

        let encrypted = engine.encrypt_string("test").unwrap();
        let serialized = encrypted.to_string().unwrap();
        let deserialized = EncryptedData::from_string(&serialized).unwrap();

        assert_eq!(encrypted.key_id, deserialized.key_id);
        assert_eq!(encrypted.nonce, deserialized.nonce);
        assert_eq!(encrypted.ciphertext, deserialized.ciphertext);
    }

    #[test]
    fn test_key_export_import() {
        let engine = EncryptionEngine::new();
        let key_id = engine.generate_key().unwrap();

        let exported = engine.export_key(&key_id).unwrap();
        engine.remove_key(&key_id).unwrap();

        let imported_id = engine
            .import_key(&exported, EncryptionAlgorithm::Aes256Gcm)
            .unwrap();
        assert!(engine.list_keys().contains(&imported_id));
    }

    #[test]
    fn test_different_plaintexts_different_ciphertexts() {
        let engine = EncryptionEngine::new();
        engine.generate_key().unwrap();

        let encrypted1 = engine.encrypt(b"test").unwrap();
        let encrypted2 = engine.encrypt(b"test").unwrap();

        // 即使明文相同，密文也应该不同（因为 Nonce 不同）
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
        assert_ne!(encrypted1.nonce, encrypted2.nonce);
    }

    #[test]
    fn test_large_data_encryption() {
        let engine = EncryptionEngine::new();
        engine.generate_key().unwrap();

        let large_data = vec![0u8; 1_000_000]; // 1MB
        let encrypted = engine.encrypt(&large_data).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();

        assert_eq!(large_data, decrypted);
    }

    #[test]
    fn test_empty_data_encryption() {
        let engine = EncryptionEngine::new();
        engine.generate_key().unwrap();

        let encrypted = engine.encrypt(b"").unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();

        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_algorithm_display() {
        assert_eq!(format!("{}", EncryptionAlgorithm::Aes256Gcm), "AES-256-GCM");
        assert_eq!(
            format!("{}", EncryptionAlgorithm::ChaCha20Poly1305),
            "ChaCha20-Poly1305"
        );
    }

    #[test]
    fn test_encryption_key_debug_redaction() {
        let key = EncryptionKey::generate(EncryptionAlgorithm::Aes256Gcm).unwrap();
        let debug_output = format!("{:?}", key);

        // 确保调试输出不包含密钥数据
        assert!(debug_output.contains("<redacted>"));
        assert!(!debug_output.contains(&BASE64.encode(key.as_bytes())));
    }

    #[test]
    fn test_default_key() {
        let engine = EncryptionEngine::new();
        assert!(!engine.has_default_key());

        engine.generate_key().unwrap();
        assert!(engine.has_default_key());
    }

    #[test]
    fn test_set_default_key() {
        let engine = EncryptionEngine::new();
        let key_id1 = engine.generate_key().unwrap();
        let key_id2 = engine.generate_key().unwrap();

        // 第二个生成的密钥应该是默认的
        let default_key = engine.get_default_key().unwrap();
        assert_eq!(default_key.key_id(), key_id2);

        // 设置第一个密钥为默认
        engine.set_default_key(&key_id1).unwrap();
        let default_key = engine.get_default_key().unwrap();
        assert_eq!(default_key.key_id(), key_id1);
    }

    #[test]
    fn test_wrong_algorithm_decryption() {
        let engine = EncryptionEngine::new();
        let aes_key = engine
            .generate_key_with_algorithm(EncryptionAlgorithm::Aes256Gcm)
            .unwrap();
        let chacha_key = engine
            .generate_key_with_algorithm(EncryptionAlgorithm::ChaCha20Poly1305)
            .unwrap();

        let aes_encrypted = engine
            .encrypt_with_key(&engine.get_key(&aes_key).unwrap(), b"test")
            .unwrap();

        // 尝试用错误的密钥解密应该失败
        let result = engine.decrypt_with_key(&engine.get_key(&chacha_key).unwrap(), &aes_encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_concurrent_encryption() {
        use std::sync::Arc;
        use std::thread;

        let engine = Arc::new(EncryptionEngine::new());
        engine.generate_key().unwrap();

        let mut handles = vec![];

        for i in 0..10 {
            let e = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                let data = format!("message_{}", i);
                let encrypted = e.encrypt_string(&data).unwrap();
                let decrypted = e.decrypt_to_string(&encrypted).unwrap();
                assert_eq!(data, decrypted);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
