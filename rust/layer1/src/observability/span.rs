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
            span.record("event", format!("{}: {}", name, fields.join(", ")));
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
