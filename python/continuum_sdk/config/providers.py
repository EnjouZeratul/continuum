"""
Provider Configuration

Multi-provider management for LLM services with support for:
- Anthropic API format
- OpenAI-compatible API format (most providers)
- Custom providers with configurable format
"""

import logging
import os

from dataclasses import dataclass, field
from enum import Enum

logger = logging.getLogger(__name__)


class ProviderType(Enum):
    """Provider type."""

    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    GOOGLE = "google"
    GEMINI = "gemini"
    AZURE = "azure"
    BEDROCK = "bedrock"
    OLLAMA = "ollama"
    CUSTOM = "custom"


class ApiFormat(Enum):
    """API request format."""

    ANTHROPIC = "anthropic"  # Anthropic native format
    OPENAI = "openai"  # OpenAI compatible format (most providers)
    GOOGLE = "google"  # Google AI format


@dataclass
class ProviderInfo:
    """Provider information."""

    name: str
    display_name: str
    default_model: str
    default_small_model: str | None = None
    default_base_url: str | None = None
    env_key_name: str | None = None
    models: list[str] = field(default_factory=list)
    api_format: ApiFormat = ApiFormat.OPENAI  # Default to OpenAI compatible format


# Builtin provider configurations
BUILTIN_PROVIDERS: dict[str, ProviderInfo] = {
    # Anthropic native format
    "anthropic": ProviderInfo(
        name="anthropic",
        display_name="Anthropic (Claude)",
        default_model="claude-sonnet-4-6",
        default_small_model="claude-haiku-4-5",
        default_base_url="https://api.anthropic.com",
        env_key_name="ANTHROPIC_API_KEY",
        models=[
            "claude-opus-4-8",
            "claude-opus-4-7",
            "claude-opus-4-6",
            "claude-opus-4-5",
            "claude-sonnet-4-6",
            "claude-sonnet-4-5",
            "claude-haiku-4-5",
            "claude-mythos-preview",
        ],
        api_format=ApiFormat.ANTHROPIC,
    ),
    # OpenAI format
    "openai": ProviderInfo(
        name="openai",
        display_name="OpenAI (GPT)",
        default_model="gpt-5.5",
        default_small_model="gpt-4.1-mini",
        default_base_url="https://api.openai.com/v1",
        env_key_name="OPENAI_API_KEY",
        models=[
            "gpt-5.5",
            "gpt-5.4",
            "gpt-5.2",
            "gpt-5.1",
            "gpt-5",
            "o3-mini",
            "o1",
            "gpt-4o",
            "gpt-4o-mini",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # Google AI format
    "google": ProviderInfo(
        name="google",
        display_name="Google (Gemini)",
        default_model="gemini-3.0-pro",
        default_small_model="gemini-3.0-flash",
        default_base_url="https://generativelanguage.googleapis.com/v1beta",
        env_key_name="GOOGLE_API_KEY",
        models=[
            "gemini-3.1-pro-preview",
            "gemini-3.5-flash",
            "gemini-3.0-pro",
            "gemini-3.0-flash",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
        ],
        api_format=ApiFormat.GOOGLE,
    ),
    "gemini": ProviderInfo(
        name="gemini",
        display_name="Google Gemini",
        default_model="gemini-3.0-pro",
        default_small_model="gemini-3.0-flash",
        default_base_url="https://generativelanguage.googleapis.com/v1beta",
        env_key_name="GOOGLE_API_KEY",
        models=[
            "gemini-3.1-pro-preview",
            "gemini-3.5-flash",
            "gemini-3.0-pro",
            "gemini-3.0-flash",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
        ],
        api_format=ApiFormat.GOOGLE,
    ),
    # Cohere
    # Cohere
    "cohere": ProviderInfo(
        name="cohere",
        display_name="Cohere",
        default_model="command",
        default_small_model="command-light",
        default_base_url="https://api.cohere.ai/v1",
        env_key_name="COHERE_API_KEY",
        models=[
            "command",
            "command-light",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # HuggingFace
    # HuggingFace
    "huggingface": ProviderInfo(
        name="huggingface",
        display_name="HuggingFace",
        default_model="",
        default_base_url="https://api-inference.huggingface.co/models",
        env_key_name="HF_API_KEY",
        models=[],  # HuggingFace supports any model, no presets needed
        api_format=ApiFormat.OPENAI,
    ),
    # Common OpenAI compatible providers
    "together": ProviderInfo(
        name="together",
        display_name="Together AI",
        default_model="meta-llama/Llama-3-70b-chat-hf",
        default_base_url="https://api.together.xyz/v1",
        env_key_name="TOGETHER_API_KEY",
        models=[
            "meta-llama/Llama-3-70b-chat-hf",
            "meta-llama/Llama-3-8b-chat-hf",
            "mistralai/Mixtral-8x7B-Instruct-v0.1",
            "mistralai/Mistral-7B-Instruct-v0.1",
            "togethercomputer/CodeLlama-34b-Instruct",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    "groq": ProviderInfo(
        name="groq",
        display_name="Groq",
        default_model="llama-3.3-70b-versatile",
        default_base_url="https://api.groq.com/openai/v1",
        env_key_name="GROQ_API_KEY",
        models=[
            "llama-3.3-70b-versatile",
            "llama-3.3-70b-specdec",
            "llama-3.1-8b-instant",
            "llama-3.1-70b-versatile",
            "llama-3.1-405b-reasoning",
            "mixtral-8x7b-32768",
            "gemma2-9b-it",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    "deepseek": ProviderInfo(
        name="deepseek",
        display_name="DeepSeek",
        default_model="deepseek-v4-pro",
        default_base_url="https://api.deepseek.com/v1",
        env_key_name="DEEPSEEK_API_KEY",
        models=[
            "deepseek-v4-pro",
            "deepseek-v4-flash",
            "deepseek-v3.2",
            "deepseek-v3.1-terminus",
            "deepseek-v3",
            "deepseek-chat",
            "deepseek-reasoner",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    "moonshot": ProviderInfo(
        name="moonshot",
        display_name="Moonshot (Kimi)",
        default_model="kimi-k2.6",
        default_base_url="https://api.moonshot.cn/v1",
        env_key_name="MOONSHOT_API_KEY",
        models=[
            "kimi-k2.6",
            "kimi-k2-thinking",
            "kimi-k2.5",
            "moonshot-v1-8k",
            "moonshot-v1-32k",
            "moonshot-v1-128k",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # Chinese providers - GLM (智谱)
    "glm": ProviderInfo(
        name="glm",
        display_name="GLM (Zhipu AI)",
        default_model="glm-5.1",
        default_base_url="https://open.bigmodel.cn/api/paas/v4",
        env_key_name="GLM_API_KEY",
        models=[
            "glm-5.1",
            "glm-5",
            "glm-4.7",
            "glm-4.6",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # Chinese providers - KIMI (月之暗面)
    "kimi": ProviderInfo(
        name="kimi",
        display_name="KIMI (Moonshot AI)",
        default_model="kimi-k2.6",
        default_base_url="https://api.moonshot.cn/v1",
        env_key_name="MOONSHOT_API_KEY",
        models=[
            "kimi-k2.6",
            "kimi-k2-thinking",
            "kimi-k2.5",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # Chinese providers - Qwen (阿里巴巴)
    "qwen": ProviderInfo(
        name="qwen",
        display_name="Qwen (Alibaba Cloud)",
        default_model="qwen3.7-max",
        default_base_url="https://dashscope.aliyuncs.com/compatible-mode/v1",
        env_key_name="QWEN_API_KEY",
        models=[
            "qwen3.7-max",
            "qwen3.6-plus",
            "qwen3.5-27B",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # International providers - Grok (xAI)
    "grok": ProviderInfo(
        name="grok",
        display_name="Grok (xAI)",
        default_model="grok-4-heavy",
        default_base_url="https://api.x.ai/v1",
        env_key_name="XAI_API_KEY",
        models=[
            "grok-4-heavy",
            "grok-4",
        ],
        api_format=ApiFormat.OPENAI,
    ),
    # Cloud provider hosted models
    "azure": ProviderInfo(
        name="azure",
        display_name="Azure OpenAI",
        default_model="gpt-4o",
        default_base_url="https://YOUR_RESOURCE.openai.azure.com",
        env_key_name="AZURE_OPENAI_API_KEY",
        models=[],
        api_format=ApiFormat.OPENAI,
    ),
    "bedrock": ProviderInfo(
        name="bedrock",
        display_name="AWS Bedrock",
        default_model="anthropic.claude-sonnet-4-6",
        default_base_url="https://bedrock-runtime.us-east-1.amazonaws.com",
        env_key_name="AWS_ACCESS_KEY_ID",
        models=[],
        api_format=ApiFormat.OPENAI,
    ),
    "ollama": ProviderInfo(
        name="ollama",
        display_name="Ollama (Local)",
        default_model="llama3",
        default_base_url="http://localhost:11434",
        env_key_name=None,
        models=[],
        api_format=ApiFormat.OPENAI,
    ),
}


def get_provider_info(name: str) -> ProviderInfo | None:
    """Get provider information."""
    return BUILTIN_PROVIDERS.get(name)


def list_providers() -> list[str]:
    """List all builtin providers."""
    return list(BUILTIN_PROVIDERS.keys())


# Fallback provider priority order (for default model fallback)
FALLBACK_PROVIDER_ORDER = ["anthropic", "openai", "google", "deepseek", "qwen"]


def get_default_model(provider: str) -> str:
    """
    Get default model for provider.

    Priority: environment variable CONTINUUM_MODEL > BUILTIN_PROVIDERS config > fallback mapping table.
    """
    # 1. Environment variable first (user may have configured specific model)
    env_model = os.environ.get("CONTINUUM_MODEL")
    if env_model:
        return env_model

    # 2. Get from builtin config
    info = BUILTIN_PROVIDERS.get(provider)
    if info:
        return info.default_model

    # 3. Fallback mapping table (try by provider priority)
    logger.info(
        f"Default model config not found for provider '{provider}', trying fallback..."
    )
    for fallback_provider in FALLBACK_PROVIDER_ORDER:
        info = BUILTIN_PROVIDERS.get(fallback_provider)
        if info:
            logger.info(
                f"Fallback: using '{fallback_provider}' default model '{info.default_model}'"
            )
            return info.default_model

    # 4. Get from first available provider
    for fallback_provider, info in BUILTIN_PROVIDERS.items():
        logger.info(
            f"Fallback: using '{fallback_provider}' default model '{info.default_model}'"
        )
        return info.default_model

    # 5. No config at all (edge case)
    raise RuntimeError(
        "Unable to get any default model config. Please configure via one of:\n"
        "1. Set environment variable CONTINUUM_MODEL\n"
        "2. Configure default model in providers"
    )


def get_default_small_model(provider: str) -> str | None:
    """Get default small model for provider."""
    info = BUILTIN_PROVIDERS.get(provider)
    if info:
        return info.default_small_model
    return None


def get_env_key_name(provider: str) -> str | None:
    """Get provider's environment variable key name."""
    info = BUILTIN_PROVIDERS.get(provider)
    if info:
        return info.env_key_name
    return None


def get_default_base_url(provider: str) -> str | None:
    """Get provider's default API URL."""
    info = BUILTIN_PROVIDERS.get(provider)
    if info:
        return info.default_base_url
    return None


def get_api_format(provider: str) -> ApiFormat:
    """Get provider's API request format."""
    info = BUILTIN_PROVIDERS.get(provider)
    if info:
        return info.api_format
    return ApiFormat.OPENAI


def list_models(provider: str) -> list[str]:
    """List models supported by provider."""
    info = BUILTIN_PROVIDERS.get(provider)
    if info:
        return info.models.copy()
    return []
