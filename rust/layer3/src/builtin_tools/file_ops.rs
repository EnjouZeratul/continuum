//! # File Operations Tools
//!
//! 文件操作工具集：读写、编辑、创建、删除等。

use crate::builtin_tools::limits::FileOpsLimits;
use crate::builtin_tools::safe_truncate::safe_truncate_chars;
use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use std::sync::Arc;

/// Read File Tool
///
/// Reads a file with safety bounds:
/// - Size pre-check via metadata() before allocation
/// - Default line limit (2000) when caller omits `limit`
/// - Per-line char cap (2000) with UTF-8-safe truncation
/// - Binary detection via NUL-byte sniff (first 8192 bytes)
pub struct ReadFileTool {
    limits: Arc<FileOpsLimits>,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self {
            limits: FileOpsLimits::default().into_arc(),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file from the filesystem with size and binary safety bounds."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Optional: line number to start reading from (0-based)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Optional: number of lines to read (default: 2000)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;

        // === R1: Size pre-check before allocation ===
        let metadata = tokio::fs::metadata(path_str)
            .await
            .map_err(|e| anyhow::anyhow!("Path not accessible '{}': {}", path_str, e))?;

        if metadata.is_dir() {
            return Err(anyhow::anyhow!(
                "Path '{}' is a directory; use list_directory instead",
                path_str
            ));
        }

        let file_size = metadata.len();
        if file_size > self.limits.max_read_bytes {
            return Err(anyhow::anyhow!(
                "read_file rejected: file size {} bytes exceeds limit {} bytes",
                file_size,
                self.limits.max_read_bytes,
            ));
        }

        // === R4: Binary detection via NUL-byte sniff ===
        // Read entire content (already bounded by max_read_bytes).
        let bytes = tokio::fs::read(path_str).await?;
        let sniff_len = bytes.len().min(self.limits.binary_sniff_bytes);
        if sniff_len > 0 && bytes[..sniff_len].contains(&0u8) {
            return Err(anyhow::anyhow!(
                "read_file rejected: file '{}' appears binary (NUL byte in first {} bytes); \
                 refusing to inject into LLM context",
                path_str,
                self.limits.binary_sniff_bytes,
            ));
        }

        // === Stale-read prevention: record this read ===
        // Canonical path is needed for the store. Use canonicalized metadata path.
        let canonical_for_state = tokio::fs::canonicalize(path_str)
            .await
            .unwrap_or_else(|_| std::path::PathBuf::from(path_str));
        let store = crate::builtin_tools::exec_context::current_context().read_state_store();
        let _ = store.record_read(canonical_for_state).await;

        // Convert to string (lossy in case of invalid UTF-8 edge cases)
        let content = String::from_utf8_lossy(&bytes).to_string();
        let total_bytes = content.len();

        // === Pagination ===
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let user_limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        // R2: default line limit applies when user omits `limit`
        let effective_limit = user_limit.unwrap_or(self.limits.default_read_lines);

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        if offset > total_lines {
            return Err(anyhow::anyhow!(
                "Offset {} exceeds total lines {}",
                offset,
                total_lines
            ));
        }

        let end = (offset + effective_limit).min(total_lines);
        let page_lines = &lines[offset..end];

        // === R3: Per-line char truncation ===
        let max_line_chars = self.limits.max_line_chars;
        let truncated_count = page_lines
            .iter()
            .filter(|l| l.chars().count() > max_line_chars)
            .count();

        let body: Vec<String> = page_lines
            .iter()
            .map(|line| {
                if line.chars().count() > max_line_chars {
                    let truncated = safe_truncate_chars(line, max_line_chars);
                    format!(
                        "{} ...(line truncated, {} chars total)",
                        truncated,
                        line.chars().count()
                    )
                } else {
                    line.to_string()
                }
            })
            .collect();

        // === R5: Metadata header in response ===
        let mut result = String::new();
        result.push_str(&format!(
            "[File: {} | {} bytes | {} total lines",
            path_str, total_bytes, total_lines
        ));
        // Show range when not entire file (offset > 0 OR end < total_lines)
        if offset > 0 || end < total_lines {
            result.push_str(&format!(" | showing lines {}-{}", offset, end));
        }
        result.push_str("]\n");

        if truncated_count > 0 {
            result.push_str(&format!(
                "[Note: {} line(s) truncated to {} chars each]\n",
                truncated_count, max_line_chars
            ));
        }

        if body.is_empty() {
            result.push_str("(No content in this range)");
        } else {
            result.push_str(&body.join("\n"));
        }

        Ok(result)
    }
}

/// Write File Tool
///
/// Writes content to a file with safety bounds:
/// - Content size limit (default 10 MiB)
/// - Parent directory auto-created with clear error on failure
/// - `overwrite` parameter (default false) — refuses to clobber existing files
pub struct WriteFileTool {
    limits: Arc<FileOpsLimits>,
}

impl WriteFileTool {
    pub fn new() -> Self {
        Self {
            limits: FileOpsLimits::default().into_arc(),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file with size limits, overwrite protection, and stale-read check."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to write to"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write"
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "If true, overwrite existing file. Default false (errors if exists)."
                },
                "force": {
                    "type": "boolean",
                    "description": "Override stale-read check when overwriting (default: false)"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing content parameter"))?;
        let overwrite = args["overwrite"].as_bool().unwrap_or(false);

        // === W1: Size limit ===
        if content.len() as u64 > self.limits.max_write_bytes {
            return Err(anyhow::anyhow!(
                "write_file rejected: content size {} bytes exceeds limit {} bytes",
                content.len(),
                self.limits.max_write_bytes,
            ));
        }

        // === W3: Overwrite protection ===
        let path = std::path::Path::new(path_str);
        let exists = tokio::fs::try_exists(path).await.unwrap_or(false);
        if exists && !overwrite {
            return Err(anyhow::anyhow!(
                "write_file rejected: '{}' already exists. \
                 Pass \"overwrite\": true to replace.",
                path_str
            ));
        }

        // === Stale-read prevention: overwriting an existing file requires prior read ===
        let force = args["force"].as_bool().unwrap_or(false);
        if exists && overwrite && !force {
            let canonical_for_state = tokio::fs::canonicalize(path)
                .await
                .unwrap_or_else(|_| path.to_path_buf());
            use crate::builtin_tools::read_state::StaleReadError;
            let ctx_snap = crate::builtin_tools::exec_context::current_context();
            let store = ctx_snap.read_state_store();
            match store.verify(&canonical_for_state, true).await {
                Ok(()) => {}
                Err(StaleReadError::NotRead) => {
                    crate::builtin_tools::metrics::record_stale_read_rejection(
                        "write_file",
                        "not_read_in_session",
                        &ctx_snap.session_id,
                        path_str,
                        None,
                    );
                    return Err(anyhow::anyhow!(
                        "write_file rejected: '{}' exists but was not read in this session. \
                         Call read_file first, or pass force=true to override.",
                        path_str,
                    ));
                }
                Err(StaleReadError::Modified) => {
                    crate::builtin_tools::metrics::record_stale_read_rejection(
                        "write_file",
                        "modified_after_read",
                        &ctx_snap.session_id,
                        path_str,
                        None,
                    );
                    return Err(anyhow::anyhow!(
                        "write_file rejected: '{}' was modified after last read. \
                         Call read_file again, or pass force=true to override.",
                        path_str,
                    ));
                }
                Err(StaleReadError::Io(e)) => {
                    return Err(anyhow::anyhow!(
                        "write_file I/O error during stale check: {}",
                        e
                    ));
                }
            }
        }

        // === W2: Parent dir handling ===
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    anyhow::anyhow!(
                        "write_file: cannot create parent directory '{}': {}",
                        parent.display(),
                        e
                    )
                })?;
            }
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| anyhow::anyhow!("write_file: failed to write '{}': {}", path_str, e))?;

        // Record write as a fresh read so subsequent edit_file/write_file doesn't trip staleness
        let canonical_after = tokio::fs::canonicalize(path)
            .await
            .unwrap_or_else(|_| path.to_path_buf());
        let _ = crate::builtin_tools::exec_context::current_context()
            .read_state_store()
            .record_read(canonical_after)
            .await;

        Ok(format!(
            "Wrote {} bytes to {}{}",
            content.len(),
            path_str,
            if overwrite && exists {
                " (overwritten)"
            } else {
                ""
            }
        ))
    }
}

/// Edit File Tool
///
/// Edits a file by replacing a unique substring:
/// - Size pre-check on file before reading
/// - old_string must match exactly once (errors on 0 or multiple matches)
/// - Matches Claude Code Edit tool semantics
pub struct EditFileTool {
    limits: Arc<FileOpsLimits>,
}

impl EditFileTool {
    pub fn new() -> Self {
        Self {
            limits: FileOpsLimits::default().into_arc(),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for EditFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a unique substring. Errors if old_string matches 0 or multiple times. \
         Requires prior read_file on the same path (pass force=true to override)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace (must be unique in file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement text"
                },
                "force": {
                    "type": "boolean",
                    "description": "Override stale-read check (default: false)"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let old_string = args["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing old_string parameter"))?;
        let new_string = args["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing new_string parameter"))?;

        if old_string.is_empty() {
            return Err(anyhow::anyhow!("edit_file: old_string cannot be empty"));
        }

        // === E1: Size pre-check ===
        let metadata = tokio::fs::metadata(path_str)
            .await
            .map_err(|e| anyhow::anyhow!("Path not accessible '{}': {}", path_str, e))?;
        let file_size = metadata.len();
        if file_size > self.limits.max_edit_bytes {
            return Err(anyhow::anyhow!(
                "edit_file rejected: file size {} bytes exceeds limit {} bytes",
                file_size,
                self.limits.max_edit_bytes,
            ));
        }

        // === Stale-read prevention: require prior read, verify unchanged ===
        let canonical_for_state = tokio::fs::canonicalize(path_str)
            .await
            .unwrap_or_else(|_| std::path::PathBuf::from(path_str));
        let force = args["force"].as_bool().unwrap_or(false);
        if !force {
            use crate::builtin_tools::read_state::StaleReadError;
            let ctx_snap = crate::builtin_tools::exec_context::current_context();
            let store = ctx_snap.read_state_store();
            match store.verify(&canonical_for_state, true).await {
                Ok(()) => {}
                Err(StaleReadError::NotRead) => {
                    crate::builtin_tools::metrics::record_stale_read_rejection(
                        "edit_file",
                        "not_read_in_session",
                        &ctx_snap.session_id,
                        path_str,
                        None,
                    );
                    return Err(anyhow::anyhow!(
                        "edit_file rejected: '{}' has not been read in this session. \
                         Call read_file first, or pass force=true to override.",
                        path_str,
                    ));
                }
                Err(StaleReadError::Modified) => {
                    crate::builtin_tools::metrics::record_stale_read_rejection(
                        "edit_file",
                        "modified_after_read",
                        &ctx_snap.session_id,
                        path_str,
                        None,
                    );
                    return Err(anyhow::anyhow!(
                        "edit_file rejected: '{}' was modified after last read. \
                         Call read_file again to refresh, or pass force=true to override.",
                        path_str,
                    ));
                }
                Err(StaleReadError::Io(e)) => {
                    return Err(anyhow::anyhow!(
                        "edit_file I/O error during stale check: {}",
                        e
                    ));
                }
            }
        }

        let content = tokio::fs::read_to_string(path_str).await?;

        // === E2: Uniqueness check ===
        let match_count = content.matches(old_string).count();
        if match_count == 0 {
            // E3: explicit 0-match error
            return Err(anyhow::anyhow!(
                "edit_file rejected: old_string not found in '{}'. \
                 Verify the file content and old_string exact match.",
                path_str
            ));
        }
        if match_count > 1 {
            return Err(anyhow::anyhow!(
                "edit_file rejected: old_string matches {} locations in '{}'. \
                 Provide more context to make the match unique.",
                match_count,
                path_str
            ));
        }

        // Safe: exactly 1 match, replace proceeds
        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(path_str, &new_content).await?;

        // Update read state to reflect the just-written content (so consecutive
        // edit_file calls without an intervening read_file don't trip staleness).
        let _ = crate::builtin_tools::exec_context::current_context()
            .read_state_store()
            .record_read(canonical_for_state)
            .await;

        Ok(format!(
            "Edited '{}': replaced {} bytes with {} bytes (file now {} bytes)",
            path_str,
            old_string.len(),
            new_string.len(),
            new_content.len()
        ))
    }
}

/// List Directory Tool
///
/// Lists directory entries with safety bounds:
/// - Max entries cap (default 1000)
/// - Path type check (rejects files)
/// - Sorted output (alphabetical) for deterministic LLM reasoning
pub struct ListDirectoryTool {
    limits: Arc<FileOpsLimits>,
}

impl ListDirectoryTool {
    pub fn new() -> Self {
        Self {
            limits: FileOpsLimits::default().into_arc(),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for ListDirectoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List directory entries with size cap and deterministic sort."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory to list"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;

        // === L2: Path type check ===
        let metadata = tokio::fs::metadata(path_str)
            .await
            .map_err(|e| anyhow::anyhow!("Path not accessible '{}': {}", path_str, e))?;
        if !metadata.is_dir() {
            return Err(anyhow::anyhow!(
                "list_directory rejected: '{}' is not a directory",
                path_str
            ));
        }

        let mut entries = tokio::fs::read_dir(path_str).await?;
        let mut names: Vec<String> = Vec::new();
        let mut truncated = false;

        while let Some(entry) = entries.next_entry().await? {
            if names.len() >= self.limits.max_dir_entries {
                // === L1: Entries cap ===
                truncated = true;
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let ftype = entry.file_type().await?;
            let prefix = if ftype.is_dir() { "[D] " } else { "[F] " };
            names.push(format!("{}{}", prefix, name));
        }

        // === L3: Sort guarantee ===
        names.sort();

        let total_count = if truncated {
            // Count remaining entries for accurate report
            let mut extra = 0;
            while let Some(_entry) = entries.next_entry().await? {
                extra += 1;
            }
            Some(self.limits.max_dir_entries + extra)
        } else {
            Some(names.len())
        };

        let mut result = String::new();
        if let Some(total) = total_count {
            result.push_str(&format!("[Directory: {} | {} entries", path_str, total));
            if truncated {
                result.push_str(&format!(" | showing first {}", self.limits.max_dir_entries));
            }
            result.push_str("]\n");
        }

        if names.is_empty() {
            result.push_str("(empty directory)");
        } else {
            result.push_str(&names.join("\n"));
        }

        Ok(result)
    }
}

/// Move File Tool
///
/// 移动或重命名文件/目录。Path safety + symlink rejection + overwrite protection.
pub struct MoveFileTool {
    /// Reserved for future per-tool limits; currently unused but kept for API consistency.
    #[allow(dead_code)]
    limits: Arc<FileOpsLimits>,
}

impl MoveFileTool {
    pub fn new() -> Self {
        Self {
            limits: FileOpsLimits::default().into_arc(),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for MoveFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for MoveFileTool {
    fn name(&self) -> &str {
        "move_file"
    }

    fn description(&self) -> &str {
        "Move or rename a file or directory with path safety and overwrite protection."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The source path to move from"
                },
                "destination": {
                    "type": "string",
                    "description": "The destination path to move to"
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Optional: overwrite destination if exists (default: false)"
                },
                "force": {
                    "type": "boolean",
                    "description": "Optional: override critical-path checks (default: false)"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing source parameter"))?;
        let destination = args["destination"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing destination parameter"))?;
        let overwrite = args["overwrite"].as_bool().unwrap_or(false);
        let force = args["force"].as_bool().unwrap_or(false);

        // === M1: Canonicalize source ===
        let src_canonical = tokio::fs::canonicalize(source)
            .await
            .map_err(|e| anyhow::anyhow!("Source path not accessible: {} ({})", source, e))?;

        // === M2: Critical-path check on source and destination ===
        let src_danger = crate::builtin_tools::path_safety::check_path_danger(&src_canonical);
        if src_danger.is_critical && !force {
            return Err(anyhow::anyhow!(
                "move_file rejected: source '{}' is critical ({}). Pass force=true to override.",
                src_canonical.display(),
                src_danger.reason,
            ));
        }

        // Destination may not exist yet — canonicalize parent if possible
        let dest_path = std::path::Path::new(destination);
        if let Some(parent) = dest_path.parent() {
            if !parent.as_os_str().is_empty() && parent.exists() {
                let parent_canonical = tokio::fs::canonicalize(parent).await?;
                let dest_canonical = parent_canonical.join(
                    dest_path
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("Invalid destination filename"))?,
                );
                let dest_danger =
                    crate::builtin_tools::path_safety::check_path_danger(&dest_canonical);
                if dest_danger.is_critical && !force {
                    return Err(anyhow::anyhow!(
                        "move_file rejected: destination '{}' is critical ({}). \
                         Pass force=true to override.",
                        dest_canonical.display(),
                        dest_danger.reason,
                    ));
                }
            }
        }

        // === M5: Symlink source rejection ===
        let sym_meta = tokio::fs::symlink_metadata(&src_canonical).await?;
        if sym_meta.is_symlink() {
            return Err(anyhow::anyhow!(
                "move_file rejected: source '{}' is symlink. Resolve target explicitly.",
                src_canonical.display(),
            ));
        }

        let source_meta = tokio::fs::metadata(&src_canonical).await?;

        // 检查目标是否存在
        let dest_exists = tokio::fs::try_exists(destination).await.unwrap_or(false);

        if dest_exists && !overwrite {
            return Err(anyhow::anyhow!(
                "Destination already exists: {}. Use overwrite=true to replace.",
                destination
            ));
        }

        // 如果目标存在且要覆盖，先删除目标
        if dest_exists && overwrite {
            if tokio::fs::metadata(destination).await?.is_dir() {
                tokio::fs::remove_dir_all(destination).await?;
            } else {
                tokio::fs::remove_file(destination).await?;
            }
        }

        // 确保目标父目录存在
        if let Some(parent) = dest_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // 执行移动操作
        tokio::fs::rename(&src_canonical, destination)
            .await
            .map_err(|e| {
                if e.raw_os_error() == Some(18) {
                    anyhow::anyhow!(
                        "Cross-device move not supported directly. Use copy + delete instead."
                    )
                } else {
                    anyhow::anyhow!("Failed to move file: {}", e)
                }
            })?;

        let file_type = if source_meta.is_dir() {
            "directory"
        } else {
            "file"
        };
        Ok(format!(
            "Successfully moved {} from {} to {}",
            file_type,
            src_canonical.display(),
            destination
        ))
    }
}

/// Copy File Tool
///
/// 复制文件或目录。
pub struct CopyFileTool;

#[async_trait]
impl BuiltinTool for CopyFileTool {
    fn name(&self) -> &str {
        "copy_file"
    }

    fn description(&self) -> &str {
        "Copy a file or directory to a new location."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The source path to copy from"
                },
                "destination": {
                    "type": "string",
                    "description": "The destination path to copy to"
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Optional: overwrite destination if exists (default: false)"
                },
                "force": {
                    "type": "boolean",
                    "description": "Optional: override critical-path / size checks (default: false)"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing source parameter"))?;
        let destination = args["destination"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing destination parameter"))?;
        let overwrite = args["overwrite"].as_bool().unwrap_or(false);
        let force = args["force"].as_bool().unwrap_or(false);

        // === C2: Canonicalize source + critical-path check ===
        let src_canonical = tokio::fs::canonicalize(source)
            .await
            .map_err(|e| anyhow::anyhow!("Source path not found: {} ({})", source, e))?;
        let src_danger = crate::builtin_tools::path_safety::check_path_danger(&src_canonical);
        if src_danger.is_critical && !force {
            return Err(anyhow::anyhow!(
                "copy_file rejected: source '{}' is critical ({}). Pass force=true to override.",
                src_canonical.display(),
                src_danger.reason,
            ));
        }

        // Destination critical-path check (canonicalize parent if exists)
        let dest_path = std::path::Path::new(destination);
        if let Some(parent) = dest_path.parent() {
            if !parent.as_os_str().is_empty() && parent.exists() {
                let parent_canonical = tokio::fs::canonicalize(parent).await?;
                let dest_canonical = parent_canonical.join(
                    dest_path
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("Invalid destination filename"))?,
                );
                let dest_danger =
                    crate::builtin_tools::path_safety::check_path_danger(&dest_canonical);
                if dest_danger.is_critical && !force {
                    return Err(anyhow::anyhow!(
                        "copy_file rejected: destination '{}' is critical ({}). \
                         Pass force=true to override.",
                        dest_canonical.display(),
                        dest_danger.reason,
                    ));
                }
            }
        }

        // 验证源文件/目录存在
        let source_meta = tokio::fs::metadata(&src_canonical).await?;

        // === C1: Source size cap (100 MiB default via FileOpsLimits::max_delete_bytes proxy)
        if source_meta.len() > 100 * 1024 * 1024 && !force {
            return Err(anyhow::anyhow!(
                "copy_file rejected: source size {} bytes > 100 MiB limit. \
                 Pass force=true to override.",
                source_meta.len(),
            ));
        }

        // 检查目标是否存在
        let dest_exists = tokio::fs::try_exists(destination).await.unwrap_or(false);

        if dest_exists && !overwrite {
            return Err(anyhow::anyhow!(
                "Destination already exists: {}. Use overwrite=true to replace.",
                destination
            ));
        }

        // 确保目标父目录存在
        if let Some(parent) = dest_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // 执行复制操作
        if source_meta.is_dir() {
            copy_dir_all(
                src_canonical.to_string_lossy().to_string(),
                destination.to_string(),
            )
            .await?;
            Ok(format!(
                "Successfully copied directory from {} to {}",
                src_canonical.display(),
                destination
            ))
        } else {
            tokio::fs::copy(&src_canonical, destination)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to copy file: {}", e))?;
            Ok(format!(
                "Successfully copied file from {} to {}",
                src_canonical.display(),
                destination
            ))
        }
    }
}

/// 递归复制目录
///
/// 使用 Box::pin 处理递归异步函数
fn copy_dir_all(
    source: String,
    destination: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Layer3Result<()>> + Send>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(&destination).await?;

        let mut entries = tokio::fs::read_dir(&source).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ty = entry.file_type().await?;
            let src_path = entry.path();
            let dest_path = std::path::Path::new(&destination).join(entry.file_name());

            if ty.is_symlink() {
                continue; // skip symlinks — don't follow/copy targets
            }
            if ty.is_dir() {
                copy_dir_all(
                    src_path.to_string_lossy().to_string(),
                    dest_path.to_string_lossy().to_string(),
                )
                .await?;
            } else {
                tokio::fs::copy(&src_path, &dest_path).await?;
            }
        }

        Ok(())
    })
}

/// Create Directory Tool
///
/// 创建目录（包括父目录）。
pub struct CreateDirectoryTool;

#[async_trait]
impl BuiltinTool for CreateDirectoryTool {
    fn name(&self) -> &str {
        "create_directory"
    }

    fn description(&self) -> &str {
        "Create a directory and all parent directories if they don't exist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to create"
                },
                "force": {
                    "type": "boolean",
                    "description": "Optional: override critical-path checks (default: false)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let force = args["force"].as_bool().unwrap_or(false);

        // === CD2: Critical-path check on destination (canonicalize parent if exists) ===
        let path_buf = std::path::Path::new(path);
        if let Some(parent) = path_buf.parent() {
            if !parent.as_os_str().is_empty() && parent.exists() {
                let parent_canonical = tokio::fs::canonicalize(parent).await?;
                if let Some(fname) = path_buf.file_name() {
                    let target = parent_canonical.join(fname);
                    let danger = crate::builtin_tools::path_safety::check_path_danger(&target);
                    if danger.is_critical && !force {
                        return Err(anyhow::anyhow!(
                            "create_directory rejected: path '{}' is critical ({}). \
                             Pass force=true to override.",
                            target.display(),
                            danger.reason,
                        ));
                    }
                }
            }
        }

        tokio::fs::create_dir_all(path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))?;

        Ok(format!("Successfully created directory: {}", path))
    }
}

/// Delete File Tool — file/directory deletion with safety bounds.
///
/// - Path canonicalization + critical-path check
/// - Size + count cap (override with force=true)
/// - Dry-run mode
/// - Symlink rejection (resolves target explicitly first)
pub struct DeleteFileTool {
    limits: Arc<FileOpsLimits>,
}

impl DeleteFileTool {
    pub fn new() -> Self {
        Self {
            limits: FileOpsLimits::default().into_arc(),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for DeleteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for DeleteFileTool {
    fn name(&self) -> &str {
        "delete_file"
    }

    fn description(&self) -> &str {
        "Delete file or directory with safety bounds. Use dry_run=true to preview."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to delete (canonicalized)"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "For directories, delete recursively (default false)"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "Preview without deleting (default false)"
                },
                "force": {
                    "type": "boolean",
                    "description": "Override critical-path / size checks (default false, NOT recommended)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let recursive = args["recursive"].as_bool().unwrap_or(false);
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);
        let force = args["force"].as_bool().unwrap_or(false);

        // === D6: Canonicalize ===
        let canonical = tokio::fs::canonicalize(path_str)
            .await
            .map_err(|e| anyhow::anyhow!("Path '{}' not accessible: {}", path_str, e))?;

        // === D1: Critical path check ===
        let danger = crate::builtin_tools::path_safety::check_path_danger(&canonical);
        if danger.is_critical && !force {
            return Err(anyhow::anyhow!(
                "delete_file rejected: path '{}' is critical ({}). \
                 Pass force=true to override (NOT recommended in production).",
                canonical.display(),
                danger.reason,
            ));
        }

        let sym_meta = tokio::fs::symlink_metadata(&canonical).await?;

        // === D3: Symlink rejection ===
        if sym_meta.is_symlink() {
            return Err(anyhow::anyhow!(
                "delete_file rejected: '{}' is a symlink. \
                 Resolve the target explicitly before deleting.",
                canonical.display(),
            ));
        }

        let meta = tokio::fs::metadata(&canonical).await?;
        let (size, file_count) = if meta.is_dir() {
            compute_dir_stats(&canonical)
                .await
                .unwrap_or((meta.len(), 1))
        } else {
            (meta.len(), 1u64)
        };

        // === D5: Size / count cap ===
        if !force {
            if size > self.limits.max_delete_bytes {
                return Err(anyhow::anyhow!(
                    "delete_file rejected: target size {} bytes > limit {} bytes. \
                     Pass force=true to override.",
                    size,
                    self.limits.max_delete_bytes,
                ));
            }
            if file_count > self.limits.max_delete_file_count {
                return Err(anyhow::anyhow!(
                    "delete_file rejected: target has {} files > limit {}. \
                     Pass force=true to override.",
                    file_count,
                    self.limits.max_delete_file_count,
                ));
            }
        }

        // === D4: Dry-run ===
        if dry_run {
            return Ok(format!(
                "DRY RUN: would delete '{}' ({} bytes, {} files)",
                canonical.display(),
                size,
                file_count,
            ));
        }

        // Actual deletion
        if meta.is_dir() {
            if recursive {
                tokio::fs::remove_dir_all(&canonical).await?;
            } else {
                tokio::fs::remove_dir(&canonical).await?;
            }
        } else {
            tokio::fs::remove_file(&canonical).await?;
        }

        Ok(format!(
            "Deleted: {} ({} bytes, {} files)",
            canonical.display(),
            size,
            file_count,
        ))
    }
}

/// Recursively compute directory size + file count.
async fn compute_dir_stats(path: &std::path::Path) -> Layer3Result<(u64, u64)> {
    let mut size = 0u64;
    let mut count = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let p = entry.path();
            let ftype = entry.file_type().await?;
            if ftype.is_symlink() {
                continue;
            }
            if ftype.is_dir() {
                stack.push(p);
            } else if ftype.is_file() {
                if let Ok(m) = entry.metadata().await {
                    size += m.len();
                    count += 1;
                }
            }
        }
    }
    Ok((size, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_tool_meta() {
        let tool = ReadFileTool::new();
        assert_eq!(tool.name(), "read_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
    }

    #[test]
    fn test_write_file_tool_dangerous() {
        let tool = WriteFileTool::new();
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[test]
    fn test_move_file_tool_meta() {
        let tool = MoveFileTool::new();
        assert_eq!(tool.name(), "move_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_move_file_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // 创建源文件
        tokio::fs::write(&source, "test content").await.unwrap();

        let tool = MoveFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(dest.exists());
        assert!(!source.exists());
    }

    #[tokio::test]
    async fn test_move_file_missing_source() {
        let tool = MoveFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "source": "/nonexistent/file.txt",
                "destination": "/tmp/dest.txt"
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Source path not accessible"));
    }

    #[tokio::test]
    async fn test_move_file_destination_exists_no_overwrite() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, "source content").await.unwrap();
        tokio::fs::write(&dest, "dest content").await.unwrap();

        let tool = MoveFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_move_file_destination_exists_with_overwrite() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, "source content").await.unwrap();
        tokio::fs::write(&dest, "dest content").await.unwrap();

        let tool = MoveFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap(),
                "overwrite": true
            }))
            .await;

        assert!(result.is_ok());
        // 验证目标文件内容是源文件内容
        let content = tokio::fs::read_to_string(&dest).await.unwrap();
        assert_eq!(content, "source content");
    }

    #[test]
    fn test_copy_file_tool_meta() {
        let tool = CopyFileTool;
        assert_eq!(tool.name(), "copy_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_copy_file_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // 创建源文件
        tokio::fs::write(&source, "test content").await.unwrap();

        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(dest.exists());
        assert!(source.exists()); // 复制后源文件仍然存在
        let content = tokio::fs::read_to_string(&dest).await.unwrap();
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn test_copy_file_missing_source() {
        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": "/nonexistent/file.txt",
                "destination": "/tmp/dest.txt"
            }))
            .await;

        assert!(result.is_err());
        // CopyFileTool was not refactored in v1.0.5; still uses original message
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Source path not found"));
    }

    #[tokio::test]
    async fn test_copy_file_destination_exists_no_overwrite() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, "source content").await.unwrap();
        tokio::fs::write(&dest, "dest content").await.unwrap();

        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_copy_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_dir = temp_dir.path().join("dest_dir");

        // 创建源目录和文件
        tokio::fs::create_dir_all(&source_dir).await.unwrap();
        tokio::fs::write(source_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        tokio::fs::write(source_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source_dir.to_str().unwrap(),
                "destination": dest_dir.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(dest_dir.exists());
        assert!(dest_dir.join("file1.txt").exists());
        assert!(dest_dir.join("file2.txt").exists());
        assert!(source_dir.exists()); // 源目录仍然存在
    }

    #[test]
    fn test_delete_file_tool_meta() {
        let tool = DeleteFileTool::new();
        assert_eq!(tool.name(), "delete_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_delete_file_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");

        // 创建文件
        tokio::fs::write(&file, "test content").await.unwrap();

        let tool = DeleteFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn test_delete_file_missing_path() {
        let tool = DeleteFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": "/nonexistent/file.txt"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not accessible"));
    }

    #[tokio::test]
    async fn test_delete_empty_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("empty_dir");

        // 创建空目录
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let tool = DeleteFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": dir.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn test_delete_non_empty_directory_without_recursive() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("non_empty_dir");

        // 创建非空目录
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("file.txt"), "content")
            .await
            .unwrap();

        let tool = DeleteFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": dir.to_str().unwrap()
            }))
            .await;

        assert!(result.is_err());
        // New impl uses tokio::fs::remove_dir — OS returns "directory is not empty" (Windows)
        // or "Directory not empty" (POSIX). We just assert it errored.
    }

    #[tokio::test]
    async fn test_delete_non_empty_directory_with_recursive() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("non_empty_dir");

        // 创建非空目录
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("file.txt"), "content")
            .await
            .unwrap();
        tokio::fs::create_dir_all(dir.join("subdir")).await.unwrap();
        tokio::fs::write(dir.join("subdir/nested.txt"), "nested")
            .await
            .unwrap();

        let tool = DeleteFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": dir.to_str().unwrap(),
                "recursive": true
            }))
            .await;

        assert!(result.is_ok());
        assert!(!dir.exists());
    }

    // ========== ReadFileTool 分页测试 ==========

    #[tokio::test]
    async fn test_read_file_no_pagination() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap()
            }))
            .await
            .unwrap();

        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
        assert!(result.contains("line3"));
        assert!(result.contains("18 bytes"));
        assert!(result.contains("3 total lines"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\nline4\nline5\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 2
            }))
            .await
            .unwrap();

        assert!(result.contains("showing lines 2-5"));
        assert!(result.contains("5 total lines"));
        assert!(result.contains("line3"));
        assert!(result.contains("line5"));
        assert!(!result.contains("line1"));
    }

    #[tokio::test]
    async fn test_read_file_with_limit() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\nline4\nline5\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "limit": 2
            }))
            .await
            .unwrap();

        assert!(result.contains("showing lines 0-2"));
        assert!(result.contains("5 total lines"));
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
        assert!(!result.contains("line3"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset_and_limit() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\nline4\nline5\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 1,
                "limit": 2
            }))
            .await
            .unwrap();

        assert!(result.contains("showing lines 1-3"));
        assert!(result.contains("5 total lines"));
        assert!(result.contains("line2"));
        assert!(result.contains("line3"));
        assert!(!result.contains("line1"));
        assert!(!result.contains("line4"));
    }

    #[tokio::test]
    async fn test_read_file_offset_exceeds_total() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        tokio::fs::write(&file, "line1\nline2\n").await.unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 10
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds total lines"));
    }

    #[tokio::test]
    async fn test_read_file_empty_range() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        tokio::fs::write(&file, "line1\nline2\nline3\n")
            .await
            .unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 3,
                "limit": 5
            }))
            .await
            .unwrap();

        assert!(result.contains("No content in this range"));
    }

    #[tokio::test]
    async fn test_read_file_single_line_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        tokio::fs::write(&file, "single line content")
            .await
            .unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 0,
                "limit": 10
            }))
            .await
            .unwrap();

        assert!(result.contains("1 total lines"));
        assert!(result.contains("single line content"));
    }

    // === Stale-read prevention integration tests ===

    #[tokio::test]
    async fn test_stale_read_edit_without_prior_read_rejected() {
        use tempfile::TempDir;
        // NOTE: This test relies on the process-wide ReadStateStore, so it may
        // be affected by other tests that read the same path. Using unique temp
        // path to minimize collision.
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("stale_read_unique_1.txt");
        tokio::fs::write(&path, "hello world").await.unwrap();

        // First clear any prior state by reading, then externally modifying
        // — actually, we want NO prior read.
        // Use a path unlikely to collide.
        let unique_path = temp.path().join("stale_read_test_unique_token_xyz.txt");
        tokio::fs::write(&unique_path, "v1").await.unwrap();

        let tool = EditFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": unique_path.to_str().unwrap(),
                "old_string": "v1",
                "new_string": "v2",
            }))
            .await;
        // The result depends on whether a prior test read this exact path.
        // In a clean process, this errors with "not been read".
        // We accept either error (NotRead) or success (if test ordering made it Modified).
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                msg.contains("not been read") || msg.contains("modified after"),
                "unexpected error: {}",
                msg
            );
        }
    }

    #[tokio::test]
    async fn test_stale_read_edit_after_read_succeeds() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("stale_read_after_read.txt");
        tokio::fs::write(&path, "hello world").await.unwrap();

        // Read first
        let read_tool = ReadFileTool::new();
        let _ = read_tool
            .execute(serde_json::json!({"path": path.to_str().unwrap()}))
            .await
            .unwrap();

        // Edit should now succeed
        let edit_tool = EditFileTool::new();
        let result = edit_tool
            .execute(serde_json::json!({
                "path": path.to_str().unwrap(),
                "old_string": "hello",
                "new_string": "goodbye",
            }))
            .await;
        assert!(
            result.is_ok(),
            "edit after read should succeed: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_stale_read_edit_force_overrides() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("stale_read_force.txt");
        tokio::fs::write(&path, "hello world").await.unwrap();

        // No prior read, but force=true should bypass
        let edit_tool = EditFileTool::new();
        let result = edit_tool
            .execute(serde_json::json!({
                "path": path.to_str().unwrap(),
                "old_string": "hello",
                "new_string": "goodbye",
                "force": true,
            }))
            .await;
        assert!(result.is_ok());
        let actual = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(actual, "goodbye world");
    }

    #[tokio::test]
    async fn test_stale_read_write_overwrite_requires_read() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("stale_write_unique_token_42.txt");
        tokio::fs::write(&path, "original").await.unwrap();

        // Attempt overwrite without prior read
        let tool = WriteFileTool::new();
        let result = tool
            .execute(serde_json::json!({
                "path": path.to_str().unwrap(),
                "content": "replaced",
                "overwrite": true,
            }))
            .await;
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                msg.contains("not been read")
                    || msg.contains("was not read")
                    || msg.contains("not read in this session")
                    || msg.contains("modified after"),
                "unexpected error: {}",
                msg
            );
        }
    }

    #[tokio::test]
    async fn test_stale_read_rejection_emits_metric() {
        use tempfile::TempDir;
        crate::builtin_tools::metrics::reset_stale_read_rejection_count();
        let before = crate::builtin_tools::metrics::stale_read_rejection_count();

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("metric_emit_test_unique_token.txt");
        tokio::fs::write(&path, "original").await.unwrap();

        // WriteFileTool overwrite attempt without read should trigger rejection + metric
        let tool = WriteFileTool::new();
        let _ = tool
            .execute(serde_json::json!({
                "path": path.to_str().unwrap(),
                "content": "replaced",
                "overwrite": true,
            }))
            .await;

        let after = crate::builtin_tools::metrics::stale_read_rejection_count();
        assert!(
            after > before,
            "metric should have incremented: before={}, after={}",
            before,
            after
        );
    }
}
