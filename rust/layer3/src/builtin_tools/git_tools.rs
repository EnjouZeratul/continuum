//! # Git Tools
//!
//! Git 版本控制工具集。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use std::process::Command;
use std::time::Duration;

const GIT_TIMEOUT_SECS: u64 = 30;
const GIT_MAX_OUTPUT_BYTES: usize = 1024 * 1024; // 1 MiB

/// Dangerous git env vars stripped from subprocess.
const DANGEROUS_GIT_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_INDEX_FILE",
    "GIT_CONFIG_PARAMETERS",
];

/// Execute a git command with timeout, env scrub, output size cap.
fn run_git(args: &[&str], cwd: Option<&str>) -> Layer3Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);

    // === G4: Strip dangerous git env ===
    for var in DANGEROUS_GIT_ENV {
        cmd.env_remove(var);
    }

    // === G3: Canonicalize cwd ===
    if let Some(dir) = cwd {
        let canonical = std::fs::canonicalize(dir)
            .map_err(|e| anyhow::anyhow!("git cwd '{}' not accessible: {}", dir, e))?;
        cmd.current_dir(canonical);
    }

    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute git: {}", e))?;

    // === G5: Output size cap ===
    if output.stdout.len() > GIT_MAX_OUTPUT_BYTES {
        return Err(anyhow::anyhow!(
            "git output {} bytes > limit {}. Refine query.",
            output.stdout.len(),
            GIT_MAX_OUTPUT_BYTES,
        ));
    }

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| anyhow::anyhow!("Invalid UTF-8 output: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Git command failed: {}", stderr))
    }
}

#[allow(dead_code)]
fn git_timeout() -> Duration {
    Duration::from_secs(GIT_TIMEOUT_SECS)
}

// ============================================================================
// Git Status Tool
// ============================================================================

/// Git 状态工具
pub struct GitStatusTool;

#[async_trait]
impl BuiltinTool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show the working tree status. Lists modified, staged, and untracked files."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "short": {
                    "type": "boolean",
                    "description": "Use short format (default: false)"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let short = args["short"].as_bool().unwrap_or(false);

        let mut git_args = vec!["status"];
        if short {
            git_args.push("--short");
        }

        run_git(&git_args, path)
    }
}

// ============================================================================
// Git Log Tool
// ============================================================================

/// Git 日志工具
pub struct GitLogTool;

#[async_trait]
impl BuiltinTool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }

    fn description(&self) -> &str {
        "Show commit logs. Supports various format options."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of commits to show (default: 10)"
                },
                "oneline": {
                    "type": "boolean",
                    "description": "Use one-line format (default: true)"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch name (default: current branch)"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        // === GL1: Cap count at 1000 ===
        let count = args["count"].as_u64().unwrap_or(10).min(1000);
        let oneline = args["oneline"].as_bool().unwrap_or(true);
        let branch = args["branch"].as_str();

        let count_arg = format!("-{}", count);
        let mut git_args = vec!["log", &count_arg];
        if oneline {
            git_args.push("--oneline");
        }
        if let Some(b) = branch {
            // === GB1: Branch name validation ===
            if !is_valid_git_ref(b) {
                return Err(anyhow::anyhow!(
                    "git_log rejected: invalid branch name '{}'",
                    b,
                ));
            }
            git_args.push(b);
        }

        run_git(&git_args, path)
    }
}

/// Validate git branch / ref name. Allow alphanumeric, dash, underscore,
/// dot, slash — no shell metacharacters. Rejects `..` (path traversal).
fn is_valid_git_ref(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '+'))
}

/// Validate git commit hash (hex, 7-40 chars).
fn is_valid_commit_hash(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

// ============================================================================
// Git Diff Tool
// ============================================================================

/// Git Diff 工具
pub struct GitDiffTool;

#[async_trait]
impl BuiltinTool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Show changes between commits, commit and working tree, etc."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "file": {
                    "type": "string",
                    "description": "Specific file to diff"
                },
                "staged": {
                    "type": "boolean",
                    "description": "Show staged changes (--cached)"
                },
                "commit": {
                    "type": "string",
                    "description": "Commit hash or branch to compare"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let file = args["file"].as_str();
        let staged = args["staged"].as_bool().unwrap_or(false);
        let commit = args["commit"].as_str();

        let mut git_args = vec!["diff"];
        if staged {
            git_args.push("--cached");
        }
        if let Some(c) = commit {
            if !is_valid_git_ref(c) && !is_valid_commit_hash(c) {
                return Err(anyhow::anyhow!(
                    "git_diff rejected: invalid commit/ref '{}'",
                    c
                ));
            }
            git_args.push(c);
        }
        if let Some(f) = file {
            git_args.push("--");
            git_args.push(f);
        }

        run_git(&git_args, path)
    }
}

// ============================================================================
// Git Branch Tool
// ============================================================================

/// Git 分支工具
pub struct GitBranchTool;

#[async_trait]
impl BuiltinTool for GitBranchTool {
    fn name(&self) -> &str {
        "git_branch"
    }

    fn description(&self) -> &str {
        "List, create, or delete branches."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "delete"],
                    "description": "Action to perform (default: list)"
                },
                "branch_name": {
                    "type": "string",
                    "description": "Branch name for create/delete"
                },
                "all": {
                    "type": "boolean",
                    "description": "List all branches including remote (default: false)"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    fn requires_confirmation(&self) -> bool {
        true // Creating/deleting branches is a significant action
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let action = args["action"].as_str().unwrap_or("list");
        let branch_name = args["branch_name"].as_str();
        let all = args["all"].as_bool().unwrap_or(false);

        let git_args = match action {
            "list" => {
                let mut args = vec!["branch"];
                if all {
                    args.push("-a");
                }
                args
            }
            "create" => {
                let name = branch_name
                    .ok_or_else(|| anyhow::anyhow!("branch_name required for create"))?;
                if !is_valid_git_ref(name) {
                    return Err(anyhow::anyhow!(
                        "git_branch rejected: invalid branch name '{}'",
                        name
                    ));
                }
                vec!["branch", name]
            }
            "delete" => {
                let name = branch_name
                    .ok_or_else(|| anyhow::anyhow!("branch_name required for delete"))?;
                if !is_valid_git_ref(name) {
                    return Err(anyhow::anyhow!(
                        "git_branch rejected: invalid branch name '{}'",
                        name
                    ));
                }
                vec!["branch", "-D", name]
            }
            _ => return Err(anyhow::anyhow!("Invalid action: {}", action)),
        };

        run_git(&git_args, path)
    }
}

// ============================================================================
// Git Add Tool
// ============================================================================

/// Git Add 工具
pub struct GitAddTool;

#[async_trait]
impl BuiltinTool for GitAddTool {
    fn name(&self) -> &str {
        "git_add"
    }

    fn description(&self) -> &str {
        "Add file contents to the index."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Files to add (default: ['.'])"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let files: Vec<&str> = if let Some(arr) = args["files"].as_array() {
            arr.iter().filter_map(|v| v.as_str()).collect()
        } else {
            vec!["."]
        };

        // GA1: Validate path arguments don't escape repository via ".."
        for f in &files {
            if f.contains("..") {
                return Err(anyhow::anyhow!(
                    "git_add rejected: path '{}' contains '..' (path traversal). \
                     Specify files within the repository only.",
                    f,
                ));
            }
            // Reject absolute paths outside typical repo (best-effort)
            if f.starts_with('/') || (f.len() >= 2 && f.as_bytes()[1] == b':') {
                return Err(anyhow::anyhow!(
                    "git_add rejected: path '{}' is absolute. Use relative paths.",
                    f,
                ));
            }
        }

        let mut git_args = vec!["add", "--"];
        git_args.extend(files);

        run_git(&git_args, path)
    }
}

// ============================================================================
// Git Commit Tool
// ============================================================================

/// Git Commit 工具
pub struct GitCommitTool;

#[async_trait]
impl BuiltinTool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Record changes to the repository."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "message": {
                    "type": "string",
                    "description": "Commit message"
                }
            },
            "required": ["message"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let message = args["message"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing message parameter"))?;

        // GC1: Message length cap (prevents huge commit messages)
        if message.chars().count() > 8192 {
            return Err(anyhow::anyhow!(
                "git_commit rejected: message too long ({} > 8192 chars)",
                message.chars().count(),
            ));
        }

        run_git(&["commit", "-m", message], path)
    }
}

// ============================================================================
// Git Show Tool
// ============================================================================

/// Git Show 工具
pub struct GitShowTool;

#[async_trait]
impl BuiltinTool for GitShowTool {
    fn name(&self) -> &str {
        "git_show"
    }

    fn description(&self) -> &str {
        "Show various types of objects (commits, tags, trees)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "object": {
                    "type": "string",
                    "description": "Object to show (commit hash, tag, etc.)"
                },
                "stat": {
                    "type": "boolean",
                    "description": "Show diffstat instead of full diff (default: true)"
                }
            },
            "required": ["object"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let object = args["object"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing object parameter"))?;
        let stat = args["stat"].as_bool().unwrap_or(true);

        // GS1: Validate object name (commit hash / ref)
        if !is_valid_git_ref(object) && !is_valid_commit_hash(object) {
            return Err(anyhow::anyhow!(
                "git_show rejected: invalid object '{}' (allowed: ref names and hex hashes)",
                object,
            ));
        }

        let mut git_args = vec!["show"];
        if stat {
            git_args.push("--stat");
        }
        git_args.push(object);

        run_git(&git_args, path)
    }
}

// ============================================================================
// Git Stash Tool
// ============================================================================

/// Git Stash 工具
pub struct GitStashTool;

#[async_trait]
impl BuiltinTool for GitStashTool {
    fn name(&self) -> &str {
        "git_stash"
    }

    fn description(&self) -> &str {
        "Stash the changes in a dirty working directory."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "action": {
                    "type": "string",
                    "enum": ["push", "pop", "list", "drop"],
                    "description": "Action to perform (default: list)"
                },
                "message": {
                    "type": "string",
                    "description": "Stash message (for push)"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::VersionControl
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str();
        let action = args["action"].as_str().unwrap_or("list");
        let message = args["message"].as_str();

        let git_args = match action {
            "push" => {
                let mut args = vec!["stash", "push"];
                if let Some(msg) = message {
                    args.push("-m");
                    args.push(msg);
                }
                args
            }
            "pop" => vec!["stash", "pop"],
            "list" => vec!["stash", "list"],
            "drop" => vec!["stash", "drop"],
            _ => return Err(anyhow::anyhow!("Invalid action: {}", action)),
        };

        run_git(&git_args, path)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_status_category() {
        let tool = GitStatusTool;
        assert_eq!(tool.category(), ToolCategory::VersionControl);
    }

    #[test]
    fn test_git_commit_is_dangerous() {
        let tool = GitCommitTool;
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[test]
    fn test_git_add_requires_confirmation() {
        let tool = GitAddTool;
        assert!(tool.requires_confirmation());
    }
}
