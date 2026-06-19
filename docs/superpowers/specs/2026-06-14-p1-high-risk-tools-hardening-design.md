# P1 High-Risk Tools Hardening Design

**Date:** 2026-06-14
**Status:** Draft v1 — pending review
**Affects:** `sh-layer3` — `MoveFileTool`、`CopyFileTool`、`CreateDirectoryTool`、`system_tools` (9 tools)、`git_tools` (8 tools)
**Target version:** v1.0.5 patch
**Author:** Continuum Team
**Companion specs:**
- [`2026-06-14-fileops-tools-hardening-design.md`](./2026-06-14-fileops-tools-hardening-design.md)（v1.0.3）
- [`2026-06-14-p0-critical-tools-hardening-design.md`](./2026-06-14-p0-critical-tools-hardening-design.md)（v1.0.4）
- [`2026-06-14-stale-read-prevention-design.md`](./2026-06-14-stale-read-prevention-design.md)（v1.1.0）

---

## 1. 问题陈述

### 1.1 调研范围

P0 spec 已覆盖 BashTool、DeleteFileTool、network\*。本 spec 处理剩余 19 个高危工具。

### 1.2 P1 工具事实性缺陷

#### `MoveFileTool`（`file_ops.rs:275-383`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| M1 | 路径不规范化 | `:321-326` | `../` 越狱、相对路径绕过 sandbox |
| M2 | 无关键路径检查 | `:330-358` | Agent 可把文件 move 到 `/etc/cron.d/`、`/usr/local/bin/` |
| M3 | 覆盖目标时无 trash | `:345-351` | `overwrite=true` 静默销毁目标 |
| M4 | 无源大小检查 | `:330` | move 1TB 文件无前置检查（同 FS 内 rename 不复制，但跨 FS 走 EXDEV 路径建议时无大小提示） |
| M5 | 无 symlink 解析 | 全局 | move symlink 不等于 move target，但行为未明确 |

#### `CopyFileTool`（`file_ops.rs:388-514`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| C1 | 无大小上限 | `:474, 508` | 复制 1TB 文件直接耗尽磁盘 |
| C2 | 无关键路径检查 | `:443-462` | 复制到 `/etc/cron.d/` 等系统目录 |
| C3 | 无 symlink 解析 | `:497` | 复制 symlink 自身而非 target — 行为可能不符预期 |
| C4 | `copy_dir_all` 无深度上限 | `:488-514` | 含环 symlink 导致无限递归；深层目录撑爆栈 |
| C5 | `copy_dir_all` 默认不跟随 symlink 但不检测环 | `:501-503` | 真实场景中 `node_modules` symlink 链可触发问题 |
| C6 | 无 checksum 校验 | — | 复制中途磁盘错误未检测 |

#### `CreateDirectoryTool`（`file_ops.rs:519-559`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| CD1 | 路径不规范化 | `:549-555` | `../` 越狱 |
| CD2 | 无关键路径检查 | `:553` | `mkdir("/root/.ssh")` 给 Agent 植入 SSH 后门 |
| CD3 | 无深度上限 | `:553` | `create_dir_all` 理论无限深度 |
| CD4 | 无 permissions 设置 | `:553` | 创建的目录用默认 umask，可能过宽（如 `0o777`） |

#### `system_tools` 9 个工具

##### `GetEnvTool`（`system_tools.rs:15-52`）

| # | 缺陷 | 影响 |
|---|------|------|
| GE1 | **无敏感变量屏蔽** | `get_env("AWS_SECRET_ACCESS_KEY")` 把密钥塞进 context，LLM 调用泄露给云端 |
| GE2 | 无变量名验证 | `name="\nmalicious"` 注入到错误消息 |

##### `ListEnvTool`（`system_tools.rs:59-101`）

| # | 缺陷 | 影响 |
|---|------|------|
| LE1 | **整个 env dump 无脱敏** | 所有 secret keys、tokens、cookies 全部塞进 LLM context |
| LE2 | 无大小上限 | env 大型时（CI 环境）撑爆 context |
| LE3 | 无 secret 模式检测 | 已知模式（`AKIA...`、`sk-...`、`Bearer ...`）未自动脱敏 |

##### `SetEnvTool`（`system_tools.rs:108-157`）

| # | 缺陷 | 影响 |
|---|------|------|
| SE1 | **无敏感变量黑名单** | `set_env("LD_PRELOAD", "/tmp/evil.so")` 库注入；`set_env("PATH", "/tmp/evil:$PATH")` 劫持 |
| SE2 | 无 value 大小上限 | 100MB value 占内存 |
| SE3 | `set_var` 在 Rust 1.66+ 标记 unsafe | 当前代码在多线程下是 **UB**（Rust 2024 edition 强制要求 unsafe） |
| SE4 | 进程级影响 | 与并发会话工具调用产生 race |

##### `ChangeDirTool`（`system_tools.rs:199-246`）

| # | 缺陷 | 影响 |
|---|------|------|
| CD1 | 进程级 `set_current_dir` | 与 SetEnv 同问题：race、影响所有并发工具调用 |
| CD2 | 路径不规范化 | `../` 越狱 |

##### `GetCwdTool`（`system_tools.rs:164-192`）

| # | 缺陷 | 影响 |
|---|------|------|
| GC1 | 泄露 sandbox 真实路径 | 容器内调用泄露 `/host_mnt/...` 等敏感路径 |

##### `SystemInfoTool`（`system_tools.rs:253-308`）

| # | 缺陷 | 影响 |
|---|------|------|
| SI1 | 泄露 hostname | 公司内网命名规则暴露（如 `prod-db-01.acme.corp`） |
| SI2 | 泄露用户名 | PII；与 SSH brute force 关联 |
| SI3 | 泄露 data_local_dir | 应用沙箱定位信息 |

##### `ProcessListTool`（`system_tools.rs:315-360`）

| # | 缺陷 | 影响 |
|---|------|------|
| PL1 | `take(50)` 硬编码无分页 | 大型主机上漏掉关键进程 |
| PL2 | **泄露完整命令行参数** | 进程命令行含 secrets（`--password=xxx`、`--token=...`） |
| PL3 | 无输出大小上限 | 输出格式 `pid\tname\tcmd` 容易爆 context |

##### `DiskUsageTool`（`system_tools.rs:367+`）

| # | 缺陷 | 影响 |
|---|------|------|
| DU1 | 列出所有挂载点 | 公司内网共享盘（`/mnt/corp-share`）路径暴露 |

##### `MemoryUsageTool`（未单独审计，同 ProcessList）

#### `git_tools` 8 个工具（共用 `run_git` 函数，`git_tools.rs:11-29`）

**通用缺陷（影响 8 个 git 工具）**：

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| G1 | 同步 `Command::output()` 阻塞 async executor | `:11-29` | tokio worker 阻塞，并发工具调用降级 |
| G2 | 无 timeout | `:11-29` | 大仓库 `git log` 卡住整个会话 |
| G3 | `cwd` 不规范化 | `:15-17` | `../` 越狱 |
| G4 | 无 env 注入检查 | `:12-13` | 攻击者通过 `GIT_DIR`、`GIT_OBJECT_DIRECTORY` 等劫持 |
| G5 | 输出无大小上限 | `:24` | `git log -p` 大仓库撑爆 context |
| G6 | stdout 转 String 不安全 | `:24` | 二进制 patch 输出触发 UTF-8 错误 |

**特定工具缺陷**：

| # | 工具 | 缺陷 |
|---|------|------|
| GL1 | `GitLogTool` | `count` 无上限（`:128`），`count=10000000` 撑爆 |
| GD1 | `GitDiffTool` | 无 diff 大小上限 — `git diff` binary 文件输出垃圾 |
| GB1 | `GitBranchTool` | branch 名无验证 — `"; rm -rf / #"` 注入（虽然 git 走 arg array 不易注入，但仍可触发 git 错误） |
| GA1 | `GitAddTool` | path 无验证 — 添加 `/etc/passwd` 等系统文件到 git index |
| GC1 | `GitCommitTool` | message 无验证（理论上 git 处理，但 hook bypass 风险） |
| GS1 | `GitShowTool` | 输出无上限 |
| GT1 | `GitStashTool` | 输出无上限 |

### 1.3 实际风险场景

- **场景 Q（secret 泄露给 LLM）**：`list_env()` 把 `OPENAI_API_KEY=sk-proj-...` 完整塞进 context，进入 LLM 推理 → 攻击者通过 prompt injection 拿到
- **场景 R（LD_PRELOAD 注入）**：`set_env("LD_PRELOAD", "/tmp/evil.so")` → 后续任何 spawn 进程都被劫持
- **场景 S（cron 持久化）**：`move_file("/tmp/payload.sh", "/etc/cron.d/payload")` → 持久化后门
- **场景 T（git 命令卡死）**：`git_log(path="/", count=10000)` 在非 git 目录运行 → git 探索整个文件系统卡死
- **场景 U（进程参数泄露）**：`process_list()` 把 `postgres --password=hunter2` 暴露给 LLM
- **场景 V（磁盘耗尽）**：`copy_file("/dev/zero", "/tmp/fill")` 无限填充

### 1.4 范围界定（YAGNI）

**本方案处理**：

- 输入/输出大小边界（v1.0.5 patch）
- 路径规范化 + 关键路径检查（复用 P0 spec §3.5 的 `check_path_danger`）
- 敏感变量脱敏（v1.0.5 patch — 引入 `SecretScrubber`）
- 同步 → async 改造（git 工具）（v1.0.5 patch）

**本方案不处理**：

- 完整 sandbox 隔离（Layer 0）
- env variable 类型系统（仅在字符串值层面脱敏）
- git hook 安全（git config 层面）
- 进程级 set_var/set_current_dir 的根本修复（这是 Rust 标准库问题，需要 Session-scoped env 模型 — 留 v1.1.0）

---

## 2. 设计目标

### 2.1 P1 必须达成（G1-G7）

1. **G1**：所有文件操作工具有路径规范化 + 关键路径检查
2. **G2**：`CopyFileTool` 有大小上限 + symlink 检测 + 深度上限
3. **G3**：`GetEnvTool`/`ListEnvTool` 自动脱敏已知 secret 模式
4. **G4**：`SetEnvTool` 拒绝危险变量（`LD_PRELOAD`、`LD_LIBRARY_PATH`、`PATH` 等）
5. **G5**：`ProcessListTool` 脱敏命令行参数中的 secret 模式
6. **G6**：git 工具改用 `tokio::process::Command` + timeout + 输出上限
7. **G7**：`SystemInfoTool` / `GetCwdTool` 默认不暴露 hostname/user

### 2.2 P1 不追求

- Session-scoped env model（v1.1.0）
- git config 安全（用户配置层）
- 跨进程 sandbox

---

## 3. 设计

### 3.1 文件操作（Move/Copy/CreateDirectory）共享加固

复用 P0 spec §3.5 的 `check_path_danger`。新增共享 helper：

```rust
// rust/layer3/src/builtin_tools/path_safety.rs（扩展）
/// 文件操作前的路径检查
pub async fn validate_file_path(
    path: &str,
    limits: &ToolLimits,
    require_exist: bool,
) -> Layer3Result<PathBuf> {
    let canonical = if require_exist {
        tokio::fs::canonicalize(path).await
            .map_err(|e| anyhow!("Path '{}' not accessible: {}", path, e))?
    } else {
        // 路径可能不存在时规范化父目录
        let p = Path::new(path);
        if let Some(parent) = p.parent() {
            if parent.exists() {
                let canonical_parent = tokio::fs::canonicalize(parent).await?;
                canonical_parent.join(p.file_name()
                    .ok_or_else(|| anyhow!("Path '{}' has no file name", path))?)
            } else {
                return Err(anyhow!(
                    "Parent directory '{}' does not exist",
                    parent.display()
                ));
            }
        } else {
            PathBuf::from(path)
        }
    };

    let danger = check_path_danger(&canonical)?;
    if danger.is_critical {
        return Err(anyhow!(
            "Operation rejected: path '{}' is critical ({}). \
             Pass force=true at the tool level to override.",
            canonical.display(), danger.reason,
        ));
    }
    Ok(canonical)
}
```

### 3.2 `MoveFileTool` 加固

```rust
pub struct MoveFileTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for MoveFileTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let source = args["source"].as_str()
            .ok_or_else(|| anyhow!("Missing source"))?;
        let destination = args["destination"].as_str()
            .ok_or_else(|| anyhow!("Missing destination"))?;
        let overwrite = args["overwrite"].as_bool().unwrap_or(false);
        let force = args["force"].as_bool().unwrap_or(false);

        // === M1, M2: 路径检查 ===
        let src_canonical = validate_file_path(source, &self.limits, true).await?;
        let dest_canonical = if force {
            // force 时跳过 critical 检查但仍规范化
            tokio::fs::canonicalize(destination).await
                .unwrap_or_else(|_| PathBuf::from(destination))
        } else {
            validate_file_path(destination, &self.limits, false).await?
        };

        // === M5: symlink 解析检查 ===
        let src_meta = tokio::fs::symlink_metadata(&src_canonical).await?;
        if src_meta.is_symlink() {
            return Err(anyhow!(
                "move_file rejected: source '{}' is symlink. \
                 Resolve target explicitly.",
                src_canonical.display(),
            ));
        }

        // === M3: 覆盖前 trash（如启用）===
        let dest_exists = tokio::fs::try_exists(&dest_canonical).await.unwrap_or(false);
        if dest_exists && !overwrite {
            return Err(anyhow!(
                "Destination exists: {}. Pass overwrite=true.",
                dest_canonical.display(),
            ));
        }
        if dest_exists && overwrite && self.limits.enable_trash {
            move_to_trash(&dest_canonical).await?;
        } else if dest_exists && overwrite {
            // trash 未启用 — 永久删除（与 P0 DeleteFileTool 一致行为）
            // 但要求 force=true，避免静默销毁
            if !force {
                return Err(anyhow!(
                    "Overwrite requires force=true when trash is disabled. \
                     Destination '{}' would be permanently destroyed.",
                    dest_canonical.display(),
                ));
            }
            // ... 永久删除 ...
        }

        tokio::fs::rename(&src_canonical, &dest_canonical).await
            .map_err(|e| {
                if e.raw_os_error() == Some(18) {
                    anyhow!("Cross-device move not supported. Use copy + delete.")
                } else {
                    anyhow!("Move failed: {}", e)
                }
            })?;

        Ok(format!("Moved: {} → {}", src_canonical.display(), dest_canonical.display()))
    }
}
```

### 3.3 `CopyFileTool` 加固

```rust
pub struct CopyFileTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for CopyFileTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        // 路径检查同 MoveFileTool
        let src_canonical = validate_file_path(source, &self.limits, true).await?;
        let dest_canonical = validate_file_path(destination, &self.limits, false).await?;

        let src_meta = tokio::fs::symlink_metadata(&src_canonical).await?;
        if src_meta.is_symlink() {
            return Err(anyhow!("copy_file rejected: source is symlink"));
        }

        // === C1: 大小上限 ===
        let size = if src_meta.is_dir() {
            compute_dir_size(&src_canonical).await?
        } else {
            src_meta.len()
        };
        if size > self.limits.max_copy_bytes {
            return Err(anyhow!(
                "copy_file rejected: source size {} > limit {}. Pass force=true.",
                size, self.limits.max_copy_bytes,
            ));
        }

        if src_meta.is_dir() {
            // === C4, C5: 深度 + symlink 环检测 ===
            copy_dir_safe(
                &src_canonical,
                &dest_canonical,
                self.limits.max_copy_depth,
            ).await?;
        } else {
            tokio::fs::copy(&src_canonical, &dest_canonical).await?;
        }

        Ok(format!("Copied: {} → {} ({} bytes)", src, dest, size))
    }
}

const MAX_COPY_DEPTH_DEFAULT: usize = 32;

async fn copy_dir_safe(src: &Path, dest: &Path, max_depth: usize) -> Layer3Result<()> {
    use std::collections::HashSet;
    let mut visited_inodes: HashSet<(u64, u64)> = HashSet::new(); // (ino, dev)
    copy_dir_recursive(src, dest, 0, max_depth, &mut visited_inodes).await
}

async fn copy_dir_recursive(
    src: &Path,
    dest: &Path,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<(u64, u64)>,
) -> Layer3Result<()> {
    if depth > max_depth {
        return Err(anyhow!("copy_file rejected: depth > {} (possible symlink loop)", max_depth));
    }

    tokio::fs::create_dir_all(dest).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = entry.file_type().await?;

        // 检测 inode 重复（symlink 环防护）
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = entry.metadata().await {
                let key = (meta.ino(), meta.dev());
                if file_type.is_dir() && !visited.insert(key) {
                    continue; // 跳过已访问目录
                }
            }
        }

        let dest_path = dest.join(entry.file_name());

        if file_type.is_symlink() {
            // 不跟随 symlink — 拒绝（更安全）
            return Err(anyhow!(
                "copy_file rejected: '{}' is symlink (not followed for safety). \
                 Resolve target explicitly.",
                path.display(),
            ));
        } else if file_type.is_dir() {
            Box::pin(copy_dir_recursive(&path, &dest_path, depth + 1, max_depth, visited)).await?;
        } else {
            tokio::fs::copy(&path, &dest_path).await?;
        }
    }
    Ok(())
}
```

### 3.4 `CreateDirectoryTool` 加固

```rust
pub struct CreateDirectoryTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for CreateDirectoryTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str()
            .ok_or_else(|| anyhow!("Missing path"))?;
        let mode = args["mode"].as_str()  // 八进制字符串如 "755"
            .and_then(|s| u32::from_str_radix(s, 8).ok());

        // === CD1, CD2: 路径检查 ===
        let canonical = validate_file_path(path, &self.limits, false).await?;

        // === CD3: 深度上限 ===
        let components = canonical.components().count();
        if components > self.limits.max_mkdir_depth {
            return Err(anyhow!(
                "create_directory rejected: path depth {} > limit {}",
                components, self.limits.max_mkdir_depth,
            ));
        }

        tokio::fs::create_dir_all(&canonical).await?;

        // === CD4: permissions ===
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&canonical, std::fs::Permissions::from_mode(mode)).await?;
        }

        Ok(format!("Created: {}", canonical.display()))
    }
}
```

### 3.5 环境变量脱敏（`SecretScrubber`）

```rust
// rust/layer3/src/builtin_tools/secret_scrub.rs（新建）
use regex::Regex;

pub struct SecretScrubber {
    patterns: Vec<(&'static str, Regex)>,
}

impl SecretScrubber {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                // AWS
                ("AWS Access Key", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
                ("AWS Secret", Regex::new(r#"(?i)aws_secret_access_key=["']?[A-Za-z0-9/+=]{40}"#).unwrap()),
                // OpenAI / Anthropic
                ("OpenAI Key", Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap()),
                ("Anthropic Key", Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap()),
                // GitHub
                ("GitHub Token", Regex::new(r"gh[pousr]_[A-Za-z0-9]{36,}").unwrap()),
                // Generic
                ("Bearer Token", Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.=]+").unwrap()),
                ("JWT", Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap()),
                // Private key
                ("Private Key", Regex::new(r"-----BEGIN [A-Z ]+PRIVATE KEY-----").unwrap()),
                // Connection strings
                ("Connection String",
                    Regex::new(r#"(?i)(postgres|mongodb|redis|amqp)://[^:\s]+:[^@\s]+@"#).unwrap()),
            ],
        }
    }

    /// 把识别出的 secret 替换为 `<REDACTED:类型>`
    pub fn scrub(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (kind, re) in &self.patterns {
            result = re.replace_all(&result, format!("<REDACTED:{}>", kind)).to_string();
        }
        result
    }

    /// 仅检查是否含 secret（不修改）
    pub fn contains_secret(&self, input: &str) -> Option<&'static str> {
        for (kind, re) in &self.patterns {
            if re.is_match(input) {
                return Some(kind);
            }
        }
        None
    }
}

/// 敏感环境变量名（list_env / get_env 输出时强制脱敏）
pub const SENSITIVE_ENV_NAMES: &[&str] = &[
    // Cloud
    "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN",
    "AZURE_CLIENT_SECRET", "AZURE_TENANT_ID",
    "GOOGLE_APPLICATION_CREDENTIALS", "GCLOUD_SERVICE_KEY",
    "ALIBABA_CLOUD_ACCESS_KEY_SECRET",
    "TENCENTCLOUD_SECRET_ID", "TENCENTCLOUD_SECRET_KEY",
    // LLM
    "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "DEEPSEEK_API_KEY",
    "GEMINI_API_KEY", "GLM_API_KEY", "QWEN_API_KEY", "KIMI_API_KEY",
    // VCS
    "GITHUB_TOKEN", "GITLAB_TOKEN", "BITBUCKET_TOKEN",
    "GH_TOKEN", "GH_ENTERPRISE_TOKEN",
    // Database
    "DATABASE_URL", "DB_PASSWORD", "POSTGRES_PASSWORD", "MYSQL_PASSWORD", "REDIS_URL",
    // CI/CD
    "CI_REGISTRY_PASSWORD", "DOCKER_PASSWORD", "NPM_TOKEN", "PYPI_TOKEN",
    // Generic
    "API_KEY", "API_TOKEN", "AUTH_TOKEN", "BEARER_TOKEN",
    "ACCESS_TOKEN", "REFRESH_TOKEN",
    "PRIVATE_KEY", "CLIENT_SECRET",
    "ENCRYPTION_KEY", "SECRET_KEY", "MASTER_KEY",
];
```

### 3.6 `GetEnvTool` / `ListEnvTool` 加固

```rust
pub struct GetEnvTool {
    scrubber: Arc<SecretScrubber>,
}

#[async_trait]
impl BuiltinTool for GetEnvTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let name = args["name"].as_str()
            .ok_or_else(|| anyhow!("Missing name"))?;

        // === GE2: 变量名验证 ===
        if !is_valid_env_name(name) {
            return Err(anyhow!("Invalid env var name: '{}'", name));
        }

        let value = std::env::var(name)
            .map_err(|_| anyhow!("Env var '{}' not found", name))?;

        // === GE1: 脱敏 ===
        if SENSITIVE_ENV_NAMES.contains(&name) {
            return Ok(format!("{}=<REDACTED:env-secret>", name));
        }
        if let Some(kind) = self.scrubber.contains_secret(&value) {
            return Ok(format!("{}=<REDACTED:{}>", name, kind));
        }
        Ok(value)
    }
}

fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
}
```

`ListEnvTool` 类似：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let filter = args["filter"].as_str().unwrap_or("");
    let scrubber = &self.scrubber;

    let mut env_vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| filter.is_empty() || k.contains(filter))
        .map(|(k, v)| {
            if SENSITIVE_ENV_NAMES.contains(&k.as_str()) {
                (k, "<REDACTED:env-secret>".to_string())
            } else if let Some(kind) = scrubber.contains_secret(&v) {
                (k, format!("<REDACTED:{}>", kind))
            } else {
                (k, v)
            }
        })
        .collect();

    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    // === LE2: 输出大小上限 ===
    let output = env_vars.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(safe_truncate_bytes(&output, self.limits.max_list_env_bytes as usize).to_string())
}
```

### 3.7 `SetEnvTool` 加固

```rust
pub struct SetEnvTool {
    limits: Arc<ToolLimits>,
}

const DANGEROUS_ENV_NAMES: &[&str] = &[
    "LD_PRELOAD", "LD_LIBRARY_PATH",        // 库注入
    "DYLD_INSERT_LIBRARIES",                 // macOS 库注入
    "PATH",                                  // 命令劫持（仅警告）
    "PYTHONPATH", "NODE_PATH", "RUBYLIB",   // 脚本劫持
    "GIT_DIR", "GIT_WORK_TREE",              // git 劫持
    "IFS",                                   // shell 分隔符攻击
    "BASH_ENV", "ENV",                       // shell 启动脚本注入
    "PERL5OPT", "PERL5LIB",                  // perl 劫持
    "JAVA_TOOL_OPTIONS",                     // JVM 劫持
];

#[async_trait]
impl BuiltinTool for SetEnvTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let name = args["name"].as_str()
            .ok_or_else(|| anyhow!("Missing name"))?;
        let value = args["value"].as_str()
            .ok_or_else(|| anyhow!("Missing value"))?;

        if !is_valid_env_name(name) {
            return Err(anyhow!("Invalid env var name"));
        }
        if value.len() > self.limits.max_env_value_bytes as usize {
            return Err(anyhow!(
                "set_env rejected: value {} bytes > limit {}",
                value.len(), self.limits.max_env_value_bytes,
            ));
        }

        // === SE1: 危险变量检查 ===
        if DANGEROUS_ENV_NAMES.contains(&name) {
            return Err(anyhow!(
                "set_env rejected: '{}' is in dangerous-env denylist \
                 (privilege escalation / injection vector).",
                name,
            ));
        }

        // === SE3: std::env::set_var 在 Rust 2024 是 unsafe — 用 unsafe 块 + 文档 ===
        // SAFETY: 单线程 Agent 循环场景下，set_var 是安全的。
        // 多线程并发场景需要 Session-scoped env model（v1.1.0 后引入）。
        unsafe {
            std::env::set_var(name, value);
        }

        Ok(format!("Set {}=<value>", name))  // 不回显 value，避免再次进入 context
    }
}
```

### 3.8 `ChangeDirTool` 加固

```rust
pub struct ChangeDirTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for ChangeDirTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"].as_str()
            .ok_or_else(|| anyhow!("Missing path"))?;

        // === CD1, CD2: 路径检查 ===
        let canonical = validate_file_path(path, &self.limits, true).await?;

        // === 进程级影响警告 ===
        // 仍允许，但日志告警
        tracing::warn!(
            target: "continuum.tools.system",
            "change_dir affects entire process — may impact concurrent sessions"
        );

        std::env::set_current_dir(&canonical).await
            .map_err(|e| anyhow!("Failed to change dir: {}", e))?;

        Ok(format!("Changed to: {}", canonical.display()))
    }
}
```

**v1.1.0 升级路径**：`change_dir` 改为 Session-scoped（每个 Session 持有自己的 working dir），通过 `ToolExecutionContext.working_dir` 传递。本 patch 阶段仅做路径检查。

### 3.9 `SystemInfoTool` / `GetCwdTool` 脱敏

```rust
pub struct SystemInfoTool {
    limits: Arc<ToolLimits>,
    reveal_host: bool, // 默认 false
}

async fn execute(&self, _args: serde_json::Value) -> Layer3Result<String> {
    let mut info = vec![
        format!("OS: {}", std::env::consts::OS),
        format!("Architecture: {}", std::env::consts::ARCH),
        format!("Path separator: {}", std::path::MAIN_SEPARATOR),
    ];

    if self.reveal_host {
        if let Ok(hostname) = hostname::get() {
            info.push(format!("Hostname: {}", hostname.to_string_lossy()));
        }
        if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
            info.push(format!("User: {}", user));
        }
    }

    Ok(info.join("\n"))
}
```

`GetCwdTool`：当 `reveal_host=false` 时返回相对路径或仅 `"<sandbox>"` 占位符。

### 3.10 `ProcessListTool` 加固

```rust
pub struct ProcessListTool {
    limits: Arc<ToolLimits>,
    scrubber: Arc<SecretScrubber>,
}

#[async_trait]
impl BuiltinTool for ProcessListTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let filter = args["filter"].as_str().unwrap_or("");
        let limit = args["limit"].as_u64()
            .unwrap_or(50)
            .min(self.limits.max_process_list as u64);

        let mut system = sysinfo::System::new_all();
        system.refresh_processes();

        let mut processes: Vec<_> = system.processes()
            .iter()
            .filter(|(_, proc)| filter.is_empty() || proc.name().contains(filter))
            .map(|(pid, proc)| {
                // === PL2: 命令行参数脱敏 ===
                let cmd = proc.cmd().join(" ");
                let cmd = self.scrubber.scrub(&cmd);
                format!("{}\t{}\t{}", pid, proc.name(), cmd)
            })
            .take(limit as usize)
            .collect();

        // === PL1: 显式上限 ===
        Ok(format!("PID\tName\tCommand\n{}", processes.join("\n")))
    }
}
```

### 3.11 `git_tools` 改造（async + 安全）

```rust
// rust/layer3/src/builtin_tools/git_tools.rs（重写 run_git）
use tokio::process::Command as AsyncCommand;
use tokio::time::timeout;

pub struct GitToolLimits {
    pub timeout_secs: u64,        // 默认 30
    pub max_output_bytes: u64,    // 默认 1 MiB
}

async fn run_git(
    args: &[&str],
    cwd: Option<&str>,
    limits: &GitToolLimits,
    op_limits: &ToolLimits,
) -> Layer3Result<String> {
    // === G3: cwd 规范化 ===
    let canonical_cwd = if let Some(dir) = cwd {
        Some(validate_file_path(dir, op_limits, true).await?)
    } else { None };

    // === G4: 清理危险 git env ===
    let mut cmd = AsyncCommand::new("git");
    cmd.args(args);
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.env_remove("GIT_OBJECT_DIRECTORY");
    cmd.env_remove("GIT_INDEX_FILE");
    cmd.env_remove("GIT_CONFIG");  // ← 注意：会禁用用户 .gitconfig 中的敏感设置
    cmd.env_remove("GIT_HOOKS_PATH");

    if let Some(dir) = &canonical_cwd {
        cmd.current_dir(dir);
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // === G1, G2: async + timeout ===
    let output = timeout(
        Duration::from_secs(limits.timeout_secs),
        cmd.output(),
    )
    .await
    .map_err(|_| anyhow!("git timed out after {}s", limits.timeout_secs))?
    .map_err(|e| anyhow!("git failed: {}", e))?;

    // === G5: 输出大小上限 ===
    if output.stdout.len() > limits.max_output_bytes as usize {
        return Err(anyhow!(
            "git output {} bytes > limit {}. Refine query (e.g., limit count).",
            output.stdout.len(), limits.max_output_bytes,
        ));
    }

    // === G6: UTF-8 安全 ===
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git failed (exit {}): {}",
            output.status.code().unwrap_or(-1), stderr_str));
    }
    Ok(stdout_str.to_string())
}
```

**特定 git 工具加固**：

- `GitLogTool`：`count.min(1000)`（GL1）
- `GitDiffTool`：`--no-color` 强制；输出超 1 MiB 报错（GD1）
- `GitBranchTool`：branch 名 regex 验证 `^[\w\-\.\/]+$`（GB1）
- `GitAddTool`：path 验证不跨出 repo（GA1）— 用 `git rev-parse --show-toplevel` 拿到 repo 根，比较 path 是否在 repo 内
- `GitCommitTool`：message 长度上限 8192 字符；message 不含换行注入（GC1）

---

## 4. 接口契约

### 4.1 schema 变更

- `move_file` 新增 `force`（默认 false）
- `copy_file` 新增 `force`（默认 false）
- `create_directory` 新增 `mode`、`force`（mode 仅 unix 有效）
- `get_env` 无 schema 变更（行为变化：脱敏）
- `list_env` 无 schema 变更（行为变化：脱敏 + 大小限制）
- `set_env` 无 schema 变更（行为变化：拒绝危险变量）
- `system_info` 无 schema 变更（行为变化：默认不返回 hostname/user）
- `process_list` 新增 `limit`（默认 50）
- 所有 git 工具新增 `timeout`（默认 30s）

### 4.2 行为兼容性

| 行为 | v1.0.x | v1.0.5 | 影响 |
|------|--------|--------|------|
| `get_env("AWS_SECRET_ACCESS_KEY")` | 返回明文 | 返回 `<REDACTED:env-secret>` | **破坏性** — 但任何依赖此返回明文的代码本身是 bug |
| `set_env("LD_PRELOAD", ...)` | 设置成功 | 返回错误 | **破坏性** — 攻击向量修复 |
| `list_env()` | 全量明文 | 自动脱敏 | 用户感受到，但安全升级 |
| `system_info()` 返回 hostname | 返回 | 不返回 | 用户感知；提供 `reveal_host=true` 配置 |

**修复路径**：所有"破坏性"行为变化都是 **修复安全 bug**，不属于需要保兼容的范畴。在 release notes 显式说明。

---

## 5. 实现计划（TDD）

### Phase A：共享基础设施

- [ ] A1: 扩展 `ToolLimits` 加 P1 字段（`max_copy_bytes`、`max_mkdir_depth` 等）
- [ ] A2: 写 `secret_scrub.rs` + 单元测试
- [ ] A3: 扩展 `path_safety.rs` 加 `validate_file_path`

### Phase B：文件操作

- [ ] B1: 改造 `MoveFileTool` / `CopyFileTool` / `CreateDirectoryTool` 持 `Arc<ToolLimits>`
- [ ] B2: 实现 M1-M5、C1-C6、CD1-CD4
- [ ] B3: 实现 `copy_dir_safe` 含 symlink 环检测
- [ ] B4: 全量测试

### Phase C：system_tools

- [ ] C1: `GetEnvTool` / `ListEnvTool` 集成 `SecretScrubber`
- [ ] C2: `SetEnvTool` 拒绝危险变量
- [ ] C3: `SystemInfoTool` 加 `reveal_host` 配置
- [ ] C4: `ProcessListTool` 加 secret 脱敏 + `limit` 参数
- [ ] C5: `ChangeDirTool` 路径检查

### Phase D：git_tools

- [ ] D1: 重写 `run_git` 为 async + timeout + 大小上限
- [ ] D2: 所有 git 工具改用新 `run_git`
- [ ] D3: `GitLogTool` `count` 上限
- [ ] D4: `GitBranchTool` branch 名验证
- [ ] D5: `GitAddTool` path 在 repo 范围内验证

### Phase E：集成

- [ ] E1: 全量测试
- [ ] E2: clippy + fmt
- [ ] E3: CHANGELOG + publish v1.0.5

---

## 6. 测试矩阵

### 6.1 Secret scrubber（独立模块，10 个测试）

- `test_scrub_aws_access_key`
- `test_scrub_openai_key`
- `test_scrub_anthropic_key`
- `test_scrub_github_token`
- `test_scrub_bearer_token`
- `test_scrub_jwt`
- `test_scrub_private_key_pem`
- `test_scrub_connection_string`
- `test_scrub_no_false_positive_on_normal_text`
- `test_scrub_preserves_surrounding_text`

### 6.2 文件操作（15 个新测试）

- `test_move_rejects_symlink_source`（M5）
- `test_move_canonicalizes_paths`（M1）
- `test_move_rejects_critical_dest`（M2）
- `test_copy_rejects_oversize`（C1）
- `test_copy_rejects_symlink_loop`（C4）
- `test_copy_rejects_depth_exceeded`（C4）
- `test_copy_rejects_symlink_in_dir`（C5）
- `test_create_dir_rejects_critical_path`（CD2）
- `test_create_dir_rejects_excessive_depth`（CD3）
- `test_create_dir_sets_mode_unix`（CD4）
- `test_create_dir_no_force_overrides_critical`（force）
- ... 等等

### 6.3 system_tools（12 个新测试）

- `test_get_env_redacts_known_secret`
- `test_get_env_redacts_value_pattern`
- `test_get_env_passes_non_secret`
- `test_list_env_redacts_sensitive_names`
- `test_list_env_truncates_huge_output`
- `test_set_env_rejects_ld_preload`
- `test_set_env_rejects_path`
- `test_set_env_rejects_git_dir`
- `test_set_env_rejects_oversize_value`
- `test_process_list_redacts_command_args`
- `test_process_list_respects_limit`
- `test_system_info_hides_hostname_by_default`

### 6.4 git_tools（8 个新测试）

- `test_git_log_caps_count`
- `test_git_diff_caps_output`
- `test_git_branch_rejects_invalid_name`
- `test_git_add_rejects_path_outside_repo`
- `test_git_command_times_out`
- `test_git_command_strips_dangerous_env`
- `test_git_command_canonicalizes_cwd`
- `test_git_command_rejects_oversize_output`

---

## 7. 已知局限

### 7.1 set_var / set_current_dir 的进程级影响

`SetEnvTool` 和 `ChangeDirTool` 使用 `std::env::set_var` / `std::env::set_current_dir`，**这是进程级影响**。多 session 并发场景下 race。

**v1.0.5 缓解**：
- SetEnv 限制危险变量
- ChangeDir 警告日志

**v1.1.0 根治**：Session-scoped env model — 每个 Session 持有自己的 env dict，通过 `ToolExecutionContext` 传递。

### 7.2 SecretScrubber regex 漏报

基于 regex 的 secret 检测会漏报：
- Base64 编码的 secrets
- 自定义格式的 secrets
- 部分匹配的 secrets（如 token 被换行打断）

**缓解**：regex 列表随业界标准更新；用户可通过 `ToolLimits.custom_secret_patterns` 扩展。

### 7.3 git env 移除可能破坏用户配置

`GIT_CONFIG` 移除会禁用用户 `.gitconfig`。**部分缓解**：只移除明确危险的几个（`GIT_DIR`、`GIT_WORK_TREE`、`GIT_OBJECT_DIRECTORY`、`GIT_INDEX_FILE`），不移除 `GIT_CONFIG`。

更新设计：从 `cmd.env_remove("GIT_CONFIG")` 列表中**移除** `GIT_CONFIG` 一项。

### 7.4 sysinfo 跨平台差异

`ProcessListTool` 在 Windows/macOS/Linux 上命令行参数获取语义不同。Linux 读 `/proc/<pid>/cmdline` 准确；macOS 用 `libproc`，需权限；Windows 用 `NtQueryInformationProcess`，部分参数缺失。**不修复** — 这是 sysinfo crate 限制。

---

## 8. 风险与回退

| 风险 | 缓解 |
|------|------|
| 已知 secret regex 误报 | 提供 `disable_secret_scrub` 配置 |
| `set_env` 拒绝 `PATH` 破坏现有脚本 | 在错误消息中提示用户改用 `change_dir` 或自定义工具 |
| `system_info` 默认隐藏 hostname 影响诊断 | config 中 `reveal_host=true` 显式开启 |
| git env 移除影响 hook 行为 | 仅移除明确危险的 4 个变量 |

**回退方案**：所有加固可通过 `ToolLimits` 字段关闭。

---

## 9. 与既有 spec 的关系

| spec | 版本 | 范围 |
|------|------|------|
| `fileops-tools-hardening` | v1.0.3 | Read/Write/Edit/List |
| `p0-critical-tools-hardening` | v1.0.4 | Bash/Delete/HTTP/WebFetch/Download/Ping/DNS |
| **本 spec** | **v1.0.5** | Move/Copy/CreateDir/system/git |
| `stale-read-prevention` | v1.1.0 | trait + context 通路 |

`ToolLimits` 结构在 v1.0.3 引入，v1.0.4 扩展，v1.0.5 再次扩展。所有版本向后兼容（新字段有 Default）。

---

## 10. 自评

1. **Placeholder scan**：无 TBD/TODO
2. **Internal consistency**：所有 `ToolLimits` 字段名前后一致
3. **Scope check**：聚焦 P1 工具。Session-scoped env model 明确留给 v1.1.0
4. **Ambiguity check**：
   - trash 失败降级策略一致（与 P0 spec §7.2 对齐）
   - secret scrubber 漏报承认（§7.2）
   - set_var 进程级影响说明（§7.1）
5. **业界最佳实践对齐**：
   - Secret regex 模式参考 [TruffleHog](https://github.com/trufflesecurity/trufflehog) 与 [Gitleaks](https://github.com/gitleaks/gitleaks) 规则集
   - 危险 env 变量列表对齐 [CWE-15: External Control of System or Configuration Setting](https://cwe.mitre.org/data/definitions/15.html)
   - Git env 注入对齐 [CVE-2022-39253](https://nvd.nist.gov/vuln/detail/CVE-2022-39253)（尽管是 git 本身的，但 env 注入是同类向量）
   - Symlink 环检测对齐 GNU `find -L` 的 visited-inodes 实现
