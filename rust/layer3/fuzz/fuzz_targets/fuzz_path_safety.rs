#![no_main]

use libfuzzer_sys::fuzz_target;
use sh_layer3::builtin_tools::path_safety::check_path_danger;
use std::path::Path;

// Invariant: check_path_danger must never panic on arbitrary input.
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = check_path_danger(Path::new(s));
    }
});
