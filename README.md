# Continuum

[![CI](https://github.com/EnjouZeratul/continuum/actions/workflows/ci.yml/badge.svg)](https://github.com/EnjouZeratul/continuum/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/Python-3.10+-blue.svg)](https://www.python.org/)
[![crates.io](https://img.shields.io/crates/v/continuum.svg)](https://crates.io/crates/continuum)
[![PyPI](https://img.shields.io/pypi/v/continuum-agent-sdk.svg)](https://pypi.org/project/continuum-agent-sdk/)

**Continuum is a concise and reliable Agent runtime.**

Rust core performance + Python friendly interface + full Agent capabilities

[中文版](#中文版)

---

## Two Products

Continuum consists of two related but independent products:

| Product | Purpose | Documentation |
|---------|---------|---------------|
| **Python SDK** | Build AI applications with agents, sessions, tools, memory, and workflows | [python/README.md](python/README.md) |
| **CLI/TUI** | Run agents from terminal with interactive TUI, session management, and toolchain | [cli/README.md](cli/README.md) |

Both products share the same Rust core engine for performance and reliability.

---

## Quick Links

- **SDK Documentation**: [python/README.md](python/README.md) — API reference, examples, security configuration
- **CLI Documentation**: [cli/README.md](cli/README.md) — commands, TUI, keyboard shortcuts, provider setup
- **Architecture**: [docs/ARCHITECTURE_V4.md](docs/ARCHITECTURE_V4.md) — six-layer design (internal design doc)

---

## Architecture Overview

```
Layer 5: Interface     → CLI + Python SDK
Layer 4: Integration   → MCP, Plugin, Worktree
Layer 3: Capabilities  → Tools, Memory, Query Engine
Layer 2: Core          → Agent Runtime, Session, Checkpoint
Layer 1: Foundation    → LLM Client, Storage, Cost Tracker
Layer 0: Security      → Input Validator, PII Scrubber, Access Control
```

- **Rust core**: Layers 0-4 implemented in Rust for performance and safety
- **Python API**: Thin wrapper providing Pythonic interface
- **CLI/TUI**: Terminal interface built on Rust core

---

## Development Status

🚧 **In Development**

| Component | Status |
|-----------|--------|
| Rust Core (Layers 0-4) | In progress |
| Python SDK | Stable API, Rust bindings + Python fallback |
| CLI/TUI | Functional, documentation in progress |

See project issues for detailed roadmap.

---

## Installation

### CLI/TUI

```bash
# From crates.io
cargo install continuum

# Or from source
cargo install --path cli
```

### Python SDK

```bash
# From PyPI
pip install continuum-agent-sdk

# Or from source
pip install -e ./python
```

---

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contact

Email: 1281676337@qq.com

---

<h2 id="中文版">中文版</h2>

<details>
<summary>点击展开中文版</summary>

**Continuum 是简洁可靠的 Agent 运行时。**

Rust 核心性能 + Python 友好接口 + 完整 Agent 能力

---

## 两个产品

Continuum 由两个相关但独立的产品组成：

| 产品 | 用途 | 文档 |
|------|------|------|
| **Python SDK** | 构建包含 Agent、会话、工具、记忆和工作流的 AI 应用 | [python/README.md](python/README.md) |
| **CLI/TUI** | 通过终端运行 Agent，支持交互式 TUI、会话管理和工具链 | [cli/README.md](cli/README.md) |

两个产品共享同一个 Rust 核心引擎，提供性能和可靠性保证。

---

## 快速链接

- **SDK 文档**: [python/README.md](python/README.md) — API 参考、示例、安全配置
- **CLI 文档**: [cli/README.md](cli/README.md) — 命令、TUI、快捷键、提供商配置
- **架构设计**: [docs/ARCHITECTURE_V4.md](docs/ARCHITECTURE_V4.md) — 六层架构（内部设计文档）

---

## 架构概览

```
Layer 5: Interface     → CLI + Python SDK
Layer 4: Integration   → MCP, Plugin, Worktree
Layer 3: Capabilities  → Tools, Memory, Query Engine
Layer 2: Core          → Agent Runtime, Session, Checkpoint
Layer 1: Foundation    → LLM Client, Storage, Cost Tracker
Layer 0: Security      → Input Validator, PII Scrubber, Access Control
```

- **Rust 核心**: Layers 0-4 使用 Rust 实现，确保性能和安全
- **Python API**: 薄封装层，提供 Pythonic 接口
- **CLI/TUI**: 基于 Rust 核心构建的终端界面

---

## 开发状态

🚧 **正在开发中**

| 组件 | 状态 |
|------|------|
| Rust 核心 (Layers 0-4) | 开发中 |
| Python SDK | API 稳定，Rust 绑定 + Python 降级 |
| CLI/TUI | 功能可用，文档完善中 |

详细路线图请参见项目 issues。

---

## 安装

### CLI/TUI

```bash
# 从 crates.io 安装
cargo install continuum

# 或从源码安装
cargo install --path cli
```

### Python SDK

```bash
# 从 PyPI 安装
pip install continuum-agent-sdk

# 或从源码安装
pip install -e ./python
```

---

## License

MIT

</details>
