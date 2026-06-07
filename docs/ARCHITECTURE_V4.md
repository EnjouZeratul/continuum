# Continuum 完整架构设计 V4.0

> **Internal Design Document** — This document is for internal reference only.
> Competitor references are for design inspiration analysis, not marketing positioning.

> 版本: v4.0
> 日期: 2026-05-10
> 状态: 完整规划（无MVP，直接开发完整产品）
> 基于: 五轮专家评审 + Claude Code + OpenClaw + LangChain + LangGraph

---

## 一、产品定位

### 1.1 双产品战略

Continuum 是两个同等重要的产品：

| 产品 | 定位 | 类比 | 目标用户 |
|------|------|------|----------|
| **CLI 产品** | 终端Agent产品 | Claude Code + Aider + OpenClaw CLI | 终端用户、开发者 |
| **Python SDK** | 开发框架 | LangChain + LangGraph + SmolAgents | AI应用开发者 |

### 1.2 核心设计原则

```
┌─────────────────────────────────────────────────────────────────┐
│                    Rust Core (性能 + 安全)                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  高性能执行引擎 | 内存安全 | 并发安全 | 零成本抽象        │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                    Python Layer (友好接口)                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  PyO3/maturin 绑定 | 简洁API | 开发者友好 | 易于推广     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**原则**：
- Rust 核心：70% 模块使用 Rust 实现
- Python 薄层：仅作为友好接口，不承担核心逻辑
- 无 MVP：直接开发完整产品，直到完毕

---

## 二、六层架构总览

### 2.1 分层架构图

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Layer 5: Interface (产品层)                                                 │
│  ┌─────────────────────────┐  ┌─────────────────────────────────────────┐ │
│  │ CLI (TUI)               │  │ Python SDK                              │ │
│  │ - 终端界面              │  │ - PyO3 绑定                             │ │
│  │ - 流式输出              │  │ - 开发者友好API                         │ │
│  │ - 会话持久化            │  │ - 类型提示                              │ │
│  └─────────────────────────┘  └─────────────────────────────────────────┘ │
├────────────────────────────────────────────────────────────────────────────┤
│ Layer 4: Integration (集成层)                                               │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐ │
│  │channel-    │ │plugin-     │ │mcp-bridge  │ │worktree-   │ │audit-    │ │
│  │gateway     │ │loader      │ │            │ │manager     │ │logger    │ │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └──────────┘ │
├────────────────────────────────────────────────────────────────────────────┤
│ Layer 3: Capabilities (能力层) - 15 模块                                    │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐ │
│  │tool-       │ │builtin-    │ │skills      │ │memory-     │ │retriever-│ │
│  │executor    │ │tools       │ │            │ │system      │ │engine    │ │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └──────────┘ │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐ │
│  │query-      │ │output-     │ │guard-rails │ │example-    │ │process-  │ │
│  │engine      │ │parsers     │ │            │ │selectors   │ │manager   │ │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └──────────┘ │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐ │
│  │sandbox-    │ │lsp-client  │ │document-   │ │text-       │ │vector-   │ │
│  │runtime     │ │            │ │loaders     │ │splitters   │ │store     │ │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └──────────┘ │
├────────────────────────────────────────────────────────────────────────────┤
│ Layer 2: Core (核心层) - 8 模块                                             │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐            │
│  │agent-      │ │session-    │ │tool-       │ │workflow-   │            │
│  │runtime     │ │manager     │ │registry    │ │engine      │            │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘            │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐            │
│  │hook-       │ │checkpoint- │ │tasks       │ │prompts     │            │
│  │system      │ │system      │ │            │ │            │            │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘            │
├────────────────────────────────────────────────────────────────────────────┤
│ Layer 1: Foundation (基础层) - 11 模块                                      │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐│
│  │llm-client  │ │embeddings  │ │storage-    │ │streaming   │ │config-   ││
│  │            │ │            │ │engine      │ │            │ │manager   ││
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └──────────┘│
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐│
│  │event-bus   │ │observability│ │cost-      │ │cache-      │ │error-   ││
│  │            │ │            │ │tracker     │ │manager     │ │handler   ││
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └──────────┘│
│  ┌────────────┐                                                            │
│  │serde-system│  ← 使用 pydantic 替代                                     │
│  └────────────┘                                                            │
├────────────────────────────────────────────────────────────────────────────┤
│ Layer 0: Security Gateway (安全网关) - 4 模块                               │
│  ┌────────────────────┐ ┌────────────────────┐                          │
│  │input-validator     │ │pii-scrubber        │                          │
│  └────────────────────┘ └────────────────────┘                          │
│  ┌────────────────────┐ ┌────────────────────┐                          │
│  │access-controller   │ │rate-limiter        │                          │
│  └────────────────────┘ └────────────────────┘                          │
└────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 模块统计

| 层级 | 模块数 | 职责 |
|------|--------|------|
| **Layer 0** | 4 | 安全网关 - 所有外部输入的验证入口 |
| **Layer 1** | 11 | 基础设施 - 通用能力提供 |
| **Layer 2** | 8 | 核心引擎 - Agent 运行核心 |
| **Layer 3** | 15 | 能力扩展 - 特定领域能力 |
| **Layer 4** | 5 | 外部集成 - 第三方系统对接 |
| **Layer 5** | 2 | 产品接口 - 用户界面 |
| **合计** | **45** | 完整产品架构 |

---

## 三、依赖规则

### 3.1 严格分层依赖

```
依赖规则：Layer N 只能依赖 Layer N-1

Layer 5 (Interface)     → 只依赖 Layer 4
Layer 4 (Integration)  → 只依赖 Layer 3
Layer 3 (Capabilities) → 只依赖 Layer 2
Layer 2 (Core)         → 只依赖 Layer 1
Layer 1 (Foundation)   → 只依赖 Layer 0
Layer 0 (Security)     → 无依赖（最底层）

禁止：
- 向上依赖（Layer 2 不能依赖 Layer 3）
- 跨层依赖（Layer 5 不能直接依赖 Layer 2）
- 循环依赖（任何形式）
```

### 3.2 安全网关模式

```
所有外部输入必须经过 Layer 0：

外部输入 → Layer 0 (Security Gateway)
              ├── input-validator: 格式验证
              ├── pii-scrubber: PII 清洗
              ├── access-controller: 权限检查
              └── rate-limiter: 速率限制
                    ↓
              Layer 1+ 内部模块
```

---

## 四、各层详细设计

### 4.1 Layer 0: Security Gateway (安全网关)

所有外部输入的统一入口，确保系统安全。

| 模块 | 职责 | Rust实现 | 关键功能 |
|------|------|----------|----------|
| **input-validator** | 输入验证 | ✅ | JSON Schema、类型检查、长度限制 |
| **pii-scrubber** | PII清洗 | ✅ | 敏感数据检测、脱敏、日志过滤 |
| **access-controller** | 访问控制 | ✅ | RBAC、权限检查、审计 |
| **rate-limiter** | 速率限制 | ✅ | Token Bucket、滑动窗口、配额管理 |

### 4.2 Layer 1: Foundation (基础层)

通用基础设施，为上层提供核心能力。

| 模块 | 职责 | Rust实现 | 关键功能 |
|------|------|----------|----------|
| **llm-client** | LLM客户端 | ✅ | 多提供商支持、重试、流式响应 |
| **embeddings** | 嵌入模型 | ✅ | 文本嵌入、批量处理、缓存 |
| **storage-engine** | 存储引擎 | ✅ | 文件存储、对象存储、事务支持 |
| **streaming** | 流式处理 | ✅ | SSE、WebSocket、背压控制 |
| **config-manager** | 配置管理 | ✅ | 多环境、热更新、验证 |
| **event-bus** | 事件总线 | ✅ | 发布订阅、事件溯源、持久化 |
| **observability** | 可观测性 | ✅ | Tracing、Metrics、Logs |
| **cost-tracker** | 成本追踪 | ✅ | Token计数、费用计算、预算控制 |
| **serde-system** | 序列化 | ❌ | 使用 pydantic 替代 |
| **cache-manager** | 缓存管理 | ✅ | LRU、TTL、分布式缓存 |
| **error-handler** | 错误处理 | ✅ | 错误分类、恢复策略、上报 |

### 4.3 Layer 2: Core (核心层)

Agent 运行时核心引擎。

| 模块 | 职责 | Rust实现 | 关键功能 |
|------|------|----------|----------|
| **agent-runtime** | Agent运行时 | ✅ | 执行循环、状态管理、调度 |
| **session-manager** | 会话管理 | ✅ | 会话生命周期、并发控制（已有Python实现需迁移） |
| **tool-registry** | 工具注册 | ✅ | 工具发现、元数据管理、版本控制 |
| **workflow-engine** | 工作流引擎 | ✅ | DAG调度、条件分支、并行执行 |
| **hook-system** | Hook系统 | ✅ | 生命周期钩子、插件扩展点 |
| **checkpoint-system** | 检查点系统 | ✅ | 状态快照、断点恢复（已有Python实现需迁移） |
| **tasks** | 任务管理 | ✅ | 任务队列、优先级、依赖 |
| **prompts** | 提示词管理 | ✅ | 模板引擎、版本控制、优化 |

### 4.4 Layer 3: Capabilities (能力层)

特定领域的能力扩展。

| 模块 | 职责 | Rust实现 | 关键功能 |
|------|------|----------|----------|
| **tool-executor** | 工具执行器 | ✅ | 工具调用、结果处理、超时控制 |
| **builtin-tools** | 内置工具集 | 🔶 | 40+工具（文件、搜索、代码等） |
| **skills** | Skills系统 | 🔶 | 技能定义、组合、编排 |
| **memory-system** | 分层记忆 | ✅ | Working/Session/Project/Long-term |
| **retriever-engine** | 检索引擎 | ✅ | 相似度搜索、混合检索、重排序 |
| **query-engine** | 查询引擎 | ✅ | 代码搜索、语义查询（Claude Code核心） |
| **output-parsers** | 输出解析 | 🔶 | 结构化解析、验证、修复 |
| **guard-rails** | 边界约束 | ✅ | 输出限制、安全检查、合规 |
| **example-selectors** | 示例选择 | 🔶 | 动态示例、相似度匹配 |
| **process-manager** | 进程管理 | ✅ | 子进程管理、资源隔离（OpenClaw风格） |
| **sandbox-runtime** | 沙箱运行时 | ✅ | 代码执行、安全隔离、资源限制 |
| **lsp-client** | LSP客户端 | ✅ | 语言服务器协议、代码智能 |
| **document-loaders** | 文档加载器 | 🔶 | 多格式解析、批量处理 |
| **text-splitters** | 文本分割器 | ✅ | 分块策略、重叠控制 |
| **vector-store** | 向量存储 | ✅ | 向量索引、持久化、查询 |

### 4.5 Layer 4: Integration (集成层)

外部系统和协议集成。

| 模块 | 职责 | Rust实现 | 关键功能 |
|------|------|----------|----------|
| **channel-gateway** | 渠道网关 | ✅ | 多渠道接入、消息路由（OpenClaw） |
| **plugin-loader** | 插件加载器 | ✅ | 动态加载、依赖管理、热更新 |
| **mcp-bridge** | MCP桥接 | ✅ | Model Context Protocol 实现 |
| **worktree-manager** | 工作树管理 | ✅ | Git Worktree、隔离环境（Claude Code） |
| **audit-logger** | 审计日志 | ✅ | 操作记录、合规报告、追溯 |

### 4.6 Layer 5: Interface (产品层)

用户界面产品。

| 产品 | 职责 | 实现 | 关键功能 |
|------|------|------|----------|
| **CLI (TUI)** | 终端产品 | Rust + Python | 流式输出、会话持久化、40+工具、LSP集成 |
| **Python SDK** | 开发框架 | PyO3 | 简洁API、类型提示、文档完善 |

---

## 五、Rust vs Python 实现分布

### 5.1 实现语言选择

```
Rust 核心 (~70% 模块):
├── 性能关键路径: agent-runtime, workflow-engine, query-engine
├── 并发安全: session-manager, checkpoint-system
├── 资源管理: process-manager, sandbox-runtime
└── 安全敏感: Security Gateway 全部模块

Python 层 (~30%):
├── 业务逻辑: builtin-tools, skills, output-parsers
├── 集成便利: document-loaders, example-selectors
├── 接口层: Python SDK 全部
└── 快速迭代: text-splitters 部分
```

### 5.2 Rust/Python 分工原则

| 原则 | Rust 负责 | Python 负责 |
|------|-----------|-------------|
| **性能关键** | ✅ 执行引擎、调度 | ❌ |
| **并发安全** | ✅ 状态管理、锁 | ❌ |
| **内存敏感** | ✅ 大数据处理 | ❌ |
| **业务逻辑** | ❌ | ✅ 工具实现、解析 |
| **快速迭代** | ❌ | ✅ 配置、模板 |
| **接口友好** | ❌ | ✅ SDK API |

---

## 六、已有代码资产

### 6.1 Python 实现需迁移

| 文件 | 行数 | 功能 | 迁移目标 |
|------|------|------|----------|
| `checkpoint_writer.py` | 754 | 原子写入、CrashRecovery | Layer 2 checkpoint-system |
| `session_concurrency.py` | 645 | ReadWriteLock、SessionManager | Layer 2 session-manager |

### 6.2 测试资产

| 文件 | 测试内容 | 状态 |
|------|----------|------|
| `test_session_concurrency.py` | 并发安全测试 | ✅ 通过 |

---

## 七、竞品能力对标

### 7.1 Claude Code 能力覆盖

| Claude Code 功能 | Continuum 模块 | 覆盖状态 |
|------------------|-------------------|----------|
| 40+ 内置工具 | builtin-tools | ✅ |
| Query Engine | query-engine | ✅ |
| LSP Client | lsp-client | ✅ |
| Worktree Manager | worktree-manager | ✅ |
| Cost Tracking | cost-tracker | ✅ |
| 流式输出 | streaming | ✅ |
| 会话持久化 | session-manager | ✅ |
| Hooks | hook-system | ✅ |

### 7.2 OpenClaw 能力覆盖

| OpenClaw 功能 | Continuum 模块 | 覆盖状态 |
|----------------|-------------------|----------|
| Plugin SDK | plugin-loader | ✅ |
| Channel Gateway | channel-gateway | ✅ |
| Process Management | process-manager | ✅ |
| Sandbox Runtime | sandbox-runtime | ✅ |
| Audit Logger | audit-logger | ✅ |
| Tenant Manager | - | ❌ 不纳入（企业功能） |

### 7.3 LangChain/LangGraph 能力覆盖

| LangChain/LangGraph 功能 | Continuum 模块 | 覆盖状态 |
|--------------------------|-------------------|----------|
| Chains | workflow-engine | ✅ |
| Agents | agent-runtime | ✅ |
| Memory | memory-system | ✅ |
| Tools | tool-registry + tool-executor | ✅ |
| Retrieval | retriever-engine + vector-store | ✅ |
| Callbacks | hook-system | ✅ |
| Output Parsers | output-parsers | ✅ |
| Example Selectors | example-selectors | ✅ |

---

## 八、安全覆盖分析

### 8.1 当前安全模块覆盖

| 安全维度 | 模块 | 覆盖率 |
|----------|------|--------|
| **输入安全** | input-validator, pii-scrubber | 100% |
| **访问控制** | access-controller, audit-logger | 80% |
| **速率限制** | rate-limiter | 100% |
| **输出安全** | guard-rails | 60% |
| **执行安全** | sandbox-runtime | 70% |
| **数据安全** | storage-engine (加密) | 50% |

### 8.2 安全增强计划（Phase 2）

建议补充的安全模块：

| 模块 | 职责 | 优先级 |
|------|------|--------|
| **secrets-manager** | 密钥管理 | P1 |
| **encryption-engine** | 加密引擎 | P1 |
| **threat-detector** | 威胁检测 | P2 |
| **compliance-checker** | 合规检查 | P2 |

---

## 九、开发路线

### Phase 1: 基础安全层 (Layer 0-1)

```
优先级: P0 (必须完成)
目标: 建立安全网关和基础设施

模块:
├── Layer 0: input-validator, pii-scrubber, access-controller, rate-limiter
├── Layer 1: llm-client, storage-engine, streaming, cost-tracker
└── 测试: 单元测试 + 集成测试
```

### Phase 2: 核心引擎层 (Layer 2)

```
优先级: P0 (必须完成)
目标: Agent 运行时核心

模块:
├── agent-runtime, session-manager (迁移Python实现)
├── tool-registry, workflow-engine
├── checkpoint-system (迁移Python实现)
├── hook-system, tasks, prompts
└── 测试: 并发测试 + 压力测试
```

### Phase 3: 能力扩展层 (Layer 3)

```
优先级: P0 (必须完成)
目标: 特定领域能力

模块:
├── tool-executor, builtin-tools (40+工具)
├── memory-system, query-engine, lsp-client
├── process-manager, sandbox-runtime
├── retriever-engine, vector-store
└── 测试: 端到端测试
```

### Phase 4: 集成与产品层 (Layer 4-5)

```
优先级: P0 (必须完成)
目标: 完整产品

模块:
├── Layer 4: mcp-bridge, plugin-loader, worktree-manager, channel-gateway
├── Layer 5: CLI (TUI) + Python SDK (PyO3)
└── 测试: 用户验收测试 + 性能测试
```

---

## 十、技术栈

### 10.1 Rust 技术栈

```
核心依赖:
├── tokio: 异步运行时
├── pyo3: Python 绑定
├── serde: 序列化
├── tracing: 可观测性
├── tower: 中间件
└── anyhow/thiserror: 错误处理

构建工具:
├── maturin: Python 包构建
├── cargo: Rust 包管理
└── cargo-release: 版本发布
```

### 10.2 Python 技术栈

```
核心依赖:
├── pydantic: 数据验证（替代 serde-system）
├── typing: 类型提示
├── asyncio: 异步支持
└── pytest: 测试框架

接口层:
├── CLI: typer + rich
├── SDK: 简洁API设计
└── 文档: mkdocs
```

---

## 十一、总结

### 架构特点

| 特点 | 说明 |
|------|------|
| **完整覆盖** | 45模块覆盖所有竞品核心能力 |
| **严格分层** | 单向依赖，无循环，易维护 |
| **安全优先** | Layer 0 安全网关，所有输入必经 |
| **性能核心** | 70% Rust 实现，Python 仅接口 |
| **双产品** | CLI + SDK 同等重要 |

### 一句话总结

> **Continuum = Rust核心性能 + Python友好接口 + 完整Agent能力**

---

**文档状态**: v4.0 完整架构设计
**下一步**: 开始 Phase 1 开发
