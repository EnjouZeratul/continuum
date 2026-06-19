//! UTF-8 safe string truncation helpers.
//!
//! Naive `&s[..n]` slicing panics when `n` falls inside a multi-byte UTF-8
//! character. These helpers find the nearest char boundary, used by file tools
//! to bound output safely.

/// Truncate to at most `max_chars` Unicode characters.
/// Returns a slice of `s` ending on a UTF-8 char boundary.
pub fn safe_truncate_chars(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    let byte_end = s
        .char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    &s[..byte_end]
}

/// Truncate to at most `max_bytes` bytes, ending on a UTF-8 char boundary.
/// If `max_bytes` lands inside a multi-byte char, find the previous boundary.
pub fn safe_truncate_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_truncate_chars_ascii() {
        assert_eq!(safe_truncate_chars("hello world", 5), "hello");
    }

    #[test]
    fn test_safe_truncate_chars_no_truncation_needed() {
        assert_eq!(safe_truncate_chars("short", 100), "short");
    }

    #[test]
    fn test_safe_truncate_chars_multibyte() {
        // "你好世界" — each char is 3 bytes in UTF-8
        let s = "你好世界";
        assert_eq!(s.len(), 12); // 4 chars × 3 bytes
        let truncated = safe_truncate_chars(s, 2);
        assert_eq!(truncated, "你好");
        assert_eq!(truncated.len(), 6);
    }

    #[test]
    fn test_safe_truncate_bytes_ascii() {
        assert_eq!(safe_truncate_bytes("hello world", 5), "hello");
    }

    #[test]
    fn test_safe_truncate_bytes_multibyte_boundary_safe() {
        // Cut at byte 4 — falls inside second char (bytes 3-5), should retreat to 3.
        let s = "你好世界";
        let truncated = safe_truncate_bytes(s, 4);
        assert_eq!(truncated, "你");
        assert_eq!(truncated.len(), 3);
    }

    #[test]
    fn test_safe_truncate_bytes_exact_boundary() {
        // Cut at byte 6 — exact boundary between 2nd and 3rd char.
        let s = "你好世界";
        let truncated = safe_truncate_bytes(s, 6);
        assert_eq!(truncated, "你好");
    }

    #[test]
    fn test_safe_truncate_bytes_zero_max() {
        assert_eq!(safe_truncate_bytes("hello", 0), "");
    }

    #[test]
    fn test_safe_truncate_chars_empty_input() {
        assert_eq!(safe_truncate_chars("", 10), "");
    }
}
