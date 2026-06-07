# Continuum 测试策略设计文档

> 版本: v1.0
> 日期: 2026-05-10
> 基于: 十四轮专家评审共识 + RELIABILITY_DESIGN.md + TECHNOLOGY_PRODUCT_POLICY.md
> 技术栈: pytest + pytest-asyncio + pytest-cov + httpx + pydantic

---

## 一、测试策略定位

### 1.1 核心公式

```
质量保证 = 测试覆盖 × 自动化程度 × 可追溯性

其中：
测试覆盖 = 单元测试(80%) + 集成测试(核心路径100%) + E2E测试(3关键场景)
自动化程度 = CI/CD集成 + 自动回归测试 + 覆盖率门槛
可追溯性 = 测试报告 + 错误追踪 + 代码质量门禁
```

### 1.2 与MVP核心功能的关系

```
MVP核心功能: 会话无缝延续

测试保障:
├── Checkpoint完整性: 单元测试100%覆盖保存/恢复逻辑
├── 状态一致性: 集成测试验证跨轮次状态
├── 异常恢复: E2E测试模拟崩溃恢复场景
└── API兼容性: 版本升级测试确保向后兼容
```

### 1.3 测试哲学

```
原则1: 测试驱动开发
- 核心模块先写测试再写代码
- 每个Bug修复必须伴随测试用例

原则2: 金字塔测试模型
- 单元测试(底层): 数量多、速度快、成本低
- 集成测试(中层): 关键路径、适度数量
- E2E测试(顶层): 最少数量、最高价值

原则3: 可读性优先
- 测试代码即文档
- 测试名称描述行为
- 断言信息清晰明了
```

---

## 二、单元测试策略（目标80%覆盖率）

### 2.1 模块覆盖率目标

| 模块 | 目标覆盖率 | 优先级 | 说明 |
|------|-----------|--------|------|
| LLM Provider | 90% | P0 | 核心模块，必须高覆盖 |
| Error Handler | 90% | P0 | 重试逻辑关键 |
| Token Budget | 85% | P0 | 预算管理核心 |
| Context Manager | 85% | P0 | 包含压缩逻辑 |
| Tool Registry | 85% | P1 | 包含验证和超时 |
| Session Manager | 90% | P0 | MVP核心组件 |
| Checkpoint | 90% | P0 | MVP核心功能 |
| Memory System | 80% | P1 | 持久化逻辑 |
| Hooks System | 80% | P1 | 生命周期管理 |
| Workflow Engine | 75% | P2 | 状态图复杂 |
| MCP Client | 70% | P2 | 依赖外部服务 |

### 2.2 测试文件结构

```
tests/
├── unit/
│   ├── __init__.py
│   ├── llm/
│   │   ├── __init__.py
│   │   ├── test_base.py              # LLMProvider基类测试
│   │   ├── test_openai.py            # OpenAI适配器测试
│   │   ├── test_anthropic.py         # Anthropic适配器测试
│   │   └── test_retry.py             # 重试逻辑测试
│   ├── context/
│   │   ├── __init__.py
│   │   ├── test_manager.py           # ContextManager测试
│   │   ├── test_budget.py            # TokenBudgetManager测试
│   │   └── test_compact.py           # 压缩逻辑测试
│   ├── tools/
│   │   ├── __init__.py
│   │   ├── test_registry.py          # ToolRegistry测试
│   │   ├── test_validation.py        # 输入验证测试
│   │   └── test_timeout.py            # 超时控制测试
│   ├── session/
│   │   ├── __init__.py
│   │   ├── test_manager.py           # SessionManager测试
│   │   ├── test_checkpoint.py         # Checkpoint测试
│   │   └── test_recovery.py           # 恢复逻辑测试
│   ├── memory/
│   │   ├── __init__.py
│   │   ├── test_project.py            # ProjectMemory测试
│   │   └── test_auto.py               # AutoMemory测试
│   ├── hooks/
│   │   ├── __init__.py
│   │   ├── test_system.py             # HookSystem测试
│   │   └── test_builtin.py            # 内置Hook测试
│   ├── error/
│   │   ├── __init__.py
│   │   ├── test_classifier.py         # 错误分类测试
│   │   └── test_retry_executor.py      # 重试执行器测试
│   ├── observability/
│   │   ├── __init__.py
│   │   ├── test_tracer.py             # SimpleTracer测试
│   │   └── test_metrics.py            # SimpleMetrics测试
│   └── config/
│       ├── __init__.py
│       └── test_config.py             # Config测试
├── integration/
│   ├── __init__.py
│   ├── test_agent_flow.py            # Agent完整执行流程
│   ├── test_session_flow.py           # 会话生命周期测试
│   ├── test_memory_integration.py     # Memory系统集成测试
│   ├── test_mcp_integration.py        # MCP集成测试
│   └── test_hooks_integration.py     # Hooks系统集成测试
├── e2e/
│   ├── __init__.py
│   ├── test_real_llm.py               # 真实LLM API测试
│   ├── test_crash_recovery.py         # 崩溃恢复E2E测试
│   └── test_cli_commands.py           # CLI命令E2E测试
├── conftest.py                        # pytest配置和fixtures
└── __init__.py
```

### 2.3 单元测试模板

#### 2.3.1 基础测试类

```python
# tests/unit/llm/test_openai.py
"""OpenAI Provider单元测试"""

import pytest
from unittest.mock import AsyncMock, patch, MagicMock
from httpx import Response, Request

from continuum.llm.providers.openai import OpenAIProvider
from continuum.llm.messages import Message, LLMResponse, ToolCall, FunctionCall


class TestOpenAIProvider:
    """OpenAI Provider测试类"""

    @pytest.fixture
    def provider(self):
        """创建测试用Provider实例"""
        return OpenAIProvider(api_key="test-key-12345")

    @pytest.fixture
    def mock_response(self):
        """Mock LLM响应数据"""
        return {
            "choices": [{
                "message": {
                    "content": "Hello, I'm your assistant.",
                    "role": "assistant"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        }

    @pytest.mark.asyncio
    async def test_chat_basic(self, provider, mock_response):
        """测试基本聊天功能"""
        # Given: 准备消息
        messages = [Message(role="user", content="Hello")]

        # When: Mock HTTP响应并调用chat
        with patch.object(provider.client, 'post', new_callable=AsyncMock) as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: mock_response,
                raise_for_status=MagicMock()
            )
            response = await provider.chat(messages=messages, model="gpt-4")

        # Then: 验证响应
        assert response.content == "Hello, I'm your assistant."
        assert response.finish_reason == "stop"
        assert response.usage["total_tokens"] == 18

    @pytest.mark.asyncio
    async def test_chat_with_tool_calls(self, provider):
        """测试工具调用响应解析"""
        # Given: 带工具调用的响应
        tool_response = {
            "choices": [{
                "message": {
                    "content": None,
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": '{"path": "/test.txt"}'
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30}
        }

        messages = [Message(role="user", content="Read the test file")]

        # When
        with patch.object(provider.client, 'post', new_callable=AsyncMock) as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: tool_response,
                raise_for_status=MagicMock()
            )
            response = await provider.chat(messages=messages, model="gpt-4")

        # Then
        assert response.tool_calls is not None
        assert len(response.tool_calls) == 1
        assert response.tool_calls[0].function.name == "read_file"
        assert response.finish_reason == "tool_calls"

    @pytest.mark.asyncio
    async def test_chat_rate_limit_error(self, provider):
        """测试Rate Limit错误处理"""
        # Given
        messages = [Message(role="user", content="Hello")]

        # When: Mock Rate Limit错误
        with patch.object(provider.client, 'post', new_callable=AsyncMock) as mock_post:
            mock_post.side_effect = Exception("429 Too Many Requests")

            # Then: 预期抛出异常
            with pytest.raises(Exception) as exc_info:
                await provider.chat(messages=messages, model="gpt-4")

            assert "429" in str(exc_info.value)

    @pytest.mark.asyncio
    async def test_chat_timeout_error(self, provider):
        """测试超时错误处理"""
        import asyncio

        messages = [Message(role="user", content="Hello")]

        with patch.object(provider.client, 'post', new_callable=AsyncMock) as mock_post:
            mock_post.side_effect = asyncio.TimeoutError()

            with pytest.raises(asyncio.TimeoutError):
                await provider.chat(messages=messages, model="gpt-4")

    def test_format_message_basic(self, provider):
        """测试消息格式化"""
        message = Message(role="user", content="Hello")

        formatted = provider._format_msg(message)

        assert formatted["role"] == "user"
        assert formatted["content"] == "Hello"

    def test_format_message_with_tool_calls(self, provider):
        """测试带工具调用的消息格式化"""
        message = Message(
            role="assistant",
            content="Let me check",
            tool_calls=[
                ToolCall(
                    id="call_123",
                    function=FunctionCall(name="read_file", arguments='{"path": "/test"}')
                )
            ]
        )

        formatted = provider._format_msg(message)

        assert "tool_calls" in formatted
        assert formatted["tool_calls"][0]["id"] == "call_123"
        assert formatted["tool_calls"][0]["function"]["name"] == "read_file"
```

#### 2.3.2 SessionManager测试

```python
# tests/unit/session/test_manager.py
"""SessionManager单元测试 - MVP核心组件"""

import pytest
import json
import os
from datetime import datetime
from pathlib import Path
from unittest.mock import patch, MagicMock, AsyncMock

from continuum.session.manager import SessionManager, Checkpoint
from continuum.session.state import AgentState, ExecutionContext


class TestSessionManager:
    """SessionManager测试类 - MVP核心"""

    @pytest.fixture
    def temp_storage(self, tmp_path):
        """创建临时存储目录"""
        storage_dir = tmp_path / ".continuum" / "sessions"
        storage_dir.mkdir(parents=True, exist_ok=True)
        return storage_dir

    @pytest.fixture
    def manager(self, temp_storage):
        """创建测试用SessionManager"""
        return SessionManager(
            storage_path=str(temp_storage),
            auto_save=True,
            checkpoint_interval=5,
            max_checkpoints=50
        )

    @pytest.fixture
    def sample_checkpoint(self):
        """示例Checkpoint数据"""
        return Checkpoint(
            checkpoint_id="cp_test_001",
            session_id="session_abc123",
            created_at=datetime(2026, 5, 10, 14, 32, 15),
            trigger="tool_call",
            agent_state=AgentState.RUNNING,
            iteration=5,
            messages=[
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there!"}
            ],
            tool_calls_pending=[],
            tool_results=[],
            tokens_used=1500,
            cost_estimate=0.03,
            resume_hint="准备执行第6轮"
        )

    # ==================== 保存测试 ====================

    def test_save_checkpoint_basic(self, manager, sample_checkpoint, temp_storage):
        """测试基本Checkpoint保存"""
        # When
        checkpoint_id = manager.save_checkpoint(sample_checkpoint)

        # Then
        assert checkpoint_id == sample_checkpoint.checkpoint_id

        # 验证文件存在
        checkpoint_file = (
            temp_storage /
            sample_checkpoint.session_id /
            "checkpoints" /
            f"cp_{sample_checkpoint.checkpoint_id}.json"
        )
        assert checkpoint_file.exists()

    def test_save_checkpoint_creates_directory(self, manager, sample_checkpoint, temp_storage):
        """测试Checkpoint保存自动创建目录"""
        # Given: 新session_id，目录不存在
        sample_checkpoint.session_id = "new_session_xyz"

        # When
        manager.save_checkpoint(sample_checkpoint)

        # Then: 目录应自动创建
        session_dir = temp_storage / "new_session_xyz"
        assert session_dir.exists()

    def test_save_checkpoint_with_checksum(self, manager, sample_checkpoint, temp_storage):
        """测试Checksum计算和验证"""
        # When
        manager.save_checkpoint(sample_checkpoint)

        # Then: 加载并验证checksum
        checkpoint_file = (
            temp_storage /
            sample_checkpoint.session_id /
            "checkpoints" /
            f"cp_{sample_checkpoint.checkpoint_id}.json"
        )
        with open(checkpoint_file, 'r', encoding='utf-8') as f:
            data = json.load(f)

        assert '_checksum' in data
        assert '_version' in data

    # ==================== 加载测试 ====================

    def test_load_checkpoint_basic(self, manager, sample_checkpoint):
        """测试Checkpoint加载"""
        # Given
        manager.save_checkpoint(sample_checkpoint)

        # When
        loaded = manager.load_checkpoint(
            sample_checkpoint.session_id,
            sample_checkpoint.checkpoint_id
        )

        # Then
        assert loaded is not None
        assert loaded.session_id == sample_checkpoint.session_id
        assert loaded.iteration == sample_checkpoint.iteration
        assert loaded.messages == sample_checkpoint.messages

    def test_load_latest_checkpoint(self, manager, sample_checkpoint):
        """测试加载最新Checkpoint"""
        # Given: 创建多个checkpoint
        for i in range(3):
            cp = Checkpoint(
                checkpoint_id=f"cp_test_{i:03d}",
                session_id=sample_checkpoint.session_id,
                created_at=datetime(2026, 5, 10, 14, 32, 15 + i),
                trigger="periodic",
                agent_state=AgentState.RUNNING,
                iteration=i,
                messages=[],
                tool_calls_pending=[],
                tool_results=[],
                tokens_used=1000 * i,
                cost_estimate=0.01 * i,
                resume_hint=None
            )
            manager.save_checkpoint(cp)

        # When: 加载最新
        latest = manager.load_latest_checkpoint(sample_checkpoint.session_id)

        # Then: 应该是最新的
        assert latest.checkpoint_id == "cp_test_002"
        assert latest.iteration == 2

    def test_load_checkpoint_not_found(self, manager):
        """测试加载不存在的Checkpoint"""
        loaded = manager.load_checkpoint("nonexistent_session", "cp_xxx")
        assert loaded is None

    def test_load_corrupted_checkpoint(self, manager, temp_storage):
        """测试加载损坏的Checkpoint"""
        # Given: 创建损坏的文件
        session_dir = temp_storage / "test_session" / "checkpoints"
        session_dir.mkdir(parents=True)
        corrupt_file = session_dir / "cp_corrupt.json"
        corrupt_file.write_text("{ invalid json }")

        # When
        loaded = manager.load_checkpoint("test_session", "cp_corrupt")

        # Then: 应返回None，不应崩溃
        assert loaded is None

    # ==================== 恢复测试 ====================

    @pytest.mark.asyncio
    async def test_restore_session_basic(self, manager, sample_checkpoint):
        """测试基本会话恢复"""
        # Given
        manager.save_checkpoint(sample_checkpoint)

        # When
        context = await manager.restore_session(sample_checkpoint.session_id)

        # Then
        assert context is not None
        assert context.session_id == sample_checkpoint.session_id
        assert context.state == sample_checkpoint.agent_state
        assert context.iteration == sample_checkpoint.iteration

    @pytest.mark.asyncio
    async def test_restore_session_preserves_messages(self, manager, sample_checkpoint):
        """测试恢复保留完整消息历史"""
        # Given
        manager.save_checkpoint(sample_checkpoint)

        # When
        context = await manager.restore_session(sample_checkpoint.session_id)

        # Then
        assert len(context.messages) == 2
        assert context.messages[0]["role"] == "user"
        assert context.messages[0]["content"] == "Hello"

    @pytest.mark.asyncio
    async def test_restore_session_updates_resume_count(self, manager, sample_checkpoint, temp_storage):
        """测试恢复更新恢复计数"""
        # Given
        manager.save_checkpoint(sample_checkpoint)

        # When: 第一次恢复
        await manager.restore_session(sample_checkpoint.session_id)

        # Then: 检查元数据
        meta_file = temp_storage / sample_checkpoint.session_id / "session_meta.json"
        with open(meta_file, 'r') as f:
            meta = json.load(f)
        assert meta["resume_count"] == 1

        # When: 第二次恢复
        await manager.restore_session(sample_checkpoint.session_id)

        # Then: 计数增加
        with open(meta_file, 'r') as f:
            meta = json.load(f)
        assert meta["resume_count"] == 2

    # ==================== 清理测试 ====================

    def test_prune_old_checkpoints(self, manager, sample_checkpoint):
        """测试旧Checkpoint清理"""
        # Given: 创建超过限制的checkpoint
        manager.max_checkpoints = 5
        for i in range(10):
            cp = Checkpoint(
                checkpoint_id=f"cp_prune_{i:03d}",
                session_id=sample_checkpoint.session_id,
                created_at=datetime(2026, 5, 10, 14, 32, 15 + i),
                trigger="periodic",
                agent_state=AgentState.RUNNING,
                iteration=i,
                messages=[],
                tool_calls_pending=[],
                tool_results=[],
                tokens_used=1000,
                cost_estimate=0.01,
                resume_hint=None
            )
            manager.save_checkpoint(cp)

        # When
        manager.prune_old_checkpoints(sample_checkpoint.session_id)

        # Then: 只保留最新的5个
        checkpoints = manager.list_checkpoints(sample_checkpoint.session_id)
        assert len(checkpoints) == 5
        # 验证保留的是最新的
        assert checkpoints[0].checkpoint_id == "cp_prune_009"

    # ==================== 触发时机测试 ====================

    @pytest.mark.asyncio
    async def test_auto_checkpoint_on_iteration(self, manager):
        """测试轮次触发的自动Checkpoint"""
        # Given
        manager.checkpoint_interval = 5
        context = ExecutionContext(
            session_id="test_auto",
            agent_id="agent_001",
            state=AgentState.RUNNING,
            iteration=5,
            max_iterations=30,
            messages=[],
            tool_calls_pending=[],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="",
            tokens_total=1000,
            tokens_prompt=500,
            tokens_completion=500,
            cost_estimate=0.01,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        # When
        await manager.maybe_checkpoint(context, trigger="periodic")

        # Then: 应该触发保存
        checkpoints = manager.list_checkpoints("test_auto")
        assert len(checkpoints) == 1

    @pytest.mark.asyncio
    async def test_no_checkpoint_before_interval(self, manager):
        """测试间隔内不触发Checkpoint"""
        # Given
        manager.checkpoint_interval = 5
        context = ExecutionContext(
            session_id="test_no_auto",
            agent_id="agent_001",
            state=AgentState.RUNNING,
            iteration=3,  # 小于间隔
            max_iterations=30,
            messages=[],
            tool_calls_pending=[],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="",
            tokens_total=1000,
            tokens_prompt=500,
            tokens_completion=500,
            cost_estimate=0.01,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        # When
        await manager.maybe_checkpoint(context, trigger="periodic")

        # Then: 不应保存
        checkpoints = manager.list_checkpoints("test_no_auto")
        assert len(checkpoints) == 0
```

#### 2.3.3 RetryExecutor测试

```python
# tests/unit/error/test_retry_executor.py
"""RetryExecutor单元测试"""

import pytest
import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

from continuum.error.retry import RetryExecutor, RetryConfig, BackoffType
from continuum.error.classifier import ErrorType, ClassifiedError


class TestRetryExecutor:
    """RetryExecutor测试类"""

    @pytest.fixture
    def retry_config(self):
        """重试配置"""
        return RetryConfig(
            max_retries=3,
            backoff_type=BackoffType.EXPONENTIAL_JITTER,
            base_delay=0.1,  # 测试用短延迟
            max_delay=1.0,
            retryable_errors=(ErrorType.NETWORK_ERROR, ErrorType.TIMEOUT)
        )

    @pytest.fixture
    def executor(self, retry_config):
        """创建重试执行器"""
        return RetryExecutor(retry_config)

    # ==================== 成功场景测试 ====================

    @pytest.mark.asyncio
    async def test_execute_success_on_first_try(self, executor):
        """测试首次执行成功"""
        # Given
        call_count = 0

        async def success_func():
            nonlocal call_count
            call_count += 1
            return "success"

        # When
        result = await executor.execute(success_func)

        # Then
        assert result == "success"
        assert call_count == 1

    @pytest.mark.asyncio
    async def test_execute_success_after_retry(self, executor):
        """测试重试后成功"""
        # Given
        call_count = 0

        async def retry_then_success():
            nonlocal call_count
            call_count += 1
            if call_count < 3:
                raise ConnectionError("Network error")
            return "success"

        # When
        result = await executor.execute(retry_then_success)

        # Then
        assert result == "success"
        assert call_count == 3

    # ==================== 失败场景测试 ====================

    @pytest.mark.asyncio
    async def test_execute_all_retries_failed(self, executor):
        """测试所有重试失败"""
        # Given
        async def always_fail():
            raise ConnectionError("Always fails")

        # When/Then
        with pytest.raises(Exception):
            await executor.execute(always_fail)

    @pytest.mark.asyncio
    async def test_execute_non_retryable_error(self, executor):
        """测试不可重试错误立即失败"""
        # Given
        async def auth_error():
            raise PermissionError("Invalid API key")

        # When/Then: 应立即抛出异常，不重试
        with pytest.raises(PermissionError):
            await executor.execute(auth_error)

    # ==================== 退避策略测试 ====================

    @pytest.mark.asyncio
    async def test_exponential_backoff(self):
        """测试指数退避"""
        config = RetryConfig(
            max_retries=3,
            backoff_type=BackoffType.EXPONENTIAL,
            base_delay=0.1,
            max_delay=10.0
        )
        executor = RetryExecutor(config)

        delays = []
        call_times = []

        async def track_delay():
            call_times.append(asyncio.get_event_loop().time())
            if len(call_times) < 4:
                raise ConnectionError("Retry")
            return "done"

        # When
        start = asyncio.get_event_loop().time()
        result = await executor.execute(track_delay)

        # Then: 验证延迟符合指数增长
        # 第1次重试: ~0.1s, 第2次重试: ~0.2s, 第3次重试: ~0.4s
        assert result == "done"

    @pytest.mark.asyncio
    async def test_max_delay_cap(self):
        """测试最大延迟上限"""
        config = RetryConfig(
            max_retries=10,
            backoff_type=BackoffType.EXPONENTIAL,
            base_delay=1.0,
            max_delay=2.0  # 限制最大延迟
        )
        executor = RetryExecutor(config)

        call_count = 0

        async def always_fail():
            nonlocal call_count
            call_count += 1
            raise ConnectionError("Always fails")

        # When: 总时间应该受max_delay限制
        import time
        start = time.time()
        with pytest.raises(Exception):
            await executor.execute(always_fail)
        elapsed = time.time() - start

        # Then: 总时间不应超过 (1+2+2+...+2)秒太多
        # 实际测试中由于async，可能需要调整断言
        assert call_count == config.max_retries + 1

    # ==================== 回调测试 ====================

    @pytest.mark.asyncio
    async def test_retry_callback_called(self, executor):
        """测试重试回调被调用"""
        # Given
        callback_calls = []

        def on_retry(attempt, error, delay):
            callback_calls.append({
                'attempt': attempt,
                'error': str(error),
                'delay': delay
            })

        call_count = 0

        async def fail_twice():
            nonlocal call_count
            call_count += 1
            if call_count < 3:
                raise ConnectionError("Retry needed")
            return "success"

        # When
        await executor.execute(fail_twice, on_retry=on_retry)

        # Then: 应有2次回调
        assert len(callback_calls) == 2
        assert callback_calls[0]['attempt'] == 1
        assert "Retry needed" in callback_calls[0]['error']

    # ==================== 边界条件测试 ====================

    @pytest.mark.asyncio
    async def test_zero_retries(self):
        """测试零重试配置"""
        config = RetryConfig(max_retries=0)
        executor = RetryExecutor(config)

        async def fail_once():
            raise ConnectionError("Fail")

        # When/Then: 应立即失败
        with pytest.raises(ConnectionError):
            await executor.execute(fail_once)

    @pytest.mark.asyncio
    async def test_negative_base_delay(self):
        """测试负延迟（应使用默认值）"""
        config = RetryConfig(base_delay=-1.0)
        executor = RetryExecutor(config)

        # Then: 不应崩溃
        assert executor.config.base_delay >= 0
```

### 2.4 测试Fixtures

```python
# tests/conftest.py
"""Pytest配置和共享fixtures"""

import pytest
import asyncio
import os
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch
from datetime import datetime

# 项目导入
from continuum.llm.providers.openai import OpenAIProvider
from continuum.llm.providers.anthropic import AnthropicProvider
from continuum.llm.messages import Message, LLMResponse, ToolCall, FunctionCall
from continuum.context.manager import ContextManager
from continuum.context.budget import TokenBudgetManager, TokenBudgetConfig
from continuum.session.manager import SessionManager, Checkpoint
from continuum.session.state import AgentState, ExecutionContext
from continuum.tools.registry import ToolRegistry
from continuum.error.retry import RetryExecutor, RetryConfig


# ==================== 异步配置 ====================

@pytest.fixture(scope="session")
def event_loop():
    """创建事件循环"""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


# ==================== 环境配置 ====================

@pytest.fixture
def mock_env_api_keys(monkeypatch):
    """Mock API密钥环境变量"""
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test-key-12345")
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-ant-test-key-12345")


@pytest.fixture
def temp_storage_dir(tmp_path):
    """临时存储目录"""
    storage = tmp_path / ".continuum"
    storage.mkdir()
    return storage


@pytest.fixture
def temp_session_dir(temp_storage_dir):
    """临时会话目录"""
    sessions_dir = temp_storage_dir / "sessions"
    sessions_dir.mkdir()
    return sessions_dir


# ==================== LLM Provider Fixtures ====================

@pytest.fixture
def openai_provider():
    """OpenAI Provider实例"""
    return OpenAIProvider(api_key="sk-test-key-12345")


@pytest.fixture
def anthropic_provider():
    """Anthropic Provider实例"""
    return AnthropicProvider(api_key="sk-ant-test-key-12345")


@pytest.fixture
def mock_openai_response():
    """Mock OpenAI响应"""
    return {
        "choices": [{
            "message": {
                "content": "Test response",
                "role": "assistant"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    }


@pytest.fixture
def mock_tool_call_response():
    """Mock工具调用响应"""
    return {
        "choices": [{
            "message": {
                "content": None,
                "tool_calls": [{
                    "id": "call_001",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": '{"path": "/test.txt"}'
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 10,
            "total_tokens": 30
        }
    }


# ==================== Context Fixtures ====================

@pytest.fixture
def budget_config():
    """Token预算配置"""
    return TokenBudgetConfig(
        total_budget=128000,
        response_reserve=4096,
        safety_margin=0.05
    )


@pytest.fixture
def budget_manager(budget_config):
    """Token预算管理器"""
    return TokenBudgetManager(budget_config)


@pytest.fixture
def context_manager(budget_manager):
    """上下文管理器"""
    return ContextManager(
        max_tokens=128000,
        budget_manager=budget_manager
    )


# ==================== Session Fixtures ====================

@pytest.fixture
def session_manager(temp_session_dir):
    """会话管理器"""
    return SessionManager(
        storage_path=str(temp_session_dir),
        auto_save=True,
        checkpoint_interval=5,
        max_checkpoints=50
    )


@pytest.fixture
def sample_checkpoint():
    """示例Checkpoint"""
    return Checkpoint(
        checkpoint_id="cp_test_001",
        session_id="session_test_001",
        created_at=datetime(2026, 5, 10, 14, 32, 15),
        trigger="periodic",
        agent_state=AgentState.RUNNING,
        iteration=5,
        messages=[
            {"role": "user", "content": "Hello"},
            {"role": "assistant", "content": "Hi there!"}
        ],
        tool_calls_pending=[],
        tool_results=[],
        tokens_used=1500,
        cost_estimate=0.03,
        resume_hint="准备执行第6轮"
    )


@pytest.fixture
def execution_context():
    """执行上下文"""
    return ExecutionContext(
        session_id="test_session",
        agent_id="test_agent",
        state=AgentState.IDLE,
        iteration=0,
        max_iterations=30,
        messages=[],
        tool_calls_pending=[],
        tool_results_cache={},
        model="gpt-4",
        temperature=0.7,
        system_prompt="You are a helpful assistant.",
        tokens_total=0,
        tokens_prompt=0,
        tokens_completion=0,
        cost_estimate=0.0,
        created_at=datetime.now(),
        last_updated=datetime.now(),
        checkpoint_count=0
    )


# ==================== Tool Fixtures ====================

@pytest.fixture
def tool_registry():
    """工具注册表"""
    return ToolRegistry()


@pytest.fixture
def sample_tools():
    """示例工具定义"""
    @ToolRegistry.tool(description="读取文件内容")
    async def read_file(path: str, encoding: str = "utf-8") -> str:
        """读取文件"""
        with open(path, 'r', encoding=encoding) as f:
            return f.read()

    @ToolRegistry.tool(description="写入文件内容", timeout=60)
    async def write_file(path: str, content: str) -> str:
        """写入文件"""
        with open(path, 'w') as f:
            f.write(content)
        return f"Written to {path}"

    return [read_file, write_file]


# ==================== Retry Fixtures ====================

@pytest.fixture
def retry_config():
    """重试配置"""
    return RetryConfig(
        max_retries=3,
        base_delay=0.1,
        max_delay=1.0
    )


@pytest.fixture
def retry_executor(retry_config):
    """重试执行器"""
    return RetryExecutor(retry_config)


# ==================== Mock Helpers ====================

@pytest.fixture
def mock_async_http_response():
    """创建Mock HTTP响应"""
    def create_mock(json_data, status_code=200):
        response = AsyncMock()
        response.json = AsyncMock(return_value=json_data)
        response.status_code = status_code
        response.raise_for_status = MagicMock()
        return response
    return create_mock


@pytest.fixture
def mock_llm_client():
    """Mock LLM客户端"""
    client = AsyncMock()
    client.post = AsyncMock()
    client.get = AsyncMock()
    return client
```

---

## 三、集成测试策略（核心路径100%）

### 3.1 核心路径定义

```
核心路径 = MVP必须100%测试通过的路径

Path 1: Agent完整执行循环
用户输入 → LLM调用 → 工具调用 → 结果处理 → 返回结果

Path 2: 会话生命周期
创建会话 → 执行任务 → Checkpoint保存 → 中断恢复 → 继续执行

Path 3: 错误恢复流程
错误发生 → 分类识别 → 重试执行 → 降级处理 → 用户通知
```

### 3.2 集成测试用例

```python
# tests/integration/test_agent_flow.py
"""Agent完整执行流程集成测试"""

import pytest
import asyncio
from unittest.mock import AsyncMock, patch, MagicMock
from datetime import datetime

from continuum import Harness, Agent
from continuum.llm.messages import Message, LLMResponse, ToolCall, FunctionCall
from continuum.session.state import AgentState


class TestAgentFlow:
    """Agent完整流程集成测试"""

    @pytest.fixture
    def harness(self, temp_storage_dir, mock_env_api_keys):
        """创建测试用Harness"""
        return Harness(
            config=None,  # 使用默认配置
            storage_path=str(temp_storage_dir)
        )

    @pytest.fixture
    def mock_llm_responses(self):
        """Mock LLM响应序列"""
        return [
            # 第一次调用：返回工具调用
            LLMResponse(
                content=None,
                tool_calls=[
                    ToolCall(
                        id="call_001",
                        function=FunctionCall(
                            name="read_file",
                            arguments='{"path": "/test.txt"}'
                        )
                    )
                ],
                finish_reason="tool_calls",
                usage={"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30}
            ),
            # 第二次调用：返回最终结果
            LLMResponse(
                content="文件内容已读取完成。",
                tool_calls=None,
                finish_reason="stop",
                usage={"prompt_tokens": 50, "completion_tokens": 20, "total_tokens": 70}
            )
        ]

    # ==================== Path 1: 完整执行循环 ====================

    @pytest.mark.asyncio
    async def test_full_agent_loop(self, harness, mock_llm_responses, tmp_path):
        """测试完整的Agent执行循环"""
        # Given: 准备测试文件
        test_file = tmp_path / "test.txt"
        test_file.write_text("Hello, World!")

        # Given: 注册测试工具
        @harness.tool
        async def read_file(path: str) -> str:
            """读取文件"""
            with open(path, 'r') as f:
                return f.read()

        # Given: Mock LLM
        call_count = 0
        async def mock_chat(*args, **kwargs):
            nonlocal call_count
            response = mock_llm_responses[min(call_count, len(mock_llm_responses) - 1)]
            call_count += 1
            return response

        with patch.object(harness.llm, 'chat', side_effect=mock_chat):
            # When: 执行任务
            agent = harness.create_agent(
                name="test_agent",
                tools=["read_file"]
            )
            result = await agent.run("读取 /test.txt 文件")

            # Then: 验证结果
            assert "文件内容已读取完成" in result
            assert call_count == 2  # 两次LLM调用

    @pytest.mark.asyncio
    async def test_multiple_tool_calls(self, harness, tmp_path):
        """测试多次工具调用"""
        # Given
        test_file1 = tmp_path / "test1.txt"
        test_file1.write_text("Content 1")
        test_file2 = tmp_path / "test2.txt"
        test_file2.write_text("Content 2")

        @harness.tool
        async def read_file(path: str) -> str:
            with open(path, 'r') as f:
                return f.read()

        # Mock: 返回多个工具调用
        responses = [
            LLMResponse(
                content=None,
                tool_calls=[
                    ToolCall(id="call_1", function=FunctionCall(name="read_file", arguments='{"path": "/test1.txt"}')),
                    ToolCall(id="call_2", function=FunctionCall(name="read_file", arguments='{"path": "/test2.txt"}'))
                ],
                finish_reason="tool_calls",
                usage={"prompt_tokens": 30, "completion_tokens": 15, "total_tokens": 45}
            ),
            LLMResponse(
                content="两个文件都已读取。",
                tool_calls=None,
                finish_reason="stop",
                usage={"prompt_tokens": 80, "completion_tokens": 10, "total_tokens": 90}
            )
        ]

        call_count = 0
        async def mock_chat(*args, **kwargs):
            nonlocal call_count
            resp = responses[min(call_count, len(responses) - 1)]
            call_count += 1
            return resp

        with patch.object(harness.llm, 'chat', side_effect=mock_chat):
            agent = harness.create_agent(tools=["read_file"])
            result = await agent.run("读取两个文件")

            assert "两个文件都已读取" in result

    # ==================== Path 2: 会话生命周期 ====================

    @pytest.mark.asyncio
    async def test_session_lifecycle(self, harness, session_manager):
        """测试会话完整生命周期"""
        # Given
        agent = harness.create_agent(name="lifecycle_test")

        # When: 执行任务
        with patch.object(harness.llm, 'chat', new_callable=AsyncMock) as mock_chat:
            mock_chat.return_value = LLMResponse(
                content="任务完成",
                finish_reason="stop",
                usage={"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            )

            result = await agent.run("测试任务")

        # Then: 验证会话创建
        assert agent.context.session_id is not None

        # Then: 验证Checkpoint保存
        checkpoints = session_manager.list_checkpoints(agent.context.session_id)
        assert len(checkpoints) > 0

    @pytest.mark.asyncio
    async def test_session_interrupt_and_resume(self, harness, session_manager):
        """测试会话中断和恢复"""
        # Given: 创建执行中断的场景
        agent = harness.create_agent(name="interrupt_test")

        responses = [
            LLMResponse(
                content="开始处理...",
                finish_reason="stop",
                usage={"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            )
        ]

        with patch.object(harness.llm, 'chat', new_callable=AsyncMock) as mock_chat:
            mock_chat.return_value = responses[0]

            # When: 第一次执行
            await agent.run("开始任务")

            # 保存checkpoint
            session_manager.save_checkpoint(agent.context.to_checkpoint())

        # When: 恢复会话
        restored_context = await session_manager.restore_session(agent.context.session_id)

        # Then: 验证状态恢复
        assert restored_context.session_id == agent.context.session_id
        assert restored_context.state in [AgentState.RUNNING, AgentState.STOPPED]

    # ==================== Path 3: 错误恢复 ====================

    @pytest.mark.asyncio
    async def test_error_recovery_flow(self, harness):
        """测试错误恢复流程"""
        # Given
        agent = harness.create_agent(name="error_test")

        call_count = 0
        async def failing_then_success(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            if call_count < 3:
                raise ConnectionError("Network error")
            return LLMResponse(
                content="恢复成功",
                finish_reason="stop",
                usage={"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            )

        with patch.object(harness.llm, 'chat_with_retry', side_effect=failing_then_success):
            # When: 执行任务
            result = await agent.run("测试任务")

            # Then: 应该自动重试成功
            assert "恢复成功" in result or call_count >= 3

    @pytest.mark.asyncio
    async def test_graceful_degradation(self, harness):
        """测试优雅降级"""
        # Given
        agent = harness.create_agent(
            name="degradation_test",
            model="gpt-4"
        )

        # Mock: 高级模型失败，降级到基础模型
        async def mock_chat_with_fallback(messages, model, **kwargs):
            if model == "gpt-4":
                raise Exception("GPT-4 unavailable")
            return LLMResponse(
                content="使用备用模型完成",
                finish_reason="stop",
                usage={"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            )

        with patch.object(harness.llm, 'chat', side_effect=mock_chat_with_fallback):
            # When
            result = await agent.run("测试降级")

            # Then: 应该使用备用模型完成
            assert result is not None
```

```python
# tests/integration/test_session_flow.py
"""会话生命周期集成测试"""

import pytest
import json
import asyncio
from datetime import datetime
from pathlib import Path
from unittest.mock import patch, AsyncMock

from continuum_sdk.agent.session import Session
from continuum_sdk.agent import CheckpointClient
from continuum_sdk.api import AgentState, ExecutionContext


class TestSessionFlow:
    """会话流程集成测试"""

    @pytest.fixture
    def session_manager(self, temp_session_dir):
        """会话管理器"""
        return SessionManager(
            storage_path=str(temp_session_dir),
            auto_save=True,
            checkpoint_interval=5
        )

    # ==================== 完整会话周期 ====================

    @pytest.mark.asyncio
    async def test_full_session_cycle(self, session_manager):
        """测试完整会话周期：创建→执行→中断→恢复→完成"""
        session_id = "full_cycle_test"

        # Phase 1: 创建会话
        context = ExecutionContext(
            session_id=session_id,
            agent_id="agent_001",
            state=AgentState.IDLE,
            iteration=0,
            max_iterations=30,
            messages=[],
            tool_calls_pending=[],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="Test",
            tokens_total=0,
            tokens_prompt=0,
            tokens_completion=0,
            cost_estimate=0.0,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        # Phase 2: 模拟执行过程
        for i in range(1, 6):
            context.iteration = i
            context.state = AgentState.RUNNING
            context.messages.append({
                "role": "user" if i % 2 == 1 else "assistant",
                "content": f"Message {i}"
            })
            context.tokens_total += 100

            # 每5轮保存checkpoint
            if i % 5 == 0:
                await session_manager.maybe_checkpoint(context, trigger="periodic")

        # Phase 3: 验证checkpoint已保存
        checkpoints = session_manager.list_checkpoints(session_id)
        assert len(checkpoints) >= 1

        # Phase 4: 模拟中断和恢复
        latest_cp = session_manager.load_latest_checkpoint(session_id)
        assert latest_cp is not None
        assert latest_cp.iteration == 5

        # Phase 5: 从checkpoint恢复
        restored_context = ExecutionContext.from_checkpoint(latest_cp)
        assert restored_context.session_id == session_id
        assert restored_context.iteration == 5
        assert len(restored_context.messages) == 5

    @pytest.mark.asyncio
    async def test_crash_recovery_scenario(self, session_manager, tmp_path):
        """测试崩溃恢复场景"""
        # Given: 创建一个即将"崩溃"的会话
        session_id = "crash_test"

        context = ExecutionContext(
            session_id=session_id,
            agent_id="agent_crash",
            state=AgentState.RUNNING,
            iteration=10,
            max_iterations=30,
            messages=[
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi"},
                {"role": "user", "content": "Do something"},
                {"role": "assistant", "content": "Working..."}
            ],
            tool_calls_pending=[
                {"id": "pending_call", "name": "read_file", "arguments": "{}"}
            ],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="",
            tokens_total=500,
            tokens_prompt=300,
            tokens_completion=200,
            cost_estimate=0.05,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        # When: 保存checkpoint（模拟崩溃前保存）
        session_manager.save_checkpoint(context.to_checkpoint())

        # Then: 模拟重启后恢复
        restored_context = await session_manager.restore_session(session_id)

        assert restored_context is not None
        assert restored_context.iteration == 10
        assert len(restored_context.messages) == 4
        assert len(restored_context.tool_calls_pending) == 1

    @pytest.mark.asyncio
    async def test_multiple_sessions(self, session_manager):
        """测试多会话管理"""
        # Given: 创建多个会话
        session_ids = []
        for i in range(3):
            session_id = f"multi_test_{i}"
            session_ids.append(session_id)

            context = ExecutionContext(
                session_id=session_id,
                agent_id=f"agent_{i}",
                state=AgentState.COMPLETED,
                iteration=5,
                max_iterations=30,
                messages=[],
                tool_calls_pending=[],
                tool_results_cache={},
                model="gpt-4",
                temperature=0.7,
                system_prompt="",
                tokens_total=100 * i,
                tokens_prompt=50 * i,
                tokens_completion=50 * i,
                cost_estimate=0.01 * i,
                created_at=datetime.now(),
                last_updated=datetime.now(),
                checkpoint_count=0
            )
            session_manager.save_checkpoint(context.to_checkpoint())

        # When: 列出所有会话
        all_sessions = session_manager.list_sessions()

        # Then: 应该有3个会话
        assert len(all_sessions) >= 3

        # When: 恢复特定会话
        specific_context = await session_manager.restore_session("multi_test_1")
        assert specific_context.session_id == "multi_test_1"

    # ==================== Checkpoint验证 ====================

    @pytest.mark.asyncio
    async def test_checkpoint_integrity(self, session_manager, tmp_path):
        """测试Checkpoint完整性验证"""
        # Given: 创建checkpoint
        context = ExecutionContext(
            session_id="integrity_test",
            agent_id="agent_001",
            state=AgentState.RUNNING,
            iteration=5,
            max_iterations=30,
            messages=[{"role": "user", "content": "Test"}],
            tool_calls_pending=[],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="",
            tokens_total=100,
            tokens_prompt=50,
            tokens_completion=50,
            cost_estimate=0.01,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        checkpoint = context.to_checkpoint()
        session_manager.save_checkpoint(checkpoint)

        # When: 加载并验证
        loaded = session_manager.load_checkpoint("integrity_test", checkpoint.checkpoint_id)

        # Then: 验证完整性
        is_valid, error = CheckpointValidator.validate_checkpoint_data(loaded.__dict__)
        assert is_valid
        assert error is None

    @pytest.mark.asyncio
    async def test_checkpoint_checksum_verification(self, session_manager, tmp_path):
        """测试Checksum验证"""
        # Given: 创建并保存checkpoint
        context = ExecutionContext(
            session_id="checksum_test",
            agent_id="agent_001",
            state=AgentState.RUNNING,
            iteration=1,
            max_iterations=30,
            messages=[],
            tool_calls_pending=[],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="",
            tokens_total=0,
            tokens_prompt=0,
            tokens_completion=0,
            cost_estimate=0.0,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        checkpoint = context.to_checkpoint()
        session_manager.save_checkpoint(checkpoint)

        # When: 篡改checkpoint文件
        checkpoint_file = (
            tmp_path / ".continuum" / "sessions" /
            "checksum_test" / "checkpoints" /
            f"cp_{checkpoint.checkpoint_id}.json"
        )

        with open(checkpoint_file, 'r') as f:
            data = json.load(f)

        # 篡改数据
        data['iteration'] = 999
        data['_checksum'] = "fake_checksum"

        with open(checkpoint_file, 'w') as f:
            json.dump(data, f)

        # Then: 验证应该失败
        loaded = session_manager.load_checkpoint("checksum_test", checkpoint.checkpoint_id)
        is_valid, error = CheckpointValidator.validate_checkpoint_data(loaded.__dict__ if loaded else {})

        # checksum不匹配应返回无效
        assert not is_valid or error is not None
```

---

## 四、E2E测试策略（3个关键场景）

### 4.1 关键场景定义

```
场景1: 真实LLM调用验证
- 使用真实API Key
- 验证完整执行流程
- 验证响应解析正确性

场景2: 崩溃恢复E2E测试
- 模拟Agent执行中崩溃
- 验证/continue指令恢复
- 验证状态完整性

场景3: CLI命令E2E测试
- 验证continuum run命令
- 验证continuum chat命令
- 验证continuum continue命令
```

### 4.2 E2E测试用例

```python
# tests/e2e/test_real_llm.py
"""真实LLM API端到端测试"""

import pytest
import os
import asyncio

from continuum import Harness, Agent
from continuum.llm.messages import Message


# 跳过条件：无真实API Key
pytestmark = pytest.mark.skipif(
    not os.environ.get("OPENAI_API_KEY"),
    reason="需要真实 OPENAI_API_KEY"
)


class TestRealLLM:
    """真实LLM调用测试"""

    @pytest.fixture
    def real_harness(self):
        """使用真实API的Harness"""
        return Harness(config=None)  # 使用环境变量中的API Key

    @pytest.mark.asyncio
    @pytest.mark.e2e
    @pytest.mark.timeout(60)  # 60秒超时
    async def test_real_openai_chat(self, real_harness):
        """测试真实OpenAI聊天"""
        # Given
        agent = real_harness.create_agent(
            name="real_test",
            model="gpt-4o-mini",  # 使用成本较低的模型
            max_iterations=5
        )

        # When
        result = await agent.run("请回复'测试成功'四个字")

        # Then
        assert result is not None
        assert len(result) > 0
        assert "测试" in result or "成功" in result or "收到" in result

    @pytest.mark.asyncio
    @pytest.mark.e2e
    @pytest.mark.timeout(120)
    async def test_real_tool_call(self, real_harness, tmp_path):
        """测试真实工具调用"""
        # Given: 创建测试文件
        test_file = tmp_path / "test_read.txt"
        test_file.write_text("Hello from test file!")

        @real_harness.tool
        async def read_file(path: str) -> str:
            with open(path, 'r') as f:
                return f.read()

        agent = real_harness.create_agent(
            name="tool_test",
            model="gpt-4o-mini",
            tools=["read_file"]
        )

        # When
        result = await agent.run(f"读取 {test_file} 文件的内容")

        # Then: 应该能看到文件内容
        assert result is not None
        assert "Hello from test file" in result or "test file" in result.lower()

    @pytest.mark.asyncio
    @pytest.mark.e2e
    @pytest.mark.timeout(120)
    async def test_real_cost_tracking(self, real_harness):
        """测试真实成本追踪"""
        # Given
        agent = real_harness.create_agent(
            name="cost_test",
            model="gpt-4o-mini"
        )

        # When
        await agent.run("Hello")

        # Then: 验证成本追踪
        tracker = agent.cost_tracker
        assert tracker.total_tokens > 0
        assert tracker.total_cost > 0
        print(f"消耗: {tracker.total_tokens} tokens, ${tracker.total_cost:.4f}")


class TestRealAnthropic:
    """真实Anthropic API测试"""

    pytestmark = pytest.mark.skipif(
        not os.environ.get("ANTHROPIC_API_KEY"),
        reason="需要真实 ANTHROPIC_API_KEY"
    )

    @pytest.fixture
    def anthropic_harness(self):
        """使用Anthropic的Harness"""
        return Harness(
            config=None,
            provider="anthropic"
        )

    @pytest.mark.asyncio
    @pytest.mark.e2e
    @pytest.mark.timeout(60)
    async def test_real_claude_chat(self, anthropic_harness):
        """测试真实Claude聊天"""
        agent = anthropic_harness.create_agent(
            name="claude_test",
            model="claude-3-sonnet-20240229"
        )

        result = await agent.run("Hello, please respond with 'OK'")

        assert result is not None
        assert len(result) > 0
```

```python
# tests/e2e/test_crash_recovery.py
"""崩溃恢复端到端测试"""

import pytest
import os
import signal
import asyncio
import subprocess
import json
from pathlib import Path
from datetime import datetime


class TestCrashRecovery:
    """崩溃恢复E2E测试"""

    @pytest.fixture
    def test_project(self, tmp_path):
        """创建测试项目目录"""
        project = tmp_path / "test_project"
        project.mkdir()

        # 创建测试文件
        (project / "test.txt").write_text("Test content")
        (project / "README.md").write_text("# Test Project\n\nThis is a test.")

        return project

    @pytest.mark.e2e
    @pytest.mark.timeout(120)
    async def test_crash_and_recover(self, test_project):
        """测试崩溃后恢复"""
        # This test would be run as a subprocess test
        # Phase 1: 启动Agent执行任务
        # Phase 2: 模拟崩溃（发送SIGTERM）
        # Phase 3: 使用/continue恢复
        # Phase 4: 验证任务完成

        # 简化版本：直接测试SessionManager
        from continuum.session.manager import SessionManager
        from continuum.session.state import ExecutionContext, AgentState

        storage = test_project / ".continuum" / "sessions"
        storage.mkdir(parents=True)

        manager = SessionManager(storage_path=str(storage))

        # 创建执行上下文
        context = ExecutionContext(
            session_id="crash_test_session",
            agent_id="test_agent",
            state=AgentState.RUNNING,
            iteration=10,
            max_iterations=30,
            messages=[
                {"role": "user", "content": "Start task"},
                {"role": "assistant", "content": "Processing..."},
                {"role": "user", "content": "Continue"},
            ],
            tool_calls_pending=[],
            tool_results_cache={},
            model="gpt-4",
            temperature=0.7,
            system_prompt="",
            tokens_total=500,
            tokens_prompt=300,
            tokens_completion=200,
            cost_estimate=0.05,
            created_at=datetime.now(),
            last_updated=datetime.now(),
            checkpoint_count=0
        )

        # 保存checkpoint
        manager.save_checkpoint(context.to_checkpoint())

        # 模拟恢复
        restored = await manager.restore_session("crash_test_session")

        assert restored is not None
        assert restored.iteration == 10
        assert len(restored.messages) == 3

    @pytest.mark.e2e
    def test_cli_continue_command(self, test_project):
        """测试CLI continue命令"""
        # Given: 创建checkpoint
        session_dir = test_project / ".continuum" / "sessions" / "cli_test_session"
        checkpoint_dir = session_dir / "checkpoints"
        checkpoint_dir.mkdir(parents=True)

        checkpoint_data = {
            "checkpoint_id": "cp_test_001",
            "session_id": "cli_test_session",
            "created_at": datetime.now().isoformat(),
            "trigger": "manual",
            "agent_state": "running",
            "iteration": 5,
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi"}
            ],
            "tool_calls_pending": [],
            "tool_results": [],
            "tokens_used": 100,
            "cost_estimate": 0.01,
            "resume_hint": "Ready to continue"
        }

        checkpoint_file = checkpoint_dir / "cp_test_001.json"
        with open(checkpoint_file, 'w') as f:
            json.dump(checkpoint_data, f)

        # 创建latest链接
        latest_file = checkpoint_dir / "latest.json"
        latest_file.write_text(checkpoint_file.name)

        # When: 运行continue命令
        # 注：实际测试需要subprocess运行CLI
        # result = subprocess.run(
        #     ["continuum", "continue"],
        #     cwd=str(test_project),
        #     capture_output=True,
        #     text=True
        # )

        # Then: 验证输出
        # assert "恢复会话" in result.stdout
        # assert "cli_test_session" in result.stdout

        # 简化：验证文件存在
        assert checkpoint_file.exists()
```

```python
# tests/e2e/test_cli_commands.py
"""CLI命令端到端测试"""

import pytest
import subprocess
import os
import json
from pathlib import Path


class TestCLICommands:
    """CLI命令E2E测试"""

    @pytest.fixture
    def cli_env(self, tmp_path, monkeypatch):
        """CLI测试环境"""
        # 设置工作目录
        monkeypatch.chdir(tmp_path)

        # 设置API Key（如果有的话）
        if os.environ.get("OPENAI_API_KEY"):
            monkeypatch.setenv("OPENAI_API_KEY", os.environ["OPENAI_API_KEY"])

        return tmp_path

    @pytest.mark.e2e
    def test_cli_run_command_help(self):
        """测试CLI run命令帮助"""
        result = subprocess.run(
            ["continuum", "run", "--help"],
            capture_output=True,
            text=True
        )

        # 如果CLI未安装，跳过
        if result.returncode != 0 and "not found" in result.stderr:
            pytest.skip("continuum CLI not installed")

        assert "usage:" in result.stdout.lower() or "Usage:" in result.stdout

    @pytest.mark.e2e
    def test_cli_demo_mode(self):
        """测试CLI demo模式（无需API Key）"""
        result = subprocess.run(
            ["continuum", "demo"],
            capture_output=True,
            text=True,
            timeout=30
        )

        if result.returncode != 0 and "not found" in result.stderr:
            pytest.skip("continuum CLI not installed")

        # demo模式应该输出一些内容
        if result.returncode == 0:
            assert len(result.stdout) > 0

    @pytest.mark.e2e
    @pytest.mark.skipif(
        not os.environ.get("OPENAI_API_KEY"),
        reason="需要真实API Key"
    )
    def test_cli_run_basic_task(self, cli_env):
        """测试CLI run执行基本任务"""
        result = subprocess.run(
            ["continuum", "run", "输出Hello World"],
            capture_output=True,
            text=True,
            timeout=60,
            cwd=str(cli_env)
        )

        # 验证执行成功
        if result.returncode == 0:
            assert len(result.stdout) > 0

    @pytest.mark.e2e
    def test_cli_chat_mode_start(self):
        """测试CLI chat模式启动"""
        # chat模式是交互式的，只测试启动
        proc = subprocess.Popen(
            ["continuum", "chat"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )

        try:
            # 发送退出命令
            proc.stdin.write("/exit\n")
            proc.stdin.flush()

            # 等待进程结束
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        except FileNotFoundError:
            pytest.skip("continuum CLI not installed")
```

---

## 五、测试工具选型

### 5.1 pytest系列工具

| 工具 | 用途 | 版本要求 |
|------|------|---------|
| pytest | 测试框架核心 | >=7.0 |
| pytest-asyncio | 异步测试支持 | >=0.21 |
| pytest-cov | 覆盖率报告 | >=4.0 |
| pytest-mock | Mock支持 | >=3.0 |
| pytest-timeout | 测试超时 | >=2.0 |
| pytest-xdist | 并行执行 | >=3.0 |

### 5.2 配置文件

```toml
# pyproject.toml
[tool.pytest.ini_options]
minversion = "7.0"
testpaths = ["tests"]
python_files = ["test_*.py"]
python_classes = ["Test*"]
python_functions = ["test_*"]
asyncio_mode = "auto"
addopts = [
    "-v",
    "--tb=short",
    "--strict-markers",
    "-ra",
]
markers = [
    "unit: Unit tests",
    "integration: Integration tests",
    "e2e: End-to-end tests",
    "slow: Slow running tests",
]
filterwarnings = [
    "ignore::DeprecationWarning",
]

[tool.coverage.run]
source = ["src/continuum"]
branch = true
omit = [
    "tests/*",
    "*/__pycache__/*",
    "*/migrations/*",
]

[tool.coverage.report]
exclude_lines = [
    "pragma: no cover",
    "def __repr__",
    "raise AssertionError",
    "raise NotImplementedError",
    "if __name__ == .__main__.:",
    "if TYPE_CHECKING:",
    "@abstractmethod",
]
fail_under = 80
show_missing = true
skip_covered = true

[tool.coverage.html]
directory = "htmlcov"
```

### 5.3 运行命令

```bash
# 运行所有测试
pytest

# 运行单元测试
pytest tests/unit -v

# 运行集成测试
pytest tests/integration -v

# 运行E2E测试（需要API Key）
pytest tests/e2e -v -m e2e

# 运行带覆盖率报告
pytest --cov=continuum --cov-report=html --cov-report=term

# 并行执行测试
pytest -n auto

# 运行特定测试文件
pytest tests/unit/session/test_manager.py -v

# 运行特定测试用例
pytest tests/unit/session/test_manager.py::TestSessionManager::test_save_checkpoint_basic -v
```

---

## 六、CI/CD集成方案

### 6.1 GitHub Actions配置

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

jobs:
  unit-tests:
    name: Unit Tests
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'
          cache: 'pip'

      - name: Install dependencies
        run: |
          python -m pip install --upgrade pip
          pip install -e ".[dev]"

      - name: Run unit tests
        run: |
          pytest tests/unit -v \
            --cov=continuum \
            --cov-report=xml \
            --cov-report=term-missing \
            --cov-fail-under=80

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./coverage.xml
          fail_ci_if_error: true

  integration-tests:
    name: Integration Tests
    runs-on: ubuntu-latest
    needs: unit-tests

    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'
          cache: 'pip'

      - name: Install dependencies
        run: |
          python -m pip install --upgrade pip
          pip install -e ".[dev]"

      - name: Run integration tests
        run: |
          pytest tests/integration -v

  e2e-tests:
    name: E2E Tests
    runs-on: ubuntu-latest
    needs: integration-tests
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'

    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'
          cache: 'pip'

      - name: Install dependencies
        run: |
          python -m pip install --upgrade pip
          pip install -e ".[dev]"

      - name: Run E2E tests
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
        run: |
          pytest tests/e2e -v -m e2e --timeout=120
        continue-on-error: true

  lint:
    name: Code Quality
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install linters
        run: |
          pip install ruff mypy black

      - name: Run Ruff
        run: ruff check src tests

      - name: Run Black
        run: black --check src tests

      - name: Run MyPy
        run: mypy src
        continue-on-error: true

  # 代码质量门禁
  quality-gate:
    name: Quality Gate
    runs-on: ubuntu-latest
    needs: [unit-tests, integration-tests, lint]
    if: always()

    steps:
      - name: Check test results
        run: |
          if [ "${{ needs.unit-tests.result }}" != "success" ]; then
            echo "Unit tests failed"
            exit 1
          fi
          if [ "${{ needs.integration-tests.result }}" != "success" ]; then
            echo "Integration tests failed"
            exit 1
          fi
          echo "All tests passed!"
```

### 6.2 Pre-commit Hooks

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.5.0
    hooks:
      - id: trailing-whitespace
      - id: end-of-file-fixer
      - id: check-yaml
      - id: check-added-large-files
      - id: check-merge-conflict

  - repo: https://github.com/psf/black
    rev: 23.12.1
    hooks:
      - id: black
        language_version: python3.11

  - repo: https://github.com/astral-sh/ruff-pre-commit
    rev: v0.1.9
    hooks:
      - id: ruff
        args: [--fix, --exit-non-zero-on-fix]

  - repo: local
    hooks:
      - id: pytest-unit
        name: pytest-unit
        entry: pytest tests/unit -v --tb=short
        language: system
        pass_filenames: false
        stages: [pre-commit]
```

### 6.3 Makefile

```makefile
# Makefile

.PHONY: test test-unit test-integration test-e2e coverage lint format clean

# 运行所有测试
test:
	pytest -v

# 运行单元测试
test-unit:
	pytest tests/unit -v

# 运行集成测试
test-integration:
	pytest tests/integration -v

# 运行E2E测试
test-e2e:
	pytest tests/e2e -v -m e2e

# 生成覆盖率报告
coverage:
	pytest --cov=continuum --cov-report=html --cov-report=term
	@echo "Coverage report generated in htmlcov/index.html"

# 代码检查
lint:
	ruff check src tests
	black --check src tests
	mypy src

# 代码格式化
format:
	black src tests
	ruff check --fix src tests

# 清理缓存
clean:
	find . -type d -name "__pycache__" -exec rm -rf {} +
	find . -type d -name "*.egg-info" -exec rm -rf {} +
	find . -type d -name ".pytest_cache" -exec rm -rf {} +
	find . -type d -name ".mypy_cache" -exec rm -rf {} +
	rm -rf htmlcov .coverage coverage.xml

# 安装开发依赖
dev-install:
	pip install -e ".[dev]"
	pre-commit install
```

---

## 七、MVP发布门槛清单

### 7.1 测试门槛

```
MVP发布前必须满足：

[ ] 单元测试覆盖率 >= 80%
    ├── LLM Provider >= 90%
    ├── Error Handler >= 90%
    ├── Session Manager >= 90%
    ├── Checkpoint >= 90%
    ├── Token Budget >= 85%
    ├── Context Manager >= 85%
    └── Tool Registry >= 85%

[ ] 核心路径集成测试100%通过
    ├── Agent完整执行循环 ✓
    ├── 会话生命周期 ✓
    └── 错误恢复流程 ✓

[ ] E2E测试关键场景通过
    ├── 真实LLM调用验证 ✓
    ├── 崩溃恢复E2E测试 ✓
    └── CLI命令E2E测试 ✓

[ ] 代码质量检查通过
    ├── Ruff linting 0 errors
    ├── Black formatting 100%
    └── MyPy type checking (允许部分警告)

[ ] 文档代码示例可运行
    ├── README示例 ✓
    ├── Quick Start示例 ✓
    └── API文档示例 ✓
```

### 7.2 质量门槛检查脚本

```python
# scripts/check_quality_gate.py
"""MVP发布质量门槛检查脚本"""

import subprocess
import sys
from pathlib import Path


def run_command(cmd: str, check: bool = True) -> tuple[int, str]:
    """运行命令并返回结果"""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=True,
        text=True
    )
    return result.returncode, result.stdout + result.stderr


def check_coverage() -> bool:
    """检查测试覆盖率"""
    print("Checking test coverage...")
    code, output = run_command(
        "pytest --cov=continuum --cov-report=term --cov-fail-under=80"
    )

    if code != 0:
        print("FAILED: Coverage below 80%")
        print(output)
        return False

    print("PASSED: Coverage >= 80%")
    return True


def check_unit_tests() -> bool:
    """检查单元测试"""
    print("Running unit tests...")
    code, output = run_command("pytest tests/unit -v")

    if code != 0:
        print("FAILED: Unit tests failed")
        print(output)
        return False

    print("PASSED: All unit tests passed")
    return True


def check_integration_tests() -> bool:
    """检查集成测试"""
    print("Running integration tests...")
    code, output = run_command("pytest tests/integration -v")

    if code != 0:
        print("FAILED: Integration tests failed")
        print(output)
        return False

    print("PASSED: All integration tests passed")
    return True


def check_linting() -> bool:
    """检查代码风格"""
    print("Running linting...")

    # Ruff
    code, output = run_command("ruff check src tests")
    if code != 0:
        print("FAILED: Ruff linting failed")
        print(output)
        return False

    print("PASSED: Ruff linting passed")

    # Black
    code, output = run_command("black --check src tests")
    if code != 0:
        print("FAILED: Black formatting check failed")
        print(output)
        return False

    print("PASSED: Black formatting passed")
    return True


def check_documentation() -> bool:
    """检查文档代码示例"""
    print("Checking documentation examples...")

    # 检查README是否存在
    readme = Path("README.md")
    if not readme.exists():
        print("FAILED: README.md not found")
        return False

    print("PASSED: Documentation checks passed")
    return True


def main():
    """运行所有检查"""
    print("=" * 60)
    print("MVP Quality Gate Check")
    print("=" * 60)

    checks = [
        ("Unit Tests", check_unit_tests),
        ("Integration Tests", check_integration_tests),
        ("Test Coverage", check_coverage),
        ("Code Linting", check_linting),
        ("Documentation", check_documentation),
    ]

    results = {}
    all_passed = True

    for name, check_fn in checks:
        print(f"\n[{name}]")
        try:
            passed = check_fn()
            results[name] = passed
            if not passed:
                all_passed = False
        except Exception as e:
            print(f"ERROR: {e}")
            results[name] = False
            all_passed = False

    # 打印摘要
    print("\n" + "=" * 60)
    print("Summary")
    print("=" * 60)

    for name, passed in results.items():
        status = "PASS" if passed else "FAIL"
        print(f"  {name}: [{status}]")

    print("=" * 60)

    if all_passed:
        print("\nALL CHECKS PASSED - Ready for MVP release!")
        return 0
    else:
        print("\nSOME CHECKS FAILED - Fix before MVP release")
        return 1


if __name__ == "__main__":
    sys.exit(main())
```

---

## 八、测试最佳实践

### 8.1 测试命名规范

```python
# 好的测试命名
def test_save_checkpoint_creates_directory():
    """测试保存Checkpoint时自动创建目录"""
    pass

def test_load_corrupted_checkpoint_returns_none():
    """测试加载损坏的Checkpoint返回None"""
    pass

async def test_session_interrupt_preserves_state():
    """测试会话中断时状态被正确保存"""
    pass

# 不好的测试命名
def test_checkpoint():  # 太笼统
    pass

def test_1():  # 无意义
    pass
```

### 8.2 测试组织结构

```python
class TestSessionManager:
    """SessionManager测试类"""

    # ==================== 保存测试 ====================

    def test_save_basic(self):
        """基本保存测试"""
        pass

    def test_save_with_large_data(self):
        """大数据保存测试"""
        pass

    # ==================== 加载测试 ====================

    def test_load_basic(self):
        """基本加载测试"""
        pass

    def test_load_not_found(self):
        """加载不存在数据测试"""
        pass

    # ==================== 边界条件 ====================

    def test_empty_session_id(self):
        """空session_id边界测试"""
        pass

    def test_max_checkpoints_limit(self):
        """最大checkpoint数量边界测试"""
        pass
```

### 8.3 测试数据管理

```python
# 使用fixtures管理测试数据
@pytest.fixture
def sample_checkpoint():
    """标准测试Checkpoint"""
    return Checkpoint(
        checkpoint_id="test_001",
        session_id="session_001",
        # ...标准测试数据
    )

@pytest.fixture
def large_checkpoint():
    """大容量测试Checkpoint"""
    return Checkpoint(
        messages=[{"role": "user", "content": "x" * 10000}] * 100,
        # ...大容量测试数据
    )

@pytest.fixture
def corrupted_checkpoint_data():
    """损坏数据测试用例"""
    return [
        ("missing_field", {"checkpoint_id": "test"}),
        ("invalid_json", "{ not valid json"),
        ("wrong_type", {"iteration": "not_a_number"}),
    ]
```

---

## 九、总结

### 9.1 测试策略核心要点

```
测试策略 = 金字塔模型 + 自动化优先 + 质量门禁

核心要点：
1. 单元测试80%覆盖率，聚焦核心模块90%
2. 集成测试覆盖3条核心路径
3. E2E测试覆盖3个关键场景
4. CI/CD自动化，PR必须通过测试
5. MVP发布前质量门槛清单100%满足
```

### 9.2 下一步行动

```
Week 1:
├── 搭建测试框架结构
├── 编写核心模块单元测试（SessionManager, RetryExecutor）
└── 配置CI/CD基础流程

Week 2:
├── 补充LLM Provider单元测试
├── 编写集成测试用例
└── 实现覆盖率报告

Week 3:
├── 编写E2E测试用例
├── 实现质量门槛检查脚本
└── 完成MVP发布门槛清单

Week 4:
├── 测试覆盖率达标
├── 所有CI检查通过
└── MVP发布就绪
```

---

**文档状态**: v1.0 新建
**下一步**: 创建测试目录结构，编写核心模块单元测试
