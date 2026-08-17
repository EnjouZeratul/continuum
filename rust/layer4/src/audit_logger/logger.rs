//! 审计日志记录器
//!
//! 主要的审计日志接口。

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;

use super::entry::{AuditAction, AuditEntry, AuditFilter, AuditResult, ExportFormat};
use super::storage::{AuditStorage, MemoryStorage};
use anyhow::Result;

/// 审计日志配置
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// 是否启用
    pub enabled: bool,
    /// 是否脱敏敏感数据
    pub sanitize_sensitive: bool,
    /// 敏感字段列表
    pub sensitive_fields: Vec<String>,
    /// 默认用户 ID (未指定时使用)
    pub default_user_id: String,
    /// 异步写入
    pub async_write: bool,
    /// 保留天数
    pub retention_days: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sanitize_sensitive: true,
            sensitive_fields: vec![
                "password".to_string(),
                "token".to_string(),
                "api_key".to_string(),
                "secret".to_string(),
                "credential".to_string(),
            ],
            default_user_id: "system".to_string(),
            async_write: true,
            retention_days: 90,
        }
    }
}

/// 审计日志记录器
pub struct AuditLogger {
    /// 存储后端
    storage: Arc<dyn AuditStorage>,
    /// 配置
    config: AuditConfig,
}

impl AuditLogger {
    /// 创建新的审计日志记录器
    pub fn new(config: AuditConfig) -> Self {
        Self {
            storage: Arc::new(MemoryStorage::new(10000)),
            config,
        }
    }

    /// 使用自定义存储后端
    pub fn with_storage(mut self, storage: Arc<dyn AuditStorage>) -> Self {
        self.storage = storage;
        self
    }

    /// 获取配置
    pub fn config(&self) -> &AuditConfig {
        &self.config
    }

    /// 记录审计日志
    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // 脱敏处理
        let entry = if self.config.sanitize_sensitive {
            self.sanitize_entry(entry)
        } else {
            entry
        };

        self.storage.save(&entry).await
    }

    /// 快速记录操作
    pub async fn log_action(
        &self,
        user_id: &str,
        action: AuditAction,
        resource_type: &str,
        resource_id: Option<&str>,
        result: AuditResult,
    ) -> Result<()> {
        let mut entry = AuditEntry::new(user_id, action, resource_type);

        if let Some(id) = resource_id {
            entry = entry.with_resource_id(id);
        }

        entry = entry.with_result(result);

        self.log(entry).await
    }

    /// 记录成功操作
    pub async fn log_success(
        &self,
        user_id: &str,
        action: AuditAction,
        resource_type: &str,
        resource_id: Option<&str>,
    ) -> Result<()> {
        self.log_action(
            user_id,
            action,
            resource_type,
            resource_id,
            AuditResult::Success,
        )
        .await
    }

    /// 记录失败操作
    pub async fn log_failure(
        &self,
        user_id: &str,
        action: AuditAction,
        resource_type: &str,
        resource_id: Option<&str>,
        error_code: &str,
        error_message: &str,
    ) -> Result<()> {
        self.log_action(
            user_id,
            action,
            resource_type,
            resource_id,
            AuditResult::failure(error_code, error_message),
        )
        .await
    }

    /// 记录拒绝访问
    pub async fn log_denied(
        &self,
        user_id: &str,
        action: AuditAction,
        resource_type: &str,
        resource_id: Option<&str>,
        reason: &str,
    ) -> Result<()> {
        self.log_action(
            user_id,
            action,
            resource_type,
            resource_id,
            AuditResult::denied(reason),
        )
        .await
    }

    /// 查询审计日志
    pub async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>> {
        self.storage.query(&filter).await
    }

    /// 导出审计日志
    pub async fn export(&self, format: ExportFormat, filter: AuditFilter) -> Result<Vec<u8>> {
        self.storage.export(format, &filter).await
    }

    /// 清理过期日志
    pub async fn cleanup(&self, before: DateTime<Utc>) -> Result<usize> {
        self.storage.cleanup(before).await
    }

    /// 获取日志条目数
    pub async fn count(&self) -> Result<usize> {
        self.storage.count().await
    }

    /// 脱敏处理审计条目
    fn sanitize_entry(&self, mut entry: AuditEntry) -> AuditEntry {
        if let Some(details) = &entry.details {
            entry.details = Some(self.sanitize_value(details.clone()));
        }
        entry
    }

    /// 脱敏 JSON 值
    fn sanitize_value(&self, value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                for field in &self.config.sensitive_fields {
                    if let Some(v) = map.get_mut(field) {
                        *v = Value::String("***REDACTED***".to_string());
                    }
                }
                for v in map.values_mut() {
                    *v = self.sanitize_value(v.clone());
                }
                Value::Object(map)
            }
            Value::Array(arr) => {
                Value::Array(arr.into_iter().map(|v| self.sanitize_value(v)).collect())
            }
            other => other,
        }
    }

    /// 创建审计条目构建器
    pub fn builder(
        &self,
        user_id: &str,
        action: AuditAction,
        resource_type: &str,
    ) -> AuditEntryBuilder<'_> {
        AuditEntryBuilder {
            entry: AuditEntry::new(user_id, action, resource_type),
            logger: self,
        }
    }
}

/// 审计条目构建器
pub struct AuditEntryBuilder<'a> {
    entry: AuditEntry,
    logger: &'a AuditLogger,
}

impl<'a> AuditEntryBuilder<'a> {
    pub fn with_session(mut self, session_id: &str) -> Self {
        self.entry = self.entry.with_session(session_id);
        self
    }

    pub fn with_resource_id(mut self, resource_id: &str) -> Self {
        self.entry = self.entry.with_resource_id(resource_id);
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.entry = self.entry.with_details(details);
        self
    }

    pub fn with_ip(mut self, ip_address: &str) -> Self {
        self.entry = self.entry.with_ip(ip_address);
        self
    }

    pub fn with_result(mut self, result: AuditResult) -> Self {
        self.entry = self.entry.with_result(result);
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.entry = self.entry.with_duration(duration_ms);
        self
    }

    pub async fn log(self) -> Result<()> {
        self.logger.log(self.entry).await
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(AuditConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let logger = AuditLogger::default();
        assert!(logger.config().enabled);
    }

    #[tokio::test]
    async fn test_log_action() {
        let logger = AuditLogger::default();

        logger
            .log_success("user1", AuditAction::Read, "document", Some("doc123"))
            .await
            .unwrap();

        let count = logger.count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_query() {
        let logger = AuditLogger::default();

        logger
            .log_success("user1", AuditAction::Read, "doc", None)
            .await
            .unwrap();
        logger
            .log_success("user2", AuditAction::Read, "doc", None)
            .await
            .unwrap();

        let filter = AuditFilter {
            user_id: Some("user1".to_string()),
            ..Default::default()
        };

        let entries = logger.query(filter).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].user_id, "user1");
    }

    #[tokio::test]
    async fn test_sanitize_sensitive_data() {
        let config = AuditConfig {
            sanitize_sensitive: true,
            ..Default::default()
        };
        let logger = AuditLogger::new(config);

        let entry =
            AuditEntry::new("user1", AuditAction::Create, "user").with_details(serde_json::json!({
                "username": "test",
                "password": "secret123"
            }));

        logger.log(entry).await.unwrap();

        let entries = logger.query(AuditFilter::default()).await.unwrap();
        let details = entries[0].details.as_ref().unwrap();

        assert_eq!(details.get("password").unwrap(), "***REDACTED***");
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let logger = AuditLogger::default();

        logger
            .builder("user1", AuditAction::Login, "session")
            .with_ip("192.168.1.1")
            .with_duration(100)
            .log()
            .await
            .unwrap();

        let entries = logger.query(AuditFilter::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].ip_address, Some("192.168.1.1".to_string()));
    }

    #[tokio::test]
    async fn test_disabled_logger() {
        let config = AuditConfig {
            enabled: false,
            ..Default::default()
        };
        let logger = AuditLogger::new(config);

        logger
            .log_success("user1", AuditAction::Read, "doc", None)
            .await
            .unwrap();

        let count = logger.count().await.unwrap();
        assert_eq!(count, 0);
    }
}
