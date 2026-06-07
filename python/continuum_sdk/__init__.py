"""
Continuum SDK

Python SDK for Continuum - A terminal agent framework with real LLM calls.

Features:
    - Real LLM API calls (Anthropic, OpenAI, Gemini)
    - Tool registration and function calling
    - Session persistence and recovery
    - Multi-provider configuration

Quick Start (3 steps):
    >>> from continuum import Agent
    >>> agent = Agent()  # Auto-configures from environment
    >>> result = agent.run("hello")

With explicit configuration:
    >>> from continuum import Agent, Config
    >>> config = Config.from_env()
    >>> agent = Agent(config=config)

Tools:
    >>> import ast
    >>> agent.register_tool(
    ...     "calc",
    ...     lambda x: ast.literal_eval(x),
    ...     description="Evaluate math expressions (safe)",
    ...     parameters={"type": "object", "properties": {"expression": {"type": "string"}}}
    ... )
"""

__version__ = "1.0.0"

# Unified API (recommended)
# Core classes (legacy, same as api.Agent)
from .agent import Agent as LegacyAgent
from .agent import Session as LegacySession
from .api import (
    HAS_RUST_BINDING,
    Agent,
    BuiltinTools,
    Session,
    get_implementation_preference,
)
from .config import (
    Config,
    ConfigLoader,
    get_default_model,
    list_providers,
    load_config,
)

# Unified errors (recommended)
from .errors import (
    AuthenticationError,
    ConfigError,
    ContinuumError,
    ErrorContext,
    RateLimitError,
    SecurityError,
    ToolExecutionError,
    ValidationError,
    config_error,
    security_error,
    tool_error,
    validation_error,
)
from .errors import (
    LLMError as UnifiedLLMError,
)

# LLM module (for advanced usage)
from .llm import (
    AnthropicClient,
    ChatResponse,
    GeminiClient,
    LlmClient,
    LlmError,
    Message,
    MessageRole,
    OpenAIClient,
    StreamChunk,
    TokenUsage,
)

__all__ = [
    # Unified API (recommended)
    "Agent",
    "Session",
    "BuiltinTools",
    "HAS_RUST_BINDING",
    "get_implementation_preference",
    # Legacy aliases (for backward compatibility)
    "LegacyAgent",
    "LegacySession",
    # Config
    "Config",
    "ConfigLoader",
    "load_config",
    "list_providers",
    "get_default_model",
    # LLM (advanced)
    "LlmClient",
    "AnthropicClient",
    "OpenAIClient",
    "GeminiClient",
    "Message",
    "MessageRole",
    "ChatResponse",
    "StreamChunk",
    "TokenUsage",
    "LlmError",
    # Unified errors (recommended)
    "ContinuumError",
    "ErrorContext",
    "ConfigError",
    "ToolExecutionError",
    "AuthenticationError",
    "RateLimitError",
    "SecurityError",
    "ValidationError",
    "UnifiedLLMError",
    "config_error",
    "tool_error",
    "validation_error",
    "security_error",
]
