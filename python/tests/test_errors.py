"""
Tests for unified error types module.

Tests all error classes, serialization, deserialization, and convenience functions.
"""

import pytest

from continuum_sdk.errors import (
    AuthenticationError,
    ConfigError,
    ContinuumError,
    ErrorContext,
    LLMError,
    RateLimitError,
    SecurityError,
    ToolExecutionError,
    ValidationError,
    config_error,
    security_error,
    tool_error,
    validation_error,
)


class TestErrorContext:
    """Tests for ErrorContext class."""

    def test_empty_context(self):
        ctx = ErrorContext()
        assert ctx.operation is None
        assert ctx.component is None
        assert ctx.suggestion is None

    def test_to_dict_empty(self):
        ctx = ErrorContext()
        assert ctx.to_dict() == {}

    def test_to_dict_with_values(self):
        ctx = ErrorContext(
            operation="read_file",
            component="file_ops",
            suggestion="Check file permissions",
            path="/tmp/test.txt",
        )
        result = ctx.to_dict()
        assert result["operation"] == "read_file"
        assert result["component"] == "file_ops"
        assert result["suggestion"] == "Check file permissions"
        assert result["path"] == "/tmp/test.txt"

    def test_to_dict_partial(self):
        ctx = ErrorContext(operation="test", suggestion="try again")
        result = ctx.to_dict()
        assert "operation" in result
        assert "suggestion" in result
        assert "component" not in result

    def test_get_method(self):
        ctx = ErrorContext(operation="test")
        assert ctx.get("operation") == "test"
        assert ctx.get("missing", "default") == "default"

    def test_contains(self):
        ctx = ErrorContext(operation="test")
        assert "operation" in ctx
        assert "missing" not in ctx

    def test_from_dict(self):
        ctx = ErrorContext.from_dict({"operation": "test", "key": "value"})
        assert ctx.operation == "test"
        assert ctx["key"] == "value"


class TestContinuumError:
    """Tests for base ContinuumError class."""

    def test_basic_creation(self):
        err = ContinuumError("Something went wrong")
        assert err.message == "Something went wrong"
        assert err.code == "UNKNOWN_ERROR"
        assert err.timestamp is not None

    def test_custom_code(self):
        err = ContinuumError("Error", code="E001")
        assert err.code == "E001"

    def test_custom_timestamp(self):
        ts = 1700000000.0
        err = ContinuumError("Error", timestamp=ts)
        assert err.timestamp == ts

    def test_context_dict(self):
        err = ContinuumError("Error", context={"operation": "test"})
        assert err.context.operation == "test"

    def test_context_object(self):
        ctx = ErrorContext(operation="test")
        err = ContinuumError("Error", context=ctx)
        assert err.context.operation == "test"

    def test_to_dict(self):
        err = ContinuumError("Test error", code="TEST001")
        result = err.to_dict()
        assert result["error_type"] == "ContinuumError"
        assert result["message"] == "Test error"
        assert result["code"] == "TEST001"
        assert "timestamp" in result
        assert "datetime" in result

    def test_to_dict_with_context(self):
        err = ContinuumError(
            "Error",
            context={"operation": "test", "suggestion": "retry"},
        )
        result = err.to_dict()
        assert "context" in result
        assert result["context"]["operation"] == "test"
        assert result["context"]["suggestion"] == "retry"

    def test_from_dict_basic(self):
        data = {
            "error_type": "ContinuumError",
            "message": "Test error",
            "code": "TEST001",
            "timestamp": 1700000000.0,
        }
        err = ContinuumError.from_dict(data)
        assert isinstance(err, ContinuumError)
        assert err.message == "Test error"
        assert err.code == "TEST001"
        assert err.timestamp == 1700000000.0

    def test_from_dict_with_context(self):
        data = {
            "error_type": "ContinuumError",
            "message": "Test",
            "code": "TEST001",
            "timestamp": 1700000000.0,
            "context": {"operation": "test"},
        }
        err = ContinuumError.from_dict(data)
        assert err.context.operation == "test"

    def test_str_representation(self):
        err = ContinuumError("Test error", code="E001")
        assert "[E001]" in str(err)
        assert "Test error" in str(err)

    def test_str_with_suggestion(self):
        err = ContinuumError(
            "Test error",
            code="E001",
            context={"suggestion": "Try again"},
        )
        result = str(err)
        assert "Suggestion: Try again" in result

    def test_repr(self):
        err = ContinuumError("Test", code="E001")
        assert "ContinuumError" in repr(err)
        assert "E001" in repr(err)

    def test_datetime_property(self):
        err = ContinuumError("Test")
        dt = err.datetime
        assert dt is not None


class TestConfigError:
    """Tests for ConfigError."""

    def test_default_code(self):
        err = ConfigError("Missing config")
        assert err.code == "CONFIG_ERROR"

    def test_custom_code(self):
        err = ConfigError("Missing key", code="CONFIG_MISSING_KEY")
        assert err.code == "CONFIG_MISSING_KEY"

    def test_inheritance(self):
        err = ConfigError("Test")
        assert isinstance(err, ContinuumError)


class TestToolExecutionError:
    """Tests for ToolExecutionError."""

    def test_basic_creation(self):
        err = ToolExecutionError("Tool failed")
        assert err.message == "Tool failed"
        assert err.code == "TOOL_EXECUTION_ERROR"

    def test_tool_name(self):
        err = ToolExecutionError("Failed", tool_name="read_file")
        assert err.tool_name == "read_file"

    def test_tool_args(self):
        err = ToolExecutionError(
            "Failed",
            tool_name="read_file",
            tool_args={"path": "/tmp/test.txt"},
        )
        assert err.tool_args == {"path": "/tmp/test.txt"}

    def test_context_includes_tool_info(self):
        err = ToolExecutionError(
            "Failed",
            tool_name="read_file",
            tool_args={"path": "/test.txt"},
        )
        result = err.to_dict()
        assert result["context"]["tool_name"] == "read_file"
        assert result["context"]["tool_args"]["path"] == "/test.txt"

    def test_inheritance(self):
        err = ToolExecutionError("Test")
        assert isinstance(err, ContinuumError)


class TestLLMError:
    """Tests for LLMError."""

    def test_basic_creation(self):
        err = LLMError("API failed")
        assert err.message == "API failed"
        assert err.code == "LLM_ERROR"

    def test_provider(self):
        err = LLMError("API failed", provider="anthropic")
        assert err.provider == "anthropic"

    def test_context_includes_provider(self):
        err = LLMError("Failed", provider="openai")
        result = err.to_dict()
        assert result["context"]["provider"] == "openai"

    def test_str_with_provider(self):
        err = LLMError("Failed", code="E001", provider="anthropic")
        result = str(err)
        assert "[anthropic]" in result

    def test_inheritance(self):
        err = LLMError("Test")
        assert isinstance(err, ContinuumError)


class TestAuthenticationError:
    """Tests for AuthenticationError."""

    def test_default_code(self):
        err = AuthenticationError("Auth failed")
        assert err.code == "AUTH_ERROR"

    def test_provider(self):
        err = AuthenticationError("Invalid key", provider="anthropic")
        assert err.provider == "anthropic"

    def test_inheritance(self):
        err = AuthenticationError("Test")
        assert isinstance(err, LLMError)
        assert isinstance(err, ContinuumError)


class TestRateLimitError:
    """Tests for RateLimitError."""

    def test_basic_creation(self):
        err = RateLimitError("Rate limited")
        assert err.message == "Rate limited"
        assert err.code == "RATE_LIMIT_ERROR"

    def test_retry_after(self):
        err = RateLimitError("Rate limited", retry_after=60)
        assert err.retry_after == 60

    def test_context_includes_retry_after(self):
        err = RateLimitError("Limited", retry_after=30)
        result = err.to_dict()
        assert result["context"]["retry_after"] == 30

    def test_str_with_retry_after(self):
        err = RateLimitError("Limited", code="RATE_LIMIT", retry_after=60)
        result = str(err)
        assert "Retry after: 60 seconds" in result

    def test_inheritance(self):
        err = RateLimitError("Test")
        assert isinstance(err, LLMError)
        assert isinstance(err, ContinuumError)


class TestSecurityError:
    """Tests for SecurityError."""

    def test_default_code(self):
        err = SecurityError("Violation")
        assert err.code == "SECURITY_ERROR"

    def test_context(self):
        err = SecurityError(
            "Path traversal",
            context={"path": "../../../etc/passwd"},
        )
        result = err.to_dict()
        assert result["context"]["path"] == "../../../etc/passwd"

    def test_inheritance(self):
        err = SecurityError("Test")
        assert isinstance(err, ContinuumError)


class TestValidationError:
    """Tests for ValidationError."""

    def test_basic_creation(self):
        err = ValidationError("Invalid value")
        assert err.message == "Invalid value"
        assert err.code == "VALIDATION_ERROR"

    def test_field_info(self):
        err = ValidationError(
            "Invalid temperature",
            field="temperature",
            value=3.0,
            valid_range="0.0-2.0",
        )
        assert err.field == "temperature"
        assert err.value == 3.0
        assert err.valid_range == "0.0-2.0"

    def test_context_includes_field_info(self):
        err = ValidationError(
            "Invalid",
            field="count",
            value=-1,
            valid_range=">=0",
        )
        result = err.to_dict()
        assert result["context"]["field"] == "count"
        assert result["context"]["value"] == -1
        assert result["context"]["valid_range"] == ">=0"

    def test_inheritance(self):
        err = ValidationError("Test")
        assert isinstance(err, ContinuumError)


class TestConvenienceFunctions:
    """Tests for convenience error creation functions."""

    def test_config_error_convenience(self):
        err = config_error(
            "API key not found",
            key="ANTHROPIC_API_KEY",
            suggestion="Set via environment variable",
        )
        assert isinstance(err, ConfigError)
        assert err.message == "API key not found"
        assert err.context["key"] == "ANTHROPIC_API_KEY"
        assert err.context.suggestion == "Set via environment variable"

    def test_tool_error_convenience(self):
        err = tool_error(
            "File not found",
            tool_name="read_file",
            tool_args={"path": "/tmp/test.txt"},
            suggestion="Check file path",
        )
        assert isinstance(err, ToolExecutionError)
        assert err.tool_name == "read_file"
        assert err.context["tool_name"] == "read_file"
        assert err.context["tool_args"]["path"] == "/tmp/test.txt"

    def test_validation_error_convenience(self):
        err = validation_error(
            "Invalid value",
            field="temperature",
            value=3.0,
            valid_range="0.0-2.0",
        )
        assert isinstance(err, ValidationError)
        assert err.field == "temperature"
        assert err.value == 3.0

    def test_security_error_convenience(self):
        err = security_error(
            "Command injection detected",
            operation="bash",
            suggestion="Use allowed commands only",
        )
        assert isinstance(err, SecurityError)
        assert err.context["operation"] == "bash"
        assert err.context.suggestion == "Use allowed commands only"


class TestSerializationRoundTrip:
    """Tests for to_dict/from_dict round-trip serialization."""

    @pytest.mark.parametrize(
        "error_class,message,extra_kwargs",
        [
            (ContinuumError, "Base error", {"code": "E001"}),
            (ConfigError, "Config missing", {}),
            (ToolExecutionError, "Tool failed", {"tool_name": "test"}),
            (LLMError, "LLM error", {"provider": "anthropic"}),
            (AuthenticationError, "Auth failed", {"provider": "openai"}),
            (RateLimitError, "Rate limited", {"retry_after": 60}),
            (SecurityError, "Security violation", {}),
            (ValidationError, "Validation failed", {"field": "test"}),
        ],
    )
    def test_round_trip(self, error_class, message, extra_kwargs):
        original = error_class(message, **extra_kwargs)
        data = original.to_dict()
        restored = ContinuumError.from_dict(data)

        assert restored.message == original.message
        assert restored.code == original.code
        assert type(restored).__name__ == error_class.__name__

    def test_round_trip_preserves_context(self):
        original = ContinuumError(
            "Test",
            code="TEST",
            context={"operation": "test", "suggestion": "retry"},
        )
        data = original.to_dict()
        restored = ContinuumError.from_dict(data)

        assert restored.context.operation == "test"
        assert restored.context.suggestion == "retry"


class TestErrorHandling:
    """Tests for error handling patterns."""

    def test_catch_base_error(self):
        with pytest.raises(ContinuumError) as exc_info:
            raise ConfigError("Test")
        assert isinstance(exc_info.value, ConfigError)

    def test_catch_llm_error_hierarchy(self):
        with pytest.raises(LLMError):
            raise AuthenticationError("Test")

        with pytest.raises(LLMError):
            raise RateLimitError("Test")

    def test_multiple_error_types_in_except(self):
        errors = []
        try:
            raise ConfigError("Config")
        except (ConfigError, SecurityError, ValidationError) as e:
            errors.append(e)

        try:
            raise SecurityError("Security")
        except (ConfigError, SecurityError, ValidationError) as e:
            errors.append(e)

        assert len(errors) == 2
        assert isinstance(errors[0], ConfigError)
        assert isinstance(errors[1], SecurityError)


class TestMissingCoverage:
    """Tests for missing coverage branches."""

    def test_tool_execution_error_with_existing_context(self):
        """Test ToolExecutionError merges tool info into existing context dict (line 253->259)."""
        existing_context = {"operation": "test_op", "suggestion": "retry"}
        err = ToolExecutionError(
            "Failed",
            tool_name="read_file",
            tool_args={"path": "/test.txt"},
            context=existing_context,
        )
        # Context should have both existing and tool-specific info
        assert err.context["operation"] == "test_op"
        assert err.context["tool_name"] == "read_file"
        assert err.context["tool_args"]["path"] == "/test.txt"

    def test_tool_execution_error_context_is_none(self):
        """Test ToolExecutionError when context is None."""
        err = ToolExecutionError(
            "Failed",
            tool_name="read_file",
            tool_args={"path": "/test.txt"},
            context=None,
        )
        assert err.context["tool_name"] == "read_file"

    def test_rate_limit_error_str_without_retry_after(self):
        """Test RateLimitError.__str__ without retry_after."""
        err = RateLimitError("Rate limited", code="RATE_LIMIT", provider="openai")
        result = str(err)
        # Should not include retry_after message
        assert "Retry after" not in result

    def test_validation_error_with_existing_context(self):
        """Test ValidationError merges field info into existing context dict (line 438->446)."""
        existing_context = {"operation": "validate"}
        err = ValidationError(
            "Invalid value",
            field="temperature",
            value=3.0,
            valid_range="0.0-2.0",
            context=existing_context,
        )
        assert err.context["operation"] == "validate"
        assert err.context["field"] == "temperature"
        assert err.context["value"] == 3.0
        assert err.context["valid_range"] == "0.0-2.0"

    def test_validation_error_context_is_none(self):
        """Test ValidationError when context is None."""
        err = ValidationError(
            "Invalid value",
            field="count",
            value=-1,
            valid_range=">=0",
            context=None,
        )
        assert err.field == "count"
        assert err.context["field"] == "count"

    def test_validation_error_without_suggestion(self):
        """Test validation_error convenience function without suggestion (line 461->463)."""
        err = validation_error(
            "Invalid value",
            field="count",
            value=-1,
            valid_range=">=0",
        )
        assert isinstance(err, ValidationError)
        assert err.field == "count"
        assert err.value == -1
        assert err.valid_range == ">=0"
        assert err.context.suggestion is None

    def test_validation_error_with_valid_range(self):
        """Test validation_error convenience function with valid_range (line 463->465)."""
        err = validation_error(
            "Invalid value",
            field="temperature",
            value=3.0,
            valid_range="0.0-2.0",
        )
        assert err.valid_range == "0.0-2.0"
        assert err.context["valid_range"] == "0.0-2.0"

    def test_security_error_without_operation(self):
        """Test security_error convenience function without operation."""
        err = security_error(
            "Path traversal detected",
            suggestion="Use absolute paths",
        )
        assert isinstance(err, SecurityError)
        assert err.context["suggestion"] == "Use absolute paths"
        assert err.context.operation is None

    def test_security_error_without_suggestion(self):
        """Test security_error convenience function without suggestion."""
        err = security_error(
            "Command injection detected",
            operation="bash",
        )
        assert isinstance(err, SecurityError)
        assert err.context["operation"] == "bash"
        assert err.context.suggestion is None

    def test_security_error_neither_operation_nor_suggestion(self):
        """Test security_error convenience function with neither."""
        err = security_error("Security violation")
        assert isinstance(err, SecurityError)
        assert err.context.operation is None
        assert err.context.suggestion is None
        assert err.context.to_dict() == {}


class TestRemainingBranchCoverage:
    """Tests for remaining branch coverage in errors.py."""

    def test_tool_execution_error_context_is_error_context(self):
        """Test ToolExecutionError when context is ErrorContext object (line 253->259)."""
        # Pass an ErrorContext object directly (not a dict)
        ctx = ErrorContext(operation="test_op")
        err = ToolExecutionError(
            "Failed",
            tool_name="read_file",
            tool_args={"path": "/test.txt"},
            context=ctx,
        )
        # The ErrorContext should be used, tool_name/tool_args should NOT be added
        # because isinstance(context, dict) is False
        assert err.context.operation == "test_op"
        assert err.tool_name == "read_file"
        assert err.tool_args == {"path": "/test.txt"}

    def test_llm_error_context_is_error_context_with_provider(self):
        """Test LLMError when context is ErrorContext object with provider."""
        ctx = ErrorContext(operation="api_call")
        err = LLMError("API failed", provider="anthropic", context=ctx)
        # provider should NOT be added to context because isinstance(context, dict) is False
        assert err.context.operation == "api_call"
        assert err.provider == "anthropic"

    def test_rate_limit_error_context_is_error_context(self):
        """Test RateLimitError when context is ErrorContext object."""
        ctx = ErrorContext(operation="rate_check")
        err = RateLimitError("Rate limited", retry_after=60, context=ctx)
        # retry_after should NOT be added to context because isinstance(context, dict) is False
        assert err.context.operation == "rate_check"
        assert err.retry_after == 60

    def test_validation_error_context_is_error_context(self):
        """Test ValidationError when context is ErrorContext object (line 438->446)."""
        ctx = ErrorContext(component="validator")
        err = ValidationError(
            "Invalid value",
            field="temperature",
            value=3.0,
            valid_range="0.0-2.0",
            context=ctx,
        )
        # field/value/valid_range should NOT be added to context because isinstance(context, dict) is False
        assert err.context.component == "validator"
        assert err.field == "temperature"
        assert err.value == 3.0
        assert err.valid_range == "0.0-2.0"

    def test_config_error_convenience_without_key(self):
        """Test config_error convenience function without key (line 461->463)."""
        err = config_error("Missing config", suggestion="Set the config")
        assert isinstance(err, ConfigError)
        assert err.context.suggestion == "Set the config"
        assert "key" not in err.context.to_dict()

    def test_config_error_convenience_without_suggestion(self):
        """Test config_error convenience function without suggestion (line 463->465)."""
        err = config_error("Missing config", key="API_KEY")
        assert isinstance(err, ConfigError)
        assert err.context["key"] == "API_KEY"
        assert err.context.suggestion is None

    def test_tool_error_convenience_without_suggestion(self):
        """Test tool_error convenience function without suggestion."""
        err = tool_error(
            "Tool failed", tool_name="read_file", tool_args={"path": "/test"}
        )
        assert isinstance(err, ToolExecutionError)
        assert err.tool_name == "read_file"
        assert err.context.suggestion is None
