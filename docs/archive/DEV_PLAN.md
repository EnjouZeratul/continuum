# Continuum 开发规划

**目标**: 完成所有 P0/P1 功能，使项目达到生产可用标准  
**团队规模**: 12人（9开发 + 3审查）  
**预计周期**: 10-15天

---

## 一、团队架构

```
                    ┌─────────────────┐
                    │   Team Lead     │
                    │  (协调/决策)     │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────┴───────┐    ┌───────┴───────┐    ┌───────┴───────┐
│  Rust 核心组   │    │  Python SDK组  │    │   CLI/TUI组   │
│   (3人+1审查)  │    │   (3人+1审查)  │    │   (3人+1审查)  │
└───────────────┘    └───────────────┘    └───────────────┘
```

### Rust 核心组 (4人)

| 成员ID | 角色 | 负责内容 |
|--------|------|----------|
| rust-llm | 开发 | agent_runtime 真实 LLM 调用 |
| rust-stream | 开发 | LLM 流式响应实现 |
| rust-rag | 开发 | VectorStore + RetrieverEngine |
| reviewer-rust | 审查 | Rust 代码质量审查 |

### Python SDK 组 (4人)

| 成员ID | 角色 | 负责内容 |
|--------|------|----------|
| py-fallback | 开发 | builtin.py Python fallback 实现 |
| py-memory | 开发 | memory 持久化存储 |
| py-binding | 开发 | Rust binding 降级完善 |
| reviewer-py | 审查 | Python 代码质量审查 |

### CLI/TUI 组 (4人)

| 成员ID | 角色 | 负责内容 |
|--------|------|----------|
| cli-tui | 开发 | TUI 基础框架 (ratatui) |
| cli-color | 开发 | 颜色高亮 + 语法着色 |
| cli-ux | 开发 | Slash 命令 + 权限确认 UI |
| reviewer-cli | 审查 | CLI/UX 代码审查 |

---

## 二、任务分配

### Phase 1: 核心功能 (Day 1-5)

#### Rust 核心组任务

| 任务ID | 任务 | 负责人 | 优先级 | 预估 | 依赖 |
|--------|------|--------|--------|------|------|
| R1 | 实现 agent_runtime 真实 LLM 调用 | rust-llm | P0 | 3天 | 无 |
| R2 | 实现 LLM 流式响应 | rust-stream | P0 | 2天 | R1 |
| R3 | 实现 VectorStore | rust-rag | P1 | 2天 | 无 |
| R4 | 实现 RetrieverEngine | rust-rag | P1 | 1天 | R3 |

#### Python SDK 组任务

| 任务ID | 任务 | 负责人 | 优先级 | 预估 | 依赖 |
|--------|------|--------|--------|------|------|
| P1 | builtin.py Python fallback | py-fallback | P0 | 2天 | 无 |
| P2 | memory 持久化 (SQLite) | py-memory | P1 | 2天 | 无 |
| P3 | Rust binding 降级完善 | py-binding | P1 | 1天 | P1 |

#### CLI/TUI 组任务

| 任务ID | 任务 | 负责人 | 优先级 | 预估 | 依赖 |
|--------|------|--------|--------|------|------|
| C1 | TUI 基础框架 | cli-tui | P0 | 3天 | 无 |
| C2 | 颜色高亮系统 | cli-color | P0 | 2天 | C1 |
| C3 | Slash 命令解析 | cli-ux | P1 | 1天 | C1 |
| C4 | 权限确认 UI | cli-ux | P1 | 1天 | C1 |

### Phase 2: 集成与完善 (Day 6-10)

| 任务ID | 任务 | 负责组 | 优先级 | 预估 |
|--------|------|--------|--------|------|
| I1 | Rust-Python 绑定集成 | rust-llm + py-binding | P0 | 2天 |
| I2 | CLI 集成 Rust 后端 | cli-tui + rust-llm | P0 | 2天 |
| I3 | 端到端测试 | 全团队 | P1 | 2天 |
| I4 | 文档更新 | reviewer-* | P2 | 1天 |

### Phase 3: 质量保证 (Day 11-15)

| 任务ID | 任务 | 负责人 | 预估 |
|--------|------|--------|------|
| Q1 | 单元测试补充 | 各开发 | 2天 |
| Q2 | 集成测试 | reviewer-* | 2天 |
| Q3 | 性能优化 | rust-llm | 1天 |
| Q4 | 最终验收 | Team Lead | 1天 |

---

## 三、详细任务规格

### R1: 实现 agent_runtime 真实 LLM 调用

**当前问题**:
```rust
// rust/layer2/src/agent_runtime.rs:321-439
fn simulate_llm_step(&mut self, messages: Vec<ChatMessage>) -> Result<LlmResponse> {
    // 这是模拟实现，返回硬编码响应
}
```

**目标实现**:
```rust
fn call_llm_real(&mut self, messages: Vec<ChatMessage>) -> Result<LlmResponse> {
    let client = self.llm_client.as_ref().ok_or(Error::NoLlmClient)?;
    let request = ChatRequest {
        model: self.model.clone(),
        messages,
        ..Default::default()
    };
    client.chat(request)
}
```

**验收标准**:
- [ ] 移除所有 simulate_* 函数
- [ ] 真实调用 Anthropic/OpenAI/Gemini API
- [ ] 错误处理完善（重试、超时、降级）
- [ ] Token 统计准确
- [ ] 单元测试通过

---

### R2: 实现 LLM 流式响应

**当前问题**:
```rust
// rust/layer1/src/llm_client.rs:155
fn send_stream(&self, request: ChatRequest) -> Result<BoxStream<StreamChunk>> {
    Ok(Box::pin(futures_util::stream::empty())) // 返回空流
}
```

**目标实现**:
- 实现 SSE 解析
- 支持 Anthropic/OpenAI 流式格式
- 提供 on_chunk 回调

**验收标准**:
- [ ] 流式返回 token
- [ ] 支持 abort 中断
- [ ] 错误恢复机制
- [ ] 集成测试通过

---

### R3/R4: 向量存储与检索

**当前问题**: VectorStore 和 RetrieverEngine trait 无实现

**目标实现**:
```rust
pub struct InMemoryVectorStore {
    vectors: Arc<RwLock<HashMap<String, VectorEntry>>>,
}

impl VectorStore for InMemoryVectorStore {
    async fn upsert(&self, id: &str, vector: Vec<f32>, metadata: HashMap<String, Value>) -> Result<()>;
    async fn search(&self, query: Vec<f32>, k: usize) -> Result<Vec<SearchResult>>;
    async fn delete(&self, id: &str) -> Result<()>;
}
```

**验收标准**:
- [ ] InMemoryVectorStore 完整实现
- [ ] cosine similarity 搜索
- [ ] 可选磁盘持久化
- [ ] RetrieverEngine 集成

---

### P1: builtin.py Python Fallback

**当前问题**:
```python
# python/continuum_sdk/tools/builtin.py:193-199
def _check_binding(self, name: str) -> None:
    if not self._executor:
        raise NotImplementedError(
            f"Tool '{name}' requires Rust binding."
        )
```

**目标实现**:
- 复用 bash.py/file_ops.py/search.py 的 Python 实现
- 当 HAS_RUST_BINDING=False 时自动降级
- 保持 API 一致

**验收标准**:
- [ ] read_file/write_file/edit_file Python 实现
- [ ] grep/glob Python 实现
- [ ] bash Python 实现
- [ ] 所有测试在无 Rust binding 时通过

---

### P2: Memory 持久化

**当前问题**:
```python
# python/continuum_sdk/memory/layers.py:169-175
# [STUB] 当前为内存存储，需集成 sh-core 持久化
storage = self._storage.get(tier, self._working)
storage.append(entry)
```

**目标实现**:
- SQLite 持久化存储
- 自动过期清理
- 支持导入/导出

**验收标准**:
- [ ] 重启后数据保留
- [ ] 支持配置持久化路径
- [ ] 性能测试通过

---

### C1: TUI 基础框架

**当前问题**: 无 TUI，只有基础 CLI 命令

**目标实现**:
```rust
// 使用 ratatui 库
pub struct ContinuumTui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    input_component: InputComponent,
    output_component: OutputComponent,
    status_component: StatusComponent,
}
```

**验收标准**:
- [ ] 响应式布局
- [ ] 输入框 + 输出区域 + 状态栏
- [ ] 键盘快捷键
- [ ] 鼠标支持

---

### C2: 颜色高亮系统

**目标实现**:
- Markdown 渲染 (代码块、标题、列表)
- 语法高亮 (使用 syntect 或 bat)
- 主题系统 (暗色/亮色)

**验收标准**:
- [ ] 代码块语法高亮
- [ ] Markdown 渲染
- [ ] 可配置主题

---

### C3/C4: Slash 命令 + 权限确认

**目标实现**:
```rust
// Slash 命令
"/help" -> 显示帮助
"/clear" -> 清空对话
"/model <name>" -> 切换模型
"/exit" -> 退出

// 权限确认
pub fn request_permission(tool: &str, args: &Value) -> bool {
    // 弹出确认对话框
}
```

**验收标准**:
- [ ] 至少 5 个 Slash 命令
- [ ] 权限确认弹窗
- [ ] 允许/拒绝/始终允许 选项

---

## 四、协作流程

### 每日同步机制

1. **早会** (Team Lead 主导)
   - 各组汇报昨日进度
   - 识别阻塞问题
   - 调整今日任务

2. **代码审查** (Reviewer 负责)
   - 每个 PR 必须经过审查
   - 审查标准：功能正确性 + 代码质量 + 测试覆盖

3. **晚检** (Team Lead 主导)
   - CI 状态检查
   - 集成测试运行
   - 更新进度看板

### 代码提交规范

```
<type>(<scope>): <description>

[type] feat/fix/refactor/docs/test
[scope] rust/python/cli/all

Examples:
feat(rust): implement real LLM calls in agent_runtime
fix(python): add fallback for builtin tools
refactor(cli): reorganize TUI components
```

### 分支策略

```
main
  ├── feature/rust-llm-real
  ├── feature/rust-streaming
  ├── feature/rust-vector-store
  ├── feature/py-fallback
  ├── feature/py-memory-persist
  ├── feature/cli-tui
  └── feature/cli-color
```

---

## 五、质量门禁

### 每个 PR 必须满足

- [ ] 单元测试通过
- [ ] 代码审查通过
- [ ] 无 clippy/ruff 警告
- [ ] 文档更新

### 每日构建必须满足

- [ ] 全量测试通过
- [ ] 集成测试通过
- [ ] Coverage ≥ 70%

### 最终发布必须满足

- [ ] 所有 P0 任务完成
- [ ] 所有 P1 任务完成
- [ ] 端到端测试通过
- [ ] 性能基准测试通过
- [ ] 文档完整

---

## 六、风险与应对

| 风险 | 概率 | 影响 | 应对措施 |
|------|------|------|----------|
| LLM API 限流 | 中 | 高 | 实现请求队列、多 API Key 轮换 |
| TUI 兼容性问题 | 低 | 中 | 测试多终端、提供降级模式 |
| Rust-Python 绑定问题 | 中 | 高 | 预留 Python fallback、maturin 调试 |
| 团队协作阻塞 | 低 | 高 | 每日同步、快速响应审查 |

---

## 七、里程碑

| 里程碑 | 日期 | 交付物 |
|--------|------|--------|
| M1: 核心功能就绪 | Day 5 | Rust LLM 调用 + Python fallback + TUI 框架 |
| M2: 集成完成 | Day 10 | 端到端可用、基础功能完整 |
| M3: 质量达标 | Day 15 | 测试覆盖、文档完整、可发布 |

---

**文档版本**: v1.0  
**创建时间**: 2026-05-25  
**负责人**: Team Lead
