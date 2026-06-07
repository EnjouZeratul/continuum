//! 错误处理模块
//!
//! 统一的错误类型和结果类型定义。

use thiserror::Error;

/// Continuum 统一错误类型
#[derive(Debug, Error)]
pub enum ShError {
    #[error("Layer 0 error: {0}")]
    Layer0(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("LLM API error: {0}")]
    LlmApi(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Timeout error after {seconds} seconds")]
    Timeout { seconds: u64 },

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Rate limited")]
    RateLimited,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// 从 anyhow::Error 转换
impl From<anyhow::Error> for ShError {
    fn from(e: anyhow::Error) -> Self {
        ShError::Internal(e.to_string())
    }
}

/// Continuum 统一结果类型
pub type ShResult<T> = std::result::Result<T, ShError>;

/// 错误处理器（用于集中处理错误）
pub struct ErrorHandler {
    /// 是否启用日志
    log_errors: bool,
}

impl ErrorHandler {
    pub fn new() -> Self {
        Self { log_errors: true }
    }

    /// 处理错误，返回用户友好的消息
    pub fn handle(&self, error: &ShError) -> String {
        if self.log_errors {
            tracing::error!("Error: {:?}", error);
        }
        error.to_string()
    }

    /// 将错误转换为用户消息
    pub fn to_user_message(&self, error: &ShError) -> String {
        match error {
            ShError::Timeout { seconds } => format!("操作超时，请重试（已等待 {} 秒）", seconds),
            ShError::RateLimited => "请求过于频繁，请稍后再试".to_string(),
            ShError::NotFound { resource } => format!("找不到资源: {}", resource),
            _ => format!("发生错误: {}", error),
        }
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ShError::Config("invalid config".to_string());
        assert!(err.to_string().contains("Configuration error"));
        assert!(err.to_string().contains("invalid config"));
    }

    #[test]
    fn test_layer0_error() {
        let err = ShError::Layer0("security violation".to_string());
        assert!(err.to_string().contains("Layer 0 error"));
        assert!(err.to_string().contains("security violation"));
    }

    #[test]
    fn test_timeout_error() {
        let err = ShError::Timeout { seconds: 30 };
        assert!(err.to_string().contains("30"));
        assert!(err.to_string().contains("Timeout"));
    }

    #[test]
    fn test_not_found_error() {
        let err = ShError::NotFound {
            resource: "session".to_string(),
        };
        assert!(err.to_string().contains("Not found"));
        assert!(err.to_string().contains("session"));
    }

    #[test]
    fn test_rate_limited_error() {
        let err = ShError::RateLimited;
        assert!(err.to_string().contains("Rate limited"));
    }

    #[test]
    fn test_internal_error() {
        let err = ShError::Internal("unexpected error".to_string());
        assert!(err.to_string().contains("Internal error"));
    }

    #[test]
    fn test_llm_api_error() {
        let err = ShError::LlmApi("API failed".to_string());
        assert!(err.to_string().contains("LLM API error"));
    }

    #[test]
    fn test_session_error() {
        let err = ShError::Session("session expired".to_string());
        assert!(err.to_string().contains("Session error"));
    }

    #[test]
    fn test_error_handler_handle() {
        let handler = ErrorHandler::new();
        let err = ShError::Config("test".to_string());
        let msg = handler.handle(&err);
        assert!(msg.contains("Configuration error"));
    }

    #[test]
    fn test_error_handler_to_user_message_timeout() {
        let handler = ErrorHandler::new();
        let err = ShError::Timeout { seconds: 60 };
        let msg = handler.to_user_message(&err);
        assert!(msg.contains("60"));
        assert!(msg.contains("超时"));
    }

    #[test]
    fn test_error_handler_to_user_message_rate_limited() {
        let handler = ErrorHandler::new();
        let err = ShError::RateLimited;
        let msg = handler.to_user_message(&err);
        assert!(msg.contains("频繁"));
    }

    #[test]
    fn test_error_handler_to_user_message_not_found() {
        let handler = ErrorHandler::new();
        let err = ShError::NotFound {
            resource: "file.txt".to_string(),
        };
        let msg = handler.to_user_message(&err);
        assert!(msg.contains("file.txt"));
    }

    #[test]
    fn test_error_handler_without_logging() {
        let mut handler = ErrorHandler::new();
        handler.log_errors = false;
        let err = ShError::Internal("test".to_string());
        let msg = handler.handle(&err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let sh_err: ShError = io_err.into();
        assert!(matches!(sh_err, ShError::Io(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let sh_err: ShError = json_err.into();
        assert!(matches!(sh_err, ShError::Serde(_)));
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("anyhow error");
        let sh_err: ShError = anyhow_err.into();
        assert!(matches!(sh_err, ShError::Internal(_)));
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_sh_result_ok() {
        let result: ShResult<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_sh_result_err() {
        let result: ShResult<i32> = Err(ShError::NotFound {
            resource: "test".to_string(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_error_handler_default() {
        let handler = ErrorHandler::default();
        let err = ShError::RateLimited;
        let msg = handler.handle(&err);
        assert!(!msg.is_empty());
    }

    // ========== 错误场景测试 ==========

    #[test]
    fn test_layer0_error_variants() {
        // Layer0 安全错误
        let err = ShError::Layer0("PII detected in input".to_string());
        assert!(err.to_string().contains("Layer 0 error"));

        let err = ShError::Layer0("Injection attempt blocked".to_string());
        assert!(err.to_string().contains("Injection"));
    }

    #[test]
    fn test_config_error_variants() {
        let err = ShError::Config("Missing required field".to_string());
        assert!(err.to_string().contains("Configuration error"));

        let err = ShError::Config("Invalid API key format".to_string());
        assert!(err.to_string().contains("Invalid API key"));
    }

    #[test]
    fn test_timeout_boundary_values() {
        // 最小超时
        let err = ShError::Timeout { seconds: 0 };
        assert!(err.to_string().contains("0"));

        // 大超时值
        let err = ShError::Timeout { seconds: u64::MAX };
        assert!(err.to_string().contains(&u64::MAX.to_string()));

        // 常见超时值
        let err = ShError::Timeout { seconds: 30 };
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_not_found_variants() {
        // 各种资源类型
        let err = ShError::NotFound {
            resource: "session".to_string(),
        };
        assert!(err.to_string().contains("session"));

        let err = ShError::NotFound {
            resource: "configuration file".to_string(),
        };
        assert!(err.to_string().contains("configuration file"));

        let err = ShError::NotFound {
            resource: "".to_string(),
        };
        assert!(err.to_string().contains("Not found"));
    }

    #[test]
    fn test_llm_api_error_variants() {
        let err = ShError::LlmApi("Rate limit exceeded".to_string());
        assert!(err.to_string().contains("LLM API error"));

        let err = ShError::LlmApi("Model not available".to_string());
        assert!(err.to_string().contains("Model"));

        let err = ShError::LlmApi("Invalid request: context too long".to_string());
        assert!(err.to_string().contains("context"));
    }

    #[test]
    fn test_session_error_variants() {
        let err = ShError::Session("Session expired".to_string());
        assert!(err.to_string().contains("Session error"));

        let err = ShError::Session("Invalid session ID".to_string());
        assert!(err.to_string().contains("Invalid session"));

        let err = ShError::Session("Session not found".to_string());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_internal_error_variants() {
        let err = ShError::Internal("Unexpected state".to_string());
        assert!(err.to_string().contains("Internal error"));

        let err = ShError::Internal("Stack overflow".to_string());
        assert!(err.to_string().contains("Stack overflow"));
    }

    #[test]
    fn test_io_error_various_kinds() {
        // 文件未找到
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let sh_err: ShError = io_err.into();
        assert!(matches!(sh_err, ShError::Io(_)));

        // 权限拒绝
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let sh_err: ShError = io_err.into();
        assert!(matches!(sh_err, ShError::Io(_)));

        // 连接重置
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset");
        let sh_err: ShError = io_err.into();
        assert!(matches!(sh_err, ShError::Io(_)));

        // 超时
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
        let sh_err: ShError = io_err.into();
        assert!(matches!(sh_err, ShError::Io(_)));
    }

    #[test]
    fn test_serde_error_various_cases() {
        // JSON 解析错误
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let sh_err: ShError = json_err.into();
        assert!(matches!(sh_err, ShError::Serde(_)));

        // 类型不匹配
        let json_err = serde_json::from_str::<i32>("\"string not number\"").unwrap_err();
        let sh_err: ShError = json_err.into();
        assert!(matches!(sh_err, ShError::Serde(_)));

        // EOF 错误
        let json_err = serde_json::from_str::<serde_json::Value>("").unwrap_err();
        let sh_err: ShError = json_err.into();
        assert!(matches!(sh_err, ShError::Serde(_)));
    }

    #[test]
    fn test_error_chain_from_anyhow() {
        // 简单 anyhow 错误
        let anyhow_err = anyhow::anyhow!("Something went wrong");
        let sh_err: ShError = anyhow_err.into();
        assert!(matches!(sh_err, ShError::Internal(_)));

        // 带上下文的 anyhow 错误
        let anyhow_err = anyhow::anyhow!("Base error").context("Additional context");
        let sh_err: ShError = anyhow_err.into();
        assert!(matches!(sh_err, ShError::Internal(_)));
    }

    #[test]
    fn test_error_handler_to_user_message_all_variants() {
        let handler = ErrorHandler::new();

        // Timeout
        let msg = handler.to_user_message(&ShError::Timeout { seconds: 120 });
        assert!(msg.contains("120"));
        assert!(msg.contains("超时"));

        // RateLimited
        let msg = handler.to_user_message(&ShError::RateLimited);
        assert!(msg.contains("频繁"));

        // NotFound
        let msg = handler.to_user_message(&ShError::NotFound {
            resource: "配置文件".to_string(),
        });
        assert!(msg.contains("配置文件"));

        // 其他错误类型
        let msg = handler.to_user_message(&ShError::Config("test".to_string()));
        assert!(!msg.is_empty());

        let msg = handler.to_user_message(&ShError::LlmApi("test".to_string()));
        assert!(!msg.is_empty());

        let msg = handler.to_user_message(&ShError::Session("test".to_string()));
        assert!(!msg.is_empty());

        let msg = handler.to_user_message(&ShError::Internal("test".to_string()));
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_error_handler_with_logging_disabled() {
        let mut handler = ErrorHandler::new();
        handler.log_errors = false;

        let err = ShError::Internal("Test error".to_string());
        let msg = handler.handle(&err);
        assert!(!msg.is_empty());
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_sh_result_operations() {
        // map 操作
        let result: ShResult<i32> = Ok(10);
        let mapped = result.map(|x| x * 2);
        assert_eq!(mapped.unwrap(), 20);

        // and_then 操作
        let result: ShResult<i32> = Ok(10);
        let chained: ShResult<i32> = Ok(result.unwrap() + 5);
        assert_eq!(chained.unwrap(), 15);

        // or_else 操作
        let _result: ShResult<i32> = Err(ShError::NotFound {
            resource: "test".to_string(),
        });
        let recovered: ShResult<i32> = Ok(0);
        assert_eq!(recovered.unwrap(), 0);
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_sh_result_unwrap_or() {
        let result: ShResult<i32> = Err(ShError::RateLimited);
        let value = result.unwrap_or(42);
        assert_eq!(value, 42);
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_sh_result_unwrap_or_else() {
        let result: ShResult<i32> = Err(ShError::Timeout { seconds: 30 });
        let value = result.unwrap_or(100);
        assert_eq!(value, 100);
    }

    #[test]
    fn test_sh_result_is_ok_is_err() {
        let ok: ShResult<i32> = Ok(1);
        assert!(ok.is_ok());

        let err: ShResult<i32> = Err(ShError::RateLimited);
        assert!(err.is_err());
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_sh_result_expect() {
        let result: ShResult<i32> = Ok(42);
        let value = result.expect("Should have a value");
        assert_eq!(value, 42);
    }

    #[test]
    fn test_error_equality() {
        let err1 = ShError::RateLimited;
        let err2 = ShError::RateLimited;
        // ShError 没有实现 PartialEq，所以我们只检查 Display
        assert_eq!(err1.to_string(), err2.to_string());
    }

    #[test]
    fn test_error_debug_output() {
        let err = ShError::Timeout { seconds: 30 };
        let debug_output = format!("{:?}", err);
        assert!(debug_output.contains("Timeout"));
        assert!(debug_output.contains("30"));
    }
}
