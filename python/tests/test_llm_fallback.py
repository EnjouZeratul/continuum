"""
FallbackLlmClient Tests / FallbackLlmClient 测试

Tests for the fallback client with provider-level redundancy.
测试具有提供商级冗余的回退客户端。

Coverage:
- Primary client success returns response / 主客户端成功时返回响应
- Fallback to backup client on transient errors / 临时错误时回退到备用客户端
- All clients failure raises error / 所有客户端失败时抛出错误
- Log message verification / 日志消息验证
"""

from unittest.mock import AsyncMock, Mock

import pytest

from continuum_sdk.llm.errors import (
    AuthenticationError,
    LlmError,
    NetworkError,
    RateLimitError,
    TimeoutError,
)
from continuum_sdk.llm.fallback import (
    FallbackConfig,
    FallbackEvent,
    FallbackEventType,
    FallbackLlmClient,
    create_fallback_client,
)
from continuum_sdk.llm.types import ChatResponse, Message, TokenUsage

# ==================== Fixtures ====================


@pytest.fixture
def mock_chat_response():
    """Create a mock successful chat response."""
    return ChatResponse(
        content="Hello! How can I help you?",
        model="claude-sonnet-4-6",
        usage=TokenUsage(input_tokens=10, output_tokens=20),
        finish_reason="stop",
    )


@pytest.fixture
def fallback_config():
    """Create a basic fallback configuration."""
    return FallbackConfig(
        primary_provider="anthropic",
        fallback_providers=["openai", "gemini"],
        api_keys={
            "anthropic": "sk-ant-test",
            "openai": "sk-openai-test",
            "gemini": "gemini-test-key",
        },
        max_retries=2,
        initial_delay_ms=100.0,
        max_delay_ms=1000.0,
        backoff_multiplier=2.0,
    )


@pytest.fixture
def fallback_client(fallback_config):
    """Create a FallbackLlmClient with mocked internal clients."""
    client = FallbackLlmClient(fallback_config)
    # Replace real clients with mocks
    client._clients = {
        "anthropic": Mock(),
        "openai": Mock(),
        "gemini": Mock(),
    }
    return client


# ==================== Primary Client Success Tests ====================


class TestPrimaryClientSuccess:
    """Tests for successful primary client responses."""

    @pytest.mark.asyncio
    async def test_primary_success_returns_response(
        self, fallback_client, mock_chat_response
    ):
        """
        Test that successful primary client returns response immediately.
        测试主客户端成功时立即返回响应。
        """
        # Mock primary client to succeed
        fallback_client._clients["anthropic"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        messages = [Message.user("Hello")]
        result = await fallback_client.chat(messages)

        assert result.content == "Hello! How can I help you?"
        assert result.model == "claude-sonnet-4-6"
        # Verify only primary client was called
        fallback_client._clients["anthropic"].chat.assert_called_once()
        fallback_client._clients["openai"].chat.assert_not_called()
        fallback_client._clients["gemini"].chat.assert_not_called()

    @pytest.mark.asyncio
    async def test_primary_success_no_fallback_events(
        self, fallback_client, mock_chat_response
    ):
        """
        Test that successful primary client doesn't trigger fallback events.
        测试主客户端成功时不触发回退事件。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        await fallback_client.chat([Message.user("Hello")])

        event_log = fallback_client.get_event_log()
        assert len(event_log) == 0

    @pytest.mark.asyncio
    async def test_primary_success_with_system_prompt(
        self, fallback_client, mock_chat_response
    ):
        """
        Test that system prompt is passed correctly to primary client.
        测试系统提示正确传递给主客户端。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        await fallback_client.chat(
            [Message.user("Hello")], system_prompt="Be helpful"
        )

        call_args = fallback_client._clients["anthropic"].chat.call_args
        assert call_args.kwargs["system_prompt"] == "Be helpful"


# ==================== Fallback to Backup Tests ====================


class TestFallbackToBackup:
    """Tests for fallback behavior when primary fails."""

    @pytest.mark.asyncio
    async def test_fallback_on_rate_limit_error(
        self, fallback_client, mock_chat_response
    ):
        """
        Test fallback to backup client on RateLimitError.
        测试 RateLimitError 时回退到备用客户端。
        """
        # Primary fails with rate limit
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limit exceeded")
        )
        # First fallback succeeds
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        result = await fallback_client.chat([Message.user("Hello")])

        assert result.content == "Hello! How can I help you?"
        # Verify both primary and fallback were called
        fallback_client._clients["anthropic"].chat.assert_called()
        fallback_client._clients["openai"].chat.assert_called_once()

    @pytest.mark.asyncio
    async def test_fallback_on_network_error(
        self, fallback_client, mock_chat_response
    ):
        """
        Test fallback to backup client on NetworkError.
        测试 NetworkError 时回退到备用客户端。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=NetworkError("Connection failed")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        result = await fallback_client.chat([Message.user("Hello")])

        assert result.content == "Hello! How can I help you?"
        fallback_client._clients["openai"].chat.assert_called_once()

    @pytest.mark.asyncio
    async def test_fallback_on_timeout_error(
        self, fallback_client, mock_chat_response
    ):
        """
        Test fallback to backup client on TimeoutError.
        测试 TimeoutError 时回退到备用客户端。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=TimeoutError("Request timed out")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        result = await fallback_client.chat([Message.user("Hello")])

        assert result is not None
        fallback_client._clients["openai"].chat.assert_called_once()

    @pytest.mark.asyncio
    async def test_fallback_on_500_error(self, fallback_client, mock_chat_response):
        """
        Test fallback on 500 server error.
        测试 500 服务器错误时回退。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=LlmError("[anthropic] 500 Internal Server Error")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        result = await fallback_client.chat([Message.user("Hello")])

        assert result is not None
        fallback_client._clients["openai"].chat.assert_called_once()

    @pytest.mark.asyncio
    async def test_fallback_chain_to_second_backup(
        self, fallback_client, mock_chat_response
    ):
        """
        Test fallback chain: primary -> first backup -> second backup.
        测试回退链：主客户端 -> 第一个备用 -> 第二个备用。
        """
        # Both primary and first backup fail
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limited")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            side_effect=NetworkError("Network error")
        )
        # Second backup succeeds
        fallback_client._clients["gemini"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        result = await fallback_client.chat([Message.user("Hello")])

        assert result.content == "Hello! How can I help you?"
        # All clients were tried in order
        fallback_client._clients["anthropic"].chat.assert_called()
        fallback_client._clients["openai"].chat.assert_called()
        fallback_client._clients["gemini"].chat.assert_called_once()

    @pytest.mark.asyncio
    async def test_fallback_logs_provider_switch_event(
        self, fallback_client, mock_chat_response
    ):
        """
        Test that provider switch events are logged.
        测试提供商切换事件被记录。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limited")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        await fallback_client.chat([Message.user("Hello")])

        event_log = fallback_client.get_event_log()
        # Should have provider_switch event
        switch_events = [
            e
            for e in event_log
            if e.event_type == FallbackEventType.PROVIDER_SWITCH
        ]
        assert len(switch_events) >= 1
        assert switch_events[0].provider == "anthropic"
        assert switch_events[0].next_provider == "openai"

    @pytest.mark.asyncio
    async def test_fallback_emits_degradation_notice(
        self, fallback_client, mock_chat_response
    ):
        """
        Test that degradation notice is emitted on fallback.
        测试回退时发出降级通知。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limited")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        await fallback_client.chat([Message.user("Hello")])

        event_log = fallback_client.get_event_log()
        degradation_events = [
            e
            for e in event_log
            if e.event_type == FallbackEventType.DEGRADATION_NOTICE
        ]
        assert len(degradation_events) >= 1
        assert "Service degradation" in degradation_events[0].message


# ==================== No Fallback on Permanent Errors ====================


class TestNoFallbackOnPermanentErrors:
    """Tests that permanent errors don't trigger fallback."""

    @pytest.mark.asyncio
    async def test_no_fallback_on_auth_error(self, fallback_client):
        """
        Test that AuthenticationError doesn't trigger fallback.
        测试 AuthenticationError 不触发回退。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=AuthenticationError("Invalid API key")
        )

        with pytest.raises(AuthenticationError):
            await fallback_client.chat([Message.user("Hello")])

        # Only primary should be called
        fallback_client._clients["anthropic"].chat.assert_called()
        fallback_client._clients["openai"].chat.assert_not_called()

    @pytest.mark.asyncio
    async def test_no_fallback_on_model_not_found(self, fallback_client):
        """
        Test that ModelNotFoundError doesn't trigger fallback.
        测试 ModelNotFoundError 不触发回退。
        """
        from continuum_sdk.llm.errors import ModelNotFoundError

        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=ModelNotFoundError("Model not found")
        )

        with pytest.raises(ModelNotFoundError):
            await fallback_client.chat([Message.user("Hello")])

        fallback_client._clients["openai"].chat.assert_not_called()

    @pytest.mark.asyncio
    async def test_no_fallback_on_content_filter(self, fallback_client):
        """
        Test that ContentFilterError doesn't trigger fallback.
        测试 ContentFilterError 不触发回退。
        """
        from continuum_sdk.llm.errors import ContentFilterError

        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=ContentFilterError("Content blocked")
        )

        with pytest.raises(ContentFilterError):
            await fallback_client.chat([Message.user("Hello")])

        fallback_client._clients["openai"].chat.assert_not_called()


# ==================== All Clients Fail Tests ====================


class TestAllClientsFail:
    """Tests for when all clients fail."""

    @pytest.mark.asyncio
    async def test_all_clients_fail_raises_error(self, fallback_client):
        """
        Test that error is raised when all clients fail.
        测试所有客户端失败时抛出错误。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limited")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            side_effect=NetworkError("Network error")
        )
        fallback_client._clients["gemini"].chat = AsyncMock(
            side_effect=TimeoutError("Timeout")
        )

        with pytest.raises(LlmError):
            await fallback_client.chat([Message.user("Hello")])

    @pytest.mark.asyncio
    async def test_all_clients_fail_logs_event(self, fallback_client):
        """
        Test that ALL_PROVIDERS_FAILED event is logged.
        测试 ALL_PROVIDERS_FAILED 事件被记录。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limited")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            side_effect=NetworkError("Network error")
        )
        fallback_client._clients["gemini"].chat = AsyncMock(
            side_effect=TimeoutError("Timeout")
        )

        with pytest.raises(LlmError):
            await fallback_client.chat([Message.user("Hello")])

        event_log = fallback_client.get_event_log()
        failed_events = [
            e
            for e in event_log
            if e.event_type == FallbackEventType.ALL_PROVIDERS_FAILED
        ]
        assert len(failed_events) == 1
        assert failed_events[0].provider == "all"


# ==================== Retry Logic Tests ====================


class TestRetryLogic:
    """Tests for retry behavior."""

    @pytest.mark.asyncio
    async def test_retry_before_fallback(self, fallback_client, mock_chat_response):
        """
        Test that retry happens before fallback to next provider.
        测试在回退到下一个提供商之前进行重试。
        """
        call_count = [0]

        async def flaky_chat(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] < 2:
                raise RateLimitError("Temporary rate limit")
            return mock_chat_response

        fallback_client._clients["anthropic"].chat = flaky_chat

        result = await fallback_client.chat([Message.user("Hello")])

        assert result.content == "Hello! How can I help you?"
        assert call_count[0] == 2  # Failed once, succeeded on retry

    @pytest.mark.asyncio
    async def test_retry_logs_event(self, fallback_client, mock_chat_response):
        """
        Test that retry attempts are logged.
        测试重试尝试被记录。
        """
        call_count = [0]

        async def flaky_chat(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] < 2:
                raise RateLimitError("Temporary rate limit")
            return mock_chat_response

        fallback_client._clients["anthropic"].chat = flaky_chat

        await fallback_client.chat([Message.user("Hello")])

        event_log = fallback_client.get_event_log()
        retry_events = [
            e for e in event_log if e.event_type == FallbackEventType.RETRY
        ]
        assert len(retry_events) == 1
        assert retry_events[0].attempt == 1

    @pytest.mark.asyncio
    async def test_max_retries_exceeded_falls_back(
        self, fallback_client, mock_chat_response
    ):
        """
        Test that after max retries, fallback to next provider occurs.
        测试达到最大重试次数后回退到下一个提供商。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Persistent rate limit")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            return_value=mock_chat_response
        )

        result = await fallback_client.chat([Message.user("Hello")])

        assert result.content == "Hello! How can I help you?"
        # Primary should be called max_retries times
        assert fallback_client._clients["anthropic"].chat.call_count == 2

    @pytest.mark.asyncio
    async def test_max_retries_exceeded_logs_event(self, fallback_client):
        """
        Test that MAX_RETRIES_EXCEEDED event is logged.
        测试 MAX_RETRIES_EXCEEDED 事件被记录。
        """
        fallback_client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Persistent rate limit")
        )
        fallback_client._clients["openai"].chat = AsyncMock(
            side_effect=NetworkError("Network error")
        )
        fallback_client._clients["gemini"].chat = AsyncMock(
            side_effect=TimeoutError("Timeout")
        )

        with pytest.raises(LlmError):
            await fallback_client.chat([Message.user("Hello")])

        event_log = fallback_client.get_event_log()
        max_retry_events = [
            e
            for e in event_log
            if e.event_type == FallbackEventType.MAX_RETRIES_EXCEEDED
        ]
        # Each provider that fails after retries should have this event
        assert len(max_retry_events) >= 1


# ==================== Event Log Tests ====================


class TestEventLog:
    """Tests for event log functionality."""

    def test_get_event_log_returns_copy(self, fallback_client):
        """
        Test that get_event_log returns a copy of the log.
        测试 get_event_log 返回日志的副本。
        """
        fallback_client._event_log.append(
            FallbackEvent(
                event_type=FallbackEventType.RETRY,
                provider="anthropic",
                attempt=1,
            )
        )

        log1 = fallback_client.get_event_log()
        log2 = fallback_client.get_event_log()

        assert log1 == log2
        assert log1 is not log2  # Should be different list objects

    def test_clear_event_log(self, fallback_client):
        """
        Test that clear_event_log removes all events.
        测试 clear_event_log 移除所有事件。
        """
        fallback_client._event_log.append(
            FallbackEvent(
                event_type=FallbackEventType.RETRY,
                provider="anthropic",
                attempt=1,
            )
        )

        fallback_client.clear_event_log()

        assert len(fallback_client.get_event_log()) == 0


# ==================== Callback Tests ====================


class TestCallback:
    """Tests for on_fallback callback."""

    @pytest.mark.asyncio
    async def test_callback_invoked_on_fallback(self, mock_chat_response):
        """
        Test that on_fallback callback is invoked on fallback events.
        测试回退时调用 on_fallback 回调。
        """
        events = []

        def on_fallback(event):
            events.append(event)

        config = FallbackConfig(
            primary_provider="anthropic",
            fallback_providers=["openai"],
            api_keys={"anthropic": "sk-test", "openai": "sk-test"},
            on_fallback=on_fallback,
        )
        client = FallbackLlmClient(config)
        client._clients = {
            "anthropic": Mock(),
            "openai": Mock(),
        }
        client._clients["anthropic"].chat = AsyncMock(
            side_effect=RateLimitError("Rate limited")
        )
        client._clients["openai"].chat = AsyncMock(return_value=mock_chat_response)

        await client.chat([Message.user("Hello")])

        assert len(events) >= 1
        assert any(e.event_type == FallbackEventType.PROVIDER_SWITCH for e in events)


# ==================== Configuration Tests ====================


class TestFallbackConfig:
    """Tests for FallbackConfig."""

    def test_default_values(self):
        """
        Test default configuration values.
        测试默认配置值。
        """
        config = FallbackConfig(
            primary_provider="anthropic",
            api_keys={"anthropic": "sk-test"},
        )

        assert config.fallback_providers == []
        assert config.max_retries == 3
        assert config.initial_delay_ms == 1000.0
        assert config.max_delay_ms == 30000.0
        assert config.backoff_multiplier == 2.0

    def test_custom_retry_settings(self):
        """
        Test custom retry settings.
        测试自定义重试设置。
        """
        config = FallbackConfig(
            primary_provider="anthropic",
            api_keys={"anthropic": "sk-test"},
            max_retries=5,
            initial_delay_ms=500.0,
            max_delay_ms=60000.0,
            backoff_multiplier=1.5,
        )

        assert config.max_retries == 5
        assert config.initial_delay_ms == 500.0
        assert config.max_delay_ms == 60000.0
        assert config.backoff_multiplier == 1.5


class TestCreateFallbackClient:
    """Tests for create_fallback_client helper function."""

    def test_create_from_dict(self):
        """
        Test creating client from configuration dictionary.
        测试从配置字典创建客户端。
        """
        config_dict = {
            "provider": {
                "primary": "anthropic",
                "fallback": ["openai"],
            },
            "api_keys": {
                "anthropic": "sk-ant-test",
                "openai": "sk-openai-test",
            },
            "retry": {
                "max_retries": 5,
                "initial_delay_ms": 500.0,
            },
        }

        client = create_fallback_client(config_dict)

        assert client.config.primary_provider == "anthropic"
        assert client.config.fallback_providers == ["openai"]
        assert client.config.max_retries == 5
        assert client.config.initial_delay_ms == 500.0

    def test_create_with_defaults(self):
        """
        Test creating client with default values.
        测试使用默认值创建客户端。
        """
        config_dict = {}

        client = create_fallback_client(config_dict)

        assert client.config.primary_provider == "anthropic"
        assert client.config.fallback_providers == []


# ==================== Client Lifecycle Tests ====================


class TestClientLifecycle:
    """Tests for client lifecycle management."""

    @pytest.mark.asyncio
    async def test_close_closes_all_clients(self, fallback_client):
        """
        Test that close() closes all underlying clients.
        测试 close() 关闭所有底层客户端。
        """
        for mock_client in fallback_client._clients.values():
            mock_client.close = AsyncMock()

        await fallback_client.close()

        for mock_client in fallback_client._clients.values():
            mock_client.close.assert_called_once()

    @pytest.mark.asyncio
    async def test_context_manager(self, fallback_config):
        """
        Test async context manager support.
        测试异步上下文管理器支持。
        """
        client = FallbackLlmClient(fallback_config)
        for mock_client in client._clients.values():
            mock_client.close = AsyncMock()

        async with client as c:
            assert c is client

        for mock_client in client._clients.values():
            mock_client.close.assert_called_once()


# ==================== Should Trigger Fallback Tests ====================


class TestShouldTriggerFallback:
    """Tests for _should_trigger_fallback method."""

    def test_rate_limit_triggers_fallback(self, fallback_client):
        """
        Test that RateLimitError triggers fallback.
        测试 RateLimitError 触发回退。
        """
        error = RateLimitError("Rate limit exceeded")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_network_error_triggers_fallback(self, fallback_client):
        """
        Test that NetworkError triggers fallback.
        测试 NetworkError 触发回退。
        """
        error = NetworkError("Connection failed")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_timeout_error_triggers_fallback(self, fallback_client):
        """
        Test that TimeoutError triggers fallback.
        测试 TimeoutError 触发回退。
        """
        error = TimeoutError("Request timed out")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_500_in_message_triggers_fallback(self, fallback_client):
        """
        Test that 500 in error message triggers fallback.
        测试错误消息中的 500 触发回退。
        """
        error = LlmError("500 Internal Server Error")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_overloaded_triggers_fallback(self, fallback_client):
        """
        Test that 'overloaded' in message triggers fallback.
        测试消息中的 'overloaded' 触发回退。
        """
        error = LlmError("Service overloaded")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_auth_error_no_fallback(self, fallback_client):
        """
        Test that AuthenticationError doesn't trigger fallback.
        测试 AuthenticationError 不触发回退。
        """
        error = AuthenticationError("Invalid API key")
        assert fallback_client._should_trigger_fallback(error) is False

    def test_model_not_found_no_fallback(self, fallback_client):
        """
        Test that ModelNotFoundError doesn't trigger fallback.
        测试 ModelNotFoundError 不触发回退。
        """
        from continuum_sdk.llm.errors import ModelNotFoundError

        error = ModelNotFoundError("Model not found")
        assert fallback_client._should_trigger_fallback(error) is False

    def test_content_filter_no_fallback(self, fallback_client):
        """
        Test that ContentFilterError doesn't trigger fallback.
        测试 ContentFilterError 不触发回退。
        """
        from continuum_sdk.llm.errors import ContentFilterError

        error = ContentFilterError("Content blocked")
        assert fallback_client._should_trigger_fallback(error) is False


# ==================== Delay Calculation Tests ====================


class TestDelayCalculation:
    """Tests for exponential backoff delay calculation."""

    def test_initial_delay(self, fallback_client):
        """
        Test initial delay is as configured.
        测试初始延迟符合配置。
        """
        delay = fallback_client._calculate_delay(1)
        assert delay == 100.0  # initial_delay_ms from fixture

    def test_exponential_backoff(self, fallback_client):
        """
        Test exponential backoff calculation.
        测试指数退避计算。
        """
        delay1 = fallback_client._calculate_delay(1)
        delay2 = fallback_client._calculate_delay(2)
        delay3 = fallback_client._calculate_delay(3)

        assert delay2 == delay1 * 2.0  # backoff_multiplier
        assert delay3 == delay2 * 2.0

    def test_max_delay_cap(self, fallback_client):
        """
        Test that delay is capped at max_delay_ms.
        测试延迟上限为 max_delay_ms。
        """
        # With max_delay_ms=1000.0, even high attempts should be capped
        delay = fallback_client._calculate_delay(10)
        assert delay <= 1000.0


# ==================== Streaming Tests ====================


class TestStreamingFallback:
    """Tests for streaming with fallback."""

    @pytest.mark.asyncio
    async def test_stream_success(self, fallback_client):
        """
        Test successful streaming response.
        测试成功的流式响应。
        """
        from continuum_sdk.llm.types import StreamChunk

        async def mock_stream(*args, **kwargs):
            yield StreamChunk(content="Hello")
            yield StreamChunk(content=" World")
            yield StreamChunk(finish_reason="stop")

        fallback_client._clients["anthropic"].chat_stream = mock_stream

        chunks = []
        async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 3
        assert chunks[0].content == "Hello"
        assert chunks[1].content == " World"

    @pytest.mark.asyncio
    async def test_stream_fallback_on_initial_error(self, fallback_client):
        """
        Test fallback on initial stream connection error.
        测试初始流连接错误时的回退。
        """
        from continuum_sdk.llm.types import StreamChunk

        # Primary fails immediately
        async def failing_stream(*args, **kwargs):
            raise RateLimitError("Rate limited")
            yield  # Make it a generator

        fallback_client._clients["anthropic"].chat_stream = failing_stream

        # Fallback succeeds
        async def success_stream(*args, **kwargs):
            yield StreamChunk(content="Fallback response")

        fallback_client._clients["openai"].chat_stream = success_stream

        chunks = []
        async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 1
        assert chunks[0].content == "Fallback response"


# ==================== Missing Client Tests ====================


class TestMissingClient:
    """Tests for handling missing client configurations."""

    @pytest.mark.asyncio
    async def test_skip_provider_without_client(self, mock_chat_response):
        """
        Test that providers without configured clients are skipped.
        测试跳过没有配置客户端的提供商。
        """
        config = FallbackConfig(
            primary_provider="anthropic",
            fallback_providers=["openai", "missing_provider"],
            api_keys={
                "anthropic": "sk-test",
                "openai": "sk-test",
                # missing_provider has no API key
            },
        )
        client = FallbackLlmClient(config)

        # Should have clients for anthropic and openai only
        assert "anthropic" in client._clients
        assert "openai" in client._clients
        assert "missing_provider" not in client._clients

    @pytest.mark.asyncio
    async def test_error_when_no_clients_available(self):
        """
        Test error when no clients are configured.
        测试没有配置客户端时的错误。
        """
        config = FallbackConfig(
            primary_provider="anthropic",
            fallback_providers=[],
            api_keys={},  # No API keys
        )
        client = FallbackLlmClient(config)

        with pytest.raises(LlmError, match="All providers failed"):
            await client.chat([Message.user("Hello")])

    @pytest.mark.asyncio
    async def test_execute_with_retry_no_client_configured(self):
        """
        Test _execute_with_retry raises error when client not configured.
        测试当客户端未配置时 _execute_with_retry 抛出错误。
        """
        config = FallbackConfig(
            primary_provider="anthropic",
            api_keys={"anthropic": "sk-test"},
        )
        client = FallbackLlmClient(config)

        # Try to execute with a provider that has no client
        with pytest.raises(LlmError, match="No client configured for provider"):
            await client._execute_with_retry(
                "nonexistent_provider",
                lambda c: c.chat(messages=[Message.user("Hi")])
            )


# ==================== Additional Should Trigger Fallback Tests ====================


class TestShouldTriggerFallbackExtended:
    """Extended tests for _should_trigger_fallback method."""

    def test_502_in_message_triggers_fallback(self, fallback_client):
        """
        Test that 502 in error message triggers fallback.
        测试错误消息中的 502 触发回退。
        """
        error = LlmError("502 Bad Gateway")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_503_in_message_triggers_fallback(self, fallback_client):
        """
        Test that 503 in error message triggers fallback.
        测试错误消息中的 503 触发回退。
        """
        error = LlmError("503 Service Unavailable")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_504_in_message_triggers_fallback(self, fallback_client):
        """
        Test that 504 in error message triggers fallback.
        测试错误消息中的 504 触发回退。
        """
        error = LlmError("504 Gateway Timeout")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_unknown_error_no_fallback(self, fallback_client):
        """
        Test that unknown LlmError without server codes doesn't trigger fallback.
        测试不含服务器错误码的未知 LlmError 不触发回退。
        """
        error = LlmError("Some unknown error")
        assert fallback_client._should_trigger_fallback(error) is False

    def test_llm_error_with_500_triggers_fallback_explicit(self, fallback_client):
        """
        Test LlmError with 500 code triggers fallback via the inner check.
        测试含 500 码的 LlmError 通过内部检查触发回退。
        """
        # This specifically tests line 191 (any() returning True)
        error = LlmError("Error 500")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_llm_error_without_server_codes_returns_false(self, fallback_client):
        """
        Test that LlmError without any server codes returns False (line 189->194).
        测试不含任何服务器码的 LlmError 返回 False（行 189->194）。
        """
        # This tests the branch where LlmError is detected but no server codes found
        error = LlmError("Random error message")
        assert fallback_client._should_trigger_fallback(error) is False

    def test_llm_error_with_mixed_case_server_code(self, fallback_client):
        """
        Test that server codes work with mixed case in error message.
        测试服务器码在错误消息中混合大小写时仍能工作。
        """
        # The check is case-insensitive due to error_str.lower()
        error = LlmError("HTTP 503 Service Unavailable")
        assert fallback_client._should_trigger_fallback(error) is True

    def test_non_llm_error_returns_false(self, fallback_client):
        """
        Test that non-LlmError returns False (branch 189->194 direct).
        测试非 LlmError 返回 False（分支 189->194 直接跳转）。
        """
        # Create a custom exception that's not an LlmError
        class CustomError(Exception):
            pass

        error = CustomError("Some custom error")
        # This tests the branch where isinstance(error, LlmError) is False
        # and we jump directly from line 189 to line 194
        assert fallback_client._should_trigger_fallback(error) is False


# ==================== Extended Streaming Tests ====================


class TestStreamingFallbackExtended:
    """Extended tests for streaming with fallback."""

    @pytest.mark.asyncio
    async def test_stream_skip_provider_without_client(self):
        """
        Test that streaming skips providers without clients.
        测试流式请求跳过没有客户端的提供商。
        """
        from continuum_sdk.llm.types import StreamChunk

        config = FallbackConfig(
            primary_provider="missing_primary",
            fallback_providers=["anthropic"],
            api_keys={
                "anthropic": "sk-test",
                # missing_primary has no API key
            },
        )
        client = FallbackLlmClient(config)

        # Mock the anthropic client
        client._clients["anthropic"] = Mock()
        async def success_stream(*args, **kwargs):
            yield StreamChunk(content="Fallback response")
        client._clients["anthropic"].chat_stream = success_stream

        chunks = []
        async for chunk in client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 1
        assert chunks[0].content == "Fallback response"

    @pytest.mark.asyncio
    async def test_stream_no_fallback_on_auth_error(self, fallback_client):
        """
        Test that streaming doesn't fallback on AuthenticationError.
        测试流式请求在 AuthenticationError 时不回退。
        """
        # Primary fails with auth error
        async def failing_stream(*args, **kwargs):
            raise AuthenticationError("Invalid API key")
            yield  # Make it a generator

        fallback_client._clients["anthropic"].chat_stream = failing_stream

        with pytest.raises(AuthenticationError):
            chunks = []
            async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
                chunks.append(chunk)

        # Fallback should not be called
        fallback_client._clients["openai"].chat.assert_not_called()

    @pytest.mark.asyncio
    async def test_stream_all_providers_fail(self, fallback_client):
        """
        Test that streaming raises error when all providers fail.
        测试流式请求在所有提供商失败时抛出错误。
        """
        async def failing_stream(*args, **kwargs):
            raise RateLimitError("Rate limited")
            yield

        fallback_client._clients["anthropic"].chat_stream = failing_stream
        fallback_client._clients["openai"].chat_stream = failing_stream
        fallback_client._clients["gemini"].chat_stream = failing_stream

        with pytest.raises(LlmError):
            chunks = []
            async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
                chunks.append(chunk)

        # Should have logged ALL_PROVIDERS_FAILED
        event_log = fallback_client.get_event_log()
        failed_events = [
            e for e in event_log
            if e.event_type == FallbackEventType.ALL_PROVIDERS_FAILED
        ]
        assert len(failed_events) == 1
        assert failed_events[0].provider == "all"

    @pytest.mark.asyncio
    async def test_stream_fallback_chain_with_degradation_notice(self, fallback_client):
        """
        Test streaming fallback chain emits degradation notices.
        测试流式回退链发出降级通知。
        """
        from continuum_sdk.llm.types import StreamChunk

        # Primary fails
        async def failing_stream(*args, **kwargs):
            raise RateLimitError("Rate limited")
            yield

        fallback_client._clients["anthropic"].chat_stream = failing_stream

        # First fallback fails too
        fallback_client._clients["openai"].chat_stream = failing_stream

        # Second fallback succeeds
        async def success_stream(*args, **kwargs):
            yield StreamChunk(content="Final fallback response")

        fallback_client._clients["gemini"].chat_stream = success_stream

        chunks = []
        async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 1
        assert chunks[0].content == "Final fallback response"

        # Check degradation notices
        event_log = fallback_client.get_event_log()
        degradation_events = [
            e for e in event_log
            if e.event_type == FallbackEventType.DEGRADATION_NOTICE
        ]
        assert len(degradation_events) >= 2

    @pytest.mark.asyncio
    async def test_stream_max_retries_before_fallback(self, fallback_client):
        """
        Test streaming retries before falling back to next provider.
        测试流式请求在回退前进行重试。
        """
        from continuum_sdk.llm.types import StreamChunk

        call_count = [0]

        async def flaky_stream(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] < 2:
                raise RateLimitError("Temporary rate limit")
            yield StreamChunk(content="Success after retry")

        fallback_client._clients["anthropic"].chat_stream = flaky_stream

        chunks = []
        async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 1
        assert call_count[0] == 2  # Failed once, succeeded on retry

    @pytest.mark.asyncio
    async def test_stream_provider_switch_continues_loop(self, fallback_client):
        """
        Test streaming provider switch continues to next provider (line 427->378).
        测试流式提供商切换后继续到下一个提供商（行 427->378）。
        """
        from continuum_sdk.llm.types import StreamChunk

        # Track which providers are tried
        tried_providers = []

        def make_failing_stream(provider_name):
            async def failing_stream(*args, **kwargs):
                tried_providers.append(provider_name)
                raise RateLimitError(f"{provider_name} rate limited")
                yield
            return failing_stream

        def make_success_stream():
            async def success_stream(*args, **kwargs):
                yield StreamChunk(content="Success")
            return success_stream

        # Primary fails after max retries
        fallback_client._clients["anthropic"].chat_stream = make_failing_stream("anthropic")
        # First fallback fails after max retries
        fallback_client._clients["openai"].chat_stream = make_failing_stream("openai")
        # Second fallback succeeds
        fallback_client._clients["gemini"].chat_stream = make_success_stream()

        chunks = []
        async for chunk in fallback_client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        # All providers should have been tried
        assert "anthropic" in tried_providers
        assert "openai" in tried_providers
        assert len(chunks) == 1
        assert chunks[0].content == "Success"

    @pytest.mark.asyncio
    async def test_stream_provider_without_last_error_skips_switch(self):
        """
        Test streaming when provider has no last_provider_error (skips switch event).
        测试流式请求当提供商没有 last_provider_error 时跳过切换事件。
        """
        from continuum_sdk.llm.types import StreamChunk

        # This tests the case where we skip a provider (not in _clients)
        # and continue to the next provider without logging a switch event
        config = FallbackConfig(
            primary_provider="missing_provider",
            fallback_providers=["anthropic", "openai"],
            api_keys={
                "anthropic": "sk-test",
                "openai": "sk-test",
                # missing_provider has no API key
            },
        )
        client = FallbackLlmClient(config)

        # Mock clients
        client._clients["anthropic"] = Mock()
        client._clients["openai"] = Mock()

        # First fallback (anthropic) fails
        async def failing_stream(*args, **kwargs):
            raise RateLimitError("Rate limited")
            yield

        # Second fallback (openai) succeeds
        async def success_stream(*args, **kwargs):
            yield StreamChunk(content="Success from openai")

        client._clients["anthropic"].chat_stream = failing_stream
        client._clients["openai"].chat_stream = success_stream

        chunks = []
        async for chunk in client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 1
        assert chunks[0].content == "Success from openai"

    @pytest.mark.asyncio
    async def test_stream_skip_multiple_providers_without_clients(self):
        """
        Test streaming skips multiple providers without clients (line 427->378 branch).
        测试流式请求跳过多个没有客户端的提供商（行 427->378 分支）。
        """
        from continuum_sdk.llm.types import StreamChunk

        # This specifically tests the branch where last_provider_error is None
        # because we skip providers without clients
        config = FallbackConfig(
            primary_provider="missing1",
            fallback_providers=["missing2", "missing3", "anthropic"],
            api_keys={
                "anthropic": "sk-test",
                # missing1, missing2, missing3 have no API keys
            },
        )
        client = FallbackLlmClient(config)

        # Mock the only client
        client._clients["anthropic"] = Mock()

        async def success_stream(*args, **kwargs):
            yield StreamChunk(content="Success")

        client._clients["anthropic"].chat_stream = success_stream

        chunks = []
        async for chunk in client.chat_stream([Message.user("Hi")]):
            chunks.append(chunk)

        assert len(chunks) == 1
        assert chunks[0].content == "Success"

        # Verify no provider switch events for skipped providers
        event_log = client.get_event_log()
        switch_events = [
            e for e in event_log
            if e.event_type == FallbackEventType.PROVIDER_SWITCH
        ]
        # No switch events because skipped providers don't have last_provider_error
        assert len(switch_events) == 0


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
