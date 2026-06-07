# Continuum Python SDK

生产级 Agent 框架，具备崩溃安全保证。

[English](README.md) | 简体中文

## 快速开始（3步）

```python
# 示例 - 需要在环境变量中设置 CONTINUUM_API_KEY
from continuum import Agent

agent = Agent()  # 自动从环境加载配置
result = agent.run("你的任务")
```

## 为什么选择 Continuum？

| 特性 | Continuum | 其他框架 |
|------|-----------|----------|
| **会话持久化** | 内置检查点与恢复 | 需手动实现 |
| **多提供商支持** | 13+ 提供商，统一API | 需分别集成 |
| **安全性** | PathValidator + AuditLogger | 无内置安全机制 |
| **国产大模型** | GLM/KIMI/DeepSeek 原生支持 | 需手动配置 |
| **开发效率** | 3行代码即可启动 | 配置复杂 |

### 核心优势

1. **崩溃安全**：会话自动保存，可随时恢复
2. **提供商切换**：一行配置即可切换 LLM 提供商
3. **内置工具**：16+ 工具开箱即用（文件、搜索、Shell、LSP）
4. **生产就绪**：安全审计、权限检查、错误恢复

## 安全机制（生产就绪）

Continuum 为生产部署提供内置安全机制。安全检查是**可选的**，通过 `workspace` 参数启用 — 一旦配置工作空间，每次文件操作都会执行路径验证、权限检查和审计日志。

```python
from continuum_sdk.security import (
    PathValidator, AuditLogger, PermissionChecker, Permission,
)
from continuum_sdk.tools import read_file, write_file, edit_file, list_directory

# 1. 直接使用安全组件
validator = PathValidator("/workspace")
result = validator.validate("./file.txt")
if result.is_valid:
    content = read_file(result.resolved_path, workspace="/workspace")

# 2. 或使用安全上下文（推荐）
from continuum_sdk.tools._security import resolve_security, secure_file_read

ctx = resolve_security(workspace="/workspace", permission=Permission.READ)
with secure_file_read(ctx, "file.txt", ...) as content:
    # 安全读取，自动记录审计日志
    process(content)
```

## 多提供商支持

支持 13+ LLM 提供商，一行配置即可切换：

```python
from continuum import Agent, Config

# Anthropic (默认)
config = Config(provider="anthropic")

# OpenAI
config = Config(provider="openai")

# 国产大模型
config = Config(provider="deepseek")  # DeepSeek
config = Config(provider="glm")       # 智谱 GLM
config = Config(provider="kimi")      # Moonshot KIMI
config = Config(provider="qwen")      # 阿里云 Qwen

agent = Agent(config=config)
```

**完整提供商列表：**
- Anthropic Claude
- OpenAI GPT
- Google Gemini
- DeepSeek
- GLM（智谱）
- KIMI（Moonshot）
- Qwen（阿里云）
- Cohere
- Mistral
- Groq
- Together AI
- Azure OpenAI
- AWS Bedrock

## 工具系统

### 内置工具

| 工具 | 功能 |
|------|------|
| `bash_execute` | Shell 命令执行（安全模式） |
| `read_file` | 文件读取（路径验证） |
| `write_file` | 文件写入（原子操作） |
| `edit_file` | 文件编辑（正则替换） |
| `list_directory` | 目录列表 |
| `search_files` | 文件搜索 |
| `web_search` | 网络搜索 |
| `web_fetch` | 网页抓取 |

### 自定义工具

```python
agent.register_tool(
    "my_tool",
    my_function,
    description="工具描述",
    parameters={
        "type": "object",
        "properties": {
            "input": {"type": "string"}
        }
    }
)
```

## 会话管理

```python
from continuum import Session

# 创建会话
session = Session(agent=agent)

# 运行任务
result = session.run("任务描述")

# 持久化保存
session.save()

# 从检查点恢复
session = Session.recover("checkpoint_path")
```

## 安装

```bash
pip install continuum-agent-sdk
```

**开发安装：**
```bash
pip install -e ".[dev]"
```

## 文档

- [API 参考](docs/API_REFERENCE.md)
- [最佳实践](docs/BEST_PRACTICES.md)
- [迁移指南](docs/MIGRATION_GUIDE.md)
- [架构设计](docs/ARCHITECTURE.md)

## 许可证

MIT License - 详见 [LICENSE](LICENSE)

## 联系方式

邮箱：1281676337@qq.com

## 贡献

欢迎贡献代码！请查看 [贡献指南](docs/CONTRIBUTING.md)。