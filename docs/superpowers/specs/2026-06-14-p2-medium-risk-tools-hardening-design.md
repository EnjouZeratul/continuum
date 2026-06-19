# P2 Medium-Risk Tools Hardening Design

**Date:** 2026-06-14
**Status:** Draft v1 — pending review
**Affects:** `sh-layer3` — `GrepTool`、`GlobTool`、`SaveMemoryTool`、`QueryMemoryTool`、`ClearMemoryTool`、`CreateCheckpointTool`、`RestoreCheckpointTool`、`ListCheckpointsTool`、`WebSearchTool`
**Target version:** v1.0.6 patch
**Author:** Continuum Team
**Companion specs:**
- [`2026-06-14-fileops-tools-hardening-design.md`](./2026-06-14-fileops-tools-hardening-design.md)（v1.0.3）
- [`2026-06-14-p0-critical-tools-hardening-design.md`](./2026-06-14-p0-critical-tools-hardening-design.md)（v1.0.4）
- [`2026-06-14-p1-high-risk-tools-hardening-design.md`](./2026-06-14-p1-high-risk-tools-hardening-design.md)（v1.0.5）
- [`2026-06-14-stale-read-prevention-design.md`](./2026-06-14-stale-read-prevention-design.md)（v1.1.0）

---

## 1. 问题陈述

### 1.1 调研范围

P0/P1 spec 覆盖 Bash/Delete/Network/Move/Copy/Mkdir/system/git（共 28 个工具）。本 spec 处理剩余 9 个中危工具。

### 1.2 P2 工具事实性缺陷

#### `GrepTool`（`search.rs:14-198`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| GR1 | 路径不规范化 | `:155-159` | grep `/etc/passwd`、`/root/.ssh/id_rsa` |
| GR2 | `walk_dir` 无 symlink 环检测 | `:49-86` | 环 symlink → 无限递归 / 栈溢出 |
| GR3 | 无文件大小预检 | `:24` | `fs::File::open` 后 `lines()` 流式但 `pattern.is_match` 缓存整行 |
| GR4 | 同步 `reader.lines()` 阻塞 async | `:25-36` | 大文件读阻塞 tokio worker |
| GR5 | **无 ReDoS 防护** | `:148-153` | `pattern="(a+)+$"` 在 `aaaaaaaaaaaaaaaaaaaaab` 上指数级回溯 → CPU 100% |
| GR6 | 整行直接进 output | `:34, 168, 182` | minified JS 单行 10MB → 单条匹配撑爆 context |
| GR7 | 无二进制检测 | 全局 | `grep "a" /bin/ls` 输出乱码 |
| GR8 | 无文件数上限 | `:173-189` | grep `/` 探索千万级文件 |
| GR9 | 无总输出大小上限 | `:195` | 输出 join 后可能 100MB+ |

#### `GlobTool`（`search.rs:201-336`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| GL1 | 同 GR2（symlink 环） | `:236-260` | 同上 |
| GL2 | 路径不规范化 | `:316-325` | 越狱 |
| GL3 | 无文件数上限 | `:233-277` | glob `**/*` 在 `node_modules` 返回百万文件 |
| GL4 | Pattern matching 朴素 | `:205-226` | `**/foo/bar` 等复杂模式不工作 |
| GL5 | mtime 排序读所有文件 metadata | `:264-274` | 网络文件系统上极慢 |

#### `SaveMemoryTool`（`memory_tools.rs:13-111`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| SM1 | 无 content 大小上限 | `:74-76, 95-104` | Agent save 10MB 字符串进 memory |
| SM2 | 无 metadata 大小上限 | `:88-92` | metadata 字段塞 100MB JSON |
| SM3 | **无 secret scrubbing** | `:97` | Agent 把 API key 存进 memory → 持久化泄露 |
| SM4 | 无 per-tier 计数上限 | `:107` | `long_term` tier 无限增长 |

#### `QueryMemoryTool`（`memory_tools.rs:113-215`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| QM1 | **`&e.content[..200]` UTF-8 边界 panic** | `:204-207` | 多字节字符切分 → 进程崩溃 |
| QM2 | `query` 无长度上限 | `:174-176` | 10MB query 字符串 |
| QM3 | `limit` 无上限 | `:178` | `limit=u64::MAX` 返回全部 memory |
| QM4 | 输出无 secret scrubbing | `:200-211` | 查询结果中含 secret 直接进 context |

#### `ClearMemoryTool`（`memory_tools.rs:217-277`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| CM1 | **`clear()` 忽略 tier 参数** | `:270-273` | 调用者传 `tier=working` 但实际清空整个 store — **bug**，与 schema 描述不符 |
| CM2 | 无确认步骤 | `:273` | 误调用清空所有 memory |
| CM3 | 无 audit log | — | 没记录清空了什么 |

#### `CreateCheckpointTool`（`workflow_tools.rs:26-147`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| WC1 | `default_checkpoint_path()` 用 `temp_dir()` | `:18-20` | 预测路径 `/tmp/continuum_checkpoints` — symlink 攻击（攻击者预先创建 symlink 指向 `/etc/cron.d/`） |
| WC2 | `with_path` 无验证 | `:39-43` | 用户 / Agent 可指定任意路径 |
| WC3 | 无 checkpoint 大小上限 | `:140` | messages 含大量历史 → 单 checkpoint 100MB |
| WC4 | `messages` 数组无大小上限 | `:112` | Agent 可塞 10GB messages 数组 |
| WC5 | 无 session_id 验证 | `:103-105` | 任意字符串作为 session_id |

#### `RestoreCheckpointTool`（`workflow_tools.rs:153-244`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| WR1 | 同 WC1/WC2（路径问题） | `:159-170` | 同上 |
| WR2 | 无 session_id 归属验证 | `:210-223` | Agent A 能恢复 Agent B 的 checkpoint（数据串扰） |

#### `ListCheckpointsTool`（`workflow_tools.rs:250-330`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| WL1 | 无分页 | `:309-326` | 列出 10000 checkpoints 全部返回 |
| WL2 | 同 WC1/WC2 | `:255-265` | 同上 |

#### `WebSearchTool`（`web_search.rs`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| WS1 | `query` 无长度上限 | `:569-571` | 10MB query 字符串 |
| WS2 | 搜索结果无大小上限 | `:599-607` | DuckDuckGo 返回大 response 全量塞进 context |
| WS3 | API key 在 `SearchEngineConfig` derive `Debug` | `:33` | `tracing::debug!` 打印 config 时泄露 |
| WS4 | RateLimiter per-instance | `:106-119` | 新建工具实例即绕过限速 |
| WS5 | 无 secret scrubbing on results | `:582-608` | snippet 中含 secret 直接进 context |

### 1.3 实际风险场景

- **场景 W（grep ReDoS）**：LLM 自主生成 regex `(a+)+b` 然后调用 `grep` 在大型文件上 — CPU 钉死，整会话挂起
- **场景 X（memory 持久泄露）**：Agent 把 `OPENAI_API_KEY=sk-...` save 到 `long_term` memory — 跨 session 持久化泄露
- **场景 Y（checkpoint 路径攻击）**：`create_checkpoint(session_id="../../etc/cron.d/payload")` — 路径遍历，cron 注入
- **场景 Z（temp symlink 攻击）**：本地攻击者预先 `ln -s /etc/passwd /tmp/continuum_checkpoints` — Agent 调用 `restore_checkpoint` 触发覆盖
- **场景 AA（glob 万文件爆破）**：`glob("**/*", "/usr")` 返回 50 万 entry → 单次响应撑爆 context

### 1.4 范围界定（YAGNI）

**本方案处理**：

- 输入/输出大小边界（v1.0.6 patch）
- 路径规范化 + 关键路径检查（复用 P0/P1）
- UTF-8 安全切片（复用 P0 `safe_truncate_bytes`）
- Symlink 环检测（复用 P1 §3.3 `visited_inodes`）
- ReDoS 防护（regex 复杂度检查 + 总匹配次数上限）
- Secret scrubbing 复用 P1 §3.5

**本方案不处理**：

- 完整全文索引（不属于 grep 工具职责）
- Memory 系统的 vector search（v1.1.0+）
- Checkpoint 加密（v1.1.0+）

---

## 2. 设计目标

### 2.1 P2 必须达成（G1-G6）

1. **G1**：搜索工具有 ReDoS 防护（regex complexity + 总匹配数 + 文件数上限）
2. **G2**：所有搜索/列出的结果有大小上限
3. **G3**：Memory 工具集成 `SecretScrubber`，存储与查询时双向脱敏
4. **G4**：Checkpoint 工具有路径安全 + 大小上限 + session_id 验证
5. **G5**：`ClearMemoryTool` 修复 CM1 bug（按 tier 清空）
6. **G6**：`WebSearchTool` rate limiter 全局化、API key 不进 Debug

---

## 3. 设计

### 3.1 `GrepTool` 加固

```rust
pub struct GrepTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for GrepTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let pattern_str = args["pattern"].as_str()
            .ok_or_else(|| anyhow!("Missing pattern"))?;

        // === GR5: ReDoS 防护 ===
        validate_regex_safety(pattern_str, &self.limits)?;

        // ... rest of args ...

        // === GR1: 路径检查 ===
        let path = args["path"].as_str().unwrap_or(".");
        let canonical = validate_file_path(path, &self.limits, true).await?;

        // Build regex
        let pattern = if case_sensitive {
            Regex::new(pattern_str)?
        } else {
            Regex::new(&format!("(?i){}", pattern_str))?
        };

        let max_results = args["max_results"].as_u64()
            .unwrap_or(100)
            .min(self.limits.max_grep_results as u64);

        let mut output_lines = Vec::new();
        let mut total_matches = 0;
        let mut total_output_bytes = 0;
        let mut files_scanned = 0;

        if canonical.is_file() {
            // === GR3, GR7: 文件大小 + 二进制检测 ===
            self.search_file_safe(&canonical, &pattern, max_results,
                                  &mut output_lines, &mut total_matches,
                                  &mut total_output_bytes, &self.limits).await?;
        } else if canonical.is_dir() {
            // === GR2, GR8: symlink 环检测 + 文件数上限 ===
            let mut visited: HashSet<(u64, u64)> = HashSet::new();
            let mut stack = vec![canonical.clone()];
            while let Some(dir) = stack.pop() {
                if files_scanned >= self.limits.max_grep_files {
                    break;
                }
                let mut entries = tokio::fs::read_dir(&dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    if files_scanned >= self.limits.max_grep_files { break; }
                    let path = entry.path();
                    let ftype = entry.file_type().await?;

                    if ftype.is_symlink() {
                        continue;  // 不跟随 symlink（与 GlobTool 一致）
                    }
                    if ftype.is_dir() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            if let Ok(m) = entry.metadata().await {
                                if !visited.insert((m.ino(), m.dev())) {
                                    continue;  // 环检测
                                }
                            }
                        }
                        stack.push(path);
                    } else if ftype.is_file() {
                        files_scanned += 1;
                        self.search_file_safe(&path, &pattern, max_results - total_matches,
                                              &mut output_lines, &mut total_matches,
                                              &mut total_output_bytes, &self.limits).await?;
                    }
                }
            }
        }

        // === GR9: 总输出上限 ===
        if output_lines.is_empty() {
            Ok("(no matches)".to_string())
        } else {
            Ok(safe_truncate_bytes(
                &output_lines.join("\n"),
                self.limits.max_grep_output_bytes as usize,
            ).to_string())
        }
    }
}

/// ReDoS 防护：检测灾难性回溯模式
fn validate_regex_safety(pattern: &str, limits: &ToolLimits) -> Layer3Result<()> {
    if pattern.len() > limits.max_regex_pattern_chars {
        return Err(anyhow!(
            "grep rejected: pattern too long ({} > {} chars)",
            pattern.len(), limits.max_regex_pattern_chars,
        ));
    }

    // 检测已知灾难性模式
    const REDOS_PATTERNS: &[&str] = &[
        r"(\w+\+)+",       // 嵌套量词
        r"(\w+\*)+",       // 嵌套量词
        r"(.+\+)+",        // 任意 + 嵌套
        r"(\w+\+\+)+",     // 双重 + 量词
        r"(\(.+\))+\+",    // 分组嵌套 + 量词
    ];
    for redos in REDOS_PATTERNS {
        if let Ok(re) = Regex::new(redos) {
            if re.is_match(pattern) {
                return Err(anyhow!(
                    "grep rejected: pattern contains potential ReDoS vector (matches '{}'). \
                     Rewrite without nested quantifiers.",
                    redos,
                ));
            }
        }
    }

    Ok(())
}

async fn search_file_safe(
    &self,
    path: &Path,
    pattern: &Regex,
    max_results: usize,
    output_lines: &mut Vec<String>,
    total_matches: &mut usize,
    total_output_bytes: &mut usize,
    limits: &ToolLimits,
) -> Layer3Result<()> {
    let meta = tokio::fs::metadata(path).await?;
    if meta.len() > limits.max_grep_file_bytes {
        return Ok(());  // 跳过大文件
    }

    // 二进制检测：读前 8192 字节 sniff NUL
    let mut file = tokio::fs::File::open(path).await?;
    use tokio::io::AsyncReadExt;
    let mut sniff = [0u8; 8192];
    let n = file.read(&mut sniff).await?;
    if sniff[..n].contains(&0u8) {
        return Ok(());  // 跳过二进制
    }

    // 重置到文件开头，流式读取
    use tokio::io::{AsyncBufReadExt, AsyncSeekExt, SeekFrom};
    file.seek(SeekFrom::Start(0)).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();

    let mut line_num = 0;
    while let Some(line_result) = lines.next_line().await? {
        line_num += 1;
        if *total_matches >= max_results { break; }
        if *total_output_bytes >= limits.max_grep_output_bytes as usize { break; }

        if pattern.is_match(&line_result) {
            // === GR6: 单行截断 ===
            let display_line = safe_truncate_bytes(&line_result, limits.max_grep_line_chars);
            let formatted = format!("{}:{}: {}", path.display(), line_num, display_line);
            *total_output_bytes += formatted.len();
            output_lines.push(formatted);
            *total_matches += 1;
        }
    }
    Ok(())
}
```

### 3.2 `GlobTool` 加固

```rust
pub struct GlobTool {
    limits: Arc<ToolLimits>,
}

async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let pattern = args["pattern"].as_str()
        .ok_or_else(|| anyhow!("Missing pattern"))?;
    let path_str = args["path"].as_str().unwrap_or(".");

    // === GL2: 路径检查 ===
    let canonical = validate_file_path(path_str, &self.limits, true).await?;
    if !canonical.is_dir() {
        return Err(anyhow!("Not a directory: {}", canonical.display()));
    }

    let mut files = Vec::new();
    let mut visited: HashSet<(u64, u64)> = HashSet::new();
    let mut stack = vec![canonical.clone()];

    while let Some(dir) = stack.pop() {
        if files.len() >= self.limits.max_glob_files { break; }

        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if files.len() >= self.limits.max_glob_files { break; }

            let path = entry.path();
            let ftype = entry.file_type().await?;

            if ftype.is_symlink() {
                continue;  // 不跟随
            }
            if ftype.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }  // 跳过 hidden
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(m) = entry.metadata().await {
                        if !visited.insert((m.ino(), m.dev())) { continue; }
                    }
                }
                stack.push(path);
            } else if ftype.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if matches_glob(&name, pattern) {
                    files.push(path);
                }
            }
        }
    }

    if files.is_empty() {
        return Ok("(no matches)".to_string());
    }

    // === GL3: 上限提示 ===
    let total = files.len();
    let truncated = total > self.limits.max_glob_files as usize;
    files.truncate(self.limits.max_glob_files as usize);

    // === GL5: lazy mtime sort（仅对返回的子集排序）===
    files.sort_by(|a, b| {
        let am = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        let bm = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        bm.cmp(&am)
    });

    let mut result = files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
    if truncated {
        result.push_str(&format!("\n... (truncated, {} total matches)", total));
    }
    Ok(result)
}

/// 标准 glob 匹配（替代朴素实现 GL4）
fn matches_glob(name: &str, pattern: &str) -> bool {
    // 使用 glob crate（workspace 已依赖 `glob = "0.3"`，见 Cargo.toml:85）
    // 简化：直接用 Pattern::matches
    match glob::Pattern::new(pattern) {
        Ok(p) => p.matches(name),
        Err(_) => false,
    }
}
```

### 3.3 `MemoryTools` 加固

```rust
pub struct SaveMemoryTool {
    store: Arc<WorkingMemory>,
    limits: Arc<ToolLimits>,
    scrubber: Arc<SecretScrubber>,
}

#[async_trait]
impl BuiltinTool for SaveMemoryTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let content = args["content"].as_str()
            .ok_or_else(|| anyhow!("Missing content"))?;

        // === SM1: content 大小上限 ===
        if content.len() > self.limits.max_memory_content_bytes as usize {
            return Err(anyhow!(
                "save_memory rejected: content {} bytes > limit {}",
                content.len(), self.limits.max_memory_content_bytes,
            ));
        }

        // === SM3: secret scrubbing ===
        let safe_content = self.scrubber.scrub(content);
        if let Some(kind) = self.scrubber.contains_secret(content) {
            tracing::warn!(
                target: "continuum.tools.memory",
                memory.secret_detected = kind,
                "save_memory: scrubbed secret of kind '{}' before storing",
                kind,
            );
        }

        // === SM2: metadata 大小上限 ===
        let metadata = if let Some(obj) = args["metadata"].as_object() {
            let serialized = serde_json::to_string(obj)?;
            if serialized.len() > self.limits.max_memory_metadata_bytes as usize {
                return Err(anyhow!(
                    "save_memory rejected: metadata {} bytes > limit {}",
                    serialized.len(), self.limits.max_memory_metadata_bytes,
                ));
            }
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        let tier_str = args["tier"].as_str().unwrap_or("working");
        let tier = parse_tier(tier_str);

        // === SM4: per-tier count cap ===
        let current_count = self.store.count_by_tier(tier).await?;
        if current_count >= self.limits.max_memories_per_tier {
            return Err(anyhow!(
                "save_memory rejected: tier '{}' has {} entries (limit {}). \
                 Clear or move to other tier.",
                tier_str, current_count, self.limits.max_memories_per_tier,
            ));
        }

        let entry = MemoryEntry {
            id: generate_short_id(),
            content: safe_content,
            tier,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            importance: 0.5,
            metadata,
            access_count: 0,
        };

        let id = self.store.store(entry).await?;
        Ok(format!("Memory saved to {} tier with ID: {}", tier_str, id))
    }
}
```

`QueryMemoryTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let query_text = args["query"].as_str()
        .ok_or_else(|| anyhow!("Missing query"))?;

    // === QM2: query 长度上限 ===
    if query_text.len() > self.limits.max_memory_query_chars {
        return Err(anyhow!(
            "query_memory rejected: query too long ({} > {})",
            query_text.len(), self.limits.max_memory_query_chars,
        ));
    }

    // === QM3: limit 上限 ===
    let limit = args["limit"].as_u64()
        .unwrap_or(10)
        .min(self.limits.max_memory_query_results as u64) as usize;

    /* ... query logic ... */

    let output: Vec<String> = results
        .iter()
        .take(limit)
        .map(|e| {
            // === QM1: UTF-8 安全截断 ===
            let preview = safe_truncate_chars(&e.content, 200);
            // === QM4: 输出 secret scrubbing ===
            let preview = self.scrubber.scrub(&preview);
            format!("{}: {}", e.id, preview)
        })
        .collect();
    Ok(output.join("\n"))
}
```

`ClearMemoryTool` — **修复 CM1 bug**：

```rust
pub struct ClearMemoryTool {
    store: Arc<WorkingMemory>,
}

async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let tier_str = args["tier"].as_str().unwrap_or("working");
    let tier = parse_tier(tier_str);
    let confirm = args["confirm"].as_bool().unwrap_or(false);

    // === CM2: 确认步骤 ===
    if !confirm {
        let count = self.store.count_by_tier(tier).await?;
        return Ok(format!(
            "Clear {} tier requires confirm=true. Current count: {}.",
            tier_str, count,
        ));
    }

    // === CM1 fix: 实际按 tier 清空 ===
    let count = self.store.clear_tier(tier).await?;

    // === CM3: audit log ===
    tracing::info!(
        target: "continuum.tools.memory",
        memory.tier = tier_str,
        memory.cleared_count = count,
        "clear_memory: cleared {} entries from {} tier",
        count, tier_str,
    );

    Ok(format!("Cleared {} memories from {} tier", count, tier_str))
}
```

**`WorkingMemory` 新增 API**：`count_by_tier(tier) -> usize` 和 `clear_tier(tier) -> usize`。

### 3.4 `WorkflowTools` 加固

```rust
pub struct CreateCheckpointTool {
    writer: Arc<CheckpointWriter>,
    limits: Arc<ToolLimits>,
}

impl CreateCheckpointTool {
    pub fn new() -> Self {
        // === WC1: 不用 temp_dir，用项目数据目录 ===
        let path = dirs::data_local_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("continuum")
            .join("checkpoints");
        Self {
            writer: Arc::new(CheckpointWriter::new(path)),
            limits: Arc::new(ToolLimits::default()),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        // === WC2: 路径验证移到 execute 时检查 ===
        Self {
            writer: Arc::new(CheckpointWriter::new(path)),
            limits: Arc::new(ToolLimits::default()),
        }
    }
}

#[async_trait]
impl BuiltinTool for CreateCheckpointTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let session_id_str = args["session_id"].as_str()
            .ok_or_else(|| anyhow!("Missing session_id"))?;

        // === WC5: session_id 验证 ===
        if !is_valid_session_id(session_id_str) {
            return Err(anyhow!(
                "create_checkpoint rejected: session_id '{}' contains invalid characters",
                session_id_str,
            ));
        }

        // === WC2: 路径验证 ===
        let writer_path = self.writer.path();
        validate_file_path(&writer_path.to_string_lossy(), &self.limits, false).await?;

        // === WC4: messages 大小上限 ===
        let messages = args["messages"].as_array().cloned().unwrap_or_default();
        let messages_size = serde_json::to_string(&messages)?.len();
        if messages_size > self.limits.max_checkpoint_messages_bytes as usize {
            return Err(anyhow!(
                "create_checkpoint rejected: messages {} bytes > limit {}",
                messages_size, self.limits.max_checkpoint_messages_bytes,
            ));
        }

        // ... build checkpoint_data ...

        let checkpoint_id = self.writer.save(&checkpoint_data).await?;

        // === WC3: checkpoint 大小记录（用于后续诊断）===
        let size = self.writer.size_of(&checkpoint_id).await?;
        tracing::info!(
            target: "continuum.tools.workflow",
            checkpoint.size_bytes = size,
            checkpoint.session_id = %session_id_str,
            "create_checkpoint: saved {} bytes",
            size,
        );

        Ok(format!("Checkpoint created: {} ({} bytes)", checkpoint_id, size))
    }
}

fn is_valid_session_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !s.contains("..")  // 防路径遍历
}
```

`RestoreCheckpointTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let session_id_str = args["session_id"].as_str()
        .ok_or_else(|| anyhow!("Missing session_id"))?;

    if !is_valid_session_id(session_id_str) {
        return Err(anyhow!("Invalid session_id"));
    }

    // === WR2: session_id 归属验证（v1.0.6 限制）===
    // 由于工具目前是 sessionless 的（v1.1.0 才有 context），无法做严格归属检查
    // 暂时只验证 session_id 格式，v1.1.0 引入 ExecutionContext 后再做严格检查
    // 此时记录 warning 让用户知晓
    tracing::warn!(
        target: "continuum.tools.workflow",
        checkpoint.session_id = %session_id_str,
        "restore_checkpoint: session ownership not verified (requires v1.1.0 ExecutionContext)",
    );

    // ... 其余逻辑 ...
}
```

### 3.5 `WebSearchTool` 加固

```rust
// === WS3: API key 不 derive Debug ===
pub struct SearchEngineConfig {
    pub engine: SearchEngine,
    #[debug(skip)]  // 使用 derivative crate 或手动 impl
    pub api_key: Option<String>,
    pub cx: Option<String>,
    pub max_results: usize,
    pub timeout_secs: u64,
    pub enable_cache: bool,
    pub cache_ttl_secs: u64,
}

impl std::fmt::Debug for SearchEngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchEngineConfig")
            .field("engine", &self.engine)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("cx", &self.cx)
            .field("max_results", &self.max_results)
            .field("timeout_secs", &self.timeout_secs)
            .field("enable_cache", &self.enable_cache)
            .field("cache_ttl_secs", &self.cache_ttl_secs)
            .finish()
    }
}

// === WS4: 全局 rate limiter（替换 per-instance）===
use once_cell::sync::Lazy;
static GLOBAL_SEARCH_RATE_LIMITER: Lazy<RwLock<Option<Instant>>> = Lazy::new(|| RwLock::new(None));

async fn search(&self, query: &str) -> Layer3Result<SearchResponse> {
    // === WS1: query 长度上限 ===
    if query.len() > self.limits.max_search_query_chars {
        return Err(anyhow!("query too long"));
    }

    // === WS4: 全局 rate limit ===
    {
        let last = GLOBAL_SEARCH_RATE_LIMITER.read().clone();
        if let Some(t) = last {
            let elapsed = t.elapsed();
            if elapsed < self.config.min_interval {
                tokio::time::sleep(self.config.min_interval - elapsed).await;
            }
        }
    }
    *GLOBAL_SEARCH_RATE_LIMITER.write() = Some(Instant::now());

    // ... 执行搜索 ...

    // === WS2, WS5: 结果大小上限 + scrubbing ===
    let limited_results: Vec<_> = response.results
        .into_iter()
        .take(self.config.max_results)
        .map(|r| SearchResult {
            title: self.scrubber.scrub(&r.title),
            snippet: self.scrubber.scrub(&safe_truncate_chars(&r.snippet, 500)),
            ..r
        })
        .collect();
    /* ... */
}
```

---

## 4. 接口契约

### 4.1 schema 变更

- `clear_memory` 新增 `confirm: boolean`（必填，default false）
- 其余工具 schema 不变（行为变化）

### 4.2 兼容性矩阵

| 行为 | v1.0.x | v1.0.6 | 影响 |
|------|--------|--------|------|
| `save_memory` 存含 secret 内容 | 完整存储 | 自动 scrub | 安全升级 |
| `query_memory` 输出含 secret | 直接输出 | scrub 后输出 | 安全升级 |
| `clear_memory(tier=working)` | 清空所有 tier | 仅清空 working | **修复 CM1 bug** |
| `grep` ReDoS 模式 | 执行 | 拒绝 | 安全升级 |
| `glob("**/*", "/usr")` | 返回所有 | 上限 + 提示截断 | 输出变短 |
| `create_checkpoint(path="/etc/cron.d/x")` | 创建 | 拒绝 | 安全升级 |
| `WebSearchTool` Debug 输出 | 含 api_key | `<redacted>` | 安全升级 |

---

## 5. 实现计划（TDD）

### Phase A：GrepTool + GlobTool

- [ ] A1: 写 `validate_regex_safety` + 单元测试（ReDoS pattern）
- [ ] A2: 重构 `GrepTool` 为 async + symlink 环检测
- [ ] A3: 实现 `search_file_safe`（二进制检测、大小预检、行截断）
- [ ] A4: 重构 `GlobTool` 用 `glob::Pattern` 替换朴素匹配
- [ ] A5: 实现 max_glob_files + 截断提示
- [ ] A6: 全量测试

### Phase B：MemoryTools

- [ ] B1: 给 3 个 memory 工具注入 `Arc<SecretScrubber>` 与 `Arc<ToolLimits>`
- [ ] B2: 实现 SM1-SM4
- [ ] B3: 实现 QM1-QM4
- [ ] B4: **修复 CM1**：给 `WorkingMemory` 加 `count_by_tier` / `clear_tier` 方法
- [ ] B5: 实现 CM2 确认步骤 + CM3 audit log
- [ ] B6: 全量测试

### Phase C：WorkflowTools

- [ ] C1: `default_checkpoint_path` 改用 `data_local_dir`
- [ ] C2: `with_path` + execute 时路径验证
- [ ] C3: 实现 WC3/4/5
- [ ] C4: 给 `RestoreCheckpointTool` 加 WR2 ownership warning
- [ ] C5: `ListCheckpointsTool` 分页
- [ ] C6: 全量测试

### Phase D：WebSearchTool

- [ ] D1: 手动 `impl Debug` for `SearchEngineConfig` 跳过 api_key
- [ ] D2: 全局 rate limiter（用 `Lazy<RwLock<Option<Instant>>>`）
- [ ] D3: query 大小上限 + 结果 scrubbing
- [ ] D4: 全量测试

### Phase E：集成

- [ ] E1: 全量测试
- [ ] E2: clippy + fmt
- [ ] E3: CHANGELOG + publish v1.0.6

---

## 6. 测试矩阵

### 6.1 GrepTool（10 个新测试）

- `test_grep_rejects_redos_pattern`（GR5）
- `test_grep_rejects_oversized_pattern`
- `test_grep_rejects_binary_file`（GR7）
- `test_grep_skips_large_file`（GR3）
- `test_grep_truncates_long_line`（GR6）
- `test_grep_caps_total_output`（GR9）
- `test_grep_caps_file_count`（GR8）
- `test_grep_detects_symlink_loop`（GR2）
- `test_grep_rejects_critical_path`（GR1）
- `test_grep_async_does_not_block`（GR4）

### 6.2 GlobTool（5 个新测试）

- `test_glob_caps_files_returned`（GL3）
- `test_glob_detects_symlink_loop`（GL1）
- `test_glob_complex_pattern`（GL4）
- `test_glob_truncated_message`
- `test_glob_rejects_critical_path`（GL2）

### 6.3 MemoryTools（10 个新测试）

- `test_save_memory_rejects_oversize_content`
- `test_save_memory_scrubs_secret`
- `test_save_memory_caps_metadata`
- `test_save_memory_caps_per_tier_count`
- `test_query_memory_truncates_utf8_safe`（QM1）
- `test_query_memory_caps_limit`
- `test_query_memory_scrubs_output`（QM4）
- `test_clear_memory_requires_confirm`
- `test_clear_memory_only_clears_specified_tier`（**CM1 fix 关键测试**）
- `test_clear_memory_logs_audit`

### 6.4 WorkflowTools（8 个新测试）

- `test_checkpoint_rejects_invalid_session_id`
- `test_checkpoint_rejects_critical_path`
- `test_checkpoint_rejects_oversize_messages`
- `test_checkpoint_default_path_is_data_dir_not_temp`
- `test_restore_warns_about_ownership`
- `test_list_checkpoints_paginates`
- `test_checkpoint_path_traversal_rejected`
- `test_checkpoint_symlink_attack_rejected`

### 6.5 WebSearchTool（4 个新测试）

- `test_search_engine_config_debug_redacts_api_key`
- `test_search_query_length_limit`
- `test_search_results_are_scrubbed`
- `test_global_rate_limiter_blocks_rapid_calls`

---

## 7. 已知局限

### 7.1 ReDoS 检测不完整

`validate_regex_safety` 用已知 pattern 列表 + 字符串特征匹配。**真正完整的检测**需要 regex AST 分析（参考 [safe-regex](https://crates.io/crates/safe-regex) 或 [regex-dfa-size-limit]）。

**v1.0.6 缓解**：pattern 列表 + 总匹配次数上限（用 `Regex::find_iter().count() <= N`）。

**v1.1.0+ 完整方案**：切换到 `regex_dfa_size_limit` 或自实现 AST 分析。

### 7.2 Memory ownership 模型

`Memory` / `Checkpoint` 当前是全局存储（无 session 边界）。多 session 场景下互相可见。**v1.1.0 引入 ExecutionContext** 后，所有 memory/checkpoint 操作可绑定 session_id 做严格隔离。

**v1.0.6 缓解**：路径验证 + 大小上限 + secret scrubbing。隔离留 v1.1.0。

### 7.3 WebSearch 跨实例 rate limit 不严格

`Lazy<RwLock<Option<Instant>>>` 是进程内全局，但**跨进程**（多 continuum 实例）无法共享。若用户在容器内运行多实例，rate limit 失效。

**缓解**：DuckDuckGo 引擎对单 IP 自身有限速；不严重。

### 7.4 GlobTool mtime sort 仍读 metadata

虽然改成只对返回子集排序，但在某些 FS（NFS / SSHFS）上仍慢。**完整方案**是 `tokio::task::spawn_blocking` 隔离 metadata 调用。**v1.0.6 不做** — `glob` 默认场景在本地 FS 已足够快。

---

## 8. 风险与回退

| 风险 | 缓解 |
|------|------|
| ReDoS pattern 列表误报合法 regex | 错误消息提示用户简化 pattern |
| Memory scrubber 误报破坏内容 | 提供 `disable_scrubber` 配置 |
| Checkpoint 路径变化破坏旧数据 | migration 工具：自动从 `temp_dir` 迁移到 `data_local_dir` |
| `clear_memory` 必须传 `confirm=true` 破坏现有 Agent 流程 | 在 release notes 显式说明，提供兼容 flag |

**回退方案**：所有新行为通过 `ToolLimits` 字段关闭。

---

## 9. 与既有 spec 的关系

| spec | 版本 | 范围 |
|------|------|------|
| `fileops-tools-hardening` | v1.0.3 | Read/Write/Edit/List |
| `p0-critical-tools-hardening` | v1.0.4 | Bash/delete/HTTP/... |
| `p1-high-risk-tools-hardening` | v1.0.5 | Move/Copy/Mkdir/system/git |
| **本 spec** | **v1.0.6** | Search/Memory/Workflow/WebSearch |
| `stale-read-prevention` | v1.1.0 | trait + context 通路 |

---

## 10. 自评

1. **Placeholder scan**：无 TBD/TODO
2. **Internal consistency**：
   - 所有 `ToolLimits` 字段名前后一致
   - `SecretScrubber` 在 P1/P2 都使用，引用一致
   - `validate_file_path` 在 P0/P1/P2 都使用
3. **Scope check**：聚焦 9 个中危工具
4. **Ambiguity check**：
   - CM1 fix 是否破坏数据？— 否，仅修复 bug，旧 bug 清空所有 tier 反而是数据丢失风险
   - ReDoS 检测策略明确（pattern 列表 + 总匹配数）
   - WebSearch 全局 rate limit 单进程有效，跨进程不严格（已承认 §7.3）
5. **业界最佳实践对齐**：
   - ReDoS 检测对齐 OWASP [Regular Expression Denial of Service](https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS)
   - Symlink 环检测对齐 GNU `find -L` 实现
   - API key 不进 Debug 对齐 [CWE-532: Insertion of Sensitive Information into Log File](https://cwe.mitre.org/data/definitions/532.html)
   - Checkpoint 路径不用 temp 对齐 [CWE-377: Insecure Temporary File](https://cwe.mitre.org/data/definitions/377.html)
   - Path traversal 防护对齐 [CWE-22: Improper Limitation of a Pathname](https://cwe.mitre.org/data/definitions/22.html)
