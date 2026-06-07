# Continuum 用户手册

> 版本: v1.0.0
> 更新时间: 2026-05-12

---

## 目录

1. [快速开始](#快速开始)
2. [安装](#安装)
3. [配置](#配置)
4. [CLI 命令](#cli-命令)
5. [TUI 使用指南](#tui-使用指南)
6. [Python SDK](#python-sdk)
7. [常见问题](#常见问题)
8. [故障排除](#故障排除)

---

## 快速开始

### 3 步启动

```bash
# 1. 安装
pip install continuum

# 2. 配置
export ANTHROPIC_API_KEY=your-key

# 3. 运行
continuum
```

---

## 安装

### 从 PyPI 安装（推荐）

```bash
pip install continuum
```

### 从源码安装

```bash
git clone https://github.com/xxx/continuum
cd continuum
pip install -e .
```

### 验证安装

```bash
continuum --version
python -c "from continuum_sdk import Agent; print('OK')"
```

---

## 配置

### 配置优先级

```
环境变量 > TOML 配置文件 > 内置默认值
```

### 配置方式

#### 1. 环境变量

```bash
# API Key
export ANTHROPIC_API_KEY=your-key
export OPENAI_API_KEY=your-key
export GEMINI_API_KEY=your-key

# 或通用 API Key
export SH_API_KEY=your-key

# 模型
export SH_MODEL=claude-3-haiku

# Base URL
export SH_BASE_URL=https://custom.api.com
```

#### 2. 配置文件

```bash
# 初始化配置
continuum config init
```

配置文件位置: `~/.sh/config.toml`

```toml
# 默认配置
model = "claude-3-haiku"
max_tokens = 4096

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
base_url = "https://api.anthropic.com"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
model = "gpt-4"
```

#### 3. CLI 命令

```bash
# 查看配置
continuum config show

# 添加提供商
continuum config add-provider custom \
  --api-key $KEY \
  --url https://custom.api.com \
  --model custom-model

# 切换提供商
continuum config use custom

# 列出提供商
continuum config list

# 验证配置
continuum config validate
```

---

## CLI 命令

### 命令概览

```
continuum              # 进入 TUI（默认）
continuum run <task>   # 执行任务
continuum session      # 会话管理
continuum config       # 配置管理
continuum help         # 帮助
```

### run 命令

```bash
# 交互模式
continuum run

# 单次执行
continuum run "分析项目结构"

# 指定模型
continuum run --model claude-3-opus "复杂任务"

# 禁用工具
continuum run --no-tools "纯对话"

# 详细输出
continuum run --verbose "调试任务"
```

### session 命令

```bash
# 列出会话
continuum session list

# 恢复会话
continuum session resume <id>

# 删除会话
continuum session delete <id>

# 按名称恢复
continuum session resume my-session
```

### config 命令

```bash
# 初始化
continuum config init

# 添加提供商
continuum config add-provider <name> --api-key <key> --url <url>

# 切换提供商
continuum config use <name>

# 显示当前配置
continuum config show

# 列出所有提供商
continuum config list

# 验证配置
continuum config validate
```

---

## TUI 使用指南

### 界面布局

```
┌─────────────────────────────────────────────────────────────┐
│ Status: active | Model: claude-3-haiku | Tokens: 1234       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  [Chat Area]                                               │
│  User: 你好                                                 │
│  Agent: 你好！有什么可以帮助你的？                           │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ > 输入消息...                                               │
└─────────────────────────────────────────────────────────────┘
```

### 快捷键

#### 全局快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+C` | 退出 |
| `Tab` | 切换焦点 |
| `Esc` | 取消/返回 |

#### 聊天区域

| 快捷键 | 功能 |
|--------|------|
| `↑` / `↓` | 滚动 |
| `PageUp` / `PageDown` | 翻页 |
| `/` | 搜索 |
| `y` | 复制消息 |

#### 输入区域

| 快捷键 | 功能 |
|--------|------|
| `Enter` | 发送 |
| `Ctrl+Enter` | 多行输入 |
| `↑` / `↓` | 历史记录 |
| `Ctrl+u` | 清空输入 |

#### 代码查看器

| 快捷键 | 功能 |
|--------|------|
| `g` | 跳转到行 |
| `/` | 搜索 |
| `n` / `N` | 下/上一个搜索结果 |
| `z` | 折叠/展开 |
| `y` | 复制代码 |

#### 会话列表

| 快捷键 | 功能 |
|--------|------|
| `↑` / `↓` | 选择 |
| `Enter` | 打开会话 |
| `d` | 删除会话 |
| `r` | 重命名 |
| `/` | 搜索 |

### 窗口切换

| 快捷键 | 窗口 |
|--------|------|
| `1` | 聊天 |
| `2` | 代码 |
| `3` | 会话列表 |
| `4` | 帮助 |

---

## Python SDK

### 基础使用

```python
from continuum_sdk import Agent

# 创建 Agent
agent = Agent()

# 执行任务
result = agent.run("写一个 Python 函数")
print(result)
```

### 会话管理

```python
from continuum_sdk import Agent, Session

# 创建会话
session = Session(name="my-session")
agent = Agent(session_id=session.id)

# 对话
agent.run("记住：我喜欢 Python")
agent.run("我之前说我喜欢什么？")  # 会记住上下文

# 保存检查点
checkpoint_id = session.save_checkpoint()

# 回滚
session.rollback(checkpoint_id)
```

### 工具注册

```python
from continuum_sdk.tools import tool, get_registry

@tool(name="calculate", description="执行数学运算")
async def calculate(expression: str) -> float:
    import ast
    return ast.literal_eval(expression)

# 注册工具
registry = get_registry()
registry.register(calculate)

# 使用工具
agent = Agent(tools_enabled=True)
result = agent.run("计算 1+1")
```

### 自定义工具

```python
from continuum_sdk.tools import CustomTool

class MyTool(CustomTool):
    @property
    def name(self) -> str:
        return "my_tool"

    @property
    def description(self) -> str:
        return "我的自定义工具"

    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "input": {"type": "string"}
            },
            "required": ["input"]
        }

    async def execute(self, **kwargs):
        return f"处理: {kwargs['input']}"
```

### 记忆系统

```python
from continuum_sdk.memory import Memory, MemoryTier

memory = Memory()

# 工作记忆（当前任务）
memory.working().add("temp_data", "value")

# 会话记忆（当前会话）
memory.session().add("user_preference", "Python")

# 项目记忆（项目级别）
memory.project().add("project_config", {...})

# 长期记忆（永久存储）
memory.longterm().add("learned_fact", "Python is awesome")

# 检索
value = memory.recall("user_preference")

# 遗忘
memory.forget("temp_data")
```

### 工作流

```python
from continuum_sdk.workflow import DAG, Node

# 创建 DAG
dag = DAG("my-workflow")

# 添加节点
dag.add(Node("step1", func=lambda: "step1"))
dag.add(Node("step2", func=lambda: "step2"))

# 设置依赖
dag.depends_on("step2", "step1")

# 执行
result = await dag.execute()
```

---

## 常见问题

### Q: 如何切换 AI 提供商？

```bash
# 临时切换（环境变量）
export SH_MODEL=gpt-4

# 永久切换（配置文件）
continuum config use openai
```

### Q: 如何查看 Token 使用情况？

在 TUI 中查看状态栏，或使用 SDK：

```python
from continuum_sdk import Agent

agent = Agent()
# ... 使用后
print(f"Tokens: {agent.total_tokens}")
```

### Q: 如何保存和恢复会话？

```python
# 自动保存
session = Session(name="my-session", auto_save=True)

# 手动保存
session.save()

# 恢复
session = Session.load("session-id")
```

### Q: 支持哪些模型？

- Anthropic: Claude 3 Haiku, Claude 3 Opus
- OpenAI: GPT-4, GPT-3.5 Turbo
- Google: Gemini Pro, Gemini Flash
- 自定义: 任何 OpenAI 兼容 API

---

## 故障排除

### 安装失败

```bash
# 检查 Python 版本（需要 3.8+）
python --version

# 使用 pip 更新
pip install --upgrade pip
pip install continuum --no-cache-dir
```

### API 错误

```bash
# 验证 API Key
continuum config validate

# 检查网络
curl https://api.anthropic.com/health

# 查看日志
export RUST_LOG=debug
continuum run "test"
```

### TUI 显示异常

```bash
# 检查终端支持
echo $TERM

# 重置终端
reset

# 使用兼容模式
TERM=xterm-256color continuum
```

### 配置问题

```bash
# 查看配置路径
continuum config show --source

# 重置配置
rm ~/.sh/config.toml
continuum config init
```

---

## 获取帮助

- GitHub Issues: https://github.com/xxx/continuum/issues
- 文档: https://continuum.readthedocs.io

---

*Continuum - Making AI Agents Easy*