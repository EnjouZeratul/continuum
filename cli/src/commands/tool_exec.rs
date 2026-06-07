//! 工具链执行模块
//!
//! 实现真实的工具执行逻辑。

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Blocked commands for security
const BLOCKED_COMMANDS: &[&str] = &[
    "sudo",
    "su",
    "doas",
    "eval",
    "exec",
    "mkfifo",
    "nc",
    "ncat",
    "telnet",
    "curl",
    "wget",
    "python",
    "python3",
    "perl",
    "ruby",
    "node",
    "php",
    "base64",
    "openssl",
    "xxd",
    "ssh-keygen",
    "docker",
    "kubectl",
    "kill",
    "pkill",
    "killall",
];

/// Dangerous commands requiring confirmation
const DANGEROUS_COMMANDS: &[&str] = &[
    "rm",
    "rmdir",
    "del",
    "format",
    "chmod",
    "chown",
    "git push",
    "git reset",
    "git checkout",
];

/// Validate path for traversal attacks
fn validate_path(path: &Path, _base_dir: &Path) -> Result<PathBuf> {
    // Check for path traversal attempts using ".."
    for component in path.components() {
        if let Component::ParentDir = component {
            bail!("Path traversal not allowed: {}", path.display());
        }
    }

    // Allow absolute paths and paths within current directory
    // The canonicalize will resolve symlinks if needed
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    Ok(canonical)
}

/// Validate command for security
fn validate_command(command: &str) -> Result<()> {
    let cmd_lower = command.to_lowercase();
    let cmd_trimmed = cmd_lower.trim();

    for blocked in BLOCKED_COMMANDS {
        if cmd_trimmed.starts_with(blocked) {
            bail!("Blocked command: {}", blocked);
        }
    }

    for dangerous in DANGEROUS_COMMANDS {
        if cmd_trimmed.starts_with(dangerous) {
            // Log warning but allow
            tracing::warn!("Dangerous command detected: {}", dangerous);
        }
    }

    Ok(())
}

/// Bash 工具执行结果
#[derive(Debug)]
pub struct BashResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration: Duration,
    pub timed_out: bool,
}

/// 执行 shell 命令
pub fn execute_bash(
    command: &str,
    cwd: Option<&str>,
    timeout_secs: u64,
    _capture_stderr: bool,
) -> Result<BashResult> {
    // Security: validate command first
    validate_command(command)?;

    let start = Instant::now();

    let default_dir = std::env::current_dir().unwrap_or_default();
    let cwd = cwd.map(Path::new).unwrap_or_else(|| default_dir.as_path());

    // 构建命令
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .output()
            .context("Failed to execute command")?
    } else {
        Command::new("sh")
            .args(["-c", command])
            .current_dir(cwd)
            .output()
            .context("Failed to execute command")?
    };

    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    // 检查超时（简化实现，实际超时由系统处理）
    let timed_out = duration > Duration::from_secs(timeout_secs);

    Ok(BashResult {
        stdout,
        stderr,
        exit_code,
        duration,
        timed_out,
    })
}

/// 读取文件
pub fn execute_read(
    file_path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
    show_line_numbers: bool,
) -> Result<String> {
    let path = Path::new(file_path);

    // Security: validate path
    let base_dir = std::env::current_dir().unwrap_or_default();
    validate_path(path, &base_dir)?;

    if !path.exists() {
        bail!("File not found: {}", file_path);
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", file_path))?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let start = offset.unwrap_or(0).min(total_lines);
    let end = if let Some(lim) = limit {
        (start + lim).min(total_lines)
    } else {
        total_lines
    };

    let result: String = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if show_line_numbers {
                format!("{:>6}\t{}", start + i + 1, line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 添加文件信息
    let info = format!(
        "\n\n---\nFile: {} ({} lines, showing {}-{})",
        file_path,
        total_lines,
        start + 1,
        end
    );

    Ok(result + &info)
}

/// 写入文件
pub fn execute_write(
    file_path: &str,
    content: Option<&str>,
    append: bool,
    backup: bool,
) -> Result<String> {
    let path = Path::new(file_path);

    // 检查父目录
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }
    }

    // 创建备份
    if backup && path.exists() {
        let backup_path = format!("{}.bak", file_path);
        fs::copy(path, &backup_path)
            .with_context(|| format!("Failed to create backup: {}", backup_path))?;
    }

    let content = content.unwrap_or("");

    if append && path.exists() {
        // 追加模式
        let mut existing = fs::read_to_string(path).unwrap_or_default();
        if !existing.ends_with('\n') && !existing.is_empty() {
            existing.push('\n');
        }
        existing.push_str(content);
        fs::write(path, &existing)
            .with_context(|| format!("Failed to write file: {}", file_path))?;
    } else {
        // 覆盖模式
        fs::write(path, content).with_context(|| format!("Failed to write file: {}", file_path))?;
    }

    let bytes_written = content.len();
    Ok(format!("Wrote {} bytes to {}", bytes_written, file_path))
}

/// 编辑文件（精确替换）
pub fn execute_edit(file_path: &str, old: &str, new: &str, replace_all: bool) -> Result<String> {
    let path = Path::new(file_path);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file_path);
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", file_path))?;

    let (new_content, count) = if replace_all {
        let count = content.matches(old).count();
        let new_content = content.replace(old, new);
        (new_content, count)
    } else {
        let count = if content.contains(old) { 1 } else { 0 };
        let new_content = content.replacen(old, new, 1);
        (new_content, count)
    };

    if count == 0 {
        anyhow::bail!("Pattern not found: {}", old);
    }

    fs::write(path, &new_content)
        .with_context(|| format!("Failed to write file: {}", file_path))?;

    Ok(format!("Replaced {} occurrence(s) in {}", count, file_path))
}

/// 搜索结果
#[derive(Debug)]
pub struct GrepMatch {
    pub file: String,
    pub line_number: usize,
    pub line: String,
}

/// 搜索文件内容
pub fn execute_grep(
    pattern: &str,
    path: &str,
    glob_filter: Option<&str>,
    ignore_case: bool,
    show_line_numbers: bool,
    context: Option<usize>,
) -> Result<Vec<GrepMatch>> {
    use regex::RegexBuilder;

    let regex = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

    let base_path = Path::new(path);
    let mut results = Vec::new();

    fn search_file(
        file_path: &Path,
        regex: &regex::Regex,
        _show_line_numbers: bool,
        _context: Option<usize>,
        results: &mut Vec<GrepMatch>,
    ) -> Result<()> {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return Ok(()), // 跳过无法读取的文件
        };

        for (line_num, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                results.push(GrepMatch {
                    file: file_path.display().to_string(),
                    line_number: line_num + 1,
                    line: line.to_string(),
                });
            }
        }
        Ok(())
    }

    fn walk_dir(
        dir: &Path,
        regex: &regex::Regex,
        glob_filter: Option<&str>,
        show_line_numbers: bool,
        context: Option<usize>,
        results: &mut Vec<GrepMatch>,
    ) -> Result<()> {
        if !dir.is_dir() {
            search_file(dir, regex, show_line_numbers, context, results)?;
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // 跳过隐藏目录和常见的排除目录
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.')
                        || name_str == "target"
                        || name_str == "node_modules"
                    {
                        continue;
                    }
                }
                walk_dir(
                    &path,
                    regex,
                    glob_filter,
                    show_line_numbers,
                    context,
                    results,
                )?;
            } else if path.is_file() {
                // 检查 glob 过滤
                if let Some(glob) = glob_filter {
                    let file_name = path.file_name().unwrap().to_string_lossy();
                    if !glob_match(glob, &file_name) {
                        continue;
                    }
                }
                search_file(&path, regex, show_line_numbers, context, results)?;
            }
        }
        Ok(())
    }

    walk_dir(
        base_path,
        &regex,
        glob_filter,
        show_line_numbers,
        context,
        &mut results,
    )?;
    Ok(results)
}

/// 简单的 glob 匹配
fn glob_match(pattern: &str, text: &str) -> bool {
    // 简化实现：支持基本的通配符
    if pattern == "*" {
        return true;
    }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..]; // 获取 .ext
        return text.ends_with(ext);
    }
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return text.starts_with(parts[0]) && text.ends_with(parts[1]);
        }
    }
    text.contains(pattern)
}

/// 查找文件
pub fn execute_glob(pattern: &str, base_path: &str) -> Result<Vec<String>> {
    let base = Path::new(base_path);
    let mut results = Vec::new();

    fn walk_and_match(dir: &Path, pattern: &str, results: &mut Vec<String>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // 跳过隐藏目录
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') {
                    continue;
                }
            }

            if path.is_dir() {
                walk_and_match(&path, pattern, results)?;
            } else if path.is_file() {
                let file_name = path.file_name().unwrap().to_string_lossy();
                if glob_match(pattern, &file_name) || path_matches_glob(&path, pattern) {
                    results.push(path.display().to_string());
                }
            }
        }
        Ok(())
    }

    walk_and_match(base, pattern, &mut results)?;

    // 按修改时间排序（最新的在前）
    results.sort_by(|a, b| {
        let meta_a = fs::metadata(a);
        let meta_b = fs::metadata(b);
        match (meta_a, meta_b) {
            (Ok(ma), Ok(mb)) => {
                let time_a = ma.modified().unwrap_or(std::time::UNIX_EPOCH);
                let time_b = mb.modified().unwrap_or(std::time::UNIX_EPOCH);
                time_b.cmp(&time_a)
            }
            _ => std::cmp::Ordering::Equal,
        }
    });

    Ok(results)
}

/// 检查路径是否匹配 glob 模式
fn path_matches_glob(path: &Path, pattern: &str) -> bool {
    let path_str = path.display().to_string();
    // 支持简单的 ** 和 * 匹配
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            return path_str.contains(parts[0]) && path_str.ends_with(parts[1]);
        }
    }
    path_str.contains(pattern.trim_start_matches("**/").trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_execute_bash() {
        let result = execute_bash("echo hello", None, 10, false).unwrap();
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_execute_read() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        let result = execute_read(file_path.to_str().unwrap(), Some(1), Some(1), true).unwrap();
        assert!(result.contains("line2"));
        assert!(result.contains("2")); // line number
    }

    #[test]
    fn test_execute_write() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");

        let result = execute_write(
            file_path.to_str().unwrap(),
            Some("hello world"),
            false,
            false,
        )
        .unwrap();
        assert!(result.contains("11 bytes"));

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_execute_edit() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello foo world foo").unwrap();

        let result = execute_edit(file_path.to_str().unwrap(), "foo", "bar", false).unwrap();
        assert!(result.contains("1 occurrence"));

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello bar world foo");
    }

    #[test]
    fn test_execute_edit_replace_all() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "foo foo foo").unwrap();

        let result = execute_edit(file_path.to_str().unwrap(), "foo", "bar", true).unwrap();
        assert!(result.contains("3 occurrence"));

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "bar bar bar");
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.rs", "test.rs"));
        assert!(glob_match("*.rs", "lib.rs"));
        assert!(!glob_match("*.rs", "test.txt"));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn test_execute_glob() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.rs"), "").unwrap();
        fs::write(dir.path().join("test.txt"), "").unwrap();
        fs::write(dir.path().join("lib.rs"), "").unwrap();

        let results = execute_glob("*.rs", dir.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 2);
    }
}
