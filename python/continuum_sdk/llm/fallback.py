"""
LLM Fallback Client

Provider-level fallback with exponential backoff retry strategy.

Features:
    - Automatic provider switching on transient errors
    - Exponential backoff with configurable parameters
    - Fallback event logging
    - Streaming support with transparent fallback

Trigger Conditions (fallback):
    - RateLimitError (429)
    - NetworkError (502, 503)
    - TimeoutError (504)
    - Server errors (500)
    - "overloaded" error type

Non-trigger Conditions (immediate error):
    - AuthenticationError (401, 403)
    - ModelNotFoundError (404)
    - ContentFilterError
    - InvalidResponseError

Usage:
    >>> from continuum_sdk.llm import FallbackClient, FallbackConfig
    >>>
    >>> config = FallbackConfig(
    ...     primary_provider="anthropic",
    ...     fallback_providers=["openai", "together"],
    ...     api_keys={"anthropic": "...", "openai": "...", "together": "..."}
    ... )
    >>> client = FallbackClient(config)
    >>> response = await client.chat([Message.user("Hello")])
"""

import asyncio
import logging
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable

from .client import BaseLlmClient, LlmClient
from .errors import (
    AuthenticationError,
    ContentFilterError,
    InvalidResponseError,
    LlmError,
    ModelNotFoundError,
    NetworkError,
    RateLimitError,
    TimeoutError,
)
from .types import ChatResponse, Message, StreamChunk, ToolDefinition

logger = logging.getLogger(__name__)


class FallbackEventType(Enum):
    """Types of fallback events."""

    RETRY = "retry"
    PROVIDER_SWITCH = "provider_switch"
    MAX_RETRIES_EXCEEDED = "max_retries_exceeded"
    ALL_PROVIDERS_FAILED = "all_providers_failed"
    DEGRADATION_NOTICE = "degradation_notice"


@dataclass
class FallbackEvent:
    """Record of a fallback event."""

    event_type: FallbackEventType
    provider: str
    error: LlmError | None = None
    next_provider: str | None = None
    attempt: int = 1
    delay_ms: float = 0.0
    timestamp: float = field(default_factory=time.time)
    message: str | None = None  # Human-readable message for degradation notices


@dataclass
class FallbackConfig:
    """Configuration for fallback behavior."""

    primary_provider: str
    fallback_providers: list[str] = field(default_factory=list)
    api_keys: dict[str, str] = field(default_factory=dict)
    models: dict[str, str] = field(default_factory=dict)
    base_urls: dict[str, str] = field(default_factory=dict)

    # Retry settings
    max_retries: int = 3
    initial_delay_ms: float = 1000.0
    max_delay_ms: float = 30000.0
    backoff_multiplier: float = 2.0

    # Callbacks
    on_fallback: Callable[[FallbackEvent], None] | None = None


class FallbackLlmClient:
    """
    LLM client with automatic provider fallback.

    Wraps multiple LLM providers and automatically switches to backup
    providers when the primary fails with transient errors.

    Example:
        >>> config = FallbackConfig(
        ...     primary_provider="anthropic",
        ...     fallback_providers=["openai", "together"],
        ...     api_keys={
        ...         "anthropic": "sk-ant-...",
        ...         "openai": "sk-...",
        ...         "together": "..."
        ...     }
        ... )
        >>> client = FallbackLlmClient(config)
        >>> response = await client.chat([Message.user("Hello")])
    """

    def __init__(self, config: FallbackConfig):
        self.config = config
        self._clients: dict[str, BaseLlmClient] = {}
        self._event_log: list[FallbackEvent] = []
        self._init_clients()

    def _init_clients(self):
        """Initialize LLM clients for all configured providers."""
        all_providers = [self.config.primary_provider] + self.config.fallback_providers

        for provider in all_providers:
            api_key = self.config.api_keys.get(provider)
            if not api_key:
                logger.warning(f"No API key configured for provider: {provider}")
                continue

            model = self.config.models.get(provider)
            base_url = self.config.base_urls.get(provider)

            self._clients[provider] = LlmClient.for_provider(
                provider=provider,
                api_key=api_key,
                model=model,
                base_url=base_url,
            )

    def _should_trigger_fallback(self, error: LlmError) -> bool:
        """
        Determine if an error should trigger fallback.

        Transient errors that warrant fallback:
            - RateLimitError (429)
            - NetworkError (502, 503)
            - TimeoutError (504)
            - Server errors (500)
            - "overloaded" error

        Permanent errors that should NOT fallback:
            - AuthenticationError (401, 403)
            - ModelNotFoundError (404)
            - ContentFilterError
            - InvalidResponseError
        """
        if isinstance(error, RateLimitError):
            return True
        if isinstance(error, NetworkError):
            return True
        if isinstance(error, TimeoutError):
            return True

        # Check for server errors (500) or overloaded
        error_str = str(error).lower()
        if "500" in error_str or "overloaded" in error_str:
            return True

        # Permanent errors - no fallback
        if isinstance(error, AuthenticationError):
            return False
        if isinstance(error, ModelNotFoundError):
            return False
        if isinstance(error, ContentFilterError):
            return False

        # For other LlmError instances, check message content
        if isinstance(error, LlmError):
            # Check for server-side errors in message
            if any(code in error_str for code in ["500", "502", "503", "504"]):
                return True

        return False

    def _calculate_delay(self, attempt: int) -> float:
        """Calculate exponential backoff delay in milliseconds."""
        delay = self.config.initial_delay_ms * (self.config.backoff_multiplier ** (attempt - 1))
        return min(delay, self.config.max_delay_ms)

    def _log_event(self, event: FallbackEvent):
        """Log a fallback event."""
        self._event_log.append(event)
        if self.config.on_fallback:
            self.config.on_fallback(event)

        log_msg = f"[Fallback] {event.event_type.value}: provider={event.provider}"
        if event.error:
            log_msg += f", error={type(event.error).__name__}"
        if event.next_provider:
            log_msg += f", next={event.next_provider}"
        if event.delay_ms > 0:
            log_msg += f", delay={event.delay_ms:.0f}ms"
        if event.message:
            log_msg += f", message={event.message}"

        logger.info(log_msg)

    def _emit_degradation_notice(self, from_provider: str, to_provider: str, reason: str):
        """Emit a degradation notice when falling back to a backup provider."""
        message = (
            f"Service degradation: {from_provider} is unavailable ({reason}). "
            f"Falling back to {to_provider}. Response quality may vary."
        )
        self._log_event(FallbackEvent(
            event_type=FallbackEventType.DEGRADATION_NOTICE,
            provider=from_provider,
            next_provider=to_provider,
            message=message,
        ))
        logger.warning(message)

    async def _execute_with_retry(
        self,
        provider: str,
        operation: Callable[..., Any],
        *args,
        **kwargs,
    ) -> Any:
        """Execute an operation with retry logic for a single provider."""
        client = self._clients.get(provider)
        if not client:
            raise LlmError(f"No client configured for provider: {provider}")

        last_error: LlmError | None = None

        for attempt in range(1, self.config.max_retries + 1):
            try:
                return await operation(client, *args, **kwargs)
            except LlmError as e:
                last_error = e

                # Check if we should retry
                if not self._should_trigger_fallback(e):
                    raise

                if attempt < self.config.max_retries:
                    delay_ms = self._calculate_delay(attempt)
                    self._log_event(FallbackEvent(
                        event_type=FallbackEventType.RETRY,
                        provider=provider,
                        error=e,
                        attempt=attempt,
                        delay_ms=delay_ms,
                    ))
                    await asyncio.sleep(delay_ms / 1000.0)

        # Max retries exceeded
        self._log_event(FallbackEvent(
            event_type=FallbackEventType.MAX_RETRIES_EXCEEDED,
            provider=provider,
            error=last_error,
            attempt=self.config.max_retries,
        ))
        raise last_error

    async def chat(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ) -> ChatResponse:
        """
        Send chat request with provider fallback.

        Tries providers in order: primary -> fallback_providers[0] -> ...

        Args:
            messages: List of conversation messages
            model: Model to use (provider-specific)
            max_tokens: Maximum tokens to generate
            temperature: Sampling temperature
            system_prompt: System prompt
            tools: Available tools for function calling
            **kwargs: Additional provider-specific options

        Returns:
            ChatResponse from the first successful provider

        Raises:
            LlmError: If all providers fail
        """
        all_providers = [self.config.primary_provider] + self.config.fallback_providers
        last_error: LlmError | None = None

        for i, provider in enumerate(all_providers):
            if provider not in self._clients:
                continue

            try:
                return await self._execute_with_retry(
                    provider,
                    lambda c, **kw: c.chat(**kw),
                    messages=messages,
                    model=model,
                    max_tokens=max_tokens,
                    temperature=temperature,
                    system_prompt=system_prompt,
                    tools=tools,
                    **kwargs,
                )
            except LlmError as e:
                last_error = e

                # Don't fallback for permanent errors
                if not self._should_trigger_fallback(e):
                    raise

                # Log provider switch
                next_provider = all_providers[i + 1] if i + 1 < len(all_providers) else None
                self._log_event(FallbackEvent(
                    event_type=FallbackEventType.PROVIDER_SWITCH,
                    provider=provider,
                    error=e,
                    next_provider=next_provider,
                ))

                # Emit degradation notice if we have a fallback target
                if next_provider:
                    reason = type(e).__name__
                    self._emit_degradation_notice(provider, next_provider, reason)

        # All providers failed
        self._log_event(FallbackEvent(
            event_type=FallbackEventType.ALL_PROVIDERS_FAILED,
            provider="all",
            error=last_error,
        ))
        raise last_error or LlmError("All providers failed")

    async def chat_stream(
        self,
        messages: list[Message],
        model: str | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        system_prompt: str | None = None,
        tools: list[ToolDefinition] | None = None,
        **kwargs,
    ):
        """
        Send streaming chat request with provider fallback.

        Note: Streaming fallback is limited - if an error occurs mid-stream,
        we cannot transparently switch providers. Fallback only occurs
        on initial connection errors.

        Yields:
            StreamChunk objects as they arrive
        """
        all_providers = [self.config.primary_provider] + self.config.fallback_providers
        last_error: LlmError | None = None

        for i, provider in enumerate(all_providers):
            if provider not in self._clients:
                continue

            client = self._clients[provider]
            last_provider_error: LlmError | None = None

            # Retry loop for this provider
            for attempt in range(1, self.config.max_retries + 1):
                try:
                    # For streaming, we try to start the stream
                    # If it fails immediately, we can fallback
                    stream = client.chat_stream(
                        messages=messages,
                        model=model,
                        max_tokens=max_tokens,
                        temperature=temperature,
                        system_prompt=system_prompt,
                        tools=tools,
                        **kwargs,
                    )

                    # Yield chunks from the stream
                    async for chunk in stream:
                        yield chunk
                    return

                except LlmError as e:
                    last_error = e
                    last_provider_error = e

                    # Don't retry/fallback for permanent errors
                    if not self._should_trigger_fallback(e):
                        raise

                    # Retry with backoff if we have attempts left
                    if attempt < self.config.max_retries:
                        delay_ms = self._calculate_delay(attempt)
                        self._log_event(FallbackEvent(
                            event_type=FallbackEventType.RETRY,
                            provider=provider,
                            error=e,
                            attempt=attempt,
                            delay_ms=delay_ms,
                        ))
                        await asyncio.sleep(delay_ms / 1000.0)
                        continue

            # Provider exhausted, try fallback
            if last_provider_error:  # pragma: no branch (loop continuation branch)
                # Log provider switch
                next_provider = all_providers[i + 1] if i + 1 < len(all_providers) else None
                self._log_event(FallbackEvent(
                    event_type=FallbackEventType.PROVIDER_SWITCH,
                    provider=provider,
                    error=last_provider_error,
                    next_provider=next_provider,
                ))

                # Emit degradation notice if we have a fallback target
                if next_provider:
                    reason = type(last_provider_error).__name__
                    self._emit_degradation_notice(provider, next_provider, reason)

        # All providers failed
        self._log_event(FallbackEvent(
            event_type=FallbackEventType.ALL_PROVIDERS_FAILED,
            provider="all",
            error=last_error,
        ))
        raise last_error or LlmError("All providers failed")

    async def close(self):
        """Close all underlying clients."""
        for client in self._clients.values():
            await client.close()

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()

    def get_event_log(self) -> list[FallbackEvent]:
        """Get the list of fallback events."""
        return self._event_log.copy()

    def clear_event_log(self):
        """Clear the event log."""
        self._event_log.clear()


# Convenience function for creating fallback client from config dict
def create_fallback_client(config_dict: dict[str, Any]) -> FallbackLlmClient:
    """
    Create a FallbackLlmClient from a configuration dictionary.

    Args:
        config_dict: Configuration with keys:
            - provider.primary: Primary provider name
            - provider.fallback: List of fallback providers
            - api_keys: Dict of provider -> API key
            - retry.max_retries: Max retry attempts
            - retry.initial_delay_ms: Initial backoff delay
            - retry.max_delay_ms: Maximum backoff delay
            - retry.backoff_multiplier: Backoff multiplier

    Returns:
        Configured FallbackLlmClient instance
    """
    provider_config = config_dict.get("provider", {})
    retry_config = config_dict.get("retry", {})

    config = FallbackConfig(
        primary_provider=provider_config.get("primary", "anthropic"),
        fallback_providers=provider_config.get("fallback", []),
        api_keys=config_dict.get("api_keys", {}),
        models=config_dict.get("models", {}),
        base_urls=config_dict.get("base_urls", {}),
        max_retries=retry_config.get("max_retries", 3),
        initial_delay_ms=retry_config.get("initial_delay_ms", 1000.0),
        max_delay_ms=retry_config.get("max_delay_ms", 30000.0),
        backoff_multiplier=retry_config.get("backoff_multiplier", 2.0),
    )

    return FallbackLlmClient(config)
