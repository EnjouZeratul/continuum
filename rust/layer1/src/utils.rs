//! Utility functions for ID generation
//!
//! Provides standardized short ID generation across all layers.

use uuid::Uuid;

/// Generate a short 8-character ID from UUID v4
///
/// This is the standard ID format used across the continuum project.
/// Format: 8 lowercase hex characters (e.g., "a1b2c3d4")
///
/// # Example
/// ```
/// use sh_layer1::utils::generate_short_id;
///
/// let id = generate_short_id();
/// assert_eq!(id.len(), 8);
/// assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
/// ```
pub fn generate_short_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

/// Generate a prefixed short ID
///
/// # Example
/// ```
/// use sh_layer1::utils::generate_prefixed_id;
///
/// let task_id = generate_prefixed_id("task");
/// assert!(task_id.starts_with("task_"));
/// assert_eq!(task_id.len(), 13); // "task_" + 8 chars
/// ```
pub fn generate_prefixed_id(prefix: &str) -> String {
    format!("{}_{}", prefix, generate_short_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_short_id() {
        let id = generate_short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_short_id_uniqueness() {
        let id1 = generate_short_id();
        let id2 = generate_short_id();
        // Very unlikely to be the same
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_prefixed_id() {
        let id = generate_prefixed_id("task");
        assert!(id.starts_with("task_"));
        assert_eq!(id.len(), 13);
    }

    #[test]
    fn test_generate_prefixed_id_various() {
        let tc_id = generate_prefixed_id("tc");
        assert!(tc_id.starts_with("tc_"));

        let call_id = generate_prefixed_id("call");
        assert!(call_id.starts_with("call_"));
    }
}
