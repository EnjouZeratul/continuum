read D:\TA\create_together_with_ali\continuum\docs\TEAM_PROTOCOL.md Frist
# Continuum 技术规范

> 版本: v1.1
> 日期: 2026-05-08
> 状态: 已确认
> 定位: 开源 AI Agent Harness 框架

---

## 一、项目概述

### 1.1 定位

Continuum 是一个**轻量级、可扩展、可观测**的 AI Agent Harness 框架，用于构建具有工具调用、上下文管理和工作流编排能力的智能代理。

### 1.2 设计原则

| 原则 | 说明 |
|------|------|
| **精简依赖** | httpx + pydantic，无 LangChain 依赖 |
| **用户自带 Key** | 从环境变量读取 API Key，无需复杂密钥管理 |
| **配置简单** | YAML + 环境变量，5 分钟上手 |
| **可调试** | 控制台追踪、指标摘要、轨迹记录 |
| **渐进式复杂度** | 简单场景开箱即用，复杂场景按需扩展 |

### 1.3 核心能力

- **多模型支持**: OpenAI、Anthropic、DashScope、ZhipuAI、DeepSeek 等
- **工具系统**: 函数注册、Schema 自动生成、超时控制、输入验证
- **上下文管理**: 消息历史、自动压缩、Token 预算管理
- **记忆系统**: 项目记忆 (EGG.md)、自动记忆、跨会话持久化
- **钩子系统**: 工具执行前后回调、错误处理钩子、自定义扩展点
- **自我修正**: 执行失败自动分析、智能重试、最多 3 次修正
- **流式输出**: 实时响应流、工具调用流式处理、用户可中断
- **工作流编排**: 状态图、条件分支、并行执行、检查点
- **可观测性**: 追踪、指标、轨迹记录
- **MCP 支持**: 标准化工具扩展、外部服务集成

---

## 二、架构决策

### 2.1 为什么移除 LangChain？

| 问题 | 说明 |
|------|------|
| 抽象过重 | Runnable/Chain 抽象增加理解和调试难度 |
| 版本频繁变更 | API 不稳定，升级成本高 |
| 性能开销 | 多层抽象带来额外性能损耗 |
| 成熟框架不用 | Aider、Claude Code、OpenCode 都直接调用 API |
| 灵活性受限 | 深度定制需要绕过框架限制 |

### 2.2 借鉴来源

| 框架 | 借鉴点 |
|------|--------|
| **SWE-Agent** | ACI 工具设计、YAML 配置驱动 |
| **Aider** | Repository Map + Graph Ranking |
| **Claude Code** | CLAUDE.md + Auto Memory + Hooks |
| **OpenCode** | Auto Compact、LSP 集成 |
| **Cline** | Human-in-the-Loop、MCP 自扩展 |
| **Continue** | config.yaml 声明式配置 |
| **AutoCodeRover** | AST 感知搜索 |
| **MetaGPT/ChatDev** | 多 Agent 角色协作 |

---

## 三、整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        Continuum                                 │
├─────────────────────────────────────────────────────────────────┤
│  Interface: CLI | Python SDK                                     │
├─────────────────────────────────────────────────────────────────┤
│  Core Engine:                                                    │
│    Agent Runtime | Context Manager | Tool System | Workflow      │
│    Planner | Error Handler | Token Budget                        │
│    Memory System | Hooks System | Self-Correction                │
├─────────────────────────────────────────────────────────────────┤
│  Observability: SimpleTracer | SimpleMetrics                     │
├─────────────────────────────────────────────────────────────────┤
│  Integration: MCP Client | LSP | Git                             │
├─────────────────────────────────────────────────────────────────┤
│  Storage: Memory | SQLite | File System                          │
├─────────────────────────────────────────────────────────────────┤
│  LLM Providers: OpenAI | Anthropic | DashScope | ZhipuAI | ...  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 四、核心模块设计

### 4.1 消息类型定义

```python
from pydantic import BaseModel
from typing import Optional

class FunctionCall(BaseModel):
    """函数调用"""
    name: str
    arguments: str  # JSON 字符串

class ToolCall(BaseModel):
    """工具调用"""
    id: str
    type: str = "function"
    function: FunctionCall

class Message(BaseModel):
    """统一消息格式"""
    role: str  # system | user | assistant | tool
    content: str
    tool_calls: Optional[list[ToolCall]] = None
    tool_call_id: Optional[str] = None
    name: Optional[str] = None

class LLMResponse(BaseModel):
    """LLM 响应"""
    content: Optional[str] = None
    tool_calls: Optional[list[ToolCall]] = None
    finish_reason: str  # stop | tool_calls | length
    usage: Optional[dict] = None
```

---

### 4.2 LLM Provider（直接 API 调用）

#### 4.2.1 基类

```python
from abc import ABC, abstractmethod
from typing import AsyncIterator, Optional

class LLMProvider(ABC):
    """LLM 提供商基类"""
    
    @abstractmethod
    async def chat(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """发送聊天请求"""
        pass
    
    async def chat_with_retry(
        self,
        messages: list[Message],
        model: str,
        retry_config: Optional["RetryConfig"] = None,
        **kwargs,
    ) -> LLMResponse:
        """带重试的聊天"""
        executor = RetryExecutor(retry_config or RetryConfig())
        return await executor.execute(
            lambda: self.chat(messages=messages, model=model, **kwargs)
        )
```

#### 4.2.2 OpenAI 适配器

```python
import httpx
import json

class OpenAIProvider(LLMProvider):
    """OpenAI 适配器 - httpx 直接调用"""
    
    BASE_URL = "https://api.openai.com/v1"
    
    def __init__(self, api_key: str, base_url: Optional[str] = None):
        self.client = httpx.AsyncClient(
            base_url=base_url or self.BASE_URL,
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=120.0,
        )
    
    async def chat(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        payload = {
            "model": model,
            "messages": [self._format_msg(m) for m in messages],
            "temperature": temperature,
        }
        if max_tokens:
            payload["max_tokens"] = max_tokens
        if tools:
            payload["tools"] = tools
            payload["tool_choice"] = "auto"
        
        resp = await self.client.post("/chat/completions", json=payload)
        resp.raise_for_status()
        return self._parse_response(resp.json())
    
    def _format_msg(self, message: Message) -> dict:
        msg = {"role": message.role, "content": message.content}
        if message.tool_calls:
            msg["tool_calls"] = [
                {
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function.name,
                        "arguments": tc.function.arguments,
                    }
                }
                for tc in message.tool_calls
            ]
        if message.tool_call_id:
            msg["tool_call_id"] = message.tool_call_id
        return msg
    
    def _parse_response(self, data: dict) -> LLMResponse:
        choice = data["choices"][0]
        message = choice["message"]
        
        tool_calls = None
        if message.get("tool_calls"):
            tool_calls = [
                ToolCall(
                    id=tc["id"],
                    function=FunctionCall(
                        name=tc["function"]["name"],
                        arguments=tc["function"]["arguments"],
                    )
                )
                for tc in message["tool_calls"]
            ]
        
        return LLMResponse(
            content=message.get("content"),
            tool_calls=tool_calls,
            finish_reason=choice["finish_reason"],
            usage=data.get("usage"),
        )
```

#### 4.2.3 Anthropic 适配器

```python
class AnthropicProvider(LLMProvider):
    """Anthropic 适配器 - 直接调用 Claude API"""
    
    BASE_URL = "https://api.anthropic.com/v1"
    
    def __init__(self, api_key: str):
        self.client = httpx.AsyncClient(
            base_url=self.BASE_URL,
            headers={
                "x-api-key": api_key,
                "anthropic-version": "2023-06-01",
            },
            timeout=120.0,
        )
    
    async def chat(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        # Claude 的 system 需要单独传递
        system_msg = None
        chat_messages = []
        for msg in messages:
            if msg.role == "system":
                system_msg = msg.content
            else:
                chat_messages.append(self._format_msg(msg))
        
        payload = {
            "model": model,
            "messages": chat_messages,
            "max_tokens": max_tokens or 4096,
            "temperature": temperature,
        }
        if system_msg:
            payload["system"] = system_msg
        if tools:
            payload["tools"] = [self._format_tool(t) for t in tools]
        
        resp = await self.client.post("/messages", json=payload)
        resp.raise_for_status()
        return self._parse_response(resp.json())
    
    def _format_tool(self, tool: dict) -> dict:
        return {
            "name": tool["function"]["name"],
            "description": tool["function"]["description"],
            "input_schema": tool["function"]["parameters"],
        }
```

---

### 4.3 Error Handler & Retry

#### 4.3.1 错误分类

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
                error_type=error_type, original_error=error,
                message=f"速率限制: {error}", recoverable=True,
                suggested_action="等待后重试", retry_after=60,
            ),
            ErrorType.TIMEOUT: ClassifiedError(
                error_type=error_type, original_error=error,
                message=f"请求超时: {error}", recoverable=True,
                suggested_action="增加超时或重试", retry_after=5,
            ),
            ErrorType.CONTEXT_TOO_LONG: ClassifiedError(
                error_type=error_type, original_error=error,
                message=f"上下文超长: {error}", recoverable=True,
                suggested_action="压缩上下文",
            ),
            ErrorType.NETWORK_ERROR: ClassifiedError(
                error_type=error_type, original_error=error,
                message=f"网络错误: {error}", recoverable=True,
                suggested_action="检查网络后重试", retry_after=10,
            ),
            ErrorType.AUTH_ERROR: ClassifiedError(
                error_type=error_type, original_error=error,
                message=f"认证错误: {error}", recoverable=False,
                suggested_action="检查 API Key",
            ),
        }
        return configs.get(error_type, ClassifiedError(
            error_type=error_type, original_error=error,
            message=str(error), recoverable=True, suggested_action="重试",
        ))
```

#### 4.3.2 重试执行器

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
    
    async def execute(self, func: Callable[..., Awaitable[T]], *args, **kwargs) -> T:
        last_error = None
        for attempt in range(self.config.max_retries + 1):
            try:
                return await func(*args, **kwargs)
            except Exception as e:
                classified = ErrorClassifier.classify(e)
                if not classified.recoverable:
                    raise
                if classified.error_type not in self.config.retryable_errors:
                    raise
                if attempt == self.config.max_retries:
                    raise RetryExhaustedError("达到最大重试次数", last_error=e)
                
                last_error = e
                delay = self._calc_delay(attempt, classified.retry_after)
                print(f"[Retry] {classified.error_type.value}, 等待 {delay:.1f}s ({attempt+1}/{self.config.max_retries})")
                await asyncio.sleep(delay)
    
    def _calc_delay(self, attempt, override=None):
        if override:
            return float(override)
        if self.config.backoff_type == BackoffType.EXPONENTIAL_JITTER:
            delay = self.config.base_delay * (2 ** attempt) * (0.5 + random.random())
            return min(delay, self.config.max_delay)
        return self.config.base_delay * (attempt + 1)

class RetryExhaustedError(Exception):
    def __init__(self, message: str, last_error: Exception):
        super().__init__(message)
        self.last_error = last_error
```

---

### 4.4 Token Budget Manager

```python
from dataclasses import dataclass, field
from collections import defaultdict

@dataclass
class BudgetAllocation:
    """预算分配"""
    system_prompt: int = 0
    repo_map: int = 0
    conversation: int = 0
    tool_results: int = 0
    response_reserve: int = 4096

@dataclass
class TokenBudgetConfig:
    """预算配置"""
    total_budget: int = 128000
    response_reserve: int = 4096
    safety_margin: float = 0.05
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
        self._usage[component]["actual"] = actual
    
    def get_available(self) -> int:
        total_used = sum(u["actual"] for u in self._usage.values())
        return self.config.total_budget - self.config.response_reserve - total_used
    
    def check_within_budget(self, tokens: int, component: str) -> bool:
        allocation = self.get_allocation()
        limits = {
            "system_prompt": allocation.system_prompt,
            "repo_map": allocation.repo_map,
            "conversation": allocation.conversation,
            "tool_results": allocation.tool_results,
        }
        return tokens <= limits.get(component, 0)
    
    def get_report(self) -> dict:
        return {
            "total": self.config.total_budget,
            "used": sum(u["actual"] for u in self._usage.values()),
            "available": self.get_available(),
            "breakdown": dict(self._usage),
        }
    
    def reset(self) -> None:
        self._usage.clear()
```

---

### 4.5 Context Manager

```python
from typing import Optional

class ContextManager:
    """上下文管理器"""
    
    def __init__(self, max_tokens=128000, budget_manager=None):
        self.max_tokens = max_tokens
        self.budget_manager = budget_manager or TokenBudgetManager(
            TokenBudgetConfig(total_budget=max_tokens)
        )
        self._messages: list[Message] = []
        self._system_prompt: Optional[str] = None
        self._important_messages: set[int] = set()
    
    async def add_message(self, message: Message, important: bool = False):
        """添加消息"""
        message_tokens = self._estimate_tokens(message)
        if not self.budget_manager.check_within_budget(message_tokens, "conversation"):
            await self.compact()
        self._messages.append(message)
        if important:
            self._important_messages.add(id(message))
    
    async def get_messages(self) -> list[Message]:
        """获取所有消息"""
        messages = []
        if self._system_prompt:
            messages.append(Message(role="system", content=self._system_prompt))
        messages.extend(self._messages)
        return messages
    
    async def set_system_prompt(self, prompt: str) -> None:
        """设置系统提示词"""
        self._system_prompt = prompt
    
    def mark_important(self, message: Message) -> None:
        """标记重要消息（压缩时优先保留）"""
        self._important_messages.add(id(message))
    
    async def compact(self, force=False):
        """压缩上下文"""
        important = [m for m in self._messages if id(m) in self._important_messages]
        others = [m for m in self._messages if id(m) not in self._important_messages]
        
        if others:
            recent_count = 5
            to_compress = others[:-recent_count] if len(others) > recent_count else []
            if to_compress:
                summary = self._generate_summary(to_compress)
                self._messages = [
                    Message(role="system", content=f"[历史摘要]\n{summary}"),
                    *others[-recent_count:],
                    *important,
                ]
    
    def _generate_summary(self, messages):
        """生成摘要"""
        tool_calls = []
        for m in messages:
            if m.tool_calls:
                for tc in m.tool_calls:
                    tool_calls.append(f"- {tc.function.name}")
        return f"共 {len(messages)} 条消息，工具调用：\n" + "\n".join(tool_calls[:10])
    
    def _estimate_tokens(self, message: Message) -> int:
        """估算消息 token 数"""
        return len(message.content) // 4 + 10
    
    def get_budget_report(self) -> dict:
        """获取预算报告"""
        return self.budget_manager.get_report()
```

---

### 4.6 Tool System

#### 4.6.1 工具定义与注册

```python
import inspect
from typing import Callable, Any, get_type_hints, get_origin, get_args, Annotated, Union
from pydantic import BaseModel
from dataclasses import dataclass, field
from functools import wraps

class ToolDefinition(BaseModel):
    """工具定义"""
    name: str
    description: str
    parameters: dict
    func: Callable
    timeout: int = 30

@dataclass
class ValidationResult:
    """验证结果"""
    valid: bool
    errors: list[str] = field(default_factory=list)

class ToolNotFoundError(Exception):
    pass

class ToolTimeoutError(Exception):
    pass

class ToolRegistry:
    """工具注册表"""
    
    def __init__(self):
        self._tools: dict[str, ToolDefinition] = {}
    
    def tool(self, name=None, description=None, timeout=30):
        """工具装饰器"""
        def decorator(func):
            tool_name = name or func.__name__
            self._tools[tool_name] = ToolDefinition(
                name=tool_name,
                description=description or func.__doc__ or "",
                parameters=self._generate_schema(func),
                func=func,
                timeout=timeout,
            )
            @wraps(func)
            async def wrapper(*args, **kwargs):
                return await self.invoke(tool_name, *args, **kwargs)
            return wrapper
        return decorator
    
    def _generate_schema(self, func) -> dict:
        """从函数签名生成 JSON Schema"""
        sig = inspect.signature(func)
        hints = get_type_hints(func, include_extras=True)
        properties, required = {}, []
        
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
            
            if "description" not in prop:
                prop["description"] = name
            
            if param.default != inspect.Parameter.empty:
                prop["default"] = param.default
            else:
                required.append(name)
            
            properties[name] = prop
        
        return {"type": "object", "properties": properties, "required": required}
    
    def _type_to_schema(self, type_hint) -> dict:
        """类型转 JSON Schema"""
        # Optional[T]
        if get_origin(type_hint) is Union:
            args = get_args(type_hint)
            non_none = [a for a in args if a is not type(None)]
            if len(non_none) == 1:
                schema = self._type_to_schema(non_none[0])
                return {**schema, "nullable": True}
        
        # List[T]
        if get_origin(type_hint) is list:
            args = get_args(type_hint)
            item_type = args[0] if args else str
            return {"type": "array", "items": self._type_to_schema(item_type)}
        
        type_map = {
            str: {"type": "string"},
            int: {"type": "integer"},
            float: {"type": "number"},
            bool: {"type": "boolean"},
            list: {"type": "array"},
            dict: {"type": "object"},
        }
        return type_map.get(type_hint, {"type": "string"})
    
    def get_schemas(self) -> list[dict]:
        """获取所有工具的 OpenAI 格式 schema"""
        return [
            {"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}}
            for t in self._tools.values()
        ]
    
    def get(self, name: str) -> Optional[ToolDefinition]:
        """获取工具定义"""
        return self._tools.get(name)
    
    async def invoke(self, name: str, **kwargs) -> Any:
        """调用工具"""
        tool = self._tools.get(name)
        if not tool:
            raise ToolNotFoundError(f"Tool '{name}' not found")
        if inspect.iscoroutinefunction(tool.func):
            return await tool.func(**kwargs)
        return tool.func(**kwargs)
    
    async def invoke_with_timeout(self, name: str, timeout: int, **kwargs) -> Any:
        """带超时的调用"""
        try:
            async with asyncio.timeout(timeout):
                return await self.invoke(name, **kwargs)
        except asyncio.TimeoutError:
            raise ToolTimeoutError(f"Tool '{name}' timed out after {timeout}s")
    
    async def validate_input(self, name: str, args: dict) -> ValidationResult:
        """验证输入"""
        tool = self._tools.get(name)
        if not tool:
            return ValidationResult(valid=False, errors=["Tool not found"])
        required = tool.parameters.get("required", [])
        for param in required:
            if param not in args:
                return ValidationResult(valid=False, errors=[f"Missing: {param}"])
        return ValidationResult(valid=True)
```

#### 4.6.2 内置工具示例

```python
from typing import Annotated
import subprocess
import os

# 文件操作工具
@registry.tool(description="读取文件内容")
async def read_file(
    path: Annotated[str, "文件路径"],
    encoding: Annotated[str, "文件编码"] = "utf-8",
) -> str:
    with open(path, "r", encoding=encoding) as f:
        return f.read()

@registry.tool(description="写入文件内容")
async def write_file(
    path: Annotated[str, "文件路径"],
    content: Annotated[str, "文件内容"],
    encoding: Annotated[str, "文件编码"] = "utf-8",
) -> str:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding=encoding) as f:
        f.write(content)
    return f"写入成功: {path}"

# Shell 命令工具
@registry.tool(description="执行 Shell 命令", timeout=60)
async def run_command(
    command: Annotated[str, "要执行的命令"],
    cwd: Annotated[str, "工作目录"] = ".",
) -> str:
    result = subprocess.run(
        command, shell=True, capture_output=True, text=True, cwd=cwd
    )
    output = result.stdout
    if result.returncode != 0:
        output += f"\n[ERROR] {result.stderr}"
    return output

# 搜索工具
@registry.tool(description="搜索文件")
async def search_files(
    pattern: Annotated[str, "搜索模式，支持 glob"],
    max_results: Annotated[int, "最大返回数量"] = 10,
) -> list[str]:
    import glob
    return glob.glob(pattern, recursive=True)[:max_results]
```

---

### 4.7 Agent Runtime

```python
import json

class AgentRuntime:
    """Agent 执行运行时 - 核心循环"""
    
    def __init__(
        self,
        llm: LLMProvider,
        model: str,
        tools: ToolRegistry,
        context: ContextManager,
        tracer: "SimpleTracer" = None,
    ):
        self.llm = llm
        self.model = model
        self.tools = tools
        self.context = context
        self.tracer = tracer or SimpleTracer()
        self.max_iterations = 30
        self.max_corrections = 3
    
    async def run(self, user_input: str) -> str:
        """执行 Agent 循环"""
        # 1. 添加用户消息
        await self.context.add_message(Message(role="user", content=user_input))
        
        # 2. Tool Calling Loop
        for i in range(self.max_iterations):
            messages = await self.context.get_messages()
            tool_schemas = self.tools.get_schemas()
            
            # 调用 LLM
            span = self.tracer.start("llm.chat", model=self.model)
            response = await self.llm.chat_with_retry(
                messages=messages,
                model=self.model,
                tools=tool_schemas if tool_schemas else None,
            )
            self.tracer.end(span)
            
            # 3. 处理响应
            if response.finish_reason == "stop":
                await self.context.add_message(Message(
                    role="assistant", content=response.content,
                ))
                return response.content
            
            elif response.finish_reason == "tool_calls":
                await self.context.add_message(Message(
                    role="assistant", content=response.content, tool_calls=response.tool_calls,
                ))
                
                # 执行所有工具调用
                for tool_call in response.tool_calls:
                    result = await self._execute_tool(tool_call)
                    
                    # 工具结果注入上下文（反馈闭环）
                    await self.context.add_message(Message(
                        role="tool",
                        content=result,
                        tool_call_id=tool_call.id,
                        name=tool_call.function.name,
                    ))
            
            else:
                break
        
        return "达到最大迭代次数"
    
    async def _execute_tool(self, tool_call: ToolCall) -> str:
        """执行单个工具调用"""
        tool_name = tool_call.function.name
        span = self.tracer.start("tool.call", tool=tool_name)
        
        try:
            args = json.loads(tool_call.function.arguments)
            
            # 验证输入
            validation = await self.tools.validate_input(tool_name, args)
            if not validation.valid:
                return f"输入验证失败: {validation.errors}"
            
            # 获取工具超时
            tool = self.tools.get(tool_name)
            timeout = getattr(tool, "timeout", 30)
            
            # 执行工具
            result = await self.tools.invoke_with_timeout(tool_name, timeout, **args)
            self.tracer.end(span)
            return str(result)
        
        except ToolTimeoutError as e:
            self.tracer.end(span, status="error")
            return f"工具超时: {e}"
        except Exception as e:
            self.tracer.end(span, status="error")
            return f"工具执行失败: {e}"
```

---

### 4.8 Workflow Engine

#### 4.8.1 状态图定义

```python
from typing import Callable, Optional
import asyncio

class WorkflowNode:
    """工作流节点"""
    def __init__(self, name: str, func: Callable):
        self.name = name
        self.func = func
    
    async def execute(self, state: dict) -> dict:
        if inspect.iscoroutinefunction(self.func):
            return await self.func(state)
        return self.func(state)

class WorkflowEdge:
    """工作流边"""
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
    """状态图"""
    
    def __init__(self, state_schema: type = dict):
        self.state_schema = state_schema
        self.nodes: dict[str, WorkflowNode] = {}
        self.edges: list[WorkflowEdge] = []
        self.entry_point: Optional[str] = None
    
    def add_node(self, name: str, func: Callable) -> "StateGraph":
        self.nodes[name] = WorkflowNode(name, func)
        return self
    
    def add_edge(self, source: str, target: str) -> "StateGraph":
        self.edges.append(WorkflowEdge(source, target))
        return self
    
    def add_conditional_edge(self, source: str, condition: Callable, targets: dict) -> "StateGraph":
        for value, target in targets.items():
            self.edges.append(WorkflowEdge(
                source, target,
                condition=lambda s, v=value: condition(s) == v,
            ))
        return self
    
    def add_parallel(self, source: str, targets: list[str]) -> "StateGraph":
        for target in targets:
            self.edges.append(WorkflowEdge(source, target, parallel=True))
        return self
    
    def set_entry_point(self, name: str) -> "StateGraph":
        self.entry_point = name
        return self
    
    def compile(self, checkpoint_dir: Optional[str] = None) -> "CompiledGraph":
        return CompiledGraph(self, checkpoint_dir)

class CompiledGraph:
    """编译后的工作流"""
    
    def __init__(self, graph: StateGraph, checkpoint_dir: Optional[str] = None):
        self.graph = graph
        self.checkpoint_manager = CheckpointManager(checkpoint_dir) if checkpoint_dir else None
    
    async def invoke(self, initial_state: dict) -> dict:
        state = initial_state.copy()
        current = self.graph.entry_point
        
        while current and current != "__end__":
            node = self.graph.nodes.get(current)
            if not node:
                break
            
            # 执行节点
            state = await node.execute(state)
            
            # 保存检查点
            if self.checkpoint_manager:
                self.checkpoint_manager.save(current, state)
            
            # 查找后续节点
            edges = [e for e in self.graph.edges if e.source == current]
            parallel = [e.target for e in edges if e.parallel]
            sequential = [e for e in edges if not e.parallel]
            
            if parallel:
                # 并行执行
                results = await asyncio.gather(
                    *[self._execute_node(t, state.copy()) for t in parallel]
                )
                for r in results:
                    state.update(r)
                current = None
            else:
                # 顺序执行
                current = None
                for edge in sequential:
                    if edge.condition is None or edge.condition(state):
                        current = edge.target
                        break
        
        return state
    
    async def _execute_node(self, node_name: str, state: dict) -> dict:
        node = self.graph.nodes.get(node_name)
        if node:
            return await node.execute(state)
        return state
```

#### 4.8.2 检查点管理

```python
import json
import os
import time

class CheckpointManager:
    """检查点管理"""
    
    def __init__(self, checkpoint_dir=".continuum/checkpoints"):
        self.checkpoint_dir = checkpoint_dir
        os.makedirs(checkpoint_dir, exist_ok=True)
    
    def save(self, node_name: str, state: dict) -> str:
        """保存检查点"""
        checkpoint_id = f"{node_name}_{int(time.time())}"
        filepath = os.path.join(self.checkpoint_dir, f"{checkpoint_id}.json")
        with open(filepath, "w") as f:
            json.dump({"id": checkpoint_id, "state": state}, f, indent=2)
        return checkpoint_id
    
    def load(self, checkpoint_id: str) -> Optional[dict]:
        """加载检查点"""
        filepath = os.path.join(self.checkpoint_dir, f"{checkpoint_id}.json")
        if os.path.exists(filepath):
            with open(filepath, "r") as f:
                return json.load(f).get("state")
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

### 4.9 Agent Planner

```python
import uuid
from enum import Enum
from dataclasses import dataclass, field

class ComplexityLevel(Enum):
    SIMPLE = "simple"
    MODERATE = "moderate"
    COMPLEX = "complex"
    VERY_COMPLEX = "very_complex"

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

@dataclass
class Plan:
    """执行计划"""
    id: str
    task: str
    complexity: ComplexityLevel
    strategy: ExecutionStrategy
    subtasks: list[SubTask]

class Planner:
    """任务规划器"""
    
    def __init__(self, llm: LLMProvider, model: str):
        self.llm = llm
        self.model = model
    
    async def analyze(self, task: str) -> tuple[ComplexityLevel, ExecutionStrategy]:
        """分析任务复杂度"""
        prompt = f"""分析任务复杂度：
任务：{task}

返回 JSON：
{{"complexity": "simple|moderate|complex|very_complex", "strategy": "single_turn|multi_turn|tool_chain|workflow"}}"""
        resp = await self.llm.chat([Message(role="user", content=prompt)], self.model)
        try:
            result = json.loads(resp.content)
            return ComplexityLevel(result["complexity"]), ExecutionStrategy(result["strategy"])
        except:
            return ComplexityLevel.MODERATE, ExecutionStrategy.MULTI_TURN
    
    async def decompose(self, task: str) -> list[SubTask]:
        """分解任务"""
        prompt = f"""分解任务为子任务：
任务：{task}

返回 JSON：
{{"subtasks": [{{"id": "subtask_1", "description": "描述", "dependencies": [], "required_tools": []}}]}}"""
        resp = await self.llm.chat([Message(role="user", content=prompt)], self.model)
        try:
            result = json.loads(resp.content)
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
        )
```

---

### 4.10 Observability（简化版）

```python
import time
from collections import defaultdict
from dataclasses import dataclass, field

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
        return (self.end_time - self.start_time) * 1000 if self.end_time else None

class SimpleTracer:
    """简单追踪器"""
    
    def __init__(self, verbose=True):
        self.verbose = verbose
        self._spans: list[TraceSpan] = []
        self._indent = 0
    
    def start(self, name: str, **attrs) -> TraceSpan:
        span = TraceSpan(name=name, start_time=time.time(), attributes=attrs)
        self._spans.append(span)
        if self.verbose:
            print(f"{'  ' * self._indent}→ {name}")
            self._indent += 1
        return span
    
    def end(self, span: TraceSpan, status="ok"):
        span.end_time = time.time()
        span.status = status
        if self.verbose:
            self._indent -= 1
            icon = "✓" if status == "ok" else "✗"
            print(f"{'  ' * self._indent}{icon} {span.name} ({span.duration_ms:.1f}ms)")
    
    def get_summary(self) -> dict:
        return {
            "total_spans": len(self._spans),
            "total_time_ms": sum(s.duration_ms or 0 for s in self._spans),
            "errors": [s.name for s in self._spans if s.status != "ok"],
        }

class SimpleMetrics:
    """简单指标收集"""
    
    def __init__(self):
        self._counters = defaultdict(int)
        self._timings = defaultdict(list)
    
    def count(self, name: str, value: int = 1):
        self._counters[name] += value
    
    def timing(self, name: str, value_ms: float):
        self._timings[name].append(value_ms)
    
    def get_summary(self) -> dict:
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
    
    def print_summary(self):
        print("\n=== 指标摘要 ===")
        for name, value in self._counters.items():
            print(f"  {name}: {value}")
        for name, stats in self.get_summary()["timings"].items():
            print(f"  {name}: avg={stats['avg_ms']:.1f}ms, count={stats['count']}")
```

---

### 4.11 Config System

```python
import os
import yaml
from typing import Any, Optional

DEFAULTS = {
    "model.provider": "openai",
    "model.name": "gpt-4-turbo",
    "model.temperature": 0.7,
    "context.max_tokens": 128000,
    "context.auto_compact": True,
    "agent.max_iterations": 30,
    "agent.max_corrections": 3,
    "retry.max_retries": 3,
    "tool.default_timeout": 30,
    "observability.verbose": True,
}

class Config:
    """配置管理"""
    
    def __init__(self, config_file=None):
        self._config = {}
        if config_file:
            self._load_file(config_file)
        elif os.path.exists("continuum.yaml"):
            self._load_file("continuum.yaml")
    
    def _load_file(self, filepath):
        with open(filepath, "r", encoding="utf-8") as f:
            self._config = yaml.safe_load(f) or {}
    
    def get(self, key, default=None):
        """获取配置（优先级：环境变量 > 配置文件 > 默认值）"""
        env_key = "EGG_" + key.upper().replace(".", "_")
        env_value = os.environ.get(env_key)
        if env_value is not None:
            if env_value.lower() in ("true", "false"):
                return env_value.lower() == "true"
            try:
                return int(env_value)
            except ValueError:
                try:
                    return float(env_value)
                except ValueError:
                    return env_value
        
        keys = key.split(".")
        value = self._config
        for k in keys:
            if isinstance(value, dict):
                value = value.get(k)
            else:
                return DEFAULTS.get(key, default)
        return value if value is not None else DEFAULTS.get(key, default)
    
    def get_api_key(self, provider):
        """获取 API Key"""
        return os.environ.get(f"{provider.upper()}_API_KEY")
```

配置文件 `continuum.yaml`:

```yaml
model:
  provider: openai
  name: gpt-4-turbo
  temperature: 0.7

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
```

---

### 4.12 Memory System

记忆系统让 Agent 能够跨会话持久化知识，分为项目记忆和自动记忆。

```python
import os
from dataclasses import dataclass, field
from typing import Optional
import json

@dataclass
class MemoryEntry:
    """记忆条目"""
    key: str
    value: str
    category: str = "general"
    created_at: float = 0.0
    access_count: int = 0
    last_accessed: float = 0.0

class ProjectMemory:
    """项目记忆 - 类似 CLAUDE.md

    用于存储项目级别的持久知识：
    - 代码风格规范
    - 架构决策
    - 常用命令
    - 注意事项
    """

    def __init__(self, project_root: str, memory_file: str = "EGG.md"):
        self.project_root = project_root
        self.memory_file = os.path.join(project_root, memory_file)
        self._cache: Optional[str] = None

    async def load(self) -> str:
        """加载项目记忆"""
        if self._cache is not None:
            return self._cache

        if os.path.exists(self.memory_file):
            with open(self.memory_file, "r", encoding="utf-8") as f:
                self._cache = f.read()
                return self._cache

        return ""

    async def save(self, content: str) -> None:
        """保存项目记忆"""
        with open(self.memory_file, "w", encoding="utf-8") as f:
            f.write(content)
        self._cache = content

    async def append(self, section: str, content: str) -> None:
        """追加内容到指定章节"""
        existing = await self.load()

        # 查找章节
        section_header = f"## {section}"
        if section_header in existing:
            # 追加到现有章节
            lines = existing.split("\n")
            new_lines = []
            inserted = False
            for line in lines:
                new_lines.append(line)
                if line.strip() == section_header and not inserted:
                    new_lines.append(content)
                    inserted = True
            await self.save("\n".join(new_lines))
        else:
            # 创建新章节
            if existing.strip():
                await self.save(f"{existing}\n\n{section_header}\n{content}")
            else:
                await self.save(f"# Project Memory\n\n{section_header}\n{content}")

    async def get_section(self, section: str) -> Optional[str]:
        """获取特定章节内容"""
        existing = await self.load()
        section_header = f"## {section}"

        if section_header not in existing:
            return None

        lines = existing.split("\n")
        in_section = False
        section_content = []

        for line in lines:
            if line.strip() == section_header:
                in_section = True
                continue
            if in_section:
                if line.startswith("## "):
                    break
                section_content.append(line)

        return "\n".join(section_content).strip()

class AutoMemory:
    """自动记忆 - 跨会话学习

    Agent 自动学习的知识：
    - 用户偏好
    - 常见错误解决方案
    - 成功的操作模式
    """

    def __init__(self, storage: "StorageBackend", project_root: str):
        self.storage = storage
        self.project_root = project_root
        self._memory_key = f"memory:{project_root}"

    async def learn(self, key: str, value: str, category: str = "general") -> None:
        """学习新知识"""
        import time

        entry = MemoryEntry(
            key=key,
            value=value,
            category=category,
            created_at=time.time(),
            access_count=0,
            last_accessed=time.time(),
        )

        memories = await self._load_memories()
        memories[key] = entry
        await self._save_memories(memories)

    async def recall(self, query: str, category: Optional[str] = None) -> list[MemoryEntry]:
        """回忆相关知识"""
        memories = await self._load_memories()

        results = []
        query_lower = query.lower()

        for entry in memories.values():
            # 分类过滤
            if category and entry.category != category:
                continue

            # 关键词匹配
            if query_lower in entry.key.lower() or query_lower in entry.value.lower():
                entry.access_count += 1
                entry.last_accessed = time.time()
                results.append(entry)

        # 按访问次数排序
        results.sort(key=lambda x: x.access_count, reverse=True)

        # 更新访问记录
        for entry in results:
            memories[entry.key] = entry
        await self._save_memories(memories)

        return results

    async def forget(self, key: str) -> bool:
        """遗忘知识"""
        memories = await self._load_memories()
        if key in memories:
            del memories[key]
            await self._save_memories(memories)
            return True
        return False

    async def _load_memories(self) -> dict[str, MemoryEntry]:
        """加载记忆"""
        data = await self.storage.get(self._memory_key) or {}
        return {k: MemoryEntry(**v) for k, v in data.items()}

    async def _save_memories(self, memories: dict[str, MemoryEntry]) -> None:
        """保存记忆"""
        data = {
            k: {
                "key": v.key,
                "value": v.value,
                "category": v.category,
                "created_at": v.created_at,
                "access_count": v.access_count,
                "last_accessed": v.last_accessed,
            }
            for k, v in memories.items()
        }
        await self.storage.set(self._memory_key, data)
```

---

### 4.13 Hooks System

钩子系统提供工具执行生命周期的扩展点，用于日志、指标、权限等横切关注点。

```python
from abc import ABC, abstractmethod
from enum import Enum
from typing import Callable, Any, Optional
from dataclasses import dataclass, field

class HookType(Enum):
    """钩子类型"""
    BEFORE_TOOL = "before_tool"
    AFTER_TOOL = "after_tool"
    ON_ERROR = "on_error"
    BEFORE_LLM = "before_llm"
    AFTER_LLM = "after_llm"
    ON_CONTEXT_COMPACT = "on_context_compact"
    ON_AGENT_START = "on_agent_start"
    ON_AGENT_END = "on_agent_end"

@dataclass
class HookContext:
    """钩子上下文"""
    hook_type: HookType
    agent_name: str
    tool_name: Optional[str] = None
    args: dict = field(default_factory=dict)
    result: Any = None
    error: Optional[Exception] = None
    metadata: dict = field(default_factory=dict)

class Hook(ABC):
    """钩子基类"""

    @property
    @abstractmethod
    def hook_type(self) -> HookType:
        """钩子类型"""
        pass

    @abstractmethod
    async def execute(self, context: HookContext) -> Optional[HookContext]:
        """执行钩子，返回修改后的上下文或 None（阻止后续执行）"""
        pass

class HookSystem:
    """钩子系统"""

    def __init__(self):
        self._hooks: dict[HookType, list[Hook]] = {t: [] for t in HookType}

    def register(self, hook: Hook) -> None:
        """注册钩子"""
        self._hooks[hook.hook_type].append(hook)

    def unregister(self, hook: Hook) -> None:
        """注销钩子"""
        self._hooks[hook.hook_type].remove(hook)

    async def trigger(self, context: HookContext) -> Optional[HookContext]:
        """触发钩子"""
        for hook in self._hooks[context.hook_type]:
            result = await hook.execute(context)
            if result is None:
                # 钩子返回 None 表示阻止执行
                return None
            context = result
        return context

# 内置钩子示例
class LoggingHook(Hook):
    """日志钩子"""

    @property
    def hook_type(self) -> HookType:
        return HookType.BEFORE_TOOL

    def __init__(self, logger):
        self.logger = logger

    async def execute(self, context: HookContext) -> HookContext:
        self.logger.info(f"[{context.agent_name}] Calling tool: {context.tool_name}")
        self.logger.debug(f"Args: {context.args}")
        return context

class MetricsHook(Hook):
    """指标钩子"""

    @property
    def hook_type(self) -> HookType:
        return HookType.AFTER_TOOL

    def __init__(self, metrics: "SimpleMetrics"):
        self.metrics = metrics

    async def execute(self, context: HookContext) -> HookContext:
        if context.tool_name:
            self.metrics.count(f"tool.{context.tool_name}")
            if context.metadata.get("duration_ms"):
                self.metrics.timing(
                    f"tool.{context.tool_name}.latency",
                    context.metadata["duration_ms"]
                )
        return context

class PermissionHook(Hook):
    """权限检查钩子"""

    @property
    def hook_type(self) -> HookType:
        return HookType.BEFORE_TOOL

    def __init__(self, blocked_tools: list[str] = None, allowed_tools: list[str] = None):
        self.blocked_tools = blocked_tools or []
        self.allowed_tools = allowed_tools

    async def execute(self, context: HookContext) -> Optional[HookContext]:
        tool_name = context.tool_name

        # 检查黑名单
        if tool_name in self.blocked_tools:
            return None  # 阻止执行

        # 检查白名单
        if self.allowed_tools and tool_name not in self.allowed_tools:
            return None

        return context

class CachingHook(Hook):
    """缓存钩子 - 为只读工具添加缓存"""

    @property
    def hook_type(self) -> HookType:
        return HookType.BEFORE_TOOL

    def __init__(self, cache: dict, read_only_tools: list[str]):
        self.cache = cache
        self.read_only_tools = read_only_tools

    async def execute(self, context: HookContext) -> Optional[HookContext]:
        if context.tool_name not in self.read_only_tools:
            return context

        # 生成缓存键
        cache_key = f"{context.tool_name}:{hash(str(context.args))}"

        if cache_key in self.cache:
            # 命中缓存，直接返回结果
            context.result = self.cache[cache_key]
            context.metadata["from_cache"] = True
            return context

        return context

# 使用示例
hooks = HookSystem()
hooks.register(LogingHook(logger))
hooks.register(MetricsHook(metrics))
hooks.register(PermissionHook(blocked_tools=["run_command", "delete_file"]))
```

---

### 4.14 自我修正循环

Agent 执行失败后的自动分析和修正机制。

```python
from dataclasses import dataclass
from typing import Optional
import json

@dataclass
class VerificationResult:
    """验证结果"""
    success: bool
    error_type: Optional[str] = None
    error_message: Optional[str] = None
    suggestion: Optional[str] = None

class SelfCorrectionLoop:
    """自我修正循环"""

    def __init__(
        self,
        llm: LLMProvider,
        model: str,
        max_corrections: int = 3,
        verification_tools: list[str] = None,
    ):
        self.llm = llm
        self.model = model
        self.max_corrections = max_corrections
        self.verification_tools = verification_tools or ["run_tests", "check_syntax", "lint"]

    async def run_with_correction(
        self,
        agent: "AgentRuntime",
        task: str,
        context: ContextManager,
    ) -> str:
        """执行任务，失败时自动修正"""

        for correction_attempt in range(self.max_corrections + 1):
            # 执行任务
            result = await agent.run(task)

            # 验证结果
            verification = await self._verify_result(agent, result)

            if verification.success:
                return result

            # 达到最大修正次数
            if correction_attempt == self.max_corrections:
                return f"修正失败（{self.max_corrections} 次后仍失败）：\n{verification.error_message}"

            # 生成修正策略
            correction_prompt = await self._generate_correction_prompt(
                result=result,
                verification=verification,
                context=context,
                attempt=correction_attempt,
            )

            # 注入修正提示到上下文
            await context.add_message(Message(
                role="user",
                content=correction_prompt,
            ))

            # 记录修正尝试
            print(f"[Self-Correction] Attempt {correction_attempt + 1}/{self.max_corrections}")
            print(f"  Error: {verification.error_type}")
            print(f"  Suggestion: {verification.suggestion}")

    async def _verify_result(
        self,
        agent: AgentRuntime,
        result: str,
    ) -> VerificationResult:
        """验证执行结果"""

        # 1. 检查语法错误（如果是代码）
        syntax_check = await self._check_syntax(agent, result)
        if not syntax_check.success:
            return syntax_check

        # 2. 运行测试（如果有）
        test_check = await self._run_tests(agent, result)
        if not test_check.success:
            return test_check

        # 3. 使用 LLM 进行语义验证
        semantic_check = await self._semantic_verify(agent, result)
        if not semantic_check.success:
            return semantic_check

        return VerificationResult(success=True)

    async def _check_syntax(self, agent: AgentRuntime, result: str) -> VerificationResult:
        """检查语法错误"""
        # 如果结果是代码，检查语法
        if "def " in result or "class " in result or "import " in result:
            try:
                compile(result, "<string>", "exec")
            except SyntaxError as e:
                return VerificationResult(
                    success=False,
                    error_type="syntax_error",
                    error_message=str(e),
                    suggestion=f"修复语法错误：第 {e.lineno} 行，{e.msg}",
                )
        return VerificationResult(success=True)

    async def _run_tests(self, agent: AgentRuntime, result: str) -> VerificationResult:
        """运行测试"""
        # 调用测试工具
        try:
            # 这里假设 agent 有工具调用能力
            test_result = await agent.tools.invoke("run_tests")
            if "FAILED" in str(test_result) or "ERROR" in str(test_result):
                return VerificationResult(
                    success=False,
                    error_type="test_failure",
                    error_message=str(test_result),
                    suggestion="检查失败的测试用例，修复相关代码",
                )
        except Exception:
            pass  # 测试工具不可用时跳过

        return VerificationResult(success=True)

    async def _semantic_verify(
        self,
        agent: AgentRuntime,
        result: str,
    ) -> VerificationResult:
        """语义验证"""
        prompt = f"""验证以下结果是否正确完成任务：

结果：
{result[:2000]}

如果正确，返回：{{"success": true}}
如果有问题，返回：
{{
  "success": false,
  "error_type": "问题类型",
  "error_message": "详细描述",
  "suggestion": "修复建议"
}}
"""
        response = await self.llm.chat(
            messages=[Message(role="user", content=prompt)],
            model=self.model,
        )

        try:
            data = json.loads(response.content)
            return VerificationResult(**data)
        except:
            return VerificationResult(success=True)

    async def _generate_correction_prompt(
        self,
        result: str,
        verification: VerificationResult,
        context: ContextManager,
        attempt: int,
    ) -> str:
        """生成修正提示"""
        # 获取历史上下文摘要
        history_summary = await self._get_recent_history(context)

        return f"""
之前的执行失败了，请根据以下信息修正：

=== 错误信息 ===
类型：{verification.error_type}
描述：{verification.error_message}

=== 修复建议 ===
{verification.suggestion}

=== 最近操作 ===
{history_summary}

=== 你的输出 ===
{result[:1000]}

请分析错误原因，并重新执行。这是第 {attempt + 1} 次修正尝试。
"""

    async def _get_recent_history(self, context: ContextManager) -> str:
        """获取最近历史摘要"""
        messages = await context.get_messages()
        recent = messages[-10:] if len(messages) > 10 else messages

        summary = []
        for msg in recent:
            if msg.role == "assistant":
                summary.append(f"Assistant: {msg.content[:100]}...")
            elif msg.role == "tool":
                summary.append(f"Tool result: {msg.content[:100]}...")

        return "\n".join(summary)
```

---

### 4.15 流式输出支持

实时流式响应，提升用户体验。

```python
from typing import AsyncIterator
from dataclasses import dataclass
import json

@dataclass
class StreamChunk:
    """流式响应块"""
    content: str
    is_final: bool = False
    tool_calls: Optional[list[dict]] = None

class StreamHandler:
    """流式处理器"""

    def __init__(self):
        self._buffer = ""
        self._tool_calls_buffer: dict[str, dict] = {}

    async def process_stream(
        self,
        stream: AsyncIterator[dict],
    ) -> AsyncIterator[StreamChunk]:
        """处理 LLM 流式响应"""
        async for chunk in stream:
            # 处理内容增量
            if "content" in chunk:
                content = chunk["content"]
                self._buffer += content
                yield StreamChunk(content=content)

            # 处理工具调用增量
            if "tool_calls_delta" in chunk:
                for tc in chunk["tool_calls_delta"]:
                    tc_id = tc.get("id")
                    if tc_id:
                        if tc_id not in self._tool_calls_buffer:
                            self._tool_calls_buffer[tc_id] = {
                                "id": tc_id,
                                "type": "function",
                                "function": {"name": "", "arguments": ""},
                            }

                        if "name" in tc:
                            self._tool_calls_buffer[tc_id]["function"]["name"] += tc["name"]
                        if "arguments" in tc:
                            self._tool_calls_buffer[tc_id]["function"]["arguments"] += tc["arguments"]

            # 流结束
            if chunk.get("is_final"):
                yield StreamChunk(
                    content="",
                    is_final=True,
                    tool_calls=list(self._tool_calls_buffer.values()) if self._tool_calls_buffer else None,
                )
                self._buffer = ""
                self._tool_calls_buffer.clear()

class StreamingAgentRuntime:
    """支持流式输出的 Agent Runtime"""

    def __init__(self, base_runtime: AgentRuntime):
        self.base = base_runtime

    async def run_streaming(
        self,
        user_input: str,
    ) -> AsyncIterator[StreamChunk]:
        """流式执行 Agent"""

        # 添加用户消息
        await self.base.context.add_message(Message(
            role="user",
            content=user_input,
        ))

        for iteration in range(self.base.max_iterations):
            messages = await self.base.context.get_messages()
            tool_schemas = self.base.tools.get_schemas()

            # 流式调用 LLM
            stream = await self.base.llm.chat_streaming(
                messages=messages,
                model=self.base.model,
                tools=tool_schemas if tool_schemas else None,
            )

            handler = StreamHandler()
            final_chunk = None

            async for chunk in handler.process_stream(stream):
                yield chunk
                if chunk.is_final:
                    final_chunk = chunk

            # 处理最终结果
            if final_chunk:
                if final_chunk.tool_calls:
                    # 有工具调用
                    await self.base.context.add_message(Message(
                        role="assistant",
                        content=self.base._buffer,
                        tool_calls=[
                            ToolCall(
                                id=tc["id"],
                                function=FunctionCall(
                                    name=tc["function"]["name"],
                                    arguments=tc["function"]["arguments"],
                                )
                            )
                            for tc in final_chunk.tool_calls
                        ]
                    ))

                    # 执行工具（可以并行）
                    for tc in final_chunk.tool_calls:
                        result = await self.base._execute_tool(
                            ToolCall(
                                id=tc["id"],
                                function=FunctionCall(
                                    name=tc["function"]["name"],
                                    arguments=tc["function"]["arguments"],
                                )
                            )
                        )
                        yield StreamChunk(content=f"\n[Tool: {tc['function']['name']}]\n{result}\n")

                elif final_chunk.content is not None or self.base._buffer:
                    # 纯文本响应，结束
                    await self.base.context.add_message(Message(
                        role="assistant",
                        content=self.base._buffer,
                    ))
                    return

# LLM Provider 流式支持
class LLMProvider(ABC):
    # ... 原有方法 ...

    @abstractmethod
    async def chat_streaming(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
    ) -> AsyncIterator[dict]:
        """流式聊天"""
        pass

# OpenAI 流式实现
class OpenAIProvider(LLMProvider):
    async def chat_streaming(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
    ) -> AsyncIterator[dict]:
        payload = {
            "model": model,
            "messages": [self._format_msg(m) for m in messages],
            "temperature": temperature,
            "stream": True,
        }
        if tools:
            payload["tools"] = tools

        async with self.client.stream("POST", "/chat/completions", json=payload) as resp:
            async for line in resp.aiter_lines():
                if line.startswith("data: "):
                    data = line[6:]
                    if data == "[DONE]":
                        yield {"is_final": True}
                        break

                    chunk = json.loads(data)
                    delta = chunk["choices"][0].get("delta", {})

                    if "content" in delta:
                        yield {"content": delta["content"]}

                    if "tool_calls" in delta:
                        yield {"tool_calls_delta": delta["tool_calls"]}
```

---

### 4.16 MCP 支持

Model Context Protocol 客户端，用于标准化工具扩展。

```python
import json
from typing import Any
from dataclasses import dataclass
import asyncio

@dataclass
class MCPTool:
    """MCP 工具定义"""
    name: str
    description: str
    input_schema: dict

@dataclass
class MCPToolResult:
    """MCP 工具执行结果"""
    content: list[dict]
    is_error: bool = False

class MCPClient:
    """MCP 协议客户端

    支持连接外部 MCP 服务器，获取其提供的工具：
    - filesystem MCP: 文件系统操作
    - github MCP: GitHub API
    - postgres MCP: 数据库查询
    - 自定义 MCP 服务器
    """

    def __init__(self):
        self._servers: dict[str, "MCPServerConnection"] = {}
        self._tools: dict[str, MCPTool] = {}

    async def connect(
        self,
        name: str,
        command: str,
        args: list[str] = None,
        env: dict = None,
    ) -> None:
        """连接 MCP 服务器

        Args:
            name: 服务器名称（用于引用）
            command: 启动命令（如 "uvx", "npx"）
            args: 命令参数（如 ["mcp-server-filesystem", "/path"]）
            env: 环境变量
        """
        server = MCPServerConnection(name, command, args or [], env or {})
        await server.start()
        self._servers[name] = server

        # 获取服务器提供的工具
        tools = await server.list_tools()
        for tool in tools:
            self._tools[tool.name] = tool

    async def disconnect(self, name: str) -> None:
        """断开 MCP 服务器"""
        if name in self._servers:
            await self._servers[name].stop()
            del self._servers[name]

    async def list_tools(self) -> list[MCPTool]:
        """列出所有已连接服务器提供的工具"""
        return list(self._tools.values())

    async def get_tool(self, name: str) -> Optional[MCPTool]:
        """获取特定工具"""
        return self._tools.get(name)

    async def call_tool(
        self,
        name: str,
        arguments: dict,
    ) -> MCPToolResult:
        """调用 MCP 工具"""
        tool = self._tools.get(name)
        if not tool:
            raise ValueError(f"Tool '{name}' not found")

        # 找到提供该工具的服务器
        for server in self._servers.values():
            if name in [t.name for t in await server.list_tools()]:
                return await server.call_tool(name, arguments)

        raise ValueError(f"No server provides tool '{name}'")

    def get_schemas(self) -> list[dict]:
        """获取所有工具的 OpenAI 格式 schema（用于 Tool Registry 集成）"""
        return [
            {
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            }
            for tool in self._tools.values()
        ]

class MCPServerConnection:
    """MCP 服务器连接"""

    def __init__(
        self,
        name: str,
        command: str,
        args: list[str],
        env: dict,
    ):
        self.name = name
        self.command = command
        self.args = args
        self.env = env
        self._process: Optional[asyncio.subprocess.Process] = None
        self._request_id = 0

    async def start(self) -> None:
        """启动 MCP 服务器进程"""
        import os

        full_env = os.environ.copy()
        full_env.update(self.env)

        self._process = await asyncio.create_subprocess_exec(
            self.command,
            *self.args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=full_env,
        )

        # 发送初始化请求
        await self._send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "continuum", "version": "1.0.0"},
        })

    async def stop(self) -> None:
        """停止服务器"""
        if self._process:
            self._process.terminate()
            await self._process.wait()
            self._process = None

    async def list_tools(self) -> list[MCPTool]:
        """获取服务器工具列表"""
        response = await self._send_request("tools/list", {})
        return [
            MCPTool(
                name=tool["name"],
                description=tool.get("description", ""),
                input_schema=tool.get("inputSchema", {}),
            )
            for tool in response.get("tools", [])
        ]

    async def call_tool(
        self,
        name: str,
        arguments: dict,
    ) -> MCPToolResult:
        """调用工具"""
        response = await self._send_request("tools/call", {
            "name": name,
            "arguments": arguments,
        })

        return MCPToolResult(
            content=response.get("content", []),
            is_error=response.get("isError", False),
        )

    async def _send_request(self, method: str, params: dict) -> dict:
        """发送 JSON-RPC 请求"""
        self._request_id += 1

        request = {
            "jsonrpc": "2.0",
            "id": self._request_id,
            "method": method,
            "params": params,
        }

        # 发送请求
        request_line = json.dumps(request) + "\n"
        self._process.stdin.write(request_line.encode())
        await self._process.stdin.drain()

        # 读取响应
        response_line = await self._process.stdout.readline()
        response = json.loads(response_line.decode())

        if "error" in response:
            raise Exception(response["error"])

        return response.get("result", {})

# 使用示例
async def setup_mcp_tools():
    mcp = MCPClient()

    # 连接 filesystem MCP
    await mcp.connect(
        "filesystem",
        "uvx",
        ["mcp-server-filesystem", "/home/user/project"],
    )

    # 连接 github MCP
    await mcp.connect(
        "github",
        "uvx",
        ["mcp-server-github"],
        env={"GITHUB_TOKEN": "ghp_xxx"},
    )

    return mcp

# 集成到 Tool Registry
class ToolRegistry:
    def __init__(self, mcp_client: Optional[MCPClient] = None):
        self._tools: dict[str, ToolDefinition] = {}
        self.mcp = mcp_client

    async def invoke(self, name: str, **kwargs) -> Any:
        # 先检查本地工具
        if name in self._tools:
            return await self._tools[name].func(**kwargs)

        # 再检查 MCP 工具
        if self.mcp:
            result = await self.mcp.call_tool(name, kwargs)
            return result.content

        raise ToolNotFoundError(f"Tool '{name}' not found")

    def get_schemas(self) -> list[dict]:
        schemas = [
            {"type": "function", "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            }}
            for t in self._tools.values()
        ]

        # 添加 MCP 工具
        if self.mcp:
            schemas.extend(self.mcp.get_schemas())

        return schemas
```

---

### 4.17 Context Manager 增强（LLM 摘要注入）

```python
class ContextManager:
    """增强的上下文管理器"""

    def __init__(
        self,
        max_tokens: int = 128000,
        budget_manager: Optional[TokenBudgetManager] = None,
        summary_llm: Optional[LLMProvider] = None,  # 新增：用于摘要的 LLM
        summary_model: str = "gpt-3.5-turbo",  # 使用轻量模型做摘要
    ):
        self.max_tokens = max_tokens
        self.budget_manager = budget_manager or TokenBudgetManager()
        self.summary_llm = summary_llm
        self.summary_model = summary_model
        self._messages: list[Message] = []
        self._system_prompt: Optional[str] = None
        self._important_messages: set[int] = set()
        self._compact_history: list[dict] = []  # 压缩历史记录

    async def compact(self, force: bool = False) -> dict:
        """智能压缩上下文

        Returns:
            压缩报告，包含原始大小、压缩后大小、节省量等
        """
        import time

        # 分离重要和非重要消息
        important = [m for m in self._messages if id(m) in self._important_messages]
        others = [m for m in self._messages if id(m) not in self._important_messages]

        if not others:
            return {"compressed": False, "reason": "没有可压缩的消息"}

        # 计算原始 token 数
        original_tokens = sum(self._estimate_tokens(m) for m in others)

        # 保留最近的消息（不压缩）
        keep_recent = 5
        to_compress = others[:-keep_recent] if len(others) > keep_recent else []
        recent = others[-keep_recent:] if len(others) > keep_recent else []

        if not to_compress:
            return {"compressed": False, "reason": "消息数少于保留阈值"}

        # 生成摘要
        summary = await self._generate_summary_with_llm(to_compress)

        # 重组消息
        summary_msg = Message(
            role="system",
            content=f"[历史摘要 - {len(to_compress)} 条消息]\n{summary}"
        )
        self._messages = [summary_msg, *recent, *important]

        # 计算压缩后 token 数
        compressed_tokens = self._estimate_tokens(summary_msg) + sum(
            self._estimate_tokens(m) for m in recent + important
        )

        # 记录压缩历史
        compact_record = {
            "timestamp": time.time(),
            "messages_compressed": len(to_compress),
            "original_tokens": original_tokens,
            "compressed_tokens": compressed_tokens,
            "saved_tokens": original_tokens - compressed_tokens,
            "compression_ratio": 1 - (compressed_tokens / original_tokens) if original_tokens > 0 else 0,
        }
        self._compact_history.append(compact_record)

        return {
            "compressed": True,
            "original_tokens": original_tokens,
            "compressed_tokens": compressed_tokens,
            "saved_tokens": original_tokens - compressed_tokens,
            "compression_ratio": compact_record["compression_ratio"],
        }

    async def _generate_summary_with_llm(self, messages: list[Message]) -> str:
        """使用 LLM 生成高质量摘要"""

        if not self.summary_llm:
            # 降级为简单摘要
            return self._generate_simple_summary(messages)

        # 构建摘要提示
        conversation_text = "\n".join(
            f"{m.role}: {m.content[:500]}"
            for m in messages
        )

        prompt = f"""请总结以下对话的关键信息：

{conversation_text}

要求：
1. 保留重要的决策和结论
2. 保留关键的工具调用及其结果
3. 保留用户的原始需求
4. 忽略闲聊和无关内容
5. 简洁明了，不超过 500 字"""

        try:
            response = await self.summary_llm.chat(
                messages=[Message(role="user", content=prompt)],
                model=self.summary_model,
                max_tokens=500,
            )
            return response.content
        except Exception:
            return self._generate_simple_summary(messages)

    def _generate_simple_summary(self, messages: list[Message]) -> str:
        """简单摘要（降级方案）"""
        tool_calls = []
        key_decisions = []

        for m in messages:
            if m.tool_calls:
                for tc in m.tool_calls:
                    tool_calls.append(f"- {tc.function.name}")
            if m.role == "assistant" and len(m.content) > 50:
                key_decisions.append(m.content[:100] + "...")

        summary_parts = []
        if tool_calls:
            summary_parts.append("工具调用：\n" + "\n".join(tool_calls[:10]))
        if key_decisions:
            summary_parts.append("关键输出：\n" + "\n".join(key_decisions[:5]))

        return "\n\n".join(summary_parts) or f"压缩了 {len(messages)} 条消息"

    def get_compact_history(self) -> list[dict]:
        """获取压缩历史记录"""
        return self._compact_history.copy()
```

---

## 五、设计模式参考

### 5.1 P0 核心模式

| 模式 | 来源 | 说明 |
|------|------|------|
| **ACI 工具设计** | SWE-Agent | 为 Agent 专门设计工具，非暴露系统 API |
| **渐进式上下文** | AutoCodeRover + Aider | AST 搜索 + Repository Map |
| **自我修正循环** | Cline + SWE-Agent | 执行→检查→修正，最多 3 次 |

### 5.2 P1 重要模式

| 模式 | 来源 | 说明 |
|------|------|------|
| **YAML 配置驱动** | SWE-Agent + Continue | 一个 YAML 定义完整 Agent |
| **沙箱执行** | OpenHands | Docker 隔离执行 |
| **Human-in-the-Loop** | Cline | 渐进式自主性控制 |

### 5.3 P2 增强模式

| 模式 | 来源 | 说明 |
|------|------|------|
| **检查点与回滚** | Cline | 每步自动快照，支持回滚 |
| **MCP 工具扩展** | Cline | 标准化工具生态 |
| **观测性** | SWE-Agent | 轨迹记录、指标收集 |

### 5.4 P3 高级模式

| 模式 | 来源 | 说明 |
|------|------|------|
| **多 Agent 协作** | MetaGPT | 角色化 + SOP 驱动 |
| **消息中心** | AgentScope | MsgHub 解耦通信 |
| **工具自生成** | Cline | Agent 动态创建工具 |

---

## 六、项目结构

```
continuum/
├── src/continuum/
│   ├── __init__.py
│   ├── core/
│   │   ├── harness.py           # Harness 主类
│   │   ├── agent.py             # Agent 类
│   │   └── config.py            # 配置管理
│   ├── llm/
│   │   ├── base.py              # LLMProvider 基类
│   │   ├── streaming.py         # 流式输出支持
│   │   ├── retry.py             # 重试逻辑
│   │   └── providers/
│   │       ├── openai.py
│   │       ├── anthropic.py
│   │       ├── dashscope.py
│   │       └── ...
│   ├── context/
│   │   ├── manager.py           # ContextManager（含 LLM 摘要）
│   │   ├── budget.py            # Token 预算
│   │   └── compact.py           # 自动压缩
│   ├── memory/
│   │   ├── project.py           # ProjectMemory（EGG.md）
│   │   ├── auto.py              # AutoMemory（自动学习）
│   │   └── storage.py           # 记忆存储后端
│   ├── hooks/
│   │   ├── system.py            # HookSystem
│   │   ├── base.py              # Hook 基类
│   │   └── builtin/
│   │       ├── logging.py       # 日志钩子
│   │       ├── metrics.py       # 指标钩子
│   │       ├── permission.py    # 权限钩子
│   │       └── caching.py       # 缓存钩子
│   ├── correction/
│   │   ├── loop.py              # SelfCorrectionLoop
│   │   ├── verifier.py          # 结果验证器
│   │   └── strategy.py          # 修正策略生成
│   ├── tools/
│   │   ├── registry.py          # ToolRegistry（含 MCP 集成）
│   │   ├── validation.py        # 输入验证
│   │   └── builtin/
│   │       ├── file_ops.py
│   │       ├── shell.py
│   │       └── search.py
│   ├── workflow/
│   │   ├── graph.py             # StateGraph
│   │   ├── checkpoint.py        # 检查点
│   │   └── node.py              # 节点定义
│   ├── runtime/
│   │   ├── loop.py              # Agent 执行循环
│   │   ├── streaming.py         # 流式 Agent Runtime
│   │   └── planner.py           # 任务规划
│   ├── mcp/
│   │   ├── client.py            # MCPClient
│   │   ├── connection.py        # MCPServerConnection
│   │   └── integration.py       # Tool Registry 集成
│   ├── observability/
│   │   ├── tracer.py            # 简单追踪
│   │   ├── metrics.py           # 简单指标
│   │   └── stream_handler.py    # 流式处理追踪
│   └── error/
│       ├── classifier.py        # 错误分类
│       └── retry.py             # 重试执行
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
├── examples/
│   ├── basic_agent.py
│   ├── workflow_example.py
│   ├── mcp_example.py
│   └── streaming_example.py
├── docs/
│   ├── SPEC.md                  # 本文档
│   ├── MEMORY_GUIDE.md          # 记忆系统使用指南
│   ├── HOOKS_GUIDE.md           # 钩子系统使用指南
│   └── MCP_GUIDE.md             # MCP 集成指南
├── pyproject.toml
├── README.md
├── EGG.md                       # 项目记忆示例
└── continuum.yaml                     # 默认配置
```

---

## 七、开发计划

### Phase 1: 核心稳定

- [ ] LLM Provider + OpenAI/Anthropic 适配器
- [ ] Error Classifier + Retry Executor
- [ ] Token Budget Manager
- [ ] Config System
- [ ] Context Manager + 预算集成

### Phase 2: Agent 运行时

- [ ] Agent Runtime (Tool Calling Loop)
- [ ] Tool Registry + Schema 生成
- [ ] Tool 输入验证 + 超时
- [ ] SimpleTracer + SimpleMetrics
- [ ] 内置工具 (文件、Shell、搜索)

### Phase 3: 核心扩展

- [ ] Memory System (ProjectMemory + AutoMemory)
- [ ] Hooks System (生命周期钩子)
- [ ] Self-Correction Loop (自动修正)
- [ ] 流式输出支持 (StreamingAgentRuntime)
- [ ] Context LLM 摘要注入

### Phase 4: 高级特性

- [ ] Agent Planner
- [ ] Workflow Engine (并行 + 检查点)
- [ ] MCP Client (工具扩展)
- [ ] 更多 LLM Provider (DashScope, ZhipuAI, DeepSeek)

### Phase 5: 集成与文档

- [ ] CLI 接口
- [ ] 文档和示例
- [ ] 测试覆盖
- [ ] 性能优化

---

## 八、使用示例

### 8.1 基础用法

```python
from continuum import Harness

# 创建 Harness
harness = Harness(config="continuum.yaml")

# 注册工具
@harness.tool(description="搜索文件")
async def search_files(pattern: str, max_results: int = 10) -> list[str]:
    import glob
    return glob.glob(pattern)[:max_results]

# 创建 Agent
agent = harness.create_agent(
    name="assistant",
    system_prompt="你是一个有用的助手",
    tools=["search_files"],
)

# 运行
response = await agent.run("帮我搜索 Python 文件")
print(response)
```

### 8.2 使用记忆系统

```python
from continuum import Harness, ProjectMemory, AutoMemory

# 创建 Harness（自动加载 EGG.md）
harness = Harness(config="continuum.yaml", project_root="/path/to/project")

# 项目记忆
project_memory = ProjectMemory("/path/to/project")
await project_memory.append("代码风格", "使用 snake_case 命名变量")

# 自动记忆
auto_memory = AutoMemory(harness.storage, "/path/to/project")
await auto_memory.learn("user_preference", "用户喜欢简洁的回答", category="preferences")

# 创建带记忆的 Agent
agent = harness.create_agent(
    name="assistant",
    memory=project_memory,  # 项目记忆
    auto_memory=auto_memory,  # 自动记忆
)

# Agent 运行时会自动加载和更新记忆
response = await agent.run("帮我重构这个函数")
```

### 8.3 使用钩子系统

```python
from continuum import Harness, HookSystem, LoggingHook, MetricsHook, PermissionHook

# 创建钩子系统
hooks = HookSystem()

# 添加日志钩子
hooks.register(LoggingHook(logger))

# 添加指标钩子
hooks.register(MetricsHook(metrics))

# 添加权限钩子（禁止危险工具）
hooks.register(PermissionHook(
    blocked_tools=["run_command", "delete_file"]
))

# 创建带钩子的 Agent
agent = harness.create_agent(
    name="safe_assistant",
    hooks=hooks,
)

# 工具执行前会自动触发钩子
response = await agent.run("删除所有测试文件")  # 被 PermissionHook 阻止
```

### 8.4 使用自我修正

```python
from continuum import Harness, SelfCorrectionLoop

# 创建 Harness
harness = Harness(config="continuum.yaml")

# 创建 Agent
agent = harness.create_agent(
    name="code_assistant",
    tools=["read_file", "write_file", "run_tests"],
)

# 创建修正循环
correction_loop = SelfCorrectionLoop(
    llm=harness.llm,
    model="gpt-4",
    max_corrections=3,
)

# 运行带修正的任务
result = await correction_loop.run_with_correction(
    agent=agent,
    task="修复 bug.py 中的错误",
    context=agent.context,
)

print(result)
```

### 8.5 使用流式输出

```python
from continuum import Harness, StreamingAgentRuntime

# 创建 Harness
harness = Harness(config="continuum.yaml")

# 创建 Agent
base_agent = harness.create_agent(
    name="streaming_assistant",
)

# 创建流式 Runtime
streaming_agent = StreamingAgentRuntime(base_agent)

# 流式运行
async for chunk in streaming_agent.run_streaming("写一个 Python 类"):
    if chunk.content:
        print(chunk.content, end="", flush=True)

    if chunk.is_final and chunk.tool_calls:
        print("\n[调用工具]")
        for tc in chunk.tool_calls:
            print(f"  - {tc['function']['name']}")
```

### 8.6 使用 MCP 工具

```python
from continuum import Harness, MCPClient

# 创建 MCP 客户端
mcp = MCPClient()

# 连接 filesystem MCP
await mcp.connect(
    "filesystem",
    "uvx",
    ["mcp-server-filesystem", "/home/user/project"],
)

# 连接 github MCP
await mcp.connect(
    "github",
    "uvx",
    ["mcp-server-github"],
    env={"GITHUB_TOKEN": "ghp_xxx"},
)

# 创建带 MCP 工具的 Harness
harness = Harness(config="continuum.yaml", mcp_client=mcp)

# Agent 可以调用 MCP 工具
agent = harness.create_agent(
    name="assistant",
    tools=["read_file", "search_files", "github_search_code"],  # 混合本地和 MCP 工具
)

response = await agent.run("搜索 GitHub 上关于 AI Agent 的代码")
```

### 8.7 工作流用法

```python
from continuum import StateGraph

# 定义工作流
workflow = StateGraph()
workflow.add_node("search", search_node)
workflow.add_node("analyze", analyze_node)
workflow.add_edge("search", "analyze")
workflow.add_edge("analyze", "__end__")
workflow.set_entry_point("search")

# 编译并执行
app = workflow.compile(checkpoint_dir=".continuum/checkpoints")
result = await app.invoke({"query": "AI Agent"})
```

### 8.8 多模型切换

```python
# 环境变量
# OPENAI_API_KEY=sk-xxx
# ANTHROPIC_API_KEY=sk-ant-xxx

# 创建不同模型的 Agent
agent_openai = harness.create_agent(
    name="openai_agent",
    llm_provider="openai",
    llm_model="gpt-4",
)

agent_claude = harness.create_agent(
    name="claude_agent",
    llm_provider="anthropic",
    llm_model="claude-3-opus",
)
```

### 8.9 完整示例：带所有特性的 Agent

```python
from continuum import (
    Harness,
    ProjectMemory,
    AutoMemory,
    HookSystem,
    LoggingHook,
    MetricsHook,
    PermissionHook,
    SelfCorrectionLoop,
    StreamingAgentRuntime,
    MCPClient,
)

# 1. 创建 MCP 客户端
mcp = MCPClient()
await mcp.connect("filesystem", "uvx", ["mcp-server-filesystem", "."])

# 2. 创建 Harness
harness = Harness(
    config="continuum.yaml",
    project_root=".",
    mcp_client=mcp,
)

# 3. 创建记忆系统
project_memory = ProjectMemory(".")
await project_memory.append("项目信息", "这是一个 Python Web 项目")

auto_memory = AutoMemory(harness.storage, ".")

# 4. 创建钩子系统
hooks = HookSystem()
hooks.register(LoggingHook(harness.logger))
hooks.register(MetricsHook(harness.metrics))
hooks.register(PermissionHook(blocked_tools=["delete_file"]))

# 5. 创建基础 Agent
base_agent = harness.create_agent(
    name="full_featured_agent",
    system_prompt="你是一个智能代码助手",
    memory=project_memory,
    auto_memory=auto_memory,
    hooks=hooks,
    tools=["read_file", "write_file", "run_tests", "search_code"],
)

# 6. 添加流式支持
streaming_agent = StreamingAgentRuntime(base_agent)

# 7. 添加自我修正
correction_loop = SelfCorrectionLoop(
    llm=harness.llm,
    model="gpt-4",
    max_corrections=3,
)

# 8. 运行完整流程
async for chunk in correction_loop.run_streaming_with_correction(
    agent=streaming_agent,
    task="重构 auth.py 使其更安全",
):
    print(chunk.content, end="")
```

---

## 九、测试策略

### 9.1 单元测试

每个模块独立测试，Mock 外部依赖。

```python
# tests/unit/test_llm_provider.py

import pytest
from unittest.mock import AsyncMock, patch
from continuum.llm.providers.openai import OpenAIProvider
from continuum.llm.messages import Message

@pytest.fixture
def mock_httpx():
    with patch("httpx.AsyncClient") as mock:
        mock.return_value.post = AsyncMock(return_value=AsyncMock(
            json=lambda: {
                "choices": [{"message": {"content": "test"}, "finish_reason": "stop"}]
            },
            raise_for_status=lambda: None,
        ))
        yield mock

async def test_openai_chat(mock_httpx):
    provider = OpenAIProvider("test-key")
    response = await provider.chat(
        messages=[Message(role="user", content="hello")],
        model="gpt-4",
    )
    assert response.content == "test"
    assert response.finish_reason == "stop"

# tests/unit/test_context_manager.py

async def test_budget_check():
    from continuum.context import ContextManager, TokenBudgetManager

    budget = TokenBudgetManager()
    context = ContextManager(budget_manager=budget)

    # 添加大量消息触发压缩检查
    for i in range(100):
        await context.add_message(Message(
            role="user",
            content="x" * 10000,
        ))

    # 验证压缩被触发
    assert len(context._messages) < 100

# tests/unit/test_hooks.py

async def test_permission_hook():
    from continuum.hooks import HookSystem, PermissionHook, HookContext, HookType

    hooks = HookSystem()
    hooks.register(PermissionHook(blocked_tools=["delete_file"]))

    context = HookContext(
        hook_type=HookType.BEFORE_TOOL,
        agent_name="test",
        tool_name="delete_file",
        args={"path": "/test"},
    )

    result = await hooks.trigger(context)
    assert result is None  # 被阻止
```

### 9.2 集成测试

测试模块间协作。

```python
# tests/integration/test_agent_flow.py

async def test_full_agent_loop():
    from continuum import Harness

    harness = Harness()

    @harness.tool
    async def echo(text: str) -> str:
        return text

    agent = harness.create_agent(
        name="test",
        tools=["echo"],
    )

    # 使用 Mock LLM
    harness.llm.chat = AsyncMock(return_value=LLMResponse(
        content=None,
        tool_calls=[ToolCall(
            id="1",
            function=FunctionCall(name="echo", arguments='{"text": "hello"}'),
        )],
        finish_reason="tool_calls",
    ))

    result = await agent.run("test")
    assert "hello" in result

# tests/integration/test_memory_integration.py

async def test_memory_persistence():
    from continuum.memory import ProjectMemory, AutoMemory

    project_memory = ProjectMemory("/tmp/test_project")

    # 写入记忆
    await project_memory.append("test_section", "test content")

    # 读取记忆
    content = await project_memory.get_section("test_section")
    assert content == "test content"

    # 清理
    import os
    os.remove("/tmp/test_project/EGG.md")
```

### 9.3 E2E 测试

使用真实 API 测试关键路径。

```python
# tests/e2e/test_real_llm.py

import pytest
import os

@pytest.mark.skipif(not os.environ.get("OPENAI_API_KEY"), reason="需要真实 API Key")
async def test_real_openai_chat():
    from continuum import Harness

    harness = Harness(config="continuum.yaml")
    agent = harness.create_agent(name="test")

    response = await agent.run("1+1等于几？只回答数字")
    assert "2" in response

@pytest.mark.skipif(not os.environ.get("ANTHROPIC_API_KEY"), reason="需要真实 API Key")
async def test_real_claude_chat():
    from continuum.llm.providers.anthropic import AnthropicProvider
    from continuum.llm.messages import Message

    provider = AnthropicProvider(os.environ["ANTHROPIC_API_KEY"])
    response = await provider.chat(
        messages=[Message(role="user", content="hello")],
        model="claude-3-sonnet-20240229",
    )
    assert response.content
```

### 9.4 测试覆盖率目标

| 模块 | 目标覆盖率 | 说明 |
|------|-----------|------|
| LLM Provider | 90% | 核心模块，必须高覆盖 |
| Context Manager | 85% | 包含压缩逻辑 |
| Tool System | 85% | 包含验证和超时 |
| Memory System | 80% | 持久化逻辑 |
| Hooks System | 80% | 生命周期管理 |
| Error Handler | 90% | 重试逻辑 |
| Workflow Engine | 75% | 状态图复杂 |
| MCP Client | 70% | 依赖外部服务 |

---

## 十、术语表

| 术语 | 定义 |
|------|------|
| **Harness** | AI Agent 的运行时环境和管理框架 |
| **Agent** | 具有自主决策能力的 AI 实体 |
| **Tool** | Agent 可调用的功能函数 |
| **Workflow** | 由多个节点和边组成的工作流程 |
| **Checkpoint** | 工作流执行状态的快照 |
| **ACI** | Agent-Computer Interface，为 Agent 设计的工具接口 |
| **Repository Map** | 代码库的关键符号地图 |
| **Auto Compact** | 接近上下文限制时自动压缩 |
| **ProjectMemory** | 项目级别的持久化记忆（EGG.md） |
| **AutoMemory** | Agent 自动学习的跨会话记忆 |
| **Hook** | 工具执行生命周期的扩展点 |
| **Self-Correction** | Agent 执行失败后的自动修正机制 |
| **Streaming** | LLM 响应的实时流式输出 |
| **MCP** | Model Context Protocol，标准化工具扩展协议 |
| **Token Budget** | 上下文的 token 预算管理 |
| **Self-Correction Loop** | 自我修正循环 |

---

## 十一、参考资料

- [SWE-Agent](https://github.com/princeton-nlp/SWE-agent)
- [Aider](https://github.com/paul-gauthier/aider)
- [Claude Code](https://github.com/anthropics/claude-code)
- [OpenCode](https://github.com/opencode-ai/opencode)
- [Cline](https://github.com/cline/cline)
- [Continue](https://github.com/continuedev/continue)
- [AutoCodeRover](https://github.com/nus-apr/auto-code-rover)
- [OpenHands](https://github.com/All-Hands-AI/OpenHands)
- [MetaGPT](https://github.com/geekan/MetaGPT)
- [MCP Specification](https://modelcontextprotocol.io/)
- [LangGraph](https://github.com/langchain-ai/langgraph) - 状态图设计参考
- [Pydantic](https://github.com/pydantic/pydantic) - 数据验证参考

---

## 十二、版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.0 | 2026-05-08 | 初始版本 |
| v1.1 | 2026-05-08 | 补充 Memory System、Hooks System、Self-Correction、Streaming、MCP、测试策略 |

---

**文档状态**: v1.1 已完成
**下一步**: 开始 Phase 1 开发
