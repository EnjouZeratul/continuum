# Egg-Harness 架构改进方案 v2.1（开源版）

> 版本: v2.1-opensource
> 日期: 2026-05-08
> 基于: architecture-v2.md
> 场景: 开源项目，用户自带 API Key 部署

---

## 一、改进概览

### 1.1 设计原则

开源项目的核心目标：**让开发者快速上手，而非企业级合规**

- 用户自己部署，自带 API Key
- 不需要 Key 加密、权限系统、审计日志
- 配置简单：YAML + 环境变量
- 重视可调试性（开发者的刚需）

### 1.2 改进清单

| 优先级 | 模块 | 类型 | 说明 |
|--------|------|------|------|
| P0 | Retry & Error Handler | 新增 | 网络问题必须重试 |
| P0 | Token Budget Manager | 新增 | Context 超限是硬伤 |
| P1 | Observability（简化版） | 新增 | 开发调试必需 |
| P1 | Tool 输入验证 + 超时 | 增强 | 防止工具卡死 |
| P2 | Agent Planner | 新增 | 让 Agent 更智能 |
| P2 | Workflow 增强 | 增强 | 并行执行、持久化 |

**移除的内容：**
- Security Manager（Key 加密、轮换、权限、审计）
- 复杂的分层配置系统
- Rate Limit 主动追踪（用户自己负责）

---

## 二、核心新增模块

### 2.1 Retry & Error Handler（P0）

个人用户也会遇到网络波动、API 限流，这是框架稳定性的基础。

#### 错误分类

```python
from enum import Enum
from dataclasses import dataclass
from typing import Optional

class ErrorType(Enum):
    """错误类型"""
    RATE_LIMIT = "rate_limit"
    TIMEOUT = "timeout"
    CONTEXT_TOO_LONG = "context_too_long"
    TOOL_FAILED = "tool_failed"
    INVALID_RESPONSE = "invalid_response"
    NETWORK_ERROR = "network_error"
    API_ERROR = "api_error"
    AUTH_ERROR = "auth_error"

@dataclass
class ClassifiedError:
    """分类后的错误"""
    error_type: ErrorType
    original_error: Exception
    message: str
    recoverable: bool
    suggested_action: str
    retry_after: Optional[int] = None

class ErrorClassifier:
    """错误分类器"""

    PATTERNS = {
        ErrorType.RATE_LIMIT: ["rate limit", "429", "too many requests", "quota"],
        ErrorType.TIMEOUT: ["timeout", "timed out"],
        ErrorType.CONTEXT_TOO_LONG: ["context length", "max tokens", "too long"],
        ErrorType.NETWORK_ERROR: ["connection", "network", "dns", "refused"],
        ErrorType.AUTH_ERROR: ["invalid api key", "unauthorized", "authentication"],
    }

    @classmethod
    def classify(cls, error: Exception) -> ClassifiedError:
        """分类错误"""
        error_str = str(error).lower()

        for error_type, patterns in cls.PATTERNS.items():
            if any(p in error_str for p in patterns):
                return cls._create_classified(error_type, error)

        return ClassifiedError(
            error_type=ErrorType.API_ERROR,
            original_error=error,
            message=str(error),
            recoverable=True,
            suggested_action="重试请求",
        )

    @classmethod
    def _create_classified(cls, error_type: ErrorType, error: Exception) -> ClassifiedError:
        configs = {
            ErrorType.RATE_LIMIT: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"速率限制: {error}",
                recoverable=True,
                suggested_action="等待后重试",
                retry_after=60,
            ),
            ErrorType.TIMEOUT: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"请求超时: {error}",
                recoverable=True,
                suggested_action="增加超时或重试",
                retry_after=5,
            ),
            ErrorType.CONTEXT_TOO_LONG: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"上下文超长: {error}",
                recoverable=True,
                suggested_action="压缩上下文",
            ),
            ErrorType.NETWORK_ERROR: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"网络错误: {error}",
                recoverable=True,
                suggested_action="检查网络后重试",
                retry_after=10,
            ),
            ErrorType.AUTH_ERROR: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"认证错误: {error}",
                recoverable=False,
                suggested_action="检查 API Key",
            ),
        }
        return configs.get(error_type, ClassifiedError(
            error_type=error_type,
            original_error=error,
            message=str(error),
            recoverable=True,
            suggested_action="重试",
        ))
```

#### 重试执行器

```python
import asyncio
import random
from enum import Enum
from typing import TypeVar, Callable, Awaitable

T = TypeVar("T")

class BackoffType(Enum):
    NONE = "none"
    LINEAR = "linear"
    EXPONENTIAL = "exponential"
    EXPONENTIAL_JITTER = "exponential_jitter"

@dataclass
class RetryConfig:
    """重试配置"""
    max_retries: int = 3
    backoff_type: BackoffType = BackoffType.EXPONENTIAL_JITTER
    base_delay: float = 1.0
    max_delay: float = 60.0
    retryable_errors: tuple = (
        ErrorType.RATE_LIMIT,
        ErrorType.TIMEOUT,
        ErrorType.NETWORK_ERROR,
        ErrorType.API_ERROR,
    )

class RetryExecutor:
    """重试执行器"""

    def __init__(self, config: RetryConfig = None):
        self.config = config or RetryConfig()

    async def execute(
        self,
        func: Callable[..., Awaitable[T]],
        *args,
        **kwargs,
    ) -> T:
        """执行带重试的异步函数"""
        last_error = None

        for attempt in range(self.config.max_retries + 1):
            try:
                return await func(*args, **kwargs)
            except Exception as e:
                classified = ErrorClassifier.classify(e)

                # 不可恢复的错误直接抛出
                if not classified.recoverable:
                    raise e

                # 不在可重试列表中
                if classified.error_type not in self.config.retryable_errors:
                    raise e

                # 达到最大重试次数
                if attempt == self.config.max_retries:
                    raise RetryExhaustedError(
                        f"达到最大重试次数 ({self.config.max_retries})",
                        last_error=e,
                    )

                last_error = e

                # 计算延迟并等待
                delay = self._calculate_delay(attempt, classified.retry_after)

                # 打印重试信息（开源项目友好）
                print(f"[Retry] {classified.error_type.value}, 等待 {delay:.1f}s 后重试 ({attempt + 1}/{self.config.max_retries})")

                await asyncio.sleep(delay)

    def _calculate_delay(self, attempt: int, override: Optional[int] = None) -> float:
        """计算延迟时间"""
        if override:
            return float(override)

        if self.config.backoff_type == BackoffType.NONE:
            return 0

        if self.config.backoff_type == BackoffType.LINEAR:
            delay = self.config.base_delay * (attempt + 1)

        elif self.config.backoff_type == BackoffType.EXPONENTIAL:
            delay = self.config.base_delay * (2 ** attempt)

        elif self.config.backoff_type == BackoffType.EXPONENTIAL_JITTER:
            delay = self.config.base_delay * (2 ** attempt)
            delay = delay * (0.5 + random.random())

        return min(delay, self.config.max_delay)

class RetryExhaustedError(Exception):
    """重试耗尽错误"""
    def __init__(self, message: str, last_error: Exception):
        super().__init__(message)
        self.last_error = last_error
```

#### LLM Provider 集成重试

```python
class LLMProvider(ABC):
    # 原有方法...

    async def chat_with_retry(
        self,
        messages: list[Message],
        model: str,
        retry_config: Optional[RetryConfig] = None,
        **kwargs,
    ) -> LLMResponse:
        """带重试的聊天"""
        retry_config = retry_config or RetryConfig()
        executor = RetryExecutor(retry_config)

        return await executor.execute(
            lambda: self.chat(messages=messages, model=model, **kwargs)
        )

# OpenAI 实现
class OpenAIProvider(LLMProvider):
    async def chat_with_retry(
        self,
        messages: list[Message],
        model: str,
        retry_config: Optional[RetryConfig] = None,
        **kwargs,
    ) -> LLMResponse:
        retry_config = retry_config or RetryConfig()
        executor = RetryExecutor(retry_config)

        return await executor.execute(
            lambda: self.chat(messages=messages, model=model, **kwargs)
        )
```

---

### 2.2 Token Budget Manager（P0）

Context 超限会导致 API 报错，必须在框架层面管理预算。

```python
from dataclasses import dataclass, field
from typing import Optional
from collections import defaultdict

@dataclass
class BudgetAllocation:
    """预算分配"""
    system_prompt: int = 0
    repo_map: int = 0
    conversation: int = 0
    tool_results: int = 0
    response_reserve: int = 4096

    @property
    def total(self) -> int:
        return (
            self.system_prompt +
            self.repo_map +
            self.conversation +
            self.tool_results +
            self.response_reserve
        )

@dataclass
class TokenBudgetConfig:
    """预算配置"""
    total_budget: int = 128000
    response_reserve: int = 4096
    safety_margin: float = 0.05  # 5% 安全余量

    # 分配比例
    system_prompt_ratio: float = 0.05
    repo_map_ratio: float = 0.15
    conversation_ratio: float = 0.50
    tool_results_ratio: float = 0.25

class TokenBudgetManager:
    """Token 预算管理器"""

    def __init__(self, config: TokenBudgetConfig = None):
        self.config = config or TokenBudgetConfig()
        self._usage: dict[str, dict] = defaultdict(lambda: {"allocated": 0, "actual": 0})

    def get_allocation(self) -> BudgetAllocation:
        """获取预算分配"""
        available = self.config.total_budget - self.config.response_reserve
        available = int(available * (1 - self.config.safety_margin))

        return BudgetAllocation(
            system_prompt=int(available * self.config.system_prompt_ratio),
            repo_map=int(available * self.config.repo_map_ratio),
            conversation=int(available * self.config.conversation_ratio),
            tool_results=int(available * self.config.tool_results_ratio),
            response_reserve=self.config.response_reserve,
        )

    def allocate(self, component: str, requested: int) -> int:
        """为组件分配预算，返回实际可用量"""
        allocation = self.get_allocation()
        limits = {
            "system_prompt": allocation.system_prompt,
            "repo_map": allocation.repo_map,
            "conversation": allocation.conversation,
            "tool_results": allocation.tool_results,
        }

        limit = limits.get(component, 0)
        actual = min(requested, limit)

        self._usage[component]["allocated"] = actual
        return actual

    def report_usage(self, component: str, actual: int) -> None:
        """报告实际使用量"""
        self._usage[component]["actual"] = actual

    def get_available(self) -> int:
        """获取剩余可用预算"""
        total_used = sum(u["actual"] for u in self._usage.values())
        return self.config.total_budget - self.config.response_reserve - total_used

    def check_within_budget(self, tokens: int, component: str) -> bool:
        """检查是否在预算内"""
        allocation = self.get_allocation()
        limits = {
            "system_prompt": allocation.system_prompt,
            "repo_map": allocation.repo_map,
            "conversation": allocation.conversation,
            "tool_results": allocation.tool_results,
        }
        return tokens <= limits.get(component, 0)

    def get_report(self) -> dict:
        """获取使用报告"""
        return {
            "total_budget": self.config.total_budget,
            "used": sum(u["actual"] for u in self._usage.values()),
            "available": self.get_available(),
            "breakdown": dict(self._usage),
        }

    def reset(self) -> None:
        """重置预算"""
        self._usage.clear()
```

#### Context Manager 集成预算管理

```python
class ContextManager:
    def __init__(
        self,
        max_tokens: int = 128000,
        budget_manager: Optional[TokenBudgetManager] = None,
    ):
        self.max_tokens = max_tokens
        self.budget_manager = budget_manager or TokenBudgetManager(
            TokenBudgetConfig(total_budget=max_tokens)
        )
        self._messages: list[Message] = []
        self._system_prompt: Optional[str] = None
        self._important_messages: set[int] = set()

    async def add_message(self, message: Message, important: bool = False) -> None:
        """添加消息"""
        # 预算检查
        message_tokens = self._estimate_tokens(message)

        if not self.budget_manager.check_within_budget(message_tokens, "conversation"):
            await self.compact()

        self._messages.append(message)

        if important:
            self._important_messages.add(id(message))

    def mark_important(self, message: Message) -> None:
        """标记重要消息（压缩时优先保留）"""
        self._important_messages.add(id(message))

    async def compact(self, force: bool = False) -> None:
        """压缩上下文"""
        # 分离重要和非重要消息
        important = [m for m in self._messages if id(m) in self._important_messages]
        others = [m for m in self._messages if id(m) not in self._important_messages]

        if others:
            # 生成摘要（简化版：保留最近 + 摘要历史）
            recent_count = 5
            to_compress = others[:-recent_count] if len(others) > recent_count else []

            if to_compress:
                summary = await self._generate_summary(to_compress)
                self._messages = [
                    Message(role="system", content=f"[历史摘要]\n{summary}"),
                    *others[-recent_count:],
                    *important,
                ]

    async def _generate_summary(self, messages: list[Message]) -> str:
        """生成摘要（需要 LLM 支持）"""
        # 简化实现：直接拼接关键信息
        tool_calls = []
        for m in messages:
            if m.tool_calls:
                for tc in m.tool_calls:
                    tool_calls.append(f"- {tc.function.name}")

        return f"共 {len(messages)} 条消息，工具调用：\n" + "\n".join(tool_calls[:10])

    def _estimate_tokens(self, message: Message) -> int:
        """估算消息 token 数"""
        # 简化估算：字符数 / 4
        return len(message.content) // 4 + 10
```

---

### 2.3 Observability 简化版（P1）

开源项目需要让用户快速定位问题，但不需要分布式追踪。

```python
from dataclasses import dataclass, field
from typing import Optional
import time
import json

@dataclass
class TraceSpan:
    """追踪跨度"""
    name: str
    start_time: float
    end_time: Optional[float] = None
    status: str = "running"
    attributes: dict = field(default_factory=dict)

    @property
    def duration_ms(self) -> Optional[float]:
        if self.end_time:
            return (self.end_time - self.start_time) * 1000
        return None

class SimpleTracer:
    """简单追踪器 - 控制台输出"""

    def __init__(self, verbose: bool = True):
        self.verbose = verbose
        self._spans: list[TraceSpan] = []
        self._indent = 0

    def start(self, name: str, **attrs) -> TraceSpan:
        """开始追踪"""
        span = TraceSpan(
            name=name,
            start_time=time.time(),
            attributes=attrs,
        )
        self._spans.append(span)

        if self.verbose:
            print(f"{'  ' * self._indent}→ {name}", attrs if attrs else "")
            self._indent += 1

        return span

    def end(self, span: TraceSpan, status: str = "ok") -> None:
        """结束追踪"""
        span.end_time = time.time()
        span.status = status

        if self.verbose:
            self._indent -= 1
            duration = span.duration_ms
            status_icon = "✓" if status == "ok" else "✗"
            print(f"{'  ' * self._indent}{status_icon} {span.name} ({duration:.1f}ms)")

    def get_summary(self) -> dict:
        """获取追踪摘要"""
        return {
            "total_spans": len(self._spans),
            "total_time_ms": sum(s.duration_ms or 0 for s in self._spans),
            "errors": [s for s in self._spans if s.status != "ok"],
            "spans": [
                {
                    "name": s.name,
                    "duration_ms": s.duration_ms,
                    "status": s.status,
                }
                for s in self._spans
            ],
        }

# 使用示例
tracer = SimpleTracer()

span = tracer.start("llm.chat", model="gpt-4", provider="openai")
response = await llm.chat(messages, model="gpt-4")
tracer.end(span, "ok" if response else "error")
```

#### 简单指标收集

```python
from collections import defaultdict

class SimpleMetrics:
    """简单指标收集"""

    def __init__(self):
        self._counters = defaultdict(int)
        self._timings = defaultdict(list)

    def count(self, name: str, value: int = 1) -> None:
        """计数"""
        self._counters[name] += value

    def timing(self, name: str, value_ms: float) -> None:
        """记录耗时"""
        self._timings[name].append(value_ms)

    def get_summary(self) -> dict:
        """获取摘要"""
        summary = {"counters": dict(self._counters), "timings": {}}

        for name, values in self._timings.items():
            if values:
                summary["timings"][name] = {
                    "count": len(values),
                    "avg_ms": sum(values) / len(values),
                    "min_ms": min(values),
                    "max_ms": max(values),
                }

        return summary

    def print_summary(self) -> None:
        """打印摘要"""
        summary = self.get_summary()
        print("\n=== 指标摘要 ===")
        for name, value in summary["counters"].items():
            print(f"{name}: {value}")
        for name, stats in summary["timings"].items():
            print(f"{name}: avg={stats['avg_ms']:.1f}ms, min={stats['min_ms']:.1f}ms, max={stats['max_ms']:.1f}ms")

# Agent 内置指标
class AgentMetrics:
    def __init__(self, metrics: SimpleMetrics):
        self.metrics = metrics

    def record_llm_call(self, provider: str, tokens: int, latency_ms: float):
        self.metrics.count(f"llm.calls.{provider}")
        self.metrics.count(f"llm.tokens.{provider}", tokens)
        self.metrics.timing(f"llm.latency.{provider}", latency_ms)

    def record_tool_call(self, tool: str, success: bool, latency_ms: float):
        self.metrics.count(f"tool.calls.{tool}")
        if not success:
            self.metrics.count(f"tool.errors.{tool}")
        self.metrics.timing(f"tool.latency.{tool}", latency_ms)
```

---

### 2.4 Tool System 增强（P1）

#### 输入验证

```python
import json
from dataclasses import dataclass, field

@dataclass
class ValidationResult:
    valid: bool
    errors: list[str] = field(default_factory=list)

class ToolRegistry:
    # 原有代码...

    async def validate_input(self, name: str, args: dict) -> ValidationResult:
        """验证工具输入"""
        tool = self._tools.get(name)
        if not tool:
            return ValidationResult(valid=False, errors=["Tool not found"])

        # 检查必需参数
        required = tool.parameters.get("required", [])
        for param in required:
            if param not in args:
                return ValidationResult(
                    valid=False,
                    errors=[f"Missing required parameter: {param}"]
                )

        # 检查参数类型（简化版）
        properties = tool.parameters.get("properties", {})
        for key, value in args.items():
            if key in properties:
                expected_type = properties[key].get("type")
                if expected_type and not self._check_type(value, expected_type):
                    return ValidationResult(
                        valid=False,
                        errors=[f"Parameter '{key}' should be {expected_type}"]
                    )

        return ValidationResult(valid=True)

    def _check_type(self, value, expected_type: str) -> bool:
        """检查类型"""
        type_map = {
            "string": str,
            "integer": int,
            "number": (int, float),
            "boolean": bool,
            "array": list,
            "object": dict,
        }
        expected = type_map.get(expected_type)
        if expected:
            return isinstance(value, expected)
        return True
```

#### 超时控制

```python
import asyncio

class ToolTimeoutError(Exception):
    """工具超时错误"""
    pass

class ToolRegistry:
    # 原有代码...

    async def invoke_with_timeout(
        self,
        name: str,
        timeout: int,
        **kwargs,
    ) -> Any:
        """带超时的工具调用"""
        try:
            async with asyncio.timeout(timeout):
                return await self.invoke(name, **kwargs)
        except asyncio.TimeoutError:
            raise ToolTimeoutError(f"Tool '{name}' timed out after {timeout}s")

# Agent Runtime 集成
class AgentRuntime:
    def __init__(self, default_tool_timeout: int = 30):
        self.default_tool_timeout = default_tool_timeout

    async def _execute_tool(self, tool_call: ToolCall) -> str:
        """执行工具"""
        tool_name = tool_call.function.name

        # 获取工具定义的超时时间，或使用默认值
        tool = self.tools.get(tool_name)
        timeout = getattr(tool, "timeout", self.default_tool_timeout)

        try:
            result = await self.tools.invoke_with_timeout(
                name=tool_name,
                timeout=timeout,
                **json.loads(tool_call.function.arguments)
            )
            return str(result)
        except ToolTimeoutError as e:
            return f"错误：{e}"
```

#### 增强 Schema 生成

```python
from typing import get_origin, get_args, Annotated, Union, Optional
import inspect

class ToolRegistry:
    # 原有代码...

    def _generate_schema(self, func: Callable) -> dict:
        """增强的 Schema 生成"""
        sig = inspect.signature(func)
        hints = get_type_hints(func, include_extras=True)

        properties = {}
        required = []

        for name, param in sig.parameters.items():
            if name == "self":
                continue

            hint = hints.get(name, str)
            prop = self._type_to_schema(hint)

            # 处理 Annotated 描述
            if get_origin(hint) is Annotated:
                args = get_args(hint)
                if len(args) >= 2 and isinstance(args[-1], str):
                    prop["description"] = args[-1]

            # 从 docstring 获取描述
            if "description" not in prop:
                prop["description"] = self._get_param_desc(func, name)

            # 默认值
            if param.default != inspect.Parameter.empty:
                prop["default"] = param.default
            else:
                required.append(name)

            properties[name] = prop

        return {
            "type": "object",
            "properties": properties,
            "required": required,
        }

    def _type_to_schema(self, type_hint) -> dict:
        """类型转 JSON Schema"""
        origin = get_origin(type_hint)

        # Optional[T]
        if origin is Union:
            args = get_args(type_hint)
            non_none = [a for a in args if a is not type(None)]
            if len(non_none) == 1:
                schema = self._type_to_schema(non_none[0])
                return {**schema, "nullable": True}

        # List[T]
        if origin is list:
            args = get_args(type_hint)
            item_type = args[0] if args else str
            return {
                "type": "array",
                "items": self._type_to_schema(item_type),
            }

        # Dict
        if origin is dict:
            return {"type": "object"}

        # 基础类型
        type_map = {
            str: {"type": "string"},
            int: {"type": "integer"},
            float: {"type": "number"},
            bool: {"type": "boolean"},
            list: {"type": "array"},
            dict: {"type": "object"},
        }

        return type_map.get(type_hint, {"type": "string"})

# 使用示例
@registry.tool()
async def search_files(
    pattern: Annotated[str, "搜索模式，支持 glob"],
    max_results: Annotated[int, "最大返回数量"] = 10,
    file_types: Annotated[Optional[list[str]], "文件类型过滤"] = None,
) -> list[str]:
    """搜索文件"""
    pass
```

---

### 2.5 Agent Planner（P2）

让 Agent 能智能分解任务。

```python
from dataclasses import dataclass, field
from enum import Enum
import time
import uuid

class ComplexityLevel(Enum):
    SIMPLE = "simple"           # 单次调用
    MODERATE = "moderate"       # 需要工具
    COMPLEX = "complex"         # 需要规划
    VERY_COMPLEX = "very_complex"  # 多 Agent

class ExecutionStrategy(Enum):
    SINGLE_TURN = "single_turn"
    MULTI_TURN = "multi_turn"
    TOOL_CHAIN = "tool_chain"
    WORKFLOW = "workflow"

@dataclass
class SubTask:
    """子任务"""
    id: str
    description: str
    dependencies: list[str] = field(default_factory=list)
    required_tools: list[str] = field(default_factory=list)
    status: str = "pending"

@dataclass
class Plan:
    """执行计划"""
    id: str
    task: str
    complexity: ComplexityLevel
    strategy: ExecutionStrategy
    subtasks: list[SubTask]
    estimated_iterations: int

class Planner:
    """任务规划器"""

    def __init__(self, llm: "LLMProvider", model: str):
        self.llm = llm
        self.model = model

    async def analyze(self, task: str) -> tuple[ComplexityLevel, ExecutionStrategy]:
        """分析任务复杂度"""
        prompt = f"""分析任务复杂度：

任务：{task}

返回 JSON：
{{"complexity": "simple|moderate|complex|very_complex", "strategy": "single_turn|multi_turn|tool_chain|workflow"}}
"""
        response = await self.llm.chat(
            messages=[{"role": "user", "content": prompt}],
            model=self.model,
        )

        try:
            import json
            result = json.loads(response.content)
            return (
                ComplexityLevel(result.get("complexity", "moderate")),
                ExecutionStrategy(result.get("strategy", "multi_turn")),
            )
        except:
            return ComplexityLevel.MODERATE, ExecutionStrategy.MULTI_TURN

    async def decompose(self, task: str) -> list[SubTask]:
        """分解任务"""
        prompt = f"""分解任务为子任务：

任务：{task}

返回 JSON：
{{
  "subtasks": [
    {{
      "id": "subtask_1",
      "description": "描述",
      "dependencies": [],
      "required_tools": ["tool1"]
    }}
  ]
}}
"""
        response = await self.llm.chat(
            messages=[{"role": "user", "content": prompt}],
            model=self.model,
        )

        try:
            import json
            result = json.loads(response.content)
            return [SubTask(**st) for st in result.get("subtasks", [])]
        except:
            return [SubTask(id="main", description=task)]

    async def create_plan(self, task: str) -> Plan:
        """创建执行计划"""
        complexity, strategy = await self.analyze(task)
        subtasks = await self.decompose(task)

        return Plan(
            id=str(uuid.uuid4())[:8],
            task=task,
            complexity=complexity,
            strategy=strategy,
            subtasks=subtasks,
            estimated_iterations=len(subtasks) * 3,
        )

    def topological_sort(self, subtasks: list[SubTask]) -> list[SubTask]:
        """拓扑排序"""
        in_degree = {st.id: len(st.dependencies) for st in subtasks}
        graph = {st.id: st for st in subtasks}

        result = []
        queue = [st for st in subtasks if in_degree[st.id] == 0]

        while queue:
            current = queue.pop(0)
            result.append(current)

            for st in subtasks:
                if current.id in st.dependencies:
                    in_degree[st.id] -= 1
                    if in_degree[st.id] == 0:
                        queue.append(st)

        return result
```

---

### 2.6 Workflow Engine 增强（P2）

#### 并行执行

```python
import asyncio

class WorkflowEdge:
    def __init__(
        self,
        source: str,
        target: str,
        condition: Optional[Callable] = None,
        parallel: bool = False,
    ):
        self.source = source
        self.target = target
        self.condition = condition
        self.parallel = parallel

class StateGraph:
    # 原有代码...

    def add_parallel(self, source: str, targets: list[str]) -> "StateGraph":
        """添加并行分支"""
        for target in targets:
            self.edges.append(WorkflowEdge(
                source=source,
                target=target,
                parallel=True,
            ))
        return self

class CompiledGraph:
    # 原有代码...

    async def invoke(self, initial_state: dict) -> dict:
        """执行工作流"""
        state = initial_state.copy()
        current = self.graph.entry_point

        while current and current != "__end__":
            node = self.graph.nodes.get(current)
            if not node:
                break

            # 执行当前节点
            state = await node.execute(state)

            # 查找后续节点
            edges = [e for e in self.graph.edges if e.source == current]

            # 分离并行和顺序节点
            parallel_targets = [e.target for e in edges if e.parallel]
            sequential_edges = [e for e in edges if not e.parallel]

            if parallel_targets:
                # 并行执行
                results = await asyncio.gather(
                    *[self._execute_node(t, state.copy()) for t in parallel_targets]
                )

                # 合并结果（简单策略：后执行的覆盖）
                for result in results:
                    state.update(result)

                current = None  # 并行后结束
            else:
                # 顺序执行
                next_node = None
                for edge in sequential_edges:
                    if edge.condition is None or edge.condition(state):
                        next_node = edge.target
                        break
                current = next_node

        return state

    async def _execute_node(self, node_name: str, state: dict) -> dict:
        """执行单个节点"""
        node = self.graph.nodes.get(node_name)
        if node:
            return await node.execute(state)
        return state
```

#### 持久化（检查点）

```python
import json
import os

class CheckpointManager:
    """检查点管理"""

    def __init__(self, checkpoint_dir: str = ".egg/checkpoints"):
        self.checkpoint_dir = checkpoint_dir
        os.makedirs(checkpoint_dir, exist_ok=True)

    def save(self, workflow_id: str, state: dict) -> str:
        """保存检查点"""
        checkpoint_id = f"{workflow_id}_{int(time.time())}"
        filepath = os.path.join(self.checkpoint_dir, f"{checkpoint_id}.json")

        with open(filepath, "w") as f:
            json.dump({
                "id": checkpoint_id,
                "workflow_id": workflow_id,
                "timestamp": time.time(),
                "state": state,
            }, f, indent=2)

        return checkpoint_id

    def load(self, checkpoint_id: str) -> Optional[dict]:
        """加载检查点"""
        filepath = os.path.join(self.checkpoint_dir, f"{checkpoint_id}.json")

        if os.path.exists(filepath):
            with open(filepath, "r") as f:
                data = json.load(f)
                return data.get("state")

        return None

    def list_checkpoints(self, workflow_id: Optional[str] = None) -> list[str]:
        """列出检查点"""
        checkpoints = []
        for filename in os.listdir(self.checkpoint_dir):
            if filename.endswith(".json"):
                if workflow_id is None or filename.startswith(workflow_id):
                    checkpoints.append(filename[:-5])
        return sorted(checkpoints)
```

---

## 三、配置系统（简化版）

开源项目配置要简单：YAML 文件 + 环境变量。

```python
import os
import yaml
from pathlib import Path
from typing import Any, Optional

class Config:
    """简单配置管理"""

    def __init__(self, config_file: Optional[str] = None):
        self._config: dict = {}

        # 加载配置文件
        if config_file:
            self._load_file(config_file)
        elif os.path.exists("egg.yaml"):
            self._load_file("egg.yaml")

    def _load_file(self, filepath: str) -> None:
        """加载 YAML 文件"""
        with open(filepath, "r", encoding="utf-8") as f:
            self._config = yaml.safe_load(f) or {}

    def get(self, key: str, default: Any = None) -> Any:
        """
        获取配置（优先级：环境变量 > 配置文件）

        环境变量格式：EGG_MODEL_NAME -> model.name
        """
        # 先检查环境变量
        env_key = "EGG_" + key.upper().replace(".", "_")
        env_value = os.environ.get(env_key)
        if env_value is not None:
            # 尝试转换类型
            if env_value.lower() in ("true", "false"):
                return env_value.lower() == "true"
            try:
                return int(env_value)
            except ValueError:
                try:
                    return float(env_value)
                except ValueError:
                    return env_value

        # 再检查配置文件
        keys = key.split(".")
        value = self._config
        for k in keys:
            if isinstance(value, dict):
                value = value.get(k)
            else:
                return default

        return value if value is not None else default

    def get_api_key(self, provider: str) -> Optional[str]:
        """获取 API Key（从环境变量）"""
        return os.environ.get(f"{provider.upper()}_API_KEY")

# 默认配置
DEFAULTS = {
    "model.provider": "openai",
    "model.name": "gpt-4-turbo",
    "model.temperature": 0.7,
    "model.max_tokens": 4096,
    "context.max_tokens": 128000,
    "context.auto_compact": True,
    "agent.max_iterations": 30,
    "agent.max_corrections": 3,
    "retry.max_retries": 3,
    "retry.backoff": "exponential_jitter",
    "tool.default_timeout": 30,
}

class EggConfig(Config):
    """带默认值的配置"""

    def get(self, key: str, default: Any = None) -> Any:
        # 环境变量 > 配置文件 > 默认值
        value = super().get(key)
        if value is None:
            return DEFAULTS.get(key, default)
        return value
```

**配置文件示例 (egg.yaml):**

```yaml
model:
  provider: openai
  name: gpt-4-turbo
  temperature: 0.7
  max_tokens: 4096

context:
  max_tokens: 128000
  auto_compact: true

agent:
  max_iterations: 30
  max_corrections: 3

retry:
  max_retries: 3
  backoff: exponential_jitter

tool:
  default_timeout: 30

observability:
  verbose: true
  trace_output: .egg/traces
```

**使用示例:**

```python
# 代码使用
config = EggConfig("egg.yaml")

model = config.get("model.name")  # gpt-4-turbo
api_key = config.get_api_key("openai")  # 从 OPENAI_API_KEY 环境变量

# 环境变量覆盖
# EGG_MODEL_NAME=gpt-3.5-turbo python app.py
```

---

## 四、项目结构（简化版）

```
egg-harness/
├── src/egg_harness/
│   ├── __init__.py
│   ├── core/
│   │   ├── __init__.py
│   │   ├── harness.py           # Harness 主类
│   │   ├── agent.py             # Agent 类
│   │   └── config.py            # 简化配置
│   ├── llm/
│   │   ├── __init__.py
│   │   ├── base.py              # LLMProvider 基类
│   │   ├── retry.py             # 重试逻辑
│   │   └── providers/
│   │       ├── openai.py
│   │       ├── anthropic.py
│   │       ├── dashscope.py
│   │       └── ...
│   ├── context/
│   │   ├── __init__.py
│   │   ├── manager.py           # ContextManager
│   │   ├── budget.py            # Token 预算
│   │   └── compact.py           # 自动压缩
│   ├── tools/
│   │   ├── __init__.py
│   │   ├── registry.py          # 增强的 ToolRegistry
│   │   ├── validation.py        # 输入验证
│   │   └── builtin/
│   │       ├── file_ops.py
│   │       ├── shell.py
│   │       └── ...
│   ├── workflow/
│   │   ├── __init__.py
│   │   ├── graph.py             # StateGraph + 并行
│   │   └── checkpoint.py        # 检查点
│   ├── runtime/
│   │   ├── __init__.py
│   │   ├── loop.py              # Agent 执行循环
│   │   └── planner.py           # 任务规划
│   ├── observability/
│   │   ├── __init__.py
│   │   ├── tracer.py            # 简单追踪
│   │   └── metrics.py           # 简单指标
│   └── error/
│       ├── __init__.py
│       ├── classifier.py        # 错误分类
│       └── retry.py             # 重试执行
├── tests/
├── examples/
├── docs/
├── pyproject.toml
├── README.md
└── egg.yaml                     # 默认配置示例
```

---

## 五、实施计划

### Phase 1: 核心稳定性（1.5 周）

- [ ] Error Classifier + Retry Executor
- [ ] Token Budget Manager
- [ ] 简单配置系统
- [ ] Context Manager 集成预算

### Phase 2: 可调试性（1 周）

- [ ] SimpleTracer（控制台输出）
- [ ] SimpleMetrics
- [ ] Tool 输入验证 + 超时

### Phase 3: 高级特性（1 周）

- [ ] Agent Planner
- [ ] Workflow 并行执行
- [ ] Workflow 检查点

---

## 六、对比总结

| 对比项 | 企业版 | 开源版 |
|--------|--------|--------|
| **Key 管理** | 加密存储、轮换 | 直接读环境变量 |
| **权限系统** | Role-based | 无（单用户） |
| **审计日志** | 完整审计 | 无 |
| **配置系统** | 四层配置 | YAML + 环境变量 |
| **追踪系统** | 分布式追踪 | 控制台输出 |
| **代码量** | 多 30% | 精简 |
| **上手难度** | 需要配置 | 开箱即用 |

---

**核心观点：开源项目应该"简单到让用户 5 分钟跑起来"，而不是"企业级到让用户配置 30 分钟"。**

**文档状态**: 已完成
**下一步**: 确认方案后开始 Phase 1 开发
