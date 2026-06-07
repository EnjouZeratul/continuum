"""
Unified Error Types for Continuum SDK

This module provides a comprehensive error hierarchy for the SDK.
All errors inherit from ContinuumError for consistent handling.

Error Hierarchy:
    ContinuumError (base)
    ├── ConfigError - Configuration issues
    ├── ToolExecutionError - Tool execution failures
    ├── LLMError - LLM API errors (from llm.errors)
    │   ├── AuthenticationError
    │   ├── RateLimitError
    │   ├── NetworkError
    │   ├── TimeoutError
    │   └── ...
    ├── SecurityError - Security violations
    └── ValidationError - Input validation failures
"""

from __future__ import annotations

import time
from datetime import datetime
from typing import Any


class ErrorContext:
    """
    Additional context for error diagnosis and recovery.

    Acts as a dict-like container for arbitrary context data
    with common convenience properties.
    """

    def __init__(
        self,
        operation: str | None = None,
        component: str | None = None,
        suggestion: str | None = None,
        **kwargs: Any,
    ):
        self._data: dict[str, Any] = {}
        if operation:
            self._data["operation"] = operation
        if component:
            self._data["component"] = component
        if suggestion:
            self._data["suggestion"] = suggestion
        self._data.update(kwargs)

    @property
    def operation(self) -> str | None:
        return self._data.get("operation")

    @property
    def component(self) -> str | None:
        return self._data.get("component")

    @property
    def suggestion(self) -> str | None:
        return self._data.get("suggestion")

    def get(self, key: str, default: Any = None) -> Any:
        return self._data.get(key, default)

    def __getitem__(self, key: str) -> Any:
        return self._data[key]

    def __contains__(self, key: str) -> bool:
        return key in self._data

    def to_dict(self) -> dict[str, Any]:
        return dict(self._data)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ErrorContext:
        return cls(**data)


class ContinuumError(Exception):
    """
    Base error class for all Continuum SDK errors.

    All SDK-specific errors inherit from this class for unified error handling.

    Attributes:
        message: Human-readable error description
        code: Error code for programmatic handling
        timestamp: When the error occurred
        context: Additional diagnostic context

    Example:
        >>> try:
        ...     raise ContinuumError("Something went wrong", code="E001")
        ... except ContinuumError as e:
        ...     print(e.to_dict())
    """

    default_code: str = "UNKNOWN_ERROR"
    default_message: str = "An error occurred"

    def __init__(
        self,
        message: str | None = None,
        *,
        code: str | None = None,
        timestamp: float | None = None,
        context: ErrorContext | dict[str, Any] | None = None,
    ):
        self.message = message or self.default_message
        self.code = code or self.default_code
        self.timestamp = timestamp or time.time()
        self._context_data = context

        super().__init__(self.message)

    @property
    def context(self) -> ErrorContext:
        """Get the error context as an ErrorContext object."""
        if isinstance(self._context_data, ErrorContext):
            return self._context_data
        elif isinstance(self._context_data, dict):
            return ErrorContext(**self._context_data)
        return ErrorContext()

    @property
    def datetime(self) -> datetime:
        """Get the timestamp as a datetime object."""
        return datetime.fromtimestamp(self.timestamp)

    def to_dict(self) -> dict[str, Any]:
        """
        Serialize error to dictionary for logging, API responses, or persistence.

        Returns:
            Dictionary containing all error information
        """
        result = {
            "error_type": self.__class__.__name__,
            "message": self.message,
            "code": self.code,
            "timestamp": self.timestamp,
            "datetime": self.datetime.isoformat(),
        }
        ctx = self.context.to_dict()
        if ctx:
            result["context"] = ctx
        return result

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ContinuumError:
        """
        Deserialize error from dictionary.

        Args:
            data: Dictionary containing error information

        Returns:
            Appropriate error instance
        """
        error_type = data.get("error_type", "ContinuumError")
        message = data.get("message")
        code = data.get("code")
        timestamp = data.get("timestamp")
        context_data = data.get("context")

        # Map error type to class
        error_classes = {
            "ContinuumError": ContinuumError,
            "ConfigError": ConfigError,
            "ToolExecutionError": ToolExecutionError,
            "LLMError": LLMError,
            "AuthenticationError": AuthenticationError,
            "RateLimitError": RateLimitError,
            "SecurityError": SecurityError,
            "ValidationError": ValidationError,
        }

        error_cls = error_classes.get(error_type, cls)

        return error_cls(
            message=message,
            code=code,
            timestamp=timestamp,
            context=context_data,
        )

    def __str__(self) -> str:
        parts = [f"[{self.code}] {self.message}"]
        if self.context.suggestion:
            parts.append(f"Suggestion: {self.context.suggestion}")
        return "\n".join(parts)

    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message={self.message!r}, code={self.code!r})"


class ConfigError(ContinuumError):
    """
    Raised when configuration is invalid or missing.

    Common causes:
        - Missing required configuration key
        - Invalid configuration value
        - Configuration file not found or unreadable

    Example:
        >>> raise ConfigError(
        ...     "API key not found",
        ...     code="CONFIG_MISSING_KEY",
        ...     context={"key": "ANTHROPIC_API_KEY", "suggestion": "Set via environment variable"}
        ... )
    """

    default_code = "CONFIG_ERROR"
    default_message = "Configuration error"


class ToolExecutionError(ContinuumError):
    """
    Raised when tool execution fails.

    Attributes:
        tool_name: Name of the tool that failed
        tool_args: Arguments passed to the tool

    Example:
        >>> raise ToolExecutionError(
        ...     "File not found: /path/to/file.txt",
        ...     code="TOOL_FILE_NOT_FOUND",
        ...     context={"tool_name": "read_file", "tool_args": {"path": "/path/to/file.txt"}}
        ... )
    """

    default_code = "TOOL_EXECUTION_ERROR"
    default_message = "Tool execution failed"

    def __init__(
        self,
        message: str | None = None,
        *,
        code: str | None = None,
        timestamp: float | None = None,
        context: ErrorContext | dict[str, Any] | None = None,
        tool_name: str | None = None,
        tool_args: dict[str, Any] | None = None,
    ):
        # Merge tool-specific info into context
        if context is None:
            context = {}
        if isinstance(context, dict):
            if tool_name:
                context["tool_name"] = tool_name
            if tool_args:
                context["tool_args"] = tool_args

        super().__init__(message=message, code=code, timestamp=timestamp, context=context)
        self.tool_name = tool_name
        self.tool_args = tool_args or {}


class LLMError(ContinuumError):
    """
    Base error for LLM API operations.

    All LLM-related errors inherit from this class.

    Attributes:
        provider: The LLM provider (e.g., 'anthropic', 'openai')

    Example:
        >>> raise LLMError(
        ...     "Model not available",
        ...     code="LLM_MODEL_NOT_FOUND",
        ...     context={"provider": "anthropic", "model": "claude-3-opus"}
        ... )
    """

    default_code = "LLM_ERROR"
    default_message = "LLM operation failed"

    def __init__(
        self,
        message: str | None = None,
        *,
        code: str | None = None,
        timestamp: float | None = None,
        context: ErrorContext | dict[str, Any] | None = None,
        provider: str | None = None,
    ):
        # Merge provider info into context
        if context is None:
            context = {}
        if isinstance(context, dict) and provider:
            context["provider"] = provider

        super().__init__(message=message, code=code, timestamp=timestamp, context=context)
        self.provider = provider

    def __str__(self) -> str:
        prefix = f"[{self.provider}] " if self.provider else ""
        return f"[{self.code}] {prefix}{self.message}"


class AuthenticationError(LLMError):
    """
    Raised when API authentication fails.

    Common causes:
        - Invalid API key
        - Expired API key
        - Wrong API key for provider
        - Missing API key

    Example:
        >>> raise AuthenticationError(
        ...     "Invalid API key",
        ...     code="AUTH_INVALID_KEY",
        ...     provider="anthropic",
        ...     context={"suggestion": "Check your API key in the dashboard"}
        ... )
    """

    default_code = "AUTH_ERROR"
    default_message = "Authentication failed"


class RateLimitError(LLMError):
    """
    Raised when rate limit is exceeded.

    Attributes:
        retry_after: Seconds to wait before retry (if provided by API)

    Example:
        >>> raise RateLimitError(
        ...     "Rate limit exceeded",
        ...     code="RATE_LIMIT",
        ...     provider="openai",
        ...     context={"retry_after": 60}
        ... )
    """

    default_code = "RATE_LIMIT_ERROR"
    default_message = "Rate limit exceeded"

    def __init__(
        self,
        message: str | None = None,
        *,
        code: str | None = None,
        timestamp: float | None = None,
        context: ErrorContext | dict[str, Any] | None = None,
        provider: str | None = None,
        retry_after: float | None = None,
    ):
        # Merge retry_after into context
        if context is None:
            context = {}
        if isinstance(context, dict) and retry_after is not None:
            context["retry_after"] = retry_after

        super().__init__(
            message=message,
            code=code,
            timestamp=timestamp,
            context=context,
            provider=provider,
        )
        self.retry_after = retry_after

    def __str__(self) -> str:
        base = super().__str__()
        if self.retry_after:
            base += f"\nRetry after: {self.retry_after} seconds"
        return base


class SecurityError(ContinuumError):
    """
    Raised when a security violation is detected.

    Common causes:
        - Path traversal attempt
        - Command injection attempt
        - Unauthorized access
        - Dangerous operation blocked

    Example:
        >>> raise SecurityError(
        ...     "Path traversal detected",
        ...     code="SEC_PATH_TRAVERSAL",
        ...     context={"path": "../../../etc/passwd", "suggestion": "Use absolute paths"}
        ... )
    """

    default_code = "SECURITY_ERROR"
    default_message = "Security violation detected"


class ValidationError(ContinuumError):
    """
    Raised when input validation fails.

    Common causes:
        - Invalid parameter type
        - Value out of range
        - Missing required field
        - Format mismatch

    Example:
        >>> raise ValidationError(
        ...     "Invalid temperature value",
        ...     code="VALIDATION_INVALID_VALUE",
        ...     context={"field": "temperature", "value": 3.0, "valid_range": "0.0-2.0"}
        ... )
    """

    default_code = "VALIDATION_ERROR"
    default_message = "Validation failed"

    def __init__(
        self,
        message: str | None = None,
        *,
        code: str | None = None,
        timestamp: float | None = None,
        context: ErrorContext | dict[str, Any] | None = None,
        field: str | None = None,
        value: Any = None,
        valid_range: str | None = None,
    ):
        # Merge validation info into context
        if context is None:
            context = {}
        if isinstance(context, dict):
            if field:
                context["field"] = field
            if value is not None:
                context["value"] = value
            if valid_range:
                context["valid_range"] = valid_range

        super().__init__(message=message, code=code, timestamp=timestamp, context=context)
        self.field = field
        self.value = value
        self.valid_range = valid_range


# Convenience functions for creating common errors

def config_error(
    message: str,
    key: str | None = None,
    suggestion: str | None = None,
) -> ConfigError:
    """Create a ConfigError with helpful context."""
    context = {}
    if key:
        context["key"] = key
    if suggestion:
        context["suggestion"] = suggestion
    return ConfigError(message, context=context)


def tool_error(
    message: str,
    tool_name: str,
    tool_args: dict[str, Any] | None = None,
    suggestion: str | None = None,
) -> ToolExecutionError:
    """Create a ToolExecutionError with tool context."""
    return ToolExecutionError(
        message,
        tool_name=tool_name,
        tool_args=tool_args,
        context={"suggestion": suggestion} if suggestion else None,
    )


def validation_error(
    message: str,
    field: str,
    value: Any = None,
    valid_range: str | None = None,
) -> ValidationError:
    """Create a ValidationError with field context."""
    return ValidationError(
        message,
        field=field,
        value=value,
        valid_range=valid_range,
    )


def security_error(
    message: str,
    operation: str | None = None,
    suggestion: str | None = None,
) -> SecurityError:
    """Create a SecurityError with operation context."""
    context = {}
    if operation:
        context["operation"] = operation
    if suggestion:
        context["suggestion"] = suggestion
    return SecurityError(message, context=context)


__all__ = [
    # Base
    "ContinuumError",
    "ErrorContext",
    # Config
    "ConfigError",
    # Tool
    "ToolExecutionError",
    # LLM
    "LLMError",
    "AuthenticationError",
    "RateLimitError",
    # Security
    "SecurityError",
    # Validation
    "ValidationError",
    # Convenience functions
    "config_error",
    "tool_error",
    "validation_error",
    "security_error",
]
