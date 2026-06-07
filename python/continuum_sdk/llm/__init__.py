"""
LLM Client Module

Real LLM API client implementations for Continuum SDK.

Supports:
    - Anthropic Claude API
    - OpenAI GPT API
    - Google Gemini API
    - Custom endpoints

Usage:
    >>> from continuum_sdk.llm import LlmClient
    >>> client = LlmClient.for_provider("anthropic", api_key="...")
    >>> response = client.chat(messages=[{"role": "user", "content": "Hello"}])
"""

from .client import (
    AnthropicClient,
    BaseLlmClient,
    CustomClient,
    GeminiClient,
    LlmClient,
    OpenAIClient,
)
from .errors import (
    AuthenticationError,
    InvalidResponseError,
    LlmError,
    NetworkError,
    RateLimitError,
    TimeoutError,
)
from .fallback import (
    FallbackConfig,
    FallbackEvent,
    FallbackEventType,
    FallbackLlmClient,
    create_fallback_client,
)
from .streaming import (
    CallbackStream,
    ContentBlockType,
    SseEvent,
    SseParser,
    StreamEvent,
    StreamState,
    StreamUsage,
)
from .types import (
    ChatResponse,
    Message,
    MessageRole,
    StreamChunk,
    TokenUsage,
    ToolDefinition,
)

__all__ = [
    # Client
    "LlmClient",
    "BaseLlmClient",
    "AnthropicClient",
    "OpenAIClient",
    "GeminiClient",
    "CustomClient",
    # Fallback
    "FallbackLlmClient",
    "FallbackConfig",
    "FallbackEvent",
    "FallbackEventType",
    "create_fallback_client",
    # Errors
    "LlmError",
    "AuthenticationError",
    "RateLimitError",
    "NetworkError",
    "TimeoutError",
    "InvalidResponseError",
    # Types
    "Message",
    "MessageRole",
    "ChatResponse",
    "StreamChunk",
    "TokenUsage",
    "ToolDefinition",
    # Streaming
    "SseParser",
    "SseEvent",
    "StreamState",
    "StreamEvent",
    "StreamUsage",
    "CallbackStream",
    "ContentBlockType",
]
