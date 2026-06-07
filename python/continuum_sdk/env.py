"""
Continuum SDK Environment Variable Access

Provides type-safe environment variable access with CONTINUUM_ prefix support.
White-list as documentation purpose, using non-documented variables will log warnings.

Design rationale:
- Python has no sandbox, whitelist cannot prevent malicious code
- Whitelist value: documentation + discover new variables + type safety
- Soft constraint: warning instead of blocking, reduces maintenance burden

提供类型安全的环境变量读取接口，支持自动类型转换和 CONTINUUM_ 前缀。
白名单作为文档用途，使用非白名单变量会记录警告日志。

设计原理：
- Python无沙箱，白名单无法阻止恶意代码
- 白名单价值：文档化 + 发现新变量 + 类型安全
- 软约束：警告而非阻塞，减少维护负担

Usage (用法):
    >>> from continuum_sdk.env import get_str, get_int, get_bool, get_list
    >>> # Get string value
    >>> api_key = get_str("API_KEY")  # Checks CONTINUUM_API_KEY, then API_KEY
    >>> # Get integer with default
    >>> timeout = get_int("TIMEOUT", default=30)
    >>> # Get boolean
    >>> debug = get_bool("DEBUG", default=False)
    >>> # Get list (comma-separated)
    >>> models = get_list("MODELS", default=["gpt-4", "claude-3"])

Environment Variable Prefix Priority (环境变量前缀优先级):
    1. CONTINUUM_{NAME} (preferred/推荐)
    2. {NAME} (fallback/回退)

Type Conversion Rules (类型转换规则):
    - String: Direct value, no conversion
    - Integer: Parses decimal integers, returns default on invalid input
    - Boolean: "true", "1", "yes", "on" (case-insensitive) -> True
               "false", "0", "no", "off" (case-insensitive) -> False
    - List: Comma-separated values, trimmed of whitespace
"""

from __future__ import annotations

import logging
import os
from typing import Final

logger = logging.getLogger(__name__)

# Documented environment variables (whitelist preserved for documentation purposes)
# 文档化的环境变量（白名单保留用于文档目的）
DOCUMENTED_ENV_VARS: Final[frozenset[str]] = frozenset({
    # Core configuration / 核心配置
    "API_KEY",
    "BASE_URL",
    "PROVIDER",
    "MODEL",
    "SMALL_MODEL",
    "DEFAULT_MODEL",
    "API_FORMAT",

    # Runtime settings / 运行时设置
    "LOG_LEVEL",
    "MAX_TOKENS",
    "TIMEOUT",
    "MAX_ITERATIONS",
    "TEMPERATURE",
    "EFFORT_LEVEL",

    # Feature flags / 功能开关
    "DEBUG",
    "VERBOSE",
    "DISABLE_TRAFFIC",
    "AUDIT_ENABLED",

    # Paths / 路径配置
    "WORKTREES_DIR",
    "PLUGINS_DIR",
    "AUDIT_LOG_PATH",
    "THEME_CONFIG",

    # List-type configurations / 列表类型配置
    "MODELS",
    "ALLOWED_TOOLS",
    "BLOCKED_TOOLS",
    "EXTRA_HEADERS",

    # Retention / 保留设置
    "AUDIT_RETENTION",

    # Test support / 测试支持
    "USE_REAL_API",
})

# Environment variable prefix / 环境变量前缀
ENV_PREFIX: Final[str] = "CONTINUUM_"


def _check_documented(name: str) -> None:
    """
    Check if variable is in documented list, log warning if not.

    检查变量是否在文档列表中，若不在则记录警告。

    Args:
        name: Variable name to check
              要检查的变量名
    """
    if name not in DOCUMENTED_ENV_VARS:
        logger.warning(
            f"Accessing non-documented env var '{name}'. "
            f"Consider adding to DOCUMENTED_ENV_VARS for consistency."
        )


def _resolve_name(name: str) -> str | None:
    """
    Resolve environment variable name with CONTINUUM_ prefix support.

    解析环境变量名称，支持 CONTINUUM_ 前缀。

    Priority (优先级):
        1. CONTINUUM_{NAME}
        2. {NAME}

    Args:
        name: Base name of the environment variable
              环境变量的基本名称

    Returns:
        The resolved environment variable name, or None if not found
        解析后的环境变量名称，如果未找到则返回 None
    """
    # Check if documented (soft constraint - warning only)
    # 检查是否在文档列表中（软约束 - 仅警告）
    _check_documented(name)

    # Check CONTINUUM_ prefix first / 优先检查 CONTINUUM_ 前缀
    prefixed_name = f"{ENV_PREFIX}{name}"
    if prefixed_name in os.environ:
        return prefixed_name

    # Fallback to base name / 回退到基本名称
    if name in os.environ:
        return name

    return None


def get_str(name: str, default: str | None = None) -> str | None:
    """
    Get a string environment variable.

    获取字符串类型的环境变量。

    This function retrieves an environment variable as a string, with support
    for the CONTINUUM_ prefix. The prefix version takes priority over the
    non-prefixed version.

    此函数获取字符串类型的环境变量，支持 CONTINUUM_ 前缀。带前缀的版本优先于
    不带前缀的版本。

    Args:
        name: Base name of the environment variable (without CONTINUUM_ prefix)
              For example, "API_KEY" will check CONTINUUM_API_KEY first, then API_KEY.
              环境变量的基本名称（不含 CONTINUUM_ 前缀）
              例如，"API_KEY" 会先检查 CONTINUUM_API_KEY，然后检查 API_KEY。
        default: Default value if the variable is not set
                 如果变量未设置时的默认值

    Returns:
        The environment variable value as a string, or the default value
        字符串类型的环境变量值，或默认值

    Example:
        >>> # If CONTINUUM_API_KEY="secret123" is set
        >>> get_str("API_KEY")
        'secret123'
        >>> # If neither CONTINUUM_API_KEY nor API_KEY is set
        >>> get_str("API_KEY", default="none")
        'none'
    """
    resolved = _resolve_name(name)
    if resolved is None:
        return default
    return os.environ.get(resolved, default)


def get_int(name: str, default: int | None = None) -> int | None:
    """
    Get an integer environment variable.

    获取整数类型的环境变量。

    This function retrieves an environment variable and converts it to an integer.
    It supports the CONTINUUM_ prefix, with the prefixed version taking priority.

    此函数获取环境变量并将其转换为整数。支持 CONTINUUM_ 前缀，带前缀的版本优先。

    Args:
        name: Base name of the environment variable (without CONTINUUM_ prefix)
              环境变量的基本名称（不含 CONTINUUM_ 前缀）
        default: Default value if the variable is not set or conversion fails
                 如果变量未设置或转换失败时的默认值

    Returns:
        The environment variable value as an integer, or the default value
        整数类型的环境变量值，或默认值

    Example:
        >>> # If CONTINUUM_TIMEOUT="30" is set
        >>> get_int("TIMEOUT")
        30
        >>> # If CONTINUUM_MAX_TOKENS="invalid" is set
        >>> get_int("MAX_TOKENS", default=4096)
        4096  # Returns default due to conversion failure
    """
    resolved = _resolve_name(name)
    if resolved is None:
        return default

    value = os.environ.get(resolved)
    if value is None:
        return default

    try:
        return int(value)
    except (ValueError, TypeError):
        logger.warning(f"Cannot convert env var '{name}' to int: {value}")
        return default


def get_bool(name: str, default: bool | None = None) -> bool | None:
    """
    Get a boolean environment variable.

    获取布尔类型的环境变量。

    This function retrieves an environment variable and converts it to a boolean.
    It supports the CONTINUUM_ prefix, with the prefixed version taking priority.

    此函数获取环境变量并将其转换为布尔值。支持 CONTINUUM_ 前缀，带前缀的版本优先。

    Truthy values (case-insensitive): "true", "1", "yes", "on"
    Falsy values (case-insensitive): "false", "0", "no", "off"

    真值（不区分大小写）: "true", "1", "yes", "on"
    假值（不区分大小写）: "false", "0", "no", "off"

    Args:
        name: Base name of the environment variable (without CONTINUUM_ prefix)
              环境变量的基本名称（不含 CONTINUUM_ 前缀）
        default: Default value if the variable is not set or has an unrecognized value
                 如果变量未设置或值无法识别时的默认值

    Returns:
        The environment variable value as a boolean, or the default value
        布尔类型的环境变量值，或默认值

    Example:
        >>> # If CONTINUUM_DEBUG="true" is set
        >>> get_bool("DEBUG")
        True
        >>> # If CONTINUUM_VERBOSE="0" is set
        >>> get_bool("VERBOSE")
        False
        >>> # If CONTINUUM_AUDIT_ENABLED="yes" is set
        >>> get_bool("AUDIT_ENABLED")
        True
    """
    resolved = _resolve_name(name)
    if resolved is None:
        return default

    value = os.environ.get(resolved)
    if value is None:
        return default

    value_lower = value.lower().strip()

    if value_lower in ("true", "1", "yes", "on"):
        return True
    elif value_lower in ("false", "0", "no", "off"):
        return False

    return default


def get_list(name: str, default: list[str] | None = None) -> list[str] | None:
    """
    Get a list environment variable (comma-separated values).

    获取列表类型的环境变量（逗号分隔值）。

    This function retrieves an environment variable and splits it into a list
    using comma as the delimiter. Whitespace around values is trimmed.
    It supports the CONTINUUM_ prefix, with the prefixed version taking priority.

    此函数获取环境变量并使用逗号作为分隔符将其拆分为列表。值周围的空白会被去除。
    支持 CONTINUUM_ 前缀，带前缀的版本优先。

    Args:
        name: Base name of the environment variable (without CONTINUUM_ prefix)
              环境变量的基本名称（不含 CONTINUUM_ 前缀）
        default: Default value if the variable is not set
                 如果变量未设置时的默认值

    Returns:
        The environment variable value as a list of strings, or the default value
        字符串列表类型的环境变量值，或默认值

    Example:
        >>> # If CONTINUUM_MODELS="gpt-4, claude-3, gemini-pro" is set
        >>> get_list("MODELS")
        ['gpt-4', 'claude-3', 'gemini-pro']
        >>> # If CONTINUUM_ALLOWED_TOOLS="read,write,execute" is set
        >>> get_list("ALLOWED_TOOLS")
        ['read', 'write', 'execute']
        >>> # Empty string results in empty list
        >>> # If CONTINUUM_MODELS="" is set
        >>> get_list("MODELS", default=["default-model"])
        []
    """
    resolved = _resolve_name(name)
    if resolved is None:
        return default

    value = os.environ.get(resolved)
    if value is None:
        return default

    if not value.strip():
        return []

    return [item.strip() for item in value.split(",") if item.strip()]


__all__ = [
    "get_str",
    "get_int",
    "get_bool",
    "get_list",
    "DOCUMENTED_ENV_VARS",
    "ENV_PREFIX",
]