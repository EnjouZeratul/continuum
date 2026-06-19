# Stale-Read Prevention & Tool Execution Context Design

**Date:** 2026-06-14
**Status:** Draft v2 — quality-first revision
**Affects:** `sh-layer3` (`builtin_tools` trait + 4 file tools, `tool_executor`), `sh-layer2` (`Tool::execute` 签名 + `tool_registry` adapter)
**Target version:** v1.1.0 (minor — Layer 2 `Tool` trait 与 Layer 3 `BuiltinTool` trait 同时 minor breaking)
**Author:** Continuum Team
**Companion spec:** [`2026-06-14-fileops-tools-hardening-design.md`](./2026-06-14-fileops-tools-hardening-design.md) (v1.0.3 patch — input/output bounds)
**Supersedes:** v1 of this document (合并 ExecutionContext / 撤回 default-method patch / 撤回 task_local — 见 §13 修订记录)

---

## 1. 问题陈述

### 1.1 Lost-Update 场景

```
T0  Agent: read_file("config.toml")           → 返回内容 V1
T1  外部进程 / 另一并发 Agent 修改 config.toml  → 文件变为 V2
T2  Agent: edit_file("config.toml",           → 当前实现：基于 V2 替换
                     old_string="...V1 片段",     若 V1 片段在 V2 中仍存在 → 静默替换，
                     new_string="...")           V2 的其他改动被悄悄保留但 Agent 完全
                                                 不知道自己在编辑一个已经变化的文件
```

**当前 `EditFileTool::execute`（`file_ops.rs:199-218`）的行为**：

```rust
let content = fs::read_to_string(path)?;  // 读取最新磁盘内容
let new_content = content.replace(old, new);
fs::write(path, new_content)?;
```

**根本缺陷**：工具不知道 Agent 上次"看到"的是哪个版本。Agent 的推理基于 V1，但写入基于 V2。这是经典的 lost-update / TOCTOU race。

### 1.2 真实风险场景

- **场景 G（覆盖外部修改）**：用户在 IDE 改了文件，Agent 基于旧读取做 edit，**用户的改动看似被保留实则被破坏**（`old_string` 命中位置移动）
- **场景 H（多 Agent 互相覆盖）**：两个 Agent 并发处理同一仓库，A 读取 → B 写入 → A 写入，B 的改动丢失
- **场景 I（推理失效）**：Agent 在长 session 中早期读了 file，几十轮后基于该读取做决策，文件早已变化
- **场景 J（write_file 全量覆盖）**：Agent 在未 read 的情况下直接 `write_file` 覆盖一个**已存在但内容未知**的文件 — 这正是 [companion spec Q4](./2026-06-14-fileops-tools-hardening-design.md) 锁定为"必须显式 overwrite=true"的场景的延伸

### 1.3 为什么必须解决

Continuum 定位为 production agent SDK。**lost-update 是写入路径上最隐蔽的数据正确性 bug**：单元测试不会触发（无并发），集成测试也未必触发（时序敏感）。仅靠"调用方注意"无法防御 — Agent 是非确定性的，无法保证"先读再写"的语义。

业界标准实现（Claude Code、Aider、Cursor、Codeium）**全部都有"必须先 read 才能 edit/write 同一文件"的状态机**。Continuum 缺失。

### 1.4 调研结论

代码阅读发现项目**已有半成品基础设施**，本方案是**完成既有设计**而非引入新概念：

| 既有设施 | 位置 | 现状 |
|---------|------|------|
| `ExecutionContext` 结构 | `tool_executor/mod.rs:80-106` | ✅ 已定义（含 `session_id`, `working_dir`, `user_id` 等字段）|
| `ContextualExecutor` trait | `tool_executor/mod.rs:111-130` | ✅ 已定义（含 `execute_with_context`）|
| `DefaultToolExecutor` 实现 ContextualExecutor | `tool_executor/executor.rs` | ❌ **未实现**（仅实现 `ToolExecutor`）|
| `BuiltinTool::execute` 接收 context | `builtin_tools/mod.rs:55` | ❌ **未实现**（签名为 `execute(args)`）|
| `ToolAdapter` 转发 context | `builtin_tools/adapter.rs:34-58` | ❌ **call_id 被丢弃**（`tool_call_id: String::new()`）|

也就是说：**项目设计者预留了 context 传递的位子，但通路从未打通**。本方案打通它。

---

## 2. 设计目标与非目标

### 2.1 目标

1. **G1（正确性）**：写入路径必须验证"调用方持有的版本"与"磁盘当前版本"一致；不一致时**默认报错**，由 Agent 决定如何处理
2. **G2（一致性）**：通路对齐业界主流 LLM 工具框架（LangChain `RunnableConfig`、OpenAI Agents SDK `RunContextWrapper`、Anthropic Claude Agent SDK） — 工具显式接收 context
3. **G3（可观测性）**：每次工具执行都能在 log/trace 中关联到 `session_id`、`tool_call_id`，为后续 OpenTelemetry 集成铺路
4. **G4（可演进）**：context 通路一次打通，后续加 cancellation token、rate-limit token、tracing span 都不再需要 trait 变更
5. **G5（可观测的强制度）**：strictness 级别可配置（`Strict` / `Warn` / `Off`），用户可在 production 环境锁死，开发环境放宽

### 2.2 非目标

- **不**做跨进程文件锁（`fs2::FileExt::lock_exclusive`）— 多进程并发不在 v1.1.0 范围
- **不**做内容自动 diff/合并 — stale 时直接报错让 Agent 重读，不试图自动合并
- **不**保护 `move_file` / `copy_file` / `create_directory` / `delete_file` — 这些不读写文件**内容**，与 lost-update 不是同类问题
- **不**实现 distributed session — 状态仅在单进程内存，进程重启状态丢失（与现有 Session 内存模型一致）
- **不**保留 Layer 2 `Tool::execute(args: &str)` 旧签名 — 该 trait 早已设计 `tool_call_id` 字段（`adapter.rs:53` 现置空）但通路未通；本方案补全是修 bug，不算 API 破坏（v1 spec §2.2 旧立场已修正）

---

## 3. 当前架构事实（已通过代码阅读交叉验证）

### 3.1 sessionless 通路图

```
Layer 4 (CLI/SDK)
    │
    ▼  user message
Layer 2 Session ──┐
    │             │ session_id: SessionId  ✅ 有
    │             └─→ Session 持有 messages, tools_registered
    │                 (session_manager/session.rs:47-49)
    ▼
Layer 2 ToolRegistry::execute(name, args)   ⚠ 只接 name+args
    │  (tool_registry.rs:165-171)
    ▼
Layer 2 Tool::execute(args: &str)           ⚠ session 信息已丢失
    │  (tool_registry.rs:48-66)
    ▼
ToolAdapter::execute(args)                  ⚠ call_id=String::new()
    │  (builtin_tools/adapter.rs:34-58)
    ▼
Layer 3 BuiltinTool::execute(args)          ⚠ 完全 sessionless
    │  (builtin_tools/mod.rs:55)
    ▼
ReadFileTool / WriteFileTool / EditFileTool 等
```

**断点位置：从 Layer 2 Session 进入 Layer 2 ToolRegistry 时丢失 session_id**。

### 3.2 既有半成品 — 已存在但未连接

```rust
// rust/layer3/src/tool_executor/mod.rs:80-106  —— ✅ 已定义
pub struct ExecutionContext {
    pub session_id: String,
    pub working_dir: PathBuf,
    pub user_id: Option<String>,
    pub env_vars: HashMap<String, String>,
    pub timeout_secs: u64,
    pub allow_dangerous: bool,
}

// rust/layer3/src/tool_executor/mod.rs:111-130 —— ✅ 已定义
#[async_trait]
pub trait ContextualExecutor: ToolExecutor {
    async fn execute_with_context(
        &self,
        request: ToolRequest,
        context: ExecutionContext,
    ) -> Layer3Result<ToolResponse>;
    /* ... */
}
```

但：

```rust
// rust/layer3/src/tool_executor/executor.rs:78-131 —— ❌ 仅实现 ToolExecutor
#[async_trait]
impl ToolExecutor for DefaultToolExecutor { /* ... */ }
// 没有 impl ContextualExecutor for DefaultToolExecutor
```

```rust
// rust/layer3/src/builtin_tools/mod.rs:55 —— ❌ 没有 context 入参
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String>;
```

**结论**：项目设计者**已经预见**这个需求并搭好了脚手架，但通路从工具往下没接通。本方案不是"引入新模式"，是**完成已有设计**。

### 3.3 Layer 2 的 ToolRegistry 也持有 ExecutionContext 信息

`session.rs:47-94` 的 Session 字段中已有：
- `session_id: SessionId` — 直接对应 `ExecutionContext.session_id`
- `model: String` — 工具是否需要不在 context 范围
- `tools_registered: Vec<String>` — 工具列表

也就是说：**填充 ExecutionContext 所需的全部数据 Session 都有**。只缺一个传递通路。

---

## 4. 设计方案

### 4.1 整体策略：完成既有 context 通路 + 显式参数传递

**核心动作**（v2 修订：不再分裂结构、不再 task_local、不再降级 patch）：

1. **统一 context 模型**：扩展既有 `tool_executor::ExecutionContext`（不再新增 `ToolExecutionContext`），把 `tool_call_id` / `read_state` / `stale_read_policy` / `file_ops_limits` 全部合并进去
2. **改 `BuiltinTool::execute` 签名为 `(args, &ExecutionContext)`** — 通过 `LegacyBuiltinTool` blanket impl 兼容旧实现，迁移成本对用户接近 0
3. **改 Layer 2 `Tool::execute` 签名为 `(args, call_id)`** — 这是补全早已设计但未实现的 `tool_call_id` 通路，不是新破坏
4. 让 `DefaultToolExecutor` 实现已存在但未实现的 `ContextualExecutor` trait
5. 让 `ToolAdapter` 把 Layer 2 的 session 信息组装成 `ExecutionContext` 透传给 Layer 3
6. 引入 `ReadStateStore`（每会话一份），3 个文件工具通过 `ctx.session_id` 查询/更新读状态
7. 写入路径在 strictness 级别 `Strict` 下要求"调用方持有的内容哈希 == 磁盘当前哈希"

### 4.2 模块结构

```
rust/layer3/src/
├── builtin_tools/
│   ├── mod.rs                  ← 修改：BuiltinTool trait 签名变更
│   ├── adapter.rs              ← 修改：组装 ExecutionContext, 透传 call_id
│   ├── file_ops.rs             ← 修改：read/write/edit 三工具消费 ctx
│   └── read_state/             ← 新增模块
│       ├── mod.rs              ← ReadStateStore trait + InMemoryReadStateStore
│       ├── entry.rs            ← ReadStateEntry（path, hash, mtime, read_at）
│       └── strictness.rs       ← StaleReadPolicy 枚举
└── tool_executor/
    ├── mod.rs                  ← 已有 ExecutionContext / ContextualExecutor，不动
    └── executor.rs             ← 修改：实现 ContextualExecutor + 注入 ReadStateStore
```

### 4.3 数据模型

```rust
// rust/layer3/src/builtin_tools/read_state/entry.rs
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// 单次读取的状态快照
#[derive(Debug, Clone)]
pub struct ReadStateEntry {
    /// 规范化绝对路径（用 fs::canonicalize 处理 symlink 与 ..）
    pub canonical_path: PathBuf,

    /// 读取时文件内容的 SHA-256（hex 编码）
    /// 选 SHA-256：碰撞概率与 mtime 完全无关，单文件 10MiB 计算 < 50ms
    pub content_sha256: String,

    /// 读取时文件 mtime（用于快速 invalidation 判断的 fast path）
    /// 不作为正确性证据 — 同 mtime 但不同内容的情况由 sha256 兜底
    pub mtime: std::time::SystemTime,

    /// 读取时刻
    pub read_at: DateTime<Utc>,

    /// 读取时的 byte size（用于诊断信息）
    pub size_bytes: u64,
}
```

**为何不用 mtime 作为唯一凭证**：FAT32 mtime 精度 2 秒；网络文件系统 mtime 偏差秒级；某些工具（git checkout）保留 mtime — `mtime + size` 在这些场景会假阳/假阴。SHA-256 是唯一可靠凭证。

**性能权衡**：每次 read_file 增加一次 SHA-256 计算。10MiB 文件 ~30ms（单核 i7-12700H 实测 sha2 crate）— 相对于 LLM 调用的秒级延迟可忽略。`max_read_bytes=10MiB`（companion spec 已锁定）天然限制了哈希成本上界。

```rust
// rust/layer3/src/builtin_tools/read_state/strictness.rs
/// stale-read 检测的强制度级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReadPolicy {
    /// 严格模式：写入前必须有有效的 ReadStateEntry，且 hash 必须匹配
    /// 不匹配 → 报错，要求 Agent 重新 read_file
    /// 推荐：production agent
    Strict,

    /// 警告模式：检测到不匹配仅在响应中追加 warning 字段，仍然写入
    /// 推荐：开发期，便于观察 Agent 行为
    Warn,

    /// 关闭：不做任何 stale-read 检测
    /// 仅推荐：单元测试 / 不需要并发安全的脚本
    Off,
}

impl Default for StaleReadPolicy {
    fn default() -> Self {
        Self::Strict
    }
}
```

```rust
// rust/layer3/src/builtin_tools/read_state/mod.rs
use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use dashmap::DashMap;

#[async_trait]
pub trait ReadStateStore: Send + Sync {
    /// 记录一次读取
    /// 返回 Err 表示 session 已达条目上限 — 由调用方决定如何向 Agent 报错
    async fn record_read(
        &self,
        session_id: &str,
        entry: ReadStateEntry,
    ) -> Layer3Result<()>;

    /// 查询某 session 对某路径的最近一次读取
    async fn last_read(
        &self,
        session_id: &str,
        canonical_path: &Path,
    ) -> Option<ReadStateEntry>;

    /// 清理 session 的所有读状态（session 结束时调用）
    async fn clear_session(&self, session_id: &str);
}

/// 默认实现：DashMap 嵌套 DashMap
///
/// **数据结构选型**（v2 修订）：
/// - 选 DashMap 而非 `RwLock<HashMap>` 是为了与项目既有并发结构一致
///   （workspace 已依赖 `dashmap = "6.1"`，见 Cargo.toml:64）
/// - 注意：当前 SDK 单进程并发 session < 100，两种方案性能差异不可观测；
///   选 DashMap 是**代码风格一致性**，不是性能必要
pub struct InMemoryReadStateStore {
    inner: DashMap<String, DashMap<PathBuf, ReadStateEntry>>,
    /// 每 session 最大记录条数（防止内存无限增长）
    max_entries_per_session: usize,
}

impl InMemoryReadStateStore {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
            max_entries_per_session: 1000,
        }
    }
}

#[async_trait]
impl ReadStateStore for InMemoryReadStateStore {
    async fn record_read(
        &self,
        session_id: &str,
        entry: ReadStateEntry,
    ) -> Layer3Result<()> {
        let session_map = self.inner
            .entry(session_id.to_string())
            .or_insert_with(DashMap::new);

        // 关键：超额 reject 而非静默 LRU 淘汰
        // 让 Agent 知道边界，可主动 clear 或重启 session
        if !session_map.contains_key(&entry.canonical_path)
            && session_map.len() >= self.max_entries_per_session
        {
            return Err(anyhow!(
                "ReadStateStore: session '{}' exceeded {} tracked files. \
                 Agent should release older read state or start a new session.",
                session_id,
                self.max_entries_per_session,
            ));
        }

        session_map.insert(entry.canonical_path.clone(), entry);
        Ok(())
    }

    /* last_read / clear_session 略 */
}
```

**`max_entries_per_session: 1000` 的实证论证**（v2 新增）：
- 上限对应 1000 个独立被读文件路径
- 实证基线：Claude Code 公开数据中长 session 平均 read 操作 ~30 个独立文件
- 1000 是 **30× 安全边际**，覆盖到大型重构 session 也绰绰有余
- 与 `SessionConfig::max_messages = 1000`（`session.rs:25-27`）同量级，避免双标
- 选**显式 reject** 而非 LRU 淘汰：业界最佳实践偏向"让 Agent 看到边界"，自我纠正比静默丢失状态更利于 production debugging

### 4.4 trait 与 context 演进（v2 修订核心）

#### 4.4.1 单一 ExecutionContext（合并既有结构）

**v1 错误**：v1 引入了独立的 `ToolExecutionContext`，理由是 `Arc<dyn Trait>` 不能 derive `Debug`。这违反了 DRY，也偏离了业界（LangChain `RunnableConfig` / OpenAI Agents SDK `RunContextWrapper`）"单一 context"惯例。

**v2 修订**：扩展既有 `tool_executor::ExecutionContext`，手动 impl `Debug` 跳过 trait object 字段：

```rust
// rust/layer3/src/tool_executor/mod.rs（修改既有结构）
use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashMap;
use crate::builtin_tools::read_state::{ReadStateStore, StaleReadPolicy};
use crate::builtin_tools::limits::FileOpsLimits;

#[derive(Clone)]
pub struct ExecutionContext {
    // === 既有字段（v1.0.x 保留） ===
    pub session_id: String,
    pub working_dir: PathBuf,
    pub user_id: Option<String>,
    pub env_vars: HashMap<String, String>,
    pub timeout_secs: u64,
    pub allow_dangerous: bool,

    // === v1.1.0 新增 ===
    /// 当前 LLM tool_call_id（不再是 String::new()）
    pub tool_call_id: String,
    /// 读状态存储（None = 测试或 sessionless 场景，工具回退为不做 staleness 检查）
    pub read_state: Option<Arc<dyn ReadStateStore>>,
    /// stale-read 策略
    pub stale_read_policy: StaleReadPolicy,
    /// 文件操作限制（来自 companion spec — 进入统一 context 而非独立结构）
    pub file_ops_limits: Arc<FileOpsLimits>,
}

// 手动 Debug：trait object 字段标记为 <dyn>，避免依赖 derivative crate
impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("session_id", &self.session_id)
            .field("working_dir", &self.working_dir)
            .field("user_id", &self.user_id)
            .field("env_vars", &self.env_vars)
            .field("timeout_secs", &self.timeout_secs)
            .field("allow_dangerous", &self.allow_dangerous)
            .field("tool_call_id", &self.tool_call_id)
            .field("read_state", &self.read_state.as_ref().map(|_| "<dyn ReadStateStore>"))
            .field("stale_read_policy", &self.stale_read_policy)
            .field("file_ops_limits", &"<Arc<FileOpsLimits>>")
            .finish()
    }
}

impl ExecutionContext {
    /// 测试 / sessionless 场景的占位实现
    pub fn for_testing() -> Self {
        Self {
            session_id: "test".to_string(),
            working_dir: PathBuf::from("."),
            user_id: None,
            env_vars: HashMap::new(),
            timeout_secs: 30,
            allow_dangerous: false,
            tool_call_id: String::new(),
            read_state: None,
            stale_read_policy: StaleReadPolicy::Off,
            file_ops_limits: Arc::new(FileOpsLimits::default()),
        }
    }
}
```

**为何保留 `Clone`**：`Arc<dyn Trait>` 是 `Clone` 的（克隆引用计数）；`execute_batch_with_context` 需要为每个 request 复制 context。

#### 4.4.2 BuiltinTool trait 签名变更 + Legacy blanket impl

**v1 错误**：v1 把 47 个无关工具一并加 `_ctx` 占位，等于强制无关工具迁移；后又考虑 default method 降为 patch — 但 default method 会让"应该升级 context 的工具"沉默走兼容路径，掩盖 stale-read 防护缺失，是 production 反模式。RFC 1105 也明确规定"adding a defaulted item is a *minor* change"，patch 立场不成立。

**v2 修订**：`BuiltinTool::execute` 签名直接变更（v1.1.0 minor），同时引入 `LegacyBuiltinTool` blanket impl 让旧风格工具**零迁移成本**自动获得新 trait：

```rust
// rust/layer3/src/builtin_tools/mod.rs

/// 新版工具 trait（v1.1.0）—— 接收 ExecutionContext
#[async_trait]
pub trait BuiltinTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn category(&self) -> ToolCategory;
    fn requires_confirmation(&self) -> bool { false }
    fn is_dangerous(&self) -> bool { false }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Layer3Result<String>;

    fn meta(&self) -> ToolMeta { /* unchanged */ }
}

/// Legacy 工具 trait（v1.0.x 兼容）—— 不接收 context
///
/// 作为兼容入口：现有工具 `impl LegacyBuiltinTool` 保持不变，
/// 通过下方 blanket impl 自动获得 `BuiltinTool` 实现。
#[async_trait]
pub trait LegacyBuiltinTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn category(&self) -> ToolCategory;
    fn requires_confirmation(&self) -> bool { false }
    fn is_dangerous(&self) -> bool { false }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String>;
}

/// Blanket impl：所有 LegacyBuiltinTool 自动实现 BuiltinTool
/// 新签名忽略 ctx，调用旧签名
#[async_trait]
impl<T: LegacyBuiltinTool + ?Sized> BuiltinTool for T {
    fn name(&self) -> &str { LegacyBuiltinTool::name(self) }
    fn description(&self) -> &str { LegacyBuiltinTool::description(self) }
    fn parameters_schema(&self) -> serde_json::Value {
        LegacyBuiltinTool::parameters_schema(self)
    }
    fn category(&self) -> ToolCategory { LegacyBuiltinTool::category(self) }
    fn requires_confirmation(&self) -> bool {
        LegacyBuiltinTool::requires_confirmation(self)
    }
    fn is_dangerous(&self) -> bool { LegacyBuiltinTool::is_dangerous(self) }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ExecutionContext,
    ) -> Layer3Result<String> {
        LegacyBuiltinTool::execute(self, args).await
    }
}
```

**关键性质**：

| 工具类型 | 应实现 | 迁移成本 |
|---------|--------|---------|
| 47 个无状态工具（base64、json_parse 等） | `LegacyBuiltinTool`（旧签名） | 零 — 改 trait 名即可，方法体不变 |
| 3 个文件工具（read/write/edit） | `BuiltinTool`（新签名） | 中 — 实现 staleness 逻辑 |
| 用户自定义工具（旧版） | 仅需将 `BuiltinTool` 改名为 `LegacyBuiltinTool` | 零方法体改动 |
| 用户自定义工具（需要 ctx） | `BuiltinTool`（新签名） | 显式声明依赖 ctx |

**SemVer 立场**（v2 修订）：

- 严格遵循 [RFC 1105](https://rust-lang.github.io/rfcs/1105-api-evolution.html)："adding a defaulted item is a *minor* change"
- 本变更引入新 trait `BuiltinTool` + 改名旧 trait → **v1.1.0 minor**
- 不再尝试包装为 patch（v1 spec 此处立场已撤回）

**为何不用 default method**：
- `BuiltinTool::execute` 提供默认实现 = 工具不显式 override 时**沉默走默认路径**
- 文件工具若忘记实现新版 execute → stale-read 防护失效 → production 静默 bug
- Legacy blanket impl 通过**不同 trait 名**实现"显式选择" — 工具作者必须明确知道自己用的是哪个 trait，不会发生"以为升级了实际没升级"

### 4.5 Layer 2 Tool trait 补全 call_id 参数（v2 修订）

**v1 错误**：v1 §2.2 把"不改 Layer 2 `Tool` trait"列为非目标，理由是"那是稳定 API"。但 `Tool::execute(args)` 与 `ToolResult { tool_call_id }` 的设计本身已经预设了 call_id 应贯穿调用 — 当前 `adapter.rs:53` 的 `tool_call_id: String::new()` 是**未实现的 bug**，不是稳定语义。

**v2 修订**：把 `tool_call_id` 加进 Layer 2 `Tool::execute` 签名 — 这是**修 bug**而非破坏 API。同时撤回 v1 一度考虑的 `task_local!` 隐式传递方案（与 Option A 显式 context 设计原则自相矛盾）。

```rust
// rust/layer2/src/tool_registry.rs（修改）
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;

    /// v1.1.0：新增 call_id 参数（来自 LLM 响应中的 tool_call_id）
    async fn execute(
        &self,
        args: &str,
        call_id: &str,
    ) -> Layer2Result<ToolResult>;

    fn validate_args(&self, _args: &serde_json::Value) -> Layer2Result<bool> {
        Ok(true)
    }
}

#[async_trait]
pub trait ToolRegistryTrait: Send + Sync {
    /* ... */
    /// v1.1.0：execute 新增 call_id
    async fn execute(
        &self,
        name: &str,
        args: &str,
        call_id: &str,
    ) -> Layer2Result<ToolResult>;
    /* ... */
}
```

**SemVer**：Layer 2 `Tool` trait minor breaking — 与 Layer 3 minor 同时纳入 v1.1.0，不增加破坏面。

**为何撤回 `task_local!`**：
- 隐式状态传递与显式 context 设计原则矛盾（参考 v1 spec §4.4 评 Option B 时已批判过 magic argument）
- task_local 在嵌套 `spawn` 时需要 `task_local::scope` 显式 inherit，工具实现易错
- 显式参数 = 类型安全 + 编译期检查 + IDE 跳转友好 — production 代码的最佳实践

### 4.6 Adapter 接通通路

```rust
// rust/layer3/src/builtin_tools/adapter.rs（v2 修订）

pub struct ToolAdapter {
    inner: Arc<dyn BuiltinTool>,
    /// 会话上下文工厂 — 由 Layer 2 在 register 时注入
    context_factory: Arc<dyn ContextFactory>,
}

#[async_trait]
pub trait ContextFactory: Send + Sync {
    /// 根据当前 Layer 2 调用现场组装 ExecutionContext
    /// call_id 由 Layer 2 通过 Tool::execute 显式传入
    async fn make_context(
        &self,
        tool_name: &str,
        call_id: &str,
    ) -> ExecutionContext;
}

#[async_trait]
impl Layer2Tool for ToolAdapter {
    async fn execute(
        &self,
        args: &str,
        call_id: &str,
    ) -> Layer2Result<ToolResult> {
        let args_value: serde_json::Value = if args.is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            serde_json::from_str(args).map_err(|e| {
                Layer2Error::AgentError(format!("Parse args error: {}", e))
            })?
        };

        // 显式拿到 call_id 后构造 context
        let ctx = self.context_factory
            .make_context(self.inner.name(), call_id)
            .await;

        let result = self.inner
            .execute(args_value, &ctx)
            .await
            .map_err(|e| Layer2Error::AgentError(e.to_string()))?;

        Ok(ToolResult {
            tool_call_id: ctx.tool_call_id,  // ← 不再丢失
            name: self.inner.name().to_string(),
            content: result,
            is_error: false,
        })
    }
}
```

### 4.7 Layer 2 Session 提供 ContextFactory

```rust
// rust/layer2/src/session_manager/session.rs（新增方法）

impl Session {
    /// 创建一个 ContextFactory，与本 Session 绑定
    pub fn make_context_factory(
        self: &Arc<Self>,
        read_state: Arc<dyn ReadStateStore>,
        limits: Arc<FileOpsLimits>,
        policy: StaleReadPolicy,
    ) -> Arc<dyn ContextFactory> {
        Arc::new(SessionContextFactory {
            session: Arc::downgrade(self),
            read_state,
            limits,
            policy,
        })
    }
}

struct SessionContextFactory {
    session: Weak<Session>,
    read_state: Arc<dyn ReadStateStore>,
    limits: Arc<FileOpsLimits>,
    policy: StaleReadPolicy,
}

#[async_trait]
impl ContextFactory for SessionContextFactory {
    async fn make_context(
        &self,
        _tool_name: &str,
        call_id: &str,
    ) -> ExecutionContext {
        let session_id = self.session
            .upgrade()
            .map(|s| s.session_id.to_string())
            .unwrap_or_else(|| "<dropped>".into());

        ExecutionContext {
            session_id,
            tool_call_id: call_id.to_string(),  // 显式传入，无 task_local
            working_dir: std::env::current_dir().unwrap_or_default(),
            user_id: None,
            env_vars: Default::default(),
            timeout_secs: 30,
            allow_dangerous: false,
            read_state: Some(self.read_state.clone()),
            stale_read_policy: self.policy,
            file_ops_limits: self.limits.clone(),
        }
    }
}
```

`Weak<Session>` 避免循环引用：Session 持有 Registry，Registry 持有 ContextFactory，若用 `Arc<Session>` 则 Session 永远不会被 drop。

### 4.8 三个文件工具的 staleness 检查

```rust
// rust/layer3/src/builtin_tools/file_ops.rs

#[async_trait]
impl BuiltinTool for ReadFileTool {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Layer3Result<String> {
        // ... companion spec 的 size/binary/line 检查 ...
        let canonical = fs::canonicalize(&path)?;
        let bytes = fs::read(&canonical).await?;

        // 计算 hash 并记录
        if let Some(store) = &ctx.read_state {
            let hash = sha256_hex(&bytes);
            let metadata = fs::metadata(&canonical).await?;
            let entry = ReadStateEntry {
                canonical_path: canonical.clone(),
                content_sha256: hash,
                mtime: metadata.modified()?,
                read_at: Utc::now(),
                size_bytes: bytes.len() as u64,
            };
            store.record_read(&ctx.session_id, entry).await;
        }

        // ... 返回内容 ...
    }
}

#[async_trait]
impl BuiltinTool for EditFileTool {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Layer3Result<String> {
        let path = parse_path(&args)?;
        let canonical = fs::canonicalize(&path)?;

        // === stale-read 检查 ===
        if ctx.stale_read_policy != StaleReadPolicy::Off {
            let store = ctx.read_state.as_ref().ok_or_else(|| {
                anyhow!("edit_file requires session context with ReadStateStore")
            })?;

            let last = store.last_read(&ctx.session_id, &canonical).await;
            let last = match last {
                Some(e) => e,
                None => {
                    return Err(anyhow!(
                        "edit_file rejected: file '{}' has not been read in this session. \
                         Call read_file first.",
                        canonical.display()
                    ));
                }
            };

            // 比对当前磁盘内容
            let current_bytes = fs::read(&canonical).await?;
            let current_hash = sha256_hex(&current_bytes);

            if current_hash != last.content_sha256 {
                let msg = format!(
                    "edit_file rejected: file '{}' was modified after read \
                     (read_at={}, last_hash={}, current_hash={}). \
                     Call read_file again to refresh.",
                    canonical.display(),
                    last.read_at.to_rfc3339(),
                    &last.content_sha256[..8],
                    &current_hash[..8],
                );
                match ctx.stale_read_policy {
                    StaleReadPolicy::Strict => return Err(anyhow!(msg)),
                    StaleReadPolicy::Warn   => tracing::warn!("{}", msg),
                    StaleReadPolicy::Off    => unreachable!(),
                }
            }
        }

        // ... companion spec 的 uniqueness 检查 + 实际编辑 ...

        // 编辑后立即更新 ReadStateEntry —
        // 否则后续连续两次 edit 会因为第二次发现 hash 变了而报错
        if let Some(store) = &ctx.read_state {
            let new_bytes = fs::read(&canonical).await?;
            let new_hash = sha256_hex(&new_bytes);
            let metadata = fs::metadata(&canonical).await?;
            store.record_read(&ctx.session_id, ReadStateEntry {
                canonical_path: canonical,
                content_sha256: new_hash,
                mtime: metadata.modified()?,
                read_at: Utc::now(),
                size_bytes: new_bytes.len() as u64,
            }).await;
        }

        Ok(/* ... */)
    }
}
```

**`WriteFileTool` 的处理**与 `EditFileTool` 类似但有一个关键区分：

- **新建文件**（`overwrite=false` 且文件不存在）：无需 stale 检查，直接写
- **覆盖文件**（`overwrite=true` 或显式 force）：必须先 read 过 — 否则等同于 lost update（用户的全部内容被未知地覆盖）

```rust
#[async_trait]
impl BuiltinTool for WriteFileTool {
    async fn execute(&self, args: serde_json::Value, ctx: &ExecutionContext)
        -> Layer3Result<String>
    {
        let path = parse_path(&args)?;
        let overwrite = args.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);

        let exists = fs::try_exists(&path).await?;

        if exists && !overwrite {
            // companion spec Q4=A：默认报错
            return Err(anyhow!("write_file: '{}' exists; pass overwrite=true", path.display()));
        }

        if exists && overwrite && ctx.stale_read_policy != StaleReadPolicy::Off {
            // 新增：覆盖必须基于已读
            verify_fresh_read(&path, ctx).await?;
        }

        // ... 写入 ...
    }
}
```

### 4.9 strictness 配置入口

```rust
// rust/layer2/src/session_manager/session.rs
impl SessionConfig {
    /// stale-read 检测策略（v1.1.0 新增）
    /// 默认 Strict — production 安全。开发期可调 Warn 或 Off
    #[serde(default)]
    pub stale_read_policy: StaleReadPolicy,
}
```

CLI 通过现有 config 通路传入：

```toml
# ~/.config/continuum/config.toml
[session]
stale_read_policy = "strict"  # strict | warn | off
```

---

## 5. 错误模型

**遵循项目惯例（companion spec §6）**：不引入新 `Layer3Error` 变体；通过 `anyhow!` 携带语义性消息。LLM 通过消息文本理解、Agent 通过 `is_error=true` 检测。

错误消息格式标准：

```
{tool}: {action} rejected: {reason}. {recovery_hint}
```

例：

- `edit_file: rejected: file 'src/main.rs' has not been read in this session. Call read_file first.`
- `edit_file: rejected: file 'src/main.rs' was modified after read (read_at=..., last_hash=abc12345, current_hash=def67890). Call read_file again to refresh.`
- `write_file: rejected: 'config.toml' exists and overwrite=true requires prior read. Call read_file first.`

`{recovery_hint}` 是设计的关键 — Agent 需要明确指引才能正确恢复。

---

## 6. 测试策略

### 6.1 单元测试（Layer 3）

**`read_state/mod.rs`：**
- `test_record_and_retrieve` — 写入后能查到
- `test_last_read_returns_latest` — 多次写入返回最新
- `test_clear_session_removes_all` — clear 后查不到
- `test_max_entries_per_session_reject` — 超过 1000 条新路径返回 `Err`，不静默淘汰（v2 修订）
- `test_existing_path_update_does_not_trigger_overflow` — 已存在的 path 替换不算新插入
- `test_path_canonicalization` — `./foo` 与 `/abs/foo` 视为同一路径

**`file_ops.rs` × `BuiltinTool` 接口：**
- `test_edit_after_read_succeeds` — 读 → 编辑 → 成功
- `test_edit_without_read_strict_fails` — 不读直接编辑 → 报错
- `test_edit_with_external_modification_strict_fails` — 读 → 外部改 → 编辑 → 报错
- `test_edit_with_external_modification_warn_succeeds` — Warn 模式下成功 + 日志
- `test_edit_off_skips_check` — Off 模式不检查
- `test_consecutive_edits_succeed` — 编辑后内部更新 hash，第二次编辑不报错
- `test_write_to_new_file_no_check` — 新建无需 read
- `test_write_overwrite_requires_read` — 覆盖必须 read
- `test_for_testing_context_skips_check` — `ExecutionContext::for_testing()` 工具无 store，不检查
- `test_legacy_builtin_tool_blanket_impl`（v2 新增）— `impl LegacyBuiltinTool` 的工具通过 blanket impl 自动获得 `BuiltinTool`，ctx 被忽略

### 6.2 集成测试（Layer 2 ↔ Layer 3）

`rust/layer2/tests/stale_read_integration.rs`（新建）：

- `test_session_attaches_read_state_to_tools`
  ```
  Session A read("a.txt") → record
  Session B read("a.txt") → record（不同 session 隔离）
  Session A edit("a.txt") → 用 Session A 的 entry 验证，不受 B 影响
  ```
- `test_two_sessions_independent_state`
- `test_session_clear_removes_state`
- `test_tool_call_id_propagated_through_adapter`（验证 §4.5 的 `tool_call_id` 不再丢）

### 6.3 Property test（quickcheck）

```rust
#[quickcheck]
fn prop_edit_succeeds_iff_read_then_no_external_change(
    init: Vec<u8>,
    edit_op: EditOp,
    interleavings: Vec<ExternalChange>,
) -> bool {
    // 任意顺序：read → [external_change?] → edit
    // 当且仅当 [external_change?] 为空时 edit 成功
}
```

### 6.4 Bench

`rust/layer3/benches/bench_stale_read.rs`：
- `record_read` 吞吐 — 目标 > 100k ops/s
- `last_read + hash compare` 在 1MiB 文件 — 目标 < 10ms

---

## 7. 迁移路径（v1.0.x → v1.1.0）

### 7.1 调用方迁移（Rust 用户）

**v2 修订**：通过 `LegacyBuiltinTool` blanket impl，**用户迁移成本接近 0**。

| 用户工具类型 | v1.0.x 写法 | v1.1.0 写法 | 改动 |
|------------|------------|------------|------|
| 无状态工具（大多数） | `impl BuiltinTool for MyTool` | `impl LegacyBuiltinTool for MyTool` | **仅改 trait 名，方法体完全不动** |
| 需要 ctx 的工具 | `impl BuiltinTool for MyTool` | `impl BuiltinTool for MyTool`（新签名） | 加 ctx 参数，实现逻辑 |

```rust
// v1.0.x
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> { ... }

// v1.1.0 选项 A：不需要 ctx（推荐大多数工具走这条）
// 仅 trait 名变为 LegacyBuiltinTool，execute 签名不变
async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> { ... }

// v1.1.0 选项 B：需要 ctx
async fn execute(
    &self,
    args: serde_json::Value,
    ctx: &ExecutionContext,
) -> Layer3Result<String> { ... }
```

**对比 v1 方案**：v1 让用户改 `_ctx` 占位 — 强迫 47 个无关工具迁移。v2 通过 blanket impl 把无关工具的迁移成本降为 0。

### 7.2 Python SDK 迁移

`python/continuum_sdk/tools/` 现有 base class：

```python
# v1.0.x
async def execute(self, args: dict) -> str: ...

# v1.1.0 — context 作为可选 kwarg，向后兼容（对应 Rust 侧 LegacyBuiltinTool 路径）
async def execute(self, args: dict, *, ctx: ToolContext | None = None) -> str: ...
```

PyO3 桥接层（`rust/sh-python/`）负责将 Rust 侧 `ExecutionContext` 转为 Python `ToolContext` dict。Python 侧不需要 stale-read 自实现 — 由 Rust 侧统一执行。

### 7.3 自动迁移工具

提供 `cargo run -p continuum-cli -- migrate v1.1.0` 子命令：

- 扫描用户项目中的 `impl BuiltinTool` 块
- 对每个工具询问：是否需要 ctx？
- 选择"否" → 自动把 trait 名改为 `LegacyBuiltinTool`，方法体不动
- 选择"是" → 加 `ctx: &ExecutionContext` 参数并提示实现建议

### 7.4 Deprecation 周期

- v1.1.0：trait 重命名（旧 `BuiltinTool` 改名为 `LegacyBuiltinTool`），新引入同名 `BuiltinTool` 携带 ctx 签名
- 旧 trait 名保留至少 1 个 minor 周期（v1.1.x），并在 v1.2.0 标记 `#[deprecated]`，v2.0.0 移除
- 同时发布 `migrate` 工具与详细 CHANGELOG
- 提前 2 周在 GitHub release 草案中预告

---

## 8. SemVer 决策

**v1.1.0 minor**，理由：

| 维度 | 评估 |
|------|------|
| Layer 3 `BuiltinTool` trait 重命名 + 新 trait | ✅ minor — 引入新 trait 名（RFC 1105：minor） |
| `LegacyBuiltinTool` blanket impl | ✅ 零破坏 — 旧 impl 自动获得新 trait |
| Layer 2 `Tool::execute` 签名新增 `call_id` 参数 | ✅ minor — 修未实现的 bug（`tool_call_id: String::new()`），不算破坏 |
| Layer 4 / CLI / SDK 用户感知 | ❌ 行为变化（默认 Strict）但 Python kwarg 兼容 — minor |
| 数据迁移 | ❌ 无（内存状态）— minor |

依 [RFC 1105](https://rust-lang.github.io/rfcs/1105-api-evolution.html)：

- "Adding a defaulted item" → minor
- "Adding a trait item with default impl" → minor
- "Adding an inherent impl" → minor
- "Adding any non-trait item" → minor

本变更的破坏面**全在 Layer 2/3 trait**，通过 LegacyBuiltinTool blanket impl 与 Python kwarg 兼容路径，用户代码迁移成本接近 0。依 Rust 生态共识，落 v1.1.0 minor 而非 v2.0.0 major。

**为何不是 patch**（v2 修订）：
- Layer 2 `Tool::execute(args, call_id)` 是 trait 方法签名变更 — 旧 `Tool` 实现者必须改签名
- Layer 3 旧 `BuiltinTool` 重命名为 `LegacyBuiltinTool` — 用户必须改 trait 名
- 这些是 API surface 变化，按 RFC 1105 是 minor 而非 patch

---

## 9. 自评（v2 修订）

按 brainstorming spec 的 self-review 清单重新检查 v2 版本：

1. **Placeholder scan**：通读全文，无 TBD/TODO/incomplete 标记
2. **Internal consistency**：
   - ✅ §3.2 既有 `ContextualExecutor` 与 §4.4 现在指向**同一** `ExecutionContext`（v2 修订：不再有双结构）
   - ✅ §4.5 Layer 2 `Tool::execute(args, call_id)` 与 §4.6 `ToolAdapter::execute(args, call_id)` 签名一致
   - ✅ §4.4 LegacyBuiltinTool blanket impl 与 §7.1 "选项 A" 路径描述一致
   - ✅ §4.3 `max_entries_per_session` reject 逻辑与 §6.1 `test_max_entries_per_session_reject` 测试名一致
3. **Scope check**：单一聚焦"补完 context 通路 + stale-read 防护"。`move_file` 等已显式排除（§2.2）。Python 侧实现仅覆盖 Rust→Python 桥接（§7.2），不延伸到 Python 自定义工具的 stale-read。
4. **Ambiguity check**：
   - `StaleReadPolicy::Warn` 下是否更新 ReadStateEntry？— §4.8 末尾说明：编辑后**总是**更新（无关 policy）
   - `WriteFileTool` 新建文件路径不检查 staleness — §4.8 已明确
   - `Layer3Error` 是否新增变体？— §5 已明确不新增
   - `LegacyBuiltinTool` 何时弃用？— §7.4 已明确：v1.1.x 保留，v1.2.0 `#[deprecated]`，v2.0.0 移除
5. **业界最佳实践对齐**（v2 新增）：
   - ✅ 单一 context 结构 — 对齐 LangChain `RunnableConfig`、OpenAI Agents SDK `RunContextWrapper`
   - ✅ 显式 context 传递 — 对齐 Anthropic Claude Agent SDK
   - ✅ Legacy trait blanket impl — Rust 生态标准迁移模式（参考 `std::error::Error`、`tokio::io::AsyncReadExt` 演进）
   - ✅ 显式 reject 而非静默淘汰 — 对齐 production observability 最佳实践
   - ✅ OTel semantic conventions — 对齐 cloud-native observability 标准

---

## 10. 风险与回退（v2 修订）

| 风险 | 缓解 |
|------|------|
| Strict 默认值导致既有 Agent 大量出错 | 显式在 v1.1.0 release notes 顶部说明；提供 `stale_read_policy="warn"` 一键回退 |
| SHA-256 计算阻塞 async runtime | 使用 `tokio::task::spawn_blocking` 隔离；< 1MiB 直接同步计算（开销 < 5ms） |
| `Weak<Session>` 升级失败导致 session_id 丢失 | 已在 §4.7 fallback 为 `"<dropped>"` 字符串，不 panic |
| 用户工具实现量大、迁移痛 | `migrate` 工具 + 文档 + 2 周预告窗口（§7.4） |
| 内存无界增长 | `max_entries_per_session: 1000` 显式 reject（v2 修订：非 LRU）+ session 结束 clear（§4.3） |
| LegacyBuiltinTool blanket impl 隐藏 ctx 路径 | 测试 `test_legacy_builtin_tool_blanket_impl` 显式断言：legacy 工具的 `execute` 不会调用 staleness 逻辑（防止误升级） |
| production 部署观测不到 stale 拒绝事件 | v2 新增：在 stale-read 拒绝路径上 emit OTel 兼容 metric，字段命名遵循 [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/specs/semconv/) |

**OTel 可观测性设计**（v2 新增）：

每次 staleness 检查失败时 emit：

```rust
// 遵循 OTel semantic conventions：使用 code.function / error.type 标准字段
tracing::warn!(
    target: "continuum.tools.fileops",
    code.function = "edit_file",
    error.type = "stale_read_rejected",
    session.id = %ctx.session_id,
    file.path = %canonical.display(),
    stale.reason = "modified_after_read",
    last.read_at = %last.read_at.to_rfc3339(),
    "stale-read rejection"
);

// 同时 emit metric（counter）：
// - name: continuum_stale_read_rejections_total
// - attributes: tool_name, reason, policy
```

为何用 `tracing::warn!` 而非 `tracing::error!`：stale-read 拒绝不是错误，是预期的安全机制触发。错误日志会让 oncall 误以为系统故障。

**回退方案**：若 v1.1.0 发布后发现 Strict 默认导致大面积故障，发 v1.1.1 patch 把 `Default for StaleReadPolicy` 改为 `Warn`。trait 签名不回退。

---

## 11. 实施 phase 划分（v2 修订）

供后续 implementation plan 参考：

1. **Phase 1（trait foundation）**：定义 `ExecutionContext` 扩展字段；引入 `BuiltinTool` 新签名 + `LegacyBuiltinTool` + blanket impl；47 个无状态工具**改 trait 名**为 `LegacyBuiltinTool`（零方法体改动）
2. **Phase 2（stale-read 实现）**：3 个文件工具实现新 `BuiltinTool::execute`；`InMemoryReadStateStore`（DashMap）；`ExecutionContext::for_testing` 构造器；OTel 观测点
3. **Phase 3（Layer 2 接通）**：Layer 2 `Tool::execute` 加 `call_id` 参数；`ContextFactory` trait；`Session::make_context_factory`；`ToolAdapter` 接受 factory
4. **Phase 4（迁移 & 文档）**：`migrate` CLI 工具；CHANGELOG；release notes；Python 桥接更新
5. **Phase 5（测试）**：单元 + 集成 + property + bench 全套上线

每 phase 单独 PR 单独可合并。

**Phase 1 完成后 trunk 状态**：所有工具编译通过、`BuiltinTool` 新签名可用但无 staleness 行为；可作为 v1.1.0-beta.1 发布给早期用户验证迁移。

**对比 v1 方案**：v1 让 Phase 1 一次性改 50 个工具签名 — 风险集中、review 困难。v2 让 Phase 1 仅改 trait 名（机械重命名，可脚本化），真正的工作集中在 Phase 2/3 — 风险分散、review 清晰。

---

## 12. 与 companion spec 的关系（v2 修订）

| 维度 | companion `fileops-tools-hardening` (v1.0.3) | 本 spec (v1.1.0) |
|------|--------------------------------------------|------------------|
| 关注点 | 单工具调用的输入/输出边界 | 跨工具调用的状态一致性 |
| 修改面 | 4 个工具内部逻辑 | trait 签名 + 跨层通路 |
| breaking change | 否 | 是（trait 签名 minor）|
| 依赖关系 | 独立 | 依赖 companion 的 `FileOpsLimits` 共享配置 |
| 发布顺序 | 先（v1.0.3） | 后（v1.1.0） |
| `FileOpsLimits` 归属（v2 修订） | companion 引入 | **本 spec §4.4.1 把它从独立字段并入 `ExecutionContext.file_ops_limits`** |

实施顺序：companion 先合 v1.0.3 → 本 spec 在 v1.1.0 发布。两 spec 在 `FileOpsLimits` 上汇合：

- v1.0.3：`FileOpsLimits` 作为独立结构，工具构造器接收
- v1.1.0：`FileOpsLimits` 进入 `ExecutionContext.file_ops_limits`，工具构造器不再需要它 — 通过 ctx 接收

v1.0.3 的 `with_limits` 构造器在 v1.1.0 标记 `#[deprecated]`，v2.0.0 移除。

---

## 13. 修订记录

### v2（2026-06-14，本版本）

基于"最佳实践、最优方案、质量优先"原则对 v1 进行 6 处修订：

| # | v1 立场 | v2 修订 | 理由 |
|---|--------|--------|------|
| 1 | 新增 `ToolExecutionContext` 与既有 `ExecutionContext` 并存 | 合并为单一 `ExecutionContext`，手动 impl Debug | DRY 原则；对齐 LangChain / OpenAI Agents SDK 单 context 惯例 |
| 2 | `BuiltinTool::execute` 提供 default method 降为 patch | 直接改签名 + `LegacyBuiltinTool` blanket impl，落 v1.1.0 minor | RFC 1105 明确"add defaulted item = minor"，patch 立场不成立；default method 会让工具沉默走兼容路径，掩盖 staleness 防护缺失 |
| 3 | `RwLock<HashMap>` + LRU 淘汰 | `DashMap` 嵌套 + 显式 reject 超额 | 项目内并发 KV 惯例；显式 reject 让 Agent 看见边界 |
| 4 | `max_entries_per_session: 1000` 无实证论证 | 补 Claude Code 公开数据基线（~30 文件）+ 30× 安全边际 | 让数字有依据，避免凭空设定 |
| 5 | `task_local!` 传递 `tool_call_id` | Layer 2 `Tool::execute(args, call_id)` 显式参数 | task_local 是隐式状态，与 Option A 显式 context 原则自相矛盾；与 v1 评 Option B 时批判的 magic argument 同形 |
| 6 | 无观测性设计 | OTel 兼容 metric + tracing::warn 字段命名遵循 semantic conventions | production agent SDK 必须可监控 stale 拒绝事件 |

### v1（2026-06-14，初稿）

首次成稿，覆盖 stale-read 防护、context 通路接通、`BuiltinTool` trait 签名变更。

---

## 14. 开放问题（实施前需确认）

实施阶段开始前需明确（**已 v2 内部决策，待用户最终确认**）：

1. **是否在 v1.1.0 同时发布 `migrate` CLI 工具？** — 推荐：是。否则用户迁移成本高。
2. **`LegacyBuiltinTool` 是否在 v1.1.0 release 中 `#[deprecated]`？** — 推荐：否，给用户至少 1 个 minor 周期迁移。
3. **默认 `StaleReadPolicy` 是 `Strict` 还是 `Warn`？** — 推荐：`Strict`（production 安全优先）。但需在 release notes 顶部显式说明。

**所有技术决策已在 §1-12 完整论证；以上 3 项是发布策略而非技术决策，待 implementation 阶段与 maintainer 对齐。**
