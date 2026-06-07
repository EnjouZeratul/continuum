//! Observability configuration.

use serde::{Deserialize, Serialize};

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogFormat {
    /// Human-readable pretty output.
    #[default]
    Pretty,
    /// JSON structured logs.
    Json,
}

/// Observability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Service name for traces and metrics.
    pub service_name: String,
    /// Enable tracing collection.
    pub tracing_enabled: bool,
    /// Enable metrics collection.
    pub metrics_enabled: bool,
    /// Log output format.
    pub log_format: LogFormat,
    /// OTLP endpoint URL (e.g., "http://localhost:4317").
    #[cfg(feature = "otel")]
    pub otlp_endpoint: Option<String>,
    /// Prometheus metrics HTTP port.
    #[cfg(feature = "prometheus")]
    pub prometheus_port: Option<u16>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "continuum".to_string(),
            tracing_enabled: true,
            metrics_enabled: true,
            log_format: LogFormat::default(),
            #[cfg(feature = "otel")]
            otlp_endpoint: None,
            #[cfg(feature = "prometheus")]
            prometheus_port: None,
        }
    }
}

impl ObservabilityConfig {
    /// Create a new config with the given service name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            ..Default::default()
        }
    }

    /// Set the log format.
    pub fn with_log_format(mut self, format: LogFormat) -> Self {
        self.log_format = format;
        self
    }

    /// Disable tracing.
    pub fn without_tracing(mut self) -> Self {
        self.tracing_enabled = false;
        self
    }

    /// Disable metrics.
    pub fn without_metrics(mut self) -> Self {
        self.metrics_enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ObservabilityConfig::default();
        assert_eq!(config.service_name, "continuum");
        assert!(config.tracing_enabled);
        assert!(config.metrics_enabled);
        assert_eq!(config.log_format, LogFormat::Pretty);
    }

    #[test]
    fn test_config_builder() {
        let config = ObservabilityConfig::new("my-service")
            .with_log_format(LogFormat::Json)
            .without_tracing();

        assert_eq!(config.service_name, "my-service");
        assert!(!config.tracing_enabled);
        assert!(config.metrics_enabled);
        assert_eq!(config.log_format, LogFormat::Json);
    }

    #[test]
    fn test_log_format_default() {
        assert_eq!(LogFormat::default(), LogFormat::Pretty);
    }
}
