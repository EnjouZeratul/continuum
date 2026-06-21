#![no_main]

use libfuzzer_sys::fuzz_target;
use sh_safety::path_safety::check_path_danger;
use std::path::Path;

// Assertion-based fuzz: not just "no panic" but verifies security invariants.
// Regression guard for the "/" starts_with bug that flagged all POSIX paths.
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let danger = check_path_danger(Path::new(s));

        // Invariant 1: never panics (implicit — we got here)

        // Invariant 2: "/" is always critical
        if s == "/" {
            assert!(danger.is_critical, "'/' must be critical");
        }

        // Invariant 3: "/etc/xxx" subtree is always critical
        if s.starts_with("/etc/") || s == "/etc" {
            assert!(danger.is_critical, "/etc subtree must be critical: {}", s);
        }

        // Invariant 4: "/tmp/xxx" is NEVER critical (regression guard)
        if s.starts_with("/tmp/") || s == "/tmp" {
            assert!(!danger.is_critical, "/tmp must NOT be critical: {}", s);
        }

        // Invariant 5: "/home/xxx" (non-.ssh) is never critical
        if s.starts_with("/home/") && !s.contains(".ssh") {
            assert!(!danger.is_critical, "/home/xxx must NOT be critical: {}", s);
        }
    }
});
