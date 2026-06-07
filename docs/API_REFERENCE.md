# Continuum Python SDK API 参考

> 版本: v1.0.0
> 更新时间: 2026-05-30

---

## 目录

1. [核心类](#核心类)
2. [工具系统](#工具系统)
3. [记忆系统](#记忆系统)
4. [会话管理](#会话管理)
5. [配置管理](#配置管理)
6. [MCP集成](#mcp集成)
7. [错误处理](#错误处理)
8. [类型定义](#类型定义)

---

## 核心类

### Agent

主入口点，用于运行 AI Agent。

```python
from continuum_sdk import Agent
```

#### 构造函数

```python
Agent(
    name: str = "continuum-agent",
    model: str = "claude-sonnet-4-6",
    provider: str = "anthropic",
    system_prompt: str | None = None,
    tools: list[Tool] | None = None,
    memory: MemorySystem | None = None,
    enable_cache: bool = True,
    debug_mode: bool = False
)
```

**参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `name` | `str` | `"continuum-agent"` | Agent 名称 |
| `model` | `str` | `"claude-sonnet-4-6"` | 模型名称 |
| `provider` | `str` | `"anthropic"` | 提供商 |
| `system_prompt` | `str \| None` | `None` | 系统提示 |
| `tools` | `list[Tool] \| None` | `None` | 自定义工具列表 |
| `memory` | `MemorySystem \| None` | `None` | 记忆系统 |
| `enable_cache` | `bool` | `True` | 启用提示缓存 |
| `debug_mode` | `bool` | `False` | 调试模式 |

#### 方法

##### run()

执行单个任务。

```python
def run(
    task: str,
    context: dict | None = None,
    retry: bool = False
) -> str
```

**参数**:
- `task`: 任务描述
- `context`: 可选上下文
- `retry`: 失败时是否重试

**返回**: `str` - 任务结果

**示例**:
```python
agent = Agent()
result = agent.run("分析这个项目的结构")
```

##### create_session()

创建新会话。

```python
def create_session(
    name: str | None = None,
    auto_save: bool = False
) -> Session
```

**返回**: `Session` - 新会话实例

##### 会话保存与加载

```python
# 保存会话
session.save("path/to/session.json")

# 加载会话
from continuum_sdk import Session
session = Session.load("path/to/session.json")
agent = Agent(session=session)
```

---

### IntelligentAgent

具有任务规划能力的智能 Agent。

```python
from continuum_sdk.agent import IntelligentAgent, AgentMode
```

#### 构造函数

```python
IntelligentAgent(
    model: str = "claude-sonnet-4-6",
    mode: AgentMode = AgentMode.AUTONOMOUS,
    max_retries: int = 3,
    checkpoint_enabled: bool = True
)
```

#### 方法

##### plan()

规划任务执行步骤。

```python
async def plan(task: str) -> Plan
```

**返回**: `Plan` - 执行计划

##### execute()

执行计划。

```python
async def execute(plan: Plan) -> ExecutionResult
```

**返回**: `ExecutionResult` - 包含完成步骤信息

##### get_progress_text()

获取进度文本。

```python
def get_progress_text() -> str
```

**返回**: 如 `"[3/5] 60% in 10s ETA: 6s"`

---

## 工具系统

### BuiltinTools

内置工具集合。

```python
from continuum_sdk.tools import BuiltinTools

tools = BuiltinTools()
```

#### 方法

| 方法 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `read_file(path, offset?, limit?)` | `str, int?, int?` | `str` | 读取文件 |
| `write_file(path, content)` | `str, str` | `None` | 写入文件 |
| `edit_file(path, old, new)` | `str, str, str` | `bool` | 编辑文件 |
| `list_directory(path)` | `str` | `list[dict]` | 列出目录 |
| `grep(pattern, path?, glob?)` | `str, str?, str?` | `list[dict]` | 搜索内容 |
| `glob(pattern, path?)` | `str, str?` | `list[str]` | 查找文件 |
| `bash(command, timeout?, cwd?)` | `str, int?, str?` | `ToolResult` | 执行命令 |

### CustomTool

自定义工具基类。

```python
from continuum_sdk.tools import CustomTool

class MyTool(CustomTool):
    @property
    def name(self) -> str:
        return "my-tool"
    
    @property
    def description(self) -> str:
        return "工具描述"
    
    def parameters_schema(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "param1": {"type": "string"}
            },
            "required": ["param1"]
        }
    
    async def execute(self, **kwargs) -> str:
        # 实现逻辑
        return "结果"
```

### @tool 装饰器

快速创建工具。

```python
from continuum_sdk.tools import tool

@tool(
    name="greet",
    description="生成问候语",
    requires_confirmation=False,
    is_dangerous=False
)
async def greet(name: str, greeting: str = "Hello") -> str:
    return f"{greeting}, {name}!"
```

### ToolRegistry

工具注册表。

```python
from continuum_sdk.tools import get_registry, register_tool

registry = get_registry()

# 注册工具
registry.register(MyTool())
register_tool(AnotherTool())  # 使用默认注册表

# 列出工具
names = registry.list_names()
tools = registry.list()

# 执行工具
result = await registry.execute("tool_name", param1="value1")

# 获取元数据
meta = registry.get_meta("tool_name")
```

---

## 记忆系统

### MemorySystem

多分层记忆系统。

```python
from continuum_sdk.memory import MemorySystem

memory = MemorySystem()
```

#### 方法

##### remember()

存储记忆。

```python
def remember(
    content: str,
    layer: str = "semantic",  # episodic | semantic | procedural | working
    ttl: int | None = None,   # 过期时间（秒）
    metadata: dict | None = None
) -> str  # 返回记忆ID
```

##### query()

查询记忆。

```python
def query(
    query: str,
    layer: str | None = None,
    limit: int = 10,
    min_score: float = 0.5
) -> list[MemoryEntry]
```

##### forget()

删除记忆。

```python
def forget(memory_id: str) -> bool
```

##### clear()

清空记忆层。

```python
def clear(layer: str | None = None) -> int  # 返回删除数量
```

### MemoryEntry

记忆条目。

```python
@dataclass
class MemoryEntry:
    id: str
    content: str
    layer: str
    created_at: datetime
    expires_at: datetime | None
    metadata: dict
    score: float  # 查询时计算
```

---

## 会话管理

### Session

会话管理类。

```python
from continuum_sdk import Session
```

#### 构造函数

```python
Session(
    id: str | None = None,
    auto_save: bool = False,
    max_messages: int = 1000
)
```

#### 方法

| 方法 | 说明 |
|------|------|
| `add_user_message(content)` | 添加用户消息 |
| `add_assistant_message(content)` | 添加助手消息 |
| `add_system_message(content)` | 添加系统消息 |
| `get_messages()` | 获取所有消息 |
| `save(name)` | 保存会话 |
| `load(name)` | 加载会话 |
| `checkpoint(name?)` | 创建检查点 |
| `restore(checkpoint_id)` | 恢复检查点 |
| `prune_messages(keep_last)` | 清理消息 |
| `export(format)` | 导出会话 |

---

## 配置管理

### Config

配置管理类。

```python
from continuum_sdk import Config
```

#### 类方法

| 方法 | 说明 |
|------|------|
| `Config.from_env()` | 从环境变量加载 |
| `Config.from_file(path)` | 从文件加载 |
| `Config.from_dict(dict)` | 从字典创建 |

#### 方法

| 方法 | 说明 |
|------|------|
| `use(provider)` | 切换提供商 |
| `set_model(model)` | 设置模型 |
| `set_api_key(key)` | 设置API密钥 |
| `validate()` | 验证配置 |
| `to_dict()` | 导出为字典 |

#### 属性

| 属性 | 类型 | 说明 |
|------|------|------|
| `model` | `str` | 当前模型 |
| `provider` | `str` | 当前提供商 |
| `api_key` | `str` | API密钥（已脱敏） |
| `base_url` | `str` | API基础URL |

---

## MCP集成

### MCPToolRegistry

MCP 工具注册表。

```python
from continuum_sdk.tools import MCPToolRegistry, create_mcp_registry
```

#### 构造函数

```python
MCPToolRegistry(
    cache_size: int = 100
)
```

#### 方法

| 方法 | 说明 |
|------|------|
| `connect_stdio(name, command, args?)` | 连接stdio服务器 |
| `connect_sse(name, url)` | 连接SSE服务器 |
| `get_tools()` | 获取所有工具 |
| `execute(tool_name, **params)` | 执行工具 |
| `close()` | 关闭连接 |

### create_mcp_registry()

快速创建 MCP 注册表。

```python
registry = create_mcp_registry(
    servers=["filesystem", "github"],
    root_path="/project"
)
```

---

## 错误处理

### 异常层次

```
ContinuumError (基类)
├── ConfigError
│   ├── ApiKeyNotFoundError
│   └── InvalidConfigError
├── NetworkError
│   ├── ConnectionTimeoutError
│   └── SslError
├── ToolExecutionError
│   ├── ToolNotFoundError
│   └── InvalidParametersError
├── LLMError
│   ├── TokenLimitError
│   └── ContentFilterError
└── SessionError
    └── SessionCorruptedError
```

### 使用示例

```python
from continuum_sdk.errors import (
    ContinuumError,
    ConfigError,
    ToolExecutionError,
    LLMError
)

try:
    result = agent.run("任务")
except ConfigError as e:
    print(f"配置错误: {e}")
except ToolExecutionError as e:
    print(f"工具 {e.tool_name} 失败: {e.message}")
    if e.recoverable:
        # 可重试
        pass
except LLMError as e:
    print(f"LLM 错误: {e}")
except ContinuumError as e:
    print(f"通用错误: {e}")
```

---

## 类型定义

### ToolResult

```python
@dataclass
class ToolResult:
    call_id: str
    name: str
    content: str
    is_error: bool
    duration_ms: int
```

### ToolMeta

```python
@dataclass
class ToolMeta:
    name: str
    description: str
    category: str
    requires_confirmation: bool
    is_dangerous: bool
    parameters: dict
```

### Plan

```python
@dataclass
class Plan:
    id: str
    task: str
    steps: list[Step]
    created_at: datetime
    
    def to_dict(self) -> dict
```

### ExecutionResult

```python
@dataclass
class ExecutionResult:
    plan_id: str
    completed_steps: int
    total_steps: int
    success: bool
    duration_ms: int
    errors: list[str]
```

---

## 参考资源

- [快速入门](./user/quick_start.md)
- [最佳实践](./user/best_practices.md)
- [性能调优](./user/performance_guide.md)
- [故障排除](./user/troubleshooting.md)

---

*Continuum - 完整的 Agent 开发框架*