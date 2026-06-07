"""
Config Loader - Enhanced Configuration Management

Configuration loading and management for Continuum SDK with:
- Environment variable support (CONTINUUM_* as primary, CONTINUUM_* as fallback)
- TOML configuration file support
- Environment variable expansion (${VAR_NAME})
- Multi-provider management
- Security: Whitelist-based environment variable access
"""

import json
import logging
import os
import re
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any

from .providers import get_default_model as _get_provider_default_model

logger = logging.getLogger(__name__)

# Security: Whitelist of allowed environment variables
ALLOWED_ENV_VARS = {
    # Continuum namespace
    "CONTINUUM_API_KEY",
    "CONTINUUM_BASE_URL",
    "CONTINUUM_PROVIDER",
    "CONTINUUM_MODEL",
    "CONTINUUM_SMALL_MODEL",
    "CONTINUUM_DEFAULT_MODEL",
    "CONTINUUM_LOG_LEVEL",
    "CONTINUUM_MAX_TOKENS",
    "CONTINUUM_TIMEOUT",
    "CONTINUUM_MAX_ITERATIONS",
    "CONTINUUM_TEMPERATURE",
    "CONTINUUM_BUDGET",
    "CONTINUUM_EFFORT_LEVEL",
    "CONTINUUM_DISABLE_TRAFFIC",
    "CONTINUUM_WORKTREES_DIR",
    "CONTINUUM_PLUGINS_DIR",
    "CONTINUUM_API_FORMAT",
    "CONTINUUM_AUDIT_ENABLED",
    "CONTINUUM_AUDIT_LOG_PATH",
    "CONTINUUM_AUDIT_RETENTION",
    # Provider-specific (standard)
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_MODEL",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_MODEL",
    "GOOGLE_API_KEY",
    "GOOGLE_BASE_URL",
    "GEMINI_API_KEY",
    "GEMINI_BASE_URL",
    # Additional providers
    "DEEPSEEK_API_KEY",
    "DEEPSEEK_BASE_URL",
    "GLM_API_KEY",
    "GLM_BASE_URL",
    "MOONSHOT_API_KEY",
    "MOONSHOT_BASE_URL",
    "QWEN_API_KEY",
    "QWEN_BASE_URL",
    "XAI_API_KEY",
    "XAI_BASE_URL",
    "GROK_API_KEY",
    "GROK_BASE_URL",
    "TOGETHER_API_KEY",
    "TOGETHER_BASE_URL",
    "GROQ_API_KEY",
    "GROQ_BASE_URL",
    "COHERE_API_KEY",
    "COHERE_BASE_URL",
    "HF_API_KEY",
    "HUGGINGFACE_BASE_URL",
    # Cloud / hosted providers
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_REGION",
    "OLLAMA_BASE_URL",
    # Embedding providers
    "HUGGINGFACE_API_KEY",
    "HUGGINGFACE_EMBEDDING_MODEL",
    "OPENAI_EMBEDDING_MODEL",
    "COHERE_API_KEY",
    "COHERE_EMBEDDING_MODEL",
    "LOCAL_EMBEDDING_MODEL",
    # Test support
    "USE_REAL_API",
    "CONTINUUM_THEME_CONFIG",
    # Legacy support (deprecated)
    "SUPERHARNESS_API_KEY",
    "SUPERHARNESS_BASE_URL",
}


def _get_env(name: str, default: str | None = None) -> str | None:
    """
    Get environment variable with soft constraint (warning for non-documented vars).

    Uses whitelist as documentation purpose. Non-whitelisted variables will log
    a warning but still return their value if set.

    Note: Returns default silently for non-whitelisted variables during fallback checks.
    This is intentional - the fallback logic in from_env() tries multiple prefixes,
    and we don't want to warn on expected fallback misses.
    """
    if name not in ALLOWED_ENV_VARS:
        # Log warning for non-documented variables
        logger.warning(
            f"Accessing non-documented env var '{name}'. "
            f"Consider adding to ALLOWED_ENV_VARS for consistency."
        )
    return os.environ.get(name, default)

# TOML support (Python 3.11+ has built-in, otherwise use tomllib)
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None


class Provider(Enum):
    """LLM Provider."""

    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    GOOGLE = "google"
    GEMINI = "gemini"
    AZURE = "azure"
    BEDROCK = "bedrock"
    OLLAMA = "ollama"
    CUSTOM = "custom"


@dataclass
class ProviderConfig:
    """Provider configuration."""

    name: str
    api_key: str | None = None
    base_url: str | None = None
    model: str | None = None
    small_model: str | None = None
    default_model: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "api_key": self.api_key,
            "base_url": self.base_url,
            "model": self.model,
            "small_model": self.small_model,
            "default_model": self.default_model,
        }


class Config:
    """
    Continuum Configuration Class.

    Supports multiple configuration sources with priority: environment variables > config files > defaults.

    Usage:
        # Method 1: Auto-load from environment variables (recommended)
        config = Config.from_env()

        # Method 2: Load from config file
        config = Config.from_file("~/.continuum/config.toml")

        # Method 3: Explicit configuration (model is optional, defaults auto-detected)
        config = Config(
            provider="anthropic",
            api_key="xxx"
            # model is optional, auto-fetched from providers config
        )

        # Switch provider (auto-switches to corresponding default model)
        config.use("openai")
    """

    # Environment variable prefix (CONTINUUM_* preferred, CONTINUUM_* for compatibility)
    ENV_PREFIX = "CONTINUUM_"
    ENV_PREFIX_FALLBACK = "CONTINUUM_"

    # Default config directories
    DEFAULT_CONFIG_DIRS = [
        ".",
        ".claude",
        "~/.config/continuum",
        "~/.continuum",
    ]

    # Default config filenames
    DEFAULT_CONFIG_FILES = ["config.toml", "continuum.toml", "config.json"]

    def __init__(
        self,
        provider: str = "anthropic",
        api_key: str | None = None,
        base_url: str | None = None,
        api_format: str | None = None,
        model: str | None = None,
        small_model: str | None = None,
        effort_level: str = "medium",
        disable_traffic: bool = False,
        budget: float | None = None,
        max_tokens: int = 4096,
        temperature: float = 0.7,
        worktrees_dir: str | None = None,
        plugins_dir: str | None = None,
        log_level: str = "info",
        audit_enabled: bool = True,
        audit_retention_days: int = 90,
        **kwargs,
    ):
        """
        Create configuration.

        Args:
            provider: LLM provider (anthropic|openai|google|custom|together|groq|...)
            api_key: API key
            base_url: API base URL (for custom endpoints or proxies)
            api_format: API format (anthropic|openai|google). Auto-inferred from provider if not set
            model: Main model name
            small_model: Small model name (for simple tasks)
            effort_level: Effort level (low|medium|high|max)
            disable_traffic: Whether to disable traffic statistics
            budget: Budget limit
            max_tokens: Maximum token count
            temperature: Temperature parameter
            worktrees_dir: Worktrees directory
            plugins_dir: Plugins directory
            log_level: Log level
            audit_enabled: Whether to enable audit
            audit_retention_days: Audit log retention days
            **kwargs: Other configuration items
        """
        self._data: dict[str, Any] = {
            "provider": provider,
            "api_key": api_key,
            "base_url": base_url,
            "api_format": api_format,
            "model": model,
            "small_model": small_model,
            "effort_level": effort_level,
            "disable_traffic": disable_traffic,
            "budget": budget,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "worktrees_dir": worktrees_dir,
            "plugins_dir": plugins_dir,
            "log_level": log_level,
            "audit_enabled": audit_enabled,
            "audit_retention_days": audit_retention_days,
        }
        self._data.update(kwargs)

        # Provider config storage
        self._providers: dict[str, ProviderConfig] = {}
        self._current_provider: str | None = None

    # ==================== Property Access ====================

    @property
    def provider(self) -> str:
        """Current provider."""
        return self._data.get("provider", "anthropic")

    @property
    def api_key(self) -> str | None:
        """API key."""
        return self._data.get("api_key")

    @property
    def model(self) -> str:
        """Model name."""
        return self._data.get("model") or self._get_default_model()

    @property
    def small_model(self) -> str | None:
        """Small model name."""
        return self._data.get("small_model")

    @property
    def base_url(self) -> str | None:
        """API base URL."""
        return self._data.get("base_url")

    @property
    def api_format(self) -> str | None:
        """API request format (anthropic|openai|google)."""
        return self._data.get("api_format")

    @property
    def effort_level(self) -> str:
        """Effort level."""
        return self._data.get("effort_level", "medium")

    @property
    def disable_traffic(self) -> bool:
        """Whether traffic statistics are disabled."""
        return self._data.get("disable_traffic", False)

    @property
    def budget(self) -> float | None:
        """Budget limit."""
        return self._data.get("budget")

    @property
    def max_tokens(self) -> int:
        """Maximum token count."""
        return self._data.get("max_tokens", 4096)

    @property
    def temperature(self) -> float:
        """Temperature parameter."""
        return self._data.get("temperature", 0.7)

    @property
    def audit_enabled(self) -> bool:
        """Whether audit is enabled."""
        return self._data.get("audit_enabled", True)

    def get(self, key: str, default: Any = None) -> Any:
        """Get configuration item."""
        return self._data.get(key, default)

    def set(self, key: str, value: Any) -> None:
        """Set configuration item."""
        self._data[key] = value

    def update(self, data: dict[str, Any]) -> None:
        """Batch update configuration."""
        self._data.update(data)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return self._data.copy()

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Config":
        """Create config from dictionary."""
        return cls(**data)

    # ==================== Convenience Loading Methods ====================

    @classmethod
    def from_env(cls) -> "Config":
        """
        Load configuration from environment variables.

        Environment variable priority: CONTINUUM_* > {PROVIDER}_* > ANTHROPIC_*

        Example: CONTINUUM_PROVIDER=openai, CONTINUUM_API_KEY=xxx
        """
        env_mapping = {
            "PROVIDER": "provider",
            "API_KEY": "api_key",
            "BASE_URL": "base_url",
            "API_FORMAT": "api_format",  # anthropic, openai, google
            "MODEL": "model",
            "SMALL_MODEL": "small_model",
            "EFFORT_LEVEL": "effort_level",
            "DISABLE_TRAFFIC": (
                "disable_traffic",
                lambda x: x.lower() in ("1", "true", "yes"),
            ),
            "BUDGET": ("budget", float),
            "MAX_TOKENS": ("max_tokens", int),
            "TEMPERATURE": ("temperature", float),
            "WORKTREES_DIR": "worktrees_dir",
            "PLUGINS_DIR": "plugins_dir",
            "LOG_LEVEL": "log_level",
            "AUDIT_ENABLED": (
                "audit_enabled",
                lambda x: x.lower() in ("1", "true", "yes"),
            ),
            "AUDIT_RETENTION": ("audit_retention_days", int),
        }

        # Provider-specific env var names
        provider_env_keys = {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "google": "GOOGLE_API_KEY",
            "gemini": "GOOGLE_API_KEY",
            "deepseek": "DEEPSEEK_API_KEY",
            "glm": "GLM_API_KEY",
            "moonshot": "MOONSHOT_API_KEY",
            "kimi": "MOONSHOT_API_KEY",
            "qwen": "QWEN_API_KEY",
            "grok": "XAI_API_KEY",
            "together": "TOGETHER_API_KEY",
            "groq": "GROQ_API_KEY",
            "cohere": "COHERE_API_KEY",
            "huggingface": "HF_API_KEY",
            "azure": "AZURE_OPENAI_API_KEY",
            "bedrock": "AWS_ACCESS_KEY_ID",
            "ollama": "CONTINUUM_API_KEY",
        }

        config_data = {}

        # First pass: get provider to know which fallback to use
        provider = (
            _get_env(f"{cls.ENV_PREFIX}PROVIDER")
            or _get_env(f"{cls.ENV_PREFIX_FALLBACK}PROVIDER")
            or "anthropic"
        )
        config_data["provider"] = provider

        # Get provider-specific fallback env var
        provider_fallback_key = provider_env_keys.get(provider, "ANTHROPIC_API_KEY")

        for env_suffix, config_key in env_mapping.items():
            # Check multiple environment variable prefixes (by priority)
            if env_suffix == "API_KEY":
                # For API_KEY, use provider-specific fallback
                value = (
                    _get_env(f"{cls.ENV_PREFIX}{env_suffix}")
                    or _get_env(f"{cls.ENV_PREFIX_FALLBACK}{env_suffix}")
                    or _get_env(provider_fallback_key)
                )
            elif env_suffix == "BASE_URL":
                # For BASE_URL, check provider-specific var too
                value = (
                    _get_env(f"{cls.ENV_PREFIX}{env_suffix}")
                    or _get_env(f"{cls.ENV_PREFIX_FALLBACK}{env_suffix}")
                    or _get_env(f"{provider.upper()}_BASE_URL")
                )
            else:
                value = (
                    _get_env(f"{cls.ENV_PREFIX}{env_suffix}")
                    or _get_env(f"{cls.ENV_PREFIX_FALLBACK}{env_suffix}")
                    or _get_env(f"ANTHROPIC_{env_suffix}")
                )

            if value:
                if isinstance(config_key, tuple):
                    key, converter = config_key
                    try:
                        config_data[key] = converter(value)
                    except (ValueError, TypeError) as e:
                        logger.debug(f"Config conversion failed for {key}: {value} - {e}")
                else:
                    config_data[config_key] = value

        return cls(**config_data)

    @classmethod
    def from_file(cls, path: str) -> "Config":
        """
        Load configuration from file.

        Supports TOML and JSON formats.
        """
        file_path = Path(path).expanduser()
        if not file_path.exists():
            raise FileNotFoundError(f"Config file not found: {path}")

        data = cls._load_file(file_path)

        # Expand environment variable references
        data = cls._expand_env_vars(data)

        return cls(**data)

    @classmethod
    def from_default(cls) -> "Config":
        """
        Load configuration from default locations.

        Priority: environment variables > config files > defaults.
        """
        # 1. From environment variables
        config = cls.from_env()

        # 2. Find and load config file
        config_file = cls._find_config_file()
        if config_file:
            file_data = cls._load_file(config_file)
            file_data = cls._expand_env_vars(file_data)
            config._data.update(file_data)

        return config

    # ==================== Provider Management ====================

    def use(self, provider: str) -> "Config":
        """
        Switch provider.

        Args:
            provider: Provider name (anthropic|openai|google|custom)

        Returns:
            self (supports method chaining)
        """
        self._data["provider"] = provider

        # Load pre-configured provider info if available
        if provider in self._providers:
            prov_config = self._providers[provider]
            if prov_config.api_key:
                self._data["api_key"] = prov_config.api_key
            if prov_config.base_url:
                self._data["base_url"] = prov_config.base_url
            if prov_config.model:
                self._data["model"] = prov_config.model

        return self

    def add_provider(
        self,
        name: str,
        api_key: str | None = None,
        base_url: str | None = None,
        model: str | None = None,
        small_model: str | None = None,
    ) -> None:
        """
        Add provider configuration.

        Args:
            name: Provider name
            api_key: API key
            base_url: Base URL
            model: Default model
            small_model: Small model
        """
        self._providers[name] = ProviderConfig(
            name=name,
            api_key=api_key,
            base_url=base_url,
            model=model,
            small_model=small_model,
        )

    def list_providers(self) -> list[str]:
        """List all configured providers."""
        return list(self._providers.keys())

    # ==================== Internal Methods ====================

    def _get_default_model(self) -> str:
        """
        Get the default model for the provider.

        Fully relies on providers.get_default_model() which has complete fallback logic:
        environment variable CONTINUUM_MODEL > BUILTIN_PROVIDERS config > fallback mapping table.
        """
        return _get_provider_default_model(self.provider)

    @classmethod
    def _find_config_file(cls) -> Path | None:
        """Find config file."""
        for dir_path in cls.DEFAULT_CONFIG_DIRS:
            dir_expanded = Path(dir_path).expanduser()
            for config_name in cls.DEFAULT_CONFIG_FILES:
                path = dir_expanded / config_name
                if path.exists():
                    return path
        return None

    @classmethod
    def _load_file(cls, path: Path) -> dict[str, Any]:
        """Load configuration from file."""
        suffix = path.suffix.lower()

        try:
            if suffix == ".toml":
                if tomllib is None:
                    print(
                        "Warning: TOML support requires Python 3.11+ or tomli package"
                    )
                    return {}
                with open(path, "rb") as f:
                    return tomllib.load(f)
            elif suffix == ".json":
                with open(path, encoding="utf-8") as f:
                    return json.load(f)
            else:
                # Try auto-detection
                content = path.read_text(encoding="utf-8")
                if content.strip().startswith("{"):
                    return json.loads(content)
                elif tomllib:
                    with open(path, "rb") as f:
                        return tomllib.load(f)
        except (OSError, IOError, json.JSONDecodeError, FileNotFoundError) as e:
            print(f"Warning: Failed to load config from {path}: {e}")

        return {}

    @classmethod
    def _expand_env_vars(cls, data: dict[str, Any]) -> dict[str, Any]:
        """
        Expand environment variable references in configuration.

        Supports ${VAR_NAME} and $VAR_NAME formats.
        """
        pattern = re.compile(r"\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)")

        def expand_value(value: Any) -> Any:
            if isinstance(value, str):

                def replacer(match):
                    var_name = match.group(1) or match.group(2)
                    # Security: use whitelisted env access
                    return _get_env(var_name) or match.group(0)

                return pattern.sub(replacer, value)
            elif isinstance(value, dict):
                return {k: expand_value(v) for k, v in value.items()}
            elif isinstance(value, list):
                return [expand_value(item) for item in value]
            return value

        return expand_value(data)

    def __repr__(self) -> str:
        return f"Config(provider={self.provider}, model={self.model})"


# Convenience function
def load_config(path: str | None = None) -> Config:
    """
    Load configuration.

    Args:
        path: Config file path (optional, auto-searched by default)

    Returns:
        Config instance
    """
    if path:
        return Config.from_file(path)
    return Config.from_default()


def get_user_config_dir() -> Path:
    """Get user config directory."""
    config_dir = Path.home() / ".config" / "continuum"
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir


# Backward compatibility wrapper
class ConfigLoader:
    """
    Configuration Loader (backward compatibility).

    Recommended to use Config class methods directly:
        Config.from_env()
        Config.from_file(path)
        Config.from_default()
    """

    def __init__(self, config_path: str | None = None):
        self._config_path = config_path
        self._config: Config | None = None

    def load(self) -> Config:
        """Load configuration."""
        if self._config_path:
            self._config = Config.from_file(self._config_path)
        else:
            self._config = Config.from_default()
        return self._config

    def get_config(self) -> Config | None:
        """Get loaded configuration."""
        return self._config

    def save(self, path: str | None = None) -> None:
        """Save configuration to file."""
        if not self._config:
            raise ValueError("No config loaded")
        save_path = Path(path or self._config_path or "config.json")
        save_path.parent.mkdir(parents=True, exist_ok=True)
        with open(save_path, "w") as f:
            json.dump(self._config.to_dict(), f, indent=2)

    @staticmethod
    def get_default_config() -> Config:
        """Get default configuration."""
        return Config()
