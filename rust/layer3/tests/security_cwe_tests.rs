//! OWASP CWE-aligned security tests (Task 6.2).
//!
//! Tests the security invariants documented in docs/SECURITY_INVARIANTS.md,
//! organized by CWE category. These cover edge cases that the M3 hardening
//! added (git_add path traversal, set_env dangerous, git_show invalid object)
//! which were missing dedicated tests.

use serde_json::json;
use sh_layer3::builtin_tools::code::GoToDefinitionTool;
use sh_layer3::builtin_tools::git_tools::{GitAddTool, GitShowTool};
use sh_layer3::builtin_tools::system_tools::SetEnvTool;
use sh_layer3::builtin_tools::BuiltinTool;

// === CWE-15: External Control of System Setting (SetEnvTool) ===

#[tokio::test]
async fn cwe15_set_env_rejects_ld_preload() {
    let result = SetEnvTool
        .execute(json!({"name": "LD_PRELOAD", "value": "/tmp/evil.so"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("dangerous"));
}

#[tokio::test]
async fn cwe15_set_env_rejects_git_dir() {
    let result = SetEnvTool
        .execute(json!({"name": "GIT_DIR", "value": "/tmp/evil"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cwe15_set_env_rejects_pythonpath() {
    let result = SetEnvTool
        .execute(json!({"name": "PYTHONPATH", "value": "/tmp/evil"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cwe15_set_env_rejects_invalid_name() {
    let result = SetEnvTool
        .execute(json!({"name": "EVIL;rm -rf /", "value": "x"}))
        .await;
    assert!(result.is_err());
}

// === CWE-22: Path Traversal (GitAddTool) ===

#[tokio::test]
async fn cwe22_git_add_rejects_path_traversal() {
    let result = GitAddTool
        .execute(json!({"files": ["../escape.txt"]}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("path traversal"));
}

#[tokio::test]
async fn cwe22_git_add_rejects_absolute_path() {
    let result = GitAddTool.execute(json!({"files": ["/etc/passwd"]})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("absolute"));
}

#[tokio::test]
async fn cwe22_git_add_rejects_windows_absolute() {
    let result = GitAddTool
        .execute(json!({"files": ["C:/Users/evil"]}))
        .await;
    assert!(result.is_err());
}

// === CWE-78: Command/Argument Injection (GitShowTool) ===

#[tokio::test]
async fn cwe78_git_show_rejects_invalid_object() {
    let result = GitShowTool
        .execute(json!({"object": "../../etc/passwd"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid object"));
}

#[tokio::test]
async fn cwe78_git_show_rejects_shell_metachar() {
    let result = GitShowTool
        .execute(json!({"object": "main; rm -rf /"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cwe78_git_show_accepts_valid_hash() {
    // Valid 7-char hex hash should pass validation (may fail at git level, but
    // the security check itself should not reject it).
    let result = GitShowTool.execute(json!({"object": "abc1234"})).await;
    // Don't assert Ok/Err — depends on git repo state. Just ensure no panic.
    let _ = result;
}

// === CWE-22: Path Traversal (code tools — round 3 audit) ===

#[tokio::test]
async fn cwe22_code_tool_rejects_critical_path() {
    // /etc/passwd is critical on Linux; nonexistent on Windows (canonicalize fails).
    // Either way, must error — never silently read system files.
    let result = GoToDefinitionTool
        .execute(json!({"file": "/etc/passwd", "line": 1, "column": 1}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cwe22_code_tool_rejects_nonexistent() {
    let result = GoToDefinitionTool
        .execute(json!({"file": "/nonexistent/code_audit_test.rs", "line": 1, "column": 1}))
        .await;
    assert!(result.is_err());
}
