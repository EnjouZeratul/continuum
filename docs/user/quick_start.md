# Continuum 快速入门指南

> 适用版本: v1.0.0+
> 更新时间: 2026-05-24

---

## 目录

1. [系统要求](#系统要求)
2. [安装步骤](#安装步骤)
3. [首次配置](#首次配置)
4. [基本使用](#基本使用)
5. [智能 Agent 进阶](#智能-agent-进阶)
6. [下一步学习](#下一步学习)

---

## 系统要求

### 必需环境

| 组件 | 最低版本 | 推荐版本 |
|------|----------|----------|
| Python | 3.10 | 3.11+ |
| pip | 21.0 | 最新 |
| 操作系统 | Windows 10 / macOS 10.15 / Linux | 最新稳定版 |

### 硬件要求

| 组件 | 最低配置 | 推荐配置 |
|------|----------|----------|
| 内存 | 4 GB | 8 GB+ |
| 存储 | 500 MB | 1 GB+ |
| CPU | 2 核心 | 4 核心+ |

### 网络要求

- 需要访问 AI 提供商 API (Anthropic/OpenAI/Google)
- 建议稳定网络连接

---

## 安装步骤

### 方式一：PyPI 安装（推荐）

```bash
# 1. 升级 pip
pip install --upgrade pip

# 2. 安装 Continuum SDK
pip install continuum-agent-sdk

# 3. 验证安装
python -c "from continuum import Agent; print('OK')"
# 输出: OK
```

### 方式二：从源码安装

```bash
# 1. 克隆仓库
git clone https://github.com/anthropics/continuum.git
cd continuum

# 2. 创建虚拟环境（推荐）
python -m venv .venv

# Windows
.venv\Scripts\activate

# macOS/Linux
source .venv/bin/activate

# 3. 安装依赖
pip install -e ".[dev]"

# 4. 验证安装
continuum --version
```

### 方式三：使用 pipx（隔离安装）

```bash
# 1. 安装 pipx（如果没有）
pip install pipx

# 2. 使用 pipx 安装
pipx install continuum

# 3. 验证
continuum --version
```

---

## 首次配置

### 步骤 1：获取 API Key

选择一个 AI 提供商并获取 API Key：

| 提供商 | 获取链接 | 免费额度 |
|--------|----------|----------|
| Anthropic | [console.anthropic.com](https://console.anthropic.com) | $5 新用户 |
| OpenAI | [platform.openai.com](https://platform.openai.com) | $18 新用户 |
| Google | [aistudio.google.com](https://aistudio.google.com) | 免费配额 |

### 步骤 2：配置 API Key

**方式 A：环境变量（推荐新手）**

```bash
# macOS/Linux - 添加到 ~/.bashrc 或 ~/.zshrc
export ANTHROPIC_API_KEY="your-api-key-here"

# Windows PowerShell - 添加到 $PROFILE
$env:ANTHROPIC_API_KEY = "your-api-key-here"

# Windows CMD - 添加到系统环境变量
setx ANTHROPIC_API_KEY "your-api-key-here"
```

**方式 B：配置文件**

```bash
# 初始化配置
continuum config init

# 这会创建 ~/.sh/config.toml 文件
```

编辑 `~/.sh/config.toml`：

```toml
model = "claude-3-haiku"

[providers.anthropic]
api_key = "your-api-key-here"
base_url = "https://api.anthropic.com"
```

**方式 C：CLI 命令**

```bash
# 添加提供商
continuum config add-provider anthropic \
  --api-key "your-api-key-here" \
  --url "https://api.anthropic.com" \
  --model "claude-3-haiku"

# 设为默认
continuum config use anthropic
```

### 步骤 3：验证配置

```bash
# 验证配置有效性
continuum config validate

# 查看当前配置
continuum config show
```

---

## 基本使用

### 第一次对话

```bash
# 启动 TUI（终端用户界面）
continuum

# 或直接运行单次任务
continuum run "你好，请介绍一下你自己"
```

### TUI 基本操作

启动后，你会看到如下界面：

```
┌─────────────────────────────────────────────────────────────┐
│ Status: idle | Model: claude-3-haiku | Tokens: 0           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Welcome to Continuum!                                   │
│  Type your message below and press Enter to send.           │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ > _                                                         │
└─────────────────────────────────────────────────────────────┘
```

**基本操作流程：**

1. 在输入框输入消息
2. 按 `Enter` 发送
3. 等待 AI 响应
4. 继续对话或按 `Ctrl+C` 退出

### 常用快捷键

| 快捷键 | 功能 |
|--------|------|
| `Enter` | 发送消息 |
| `Ctrl+Enter` | 换行（多行输入） |
| `Ctrl+C` | 退出程序 |
| `Tab` | 切换焦点区域 |
| `↑` / `↓` | 浏览历史消息 |
| `/` | 搜索消息 |
| `?` | 显示帮助 |

### 执行代码任务

Continuum 可以帮你写代码：

```bash
continuum run "写一个 Python 函数，计算斐波那契数列"
```

或者更复杂的任务：

```bash
continuum run "创建一个 Flask 应用，包含用户登录功能"
```

### 会话管理

```bash
# 查看所有会话
continuum session list

# 恢复之前的会话
continuum session resume <session-id>

# 删除会话
continuum session delete <session-id>
```

### 使用 Python SDK

**基础 Agent（3步上手）：**

```python
from continuum import Agent

# 创建 Agent（自动从环境变量配置）
agent = Agent()

# 执行任务
result = agent.run("分析当前目录的项目结构")
print(result)
```

**会话管理：**

```python
from continuum import Agent, Session

# 创建带会话的 Agent
agent = Agent()
session = agent.create_session()

# 多轮对话（上下文保持）
agent.run("记住：我的项目使用 Python 3.11")
agent.run("我之前说使用的什么版本？")  # 会记住上下文

# 保存会话
session.save("my_session.json")

# 加载会话
from continuum_sdk import Session
saved_session = Session.load("my_session.json")
agent = Agent(session=saved_session)
```

---

## 智能 Agent 进阶

IntelligentAgent 提供任务规划、自校正和进度跟踪功能：

```python
from continuum_sdk.agent import IntelligentAgent, AgentMode

# 创建智能 Agent（自主模式）
agent = IntelligentAgent(
    api_key="your-api-key",  # 或使用环境变量
    mode=AgentMode.AUTONOMOUS
)

# 规划任务
plan = await agent.plan("修复 auth.py 中的 bug")
print(plan.to_dict())  # 查看规划

# 执行规划
result = await agent.execute(plan)
print(f"完成 {result.completed_steps}/{result.total_steps} 步")
```

**执行模式：**

| 模式 | 说明 |
|------|------|
| `AUTONOMOUS` | 自动执行，无需确认 |
| `INTERACTIVE` | 每步询问确认 |
| `STEP_BY_STEP` | 每步暂停等待 |

**进度跟踪：**

```python
# 查看执行进度
agent.get_progress_text()
# 输出: [2/5] 40% in 5s ETA: 7s

# 查看规划摘要
agent.get_plan_summary()
# 输出: 
# Plan: 修复 auth.py 中的 bug
#   ○ [s1] 搜索相关代码
#   ○ [s2] 分析 bug 原因
#   ○ [s3] 应用修复
#   ...
```

---

## 下一步学习

### 推荐阅读顺序

1. **[工具使用指南](./tools_guide.md)** - 学习如何使用内置工具
2. **[常见问题解答](./faq.md)** - 解决常见问题
3. **[完整用户手册](../USER_MANUAL.md)** - 详细功能说明

### 进阶主题

| 主题 | 描述 |
|------|------|
| 自定义工具 | 创建自己的工具扩展 |
| 工作流 | 使用 DAG 构建复杂流程 |
| 记忆系统 | 让 Agent 记住重要信息 |
| 多模型切换 | 在不同 AI 模型间切换 |
| MCP集成 | 连接外部工具服务器 |

### 自定义工具示例

```python
from continuum_sdk.tools import tool

# 使用装饰器创建自定义工具
@tool(name="weather", description="获取天气信息")
async def get_weather(city: str, unit: str = "celsius") -> str:
    """获取指定城市的天气"""
    return f"{city} 天气: 25°{unit[0].upper()}"

# 工具会自动注册，可直接使用
```

### 记忆系统示例

```python
# Illustrative - requires session context
from continuum_sdk.api import MemorySystem

memory = MemorySystem(session_id="quickstart")

# 分层记忆存储
memory.store("working", "用户偏好暗色模式")     # 当前对话
memory.store("session", "项目使用 Python 3.11")  # 会话事实
memory.store("project", "测试命令: pytest tests/")  # 项目知识

# 查询记忆
results = memory.query("如何运行测试?")
print(results)  # 返回相关记忆列表
```

### MCP集成示例

```python
from continuum_sdk.tools import create_mcp_registry

# 快速配置 MCP 服务器
registry = create_mcp_registry(
    ["filesystem", "github"],
    root_path="/your/project"
)

# 获取工具列表
for tool in registry.get_tools():
    print(f"{tool.name}: {tool.description}")

# 执行 MCP 工具
result = await registry.execute("filesystem/read_file", path="README.md")
```

### 示例项目

查看 `examples/` 目录获取更多示例：

```bash
ls examples/
# basic_usage.py      - 基础用法
# custom_tools.py     - 自定义工具
# workflows.py        - 工作流示例
# memory_demo.py      - 记忆系统演示
```

---

## 获取帮助

- **文档**: https://continuum.readthedocs.io
- **GitHub**: https://github.com/anthropics/continuum
- **问题反馈**: https://github.com/anthropics/continuum/issues

---

*Continuum - 让 AI Agent 开发变得简单*
