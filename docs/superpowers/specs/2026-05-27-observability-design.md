# Observability Module Design

> Date: 2026-05-27
> Status: Draft
> Task: #79 - Complete Observability module in Layer1

## Overview

Replace the 60-line stub in `rust/layer1/src/observability.rs` with a production-ready observability module providing tracing, metrics, and structured logging.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Observability                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   Tracing   │  │   Metrics   │  │    Logs     │  │
│  │  (tracing)  │  │  (metrics)  │  │ (tracing)   │  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  │
│         │                │                │         │
│  ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐  │
│  │OTLP Exporter│  │Prom Exporter│  │JSON/Console │  │
│  │  (optional) │  │  (optional) │  │   (always)  │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Components

### 1. ObservabilityConfig

Configuration struct for initialization:

```rust
pub struct ObservabilityConfig {
    /// Service name for traces and metrics
    pub service_name: String,
    /// Enable tracing collection
    pub tracing_enabled: bool,
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Log output format
    pub log_format: LogFormat,
    /// OTLP endpoint URL (e.g., "http://localhost:4317")
    pub otlp_endpoint: Option<String>,
    /// Prometheus metrics HTTP port
    pub prometheus_port: Option<u16>,
}

pub enum LogFormat {
    /// Human-readable pretty output
    Pretty,
    /// JSON structured logs
    Json,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "continuum".to_string(),
            tracing_enabled: true,
            metrics_enabled: true,
            log_format: LogFormat::Pretty,
            otlp_endpoint: None,
            prometheus_port: None,
        }
    }
}
```

### 2. Observability (Main Manager)

```rust
pub struct Observability {
    config: ObservabilityConfig,
    /// In-memory metrics storage (used when prometheus feature disabled)
    metrics: std::sync::RwLock<std::collections::HashMap<String, MetricValue>>,
    /// Guard for OTLP tracer provider shutdown (used when otel feature enabled)
    #[cfg(feature = "otel")]
    tracer_provider: Option<opentelemetry::sdk::trace::TracerProvider>,
}

/// Internal metric value storage
enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

impl Observability {
    /// Initialize observability with configuration
    pub fn new(config: ObservabilityConfig) -> ShResult<Self>;

    /// Create a new span for tracing
    pub fn span(&self, name: &str) -> SpanGuard;

    /// Get or create a counter metric
    pub fn counter(&self, name: &str) -> Counter;

    /// Get or create a histogram metric
    pub fn histogram(&self, name: &str) -> Histogram;

    /// Get or create a gauge metric
    pub fn gauge(&self, name: &str) -> Gauge;

    /// Record a structured log event
    pub fn log(&self, level: Level, message: &str, attributes: &[(&str, &str)]);

    /// Graceful shutdown
    pub fn shutdown(self) -> ShResult<()>;
}
```

### 3. SpanGuard

Wrapper for tracing spans:

```rust
pub struct SpanGuard {
    inner: Option<tracing::Span>,
}

impl SpanGuard {
    /// Set an attribute on the span
    pub fn set_attribute(&self, key: &str, value: &str);

    /// Add an event to the span
    pub fn add_event(&self, name: &str, attributes: &[(&str, &str)]);
}
```

### 4. Metrics Types

```rust
pub struct Counter {
    name: String,
    observability: std::sync::Arc<ObservabilityInner>,
}

impl Counter {
    pub fn increment(&self, delta: u64) {
        // When prometheus feature enabled: use metrics crate
        // When disabled: update in-memory counter
    }
}

pub struct Histogram {
    name: String,
    observability: std::sync::Arc<ObservabilityInner>,
}

impl Histogram {
    pub fn record(&self, value: f64) {
        // When prometheus feature enabled: use metrics crate
        // When disabled: append to in-memory histogram
    }
}

pub struct Gauge {
    name: String,
    observability: std::sync::Arc<ObservabilityInner>,
}

impl Gauge {
    pub fn set(&self, value: f64) {
        // When prometheus feature enabled: use metrics crate
        // When disabled: update in-memory gauge
    }
}
```

**Behavior without features:**
- Metrics are stored in-memory in a `HashMap<String, MetricValue>`
- Useful for testing and debugging
- Can be retrieved via `Observability::get_metric_value(name)` for inspection

## Feature Flags

Add to `rust/layer1/Cargo.toml`:

```toml
[features]
default = []
otel = ["opentelemetry", "tracing-opentelemetry", "opentelemetry-otlp"]
prometheus = ["metrics", "metrics-exporter-prometheus"]

[dependencies]
# ... existing dependencies ...

# Optional observability dependencies
opentelemetry = { version = "0.27", optional = true }
tracing-opentelemetry = { version = "0.28", optional = true }
opentelemetry-otlp = { version = "0.27", optional = true }
metrics = { version = "0.24", optional = true }
metrics-exporter-prometheus = { version = "0.15", optional = true }
```

## Implementation Plan

### Phase 1: Core Implementation (No external deps)
1. Define `ObservabilityConfig` with defaults
2. Implement basic `Observability` with tracing integration
3. Implement `SpanGuard` wrapping `tracing::Span`
4. Implement structured logging with JSON/Pretty formats
5. Add basic in-memory metrics (counter, histogram, gauge)

### Phase 2: Feature-Gated Exports (Optional deps)
1. Add OTLP exporter behind `otel` feature
2. Add Prometheus exporter behind `prometheus` feature
3. Implement proper shutdown handling

### Phase 3: Testing
1. Unit tests for config, spans, metrics
2. Integration tests for log format output
3. Feature-specific tests for OTLP and Prometheus

## Error Handling

All errors use existing `ShError` and `ShResult` from `error_handler.rs`:

```rust
impl Observability {
    pub fn new(config: ObservabilityConfig) -> ShResult<Self> {
        // Errors: Config validation, subscriber initialization
    }
}
```

## Integration Points

### Usage in Other Layers

```rust
// In layer2 or layer3
use sh_layer1::{Observability, ObservabilityConfig};

let obs = Observability::new(ObservabilityConfig {
    service_name: "continuum-agent".to_string(),
    log_format: LogFormat::Json,
    ..Default::default()
})?;

let span = obs.span("agent_execution");
span.set_attribute("model", "claude-3");
// ... do work ...
obs.counter("tokens_used").increment(1000);
```

### Shutdown Handling

```rust
// At application exit
obs.shutdown()?;  // Flushes all pending traces/metrics
```

## Success Criteria

1. All existing tests continue to pass
2. New tests cover config, spans, metrics, logging
3. Zero additional required dependencies (features optional)
4. Clean API that integrates with existing `tracing` usage
5. Proper documentation and examples
