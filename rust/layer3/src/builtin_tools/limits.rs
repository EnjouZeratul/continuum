//! Shared configuration for file operation tools.
//!
//! Provides size / line / binary-detection limits that all file tools honor.
//! Follows the project's config-builder pattern (see `WebSearchTool::with_config`,
//! `SaveMemoryTool::with_store`).

use std::sync::Arc;

/// Limits shared by `ReadFileTool` / `WriteFileTool` / `EditFileTool` / `ListDirectoryTool`.
#[derive(Debug, Clone)]
pub struct FileOpsLimits {
    /// Maximum byte size for `read_file` / `edit_file` pre-checks.
    /// Files larger than this are rejected before any allocation.
    /// Default: 10 MiB.
    pub max_read_bytes: u64,

    /// Maximum byte size for `write_file` content argument.
    /// Default: 10 MiB.
    pub max_write_bytes: u64,

    /// Maximum byte size of file that `edit_file` accepts.
    /// Default: 10 MiB.
    pub max_edit_bytes: u64,

    /// Default line limit when caller does not pass `limit`.
    /// Default: 2000 lines.
    pub default_read_lines: usize,

    /// Maximum characters per line in `read_file` output.
    /// Lines exceeding this are truncated.
    /// Default: 2000 chars.
    pub max_line_chars: usize,

    /// Number of bytes to sniff from file head for binary detection.
    /// Default: 8192 bytes (matches Git's xdiff/xutils heuristic).
    pub binary_sniff_bytes: usize,

    /// Maximum entries returned by `list_directory`.
    /// Default: 1000 entries.
    pub max_dir_entries: usize,

    // === P0: Shell (v1.0.4) ===
    /// Maximum stdout/stderr size returned by `bash`. Default: 1 MiB.
    pub max_bash_output_bytes: u64,
    /// Maximum command string length. Default: 8192 chars.
    pub max_bash_command_chars: usize,
    /// Default bash timeout. Default: 30000 ms.
    pub bash_default_timeout_ms: u64,
    /// Hard cap on bash timeout. Default: 300000 ms (5 min).
    pub bash_max_timeout_ms: u64,

    // === P0: Delete (v1.0.4) ===
    /// Maximum bytes delete_file accepts without force=true. Default: 100 MiB.
    pub max_delete_bytes: u64,
    /// Maximum file count delete_file accepts without force=true. Default: 10000.
    pub max_delete_file_count: u64,
    /// If true, delete_file moves to OS trash instead of permanent delete. Default: true.
    pub enable_trash: bool,

    // === P0: Network (v1.0.4) ===
    /// Maximum HTTP response body. Default: 10 MiB.
    pub max_http_response_bytes: u64,
    /// Maximum HTTP request body. Default: 1 MiB.
    pub max_http_request_body_bytes: u64,
    /// Maximum response header count. Default: 50.
    pub max_http_header_count: usize,
    /// Maximum redirects followed. Default: 0 (no redirects).
    pub max_http_redirect: usize,
    /// Default HTTP timeout. Default: 30 seconds.
    pub http_default_timeout_secs: u64,
    /// Hard cap on HTTP timeout. Default: 300 seconds.
    pub http_max_timeout_secs: u64,
    /// Block private IPs (RFC 1918). Default: true.
    pub block_private_ips: bool,
    /// Block loopback (127.0.0.0/8, ::1). Default: true.
    pub block_loopback: bool,
    /// Block link-local (169.254.0.0/16). Default: true.
    pub block_link_local: bool,
    /// Block known cloud metadata endpoints. Default: true.
    pub block_metadata_endpoints: bool,
}

/// Forward-compatible alias. v1.0.5 may rename FileOpsLimits → ToolLimits.
pub type ToolLimits = FileOpsLimits;

impl FileOpsLimits {
    /// Create with project defaults (matches `Default::default()`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: override `max_read_bytes`.
    pub fn with_max_read_bytes(mut self, n: u64) -> Self {
        self.max_read_bytes = n;
        self
    }

    /// Builder: override `max_write_bytes`.
    pub fn with_max_write_bytes(mut self, n: u64) -> Self {
        self.max_write_bytes = n;
        self
    }

    /// Builder: override `max_edit_bytes`.
    pub fn with_max_edit_bytes(mut self, n: u64) -> Self {
        self.max_edit_bytes = n;
        self
    }

    /// Builder: override `default_read_lines`.
    pub fn with_default_read_lines(mut self, n: usize) -> Self {
        self.default_read_lines = n;
        self
    }

    /// Builder: override `max_line_chars`.
    pub fn with_max_line_chars(mut self, n: usize) -> Self {
        self.max_line_chars = n;
        self
    }

    /// Builder: override `binary_sniff_bytes`.
    pub fn with_binary_sniff_bytes(mut self, n: usize) -> Self {
        self.binary_sniff_bytes = n;
        self
    }

    /// Builder: override `max_dir_entries`.
    pub fn with_max_dir_entries(mut self, n: usize) -> Self {
        self.max_dir_entries = n;
        self
    }

    // === P0 builders ===

    /// Builder: override `max_bash_output_bytes`.
    pub fn with_max_bash_output_bytes(mut self, n: u64) -> Self {
        self.max_bash_output_bytes = n;
        self
    }

    /// Builder: override `max_bash_command_chars`.
    pub fn with_max_bash_command_chars(mut self, n: usize) -> Self {
        self.max_bash_command_chars = n;
        self
    }

    /// Builder: override `max_delete_bytes`.
    pub fn with_max_delete_bytes(mut self, n: u64) -> Self {
        self.max_delete_bytes = n;
        self
    }

    /// Builder: override `enable_trash`.
    pub fn with_enable_trash(mut self, b: bool) -> Self {
        self.enable_trash = b;
        self
    }

    /// Builder: override `max_http_redirect`.
    pub fn with_max_http_redirect(mut self, n: usize) -> Self {
        self.max_http_redirect = n;
        self
    }

    /// Builder: override `block_metadata_endpoints`.
    pub fn with_block_metadata_endpoints(mut self, b: bool) -> Self {
        self.block_metadata_endpoints = b;
        self
    }

    /// Convenience: wrap in `Arc` for sharing across tool instances.
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

impl Default for FileOpsLimits {
    fn default() -> Self {
        Self {
            // v1.0.3 fields
            max_read_bytes: 10 * 1024 * 1024,  // 10 MiB
            max_write_bytes: 10 * 1024 * 1024, // 10 MiB
            max_edit_bytes: 10 * 1024 * 1024,  // 10 MiB
            default_read_lines: 2000,
            max_line_chars: 2000,
            binary_sniff_bytes: 8192,
            max_dir_entries: 1000,
            // P0: Shell
            max_bash_output_bytes: 1024 * 1024, // 1 MiB
            max_bash_command_chars: 8192,
            bash_default_timeout_ms: 30000,
            bash_max_timeout_ms: 300000,
            // P0: Delete
            max_delete_bytes: 100 * 1024 * 1024, // 100 MiB
            max_delete_file_count: 10000,
            enable_trash: true,
            // P0: Network
            max_http_response_bytes: 10 * 1024 * 1024, // 10 MiB
            max_http_request_body_bytes: 1024 * 1024,  // 1 MiB
            max_http_header_count: 50,
            max_http_redirect: 0,
            http_default_timeout_secs: 30,
            http_max_timeout_secs: 300,
            block_private_ips: true,
            block_loopback: true,
            block_link_local: true,
            block_metadata_endpoints: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let l = FileOpsLimits::default();
        // v1.0.3
        assert_eq!(l.max_read_bytes, 10 * 1024 * 1024);
        assert_eq!(l.max_write_bytes, 10 * 1024 * 1024);
        assert_eq!(l.default_read_lines, 2000);
        assert_eq!(l.max_line_chars, 2000);
        assert_eq!(l.binary_sniff_bytes, 8192);
        assert_eq!(l.max_dir_entries, 1000);
        // P0: Shell
        assert_eq!(l.max_bash_output_bytes, 1024 * 1024);
        assert_eq!(l.max_bash_command_chars, 8192);
        assert_eq!(l.bash_max_timeout_ms, 300000);
        // P0: Delete
        assert_eq!(l.max_delete_bytes, 100 * 1024 * 1024);
        assert!(l.enable_trash);
        // P0: Network
        assert_eq!(l.max_http_response_bytes, 10 * 1024 * 1024);
        assert_eq!(l.max_http_redirect, 0);
        assert!(l.block_metadata_endpoints);
    }

    #[test]
    fn test_builder_methods() {
        let l = FileOpsLimits::new()
            .with_max_read_bytes(1024)
            .with_default_read_lines(100)
            .with_max_line_chars(500);
        assert_eq!(l.max_read_bytes, 1024);
        assert_eq!(l.default_read_lines, 100);
        assert_eq!(l.max_line_chars, 500);
        // Unspecified fields keep defaults
        assert_eq!(l.max_dir_entries, 1000);
    }

    #[test]
    fn test_p0_builders() {
        let l = FileOpsLimits::new()
            .with_max_bash_command_chars(100)
            .with_max_delete_bytes(50)
            .with_enable_trash(false)
            .with_max_http_redirect(3);
        assert_eq!(l.max_bash_command_chars, 100);
        assert_eq!(l.max_delete_bytes, 50);
        assert!(!l.enable_trash);
        assert_eq!(l.max_http_redirect, 3);
    }

    #[test]
    fn test_into_arc() {
        let arc = FileOpsLimits::default().into_arc();
        assert_eq!(arc.max_read_bytes, 10 * 1024 * 1024);
    }
}
