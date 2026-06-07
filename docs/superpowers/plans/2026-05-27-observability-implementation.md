# Observability Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 60-line stub in `rust/layer1/src/observability.rs` with a production-ready observability module providing tracing, metrics, and structured logging.

**Architecture:** Three-layer design (Tracing, Metrics, Logs) with optional OTLP and Prometheus exporters via feature flags. Uses existing `tracing` crate for spans and structured logging, with in-memory metrics when no exporter is configured.

**Tech Stack:** Rust, tracing, tracing-subscriber, parking_lot::RwLock, serde

---

## File Structure

```
rust/layer1/src/observability/
├── mod.rs           # Public API re-exports
├── config.rs        # ObservabilityConfig and LogFormat
├── metrics.rs       # Counter, Histogram, Gauge, MetricValue
├── span.rs          # SpanGuard wrapper
└── logging.rs       # Structured logging utilities

rust/layer1/Cargo.toml  # Add optional dependencies
```

**Note:** The current `observability.rs` will be replaced with a module directory for better organization.

---

## Task 1: Update Cargo.toml with Optional Dependencies

**Files:**
- Modify: `rust/layer1/Cargo.toml`

- [ ] **Step 1: Add optional dependencies for observability features**

```toml
[features]
default = []
local-embeddings = []
otel = ["opentelemetry", "tracing-opentelemetry", "opentelemetry-otlp"]
prometheus = ["metrics", "metrics-exporter-prometheus"]

[dependencies]
# ... existing dependencies ...

# Optional observability dependencies
opentelemetry = { version = "0.27", optional = true }
tracing-opentelemetry = { version = "0.28", optional = true }
opentelemetry-otlp = { version = "0.27", optional = true, features = ["grpc-tonic"] }
metrics = { version = "0.24", optional = true }
metrics-exporter-prometheus = { version = "0.15", optional = true }
```

- [ ] **Step 2: Verify Cargo.toml compiles**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo check`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add rust/layer1/Cargo.toml
git commit -m "feat(layer1): add optional observability dependencies"
```

---

## Task 2: Create Config Module

**Files:**
- Create: `rust/layer1/src/observability/config.rs`
- Create: `rust/layer1/src/observability/mod.rs` (initial)

- [ ] **Step 1: Write failing test for ObservabilityConfig**

Create test file structure first. Since we're converting from a single file to a module, we'll write the module with tests inline.

- [ ] **Step 2: Create observability module directory**

```bash
mkdir -p D:/TA/create_together_with_ali/continuum/rust/layer1/src/observability
```

- [ ] **Step 3: Create config.rs with ObservabilityConfig**

```rust
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
```

- [ ] **Step 4: Run tests to verify**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test observability::config --no-fail-fast`
Expected: 3 tests pass

- [ ] **Step 5: Commit**

```bash
git add rust/layer1/src/observability/config.rs
git commit -m "feat(layer1): add ObservabilityConfig with builder pattern"
```

---

## Task 3: Create Metrics Types

**Files:**
- Create: `rust/layer1/src/observability/metrics.rs`

- [ ] **Step 1: Create metrics.rs with Counter, Histogram, Gauge, MetricValue**

```rust
//! Metrics types for observability.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Internal metric value storage.
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

impl MetricValue {
    /// Get the counter value, or 0 if not a counter.
    pub fn as_counter(&self) -> u64 {
        match self {
            MetricValue::Counter(v) => *v,
            _ => 0,
        }
    }

    /// Get the gauge value, or 0.0 if not a gauge.
    pub fn as_gauge(&self) -> f64 {
        match self {
            MetricValue::Gauge(v) => *v,
            _ => 0.0,
        }
    }

    /// Get the histogram values, or empty if not a histogram.
    pub fn as_histogram(&self) -> &[f64] {
        match self {
            MetricValue::Histogram(v) => v,
            _ => &[],
        }
    }
}

/// Internal metrics storage shared by Counter, Histogram, Gauge.
#[derive(Debug, Default)]
pub struct MetricsStorage {
    metrics: RwLock<HashMap<String, MetricValue>>,
}

impl MetricsStorage {
    /// Create a new metrics storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment a counter metric.
    pub fn increment_counter(&self, name: &str, delta: u64) {
        let mut metrics = self.metrics.write();
        let entry = metrics.entry(name.to_string()).or_insert(MetricValue::Counter(0));
        if let MetricValue::Counter(v) = entry {
            *v += delta;
        }
    }

    /// Set a gauge metric.
    pub fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write();
        metrics.insert(name.to_string(), MetricValue::Gauge(value));
    }

    /// Record a histogram value.
    pub fn record_histogram(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write();
        let entry = metrics.entry(name.to_string()).or_insert(MetricValue::Histogram(Vec::new()));
        if let MetricValue::Histogram(v) = entry {
            v.push(value);
        }
    }

    /// Get a metric value by name.
    pub fn get(&self, name: &str) -> Option<MetricValue> {
        self.metrics.read().get(name).cloned()
    }

    /// List all metric names.
    pub fn list_names(&self) -> Vec<String> {
        self.metrics.read().keys().cloned().collect()
    }
}

/// Counter metric.
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    storage: Arc<MetricsStorage>,
}

impl Counter {
    /// Create a new counter.
    pub fn new(name: impl Into<String>, storage: Arc<MetricsStorage>) -> Self {
        Self {
            name: name.into(),
            storage,
        }
    }

    /// Increment the counter by the given delta.
    pub fn increment(&self, delta: u64) {
        #[cfg(feature = "prometheus")]
        {
            let _ = metrics::counter!(self.name.clone()).increment(delta);
        }
        
        #[cfg(not(feature = "prometheus"))]
        {
            self.storage.increment_counter(&self.name, delta);
        }
    }

    /// Get the current counter value.
    pub fn get(&self) -> u64 {
        self.storage.get(&self.name).map(|v| v.as_counter()).unwrap_or(0)
    }
}

/// Histogram metric.
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    storage: Arc<MetricsStorage>,
}

impl Histogram {
    /// Create a new histogram.
    pub fn new(name: impl Into<String>, storage: Arc<MetricsStorage>) -> Self {
        Self {
            name: name.into(),
            storage,
        }
    }

    /// Record a value in the histogram.
    pub fn record(&self, value: f64) {
        #[cfg(feature = "prometheus")]
        {
            let _ = metrics::histogram!(self.name.clone()).record(value);
        }
        
        #[cfg(not(feature = "prometheus"))]
        {
            self.storage.record_histogram(&self.name, value);
        }
    }

    /// Get all recorded values.
    pub fn get_values(&self) -> Vec<f64> {
        self.storage.get(&self.name)
            .map(|v| v.as_histogram().to_vec())
            .unwrap_or_default()
    }
}

/// Gauge metric.
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    storage: Arc<MetricsStorage>,
}

impl Gauge {
    /// Create a new gauge.
    pub fn new(name: impl Into<String>, storage: Arc<MetricsStorage>) -> Self {
        Self {
            name: name.into(),
            storage,
        }
    }

    /// Set the gauge to a value.
    pub fn set(&self, value: f64) {
        #[cfg(feature = "prometheus")]
        {
            let _ = metrics::gauge!(self.name.clone()).set(value);
        }
        
        #[cfg(not(feature = "prometheus"))]
        {
            self.storage.set_gauge(&self.name, value);
        }
    }

    /// Get the current gauge value.
    pub fn get(&self) -> f64 {
        self.storage.get(&self.name).map(|v| v.as_gauge()).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_increment() {
        let storage = Arc::new(MetricsStorage::new());
        let counter = Counter::new("test_counter", storage);
        
        counter.increment(5);
        assert_eq!(counter.get(), 5);
        
        counter.increment(3);
        assert_eq!(counter.get(), 8);
    }

    #[test]
    fn test_gauge_set() {
        let storage = Arc::new(MetricsStorage::new());
        let gauge = Gauge::new("test_gauge", storage);
        
        gauge.set(42.0);
        assert_eq!(gauge.get(), 42.0);
        
        gauge.set(100.0);
        assert_eq!(gauge.get(), 100.0);
    }

    #[test]
    fn test_histogram_record() {
        let storage = Arc::new(MetricsStorage::new());
        let histogram = Histogram::new("test_histogram", storage);
        
        histogram.record(1.0);
        histogram.record(2.0);
        histogram.record(3.0);
        
        let values = histogram.get_values();
        assert_eq!(values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_metrics_storage_list_names() {
        let storage = Arc::new(MetricsStorage::new());
        let counter = Counter::new("counter1", Arc::clone(&storage));
        let gauge = Gauge::new("gauge1", Arc::clone(&storage));
        
        counter.increment(1);
        gauge.set(1.0);
        
        let names = storage.list_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"counter1".to_string()));
        assert!(names.contains(&"gauge1".to_string()));
    }
}
```

- [ ] **Step 2: Run tests to verify**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test observability::metrics --no-fail-fast`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add rust/layer1/src/observability/metrics.rs
git commit -m "feat(layer1): add Counter, Histogram, Gauge metrics types"
```

---

## Task 4: Create SpanGuard

**Files:**
- Create: `rust/layer1/src/observability/span.rs`

- [ ] **Step 1: Create span.rs with SpanGuard**

```rust
//! Span utilities for tracing.

use tracing::Span;

/// Guard for a tracing span.
/// 
/// When dropped, the span is automatically closed.
pub struct SpanGuard {
    span: Option<Span>,
}

impl SpanGuard {
    /// Create a new span guard from a tracing span.
    pub fn new(span: Span) -> Self {
        Self { span: Some(span) }
    }

    /// Create a no-op span guard (for disabled observability).
    pub fn noop() -> Self {
        Self { span: None }
    }

    /// Set an attribute on the span.
    pub fn set_attribute(&self, key: &str, value: &str) {
        if let Some(span) = &self.span {
            span.record(key, value);
        }
    }

    /// Add an event to the span.
    pub fn add_event(&self, name: &str, attributes: &[(&str, &str)]) {
        if let Some(span) = &self.span {
            let mut fields = Vec::new();
            for (k, v) in attributes {
                fields.push(format!("{}={}", k, v));
            }
            span.record("event", &format!("{}: {}", name, fields.join(", ")));
        }
    }

    /// Get a reference to the underlying span.
    pub fn as_ref(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

impl Default for SpanGuard {
    fn default() -> Self {
        Self::noop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_guard_noop() {
        let guard = SpanGuard::noop();
        guard.set_attribute("key", "value");
        guard.add_event("test", &[("a", "b")]);
        // Should not panic
    }

    #[test]
    fn test_span_guard_default() {
        let guard = SpanGuard::default();
        assert!(guard.as_ref().is_none());
    }

    #[test]
    fn test_span_guard_with_span() {
        let span = tracing::info_span!("test_span");
        let guard = SpanGuard::new(span);
        
        guard.set_attribute("key", "value");
        guard.add_event("event", &[("status", "ok")]);
        
        assert!(guard.as_ref().is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test observability::span --no-fail-fast`
Expected: 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add rust/layer1/src/observability/span.rs
git commit -m "feat(layer1): add SpanGuard for tracing spans"
```

---

## Task 5: Create Logging Utilities

**Files:**
- Create: `rust/layer1/src/observability/logging.rs`

- [ ] **Step 1: Create logging.rs with structured logging**

```rust
//! Structured logging utilities.

use crate::observability::config::LogFormat;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::FmtContext;
use tracing_subscriber::fmt::{FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

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
    
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
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
```

- [ ] **Step 2: Run tests to verify**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test observability::logging --no-fail-fast`
Expected: 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add rust/layer1/src/observability/logging.rs
git commit -m "feat(layer1): add structured logging utilities"
```

---

## Task 6: Create Main Module and Observability Manager

**Files:**
- Create: `rust/layer1/src/observability/mod.rs` (full implementation)
- Modify: `rust/layer1/src/observability.rs` → delete and replace with re-export

- [ ] **Step 1: Create the main mod.rs with Observability manager**

```rust
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

use crate::error_handler::{ShError, ShResult};
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
```

- [ ] **Step 2: Delete old observability.rs and update lib.rs**

Delete the old `rust/layer1/src/observability.rs` file since we now have a module directory.

Update `rust/layer1/src/lib.rs` to re-export from the new module:

```rust
// In lib.rs, the existing line:
pub mod observability;
pub use observability::Observability;

// Should still work because mod.rs exports Observability
```

- [ ] **Step 3: Run all layer1 tests**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test --no-fail-fast`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add rust/layer1/src/observability/
git add rust/layer1/src/lib.rs
git commit -m "feat(layer1): implement Observability manager with tracing, metrics, logging"
```

---

## Task 7: Update Exports in lib.rs

**Files:**
- Modify: `rust/layer1/src/lib.rs`

- [ ] **Step 1: Update lib.rs to export all public types**

```rust
// Update the observability exports
pub mod observability;
pub use observability::{
    LogFormat, LogLevel, Observability, ObservabilityConfig,
    Counter, Gauge, Histogram, MetricValue, SpanGuard,
};
```

- [ ] **Step 2: Verify layer1 compiles and tests pass**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test --no-fail-fast`
Expected: All tests pass

- [ ] **Step 3: Verify layer2 and layer3 still compile**

Run: `cd D:/TA/create_together_with_ali/continuum && cargo check`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add rust/layer1/src/lib.rs
git commit -m "feat(layer1): export all observability types"
```

---

## Task 8: Final Verification

**Files:**
- All modified files

- [ ] **Step 1: Run all layer1 tests**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo test --no-fail-fast`
Expected: All tests pass

- [ ] **Step 2: Run all workspace tests**

Run: `cd D:/TA/create_together_with_ali/continuum && cargo test --no-fail-fast`
Expected: All tests pass

- [ ] **Step 3: Verify clippy passes**

Run: `cd D:/TA/create_together_with_ali/continuum/rust/layer1 && cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat(layer1): complete Observability module with tracing, metrics, logging"
```

---

## Success Criteria

1. ✅ All existing tests continue to pass
2. ✅ New tests cover config, spans, metrics, logging
3. ✅ Zero additional required dependencies (features optional)
4. ✅ Clean API that integrates with existing `tracing` usage
5. ✅ Proper documentation and examples
