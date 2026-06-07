# Continuum 产品身份

> 版本: v1.2
> 日期: 2026-05-27
> 目标: 明确双产品线定位

---

## 一、产品线概览

Continuum 是**双产品线架构**：

| 产品线 | 形态 | 技术栈 |
|--------|------|--------|
| **CLI/TUI** | 终端Agent工具 | Rust (ratatui) |
| **Python SDK** | 可嵌入Agent系统 | Python + Rust bindings |

---

## 二、CLI/TUI 产品线

### 2.1 定位

开源终端Agent工具

### 2.2 核心卖点

| 卖点 | 说明 |
|------|------|
| **开源可定制** | MIT许可证，源码完全开放 |
| **多Provider支持** | Anthropic/OpenAI/Gemini等 |
| **Rust后端** | 原生高性能 |
| **TUI界面** | ratatui框架，5种主题 |

### 2.3 功能清单

- 聊天对话（流式响应）
- 文件操作（Read/Write/Edit）
- Shell执行（权限确认）
- Git集成（Status/Diff/Commit）
- 会话管理
- Slash命令系统

---

## 三、Python SDK 产品线

### 3.1 定位

可嵌入的Agent系统

### 3.2 核心卖点

| 卖点 | 说明 |
|------|------|
| **预集成** | Memory + RAG + 规划 + 工具 |
| **极简依赖** | httpx + pydantic |
| **YAML配置** | 可配置驱动 |
| **双层Memory** | Project + Auto Memory |
| **三合一规划** | One-Shot / Plan-Execute / ReWOO |

### 3.3 使用方式

```python
from continuum import Agent
agent = Agent()
result = agent.run("任务")
```

---

## 四、技术架构

```
Layer 5: Interface (CLI/TUI + Python SDK)
Layer 4: Integration (MCP/Audit)
Layer 3: Capabilities (Tools/Memory/RAG)
Layer 2: Core (Agent/Session)
Layer 1: Foundation (LLM/Streaming)
Layer 0: Security
```

---

## 五、当前短板

| 短板 | 说明 | 改进方向 |
|------|------|----------|
| 工具生态 | 内置工具较少 | MCP扩展 |
| LLM后端 | 支持Provider有限 | 覆盖主流 |
| 社区规模 | 新项目 | 积累案例 |
| 生产案例 | 待验证 | 收集反馈 |

---

## 六、Logo设计方向

基于"Continuum"（连续体）概念：

| 方向 | 元素 |
|------|------|
| A | C字母 + 无限循环 |
| B | 终端符号 + 盒子 |
| C | Rust + Python融合 |

---

**文档状态**: v1.2
**说明**: 仅陈述可验证事实，不包含对竞品的未经证实描述