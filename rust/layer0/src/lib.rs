//! # Continuum Layer 0: Security Gateway
//!
//! 所有外部输入的安全网关层。
//!
//! ## 模块
//! - `input_validator`: 输入验证
//! - `pii_scrubber`: PII 数据清洗
//! - `access_controller`: 访问控制
//! - `rate_limiter`: 速率限制
//! - `secrets_manager`: 密钥管理
//! - `encryption_engine`: 加密引擎
//! - `threat_detector`: 威胁检测

pub mod access_controller;
pub mod encryption_engine;
pub mod input_validator;
pub mod pii_scrubber;
pub mod rate_limiter;
pub mod secrets_manager;
pub mod threat_detector;

pub use access_controller::{AccessController, Permission, Role};
pub use encryption_engine::{
    derive_key_from_password, generate_salt, EncryptedData, EncryptionAlgorithm, EncryptionConfig,
    EncryptionEngine, EncryptionError, EncryptionKey,
};
pub use input_validator::{InputValidator, ValidationResult};
pub use pii_scrubber::{PiiScrubber, ScrubResult};
pub use rate_limiter::{RateLimitConfig, RateLimiter};
pub use secrets_manager::{
    AuditAction, AuditLogEntry, SecretMetadataInfo, SecretsManager, SecretsManagerConfig,
};
pub use threat_detector::{
    DetectionRule, ResponseAction, ResponseRule, Threat, ThreatDetector, ThreatDetectorConfig,
    ThreatError, ThreatLevel, ThreatStats, ThreatType,
};

/// 安全网关 - 所有外部输入的入口
pub struct SecurityGateway {
    input_validator: InputValidator,
    pii_scrubber: PiiScrubber,
    access_controller: AccessController,
    rate_limiter: RateLimiter,
}

impl SecurityGateway {
    pub fn new() -> Self {
        Self {
            input_validator: InputValidator::new(),
            pii_scrubber: PiiScrubber::new(),
            access_controller: AccessController::new(),
            rate_limiter: RateLimiter::new(),
        }
    }

    /// 验证并清理输入
    pub async fn validate_input(&self, input: &str) -> anyhow::Result<String> {
        // 1. 验证输入格式
        self.input_validator.validate(input)?;

        // 2. 清理 PII 数据
        let result = self.pii_scrubber.scrub(input);

        Ok(result.scrubbed)
    }

    /// 检查访问权限
    pub fn check_access(&self, user_id: &str, resource: &str, action: &str) -> bool {
        self.access_controller.check(user_id, resource, action)
    }

    /// 检查速率限制
    pub async fn check_rate(&self, key: &str) -> anyhow::Result<bool> {
        self.rate_limiter.check(key).await
    }
}

impl Default for SecurityGateway {
    fn default() -> Self {
        Self::new()
    }
}
