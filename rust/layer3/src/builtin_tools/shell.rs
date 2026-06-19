//! # Shell Tools
//!
//! Shell execution tool with safety bounds.

use crate::builtin_tools::limits::FileOpsLimits;
use crate::builtin_tools::safe_truncate::safe_truncate_bytes;
use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Patterns that are always rejected (denylist).
const FORBIDDEN_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf $HOME",
    "rm -rf $HOME/",
    "rm -rf *",
    ":(){", // fork bomb signature
    "mkfs",
    "dd if=/dev/zero of=/dev/",
    "shutdown",
    "halt",
    "reboot",
    "> /dev/sda",
    "chmod -R 777 /",
];

/// Bash Tool — execute shell commands with safety bounds.
pub struct BashTool {
    limits: Arc<FileOpsLimits>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            limits: Arc::new(FileOpsLimits::default()),
        }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash shell command with timeout, size limits, and dangerous-pattern denylist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Optional: timeout in milliseconds (default: 30000, max: 300000)"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional: working directory for the command (canonicalized)"
                }
            },
            "required": ["command"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Shell
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command parameter"))?;

        // === B2: Command length ===
        let cmd_chars = command.chars().count();
        if cmd_chars > self.limits.max_bash_command_chars {
            return Err(anyhow::anyhow!(
                "bash rejected: command too long ({} > {} chars)",
                cmd_chars,
                self.limits.max_bash_command_chars,
            ));
        }

        // === B3: Forbidden pattern denylist ===
        for pattern in FORBIDDEN_PATTERNS {
            if command.contains(pattern) {
                return Err(anyhow::anyhow!(
                    "bash rejected: command contains forbidden pattern '{}'",
                    pattern,
                ));
            }
        }

        // === B4: Working dir canonicalization ===
        let working_dir = if let Some(dir) = args["working_dir"].as_str() {
            let canonical = tokio::fs::canonicalize(dir)
                .await
                .map_err(|e| anyhow::anyhow!("working_dir '{}' not accessible: {}", dir, e))?;
            Some(canonical)
        } else {
            None
        };

        // === Timeout cap ===
        let timeout_ms = args["timeout"]
            .as_u64()
            .unwrap_or(self.limits.bash_default_timeout_ms)
            .min(self.limits.bash_max_timeout_ms);

        // Build platform-specific command
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(dir) = &working_dir {
            cmd.current_dir(dir);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null()); // B9: refuse interactive prompts

        let timeout_duration = Duration::from_millis(timeout_ms);
        let output = timeout(timeout_duration, cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("Command timed out after {}ms", timeout_ms))?
            .map_err(|e| anyhow::anyhow!("Failed to execute command: {}", e))?;

        // === B5: Binary output detection ===
        let sniff = &output.stdout[..output.stdout.len().min(self.limits.binary_sniff_bytes)];
        if sniff.contains(&0u8) {
            return Err(anyhow::anyhow!(
                "bash rejected: stdout appears binary ({} bytes); \
                 refusing to inject into LLM context",
                output.stdout.len(),
            ));
        }

        // === B1: Output size cap with UTF-8-safe truncation ===
        let max_out = self.limits.max_bash_output_bytes as usize;
        let stdout_str =
            safe_truncate_bytes(&String::from_utf8_lossy(&output.stdout), max_out).to_string();
        let stderr_str =
            safe_truncate_bytes(&String::from_utf8_lossy(&output.stderr), max_out).to_string();

        // === B6: Always return stderr (also on success) ===
        if output.status.success() {
            let mut result = stdout_str;
            if !stderr_str.is_empty() {
                result.push_str("\n--- stderr ---\n");
                result.push_str(&stderr_str);
            }
            Ok(result)
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            Err(anyhow::anyhow!(
                "Exit code: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
                exit_code,
                stdout_str,
                stderr_str,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bash_tool_dangerous() {
        let tool = BashTool::new();
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_bash_execute_success() {
        let tool = BashTool::new();
        let result = tool.execute(json!({"command": "echo hello"})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_execute_failure() {
        let tool = BashTool::new();
        let result = tool.execute(json!({"command": "exit 1"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Exit code: 1"));
    }

    #[tokio::test]
    async fn test_bash_execute_timeout() {
        let tool = BashTool::new();
        #[cfg(windows)]
        let result = tool
            .execute(json!({"command": "ping -n 10 localhost", "timeout": 100}))
            .await;
        #[cfg(not(windows))]
        let result = tool
            .execute(json!({"command": "sleep 10", "timeout": 100}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn test_bash_missing_command() {
        let tool = BashTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing command"));
    }

    // P0 hardening tests:
    #[tokio::test]
    async fn test_bash_rejects_oversized_command() {
        let limits = FileOpsLimits::new().with_max_bash_command_chars(100);
        let tool = BashTool::with_limits(Arc::new(limits));
        let big = "x".repeat(200);
        let result = tool.execute(json!({"command": big})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too long"));
    }

    #[tokio::test]
    async fn test_bash_rejects_rm_rf_root() {
        let tool = BashTool::new();
        let result = tool.execute(json!({"command": "rm -rf /"})).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("forbidden pattern"));
    }

    #[tokio::test]
    async fn test_bash_rejects_fork_bomb() {
        let tool = BashTool::new();
        let result = tool.execute(json!({"command": ":(){ :|:& };:"})).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("forbidden pattern"));
    }

    #[tokio::test]
    async fn test_bash_truncates_large_output() {
        let tool = BashTool::new();
        // Cross-platform: use python or powershell on Windows, sh on Unix
        #[cfg(windows)]
        let cmd =
            "powershell -Command \"1..10000 | ForEach-Object { Write-Output ('line' + $_) }\"";
        #[cfg(not(windows))]
        let cmd = "for i in $(seq 1 10000); do echo line$i; done";
        let result = tool.execute(json!({"command": cmd})).await.unwrap();
        // 1 MiB cap + some metadata slack
        assert!(result.len() <= 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_bash_returns_stderr_on_success() {
        let tool = BashTool::new();
        #[cfg(windows)]
        let cmd = "echo out& echo err 1>&2";
        #[cfg(not(windows))]
        let cmd = "echo out; echo err 1>&2";
        let result = tool.execute(json!({"command": cmd})).await.unwrap();
        assert!(result.contains("out"));
        assert!(result.contains("err"));
        assert!(result.contains("stderr"));
    }
}
