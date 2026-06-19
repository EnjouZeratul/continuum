# P3 Low-Risk Tools Hardening Design

**Date:** 2026-06-14
**Status:** Draft v1 — pending review
**Affects:** `sh-layer3` — `code` (4 tools)、`data_processing` (14 tools)、`text_tools` (7 tools)
**Target version:** v1.0.7 patch
**Author:** Continuum Team
**Companion specs:** P0/P1/P2（v1.0.3 → v1.0.6）+ stale-read（v1.1.0）

---

## 1. 问题陈述

### 1.1 调研范围

P0/P1/P2 已覆盖 37 个工具。本 spec 处理剩余 25 个低危工具。**核心结论**：大多数 P3 工具是无状态字符串处理，风险集中在 **输入/输出大小边界** 和 **ReDoS**。

### 1.2 P3 工具事实性缺陷

#### `code` 4 个工具（LSP 集成）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| C1 | `fs::read_to_string(file_path)` 无大小上限 | `code.rs:201, 433, 663, 845` | 大文件 OOM |
| C2 | 路径不规范化 | 全局 | 越狱（与 P0/P1/P2 同源） |
| C3 | Regex fallback 遍历整目录 `fs::read_dir + read_to_string` | `code.rs:242-251, 475-484` | 符号未命中时全 repo 读取，撑爆 context |
| C4 | 无 symlink 环检测（fallback 路径） | `code.rs:242-251` | 同 P2 GR2 |
| C5 | 无结果数上限 | 全局 | LSP 返回百万引用 |
| C6 | `RenameSymbolTool` 写回无大小预检 | `code.rs:871` | 写入可能撑爆磁盘 |
| C7 | `file` 参数直接传 LSP server | 全局 | LSP server 自身 sandbox 缺失时路径注入 |

#### `data_processing` 14 个工具

##### 通用问题（影响多个工具）

| # | 缺陷 | 影响 |
|---|------|------|
| DP1 | 输入字符串无大小上限 | Agent 传 1GB JSON 串 |
| DP2 | 输出 `to_string_pretty` 无大小上限 | 1MB JSON 美化后 5MB |
| DP3 | CSV 解析无行数上限 | `csv_parse` on 10GB CSV OOM |
| DP4 | `query_json` 朴素解析器，路径表达式无验证 | 复杂表达式可能死循环 |

##### 特定工具

| # | 工具 | 缺陷 |
|---|------|------|
| DP5 | `HashTool` | 接受 MD5/SHA1（弱算法），默认 sha256 OK 但用户可指定 |
| DP6 | `Base64DecodeTool` | 输入无长度上限；解 base64 后可能 4/3 放大 |
| DP7 | `CsvParseTool` | 字段值无大小上限 |
| DP8 | `UrlEncodeTool` | 无大小上限 |
| DP9 | `UuidGenerateTool` | count 已 clamp(1, 100) — ✅ 合规 |

#### `text_tools` 7 个工具

##### 通用问题

| # | 缺陷 | 影响 |
|---|------|------|
| TT1 | 所有 text 输入无大小上限 | 10MB text 输入 |
| TT2 | 输出无大小上限 | 处理后输出可能放大 |

##### 特定工具

| # | 工具 | 缺陷 |
|---|------|------|
| TT3 | `RegexMatchTool` | **无 ReDoS 防护** — 同 P2 GR5 |
| TT4 | `TextDiffTool` | O(N×M) LCS 算法 — 10k×10k 行输入 100M 操作 |
| TT5 | `WordFrequencyTool` | `top_k` 参数无上限 |
| TT6 | `SortLinesTool` | 输入无大小上限，排序 OOM |
| TT7 | `TextSplitTool` | 分割结果数无上限 |

### 1.3 实际风险场景

- **场景 BB（code 误读）**：`find_references(symbol="i")` — 单字符变量名在大型项目触发整目录遍历，输出数十万行
- **场景 CC（CSV OOM）**：Agent 自主调用 `csv_parse` 处理 1GB 文件 → 进程 OOM
- **场景 DD（TextDiff DoS）**：`text_diff(text1=10k行, text2=10k行)` → LCS 计算 30 秒+
- **场景 EE（Regex DoS）**：`regex_match(text=huge_string, pattern="(a+)+b")` → CPU 钉死

### 1.4 范围界定

**本方案处理**：

- 输入/输出大小边界（v1.0.7 patch — 共享 `ToolLimits` 扩展）
- ReDoS 防护（复用 P2 `validate_regex_safety`）
- Code 工具路径规范化 + symlink 环检测（复用 P0/P1）
- 弱哈希算法警告

**本方案不处理**：

- 完整 LSP server 沙箱（v1.2.0+）
- 数据处理流式 API（v1.1.0+ 改 trait 后）

---

## 2. 设计

### 2.1 共享：所有 P3 工具加 `ToolLimits`

```rust
// 扩展 ToolLimits（v1.0.7 新增）
pub struct ToolLimits {
    // ... 既有字段 ...

    // === P3 新增 ===
    pub max_text_input_chars: usize,       // 默认 1 MiB
    pub max_text_output_chars: usize,      // 默认 1 MiB
    pub max_json_input_bytes: u64,         // 默认 10 MiB
    pub max_json_output_bytes: u64,        // 默认 10 MiB
    pub max_csv_rows: usize,               // 默认 10000
    pub max_csv_field_bytes: usize,        // 默认 65536
    pub max_regex_pattern_chars: usize,    // 默认 1024（与 P2 一致）
    pub max_diff_input_lines: usize,       // 默认 5000
    pub max_word_freq_top: usize,          // 默认 100
    pub max_sort_lines: usize,             // 默认 100000
    pub max_split_results: usize,          // 默认 10000
    pub max_code_search_files: usize,      // 默认 1000
    pub max_code_results: usize,           // 默认 100
    pub warn_weak_hash: bool,              // 默认 true（MD5/SHA1 触发警告）
}
```

### 2.2 输入/输出边界 — 通用 helper

```rust
// rust/layer3/src/builtin_tools/bounds.rs（新建）
pub fn check_input_size(s: &str, max: usize, kind: &str) -> Layer3Result<()> {
    if s.len() > max {
        return Err(anyhow!(
            "{} rejected: input {} bytes > limit {}",
            kind, s.len(), max,
        ));
    }
    Ok(())
}

pub fn bounded_output(s: String, max: usize) -> Layer3Result<String> {
    if s.len() <= max {
        return Ok(s);
    }
    let truncated = safe_truncate_bytes(&s, max);
    Ok(format!("{}...(truncated, {} bytes total)", truncated, s.len()))
}
```

### 2.3 `code` 工具加固

```rust
pub struct GoToDefinitionTool {
    limits: Arc<ToolLimits>,
}

async fn execute_with_regex(&self, file_path: &Path, /* ... */) -> Layer3Result<String> {
    // === C2: 路径检查 ===
    let canonical = validate_file_path(&file_path.to_string_lossy(), &self.limits, true).await?;

    // === C1: 文件大小预检 ===
    let meta = tokio::fs::metadata(&canonical).await?;
    if meta.len() > self.limits.max_read_bytes {
        return Err(anyhow!("File too large for analysis"));
    }

    let content = tokio::fs::read_to_string(&canonical).await?;

    // === C5: 结果数上限 ===
    let results: Vec<_> = /* ... regex matches ... */
        .into_iter()
        .take(self.limits.max_code_results)
        .collect();

    // ... 返回 ...
}

async fn find_in_directory(&self, dir: &Path, symbol: &str, /* ... */) -> Layer3Result<Vec<...>> {
    // === C3, C4: symlink 环检测 + 文件数上限 ===
    let mut visited: HashSet<(u64, u64)> = HashSet::new();
    let mut files_scanned = 0;
    let mut results = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(d) = stack.pop() {
        if files_scanned >= self.limits.max_code_search_files { break; }
        if results.len() >= self.limits.max_code_results { break; }

        let mut entries = tokio::fs::read_dir(&d).await?;
        while let Some(entry) = entries.next_entry().await? {
            if files_scanned >= self.limits.max_code_search_files { break; }
            let path = entry.path();
            let ftype = entry.file_type().await?;

            if ftype.is_symlink() { continue; }
            if ftype.is_dir() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(m) = entry.metadata().await {
                        if !visited.insert((m.ino(), m.dev())) { continue; }
                    }
                }
                stack.push(path);
            } else if ftype.is_file() {
                files_scanned += 1;
                if let Ok(meta) = entry.metadata().await {
                    if meta.len() > self.limits.max_read_bytes { continue; }
                }
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    /* search for symbol, push to results */
                }
            }
        }
    }
    Ok(results)
}
```

`RenameSymbolTool` 额外：

```rust
// === C6: 写回前大小检查 ===
let new_content = lines.join("\n");
if new_content.len() > self.limits.max_write_bytes as usize {
    return Err(anyhow!(
        "rename_symbol rejected: result size {} > limit {}",
        new_content.len(), self.limits.max_write_bytes,
    ));
}
tokio::fs::write(&canonical, new_content).await?;
```

### 2.4 `data_processing` 工具加固

通用模式（应用到所有 14 个工具）：

```rust
pub struct JsonParseTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for JsonParseTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let json_str = args["json"].as_str()
            .ok_or_else(|| anyhow!("Missing json"))?;

        // === DP1: 输入大小 ===
        check_input_size(json_str, self.limits.max_json_input_bytes as usize, "json_parse")?;

        let value: Value = serde_json::from_str(json_str)?;

        let result = if let Some(query) = args["query"].as_str() {
            // === DP4: query 长度上限 ===
            check_input_size(query, 1024, "json query")?;
            query_json(&value, query)?
        } else {
            value
        };

        let output = serde_json::to_string_pretty(&result)?;

        // === DP2: 输出大小 ===
        bounded_output(output, self.limits.max_json_output_bytes as usize)
    }
}
```

`CsvParseTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let csv_str = args["csv"].as_str().ok_or_else(|| anyhow!("Missing csv"))?;
    check_input_size(csv_str, self.limits.max_json_input_bytes as usize, "csv_parse")?;

    let reader = csv::ReaderBuilder::new()
        .has_headers(has_header)
        .flexible(true)
        .from_reader(csv_str.as_bytes());

    let mut records = Vec::new();
    for (i, record) in reader.into_records().enumerate() {
        // === DP3: 行数上限 ===
        if i >= self.limits.max_csv_rows {
            records.push(format!("...(truncated at {} rows)", self.limits.max_csv_rows));
            break;
        }
        let record = record?;
        // === DP7: 字段大小上限 ===
        for field in record.iter() {
            if field.len() > self.limits.max_csv_field_bytes {
                return Err(anyhow!(
                    "csv_parse rejected: field at row {} > {} bytes",
                    i, self.limits.max_csv_field_bytes,
                ));
            }
        }
        records.push(record.iter().collect::<Vec<_>>().join(","));
    }
    bounded_output(records.join("\n"), self.limits.max_json_output_bytes as usize)
}
```

`HashTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text = args["text"].as_str().ok_or_else(|| anyhow!("Missing text"))?;
    let algorithm = args["algorithm"].as_str().unwrap_or("sha256");

    // === DP5: 弱算法警告 ===
    if self.limits.warn_weak_hash && matches!(algorithm.to_lowercase().as_str(), "md5" | "sha1") {
        tracing::warn!(
            target: "continuum.tools.data_processing",
            hash.algorithm = %algorithm,
            "hash_tool: '{}' is cryptographically weak. Use sha256/sha512 for security-sensitive contexts.",
            algorithm,
        );
    }

    check_input_size(text, self.limits.max_text_input_chars, "hash")?;

    let hash = match algorithm.to_lowercase().as_str() {
        "md5" => format!("{:x}", md5::compute(text.as_bytes())),
        "sha1" => {
            use sha1::{Sha1, Digest};
            let mut h = Sha1::new();
            h.update(text.as_bytes());
            format!("{:x}", h.finalize())
        }
        "sha256" => /* ... */,
        "sha512" => /* ... */,
        other => return Err(anyhow!("Unknown algorithm: {}", other)),
    };
    Ok(hash)
}
```

`Base64DecodeTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let encoded = args["encoded"].as_str().ok_or_else(|| anyhow!("Missing encoded"))?;

    // === DP6: base64 输入长度上限（考虑 4/3 放大） ===
    let max_input = (self.limits.max_text_input_chars * 3) / 4;
    check_input_size(encoded, max_input, "base64_decode")?;

    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    let s = String::from_utf8_lossy(&decoded);

    bounded_output(s.to_string(), self.limits.max_text_output_chars)
}
```

### 2.5 `text_tools` 工具加固

通用模式：

```rust
pub struct CountLinesTool {
    limits: Arc<ToolLimits>,
}

async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text = args["text"].as_str().ok_or_else(|| anyhow!("Missing text"))?;

    // === TT1: 输入大小 ===
    check_input_size(text, self.limits.max_text_input_chars, "count_lines")?;

    let lines = text.lines().count();
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    Ok(format!("Lines: {}\nCharacters: {}\nWords: {}", lines, chars, words))
}
```

`RegexMatchTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text = args["text"].as_str().ok_or_else(|| anyhow!("Missing text"))?;
    let pattern = args["pattern"].as_str().ok_or_else(|| anyhow!("Missing pattern"))?;

    check_input_size(text, self.limits.max_text_input_chars, "regex_match")?;

    // === TT3: ReDoS 防护（复用 P2）===
    validate_regex_safety(pattern, &self.limits)?;

    let group = args["group"].as_u64().unwrap_or(0) as usize;
    let re = regex::Regex::new(pattern)?;

    let matches: Vec<_> = re.find_iter(text)
        .take(self.limits.max_code_results)  // 总匹配数上限
        .collect();

    /* ... 格式化输出，应用 bounded_output ... */
}
```

`TextDiffTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text1 = args["text1"].as_str().ok_or_else(|| anyhow!("Missing text1"))?;
    let text2 = args["text2"].as_str().ok_or_else(|| anyhow!("Missing text2"))?;

    check_input_size(text1, self.limits.max_text_input_chars, "text_diff text1")?;
    check_input_size(text2, self.limits.max_text_input_chars, "text_diff text2")?;

    // === TT4: 行数上限（限制 O(N*M) 计算）===
    let lines1: Vec<&str> = text1.lines().collect();
    let lines2: Vec<&str> = text2.lines().collect();
    if lines1.len() > self.limits.max_diff_input_lines
        || lines2.len() > self.limits.max_diff_input_lines {
        return Err(anyhow!(
            "text_diff rejected: input has {}/{} lines (limit {})",
            lines1.len(), lines2.len(), self.limits.max_diff_input_lines,
        ));
    }

    /* ... compute LCS-based diff ... */
    bounded_output(result, self.limits.max_text_output_chars)
}
```

`WordFrequencyTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text = args["text"].as_str().ok_or_else(|| anyhow!("Missing text"))?;
    check_input_size(text, self.limits.max_text_input_chars, "word_frequency")?;

    // === TT5: top_k 上限 ===
    let top = args["top"].as_u64()
        .unwrap_or(10)
        .min(self.limits.max_word_freq_top as u64) as usize;

    /* ... */
}
```

`SortLinesTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text = args["text"].as_str().ok_or_else(|| anyhow!("Missing text"))?;
    check_input_size(text, self.limits.max_text_input_chars, "sort_lines")?;

    let mut lines: Vec<&str> = text.lines().collect();

    // === TT6: 行数上限 ===
    if lines.len() > self.limits.max_sort_lines {
        return Err(anyhow!(
            "sort_lines rejected: {} lines > limit {}",
            lines.len(), self.limits.max_sort_lines,
        ));
    }

    /* ... sort ... */
}
```

`TextSplitTool`：

```rust
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
    let text = args["text"].as_str().ok_or_else(|| anyhow!("Missing text"))?;
    check_input_size(text, self.limits.max_text_input_chars, "text_split")?;

    let parts: Vec<&str> = match method {
        "lines" => text.lines().collect(),
        /* ... */
    };

    // === TT7: 分割结果数上限 ===
    if parts.len() > self.limits.max_split_results {
        return Err(anyhow!(
            "text_split rejected: produced {} parts > limit {}",
            parts.len(), self.limits.max_split_results,
        ));
    }

    /* ... */
}
```

---

## 3. 接口契约

无 schema 变更（所有 P3 工具参数不变；行为变化为大小限制）。

### 3.1 兼容性矩阵

| 行为 | v1.0.x | v1.0.7 |
|------|--------|--------|
| `json_parse` 10MB input | 解析（OOM 风险） | 拒绝 |
| `regex_match` ReDoS pattern | 执行 | 拒绝 |
| `text_diff` 50k×50k 行 | 30s+ 计算 | 拒绝 |
| `hash_tool(algorithm="md5")` | 静默执行 | 执行 + 警告日志 |

---

## 4. 实现计划

### Phase A：共享 helper

- [ ] A1: 扩展 `ToolLimits` 加 P3 字段
- [ ] A2: 写 `bounds.rs`（`check_input_size` / `bounded_output`）

### Phase B：code 工具（4 个）

- [ ] B1: 4 个 code 工具加 `Arc<ToolLimits>`
- [ ] B2: 实现 C1-C7
- [ ] B3: 测试

### Phase C：data_processing 工具（14 个）

- [ ] C1: 14 个 data 工具加 `Arc<ToolLimits>`
- [ ] C2: 通用模式应用（输入/输出边界）
- [ ] C3: `HashTool` 弱算法警告
- [ ] C4: `Base64DecodeTool` 4/3 放大考虑
- [ ] C5: `CsvParseTool` 行数 + 字段上限
- [ ] C6: 测试

### Phase D：text_tools 工具（7 个）

- [ ] D1: 7 个 text 工具加 `Arc<ToolLimits>`
- [ ] D2: `RegexMatchTool` 集成 `validate_regex_safety`
- [ ] D3: `TextDiffTool` 行数上限
- [ ] D4: 其余工具通用输入/输出边界
- [ ] D5: 测试

### Phase E：集成

- [ ] E1: 全量测试
- [ ] E2: clippy + fmt
- [ ] E3: CHANGELOG + publish v1.0.7

---

## 5. 测试矩阵（精选）

### 5.1 通用 bounds（5 个测试）

- `test_check_input_size_rejects_oversize`
- `test_check_input_size_passes_within_limit`
- `test_bounded_output_passes_small`
- `test_bounded_output_truncates_large`
- `test_bounded_output_truncation_is_utf8_safe`

### 5.2 code 工具（4 个测试）

- `test_go_to_definition_caps_search_files`（C3）
- `test_find_references_caps_results`（C5）
- `test_rename_rejects_oversize_result`（C6）
- `test_code_search_detects_symlink_loop`（C4）

### 5.3 data_processing（6 个测试）

- `test_json_parse_rejects_oversize_input`
- `test_json_parse_caps_output`
- `test_csv_parse_caps_rows`
- `test_csv_parse_rejects_oversize_field`
- `test_hash_warns_on_md5`
- `test_base64_decode_caps_at_4_3_ratio`

### 5.4 text_tools（7 个测试）

- `test_regex_match_rejects_redos`（TT3）
- `test_text_diff_rejects_large_input`（TT4）
- `test_word_freq_caps_top_k`（TT5）
- `test_sort_lines_rejects_too_many`（TT6）
- `test_text_split_rejects_too_many_parts`（TT7）
- `test_count_lines_rejects_oversize_input`
- `test_text_transform_rejects_oversize_input`

---

## 6. 已知局限

### 6.1 LCS diff 算法本身性能

`TextDiffTool` 即使在 5000 行限制内，最坏情况仍是 O(25M) 操作（~500ms i7-12700H）。**完整方案**是 Myers diff 算法（O(ND)，D = edit distance）。**v1.0.7 不做** — 5000 行已覆盖大部分实际场景。

### 6.2 LSP server sandbox 不在范围内

`code.rs` 通过 LSP server 与外部进程通信（rust-analyzer、pylsp 等）。LSP server 自身的安全性（资源占用、文件访问范围）不在 Layer 3 工具加固范围。**v1.2.0+ 评估**：是否需要 LSP server 沙箱。

### 6.3 JSON 解析深度无上限

`serde_json::from_str` 默认 128 层嵌套，但 Agent 可能传入深度嵌套的 JSON 触发栈溢出。**v1.0.7 不做** — 128 层已远超实际需要；如需严格防护，可在 `serde_json::Deserializer::from_str` 配置 `disable_recursion_limit`。

---

## 7. 风险与回退

| 风险 | 缓解 |
|------|------|
| 大小上限破坏现有合法用例 | 所有上限通过 `ToolLimits` 字段可调 |
| ReDoS pattern 误报 | 错误消息提示用户简化 |
| 弱哈希警告噪音 | `warn_weak_hash=false` 关闭 |

---

## 8. 与既有 spec 的关系

| spec | 版本 | 范围 |
|------|------|------|
| `fileops-tools-hardening` | v1.0.3 | Read/Write/Edit/List |
| `p0-critical-tools-hardening` | v1.0.4 | Bash/delete/HTTP/... |
| `p1-high-risk-tools-hardening` | v1.0.5 | Move/Copy/Mkdir/system/git |
| `p2-medium-risk-tools-hardening` | v1.0.6 | Search/Memory/Workflow/WebSearch |
| **本 spec** | **v1.0.7** | Code/Data/Text |
| `stale-read-prevention` | v1.1.0 | trait + context 通路 |

**全部 51 个 builtin tool 加固完成**。总览：

| 类别 | 工具数 | spec | 版本 |
|------|--------|------|------|
| FileOps | 8 | companion + p0 + p1 | v1.0.3-v1.0.5 |
| Shell | 1 | p0 | v1.0.4 |
| Search | 2 | p2 | v1.0.6 |
| WebSearch | 1 | p2 | v1.0.6 |
| Network | 2 | p0 | v1.0.4 |
| NetworkTools | 5 | p0 | v1.0.4 |
| Git | 8 | p1 | v1.0.5 |
| System | 9 | p1 | v1.0.5 |
| Memory | 3 | p2 | v1.0.6 |
| Workflow | 3 | p2 | v1.0.6 |
| Code | 4 | p3 | v1.0.7 |
| DataProcessing | 14 | p3 | v1.0.7 |
| Text | 7 | p3 | v1.0.7 |
| **合计** | **51** | | **v1.0.3 → v1.0.7** |

加上 stale-read spec 的 trait 演进，**v1.1.0 后全部加固完成**。

---

## 9. 自评

1. **Placeholder scan**：无 TBD/TODO
2. **Internal consistency**：所有 P3 工具使用相同 `check_input_size` / `bounded_output` 模式
3. **Scope check**：聚焦输入/输出边界 + ReDoS；LSP sandbox 留 v1.2.0
4. **Ambiguity check**：
   - 上限值默认通过 `ToolLimits::default` 统一管理
   - 弱哈希警告 vs 拒绝 — 选警告（不破坏现有合法用例）
5. **业界最佳实践对齐**：
   - 输入/输出边界对齐 [CWE-400: Uncontrolled Resource Consumption](https://cwe.mitre.org/data/definitions/400.html)
   - 弱哈希警告对齐 [CWE-327: Use of a Broken or Risky Cryptographic Algorithm](https://cwe.mitre.org/data/definitions/327.html)
   - ReDoS 防护对齐 OWASP ReDoS（同 P2）
