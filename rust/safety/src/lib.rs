//! # Continuum Safety
//!
//! Pure validation functions for path safety, UTF-8 truncation, and secret scrubbing.
//! No async, no I/O, no heavy dependencies — suitable for fuzzing.

pub mod path_safety;
pub mod safe_truncate;
pub mod secret_scrub;
pub mod self_mod_policy;
