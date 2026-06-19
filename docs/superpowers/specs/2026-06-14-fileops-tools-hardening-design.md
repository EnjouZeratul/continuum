# File Operations Tools Hardening Design

**Date:** 2026-06-14
**Status:** Draft v2 — pending review
**Affects:** `sh-layer3` (内置工具层) → `builtin_tools::file_ops`
**Target version:** v1.0.3 (patch)
**Author:** Continuum Team
**Supersedes:** v1 of this document (which was scoped to `ReadFileTool` only)

---

## 1. 背景与问题陈述

### 1.1 调研范围

本方案基于对 `rust/layer3/src/builtin_tools/file_ops.rs` 共 1148 行源码的完整阅读，并交叉确认了：

- `rust/layer3/src/builtin_tools/mod.rs` 的工具注册方式（`mod.rs:99-107`）
- `rust/layer3/src/types.rs:294-359` 的错误模型（`Layer3Result = anyhow::Result`）
- 项目内其他 `with_config` / `with_limits` 模式（`WebSearchTool`、`SaveMemoryTool`、`CreateCheckpointTool`）

下表为**已通过代码阅读确认**的事实，每条注明源码行号：

### 1.2 四个工具的事实性缺陷

#### `ReadFileTool`（`file_ops.rs:9-101`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| R1 | 无文件大小预检 | `:52` | `read_to_string` 一次性分配整个文件 buffer，RSS 暴涨 |
| R2 | 无默认行数上限 | `:61-64` | 不传 `limit` 时返回完整内容，可能百万行 |
| R3 | 无单行长度上限 | `:81` | minified JS / 含 base64 的日志单行可达 MB 级 |
| R4 | 无 UTF-8 二进制检测 | `:52` | 遇非 UTF-8 才报错，但已分配 buffer |
| R5 | 无返回元数据 | `:62` | 调用者无法判断"是否被截断"、"还有多少未读" |

#### `WriteFileTool`（`file_ops.rs:103-155`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| W1 | 无写入内容大小限制 | `:152` | Agent 可写入任意大小 → 填满磁盘、撑爆 inode |
| W2 | 无父目录存在检查 | `:152` | 写入不存在路径会报错，但错误消息不友好 |
| W3 | 静默覆盖已有文件 | `:152` | 与 `WriteFileTool` 标记 `is_dangerous=true` 的语义不一致；Agent 可能意外覆盖重要文件 |

#### `EditFileTool`（`file_ops.rs:157-219`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| E1 | 无文件大小预检 | `:210` | 先 `read_to_string` 整个文件 → 大文件 OOM 风险，与 R1 同源 |
| E2 | 无 `old_string` 唯一性检查 | `:215` | `replace` 全量替换：若 `old_string` 在文件中出现 N 次会全部替换 — 静默地不符合"编辑特定位置"的预期 |
| E3 | 替换 0 次时仍写回 | `:215-216` | 当前先 `contains` 检查兜住了这点（若不含则报错），但 `replace` 替换 0 次的边界情况无显式断言 |

#### `ListDirectoryTool`（`file_ops.rs:221-270`）

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| L1 | 无条目数上限 | `:258` | 列出含 1000 万文件的目录会撑爆 context |
| L2 | 无路径类型检查 | `:255` | 若 path 是文件而非目录，错误来自 OS 层、消息不友好 |
| L3 | 无排序保证 | `:258` | 不同 OS / FS 返回顺序不同 → LLM 推理结果不稳定 |

### 1.3 实际风险场景

Agent 在自主循环中调用这些工具时：

- **场景 A（成本爆炸）**：`read_file("/var/log/huge.log")` 50MB → ~12.5M tokens → 单次费用从 $0.01 升到 $25+
- **场景 B（OOM）**：`read_file` 或 `edit_file` 1GB 文件 → 进程 RSS 增加 1GB → 容器 OOM killed
- **场景 C（数据破坏）**：`edit_file` 一个含多个 `import React` 的文件，`old_string="import React"` → 全部被替换，破坏代码
- **场景 D（磁盘耗尽）**：`write_file` 写 10GB 内容 → 填满磁盘
- **场景 E（context 污染）**：`list_directory` 列出 `node_modules` 数十万 entry → 单次响应不可用
- **场景 F（垃圾数据）**：`read_file` 二进制 `.exe` → UTF-8 错误或乱码塞进 context

### 1.4 范围界定（YAGNI）

**本方案处理：** 4 个工具的输入/输出边界硬化，引入共享 `FileOpsLimits` 配置。

**本方案不处理：**

- 全局工具调用配额（属于 Layer 2 编排层职责）
- Sandbox 路径白名单（Layer 0 已有 `access_controller.rs`）
- 流式读取 / chunked response（要改 `BuiltinTool::execute` 返回类型，超出 patch 范围）
- `MoveFileTool` / `CopyFileTool` / `CreateDirectoryTool` / `DeleteFileTool`（这 4 个不读写文件**内容**，不是同类风险，独立审视）
- Python SDK 对应工具的镜像修复（v1.1.0）
- `document_loaders/*` 中的同类问题（独立 spec）

---

## 2. 设计目标

### 2.1 必须达成（P0）

1. **防 OOM**：4 个工具任何输入下进程内存增量必须有可预测上限
2. **防 token 爆炸**：默认输出不超过约 ~50k tokens
3. **防数据破坏**：`edit_file` 的 `old_string` 必须唯一才允许替换；`write_file` 不允许静默覆盖
4. **防磁盘耗尽**：`write_file` 内容大小有硬上限
5. **行为可观测**：返回值给出 size / 截断 / 覆盖等元信息
6. **向后兼容**：现有 Read/Move/Copy/Delete 共 18 个测试全部继续通过；外部 API（trait、注册函数）签名不变
7. **可配置**：通过 `FileOpsLimits` 结构体集中管理，调用方可调整
8. **拒绝时给出指导**：错误消息必须告诉调用者下一步该做什么

### 2.2 不追求

- 完美的二进制检测（业界都用启发式 NUL 探测）
- 极致性能（IO bound 操作，几十 μs 的检查不影响）
- 编码自动检测（仅支持 UTF-8）
- 流式 IO 重写（patch 版本风险过高）

---

## 3. 设计

### 3.1 共享配置：`FileOpsLimits`

新增到 `file_ops.rs` 顶部（紧跟 `use` 语句之后）：

```rust
/// 文件操作工具的安全限制配置
///
/// 集中管理 ReadFileTool / WriteFileTool / EditFileTool / ListDirectoryTool
/// 的所有边界值，便于按部署场景调整（嵌入式、桌面、服务器各取所需）。
///
/// 所有限制均为防御 LLM Agent 滥用工具的硬边界，而非性能调优。
#[derive(Debug, Clone)]
pub struct FileOpsLimits {
    /// 单文件读取上限（bytes）。超过则要求 offset+limit 分块读取。
    /// 默认 10 MiB，覆盖绝大多数源码与配置文件，拒绝日志/数据文件。
    pub max_read_bytes: u64,

    /// 单文件写入上限（bytes）。超过直接拒绝。
    /// 默认 10 MiB，对称于 max_read_bytes。
    pub max_write_bytes: u64,

    /// 单文件编辑（read+write）上限（bytes）。超过直接拒绝。
    /// 默认 10 MiB，与 max_read_bytes 一致 — 因为 Edit 内部要 read。
    pub max_edit_bytes: u64,

    /// 不指定 limit 时 ReadFileTool 默认返回的最大行数。
    /// 默认 2000，覆盖 ~80 字符/行的源码 ~160KB → ~40k tokens。
    pub default_read_lines: usize,

    /// ReadFileTool 单行字符上限（超过则截断并标注）。
    /// 默认 2000，与默认行数同量级。
    pub max_line_chars: usize,

    /// 二进制嗅探缓冲区字节数。
    /// 默认 8192，足够覆盖大多数 magic bytes 同时避免不必要 IO。
    pub binary_sniff_bytes: usize,

    /// ListDirectoryTool 返回条目数上限（超过则截断并提示）。
    /// 默认 1000，覆盖正常项目目录，拒绝 node_modules 这类。
    pub max_dir_entries: usize,
}

impl Default for FileOpsLimits {
    fn default() -> Self {
        Self {
            max_read_bytes:       10 * 1024 * 1024,
            max_write_bytes:      10 * 1024 * 1024,
            max_edit_bytes:       10 * 1024 * 1024,
            default_read_lines:   2000,
            max_line_chars:       2000,
            binary_sniff_bytes:   8192,
            max_dir_entries:      1000,
        }
    }
}
```

**为什么三个 size 字段独立而非一个 `max_file_bytes`：**

不同工具的内存占用模型不同（Read 要存全文 + lines vec；Edit 要存 old + new 两份；Write 仅持有 `&str`），未来若要差异化调整不需要改类型签名。当前默认值恰好相同只是巧合。

### 3.2 工具结构体改造

四个工具从单元结构体（`pub struct XxxTool;`）改为持有配置：

```rust
pub struct ReadFileTool { limits: FileOpsLimits }
pub struct WriteFileTool { limits: FileOpsLimits }
pub struct EditFileTool { limits: FileOpsLimits }
pub struct ListDirectoryTool { limits: FileOpsLimits }
```

每个工具提供两个构造器，**沿用项目惯例**（`WebSearchTool::new()` / `with_config()`）：

```rust
impl ReadFileTool {
    pub fn new() -> Self {
        Self { limits: FileOpsLimits::default() }
    }
    pub fn with_limits(limits: FileOpsLimits) -> Self {
        Self { limits }
    }
}
```

### 3.3 注册方式的兼容性问题

**当前两处注册点**（v2 修订：v1 漏列了 adapter）：

```rust
// rust/layer3/src/builtin_tools/mod.rs:100-107
registry.register(Box::new(file_ops::ReadFileTool));   // 单元结构体

// rust/layer3/src/builtin_tools/adapter.rs:71-74
registry.register(Box::new(ToolAdapter::new(Box::new(ReadFileTool))))?;
```

改造后两处都必须变成 `ReadFileTool::new()`。

**adapter.rs 的测试也需要更新**（`adapter.rs:107-111`）。

为彻底防止外部反对，**追加 `Default` impl**：

```rust
impl Default for ReadFileTool {
    fn default() -> Self { Self::new() }
}
```

这样 `Box::new(ReadFileTool::default())` 也工作，与 `BuiltinToolRegistry::default()` 一致。

### 3.4 检查管线

#### 3.4.1 `ReadFileTool::execute` 流程

```
1. 解析 path/offset/limit                          [现有]
2. metadata(path)                                  [新增]
   ├─ 不存在 → Err("File not found: {path}")
   ├─ 是目录 → Err("Path is a directory: {path}, use list_directory")
   └─ size > max_read_bytes
      → Err("File too large ({}MB > {}MB limit). Use offset+limit ...")
3. 读取前 binary_sniff_bytes 字节做嗅探             [新增]
   └─ 含 NUL → Err("File appears to be binary: {path}")
4. tokio::fs::read_to_string(path)                 [现有]
5. 行级分页 + 单行截断 + 元数据封装                  [改造]
```

#### 3.4.2 `WriteFileTool::execute` 流程

```
1. 解析 path/content/overwrite                     [改造：新增 overwrite 参数]
2. content.len() > max_write_bytes
   → Err("Content too large ({}MB > {}MB limit)")
3. 检查 path 是否已存在
   ├─ 存在 且 overwrite=false
   │  → Err("File exists: {path}. Pass overwrite=true to replace")
   └─ 存在 且 overwrite=true → 继续
4. 确保父目录存在（与 MoveFileTool 一致的模式）
5. tokio::fs::write(path, content)                 [现有]
```

**注意 W3 的修复属于行为变更（破坏性）：** 详见 §4.2。

#### 3.4.3 `EditFileTool::execute` 流程

```
1. 解析 path/old_string/new_string                 [现有]
2. metadata(path)                                  [新增]
   ├─ 不存在 → Err
   ├─ 是目录 → Err
   └─ size > max_edit_bytes → Err
3. 二进制嗅探                                       [新增]
4. tokio::fs::read_to_string                        [现有]
5. old_string 在文件中出现次数 = matches            [新增]
   ├─ 0 次 → Err("old_string not found")              [现有覆盖]
   ├─ >1 次 → Err("old_string appears {n} times. Provide more context to make it unique")  [新增]
   └─ 1 次 → 继续
6. content.replacen(old, new, 1)                    [改造：用 replacen 替代 replace]
7. 检查 new content 大小 ≤ max_edit_bytes           [新增]
8. tokio::fs::write                                  [现有]
```

#### 3.4.4 `ListDirectoryTool::execute` 流程

```
1. 解析 path                                       [现有]
2. metadata(path)                                  [新增]
   ├─ 不存在 → Err
   └─ 不是目录 → Err("Path is not a directory: {path}")
3. 收集所有 entry 到 Vec                            [现有]
4. 按文件名排序（稳定输出）                         [新增]
5. 若 entries.len() > max_dir_entries：             [新增]
   ├─ 截断到 max_dir_entries
   └─ 输出末尾追加 "[truncated: {n} more entries not shown]"
6. join("\n") 返回                                  [现有]
```

### 3.5 二进制检测策略（共享辅助函数）

```rust
async fn looks_like_binary(path: &str, sniff_bytes: usize) -> Layer3Result<bool> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await?;
    let mut buf = vec![0u8; sniff_bytes];
    let n = file.read(&mut buf).await?;
    Ok(buf[..n].contains(&0u8))
}
```

**理由（与 v1 spec 相同）：**

- 合法 UTF-8 文本不含 NUL
- Git `xdiff/xutils.c::buffer_is_binary` 同款方法
- ripgrep 默认也用此方法
- 已知 false positive：UTF-16 文本会被识别为二进制 — 可接受（现有 `read_to_string` 也无法处理）

### 3.6 单行截断（共享辅助函数）

```rust
fn truncate_long_line(line: &str, max_chars: usize) -> (String, bool) {
    if line.chars().count() <= max_chars {
        (line.to_string(), false)
    } else {
        let truncated: String = line.chars().take(max_chars).collect();
        let total = line.chars().count();
        (format!("{}...[line truncated, {} chars total]", truncated, total), true)
    }
}
```

用 `chars().count()` 而非 `len()` 避免在 multi-byte UTF-8 字符中间切断 panic。

### 3.7 错误模型

所有新增错误使用项目已有的 `anyhow!` 风格（`Layer3Result<T> = anyhow::Result<T>`），不引入新 `Layer3Error` 变体（保持 patch 版本兼容）。

错误消息模式：**问题 + 可执行建议**：

| 工具 | 触发条件 | 错误消息 |
|------|---------|---------|
| Read | 是目录 | `Path is a directory: {path}. Use list_directory tool instead.` |
| Read | 超大无分页 | `File too large: {n} bytes (limit: {max}). Use offset and limit parameters to read in chunks. Total estimated lines: {est}` |
| Read | 二进制 | `File appears to be binary (contains NUL bytes): {path}. Use a binary-aware tool instead.` |
| Write | 内容过大 | `Content too large: {n} bytes (limit: {max}).` |
| Write | 已存在 | `File already exists: {path}. Pass overwrite=true to replace.` |
| Edit | 多次匹配 | `old_string appears {n} times in {path}. Provide more context to make it unique.` |
| Edit | 编辑后过大 | `Edit would exceed file size limit: {n} bytes (limit: {max}).` |
| List | 不是目录 | `Path is not a directory: {path}. Use read_file for files.` |

---

## 4. 接口契约（外部可见行为）

### 4.1 输入 schema 变更

#### `ReadFileTool` — 仅描述更新

```json
{
  "path": { "type": "string" },
  "offset": {
    "type": "integer",
    "description": "Optional: 0-based line number to start reading from. Required when file exceeds 10 MiB."
  },
  "limit": {
    "type": "integer",
    "description": "Optional: number of lines to read. Defaults to 2000 if omitted. Lines longer than 2000 chars are truncated."
  }
}
```

#### `WriteFileTool` — 新增 `overwrite` 参数

```json
{
  "path": { "type": "string" },
  "content": { "type": "string", "description": "...; max 10 MiB." },
  "overwrite": {
    "type": "boolean",
    "description": "Optional: replace existing file (default: false). Required true when target exists.",
    "default": false
  }
}
```

#### `EditFileTool` — 仅描述更新

```json
{
  "path": { "type": "string" },
  "old_string": {
    "type": "string",
    "description": "Text to replace. MUST appear exactly once in the file; provide enough surrounding context for uniqueness."
  },
  "new_string": { "type": "string" }
}
```

#### `ListDirectoryTool` — 仅描述更新

```json
{
  "path": { "type": "string", "description": "Directory path. Returns entries sorted by name, max 1000 entries." }
}
```

### 4.2 兼容性矩阵

| 调用形式 | v1.0.2 行为 | v1.0.3 行为 | 兼容? |
|---------|------------|------------|-------|
| `read_file` 100 行小文件无参 | 完整内容 | 完整内容 | ✅ |
| `read_file` 5000 行无参 | 5000 行 | 前 2000 行 + 元数据 | ⚠️ 行为变化 |
| `read_file` 1GB | OOM | `Err: too large` | ⚠️ 错误变化 |
| `read_file` 含 5MB 单行 | 5MB 字符串 | 截断到 2000 chars | ⚠️ 行为变化 |
| `read_file` `.png` | UTF-8 错误 | `Err: appears to be binary` | ⚠️ 错误变化 |
| `write_file` 新文件 | Ok | Ok | ✅ |
| `write_file` 已存在文件无 overwrite | **静默覆盖** | `Err: exists, pass overwrite=true` | ⚠️ **破坏性** |
| `write_file` 已存在 + overwrite=true | （旧版无此参数）默认覆盖 | Ok 覆盖 | ✅ |
| `write_file` 100MB | Ok | `Err: content too large` | ⚠️ 错误变化 |
| `edit_file` `old_string` 唯一匹配 | Ok | Ok | ✅ |
| `edit_file` `old_string` 多处匹配 | **全部替换** | `Err: appears N times` | ⚠️ **破坏性** |
| `edit_file` 1GB 文件 | OOM | `Err: too large` | ⚠️ 错误变化 |
| `list_directory` 100 entries | 全列 | 全列 + 排序 | ⚠️ 顺序变化 |
| `list_directory` 100k entries | 全列（context 爆炸） | 前 1000 + 截断标注 | ⚠️ 行为变化 |

**评估：**

- 所有"⚠️ 行为变化"和"⚠️ 错误变化"都是从"危险/失败"变到"安全/明确"
- 两处标记"⚠️ **破坏性**"特别需要注意：
  - `write_file` 静默覆盖 → 必须显式 `overwrite=true`
  - `edit_file` 全部替换 → 必须 `old_string` 唯一
- 这两处恰好就是 §1.2 中标记的 W3、E2 缺陷修复。**旧行为本身就是 bug**，无合理用户依赖。

**结论：** 视为可接受的 patch 版本变更。如审阅者认为不可接受，备选：升 minor（v1.1.0）。

---

## 5. 实现计划（TDD）

每个 commit 对应一个原子任务。**严格遵循 TDD：先写失败测试 → 实现 → 测试通过 → commit**。

工作量估算：12 commits × 平均 ~20 分钟 = ~4 小时；自审 + clippy + fmt + CI ≈ ~1 小时；总计 **~5 小时**。

### Phase 1: 共享基础设施

#### Task 1: 引入 `FileOpsLimits` 与共享辅助函数

**Files:** `rust/layer3/src/builtin_tools/file_ops.rs`

- [ ] 在 `use` 语句后添加 `FileOpsLimits` 结构体 + `Default` impl（§3.1）
- [ ] 添加 private `looks_like_binary(path, sniff_bytes) -> Layer3Result<bool>`
- [ ] 添加 private `truncate_long_line(&str, max_chars) -> (String, bool)`
- [ ] 测试：`FileOpsLimits::default()` 字段值正确
- [ ] 测试：`looks_like_binary` 对纯文本返回 false、对含 NUL 字节文件返回 true
- [ ] 测试：`truncate_long_line` 短行不变、长行被截断、UTF-8 emoji 行不 panic
- [ ] `cargo test -p sh-layer3 builtin_tools::file_ops` 全绿
- [ ] commit: `chore(layer3): introduce FileOpsLimits and shared file guards`

### Phase 2: ReadFileTool 加固

#### Task 2: ReadFileTool 改为持有 limits + 注册点更新

- [ ] 修改 `ReadFileTool` → 含 `limits: FileOpsLimits` 字段
- [ ] 添加 `pub fn new()`、`pub fn with_limits()`、`impl Default`
- [ ] 修改 `mod.rs:100` 为 `Box::new(file_ops::ReadFileTool::new())`
- [ ] 现有 11 个测试全部更新为 `let tool = ReadFileTool::new();`
- [ ] `cargo test` 全绿（仅是结构改造，无功能变化）
- [ ] commit: `refactor(layer3): make ReadFileTool configurable via FileOpsLimits`

#### Task 3: ReadFileTool — metadata 预检 + 二进制检测

- [ ] 写失败测试：传入目录路径 → `Err("Path is a directory")`
- [ ] 写失败测试：构造 11MB 文件 → `Err("File too large")`
- [ ] 写失败测试：含 NUL 字节文件 → `Err("appears to be binary")`
- [ ] `cargo test`：3 个 FAIL
- [ ] 在 `execute` 开头插入 metadata 检查 + binary 嗅探
- [ ] `cargo test`：PASS
- [ ] commit: `feat(layer3): add file size, type and binary guards to ReadFileTool`

#### Task 4: ReadFileTool — 默认 limit + 单行截断 + 元数据 header

- [ ] 写测试：5000 行无 limit → 前 2000 行 + header 含 `5000 total`
- [ ] 写测试：含 5000 字符长行 → `[line truncated, 5000 chars total]`
- [ ] 写测试：header 包含 `size=N bytes`
- [ ] `cargo test`：FAIL
- [ ] 修改分页逻辑：`limit.unwrap_or(self.limits.default_read_lines)`、对每行应用 `truncate_long_line`、header 增加 `size`
- [ ] 保留情况 A 短路（小文件无截断仍直接返回 raw content）以保护 `test_read_file_no_pagination`
- [ ] `cargo test`：全绿（含原 11 个测试）
- [ ] commit: `feat(layer3): apply default line limit and per-line truncation in ReadFileTool`

### Phase 3: WriteFileTool 加固

#### Task 5: WriteFileTool 改为持有 limits

- [ ] 同 Task 2 模式：`WriteFileTool` 含字段、`new`、`with_limits`、`Default`
- [ ] 更新 `mod.rs:101` 注册点
- [ ] 更新现有 1 个测试（`test_write_file_tool_dangerous`）
- [ ] `cargo test` 全绿
- [ ] commit: `refactor(layer3): make WriteFileTool configurable`

#### Task 6: WriteFileTool — 内容大小限制 + overwrite 语义

- [ ] 写失败测试：写 11MB content → `Err("Content too large")`
- [ ] 写失败测试：写已存在文件无 overwrite → `Err("already exists")`
- [ ] 写测试：写已存在文件 + `overwrite=true` → Ok
- [ ] 写测试：写新文件无 overwrite → Ok（兼容）
- [ ] `cargo test`：4 个 FAIL
- [ ] 修改 `parameters_schema()` 增加 `overwrite` 字段
- [ ] 修改 `execute`：解析 overwrite、size 检查、existence 检查、父目录确保
- [ ] `cargo test`：PASS
- [ ] commit: `feat(layer3): enforce write size limit and explicit overwrite in WriteFileTool`

### Phase 4: EditFileTool 加固

#### Task 7: EditFileTool 改为持有 limits

- [ ] 同 Task 2 模式
- [ ] 更新 `mod.rs:102` 注册点
- [ ] `cargo test` 全绿（无现有 EditFileTool 测试需更新；现有仅有 `test_*` 通过 `EditFileTool;` 这种语法的搜索 — 见下）
- [ ] commit: `refactor(layer3): make EditFileTool configurable`

> **核查：** 当前 file_ops.rs 全文搜索 `EditFileTool` 的使用：仅出现在 §3.2 的 `pub struct` 定义、`mod.rs` 注册、以及实现块。**没有**单独的 EditFileTool 测试。Phase 4 必须**新增**测试覆盖。

#### Task 8: EditFileTool — 文件大小预检 + 二进制检测

- [ ] 写失败测试：edit 11MB 文件 → `Err("too large")`
- [ ] 写失败测试：edit 二进制文件 → `Err("appears to be binary")`
- [ ] 写测试：edit 不存在文件 → `Err("File not found")`
- [ ] `cargo test`：FAIL
- [ ] 在 `execute` 开头插入 metadata + binary 检查
- [ ] `cargo test`：PASS
- [ ] commit: `feat(layer3): add size and binary guards to EditFileTool`

#### Task 9: EditFileTool — old_string 唯一性

- [ ] 写测试：`old_string` 出现 2 次 → `Err("appears 2 times")`
- [ ] 写测试：`old_string` 出现 1 次 → Ok（替换成功）
- [ ] 写测试：`old_string` 出现 0 次 → `Err("not found")` (兼容现有行为)
- [ ] 写测试：替换后内容超过 `max_edit_bytes` → `Err("would exceed")`
- [ ] `cargo test`：FAIL
- [ ] 把 `replace` 改为 `matches().count()` 检查 + `replacen(_, _, 1)`
- [ ] 增加替换后 size 检查
- [ ] `cargo test`：PASS
- [ ] commit: `feat(layer3): require unique old_string in EditFileTool`

### Phase 5: ListDirectoryTool 加固

#### Task 10: ListDirectoryTool 改为持有 limits

- [ ] 同 Task 2 模式
- [ ] 更新 `mod.rs:103` 注册点
- [ ] `cargo test` 全绿
- [ ] commit: `refactor(layer3): make ListDirectoryTool configurable`

#### Task 11: ListDirectoryTool — 类型检查 + 排序 + 截断

- [ ] 写测试：传入文件路径 → `Err("not a directory")`
- [ ] 写测试：传入不存在路径 → `Err("not found")`
- [ ] 写测试：列出 1500 entries 的目录 → 仅返回前 1000 + 截断标注
- [ ] 写测试：列表按名称稳定排序
- [ ] `cargo test`：FAIL
- [ ] 实现：metadata 检查、收集后排序、截断
- [ ] `cargo test`：PASS
- [ ] commit: `feat(layer3): sort, validate and cap entries in ListDirectoryTool`

### Phase 6: 收尾

#### Task 12: 文档与 schema 更新

- [ ] 4 个 `parameters_schema()` description 字段更新（§4.1）
- [ ] 4 个 `description()` 更新提及限制
- [ ] `file_ops.rs` 顶部模块 doc 注释更新
- [ ] `cargo doc -p sh-layer3` 无 warning
- [ ] commit: `docs(layer3): document FileOps tool safety limits`

### 整体验证

- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 无新 lint
- [ ] `cargo test -p sh-layer3` 全绿
- [ ] `cargo test --workspace`（防止下游回归）全绿
- [ ] CI 全绿
- [ ] 版本号：workspace `Cargo.toml` 与 `python/pyproject.toml` 1.0.2 → 1.0.3
- [ ] 内部依赖版本同步更新（workspace.dependencies 中的 sh-layer\* version）
- [ ] commit: `chore(release): bump to 1.0.3 for FileOps hardening`

---

## 6. 测试矩阵

新增测试约 25 个。完整列表：

### ReadFileTool（新增 8 个，保留原 11 个）

| 测试名 | 期望 |
|--------|------|
| `test_read_file_rejects_directory` | `Err("Path is a directory")` |
| `test_read_file_rejects_oversized` | `Err("File too large")` |
| `test_read_file_rejects_binary` | `Err("appears to be binary")` |
| `test_read_file_default_limit_applied` | 前 2000 行 + header `5000 total` |
| `test_read_file_truncates_long_line` | 含 `[line truncated, 5000 chars total]` |
| `test_read_file_header_contains_size` | header 包含 `size=` |
| `test_read_file_unicode_truncation_safe` | 全 emoji 长行不 panic |
| `test_read_file_with_custom_limits` | `with_limits` 后小限制生效 |

### WriteFileTool（新增 5 个，保留原 1 个）

| 测试名 | 期望 |
|--------|------|
| `test_write_file_rejects_oversized_content` | `Err("Content too large")` |
| `test_write_file_rejects_existing_no_overwrite` | `Err("already exists")` |
| `test_write_file_overwrite_true_succeeds` | Ok |
| `test_write_file_creates_parent_dir` | 父目录自动创建 |
| `test_write_file_with_custom_limits` | `with_limits` 后小限制生效 |

### EditFileTool（新增 7 个，原 0 个）

| 测试名 | 期望 |
|--------|------|
| `test_edit_file_basic_replace` | 单次匹配，正常替换 |
| `test_edit_file_not_found` | `Err("File not found")` |
| `test_edit_file_old_string_not_in_file` | `Err("not found")` |
| `test_edit_file_rejects_multiple_matches` | `Err("appears 2 times")` |
| `test_edit_file_rejects_oversized` | `Err("too large")` |
| `test_edit_file_rejects_binary` | `Err("appears to be binary")` |
| `test_edit_file_rejects_oversized_after_edit` | `Err("would exceed")` |

### ListDirectoryTool（新增 4 个，原 0 个）

| 测试名 | 期望 |
|--------|------|
| `test_list_directory_basic` | 正常列出（兼容原行为） |
| `test_list_directory_rejects_file_path` | `Err("not a directory")` |
| `test_list_directory_truncates_large` | 1500 entries → 前 1000 + 标注 |
| `test_list_directory_sorted_output` | 稳定排序 |

**累计：** 25 新增 + 18 保留 = 43 个测试。

---

## 7. 已知局限与风险

### 7.1 未完全闭合的风险

**`tokio::fs::read_to_string` 仍一次性加载**：

即使 metadata 拦截了 >10MB，10MB 以内仍一次加载。10MB 内存峰值在现代机器上不显著，留作 v1.1.0 的流式重写。

### 7.2 二进制检测边界

- 前 8KB 全文本但后面是二进制（如含文本 header 的 ZIP）→ 漏检 → fallback 到 `read_to_string` 的 UTF-8 错误
- UTF-16 文件被误判为二进制 → 与现有不支持一致

### 7.3 行为兼容性

§4.2 中两个 ⚠️ **破坏性** 项：

- `write_file` 静默覆盖 → 显式 overwrite
- `edit_file` 多次替换 → 唯一匹配

**严格按 SemVer：** 这两项可视为安全 bug 修复，符合 patch 语义。但若审阅者认为可观察行为变化太大：
- 选项 A（推荐）：作为 v1.0.3 patch，理由是修复 bug
- 选项 B（保守）：作为 v1.1.0 minor，更安全

### 7.4 资源竞争与 TOCTOU

`metadata(path)` → `read_to_string(path)` 之间存在 TOCTOU 窗口：文件可能在两次系统调用之间被替换。

**评估：**
- 单进程内 Agent 工作场景下，这不是攻击向量（Agent 不会和外部进程对抗）
- 多进程场景下，Layer 0 sandbox 应承担隔离责任
- 完美修复需要打开文件后用 `fstat` 而非用路径再次 stat — 当前 Tokio API 支持但侵入性大

**决定：** 不修复，注明已知局限。

### 7.5 不在范围内的剩余漏洞

`MoveFileTool` / `CopyFileTool` 没有大小限制：理论上能复制 1TB 文件。**评估：** 这是文件系统级操作，OS 已有自然限制（磁盘满会失败），且 Agent 调用语义清楚（用户知道在移文件），**风险等级低于 Read/Write/Edit**，不在本方案范围。

---

## 8. 与项目规范的契合度

| 规范来源 | 检查项 | 本方案 |
|----------|--------|--------|
| `docs/superpowers/specs/` 目录约定 | 文件名 `YYYY-MM-DD-<topic>-design.md` | ✅ |
| `BuiltinTool` trait 接口 | 不修改 trait | ✅ |
| `Layer3Result = anyhow::Result` | 不引入新 Error 变体 | ✅ |
| 测试惯例（`tempfile::TempDir` + `tokio::test`） | 沿用 | ✅ |
| 配置化构造器模式（`new` + `with_xxx`） | 与 `WebSearchTool`、`SaveMemoryTool`、`CreateCheckpointTool` 一致 | ✅ |
| `Default` impl 惯例 | 4 个工具都加 | ✅ |
| const 命名 SCREAMING_SNAKE_CASE | N/A（用结构体字段不用 const） | — |
| 中英混合注释 | 顶层中文注释，代码英文 | ✅ |
| `clippy -D warnings` 通过 | 实现时遵守 | ✅ |
| `cargo fmt` | 实现时遵守 | ✅ |
| 注册点向后兼容 | 仅破坏单元结构体 → struct 的形式，且无外部反向依赖 | ✅ |

---

## 9. 决策点（最终敲定）

基于用户已选定的方向：

| Q | 决策 |
|---|------|
| Q1 版本号 | **v1.0.3 patch** |
| Q2 配置化 | **引入 `FileOpsLimits`**（4 工具共享） |
| Q3 范围 | **4 工具（Read/Write/Edit/ListDirectory）** |

剩余开放问题已敲定：

### Q4: `write_file` 默认 overwrite 行为 — **决策：A（默认 false，已存在则报错）**

理由：最安全、符合 LLM 工具最佳实践、与"`is_dangerous=true` 标记"的语义一致。Agent 显式传入 `overwrite=true` 才能覆盖。

### Q5: `edit_file` 多次匹配处理 — **决策：A（直接报错）**

理由：与 Claude Code Edit 工具一致，对 LLM 友好（错误消息引导其加上下文使匹配唯一）。`replace_all` 留作未来按需扩展。

**所有决策点已敲定，无剩余开放问题，可进入实施阶段。**

> **后续 spec：** "防止 stale-read 导致的 lost-update"作为独立设计 `2026-06-14-stale-read-prevention-design.md`（v1.1.0），不在本方案范围。本方案承担"工具自身边界"，stale-read spec 承担"跨工具调用一致性"。

> **v2 后续修订（来自 stale-read spec §12）：** 本方案在 v1.0.3 引入的 `FileOpsLimits` 独立结构，将在 v1.1.0 由 stale-read spec 合并到 `ExecutionContext.file_ops_limits` 字段。届时本方案的 `with_limits()` 构造器会被标记 `#[deprecated]`，工具改为通过 ctx 接收 limits。v1.0.3 实施时无需考虑此点，但 §3.2 的 `with_limits` 设计应保持向后兼容（构造器可独立调用、不与未来 ctx 强耦合）。

---

## 10. 实施完成定义（DoD）

- [ ] 现有 18 个 file_ops 测试全部通过（含未变更的 Move/Copy/Delete/CreateDirectory）
- [ ] 25 个新增测试全部通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 无新 lint
- [ ] `cargo fmt --check` 通过
- [ ] CI 全绿
- [ ] v1.0.3 已发布到 crates.io（受影响 crate：`sh-layer3`、`sh-core`、`continuum`）
- [ ] PyPI 不受影响
- [ ] 本文档作为 PR 描述链接

---

## 11. 参考

- 项目代码：`rust/layer3/src/builtin_tools/file_ops.rs`（1148 行）
- 注册点：`rust/layer3/src/builtin_tools/mod.rs:99-107`
- 错误模型：`rust/layer3/src/types.rs:294-359`
- 配置化模式参考：
  - `rust/layer3/src/builtin_tools/web_search.rs:225-260` (`WebSearchTool::with_config`)
  - `rust/layer3/src/builtin_tools/memory_tools.rs:18-29` (`SaveMemoryTool::with_store`)
  - `rust/layer3/src/builtin_tools/workflow_tools.rs:30-43` (`CreateCheckpointTool::with_path`)
- 二进制检测参考：Git `xdiff/xutils.c::buffer_is_binary`、ripgrep `searcher::core::ContentInspector`
- 设计参考：Claude Code Edit 工具的"`old_string` 必须唯一"语义
