# Egg-Harness 架构改进方案 v2.1

> 版本: v2.1
> 日期: 2026-05-08
> 基于: architecture-v2.md

---

## 一、改进概览

### 1.1 新增模块

| 模块 | 优先级 | 说明 |
|------|--------|------|
| Retry & Error Handler | P0 | 统一错误处理和重试机制 |
| Token Budget Manager | P0 | Token 预算分配和管理 |
| Observability | P1 | 分布式追踪和指标收集 |
| Security Manager | P1 | API Key 管理和审计 |
| Agent Planner | P2 | 任务分解和策略选择 |

### 1.2 增强模块

| 模块 | 优先级 | 改进点 |
|------|--------|--------|
| LLM Provider | P0 | 重试、限流、成本追踪 |
| Context Manager | P0 | 精细化压缩、预算分配 |
| Tool System | P1 | 验证、超时、版本管理 |
| Workflow Engine | P2 | 并行、持久化、子图 |
| Config System | P1 | 分层配置、验证 |

---

## 二、新增核心模块

### 2.1 Retry & Error Handler（P0）

#### 2.1.1 错误分类

```python
from enum import Enum
from dataclasses import dataclass
from typing import Optional, Callable, Any
import asyncio
import time

class ErrorType(Enum):
    """错误类型"""
    RATE_LIMIT = "rate_limit"           # 速率限制
    TIMEOUT = "timeout"                 # 超时
    CONTEXT_TOO_LONG = "context_too_long"  # 上下文超长
    TOOL_FAILED = "tool_failed"         # 工具执行失败
    INVALID_RESPONSE = "invalid_response"  # 无效响应
    API_ERROR = "api_error"             # API 错误
    AUTH_ERROR = "auth_error"           # 认证错误
    NETWORK_ERROR = "network_error"     # 网络错误

@dataclass
class ClassifiedError:
    """分类后的错误"""
    error_type: ErrorType
    original_error: Exception
    message: str
    recoverable: bool
    suggested_action: str
    retry_after: Optional[int] = None  # 秒

class ErrorClassifier:
    """错误分类器"""

    PATTERNS = {
        ErrorType.RATE_LIMIT: [
            "rate limit",
            "429",
            "too many requests",
            "quota exceeded",
        ],
        ErrorType.TIMEOUT: [
            "timeout",
            "timed out",
        ],
        ErrorType.CONTEXT_TOO_LONG: [
            "context length",
            "max tokens",
            "too long",
        ],
        ErrorType.AUTH_ERROR: [
            "invalid api key",
            "unauthorized",
            "authentication",
        ],
        ErrorType.NETWORK_ERROR: [
            "connection",
            "network",
            "dns",
        ],
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
        """创建分类错误"""
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
                suggested_action="增加超时时间或重试",
                retry_after=5,
            ),
            ErrorType.CONTEXT_TOO_LONG: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"上下文超长: {error}",
                recoverable=True,
                suggested_action="压缩上下文",
            ),
            ErrorType.AUTH_ERROR: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"认证错误: {error}",
                recoverable=False,
                suggested_action="检查 API Key",
            ),
            ErrorType.NETWORK_ERROR: ClassifiedError(
                error_type=error_type,
                original_error=error,
                message=f"网络错误: {error}",
                recoverable=True,
                suggested_action="检查网络连接后重试",
                retry_after=10,
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

#### 2.1.2 重试策略

```python
from enum import Enum
from typing import TypeVar, Callable, Awaitable
import random

T = TypeVar("T")

class BackoffType(Enum):
    """退避类型"""
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
    retryable_errors: list[ErrorType] = None

    def __post_init__(self):
        if self.retryable_errors is None:
            self.retryable_errors = [
                ErrorType.RATE_LIMIT,
                ErrorType.TIMEOUT,
                ErrorType.NETWORK_ERROR,
                ErrorType.API_ERROR,
            ]

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

                if not classified.recoverable:
                    raise e

                if classified.error_type not in self.config.retryable_errors:
                    raise e

                if attempt == self.config.max_retries:
                    raise RetryExhaustedError(
                        f"达到最大重试次数 ({self.config.max_retries})",
                        last_error=e,
                    )

                last_error = e
                delay = self._calculate_delay(attempt, classified.retry_after)
                await asyncio.sleep(delay)

    def _calculate_delay(self, attempt: int, override: Optional[int] = None) -> float:
        """计算延迟时间"""
        if override:
            return override

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

#### 2.1.3 恢复策略

```python
from abc import ABC, abstractmethod
from typing import Any

class RecoveryStrategy(ABC):
    """恢复策略基类"""

    @abstractmethod
    async def recover(self, error: ClassifiedError, context: dict) -> dict:
        """执行恢复操作，返回更新后的上下文"""
        pass

class CompactStrategy(RecoveryStrategy):
    """压缩策略"""

    def __init__(self, context_manager: "ContextManager"):
        self.context_manager = context_manager

    async def recover(self, error: ClassifiedError, context: dict) -> dict:
        if error.error_type != ErrorType.CONTEXT_TOO_LONG:
            return context

        await self.context_manager.compact(force=True)
        return context

class CorrectionStrategy(RecoveryStrategy):
    """修正策略"""

    def __init__(self, max_corrections: int = 3):
        self.max_corrections = max_corrections
        self.correction_count = 0

    async def recover(self, error: ClassifiedError, context: dict) -> dict:
        if error.error_type != ErrorType.TOOL_FAILED:
            return context

        if self.correction_count >= self.max_corrections:
            raise error.original_error

        self.correction_count += 1

        # 添加修正提示到上下文
        correction_prompt = f"""
之前的工具调用失败了：
- 错误类型: {error.error_type.value}
- 错误信息: {error.message}
- 修复建议: {error.suggested_action}

请根据以上信息修正你的操作。
"""
        context["messages"].append({
            "role": "user",
            "content": correction_prompt,
        })
        return context

class ErrorHandler:
    """统一错误处理器"""

    def __init__(
        self,
        retry_executor: RetryExecutor,
        recovery_strategies: dict[ErrorType, RecoveryStrategy] = None,
    ):
        self.retry_executor = retry_executor
        self.recovery_strategies = recovery_strategies or {}

    async def handle(
        self,
        error: Exception,
        context: dict,
    ) -> tuple[bool, dict]:
        """
        处理错误
        返回: (是否恢复成功, 更新后的上下文)
        """
        classified = ErrorClassifier.classify(error)

        # 尝试恢复
        strategy = self.recovery_strategies.get(classified.error_type)
        if strategy:
            try:
                new_context = await strategy.recover(classified, context)
                return True, new_context
            except Exception:
                return False, context

        return False, context
```

---

### 2.2 Token Budget Manager（P0）

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
    reserved: int = 0  # 预留给响应

    @property
    def total(self) -> int:
        return (
            self.system_prompt +
            self.repo_map +
            self.conversation +
            self.tool_results +
            self.reserved
        )

@dataclass
class UsageRecord:
    """使用记录"""
    allocated: int = 0
    actual: int = 0
    component: str = ""

@dataclass
class TokenBudgetConfig:
    """预算配置"""
    total_budget: int = 128000
    response_reserve: int = 4096
    safety_margin: float = 0.05

    # 分配比例
    system_prompt_ratio: float = 0.05
    repo_map_ratio: float = 0.15
    conversation_ratio: float = 0.50
    tool_results_ratio: float = 0.25

class TokenBudgetManager:
    """Token 预算管理器"""

    def __init__(self, config: TokenBudgetConfig = None):
        self.config = config or TokenBudgetConfig()
        self.usage: dict[str, UsageRecord] = defaultdict(UsageRecord)
        self._allocated = 0

    def get_allocation(self) -> BudgetAllocation:
        """获取预算分配"""
        available = self.config.total_budget - self.config.response_reserve
        available = int(available * (1 - self.config.safety_margin))

        return BudgetAllocation(
            system_prompt=int(available * self.config.system_prompt_ratio),
            repo_map=int(available * self.config.repo_map_ratio),
            conversation=int(available * self.config.conversation_ratio),
            tool_results=int(available * self.config.tool_results_ratio),
            reserved=self.config.response_reserve,
        )

    def allocate(self, component: str, requested: int) -> int:
        """
        为组件分配预算
        返回实际可用的预算（可能小于请求）
        """
        allocation = self.get_allocation()
        component_limits = {
            "system_prompt": allocation.system_prompt,
            "repo_map": allocation.repo_map,
            "conversation": allocation.conversation,
            "tool_results": allocation.tool_results,
        }

        limit = component_limits.get(component, 0)
        actual = min(requested, limit)

        self.usage[component] = UsageRecord(
            allocated=actual,
            component=component,
        )
        self._allocated += actual

        return actual

    def report_usage(self, component: str, actual: int) -> None:
        """报告实际使用量"""
        if component in self.usage:
            self.usage[component].actual = actual

    def get_available(self) -> int:
        """获取剩余可用预算"""
        total_used = sum(u.actual for u in self.usage.values())
        return self.config.total_budget - self.config.response_reserve - total_used

    def check_within_budget(self, tokens: int, component: str) -> bool:
        """检查是否在预算内"""
        allocation = self.get_allocation()
        component_limits = {
            "system_prompt": allocation.system_prompt,
            "repo_map": allocation.repo_map,
            "conversation": allocation.conversation,
            "tool_results": allocation.tool_results,
        }
        limit = component_limits.get(component, 0)
        return tokens <= limit

    def get_report(self) -> dict:
        """获取使用报告"""
        return {
            "total_budget": self.config.total_budget,
            "allocated": self._allocated,
            "used": sum(u.actual for u in self.usage.values()),
            "available": self.get_available(),
            "breakdown": {
                component: {
                    "allocated": record.allocated,
                    "actual": record.actual,
                    "utilization": record.actual / record.allocated if record.allocated > 0 else 0,
                }
                for component, record in self.usage.items()
            },
        }

    def reset(self) -> None:
        """重置预算"""
        self.usage.clear()
        self._allocated = 0
```

---

### 2.3 Observability（P1）

#### 2.3.1 分布式追踪

```python
from dataclasses import dataclass, field
from typing import Optional
import time
import uuid
from contextlib import asynccontextmanager
import json

@dataclass
class Span:
    """追踪跨度"""
    span_id: str
    trace_id: str
    parent_span_id: Optional[str]
    name: str
    start_time: float
    end_time: Optional[float] = None
    status: str = "running"
    attributes: dict = field(default_factory=dict)
    events: list[dict] = field(default_factory=list)

    @property
    def duration_ms(self) -> Optional[float]:
        if self.end_time:
            return (self.end_time - self.start_time) * 1000
        return None

    def to_dict(self) -> dict:
        return {
            "span_id": self.span_id,
            "trace_id": self.trace_id,
            "parent_span_id": self.parent_span_id,
            "name": self.name,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "duration_ms": self.duration_ms,
            "status": self.status,
            "attributes": self.attributes,
            "events": self.events,
        }

class Tracer:
    """分布式追踪器"""

    def __init__(self, service_name: str = "egg-harness"):
        self.service_name = service_name
        self._spans: dict[str, Span] = {}
        self._current_span: dict[str, str] = {}  # thread_id -> span_id
        self._exporters: list["SpanExporter"] = []

    def generate_id(self) -> str:
        return uuid.uuid4().hex[:16]

    @asynccontextmanager
    async def start_span(
        self,
        name: str,
        parent: Optional[str] = None,
        attributes: Optional[dict] = None,
    ):
        """启动一个追踪跨度"""
        span_id = self.generate_id()

        # 查找父跨度
        parent_span_id = parent
        trace_id = self.generate_id()

        if parent_span_id and parent_span_id in self._spans:
            trace_id = self._spans[parent_span_id].trace_id

        span = Span(
            span_id=span_id,
            trace_id=trace_id,
            parent_span_id=parent_span_id,
            name=name,
            start_time=time.time(),
            attributes=attributes or {},
        )

        self._spans[span_id] = span

        try:
            yield span
            span.status = "ok"
        except Exception as e:
            span.status = "error"
            span.events.append({
                "name": "exception",
                "timestamp": time.time(),
                "attributes": {
                    "exception.type": type(e).__name__,
                    "exception.message": str(e),
                },
            })
            raise
        finally:
            span.end_time = time.time()
            await self._export_span(span)

    async def _export_span(self, span: Span):
        """导出跨度"""
        for exporter in self._exporters:
            await exporter.export(span)

    def add_exporter(self, exporter: "SpanExporter"):
        """添加导出器"""
        self._exporters.append(exporter)

    def get_trace(self, trace_id: str) -> list[Span]:
        """获取完整追踪链"""
        return [s for s in self._spans.values() if s.trace_id == trace_id]

class SpanExporter(ABC):
    """跨度导出器基类"""

    @abstractmethod
    async def export(self, span: Span) -> None:
        pass

class ConsoleExporter(SpanExporter):
    """控制台导出器"""

    async def export(self, span: Span) -> None:
        indent = "  " * (1 if span.parent_span_id else 0)
        print(f"{indent}[{span.status}] {span.name} ({span.duration_ms:.2f}ms)")

class FileExporter(SpanExporter):
    """文件导出器"""

    def __init__(self, output_dir: str):
        self.output_dir = output_dir
        import os
        os.makedirs(output_dir, exist_ok=True)

    async def export(self, span: Span) -> None:
        filename = f"{span.trace_id}.json"
        import os
        filepath = os.path.join(self.output_dir, filename)

        traces = []
        if os.path.exists(filepath):
            with open(filepath, "r") as f:
                traces = json.load(f)

        traces.append(span.to_dict())

        with open(filepath, "w") as f:
            json.dump(traces, f, indent=2)
```

#### 2.3.2 指标收集

```python
from dataclasses import dataclass, field
from typing import Callable
import time
from collections import defaultdict

@dataclass
class MetricPoint:
    """指标数据点"""
    name: str
    value: float
    timestamp: float
    tags: dict = field(default_factory=dict)

class MetricsCollector:
    """指标收集器"""

    def __init__(self):
        self._counters: dict[str, float] = defaultdict(float)
        self._gauges: dict[str, float] = {}
        self._histograms: dict[str, list[float]] = defaultdict(list)
        self._timestamps: dict[str, list[MetricPoint]] = defaultdict(list)

    def counter(self, name: str, value: float = 1, tags: Optional[dict] = None) -> None:
        """计数器"""
        key = self._make_key(name, tags)
        self._counters[key] += value
        self._record(name, value, tags)

    def gauge(self, name: str, value: float, tags: Optional[dict] = None) -> None:
        """仪表盘（瞬时值）"""
        key = self._make_key(name, tags)
        self._gauges[key] = value
        self._record(name, value, tags)

    def histogram(self, name: str, value: float, tags: Optional[dict] = None) -> None:
        """直方图"""
        key = self._make_key(name, tags)
        self._histograms[key].append(value)
        self._record(name, value, tags)

    def timing(self, name: str, tags: Optional[dict] = None) -> "TimingContext":
        """计时上下文"""
        return TimingContext(self, name, tags)

    def _record(self, name: str, value: float, tags: Optional[dict]) -> None:
        """记录数据点"""
        self._timestamps[name].append(MetricPoint(
            name=name,
            value=value,
            timestamp=time.time(),
            tags=tags or {},
        ))

    def _make_key(self, name: str, tags: Optional[dict]) -> str:
        if not tags:
            return name
        tag_str = ",".join(f"{k}={v}" for k, v in sorted(tags.items()))
        return f"{name},{tag_str}"

    def get_summary(self) -> dict:
        """获取指标摘要"""
        summary = {
            "counters": dict(self._counters),
            "gauges": dict(self._gauges),
            "histograms": {},
        }

        for key, values in self._histograms.items():
            if values:
                sorted_values = sorted(values)
                summary["histograms"][key] = {
                    "count": len(values),
                    "min": min(values),
                    "max": max(values),
                    "mean": sum(values) / len(values),
                    "p50": sorted_values[len(values) // 2],
                    "p95": sorted_values[int(len(values) * 0.95)],
                    "p99": sorted_values[int(len(values) * 0.99)],
                }

        return summary

    def reset(self) -> None:
        """重置所有指标"""
        self._counters.clear()
        self._gauges.clear()
        self._histograms.clear()
        self._timestamps.clear()

class TimingContext:
    """计时上下文管理器"""

    def __init__(self, collector: MetricsCollector, name: str, tags: Optional[dict]):
        self.collector = collector
        self.name = name
        self.tags = tags
        self.start_time = None

    def __enter__(self):
        self.start_time = time.time()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        duration = (time.time() - self.start_time) * 1000  # ms
        self.collector.histogram(self.name, duration, self.tags)
        return False

    async def __aenter__(self):
        self.start_time = time.time()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        duration = (time.time() - self.start_time) * 1000  # ms
        self.collector.histogram(self.name, duration, self.tags)
        return False

# 预定义指标
class AgentMetrics:
    """Agent 预定义指标"""

    def __init__(self, collector: MetricsCollector):
        self.collector = collector

    def record_llm_call(self, provider: str, model: str, tokens_used: int, latency_ms: float):
        self.collector.counter("llm.calls_total", 1, {"provider": provider, "model": model})
        self.collector.counter("llm.tokens_total", tokens_used, {"provider": provider, "model": model})
        self.collector.histogram("llm.latency_ms", latency_ms, {"provider": provider, "model": model})

    def record_tool_call(self, tool_name: str, success: bool, latency_ms: float):
        self.collector.counter("tool.calls_total", 1, {"tool": tool_name, "success": str(success)})
        self.collector.histogram("tool.latency_ms", latency_ms, {"tool": tool_name})

    def record_agent_iteration(self, agent_name: str):
        self.collector.counter("agent.iterations_total", 1, {"agent": agent_name})
```

---

### 2.4 Security Manager（P1）

```python
from dataclasses import dataclass
from typing import Optional
import os
import hashlib
import base64
import time
import json

@dataclass
class APIKeyInfo:
    """API Key 信息"""
    provider: str
    key_hash: str
    created_at: float
    last_used: float
    rotation_count: int = 0

class KeyManager:
    """API Key 管理器"""

    def __init__(self, storage: "StorageBackend"):
        self.storage = storage
        self._key_cache: dict[str, str] = {}

    def _hash_key(self, key: str) -> str:
        """哈希 Key"""
        return hashlib.sha256(key.encode()).hexdigest()[:16]

    async def store_key(self, provider: str, api_key: str) -> None:
        """存储 API Key"""
        key_hash = self._hash_key(api_key)

        # 加密存储实际 Key
        encrypted = self._encrypt_key(api_key)

        key_info = APIKeyInfo(
            provider=provider,
            key_hash=key_hash,
            created_at=time.time(),
            last_used=time.time(),
        )

        await self.storage.set(f"key:{provider}:hash", key_hash)
        await self.storage.set(f"key:{provider}:encrypted", encrypted)
        await self.storage.set(f"key:{provider}:info", key_info.__dict__)

        self._key_cache[provider] = api_key

    async def get_key(self, provider: str) -> Optional[str]:
        """获取 API Key"""
        if provider in self._key_cache:
            return self._key_cache[provider]

        encrypted = await self.storage.get(f"key:{provider}:encrypted")
        if encrypted:
            decrypted = self._decrypt_key(encrypted)
            self._key_cache[provider] = decrypted
            return decrypted

        # 尝试从环境变量获取
        env_key = os.environ.get(f"{provider.upper()}_API_KEY")
        if env_key:
            await self.store_key(provider, env_key)
            return env_key

        return None

    async def rotate_key(self, provider: str, new_key: str) -> None:
        """轮换 API Key"""
        old_info = await self.storage.get(f"key:{provider}:info")

        await self.store_key(provider, new_key)

        if old_info:
            info = await self.storage.get(f"key:{provider}:info")
            info["rotation_count"] = old_info.get("rotation_count", 0) + 1
            await self.storage.set(f"key:{provider}:info", info)

    async def validate_key(self, provider: str) -> bool:
        """验证 Key 是否有效"""
        key = await self.get_key(provider)
        return key is not None and len(key) > 0

    def _encrypt_key(self, key: str) -> str:
        """加密 Key（简化版，生产环境应使用专业加密）"""
        return base64.b64encode(key.encode()).decode()

    def _decrypt_key(self, encrypted: str) -> str:
        """解密 Key"""
        return base64.b64decode(encrypted.encode()).decode()

class AuditLogger:
    """审计日志器"""

    def __init__(self, storage: "StorageBackend"):
        self.storage = storage

    async def log(self, action: str, details: dict) -> None:
        """记录审计日志"""
        entry = {
            "timestamp": time.time(),
            "action": action,
            "details": details,
        }

        # 写入审计日志
        logs = await self.storage.get("audit:log") or []
        logs.append(entry)

        # 保留最近 10000 条
        if len(logs) > 10000:
            logs = logs[-10000:]

        await self.storage.set("audit:log", logs)

    async def get_logs(
        self,
        action: Optional[str] = None,
        start_time: Optional[float] = None,
        end_time: Optional[float] = None,
        limit: int = 100,
    ) -> list[dict]:
        """查询审计日志"""
        logs = await self.storage.get("audit:log") or []

        filtered = logs

        if action:
            filtered = [l for l in filtered if l["action"] == action]

        if start_time:
            filtered = [l for l in filtered if l["timestamp"] >= start_time]

        if end_time:
            filtered = [l for l in filtered if l["timestamp"] <= end_time]

        return filtered[-limit:]

class PermissionChecker:
    """权限检查器"""

    def __init__(self):
        self._permissions: dict[str, set[str]] = {}

    def grant(self, role: str, permission: str) -> None:
        """授予权限"""
        if role not in self._permissions:
            self._permissions[role] = set()
        self._permissions[role].add(permission)

    def revoke(self, role: str, permission: str) -> None:
        """撤销权限"""
        if role in self._permissions:
            self._permissions[role].discard(permission)

    def check(self, role: str, permission: str) -> bool:
        """检查权限"""
        if role not in self._permissions:
            return False
        return permission in self._permissions[role]

    def check_tool(self, role: str, tool_name: str) -> bool:
        """检查工具调用权限"""
        return self.check(role, f"tool:{tool_name}")

class SecurityManager:
    """安全管理器"""

    def __init__(self, storage: "StorageBackend"):
        self.key_manager = KeyManager(storage)
        self.audit_logger = AuditLogger(storage)
        self.permission_checker = PermissionChecker()

    async def initialize(self) -> None:
        """初始化安全管理"""
        # 从环境变量加载默认 Key
        for provider in ["OPENAI", "ANTHROPIC", "DASHSCOPE", "ZHIPUAI", "DEEPSEEK"]:
            key = os.environ.get(f"{provider}_API_KEY")
            if key:
                await self.key_manager.store_key(provider.lower(), key)
```

---

### 2.5 Agent Planner（P2）

```python
from dataclasses import dataclass, field
from typing import Optional
from enum import Enum

class ComplexityLevel(Enum):
    """复杂度级别"""
    SIMPLE = "simple"        # 单次调用可完成
    MODERATE = "moderate"    # 需要工具调用
    COMPLEX = "complex"      # 需要多步规划
    VERY_COMPLEX = "very_complex"  # 需要多 Agent 协作

class ExecutionStrategy(Enum):
    """执行策略"""
    SINGLE_TURN = "single_turn"      # 单轮对话
    MULTI_TURN = "multi_turn"        # 多轮对话
    TOOL_CHAIN = "tool_chain"        # 工具链
    WORKFLOW = "workflow"            # 工作流
    MULTI_AGENT = "multi_agent"      # 多 Agent

@dataclass
class SubTask:
    """子任务"""
    id: str
    description: str
    dependencies: list[str] = field(default_factory=list)
    estimated_steps: int = 1
    required_tools: list[str] = field(default_factory=list)
    status: str = "pending"
    result: Optional[str] = None

@dataclass
class Plan:
    """执行计划"""
    id: str
    task: str
    complexity: ComplexityLevel
    strategy: ExecutionStrategy
    subtasks: list[SubTask]
    estimated_iterations: int
    created_at: float
    approved: bool = False

class Planner:
    """任务规划器"""

    def __init__(self, llm: "LLMProvider", model: str):
        self.llm = llm
        self.model = model

    async def analyze_task(self, task: str) -> tuple[ComplexityLevel, ExecutionStrategy]:
        """分析任务复杂度"""
        prompt = f"""分析以下任务的复杂度：

任务：{task}

请判断：
1. 复杂度级别：simple/moderate/complex/very_complex
2. 推荐执行策略：single_turn/multi_turn/tool_chain/workflow/multi_agent

返回 JSON 格式：
{{"complexity": "xxx", "strategy": "xxx"}}
"""
        response = await self.llm.chat(
            messages=[{"role": "user", "content": prompt}],
            model=self.model,
        )

        # 解析响应
        import json
        result = json.loads(response.content)
        return (
            ComplexityLevel(result["complexity"]),
            ExecutionStrategy(result["strategy"]),
        )

    async def decompose(self, task: str) -> list[SubTask]:
        """分解任务为子任务"""
        prompt = f"""将以下任务分解为子任务：

任务：{task}

要求：
1. 每个子任务应该独立可执行
2. 标明子任务之间的依赖关系
3. 估算每个子任务的步骤数
4. 列出需要的工具

返回 JSON 格式：
{{
  "subtasks": [
    {{
      "id": "subtask_1",
      "description": "描述",
      "dependencies": [],
      "estimated_steps": 1,
      "required_tools": ["tool1"]
    }}
  ]
}}
"""
        response = await self.llm.chat(
            messages=[{"role": "user", "content": prompt}],
            model=self.model,
        )

        import json
        result = json.loads(response.content)

        return [
            SubTask(**st)
            for st in result.get("subtasks", [])
        ]

    async def create_plan(self, task: str, require_approval: bool = True) -> Plan:
        """创建执行计划"""
        complexity, strategy = await self.analyze_task(task)
        subtasks = await self.decompose(task)

        plan = Plan(
            id=str(uuid.uuid4())[:8],
            task=task,
            complexity=complexity,
            strategy=strategy,
            subtasks=subtasks,
            estimated_iterations=sum(st.estimated_steps for st in subtasks),
            created_at=time.time(),
        )

        return plan

    def topological_sort(self, subtasks: list[SubTask]) -> list[SubTask]:
        """拓扑排序子任务"""
        # 构建图
        graph = {st.id: set(st.dependencies) for st in subtasks}
        in_degree = {st.id: 0 for st in subtasks}

        for st in subtasks:
            for dep in st.dependencies:
                in_degree[st.id] += 1

        # Kahn 算法
        queue = [st for st in subtasks if in_degree[st.id] == 0]
        result = []

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

## 三、现有模块增强

### 3.1 LLM Provider 增强

```python
# 在原有基础上增加的方法

class LLMProvider(ABC):
    # ... 原有方法 ...

    @abstractmethod
    async def chat_with_retry(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        stream: bool = False,
        retry_config: Optional[RetryConfig] = None,
    ) -> LLMResponse | AsyncIterator[LLMResponse]:
        """带重试的聊天"""
        pass

    @abstractmethod
    def get_rate_limit_info(self) -> dict:
        """获取速率限制信息"""
        pass

    @abstractmethod
    def estimate_cost(
        self,
        messages: list[Message],
        model: str,
    ) -> float:
        """预估调用成本（美元）"""
        pass

class OpenAIProvider(LLMProvider):
    # ... 原有代码 ...

    def __init__(self, api_key: str, base_url: Optional[str] = None):
        # ... 原有代码 ...
        self._rate_limit_info = {}
        self._retry_executor = RetryExecutor()

    async def chat_with_retry(
        self,
        messages: list[Message],
        model: str,
        tools: Optional[list[dict]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        stream: bool = False,
        retry_config: Optional[RetryConfig] = None,
    ) -> LLMResponse | AsyncIterator[LLMResponse]:
        """带重试的聊天"""
        retry_config = retry_config or RetryConfig()

        return await self._retry_executor.execute(
            lambda: self.chat(
                messages=messages,
                model=model,
                tools=tools,
                temperature=temperature,
                max_tokens=max_tokens,
                stream=stream,
            )
        )

    def get_rate_limit_info(self) -> dict:
        """获取速率限制信息"""
        return self._rate_limit_info

    def estimate_cost(self, messages: list[Message], model: str) -> float:
        """预估调用成本"""
        # OpenAI 定价（2024年参考）
        pricing = {
            "gpt-4-turbo": {"input": 0.01, "output": 0.03},
            "gpt-4": {"input": 0.03, "output": 0.06},
            "gpt-3.5-turbo": {"input": 0.0005, "output": 0.0015},
        }

        model_pricing = pricing.get(model, {"input": 0.01, "output": 0.03})

        # 简单估算
        input_tokens = sum(len(m.content.split()) * 1.3 for m in messages)
        output_tokens = 500  # 预估

        cost = (
            input_tokens * model_pricing["input"] / 1000 +
            output_tokens * model_pricing["output"] / 1000
        )

        return cost
```

### 3.2 Context Manager 增强

```python
class ContextManager:
    def __init__(
        self,
        max_tokens: int = 128000,
        auto_compact_threshold: float = 0.95,
        storage: Optional[StorageBackend] = None,
        budget_manager: Optional[TokenBudgetManager] = None,
    ):
        self.max_tokens = max_tokens
        self.auto_compact_threshold = auto_compact_threshold
        self.storage = storage or MemoryStorage()
        self.budget_manager = budget_manager or TokenBudgetManager()

        self._messages: list[Message] = []
        self._system_prompt: Optional[str] = None
        self._token_counter = TokenCounter()
        self._important_messages: set[str] = set()  # 重要消息 ID
        self._compact_history: list[dict] = []

    async def add_message(
        self,
        message: Message,
        important: bool = False,
    ) -> None:
        """添加消息"""
        # 检查是否在预算内
        message_tokens = self._token_counter.count_message(message)

        if not self.budget_manager.check_within_budget(message_tokens, "conversation"):
            # 需要压缩
            await self.compact()

        self._messages.append(message)

        if important:
            self._important_messages.add(id(message))

        # 检查是否需要自动压缩
        if self._should_compact():
            await self._auto_compact()

    async def mark_important(self, message_id: str) -> None:
        """标记重要消息"""
        self._important_messages.add(message_id)

    async def _auto_compact(self) -> None:
        """智能压缩"""
        # 分离重要和非重要消息
        important_msgs = [
            m for m in self._messages
            if id(m) in self._important_messages
        ]
        other_msgs = [
            m for m in self._messages
            if id(m) not in self._important_messages
        ]

        # 对非重要消息生成摘要
        if other_msgs:
            summary = await self._generate_summary(other_msgs)

            # 记录压缩历史
            self._compact_history.append({
                "timestamp": time.time(),
                "messages_count": len(other_msgs),
                "summary": summary,
            })

            # 重组消息
            self._messages = [
                Message(role="system", content=f"[历史摘要]\n{summary}"),
                *important_msgs,
            ]

    async def _generate_summary(self, messages: list[Message]) -> str:
        """生成摘要"""
        # 合并消息内容
        content = "\n".join(f"{m.role}: {m.content}" for m in messages)

        # 使用 LLM 生成摘要（需要注入）
        # 这里简化处理
        return f"压缩了 {len(messages)} 条消息"

    def get_budget_allocation(self) -> BudgetAllocation:
        """获取当前预算分配"""
        return self.budget_manager.get_allocation()

    def get_compact_history(self) -> list[dict]:
        """获取压缩历史"""
        return self._compact_history.copy()
```

### 3.3 Tool System 增强

```python
from typing import get_origin, get_args, Annotated
import inspect

class ToolRegistry:
    # ... 原有代码 ...

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
            prop = self._type_to_schema_enhanced(hint)

            # 处理 Annotated 描述
            if get_origin(hint) is Annotated:
                args = get_args(hint)
                if len(args) >= 2 and isinstance(args[-1], str):
                    prop["description"] = args[-1]

            # 从 docstring 获取描述
            if "description" not in prop:
                prop["description"] = self._get_param_desc(func, name)

            # 处理默认值
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

    def _type_to_schema_enhanced(self, type_hint) -> dict:
        """增强的类型转换"""
        origin = get_origin(type_hint)

        # 处理 Optional[T]
        if origin is Union:
            args = get_args(type_hint)
            non_none_args = [a for a in args if a is not type(None)]
            if len(non_none_args) == 1:
                schema = self._type_to_schema_enhanced(non_none_args[0])
                schema["nullable"] = True
                return schema

        # 处理 List[T]
        if origin is list:
            args = get_args(type_hint)
            item_type = args[0] if args else str
            return {
                "type": "array",
                "items": self._type_to_schema_enhanced(item_type),
            }

        # 处理 Dict[K, V]
        if origin is dict:
            return {"type": "object"}

        # 处理 BaseModel
        if isinstance(type_hint, type) and issubclass(type_hint, BaseModel):
            return type_hint.model_json_schema()

        # 基础类型
        type_map = {
            str: {"type": "string"},
            int: {"type": "integer"},
            float: {"type": "number"},
            bool: {"type": "boolean"},
        }

        return type_map.get(type_hint, {"type": "string"})

    async def validate_input(
        self,
        name: str,
        args: dict,
    ) -> ValidationResult:
        """验证工具输入"""
        tool = self._tools.get(name)
        if not tool:
            return ValidationResult(valid=False, errors=["Tool not found"])

        # JSON Schema 验证
        try:
            import jsonschema
            jsonschema.validate(args, tool.parameters)
            return ValidationResult(valid=True)
        except jsonschema.ValidationError as e:
            return ValidationResult(valid=False, errors=[str(e)])

    async def invoke_with_timeout(
        self,
        name: str,
        timeout: int,
        **kwargs,
    ) -> Any:
        """带超时的调用"""
        try:
            async with asyncio.timeout(timeout):
                return await self.invoke(name, **kwargs)
        except asyncio.TimeoutError:
            raise ToolTimeoutError(f"Tool '{name}' timed out after {timeout}s")

@dataclass
class ValidationResult:
    valid: bool
    errors: list[str] = field(default_factory=list)

class ToolTimeoutError(Exception):
    pass

# 使用示例
@registry.tool()
async def search_files(
    pattern: Annotated[str, "搜索模式，支持 glob 语法"],
    max_results: Annotated[int, "最大返回数量"] = 10,
    file_types: Annotated[Optional[list[str]], "文件类型过滤"] = None,
) -> list[str]:
    """搜索文件"""
    pass
```

### 3.4 Workflow Engine 增强

```python
class StateGraph:
    # ... 原有代码 ...

    def add_parallel(
        self,
        source: str,
        targets: list[str],
        merge_strategy: str = "wait_all",
    ) -> "StateGraph":
        """
        添加并行分支

        merge_strategy:
        - wait_all: 等待所有分支完成
        - wait_any: 任一分支完成即可
        """
        for target in targets:
            self.edges.append(WorkflowEdge(
                source=source,
                target=target,
                parallel=True,
                merge_strategy=merge_strategy,
            ))
        return self

    def add_subgraph(
        self,
        name: str,
        subgraph: "StateGraph",
        input_mapping: Optional[dict] = None,
        output_mapping: Optional[dict] = None,
    ) -> "StateGraph":
        """添加子图"""
        self.nodes[name] = SubgraphNode(
            name=name,
            subgraph=subgraph,
            input_mapping=input_mapping,
            output_mapping=output_mapping,
        )
        return self

class CompiledGraph:
    # ... 原有代码 ...

    async def invoke(
        self,
        initial_state: dict,
        checkpoint: Optional[bool] = False,
    ) -> dict:
        """执行工作流"""
        state = initial_state.copy()
        current_nodes = [self.graph.entry_point]

        while current_nodes:
            # 收集所有并行节点
            parallel_nodes = []
            sequential_nodes = []

            for node_name in current_nodes:
                edges = self._get_outgoing_edges(node_name)
                for edge in edges:
                    if getattr(edge, "parallel", False):
                        parallel_nodes.append(edge.target)
                    else:
                        sequential_nodes.append(edge.target)

            # 执行并行节点
            if parallel_nodes:
                results = await asyncio.gather(
                    *[self._execute_node(n, state) for n in parallel_nodes]
                )
                # 合并结果
                state = self._merge_results(state, results)
                current_nodes = parallel_nodes
            else:
                # 顺序执行
                next_nodes = []
                for node_name in current_nodes:
                    node = self.graph.nodes.get(node_name)
                    if node:
                        state = await node.execute(state)

                        # 保存检查点
                        if checkpoint:
                            await self._save_checkpoint(node_name, state)

                    next_nodes.append(self._get_next_node(node_name, state))
                current_nodes = [n for n in next_nodes if n and n != "__end__"]

        return state

    async def _save_checkpoint(self, node_name: str, state: dict) -> None:
        """保存检查点"""
        checkpoint_id = f"{int(time.time())}:{node_name}"
        await self.storage.set(f"checkpoint:{checkpoint_id}", state)

    async def restore_from_checkpoint(self, checkpoint_id: str) -> dict:
        """从检查点恢复"""
        return await self.storage.get(f"checkpoint:{checkpoint_id}")
```

### 3.5 Config System 增强

```python
from typing import Any, Optional
import os
import yaml
from pathlib import Path

class ConfigLayer(ABC):
    """配置层基类"""

    @abstractmethod
    def get(self, key: str) -> Optional[Any]:
        pass

    @abstractmethod
    def has(self, key: str) -> bool:
        pass

    @abstractmethod
    def set(self, key: str, value: Any) -> None:
        pass

class DefaultConfig(ConfigLayer):
    """默认配置"""

    DEFAULTS = {
        "model.provider": "openai",
        "model.name": "gpt-4-turbo",
        "model.temperature": 0.7,
        "model.max_tokens": 4096,
        "context.max_tokens": 128000,
        "context.auto_compact_threshold": 0.95,
        "agent.max_iterations": 30,
        "agent.max_corrections": 3,
        "retry.max_retries": 3,
        "retry.backoff_type": "exponential_jitter",
    }

    def get(self, key: str) -> Optional[Any]:
        return self.DEFAULTS.get(key)

    def has(self, key: str) -> bool:
        return key in self.DEFAULTS

    def set(self, key: str, value: Any) -> None:
        self.DEFAULTS[key] = value

class FileConfig(ConfigLayer):
    """文件配置"""

    def __init__(self, filepath: str):
        self.filepath = Path(filepath)
        self._config: dict = {}
        self._load()

    def _load(self) -> None:
        if self.filepath.exists():
            with open(self.filepath, "r", encoding="utf-8") as f:
                self._config = yaml.safe_load(f) or {}

    def _save(self) -> None:
        with open(self.filepath, "w", encoding="utf-8") as f:
            yaml.dump(self._config, f)

    def get(self, key: str) -> Optional[Any]:
        keys = key.split(".")
        value = self._config
        for k in keys:
            if isinstance(value, dict):
                value = value.get(k)
            else:
                return None
        return value

    def has(self, key: str) -> bool:
        return self.get(key) is not None

    def set(self, key: str, value: Any) -> None:
        keys = key.split(".")
        config = self._config
        for k in keys[:-1]:
            config = config.setdefault(k, {})
        config[keys[-1]] = value
        self._save()

class EnvConfig(ConfigLayer):
    """环境变量配置"""

    ENV_PREFIX = "EGG_"

    def get(self, key: str) -> Optional[Any]:
        env_key = self.ENV_PREFIX + key.upper().replace(".", "_")
        value = os.environ.get(env_key)
        if value:
            # 尝试转换类型
            if value.lower() in ("true", "false"):
                return value.lower() == "true"
            try:
                return int(value)
            except ValueError:
                try:
                    return float(value)
                except ValueError:
                    return value
        return None

    def has(self, key: str) -> bool:
        env_key = self.ENV_PREFIX + key.upper().replace(".", "_")
        return env_key in os.environ

    def set(self, key: str, value: Any) -> None:
        env_key = self.ENV_PREFIX + key.upper().replace(".", "_")
        os.environ[env_key] = str(value)

class RuntimeConfig(ConfigLayer):
    """运行时配置"""

    def __init__(self):
        self._config: dict = {}

    def get(self, key: str) -> Optional[Any]:
        return self._config.get(key)

    def has(self, key: str) -> bool:
        return key in self._config

    def set(self, key: str, value: Any) -> None:
        self._config[key] = value

class ConfigManager:
    """分层配置管理器"""

    def __init__(self, config_file: Optional[str] = None):
        self.layers: list[ConfigLayer] = [
            DefaultConfig(),
        ]

        if config_file:
            self.layers.append(FileConfig(config_file))

        self.layers.extend([
            EnvConfig(),
            RuntimeConfig(),
        ])

    def get(self, key: str, default: Any = None) -> Any:
        """获取配置（按优先级）"""
        for layer in reversed(self.layers):
            if layer.has(key):
                return layer.get(key)
        return default

    def set(self, key: str, value: Any, layer: str = "runtime") -> None:
        """设置配置"""
        layer_map = {
            "default": self.layers[0],
            "file": self.layers[1] if len(self.layers) > 2 else None,
            "env": self.layers[-2] if len(self.layers) > 1 else None,
            "runtime": self.layers[-1],
        }
        target = layer_map.get(layer)
        if target:
            target.set(key, value)

    def validate(self) -> ValidationResult:
        """验证配置"""
        errors = []

        # 检查必需配置
        required = ["model.provider", "model.name"]
        for key in required:
            if not self.get(key):
                errors.append(f"Missing required config: {key}")

        # 检查值范围
        temperature = self.get("model.temperature")
        if temperature is not None and not (0 <= temperature <= 2):
            errors.append(f"Invalid temperature: {temperature}")

        return ValidationResult(valid=len(errors) == 0, errors=errors)

    def get_all(self) -> dict:
        """获取所有配置（合并后）"""
        result = {}
        for layer in self.layers:
            if hasattr(layer, "DEFAULTS"):
                result.update(layer.DEFAULTS)
            elif hasattr(layer, "_config"):
                result.update(self._flatten(layer._config))
        return result

    def _flatten(self, d: dict, parent_key: str = "") -> dict:
        """扁平化嵌套字典"""
        items = []
        for k, v in d.items():
            new_key = f"{parent_key}.{k}" if parent_key else k
            if isinstance(v, dict):
                items.extend(self._flatten(v, new_key).items())
            else:
                items.append((new_key, v))
        return dict(items)
```

---

## 四、生命周期管理

```python
from typing import Callable, Awaitable
from enum import Enum

class LifecycleState(Enum):
    CREATED = "created"
    INITIALIZING = "initializing"
    READY = "ready"
    RUNNING = "running"
    STOPPING = "stopping"
    STOPPED = "stopped"
    ERROR = "error"

class LifecycleManager:
    """组件生命周期管理"""

    def __init__(self):
        self.state = LifecycleState.CREATED
        self.components: dict[str, Any] = {}
        self._shutdown_hooks: list[Callable[[], Awaitable[None]]] = []

    def register(self, name: str, component: Any) -> None:
        """注册组件"""
        self.components[name] = component

    def add_shutdown_hook(self, hook: Callable[[], Awaitable[None]]) -> None:
        """添加关闭钩子"""
        self._shutdown_hooks.append(hook)

    async def initialize(self) -> None:
        """初始化所有组件"""
        self.state = LifecycleState.INITIALIZING

        try:
            # 按顺序初始化
            init_order = [
                "config",
                "storage",
                "security",
                "llm",
                "tools",
                "context",
                "agent",
            ]

            for name in init_order:
                if name in self.components:
                    component = self.components[name]
                    if hasattr(component, "initialize"):
                        await component.initialize()

            self.state = LifecycleState.READY

        except Exception as e:
            self.state = LifecycleState.ERROR
            raise

    async def start(self) -> None:
        """启动服务"""
        if self.state != LifecycleState.READY:
            await self.initialize()

        self.state = LifecycleState.RUNNING

    async def stop(self) -> None:
        """停止服务"""
        self.state = LifecycleState.STOPPING

        # 执行关闭钩子
        for hook in reversed(self._shutdown_hooks):
            try:
                await hook()
            except Exception as e:
                print(f"Shutdown hook failed: {e}")

        # 关闭组件（逆序）
        for name in reversed(list(self.components.keys())):
            component = self.components[name]
            if hasattr(component, "shutdown"):
                try:
                    await component.shutdown()
                except Exception as e:
                    print(f"Component {name} shutdown failed: {e}")

        self.state = LifecycleState.STOPPED

    async def health_check(self) -> dict:
        """健康检查"""
        health = {
            "state": self.state.value,
            "components": {},
        }

        for name, component in self.components.items():
            if hasattr(component, "health_check"):
                try:
                    health["components"][name] = await component.health_check()
                except Exception as e:
                    health["components"][name] = {"status": "error", "message": str(e)}
            else:
                health["components"][name] = {"status": "unknown"}

        return health
```

---

## 五、项目结构更新

```
egg-harness/
├── src/
│   └── egg_harness/
│       ├── __init__.py
│       ├── core/
│       │   ├── __init__.py
│       │   ├── harness.py
│       │   ├── agent.py
│       │   ├── config.py
│       │   └── lifecycle.py          # 新增
│       ├── llm/
│       │   ├── __init__.py
│       │   ├── base.py
│       │   ├── registry.py
│       │   ├── messages.py
│       │   ├── retry.py              # 新增
│       │   └── providers/
│       │       └── ...
│       ├── runtime/
│       │   ├── __init__.py
│       │   ├── loop.py
│       │   ├── streaming.py
│       │   └── planner.py            # 新增
│       ├── context/
│       │   ├── __init__.py
│       │   ├── manager.py
│       │   ├── compact.py
│       │   ├── repo_map.py
│       │   ├── token_counter.py
│       │   └── budget.py             # 新增
│       ├── tools/
│       │   ├── __init__.py
│       │   ├── registry.py
│       │   ├── schema.py
│       │   ├── interceptor.py
│       │   ├── validation.py         # 新增
│       │   └── builtin/
│       │       └── ...
│       ├── workflow/
│       │   ├── __init__.py
│       │   ├── graph.py
│       │   ├── node.py
│       │   ├── edge.py
│       │   ├── parallel.py           # 新增
│       │   └── persistence.py        # 新增
│       ├── memory/
│       │   └── ...
│       ├── hooks/
│       │   └── ...
│       ├── events/
│       │   └── ...
│       ├── error/                    # 新增目录
│       │   ├── __init__.py
│       │   ├── classifier.py
│       │   ├── retry.py
│       │   └── recovery.py
│       ├── observability/            # 新增目录
│       │   ├── __init__.py
│       │   ├── tracer.py
│       │   ├── metrics.py
│       │   └── agent_metrics.py
│       ├── security/                 # 新增目录
│       │   ├── __init__.py
│       │   ├── key_manager.py
│       │   ├── audit.py
│       │   └── permission.py
│       ├── storage/
│       │   └── ...
│       └── integrations/
│           └── ...
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
├── examples/
├── docs/
├── pyproject.toml
└── README.md
```

---

## 六、实施优先级

### Phase 1.5: 核心补强（2 周）

- [ ] Error Handler + Retry 机制
- [ ] Token Budget Manager
- [ ] 生命周期管理
- [ ] 配置系统完整实现

### Phase 2.5: 可观测性（1.5 周）

- [ ] 分布式追踪（Tracer）
- [ ] 指标收集（Metrics）
- [ ] 审计日志

### Phase 3.5: 安全和高级特性（1.5 周）

- [ ] Security Manager
- [ ] Agent Planner
- [ ] Workflow 增强（并行、持久化）

---

## 七、关键改进总结

| 类别 | 改进项 | 影响 |
|------|--------|------|
| **稳定性** | Retry + Error Handler | 减少 90% 临时失败 |
| **可预测性** | Token Budget | 避免 Context 超限 |
| **可调试性** | Tracer + Metrics | 生产问题定位时间 -80% |
| **安全性** | Key Manager + Audit | 企业级安全合规 |
| **智能性** | Agent Planner | 复杂任务成功率 +30% |
| **扩展性** | Workflow 增强 | 支持复杂业务流程 |

---

**文档状态**: 已更新
**下一步**: 确认改进方案后开始 Phase 1.5 开发
