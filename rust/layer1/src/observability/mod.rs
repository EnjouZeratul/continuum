//! Observability module.
//!
//! Provides tracing, metrics, and structured logging capabilities.

pub mod config;
pub mod logging;
pub mod metrics;
pub mod span;

pub use config::{LogFormat, ObservabilityConfig};
pub use logging::{log, LogLevel};
pub use metrics::{Counter, Gauge, Histogram, MetricValue, MetricsStorage};
pub use span::SpanGuard;

use crate::error_handler::ShResult;
use std::sync::Arc;

/// Main observability manager.
pub struct Observability {
    config: ObservabilityConfig,
    metrics_storage: Arc<MetricsStorage>,
}

impl Observability {
    /// Create a new observability instance with the given configuration.
    pub fn new(config: ObservabilityConfig) -> ShResult<Self> {
        // Initialize tracing subscriber if enabled
        if config.tracing_enabled {
            // Note: Subscriber can only be set once per process
            // We use try_init which handles the "already set" case gracefully
            let _ = logging::init_subscriber(config.log_format);
        }

        Ok(Self {
            config,
            metrics_storage: Arc::new(MetricsStorage::new()),
        })
    }

    /// Create a new observability instance with default configuration.
    pub fn with_defaults() -> ShResult<Self> {
        Self::new(ObservabilityConfig::default())
    }

    /// Create a new span for tracing.
    pub fn span(&self, name: &str) -> SpanGuard {
        if !self.config.tracing_enabled {
            return SpanGuard::noop();
        }

        let span = tracing::info_span!(
            "operation",
            service = %self.config.service_name,
            name = name
        );
        SpanGuard::new(span)
    }

    /// Get or create a counter metric.
    pub fn counter(&self, name: &str) -> Counter {
        Counter::new(name, Arc::clone(&self.metrics_storage))
    }

    /// Get or create a histogram metric.
    pub fn histogram(&self, name: &str) -> Histogram {
        Histogram::new(name, Arc::clone(&self.metrics_storage))
    }

    /// Get or create a gauge metric.
    pub fn gauge(&self, name: &str) -> Gauge {
        Gauge::new(name, Arc::clone(&self.metrics_storage))
    }

    /// Log a structured message.
    pub fn log(&self, level: LogLevel, message: &str, attributes: &[(&str, &str)]) {
        if self.config.tracing_enabled {
            logging::log(level, message, attributes);
        }
    }

    /// Get a metric value by name.
    pub fn get_metric(&self, name: &str) -> Option<MetricValue> {
        self.metrics_storage.get(name)
    }

    /// List all metric names.
    pub fn list_metrics(&self) -> Vec<String> {
        self.metrics_storage.list_names()
    }

    /// Get the configuration.
    pub fn config(&self) -> &ObservabilityConfig {
        &self.config
    }

    /// Graceful shutdown.
    pub fn shutdown(self) -> ShResult<()> {
        #[cfg(feature = "otel")]
        {
            // Flush pending traces
            if self.config.tracing_enabled {
                tracing::info!("Shutting down observability");
            }
        }

        Ok(())
    }
}

impl Default for Observability {
    fn default() -> Self {
        Self::with_defaults().expect("Failed to create default Observability")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_creation() {
        let config = ObservabilityConfig::new("test-service");
        let obs = Observability::new(config).expect("Failed to create observability");
        assert_eq!(obs.config().service_name, "test-service");
    }

    #[test]
    fn test_observability_default() {
        let obs = Observability::default();
        assert_eq!(obs.config().service_name, "continuum");
    }

    #[test]
    fn test_span_creation() {
        let obs = Observability::default();
        let span = obs.span("test_operation");
        span.set_attribute("key", "value");
    }

    #[test]
    fn test_counter_operations() {
        let obs = Observability::default();
        let counter = obs.counter("requests");

        counter.increment(1);
        counter.increment(2);

        let value = obs.get_metric("requests").expect("Counter should exist");
        assert_eq!(value.as_counter(), 3);
    }

    #[test]
    fn test_gauge_operations() {
        let obs = Observability::default();
        let gauge = obs.gauge("temperature");

        gauge.set(25.5);

        let value = obs.get_metric("temperature").expect("Gauge should exist");
        assert_eq!(value.as_gauge(), 25.5);
    }

    #[test]
    fn test_histogram_operations() {
        let obs = Observability::default();
        let histogram = obs.histogram("latency");

        histogram.record(0.1);
        histogram.record(0.2);
        histogram.record(0.3);

        let value = obs.get_metric("latency").expect("Histogram should exist");
        let values = value.as_histogram();
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_list_metrics() {
        let obs = Observability::default();

        obs.counter("c1").increment(1);
        obs.gauge("g1").set(1.0);
        obs.histogram("h1").record(1.0);

        let names = obs.list_metrics();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"c1".to_string()));
        assert!(names.contains(&"g1".to_string()));
        assert!(names.contains(&"h1".to_string()));
    }

    #[test]
    fn test_disabled_tracing() {
        let config = ObservabilityConfig::default().without_tracing();
        let obs = Observability::new(config).expect("Failed to create observability");

        let span = obs.span("test");
        // Span should be a no-op (no underlying span)
        assert!(span.as_ref().is_none());
    }

    #[test]
    fn test_log_message() {
        let obs = Observability::default();
        obs.log(LogLevel::Info, "test message", &[("key", "value")]);
        // Should not panic
    }
}
