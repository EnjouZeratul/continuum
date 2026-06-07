# Continuum SDK API Reference

> Version: 1.0.0
> 版本: 1.0.0

## Table of Contents

- [Core API](#core-api)
  - [Agent](#agent)
  - [Session](#session)
  - [Config](#config)
  - [BuiltinTools](#builtintools)
- [Error Handling](#error-handling)
  - [Error Types](#error-types)
  - [Convenience Functions](#convenience-functions)
- [Environment Variables](#environment-variables-module)
  - [Type-safe Access](#type-safe-access)
  - [Allowed Variables](#allowed-variables)
- [LLM Module](#llm-module)
  - [Clients](#clients)
  - [Types](#types)
- [Agent Intelligence](#agent-intelligence)
  - [Checkpoint](#checkpoint)
  - [History](#history)
  - [Planner](#planner)
  - [Progress](#progress)
  - [SelfCorrection](#selfcorrection)
- [Configuration](#configuration)
  - [Providers](#providers)
  - [Theme](#theme)
- [Security Module](#security-module)
- [Memory System](#memory-system)
- [RAG Module](#rag-module)
- [Render Module](#render-module)

---

## Core API

### Agent

The main entry point for interacting with the Continuum framework.
Continuum 框架的主入口点。

```python
from continuum_sdk import Agent

# Basic usage (auto-configures from environment)
# 基本用法（从环境变量自动配置）
agent = Agent()

# With explicit configuration
# 使用显式配置
from continuum_sdk import Config
config = Config.from_env()
agent = Agent(config=config)

# Check implementation preference
# 检查实现偏好
from continuum_sdk import HAS_RUST_BINDING, get_implementation_preference
print(HAS_RUST_BINDING)  # True if Rust binding available
print(get_implementation_preference())  # "rust" or "python"
```

#### Methods
#### 方法

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `run(prompt)` | `prompt: str` | `str` | Execute a prompt and return the result / 执行提示并返回结果 |
| `arun(prompt)` | `prompt: str` | `str` | Execute asynchronously / 异步执行 |
| `register_tool(name, func, ...)` | `name: str, func: Callable, description: str, parameters: dict` | `None` | Register a custom tool / 注册自定义工具 |
| `create_session()` | - | `Session` | Create a new session / 创建新会话 |
| `implementation` | - | `str` | Get current implementation type ("rust" or "python") / 获取当前实现类型 |

#### Tool Registration
#### 工具注册

```python
agent.register_tool(
    "calculator",
    lambda expr: eval(expr),  # Use ast.literal_eval for safety
    description="Evaluate math expressions",
    parameters={
        "type": "object",
        "properties": {
            "expression": {"type": "string"}
        }
    }
)
```

---

### BuiltinTools

Built-in tools for file operations, search, and shell commands.
用于文件操作、搜索和 Shell 命令的内置工具。

```python
from continuum_sdk import BuiltinTools

tools = BuiltinTools()

# File operations / 文件操作
content = tools.read_file("/path/to/file.txt")
tools.write_file("/path/to/file.txt", "Hello, World!")
tools.edit_file("/path/to/file.txt", "old", "new")

# Search / 搜索
results = tools.grep("pattern", path="/src", glob="*.py")
files = tools.glob("**/*.py", path="/project")

# Shell / Shell 命令
output = tools.bash("ls -la", timeout_ms=5000, working_dir="/project")

# List tools / 列出工具
available = tools.list_tools()
```

#### Methods
#### 方法

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `read_file(path, offset, limit)` | `path: str, offset?: int, limit?: int` | `str` | Read file contents / 读取文件内容 |
| `write_file(path, content)` | `path: str, content: str` | `str` | Write content to file / 写入文件 |
| `edit_file(path, old, new)` | `path: str, old: str, new: str` | `str` | Edit file by replacing text / 编辑文件 |
| `grep(pattern, path, glob)` | `pattern: str, path?: str, glob?: str` | `str` | Search file contents / 搜索文件内容 |
| `glob(pattern, path)` | `pattern: str, path?: str` | `str` | Find files matching pattern / 查找匹配模式的文件 |
| `bash(command, timeout_ms, working_dir)` | `command: str, timeout_ms?: int, working_dir?: str` | `str` | Execute shell command / 执行 Shell 命令 |
| `list_tools()` | - | `list[dict]` | List available tools / 列出可用工具 |

---

### Session

Session management for conversation persistence and recovery.
用于对话持久化和恢复的会话管理。

```python
from continuum_sdk import Session

# Create session / 创建会话
session = agent.create_session()
# Or directly / 或直接创建
session = Session(id="my-session")

# Recover session from checkpoint / 从检查点恢复会话
session = Session.recover("~/.continuum/checkpoints/session-001.json")
```

#### Methods
#### 方法

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `save()` | - | `str` | Save session and return ID / 保存会话并返回 ID |
| `recover(checkpoint_path)` | `checkpoint_path: str \| Path` | `Session` | Recover session from checkpoint (classmethod) / 从检查点恢复会话（类方法） |
| `load(path)` | `path: str \| Path` | `Session` | Load session from file (classmethod) / 从文件加载会话（类方法） |
| `clear()` | - | `None` | Clear session history / 清除会话历史 |
| `add_user_message(content)` | `content: str` | `Message` | Add user message / 添加用户消息 |
| `add_assistant_message(content)` | `content: str` | `Message` | Add assistant message / 添加助手消息 |
| `get_messages(limit)` | `limit?: int` | `list[Message]` | Get message history / 获取消息历史 |

#### Properties
#### 属性

| Property | Type | Description |
|----------|------|-------------|
| `id` | `str` | Session identifier / 会话标识符 |
| `created_at` | `datetime` | Creation timestamp / 创建时间戳 |
| `message_count` | `int` | Number of messages / 消息数量 |
| `cost` | `float` | Total accumulated cost / 累计成本 |
| `tokens` | `int` | Total token count / 总 token 数量 |

#### Recovery Example
#### 恢复示例

```python
# Recover a session from a checkpoint file
# 从检查点文件恢复会话
try:
    session = Session.recover("~/.continuum/checkpoints/chat-001.json")
    print(f"Recovered session: {session.id}")
    print(f"Messages: {session.message_count}")
except FileNotFoundError:
    print("Checkpoint file not found")
except ValueError as e:
    print(f"Invalid checkpoint format: {e}")
```

---

### Config

Configuration management with multi-provider support.
支持多提供商的配置管理。

```python
from continuum_sdk import Config, load_config

# From environment / 从环境变量
config = Config.from_env()

# From file / 从文件
config = load_config("config.toml")

# Explicit configuration / 显式配置
config = Config(
    provider="anthropic",
    model="claude-sonnet-4-6",
    api_key="sk-..."
)
```

#### AgentConfig Fields
#### AgentConfig 字段

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `provider` | `str` | `"anthropic"` | LLM provider name / LLM 提供商名称 |
| `model` | `str` | Provider default | Model identifier / 模型标识符 |
| `api_key` | `str \| None` | From env | API key / API 密钥 |
| `base_url` | `str \| None` | Provider default | API base URL / API 基础 URL |
| `max_tokens` | `int` | `4096` | Maximum response tokens / 最大响应 token 数 |
| `max_iterations` | `int` | `100` or `CONTINUUM_MAX_ITERATIONS` | Maximum iterations in agent loop / Agent 循环最大迭代次数 |
| `temperature` | `float` | `0.7` | Sampling temperature / 采样温度 |
| `timeout` | `float` | `60.0` | Request timeout in seconds / 请求超时时间（秒） |
| `budget` | `float \| None` | `None` | Optional cost budget limit / 可选的成本预算限制 |
| `system_prompt` | `str \| None` | `None` | Optional system prompt / 可选的系统提示 |

```python
from continuum_sdk.agent.runtime import AgentConfig

# Create config with max_iterations / 创建带 max_iterations 的配置
config = AgentConfig(
    name="my-agent",
    model="claude-sonnet-4-6",
    provider="anthropic",
    max_iterations=50,  # Limit agent loop iterations / 限制 agent 循环迭代次数
    max_tokens=8192,
)

# max_iterations can also be set via environment variable
# max_iterations 也可以通过环境变量设置
# CONTINUUM_MAX_ITERATIONS=200
```

---

## Error Handling

Unified error hierarchy for consistent error handling across the SDK.
用于 SDK 全局一致错误处理的统一错误层次结构。

```python
from continuum_sdk.errors import (
    ContinuumError,
    ConfigError,
    ToolExecutionError,
    LLMError,
    AuthenticationError,
    RateLimitError,
    SecurityError,
    ValidationError,
    ErrorContext,
)
```

### Error Types
### 错误类型

#### Error Hierarchy
#### 错误层次结构

```
ContinuumError (base)
├── ConfigError - Configuration issues / 配置问题
├── ToolExecutionError - Tool execution failures / 工具执行失败
├── LLMError - LLM API errors / LLM API 错误
│   ├── AuthenticationError - Auth failures / 认证失败
│   └── RateLimitError - Rate limit exceeded / 超出速率限制
├── SecurityError - Security violations / 安全违规
└── ValidationError - Input validation failures / 输入验证失败
```

#### ContinuumError

Base error class for all Continuum SDK errors.
所有 Continuum SDK 错误的基类。

```python
from continuum_sdk.errors import ContinuumError, ErrorContext

try:
    raise ContinuumError(
        "Something went wrong",
        code="E001",
        context={"operation": "test", "suggestion": "Try again"}
    )
except ContinuumError as e:
    print(e.message)      # "Something went wrong"
    print(e.code)         # "E001"
    print(e.context.suggestion)  # "Try again"
    print(e.to_dict())    # Serialize for logging/API
```

#### ConfigError

Raised when configuration is invalid or missing.
配置无效或缺失时抛出。

```python
from continuum_sdk.errors import ConfigError, config_error

raise ConfigError(
    "API key not found",
    code="CONFIG_MISSING_KEY",
    context={"key": "ANTHROPIC_API_KEY", "suggestion": "Set via environment variable"}
)

# Convenience function / 便捷函数
raise config_error("API key not found", key="ANTHROPIC_API_KEY", suggestion="Set ANTHROPIC_API_KEY")
```

#### ToolExecutionError

Raised when tool execution fails.
工具执行失败时抛出。

```python
from continuum_sdk.errors import ToolExecutionError, tool_error

raise ToolExecutionError(
    "File not found: /path/to/file.txt",
    tool_name="read_file",
    tool_args={"path": "/path/to/file.txt"}
)

# Convenience function / 便捷函数
raise tool_error("File not found", tool_name="read_file", suggestion="Check file path")
```

#### LLMError

Base error for LLM API operations.
LLM API 操作的基础错误。

```python
from continuum_sdk.errors import LLMError

raise LLMError(
    "Model not available",
    code="LLM_MODEL_NOT_FOUND",
    provider="anthropic",
    context={"model": "claude-3-opus"}
)
```

#### AuthenticationError

Raised when API authentication fails.
API 认证失败时抛出。

```python
from continuum_sdk.errors import AuthenticationError

raise AuthenticationError(
    "Invalid API key",
    code="AUTH_INVALID_KEY",
    provider="anthropic"
)
```

#### RateLimitError

Raised when rate limit is exceeded.
超出速率限制时抛出。

```python
from continuum_sdk.errors import RateLimitError

raise RateLimitError(
    "Rate limit exceeded",
    provider="openai",
    retry_after=60  # Seconds to wait before retry
)
```

### Convenience Functions
### 便捷函数

```python
from continuum_sdk.errors import (
    config_error,
    tool_error,
    validation_error,
    security_error,
)

# Create ConfigError with helpful context / 创建带上下文的 ConfigError
raise config_error("API key not found", key="ANTHROPIC_API_KEY", suggestion="Set env var")

# Create ToolExecutionError / 创建 ToolExecutionError
raise tool_error("File not found", tool_name="read_file", suggestion="Check path")

# Create ValidationError / 创建 ValidationError
raise validation_error("Invalid temperature", field="temperature", value=3.0, valid_range="0.0-2.0")

# Create SecurityError / 创建 SecurityError
raise security_error("Path traversal detected", operation="read_file", suggestion="Use absolute paths")
```

---

## Environment Variables Module

Type-safe environment variable access with CONTINUUM_ prefix support.
支持 CONTINUUM_ 前缀的类型安全环境变量访问。

```python
from continuum_sdk.env import get_str, get_int, get_bool, get_list
```

### Type-safe Access
### 类型安全访问

```python
from continuum_sdk.env import get_str, get_int, get_bool, get_list

# String value / 字符串值
api_key = get_str("API_KEY")  # Checks CONTINUUM_API_KEY, then API_KEY

# Integer with default / 带默认值的整数
timeout = get_int("TIMEOUT", default=30)
max_iterations = get_int("MAX_ITERATIONS", default=100)

# Boolean / 布尔值
debug = get_bool("DEBUG", default=False)
# True: "true", "1", "yes", "on"
# False: "false", "0", "no", "off"

# List (comma-separated) / 列表（逗号分隔）
models = get_list("MODELS", default=["gpt-4", "claude-3"])
# "gpt-4, claude-3, gemini-pro" -> ["gpt-4", "claude-3", "gemini-pro"]
```

### Environment Variable Prefix Priority
### 环境变量前缀优先级

1. `CONTINUUM_{NAME}` (preferred / 推荐)
2. `{NAME}` (fallback / 回退)

For example, `get_str("API_KEY")` checks:
- `CONTINUUM_API_KEY` first
- `API_KEY` if not found

### Allowed Variables
### 允许的变量

Only whitelisted environment variables are accessible for security.
出于安全考虑，只有白名单中的环境变量可访问。

| Category | Variables |
|----------|-----------|
| Core configuration | `API_KEY`, `BASE_URL`, `PROVIDER`, `MODEL`, `SMALL_MODEL`, `DEFAULT_MODEL`, `API_FORMAT` |
| Runtime settings | `LOG_LEVEL`, `MAX_TOKENS`, `TIMEOUT`, `MAX_ITERATIONS`, `TEMPERATURE`, `EFFORT_LEVEL` |
| Feature flags | `DEBUG`, `VERBOSE`, `DISABLE_TRAFFIC`, `AUDIT_ENABLED` |
| Paths | `WORKTREES_DIR`, `PLUGINS_DIR`, `AUDIT_LOG_PATH`, `THEME_CONFIG` |
| Lists | `MODELS`, `ALLOWED_TOOLS`, `BLOCKED_TOOLS`, `EXTRA_HEADERS` |
| Other | `AUDIT_RETENTION`, `USE_REAL_API` |

```python
# Example: Getting runtime settings / 示例：获取运行时设置
from continuum_sdk.env import get_int, get_bool

max_iterations = get_int("MAX_ITERATIONS", default=100)
debug_mode = get_bool("DEBUG", default=False)
timeout = get_int("TIMEOUT", default=60)
```

---

## LLM Module

### Clients

#### LlmClient (Base)

Abstract base class for all LLM clients.

```python
from continuum_sdk.llm import LlmClient

class CustomClient(LlmClient):
    async def chat(self, messages: list[Message]) -> ChatResponse:
        ...
    
    async def chat_stream(self, messages: list[Message]) -> AsyncIterator[StreamChunk]:
        ...
```

#### AnthropicClient

```python
from continuum_sdk.llm import AnthropicClient

client = AnthropicClient(
    api_key="sk-ant-...",
    model="claude-sonnet-4-6"
)
```

#### OpenAIClient

```python
from continuum_sdk.llm import OpenAIClient

client = OpenAIClient(
    api_key="sk-...",
    model="gpt-4.1"
)
```

#### GeminiClient

```python
from continuum_sdk.llm import GeminiClient

client = GeminiClient(
    api_key="...",
    model="gemini-2.5-pro"
)
```

---

### Types

#### Message

```python
from continuum_sdk.llm import Message, MessageRole

message = Message(
    role=MessageRole.USER,
    content="Hello, world!"
)
```

#### ChatResponse

```python
@dataclass
class ChatResponse:
    content: str
    role: MessageRole
    token_usage: TokenUsage
    finish_reason: str
```

#### StreamChunk

```python
@dataclass
class StreamChunk:
    content: str
    delta: str
    finish_reason: str | None
```

---

## Agent Intelligence

### Checkpoint

State snapshot and recovery for agent execution.

```python
from continuum_sdk.agent import Checkpoint, CheckpointManager

# Create checkpoint
checkpoint = Checkpoint.create(agent_state)

# Manager
manager = CheckpointManager(max_checkpoints=10)
manager.save(checkpoint)
recovered = manager.load(checkpoint_id)
```

### History

Conversation history management.

```python
from continuum_sdk.agent import History, HistoryEntry

history = History(max_entries=1000)
history.add_entry(HistoryEntry(role="user", content="Hello"))
entries = history.get_recent(10)
```

### Planner

Task planning and decomposition.

```python
from continuum_sdk.agent import Planner, Plan

planner = Planner()
plan = planner.create_plan(
    task="Build a REST API",
    context=agent_context
)
```

### Progress

Execution progress tracking.

```python
from continuum_sdk.agent import Progress, ProgressTracker

tracker = ProgressTracker()
tracker.start_step("data_processing")
tracker.complete_step("data_processing", result={"rows": 100})
```

### SelfCorrection

Error detection and auto-correction.

```python
from continuum_sdk.agent import SelfCorrection

corrector = SelfCorrection(max_retries=3)
result = await corrector.execute_with_retry(
    operation=risky_operation,
    error_handler=custom_handler
)
```

---

## Configuration

### Providers

Multi-provider management with 13 built-in providers.

```python
from continuum_sdk.config import (
    list_providers,
    get_default_model,
    list_models,
)

# List all providers
providers = list_providers()
# ['anthropic', 'openai', 'google', 'gemini', 'cohere', 
#  'huggingface', 'together', 'groq', 'deepseek', 'moonshot',
#  'glm', 'kimi']

# Get default model for a provider
model = get_default_model("anthropic")  # "claude-sonnet-4-6"

# List models for a provider
models = list_models("openai")
# ['gpt-5.5', 'gpt-5.4', 'gpt-5.4-mini', ...]
```

#### Built-in Providers

| Provider | Default Model | Models Count |
|----------|---------------|--------------|
| anthropic | claude-sonnet-4-6 | 8 |
| openai | gpt-5.5 | 9 |
| google | gemini-3.0-pro | 6 |
| gemini | gemini-3.0-pro | 6 |
| cohere | command | 2 |
| huggingface | (any) | 0 |
| together | meta-llama/Llama-3-70b-chat-hf | 5 |
| groq | llama-3.3-70b-versatile | 7 |
| deepseek | deepseek-v4-pro | 7 |
| moonshot | kimi-k2.6 | 5 |
| glm | glm-5.1 | 4 |
| kimi | kimi-k2.6 | 3 |
| qwen | qwen3.7-max | 3 |
| grok | grok-4-heavy | 2 |
| azure | gpt-4o | 0 |
| bedrock | anthropic.claude-sonnet-4-6 | 0 |
| ollama | llama3 | 0 |

### Theme

Theme system for TUI customization.

```python
from continuum_sdk.config import ThemeManager, ColorScheme, PresetTheme

# Use preset theme
theme_manager = ThemeManager()
theme_manager.apply_theme(PresetTheme.DARK)

# Custom theme
custom = ColorScheme(
    primary="#ff6b6b",
    secondary="#4ecdc4",
    background="#1a1a2e",
    foreground="#eaeaea"
)
theme_manager.set_custom_theme(custom)

# Save to file
theme_manager.save("theme.toml")
```

#### Preset Themes

- `DARK` - Dark mode (default)
- `LIGHT` - Light mode
- `NORD` - Nord color palette
- `DRACULA` - Dracula theme
- `GRUVBOX` - Gruvbox dark
- `CATPPUCCIN` - Catppuccin Mocha
- `TOKYO_NIGHT` - Tokyo Night
- `ONE_DARK` - One Dark

---

## Security Module

Path validation and permission checking.

```python
from continuum_sdk.security import (
    PathValidator,
    PermissionChecker,
    AuditLogger,
    ChangePreviewer,
    RiskLevel,
    Permission,
)

# Path validation
validator = PathValidator(base_dir="/workspace")
is_valid = validator.validate("/workspace/file.txt")

# Permission checking
checker = PermissionChecker()
result = checker.check("/file.txt", Permission.WRITE)
can_write = result.has_permission

# Audit logging
audit = AuditLogger(log_file="audit.json")
audit.log(operation="read", path="/file.txt", user="agent")

# Change preview
previewer = ChangePreviewer()
preview = previewer.preview_change("/file.txt", "new content")
# preview.risk_level == RiskLevel.LOW
```

---

## Memory System

Multi-tier memory with SQLite persistence.

```python
from continuum_sdk.memory import (
    Memory,
    SQLiteStorage,
    MemoryTier,
)

# Create memory system with SQLite
memory = Memory.create_with_sqlite_storage()

# Store memory
entry_id = memory.remember(
    tier=MemoryTier.WORKING,
    content="Important information",
    metadata={"source": "user"}
)

# Query memory
results = memory.recall(
    query="information",
    tier=MemoryTier.WORKING,
    limit=10
)
```

#### Memory Tiers

| Tier | Description | Persistence |
|------|-------------|-------------|
| `WORKING` | Current task context | In-memory |
| `SESSION` | Session-scoped data | SQLite |
| `PROJECT` | Project-level data | SQLite |
| `LONGTERM` | Long-term storage | SQLite |

---

## RAG Module

Vector storage for retrieval and search.

```python
from continuum_sdk.rag import InMemoryVectorStore, DistanceMetric

# Create vector store
store = InMemoryVectorStore(metric=DistanceMetric.COSINE)

# Add documents (requires pre-computed vectors)
store.upsert("doc_001", [0.1, 0.2, ...], {"text": "Hello world"})
store.upsert("doc_002", [0.3, 0.4, ...], {"text": "Python is great"})

# Search
results = store.search([0.15, 0.25, ...], top_k=5)
```

---

## Render Module

Markdown rendering for TUI.

```python
from continuum_sdk.render import MarkdownRenderer

renderer = MarkdownRenderer()
output = renderer.render("# Hello **World**")
# Returns rich.Text object for terminal display

# Custom style
renderer = MarkdownRenderer(
    code_theme="monokai",
    quote_style="italic"
)
```

---

## LLM Error Handling (Legacy)
## LLM 错误处理（旧版）

For unified error handling, see the [Error Handling](#error-handling) section above.
统一错误处理请参考上方的 [Error Handling](#error-handling) 章节。

```python
from continuum_sdk.llm import LlmError
from continuum_sdk.errors import RateLimitError, AuthenticationError, ConfigError

try:
    response = await client.chat(messages)
except RateLimitError as e:
    # Handle rate limiting / 处理速率限制
    await asyncio.sleep(e.retry_after)
except AuthenticationError as e:
    # Handle auth issues / 处理认证问题
    raise ConfigError("Invalid API key")
except LlmError as e:
    # Generic LLM error / 通用 LLM 错误
    logger.error(f"LLM error: {e}")
```

---

## Provider Environment Variables
## 提供商环境变量

API keys for different providers. These are used by Config for auto-configuration.
不同提供商的 API 密钥，用于 Config 的自动配置。

| Variable | Description | Providers |
|----------|-------------|-----------|
| `ANTHROPIC_API_KEY` | Anthropic API key / Anthropic API 密钥 | anthropic |
| `OPENAI_API_KEY` | OpenAI API key / OpenAI API 密钥 | openai |
| `GOOGLE_API_KEY` | Google API key / Google API 密钥 | google, gemini |
| `COHERE_API_KEY` | Cohere API key / Cohere API 密钥 | cohere |
| `HF_API_KEY` | HuggingFace API key / HuggingFace API 密钥 | huggingface |
| `TOGETHER_API_KEY` | Together API key / Together API 密钥 | together |
| `GROQ_API_KEY` | Groq API key / Groq API 密钥 | groq |
| `DEEPSEEK_API_KEY` | DeepSeek API key / DeepSeek API 密钥 | deepseek |
| `MOONSHOT_API_KEY` | Moonshot API key / Moonshot API 密钥 | moonshot, kimi |
| `GLM_API_KEY` | GLM API key / GLM API 密钥 | glm |

For SDK-specific environment variables with CONTINUUM_ prefix, see [Environment Variables Module](#environment-variables-module).
带有 CONTINUUM_ 前缀的 SDK 特定环境变量请参考 [Environment Variables Module](#environment-variables-module)。

---

## Version Info
## 版本信息

```python
import continuum_sdk

print(continuum_sdk.__version__)  # "1.0.0"
```
