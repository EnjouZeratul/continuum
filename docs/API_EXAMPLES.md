# Continuum API 完整示例文档

> 版本: v1.0.0
> 更新时间: 2026-05-30

本文档为每个公开API提供完整示例，包含使用场景、参数说明、返回值示例。

---

## 目录

1. [核心 API](#核心-api)
2. [配置管理](#配置管理)
3. [LLM 客户端](#llm-客户端)
4. [工具系统](#工具系统)
5. [记忆系统](#记忆系统)
6. [会话管理](#会话管理)
7. [工作流](#工作流)
8. [RAG 检索](#rag-检索)
9. [权限管理](#权限管理)

---

## 核心 API

### Agent

**使用场景**: 创建和运行AI Agent执行任务。

#### 基础用法

```python
from continuum_sdk import Agent

# 场景1: 最简单的用法 - 自动配置
agent = Agent()
result = agent.run("分析当前目录的项目结构")
print(result)
# 返回值示例:
# "项目结构分析：
# - 根目录包含 README.md, pyproject.toml
# - src/ 目录包含核心源代码
# - tests/ 目录包含测试文件..."

# 场景2: 指定模型
agent = Agent(model="claude-sonnet-4-6")
result = agent.run("生成单元测试")

# 场景3: 挨定名称用于日志追踪
agent = Agent(name="code-reviewer", model="claude-opus-4-7")
```

#### 参数说明

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `name` | `str` | 否 | `"default"` | Agent名称，用于日志追踪 |
| `model` | `str` | 否 | `"claude-sonnet-4-6"` | 模型标识符 |
| `impl` | `str` | 否 | 自动选择 | 强制实现方式 (`"rust"` 或 `"python"`) |

#### 高级用法

```python
# 异步执行
import asyncio

async def async_task():
    agent = Agent()
    result = await agent.arun("复杂分析任务")
    return result

result = asyncio.run(async_task())

# 注册自定义工具
def my_calculator(expression: str) -> str:
    """安全计算数学表达式"""
    import ast
    try:
        result = ast.literal_eval(expression)
        return str(result)
    except Exception as e:
        return f"错误: {e}"

agent = Agent()
agent.register_tool(
    name="calculator",
    func=my_calculator,
    description="计算数学表达式",
    parameters={
        "type": "object",
        "properties": {
            "expression": {
                "type": "string",
                "description": "数学表达式如 '2+3*4'"
            }
        },
        "required": ["expression"]
    }
)

result = agent.run("使用calculator计算 15 * 8 + 20")
```

---

### Session

**使用场景**: 管理多轮对话和会话状态。

#### 基础用法

```python
from continuum_sdk import Agent, Session

# 场景1: 创建会话
session = Session()
session_id = session.id  # 自动生成的唯一ID
print(f"会话ID: {session_id}")
# 输出: 会话ID: sess_abc123def456

# 场景2: 通过Agent创建会话
agent = Agent()
session = agent.create_session()

# 场景3: 恢复已保存的会话
session = Session(session_id="saved_session_001")
```

#### 会话操作

```python
# 添加消息
session.add_user_message("请记住：项目使用Python 3.11")
session.add_assistant_message("好的，我记住了项目使用Python 3.11")
session.add_system_message("你是一个专业的代码助手")

# 获取消息历史
messages = session.get_messages()
for msg in messages:
    print(f"{msg.role}: {msg.content[:50]}...")

# 保存会话
session.save("my_project_session")

# 加载会话
loaded_session = Session.load("my_project_session")

# 导出会话
export_data = session.export(format="json")
# 返回值示例:
# {
#   "id": "sess_abc123",
#   "messages": [...],
#   "created_at": "2026-05-30T10:00:00Z",
#   "metadata": {}
# }
```

#### Checkpoint 使用

```python
from continuum_sdk.agent import CheckpointClient

client = CheckpointClient()

# 保存检查点
session_id = "my-session"
state = {"messages": [...], "context": {...}}
checkpoint_id = client.save(session_id, state)

# 加载检查点
restored = client.load(session_id, checkpoint_id)
# 或加载最新
latest = client.load(session_id)
```

---

## 配置管理

### Config

**使用场景**: 加载和管理多提供商配置。

#### 从环境变量加载

```python
from continuum_sdk import Config

# 场景1: 自动加载
config = Config.from_env()
print(f"当前模型: {config.model}")
print(f"当前提供商: {config.provider}")
# 输出:
# 当前模型: claude-sonnet-4-6
# 当前提供商: anthropic

# 场景2: 优先级
# 环境变量优先级: CONTINUUM_* > ANTHROPIC_* > OPENAI_*
# export CONTINUUM_MODEL=claude-opus-4-7
# config.model 将是 "claude-opus-4-7"
```

#### 从文件加载

```python
# 场景1: 从TOML文件加载
config = Config.from_file("~/.continuum/config.toml")

# 场景2: 从字典创建
config = Config.from_dict({
    "provider": "anthropic",
    "model": "claude-sonnet-4-6",
    "api_key": "${ANTHROPIC_API_KEY}"
})

# 验证配置
try:
    config.validate()
    print("配置有效")
except ConfigError as e:
    print(f"配置错误: {e}")
```

#### 切换提供商

```python
config = Config.from_env()

# 切换到OpenAI
config.use("openai")
print(config.model)  # 输出: gpt-4

# 切换回Anthropic
config.use("anthropic")
print(config.model)  # 输出: claude-sonnet-4-6

# 列出可用提供商
providers = config.list_providers()
# 返回值: ["anthropic", "openai", "google"]
```

---

## LLM 客户端

### LlmClient

**使用场景**: 直接调用LLM API，用于高级场景。

#### 创建客户端

```python
from continuum_sdk import LlmClient, AnthropicClient, OpenAIClient, GeminiClient

# 场景1: 使用统一接口（自动选择）
client = LlmClient.from_env()

# 场景2: 显式创建Anthropic客户端
anthropic = AnthropicClient(
    api_key="your-api-key",
    model="claude-sonnet-4-6"
)

# 场景3: 显式创建OpenAI客户端
openai = OpenAIClient(
    api_key="your-openai-key",
    model="gpt-4"
)

# 场景4: 显式创建Gemini客户端
gemini = GeminiClient(
    api_key="your-google-key",
    model="gemini-pro"
)
```

#### 发送消息

```python
from continuum_sdk import Message, MessageRole

# 创建消息
messages = [
    Message(role=MessageRole.USER, content="你好"),
    Message(role=MessageRole.ASSISTANT, content="你好！有什么可以帮助你的？"),
    Message(role=MessageRole.USER, content="写一首关于代码的诗"),
]

# 同步调用
response = client.chat(messages)
print(response.content)
print(f"Token使用: {response.usage}")
# 输出:
# 代码如诗行行美，
# 逻辑严谨韵味长...
# Token使用: TokenUsage(input=45, output=128)

# 异步调用
async def async_chat():
    response = await client.achat(messages)
    return response

response = asyncio.run(async_chat())
```

#### 流式响应

```python
# 同步流式
for chunk in client.stream(messages):
    print(chunk.content, end="", flush=True)
# 输出: 代码如诗...

# 异步流式
async def async_stream():
    async for chunk in client.astream(messages):
        print(chunk.content, end="", flush=True)

asyncio.run(async_stream())
```

---

## 工具系统

### BuiltinTools

**使用场景**: 使用内置文件、搜索、Shell工具。

```python
from continuum_sdk import BuiltinTools

tools = BuiltinTools()

# 文件操作
content = tools.read_file("README.md")
# 返回值: 文件内容字符串

tools.write_file("output.txt", "Hello, Continuum!")
# 返回值: None

success = tools.edit_file("config.py", old="DEBUG=False", new="DEBUG=True")
# 返回值: True（成功）或 False（未找到）

entries = tools.list_directory("src/")
# 返回值: [{"name": "main.py", "type": "file"}, {"name": "utils", "type": "dir"}]

# 搜索操作
matches = tools.grep("def test_", path="tests/", glob="*.py")
# 返回值: [
#   {"file": "tests/test_main.py", "line": 10, "content": "def test_main():"},
#   {"file": "tests/test_utils.py", "line": 5, "content": "def test_helper():"}
# ]

files = tools.glob("**/*.py", path="src/")
# 返回值: ["src/main.py", "src/utils/helpers.py", ...]

# Shell执行
result = tools.bash("git status --short", timeout_ms=5000)
# 返回值: ToolResult(
#   call_id="call_abc123",
#   name="bash",
#   content="M README.md\n?? new_file.py",
#   is_error=False,
#   duration_ms=234
# )
```

### 自定义工具

**使用场景**: 创建自定义工具扩展Agent能力。

#### 装饰器方式

```python
from continuum_sdk.tools import tool

@tool(
    name="weather",
    description="获取指定城市的天气信息",
    requires_confirmation=False
)
async def get_weather(city: str, unit: str = "celsius") -> str:
    """
    获取天气信息
    
    Args:
        city: 城市名称
        unit: 温度单位 (celsius/fahrenheit)
    
    Returns:
        天气描述字符串
    """
    # 实际实现会调用天气API
    return f"{city} 天气: 晴, 25°{unit[0].upper()}"

# 工具自动注册
```

#### 类方式

```python
from continuum_sdk.tools import CustomTool

class DatabaseQueryTool(CustomTool):
    """数据库查询工具"""
    
    @property
    def name(self) -> str:
        return "db_query"
    
    @property
    def description(self) -> str:
        return "执行SQL查询"
    
    @property
    def category(self) -> str:
        return "database"
    
    @property
    def is_dangerous(self) -> bool:
        return True  # 需要用户确认
    
    def parameters_schema(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "SQL查询语句"
                },
                "readonly": {
                    "type": "boolean",
                    "description": "是否只读查询",
                    "default": True
                }
            },
            "required": ["query"]
        }
    
    async def execute(self, **kwargs) -> str:
        query = kwargs["query"]
        readonly = kwargs.get("readonly", True)
        
        # 实际实现
        return f"查询结果: {query}"

# 注册
from continuum_sdk.tools import get_registry
registry = get_registry()
registry.register(DatabaseQueryTool())
```

---

## 记忆系统

### MemorySystem

**使用场景**: 分层存储和检索 Agent 记忆。

```python
# Illustrative - requires session context
from continuum_sdk.api import MemorySystem

memory = MemorySystem(session_id="my-session")

# Working: 当前对话上下文
memory.store("working", "用户请求生成测试用例")

# Session: 会话级别事实
memory.store("session", "项目使用 pytest 作为测试框架")

# Project: 项目知识
memory.store("project", "运行测试命令: pytest tests/")

# Long-term: 跨项目知识
memory.store("long_term", "Python 3.11 支持 match 语句")
```

#### 查询记忆

```python
# 基本查询
results = memory.query("如何运行测试")
for entry in results:
    print(f"- {entry['content']}")
    # 输出: 运行测试命令: pytest tests/

# 指定层级查询
results = memory.query("pytest", tier="session", limit=5)

# 删除特定记忆
memory.delete("session", "mem_id")

# 清空特定层级
count = memory.clear("working")
print(f"清除了 {count} 条记忆")
```

---

## 工作流

### DAG 工作流

**使用场景**: 构建复杂任务流程。

```python
from continuum_sdk.workflow import DAG, Node

# 创建工作流
dag = DAG(id="code-review-pipeline")

# 添加节点
dag.add(Node("analyze", func=lambda: "分析代码库结构和依赖"))
dag.add(Node("lint", func=lambda: "代码检查完成").depends_on("analyze"))
dag.add(Node("test", func=lambda: "测试完成").depends_on("analyze"))
dag.add(Node("report", func=lambda: "报告生成").depends_on("lint", "test"))

# 查看工作流结构
print(dag.visualize())
# 输出:
# DAG: code-review-pipeline
# ----------------------------------------
#   analyze <- [none]
#   lint <- [analyze]
#   test <- [analyze]
#   report <- [lint, test]

# 执行工作流
async def run_workflow():
    result = await dag.execute()
    print(f"状态: {result.status.value}")
    print(f"执行顺序: {result.execution_order()}")
    for node_id, node_result in result.get_all_outputs().items():
        print(f"  {node_id}: {node_result}")
    return result

result = asyncio.run(run_workflow())
```

#### 并行执行

```python
# analyze, lint, test 可并行，report 依赖前两者
dag = DAG(id="parallel-analysis")

def fetch_a(): return "A结果"
def fetch_b(): return "B结果"
def fetch_c(): return "C结果"
def summarize(a=None, b=None, c=None): return f"汇总: {a}, {b}, {c}"

dag.add(Node("a", func=fetch_a))
dag.add(Node("b", func=fetch_b))
dag.add(Node("c", func=fetch_c))
dag.add(Node("summary", func=summarize).depends_on("a", "b", "c"))

# 执行时 a, b, c 并行，然后执行 summary
async def run_parallel():
    result = await dag.execute(parallel=True, max_workers=3)
    return result

result = asyncio.run(run_parallel())
```

---

## RAG 检索

### VectorStore

**使用场景**: 向量存储和相似度搜索。

```python
from continuum_sdk.rag import InMemoryVectorStore, DistanceMetric

# 创建向量存储
store = InMemoryVectorStore(metric=DistanceMetric.COSINE)

# 添加向量
store.upsert(
    id="doc_001",
    vector=[0.1, 0.2, 0.3, ...],  # 向量数据
    metadata={"source": "README.md", "section": "intro"}
)

# 批量添加
vectors = [
    ("doc_002", [0.4, 0.5, 0.6, ...], {"source": "API.md"}),
    ("doc_003", [0.7, 0.8, 0.9, ...], {"source": "GUIDE.md"}),
]
store.upsert_batch(vectors)

# 搜索
results = store.search(
    vector=[0.15, 0.25, 0.35, ...],
    top_k=5
)
for result in results:
    print(f"{result.id}: {result.score:.4f}")
    print(f"  元数据: {result.metadata}")

# 获取向量数量
count = store.count()
print(f"存储了 {count} 个向量")

# 删除
store.delete("doc_001")

# 清空存储
store.clear()
```

### RetrieverEngine

**使用场景**: 文档检索和上下文构建。

```python
from continuum_sdk.rag import (
    DefaultRetrieverEngine,
    MockEmbeddingModel,
    Document,
    FixedSizeChunker,
)

# 创建检索引擎
engine = DefaultRetrieverEngine(
    embedding_model=MockEmbeddingModel(dimension=128),
    chunker=FixedSizeChunker(chunk_size=500, overlap=50)
)

# 索引文档
async def index_and_retrieve():
    docs = [
        Document(content="API密钥配置方法...", source="docs/API.md"),
        Document(content="用户指南内容...", source="docs/GUIDE.md"),
    ]
    doc_ids = await engine.index(docs)
    
    # 检索
    results = await engine.retrieve(query="如何配置API密钥", top_k=3)
    for doc in results:
        print(f"ID: {doc.doc_id}")
        print(f"内容: {doc.content[:100]}...")
        print(f"分数: {doc.score:.4f}")
        print(f"来源: {doc.source}")

asyncio.run(index_and_retrieve())
```

---

## 权限管理

### PermissionManager

**使用场景**: 管理操作权限和安全策略。

```python
from continuum_sdk.permission import (
    PermissionManager,
    PermissionPolicy,
    SecurityLevel,
    PermissionAction,
    PermissionRequest,
)

# 创建权限管理器（带安全策略）
policy = PermissionPolicy(level=SecurityLevel.STANDARD)
pm = PermissionManager(policy=policy)

# 配置安全策略
policy.trusted_paths.append("/workspace/safe_dir")
policy.blocked_commands.append("rm -rf")

# 检查权限
request = pm.request_command("git", ["status", "--short"])
response = pm.check_permission(request)
if response.is_allowed():
    # 执行操作
    pass

# 文件操作权限
file_request = pm.request_file_write("/workspace/output.txt", "content preview")
response = pm.check_permission(file_request)

# 设置交互提示回调（用于 TUI）
def prompt_user(req: PermissionRequest) -> PermissionResponse:
    # 在 TUI 中显示提示，获取用户决定
    from continuum_sdk.permission import PermissionDecision
    user_choice = show_confirmation_dialog(req.action.description())
    if user_choice == "allow":
        return PermissionResponse(request_id=req.id, decision=PermissionDecision.ALLOW)
    else:
        return PermissionResponse(request_id=req.id, decision=PermissionDecision.DENY)

pm.set_prompt_callback(prompt_user)

# 安全级别切换
# TRUSTED: 自动允许所有操作（适合开发环境）
# STANDARD: 仅对危险操作提示确认
# STRICT: 所有操作需确认
# PARANOID: 严格模式 + 完整审计日志
pm.set_policy(PermissionPolicy.trusted())

# 查看审计日志
audit_log = pm.get_audit_log()
for entry in audit_log:
    print(f"{entry['timestamp']}: {entry['action']} -> {entry['decision']}")
```

---

## 参考资源

- [API参考](./API_REFERENCE.md)
- [最佳实践](./user/best_practices.md)
- [性能调优](./user/performance_guide.md)
- [故障排除](./user/troubleshooting.md)

---

*Continuum - 完整的Agent开发框架*