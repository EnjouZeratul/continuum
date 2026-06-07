//! Structured logging utilities.

use crate::observability::config::LogFormat;
use tracing::Level;

/// Log level wrapper for structured logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

impl From<Level> for LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::TRACE => LogLevel::Trace,
            Level::DEBUG => LogLevel::Debug,
            Level::INFO => LogLevel::Info,
            Level::WARN => LogLevel::Warn,
            Level::ERROR => LogLevel::Error,
        }
    }
}

/// Log a structured message with attributes.
pub fn log(level: LogLevel, message: &str, attributes: &[(&str, &str)]) {
    let level: Level = level.into();

    match level {
        Level::TRACE => tracing::trace!(message, ?attributes),
        Level::DEBUG => tracing::debug!(message, ?attributes),
        Level::INFO => tracing::info!(message, ?attributes),
        Level::WARN => tracing::warn!(message, ?attributes),
        Level::ERROR => tracing::error!(message, ?attributes),
    }
}

/// Initialize the tracing subscriber with the given format.
pub fn init_subscriber(log_format: LogFormat) -> Result<(), String> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    match log_format {
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .try_init()
                .map_err(|e| format!("Failed to init subscriber: {}", e))?;
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().json())
                .try_init()
                .map_err(|e| format!("Failed to init subscriber: {}", e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(Level::from(LogLevel::Trace), Level::TRACE);
        assert_eq!(Level::from(LogLevel::Debug), Level::DEBUG);
        assert_eq!(Level::from(LogLevel::Info), Level::INFO);
        assert_eq!(Level::from(LogLevel::Warn), Level::WARN);
        assert_eq!(Level::from(LogLevel::Error), Level::ERROR);
    }

    #[test]
    fn test_log_level_reverse_conversion() {
        assert_eq!(LogLevel::from(Level::TRACE), LogLevel::Trace);
        assert_eq!(LogLevel::from(Level::DEBUG), LogLevel::Debug);
        assert_eq!(LogLevel::from(Level::INFO), LogLevel::Info);
        assert_eq!(LogLevel::from(Level::WARN), LogLevel::Warn);
        assert_eq!(LogLevel::from(Level::ERROR), LogLevel::Error);
    }

    #[test]
    fn test_log_function() {
        // Should not panic
        log(LogLevel::Info, "test message", &[("key", "value")]);
        log(LogLevel::Debug, "another message", &[]);
    }
}
