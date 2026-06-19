//! Property-based tests for security invariants (Task 2.2).
//!
//! Tests invariants holding for ALL inputs. Run with `PROPTEST_CASES=1024`
//! for stricter runs (default 256). Zero-arg tests stay outside `proptest!`
//! (the macro requires at least one `in` parameter).

use proptest::prelude::*;
use sh_layer3::builtin_tools::path_safety::check_path_danger;
use sh_layer3::builtin_tools::safe_truncate::{safe_truncate_bytes, safe_truncate_chars};
use sh_layer3::builtin_tools::secret_scrub::{is_valid_env_name, SecretScrubber};
use std::path::Path;

proptest! {
    #[test]
    fn stb_never_panics(s in ".*", max in 0..2000usize) {
        let _ = safe_truncate_bytes(&s, max);
    }

    #[test]
    fn stb_prefix_and_bounded(s in ".*", max in 0..2000usize) {
        let t = safe_truncate_bytes(&s, max);
        prop_assert!(t.len() <= max || t.len() == s.len());
        prop_assert!(s.starts_with(t));
    }

    #[test]
    fn stb_utf8_safe_on_random(s in ".*", max in 0..500usize) {
        let t = safe_truncate_bytes(&s, max);
        prop_assert!(std::str::from_utf8(t.as_bytes()).is_ok());
        prop_assert!(s.starts_with(t));
    }

    #[test]
    fn stc_char_count(s in ".*", max in 0..500usize) {
        let t = safe_truncate_chars(&s, max);
        prop_assert!(t.chars().count() <= max);
        prop_assert!(s.starts_with(t));
    }

    #[test]
    fn cpd_never_panics(p in ".*") {
        let _ = check_path_danger(Path::new(&p));
    }

    #[test]
    fn cpd_etc_subtree_critical(sub in "[a-zA-Z0-9_/]*") {
        let path = format!("/etc/{}", sub);
        prop_assert!(check_path_danger(Path::new(&path)).is_critical);
    }

    #[test]
    fn ss_aws_key(suffix in "[A-Z0-9]{16}") {
        let secret = format!("AKIA{}", suffix);
        let input = format!("config: {} end", secret);
        let cleaned = SecretScrubber::new().scrub(&input);
        prop_assert!(!cleaned.contains(&secret), "leaked: {}", cleaned);
    }

    #[test]
    fn ss_openai_key(body in "[a-zA-Z0-9]{30}") {
        let secret = format!("sk-{}", body);
        let input = format!("before {} after", secret);
        let cleaned = SecretScrubber::new().scrub(&input);
        prop_assert!(!cleaned.contains(&secret), "leaked");
    }

    #[test]
    fn ss_idempotent(s in ".*") {
        let sc = SecretScrubber::new();
        let once = sc.scrub(&s);
        let twice = sc.scrub(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn ss_preserves_plain_text(s in "[a-zA-Z0-9 .,!?]{0,100}") {
        prop_assert_eq!(SecretScrubber::new().scrub(&s), s);
    }

    #[test]
    fn ven_valid(first in "[A-Za-z_]", rest in "[A-Za-z0-9_]{0,50}") {
        let name = format!("{}{}", first, rest);
        prop_assert!(is_valid_env_name(&name), "invalid: {}", name);
    }

    #[test]
    fn ven_too_long(n in 257..500usize) {
        let name: String = "a".repeat(n);
        prop_assert!(!is_valid_env_name(&name));
    }
}

// Zero-arg invariant (proptest! requires an `in` parameter, so this is a plain test)
#[test]
fn cpd_root_always_critical() {
    for _ in 0..100 {
        assert!(check_path_danger(Path::new("/")).is_critical);
    }
}
