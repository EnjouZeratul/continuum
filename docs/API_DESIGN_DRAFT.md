# Continuum Python SDK API 设计草稿

> 版本: v1.0 (已实现)
> 作者: Terminal 3
> 日期: 2026-05-11
> 更新日期: 2026-05-26
> 状态: ✅ SDK 基础 API 已实现 (参见 python/continuum_sdk/)

---

## 一、Tool API 设计

### 1.1 builtin.py - 内置工具

```python
"""内置工具 API

提供对 Continuum 内置工具的 Python 访问。
"""

from typing import Any, Dict, List, Optional
from enum import Enum

class ToolCategory(Enum):
    """工具分类"""
    FILE_OPS = "file_ops"
    SEARCH = "search"
    SHELL = "shell"
    NETWORK = "network"
    CODE_ANALYSIS = "code_analysis"
    MEMORY = "memory"
    WORKFLOW = "workflow"
    SYSTEM = "system"
    OTHER = "other"


class ToolMeta:
    """工具元数据"""
    name: str
    description: str
    category: ToolCategory
    requires_confirmation: bool
    is_dangerous: bool
    parameters: Dict[str, Any]


class ToolResult:
    """工具执行结果"""
    call_id: str
    name: str
    content: str
    is_error: bool
    duration_ms: int


class BuiltinTools:
    """内置工具集合

    Usage:
        from continuum_sdk.tools.builtin import BuiltinTools

        tools = BuiltinTools()

        # 文件操作
        content = tools.read_file("path/to/file")
        tools.write_file("path/to/file", content)
        tools.edit_file("path/to/file", old="foo", new="bar")

        # 搜索
        matches = tools.grep("pattern", path="src/")
        files = tools.glob("**/*.py")

        # Shell
        result = tools.bash("echo hello")

        # 代码分析
        definition = tools.go_to_definition("src/main.rs", line=10, column=5)
        refs = tools.find_references("src/main.rs", line=10, column=5)
    """

    def __init__(self):
        """初始化内置工具"""
        ...

    # ==================== 文件操作 ====================

    def read_file(
        self,
        path: str,
        offset: Optional[int] = None,
        limit: Optional[int] = None
    ) -> str:
        """读取文件内容

        Args:
            path: 文件路径
            offset: 起始行号（可选）
            limit: 读取行数（可选）

        Returns:
            文件内容
        """
        ...

    def write_file(self, path: str, content: str) -> None:
        """写入文件内容

        Args:
            path: 文件路径
            content: 写入内容

        Note:
            此操作需要用户确认
        """
        ...

    def edit_file(self, path: str, old: str, new: str) -> bool:
        """编辑文件（查找替换）

        Args:
            path: 文件路径
            old: 要替换的文本
            new: 新文本

        Returns:
            是否成功
        """
        ...

    def list_directory(self, path: str) -> List[Dict[str, Any]]:
        """列出目录内容

        Args:
            path: 目录路径

        Returns:
            条目列表，每项包含 name, type (file/dir)
        """
        ...

    # ==================== 搜索 ====================

    def grep(
        self,
        pattern: str,
        path: Optional[str] = None,
        glob: Optional[str] = None
    ) -> List[Dict[str, Any]]:
        """搜索文件内容

        Args:
            pattern: 正则表达式
            path: 搜索路径（可选）
            glob: 文件过滤模式（可选）

        Returns:
            匹配结果列表
        """
        ...

    def glob(self, pattern: str, path: Optional[str] = None) -> List[str]:
        """查找文件

        Args:
            pattern: glob 模式（如 "**/*.py"）
            path: 搜索路径（可选）

        Returns:
            匹配的文件路径列表
        """
        ...

    # ==================== Shell ====================

    def bash(
        self,
        command: str,
        timeout: Optional[int] = None,
        working_dir: Optional[str] = None
    ) -> ToolResult:
        """执行 Shell 命令

        Args:
            command: bash 命令
            timeout: 超时时间（毫秒）
            working_dir: 工作目录

        Returns:
            执行结果

        Note:
            此操作需要用户确认
        """
        ...

    # ==================== 代码分析 ====================

    def go_to_definition(
        self,
        file: str,
        line: int,
        column: int
    ) -> Optional[Dict[str, Any]]:
        """跳转到定义

        Args:
            file: 文件路径
            line: 行号（1-based）
            column: 列号（1-based）

        Returns:
            定义位置，包含 file, line, column
        """
        ...

    def find_references(
        self,
        file: str,
        line: int,
        column: int
    ) -> List[Dict[str, Any]]:
        """查找引用

        Args:
            file: 文件路径
            line: 行号
            column: 列号

        Returns:
            引用位置列表
        """
        ...

    # ==================== 工具元数据 ====================

    def list_tools(self) -> List[ToolMeta]:
        """列出所有内置工具"""
        ...

    def get_tool_meta(self, name: str) -> Optional[ToolMeta]:
        """获取工具元数据"""
        ...
```

### 1.2 custom.py - 自定义工具

```python
"""自定义工具 API

用于创建和注册自定义工具。
"""

from typing import Any, Callable, Dict, List, Optional
from abc import ABC, abstractmethod

class CustomTool(ABC):
    """自定义工具基类

    Usage:
        from continuum_sdk.tools.custom import CustomTool, tool

        # 方式1: 继承类
        class MyTool(CustomTool):
            @property
            def name(self) -> str:
                return "my_tool"

            @property
            def description(self) -> str:
                return "My custom tool"

            def parameters_schema(self) -> Dict[str, Any]:
                return {
                    "type": "object",
                    "properties": {
                        "input": {"type": "string"}
                    },
                    "required": ["input"]
                }

            async def execute(self, **kwargs) -> str:
                return f"Processed: {kwargs['input']}"

        # 方式2: 装饰器
        @tool(name="hello", description="Say hello")
        async def hello_tool(name: str) -> str:
            return f"Hello, {name}!"
    """

    @property
    @abstractmethod
    def name(self) -> str:
        """工具名称"""
        ...

    @property
    @abstractmethod
    def description(self) -> str:
        """工具描述"""
        ...

    @abstractmethod
    def parameters_schema(self) -> Dict[str, Any]:
        """参数 JSON Schema"""
        ...

    @abstractmethod
    async def execute(self, **kwargs) -> str:
        """执行工具"""
        ...

    @property
    def category(self) -> str:
        """工具分类"""
        return "other"

    @property
    def requires_confirmation(self) -> bool:
        """是否需要用户确认"""
        return False

    @property
    def is_dangerous(self) -> bool:
        """是否为危险操作"""
        return False


def tool(
    name: str,
    description: str,
    parameters: Optional[Dict[str, Any]] = None,
    requires_confirmation: bool = False,
    is_dangerous: bool = False
) -> Callable:
    """工具装饰器

    Args:
        name: 工具名称
        description: 工具描述
        parameters: 参数 Schema（可选，自动推断）
        requires_confirmation: 是否需要确认
        is_dangerous: 是否危险

    Usage:
        @tool(name="add", description="Add two numbers")
        async def add(a: int, b: int) -> int:
            return a + b
    """
    ...


class ToolRegistry:
    """工具注册表

    Usage:
        from continuum_sdk.tools.custom import ToolRegistry, CustomTool

        registry = ToolRegistry()

        # 注册自定义工具
        registry.register(MyTool())

        # 列出所有工具
        tools = registry.list()

        # 执行工具
        result = await registry.execute("my_tool", input="test")
    """

    def __init__(self):
        """初始化注册表"""
        ...

    def register(self, tool: CustomTool) -> None:
        """注册工具

        Args:
            tool: 自定义工具实例
        """
        ...

    def unregister(self, name: str) -> bool:
        """注销工具"""
        ...

    def get(self, name: str) -> Optional[CustomTool]:
        """获取工具"""
        ...

    def list(self) -> List[CustomTool]:
        """列出所有工具"""
        ...

    async def execute(self, name: str, **kwargs) -> str:
        """执行工具

        Args:
            name: 工具名称
            **kwargs: 工具参数

        Returns:
            执行结果
        """
        ...
```

---

## 二、Memory API 设计

### 2.1 layers.py - 分层记忆

```python
"""分层记忆 API

提供 Working -> Session -> Project -> LongTerm 四层记忆。
"""

from typing import Any, Dict, List, Optional
from datetime import datetime
from enum import Enum


class MemoryTier(Enum):
    """记忆层级"""
    WORKING = "working"      # 当前对话上下文
    SESSION = "session"     # 会话记忆
    PROJECT = "project"     # 项目知识库
    LONG_TERM = "long_term" # 跨项目知识


class MemoryEntry:
    """记忆条目"""
    id: str
    tier: MemoryTier
    content: str
    metadata: Dict[str, Any]
    created_at: datetime
    last_accessed: datetime
    access_count: int
    importance: float


class Memory:
    """分层记忆系统

    Usage:
        from continuum_sdk.memory import Memory, MemoryTier

        memory = Memory(session_id="session-123")

        # 存储记忆
        memory.store("working", "Important fact")
        memory.store("project", "Project config")

        # 查询记忆
        results = memory.query("fact")

        # 获取特定层级
        working = memory.get_tier(MemoryTier.WORKING)

        # 统计
        stats = memory.stats()
    """

    def __init__(self, session_id: str):
        """初始化记忆系统

        Args:
            session_id: 会话 ID
        """
        ...

    def remember(
        self,
        content: str,
        tier: MemoryTier = MemoryTier.WORKING,
        metadata: Optional[Dict[str, Any]] = None,
        importance: float = 0.5
    ) -> str:
        """存储记忆

        Args:
            content: 记忆内容
            tier: 记忆层级
            metadata: 元数据（可选）
            importance: 重要性分数 (0.0-1.0)

        Returns:
            记忆 ID
        """
        ...

    def recall(
        self,
        query: str,
        tier: Optional[MemoryTier] = None,
        limit: int = 10
    ) -> List[MemoryEntry]:
        """查询记忆

        Args:
            query: 查询文本
            tier: 限制层级（可选）
            limit: 结果数量限制

        Returns:
            匹配的记忆条目列表
        """
        ...

    def get(self, tier: MemoryTier, memory_id: str) -> Optional[MemoryEntry]:
        """获取特定记忆

        Args:
            tier: 记忆层级
            memory_id: 记忆 ID

        Returns:
            记忆条目（如果存在）
        """
        ...

    def forget(self, tier: MemoryTier, memory_id: str) -> bool:
        """删除记忆

        Args:
            tier: 记忆层级
            memory_id: 记忆 ID

        Returns:
            是否成功删除
        """
        ...

    def clear(self, tier: MemoryTier) -> int:
        """清空指定层级

        Args:
            tier: 记忆层级

        Returns:
            删除的记忆数量
        """
        ...

    def stats(self) -> Dict[MemoryTier, int]:
        """获取各层级统计

        Returns:
            各层级记忆数量
        """
        ...

    # ==================== 便捷方法 ====================

    def working(self) -> 'TierProxy':
        """获取工作记忆代理

        Usage:
            memory.working().add("临时信息")
            results = memory.working().search("关键词")
        """
        ...

    def session(self) -> 'TierProxy':
        """获取会话记忆代理"""
        ...

    def project(self) -> 'TierProxy':
        """获取项目记忆代理"""
        ...

    def long_term(self) -> 'TierProxy':
        """获取长期记忆代理"""
        ...


class TierProxy:
    """层级代理

    Usage:
        # 便捷访问特定层级
        memory.working().add("内容")
        memory.working().search("查询")
        memory.working().clear()
    """

    def __init__(self, memory: Memory, tier: MemoryTier):
        ...

    def add(self, content: str, **kwargs) -> str:
        """添加记忆"""
        ...

    def search(self, query: str, limit: int = 10) -> List[MemoryEntry]:
        """搜索记忆"""
        ...

    def get(self, memory_id: str) -> Optional[MemoryEntry]:
        """获取记忆"""
        ...

    def remove(self, memory_id: str) -> bool:
        """删除记忆"""
        ...

    def clear(self) -> int:
        """清空层级"""
        ...

    def count(self) -> int:
        """获取数量"""
        ...
```

---

## 三、Workflow API 设计

### 3.1 dag.py - 工作流 DAG

```python
"""工作流 DAG API

定义和执行 DAG 工作流。
"""

from typing import Any, Callable, Dict, List, Optional
from enum import Enum
from dataclasses import dataclass


class NodeStatus(Enum):
    """节点状态"""
    PENDING = "pending"
    RUNNING = "running"
    SUCCESS = "success"
    FAILED = "failed"
    SKIPPED = "skipped"


@dataclass
class NodeResult:
    """节点执行结果"""
    node_id: str
    status: NodeStatus
    output: Any
    error: Optional[str]
    duration_ms: int


class Node:
    """工作流节点

    Usage:
        from continuum_sdk.workflow import Node

        # 创建节点
        node = Node("process", func=process_data)

        # 添加依赖
        node.depends_on("fetch")
    """

    def __init__(
        self,
        id: str,
        func: Optional[Callable] = None,
        name: Optional[str] = None,
        description: Optional[str] = None
    ):
        """初始化节点

        Args:
            id: 节点 ID
            func: 执行函数（可选）
            name: 显示名称
            description: 节点描述
        """
        ...

    def depends_on(self, *node_ids: str) -> 'Node':
        """添加依赖节点

        Args:
            *node_ids: 依赖的节点 ID

        Returns:
            self（支持链式调用）
        """
        ...

    def set_func(self, func: Callable) -> 'Node':
        """设置执行函数"""
        ...


class DAG:
    """工作流 DAG

    Usage:
        from continuum_sdk.workflow import DAG, Node

        # 创建 DAG
        dag = DAG("my_workflow")

        # 添加节点
        dag.add(Node("fetch", func=fetch_data))
        dag.add(Node("process", func=process).depends_on("fetch"))
        dag.add(Node("save", func=save).depends_on("process"))

        # 执行
        result = await dag.execute()

        # 获取结果
        output = result.get_output("save")
    """

    def __init__(self, id: str, name: Optional[str] = None):
        """初始化 DAG

        Args:
            id: DAG ID
            name: 显示名称
        """
        ...

    def add(self, node: Node) -> 'DAG':
        """添加节点

        Args:
            node: 工作流节点

        Returns:
            self（支持链式调用）
        """
        ...

    def get(self, node_id: str) -> Optional[Node]:
        """获取节点"""
        ...

    def remove(self, node_id: str) -> bool:
        """移除节点"""
        ...

    def depends_on(self, node_id: str, *depends: str) -> 'DAG':
        """添加依赖关系

        Args:
            node_id: 节点 ID
            *depends: 依赖的节点 ID

        Returns:
            self
        """
        ...

    def validate(self) -> List[str]:
        """验证 DAG

        Returns:
            错误消息列表（空列表表示验证通过）
        """
        ...

    async def execute(
        self,
        inputs: Optional[Dict[str, Any]] = None,
        parallel: bool = True
    ) -> 'DAGResult':
        """执行工作流

        Args:
            inputs: 输入参数
            parallel: 是否并行执行独立节点

        Returns:
            执行结果
        """
        ...

    def visualize(self) -> str:
        """生成可视化字符串

        Returns:
            ASCII 图形表示
        """
        ...


class DAGResult:
    """DAG 执行结果"""

    def __init__(self, dag_id: str):
        ...

    @property
    def status(self) -> NodeStatus:
        """整体状态"""
        ...

    def get_output(self, node_id: str) -> Optional[Any]:
        """获取节点输出

        Args:
            node_id: 节点 ID

        Returns:
            节点输出结果
        """
        ...

    def get_result(self, node_id: str) -> Optional[NodeResult]:
        """获取节点结果

        Args:
            node_id: 节点 ID

        Returns:
            节点执行结果
        """
        ...

    def get_all_outputs(self) -> Dict[str, Any]:
        """获取所有节点输出"""
        ...

    def failed_nodes(self) -> List[str]:
        """获取失败的节点 ID"""
        ...

    def execution_order(self) -> List[str]:
        """获取实际执行顺序"""
        ...


# ==================== 便捷函数 ====================

def workflow(id: str) -> DAG:
    """创建工作流的便捷函数

    Usage:
        from continuum_sdk.workflow import workflow, node

        @workflow("example")
        def my_workflow():
            @node("step1")
            async def step1():
                return "result1"

            @node("step2", depends=["step1"])
            async def step2(prev):
                return f"processed: {prev}"

            return step1, step2
    """
    ...


def node(
    id: str,
    depends: Optional[List[str]] = None
) -> Callable:
    """节点装饰器

    Args:
        id: 节点 ID
        depends: 依赖节点列表
    """
    ...
```

---

## 四、示例代码草稿

### 4.1 hello_agent.py

```python
"""Hello Agent 示例

最简单的 Agent 使用示例。
"""

from continuum_sdk import Agent

async def main():
    # 创建 Agent
    agent = Agent()

    # 发送消息（异步）
    response = await agent.arun("Hello, Continuum!")
    print(response)

    # 使用内置工具
    from continuum_sdk import BuiltinTools
    tools = BuiltinTools()
    content = tools.read_file("README.md")
    print(content)

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
```

### 4.2 session_example.py

```python
"""会话管理示例

展示会话创建、保存、恢复。
"""

from continuum_sdk import Agent, Session
from continuum_sdk.agent import CheckpointClient

async def main():
    # 创建 Agent
    agent = Agent()

    # 创建新会话
    session = agent.create_session()
    print(f"Session ID: {session.id}")

    # 在会话中工作
    await agent.arun("Remember my name is Alice")

    # 保存检查点
    checkpoint_client = CheckpointClient()
    checkpoint_id = checkpoint_client.save(session.id, {"state": session.export()})
    print(f"Checkpoint: {checkpoint_id}")

    # 稍后恢复
    restored_state = checkpoint_client.load(session.id, checkpoint_id)
    agent2 = Agent()
    await agent2.arun("What's my name?")  # 应该记得 "Alice"

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
```

### 4.3 custom_tools.py

```python
"""自定义工具示例

展示如何创建和注册自定义工具。
"""

from continuum_sdk.tools import CustomTool, ToolRegistry, tool

# 方式1: 继承 CustomTool
class CalculatorTool(CustomTool):
    @property
    def name(self) -> str:
        return "calculator"

    @property
    def description(self) -> str:
        return "Simple calculator for basic math"

    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Math expression to evaluate"
                }
            },
            "required": ["expression"]
        }

    async def execute(self, expression: str) -> str:
        try:
            import ast
            result = ast.literal_eval(expression)
            return str(result)
        except (ValueError, SyntaxError) as e:
            return f"Error: {e}"


# 方式2: 使用装饰器
@tool(name="greet", description="Generate a greeting")
async def greet_tool(name: str, greeting: str = "Hello") -> str:
    return f"{greeting}, {name}!"


async def main():
    # 创建注册表
    registry = ToolRegistry()

    # 注册工具
    registry.register(CalculatorTool())
    registry.register(greet_tool)

    # 执行工具
    result1 = await registry.execute("calculator", expression="2 + 2 * 3")
    print(f"Calculator: {result1}")

    result2 = await registry.execute("greet", name="World")
    print(f"Greet: {result2}")

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
```

### 4.4 workflow.py

```python
"""工作流示例

展示 DAG 工作流的创建和执行。
"""

from continuum_sdk.workflow import DAG, Node

async def fetch_data():
    """步骤1: 获取数据"""
    print("Fetching data...")
    return {"data": [1, 2, 3, 4, 5]}

async def process_data(prev_result):
    """步骤2: 处理数据"""
    print(f"Processing: {prev_result}")
    data = prev_result["data"]
    return {"processed": [x * 2 for x in data]}

async def save_result(prev_result):
    """步骤3: 保存结果"""
    print(f"Saving: {prev_result}")
    return {"saved": True, "count": len(prev_result["processed"])}

async def main():
    # 创建工作流
    dag = DAG("data_pipeline")

    # 添加节点并定义依赖
    dag.add(Node("fetch", func=fetch_data))
    dag.add(Node("process", func=process_data).depends_on("fetch"))
    dag.add(Node("save", func=save_result).depends_on("process"))

    # 验证
    errors = dag.validate()
    if errors:
        print(f"Validation errors: {errors}")
        return

    # 可视化
    print("Workflow:")
    print(dag.visualize())

    # 执行
    result = await dag.execute()

    # 获取结果
    print(f"Status: {result.status}")
    print(f"Final output: {result.get_output('save')}")

    # 检查执行顺序
    print(f"Execution order: {result.execution_order()}")

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
```

---

## 五、API 一致性检查

| 模块 | 导入路径 | 主要类/函数 |
|------|----------|-------------|
| Tool | `continuum_sdk.tools` | `BuiltinTools`, `CustomTool`, `ToolRegistry`, `@tool` |
| Memory | `continuum_sdk.memory` | `Memory`, `MemoryTier`, `MemoryEntry` |
| Workflow | `continuum_sdk.workflow` | `DAG`, `Node`, `DAGResult` |

---

**草稿状态**: 完成
**等待**: Terminal 1 SDK 基础 API
