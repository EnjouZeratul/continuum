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
        let entry = metrics
            .entry(name.to_string())
            .or_insert(MetricValue::Counter(0));
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
        let entry = metrics
            .entry(name.to_string())
            .or_insert(MetricValue::Histogram(Vec::new()));
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
        self.storage
            .get(&self.name)
            .map(|v| v.as_counter())
            .unwrap_or(0)
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
        self.storage
            .get(&self.name)
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
        self.storage
            .get(&self.name)
            .map(|v| v.as_gauge())
            .unwrap_or(0.0)
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
