//! Observability metrics for built-in tools.
//!
//! OTel-aligned field naming (see semantic conventions):
//! - `code.function`: tool / function name
//! - `error.type`: high-level rejection category
//! - `session.id`: session identifier
//! - `file.path`: canonical file path involved
//!
//! Counters are simple `AtomicU64` (no prometheus / opentelemetry SDK
//! dependency). External monitoring can poll `stale_read_rejection_count()`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

/// Total stale-read rejections across all tools / reasons / policies.
static STALE_READ_REJECTIONS: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

/// Record a stale-read rejection event.
///
/// Emits a structured `tracing::warn!` with OTel semantic-convention fields
/// and increments the global counter. Called from `EditFileTool` /
/// `WriteFileTool` when stale-read check rejects an operation.
pub fn record_stale_read_rejection(
    tool_name: &'static str,
    reason: &'static str,
    session_id: &str,
    file_path: &str,
    last_read_at: Option<&str>,
) {
    STALE_READ_REJECTIONS.fetch_add(1, Ordering::Relaxed);

    // OTel semantic-convention field names use dots (code.function, error.type,
    // session.id, file.path). Rust `tracing` field names must be valid Rust
    // identifiers, so we use underscores here. Downstream OTel integration can
    // remap field names if needed (e.g., via tracing-opentelemetry layer config).
    tracing::warn!(
        target: "continuum.tools.fileops",
        code_function = tool_name,
        error_type = "stale_read_rejected",
        session_id = %session_id,
        file_path = %file_path,
        stale_reason = reason,
        last_read_at = ?last_read_at,
        "stale-read rejection"
    );
}

/// Total stale-read rejections since process start.
///
/// Monitoring systems can poll this periodically and compute deltas.
/// Attributes (tool_name / reason / policy) are not preserved in this
/// minimal counter — full cardinality requires OTel SDK integration.
pub fn stale_read_rejection_count() -> u64 {
    STALE_READ_REJECTIONS.load(Ordering::Relaxed)
}

/// Reset counter (for testing only).
#[cfg(test)]
pub fn reset_stale_read_rejection_count() {
    STALE_READ_REJECTIONS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_increments() {
        let before = stale_read_rejection_count();
        // Directly increment by calling record (will emit warn log too)
        record_stale_read_rejection(
            "edit_file",
            "modified_after_read",
            "test-session",
            "/tmp/test.txt",
            None,
        );
        let after = stale_read_rejection_count();
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_reset() {
        reset_stale_read_rejection_count();
        assert_eq!(stale_read_rejection_count(), 0);
    }
}
