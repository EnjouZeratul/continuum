"""
Tests for environment variable access module.
环境变量访问模块测试

This module tests the type-safe environment variable access functions with
CONTINUUM_ prefix support and soft constraint (warning instead of error).

本模块测试类型安全的环境变量访问函数，包括 CONTINUUM_ 前缀支持和软约束机制（警告而非错误）。
"""

import logging
import os

import pytest

from continuum_sdk.env import (
    DOCUMENTED_ENV_VARS,
    ENV_PREFIX,
    get_bool,
    get_int,
    get_list,
    get_str,
)


class TestSoftConstraint:
    """Tests for soft constraint behavior.
    软约束行为测试。"""

    def test_non_documented_var_logs_warning(self, caplog):
        """Non-whitelisted variable should log warning.
        非白名单变量应记录警告。"""
        with caplog.at_level(logging.WARNING):
            result = get_str("NON_DOCUMENTED_VAR")
        assert "non-documented env var" in caplog.text
        assert result is None  # But does not raise exception

    def test_documented_var_no_warning(self, caplog, monkeypatch):
        """Whitelisted variable should not log warning.
        白名单变量不应警告。"""
        monkeypatch.setenv("API_KEY", "test")
        with caplog.at_level(logging.WARNING):
            result = get_str("API_KEY")
        assert "non-documented" not in caplog.text
        assert result == "test"

    def test_non_documented_with_value_logs_warning(self, caplog, monkeypatch):
        """Non-whitelisted variable with value should still log warning.
        有值的非白名单变量仍应记录警告。"""
        monkeypatch.setenv("CUSTOM_VAR", "custom_value")
        with caplog.at_level(logging.WARNING):
            result = get_str("CUSTOM_VAR")
        assert "non-documented env var 'CUSTOM_VAR'" in caplog.text
        assert result == "custom_value"  # But value is still returned

    def test_all_get_functions_use_soft_constraint(self, caplog, monkeypatch):
        """All get_* functions should use soft constraint.
        所有 get_* 函数都应使用软约束。"""
        with caplog.at_level(logging.WARNING):
            # get_str
            get_str("UNKNOWN_VAR")
            assert "non-documented env var 'UNKNOWN_VAR'" in caplog.text

            # get_int
            caplog.clear()
            get_int("UNKNOWN_INT")
            assert "non-documented env var 'UNKNOWN_INT'" in caplog.text

            # get_bool
            caplog.clear()
            get_bool("UNKNOWN_BOOL")
            assert "non-documented env var 'UNKNOWN_BOOL'" in caplog.text

            # get_list
            caplog.clear()
            get_list("UNKNOWN_LIST")
            assert "non-documented env var 'UNKNOWN_LIST'" in caplog.text


class TestGetStr:
    """Tests for get_str function.
    get_str 函数测试。"""

    def test_get_str_returns_string_value(self, monkeypatch):
        """Test that get_str correctly returns a string value.
        测试 get_str 正确返回字符串值。"""
        monkeypatch.setenv("API_KEY", "test-value-123")
        result = get_str("API_KEY")
        assert result == "test-value-123"

    def test_get_str_returns_default_when_not_set(self, monkeypatch):
        """Test that get_str returns default when variable is not set.
        测试变量未设置时 get_str 返回默认值。"""
        monkeypatch.delenv("API_KEY", raising=False)
        result = get_str("API_KEY", default="default-value")
        assert result == "default-value"

    def test_get_str_returns_none_when_not_set_and_no_default(self, monkeypatch):
        """Test that get_str returns None when variable is not set and no default provided.
        测试变量未设置且无默认值时 get_str 返回 None。"""
        monkeypatch.delenv("API_KEY", raising=False)
        result = get_str("API_KEY")
        assert result is None

    def test_get_str_empty_string(self, monkeypatch):
        """Test that get_str correctly handles empty string.
        测试 get_str 正确处理空字符串。"""
        monkeypatch.setenv("API_KEY", "")
        result = get_str("API_KEY")
        assert result == ""

    def test_get_str_with_whitespace(self, monkeypatch):
        """Test that get_str preserves whitespace in values.
        测试 get_str 保留值中的空白字符。"""
        monkeypatch.setenv("API_KEY", "  value with spaces  ")
        result = get_str("API_KEY")
        assert result == "  value with spaces  "

    def test_get_str_continuum_prefix_priority(self, monkeypatch):
        """Test that CONTINUUM_ prefix takes priority over base name.
        测试 CONTINUUM_ 前缀优先于基本名称。"""
        monkeypatch.setenv("API_KEY", "base-value")
        monkeypatch.setenv("CONTINUUM_API_KEY", "prefix-value")
        result = get_str("API_KEY")
        assert result == "prefix-value"

    def test_get_str_fallback_to_base_name(self, monkeypatch):
        """Test that get_str falls back to base name when prefix not set.
        测试前缀未设置时 get_str 回退到基本名称。"""
        monkeypatch.delenv("CONTINUUM_API_KEY", raising=False)
        monkeypatch.setenv("API_KEY", "base-value")
        result = get_str("API_KEY")
        assert result == "base-value"


class TestGetInt:
    """Tests for get_int function.
    get_int 函数测试。"""

    def test_get_int_returns_integer_value(self, monkeypatch):
        """Test that get_int correctly converts string to integer.
        测试 get_int 正确将字符串转换为整数。"""
        monkeypatch.setenv("TIMEOUT", "42")
        result = get_int("TIMEOUT")
        assert result == 42
        assert isinstance(result, int)

    def test_get_int_returns_default_when_not_set(self, monkeypatch):
        """Test that get_int returns default when variable is not set.
        测试变量未设置时 get_int 返回默认值。"""
        monkeypatch.delenv("TIMEOUT", raising=False)
        result = get_int("TIMEOUT", default=30)
        assert result == 30

    def test_get_int_returns_none_when_not_set_and_no_default(self, monkeypatch):
        """Test that get_int returns None when variable is not set and no default provided.
        测试变量未设置且无默认值时 get_int 返回 None。"""
        monkeypatch.delenv("TIMEOUT", raising=False)
        result = get_int("TIMEOUT")
        assert result is None

    def test_get_int_invalid_value_returns_default(self, caplog, monkeypatch):
        """Test that get_int returns default when value cannot be converted to int.
        测试值无法转换为整数时 get_int 返回默认值并记录警告。"""
        monkeypatch.setenv("TIMEOUT", "not-a-number")
        with caplog.at_level(logging.WARNING):
            result = get_int("TIMEOUT", default=30)
        assert result == 30
        assert "Cannot convert" in caplog.text

    def test_get_int_invalid_value_returns_none_without_default(self, caplog, monkeypatch):
        """Test that get_int returns None for invalid value without default.
        测试无效值且无默认值时 get_int 返回 None。"""
        monkeypatch.setenv("TIMEOUT", "invalid")
        with caplog.at_level(logging.WARNING):
            result = get_int("TIMEOUT")
        assert result is None
        assert "Cannot convert" in caplog.text

    def test_get_int_negative_value(self, monkeypatch):
        """Test that get_int handles negative integers.
        测试 get_int 处理负整数。"""
        monkeypatch.setenv("TIMEOUT", "-10")
        result = get_int("TIMEOUT")
        assert result == -10

    def test_get_int_zero_value(self, monkeypatch):
        """Test that get_int handles zero.
        测试 get_int 处理零值。"""
        monkeypatch.setenv("TIMEOUT", "0")
        result = get_int("TIMEOUT")
        assert result == 0

    def test_get_int_large_value(self, monkeypatch):
        """Test that get_int handles large integers.
        测试 get_int 处理大整数。"""
        monkeypatch.setenv("TIMEOUT", "999999999")
        result = get_int("TIMEOUT")
        assert result == 999999999

    def test_get_int_float_value_returns_default(self, caplog, monkeypatch):
        """Test that get_int returns default for float-like string.
        测试类浮点数字符串时 get_int 返回默认值。"""
        monkeypatch.setenv("TIMEOUT", "3.14")
        with caplog.at_level(logging.WARNING):
            result = get_int("TIMEOUT", default=30)
        assert result == 30
        assert "Cannot convert" in caplog.text

    def test_get_int_continuum_prefix_priority(self, monkeypatch):
        """Test that CONTINUUM_ prefix takes priority for integers.
        测试整数的 CONTINUUM_ 前缀优先级。"""
        monkeypatch.setenv("TIMEOUT", "10")
        monkeypatch.setenv("CONTINUUM_TIMEOUT", "20")
        result = get_int("TIMEOUT")
        assert result == 20


class TestGetBool:
    """Tests for get_bool function.
    get_bool 函数测试。"""

    @pytest.mark.parametrize("value,expected", [
        ("true", True),
        ("True", True),
        ("TRUE", True),
        ("1", True),
        ("yes", True),
        ("Yes", True),
        ("YES", True),
        ("on", True),
        ("On", True),
        ("ON", True),
    ])
    def test_get_bool_truthy_values(self, monkeypatch, value, expected):
        """Test that get_bool correctly identifies truthy values.
        测试 get_bool 正确识别真值。"""
        monkeypatch.setenv("DEBUG", value)
        result = get_bool("DEBUG")
        assert result is expected

    @pytest.mark.parametrize("value,expected", [
        ("false", False),
        ("False", False),
        ("FALSE", False),
        ("0", False),
        ("no", False),
        ("No", False),
        ("NO", False),
        ("off", False),
        ("Off", False),
        ("OFF", False),
    ])
    def test_get_bool_falsy_values(self, monkeypatch, value, expected):
        """Test that get_bool correctly identifies falsy values.
        测试 get_bool 正确识别假值。"""
        monkeypatch.setenv("DEBUG", value)
        result = get_bool("DEBUG")
        assert result is expected

    def test_get_bool_returns_default_when_not_set(self, monkeypatch):
        """Test that get_bool returns default when variable is not set.
        测试变量未设置时 get_bool 返回默认值。"""
        monkeypatch.delenv("DEBUG", raising=False)
        result = get_bool("DEBUG", default=True)
        assert result is True

    def test_get_bool_returns_none_when_not_set_and_no_default(self, monkeypatch):
        """Test that get_bool returns None when variable is not set and no default provided.
        测试变量未设置且无默认值时 get_bool 返回 None。"""
        monkeypatch.delenv("DEBUG", raising=False)
        result = get_bool("DEBUG")
        assert result is None

    def test_get_bool_unrecognized_value_returns_default(self, monkeypatch):
        """Test that get_bool returns default for unrecognized value.
        测试无法识别的值时 get_bool 返回默认值。"""
        monkeypatch.setenv("DEBUG", "maybe")
        result = get_bool("DEBUG", default=False)
        assert result is False

    def test_get_bool_unrecognized_value_returns_none_without_default(self, monkeypatch):
        """Test that get_bool returns None for unrecognized value without default.
        测试无法识别的值且无默认值时 get_bool 返回 None。"""
        monkeypatch.setenv("DEBUG", "invalid")
        result = get_bool("DEBUG")
        assert result is None

    def test_get_bool_with_whitespace(self, monkeypatch):
        """Test that get_bool handles values with surrounding whitespace.
        测试 get_bool 处理带有周围空白的值。"""
        monkeypatch.setenv("DEBUG", "  true  ")
        result = get_bool("DEBUG")
        assert result is True

    def test_get_bool_continuum_prefix_priority(self, monkeypatch):
        """Test that CONTINUUM_ prefix takes priority for booleans.
        测试布尔值的 CONTINUUM_ 前缀优先级。"""
        monkeypatch.setenv("DEBUG", "false")
        monkeypatch.setenv("CONTINUUM_DEBUG", "true")
        result = get_bool("DEBUG")
        assert result is True


class TestGetList:
    """Tests for get_list function.
    get_list 函数测试。"""

    def test_get_list_returns_list_value(self, monkeypatch):
        """Test that get_list correctly splits comma-separated values.
        测试 get_list 正确分割逗号分隔的值。"""
        monkeypatch.setenv("MODELS", "gpt-4,claude-3,gemini-pro")
        result = get_list("MODELS")
        assert result == ["gpt-4", "claude-3", "gemini-pro"]

    def test_get_list_trims_whitespace(self, monkeypatch):
        """Test that get_list trims whitespace around values.
        测试 get_list 去除值周围的空白。"""
        monkeypatch.setenv("MODELS", "  gpt-4 , claude-3  , gemini-pro  ")
        result = get_list("MODELS")
        assert result == ["gpt-4", "claude-3", "gemini-pro"]

    def test_get_list_returns_default_when_not_set(self, monkeypatch):
        """Test that get_list returns default when variable is not set.
        测试变量未设置时 get_list 返回默认值。"""
        monkeypatch.delenv("MODELS", raising=False)
        result = get_list("MODELS", default=["default-model"])
        assert result == ["default-model"]

    def test_get_list_returns_none_when_not_set_and_no_default(self, monkeypatch):
        """Test that get_list returns None when variable is not set and no default provided.
        测试变量未设置且无默认值时 get_list 返回 None。"""
        monkeypatch.delenv("MODELS", raising=False)
        result = get_list("MODELS")
        assert result is None

    def test_get_list_empty_string_returns_empty_list(self, monkeypatch):
        """Test that get_list returns empty list for empty string.
        测试空字符串时 get_list 返回空列表。"""
        monkeypatch.setenv("MODELS", "")
        result = get_list("MODELS", default=["default-model"])
        assert result == []

    def test_get_list_whitespace_only_returns_empty_list(self, monkeypatch):
        """Test that get_list returns empty list for whitespace-only string.
        测试仅含空白的字符串时 get_list 返回空列表。"""
        monkeypatch.setenv("MODELS", "   ")
        result = get_list("MODELS", default=["default-model"])
        assert result == []

    def test_get_list_single_value(self, monkeypatch):
        """Test that get_list handles single value correctly.
        测试 get_list 正确处理单个值。"""
        monkeypatch.setenv("MODELS", "gpt-4")
        result = get_list("MODELS")
        assert result == ["gpt-4"]

    def test_get_list_filters_empty_items(self, monkeypatch):
        """Test that get_list filters out empty items.
        测试 get_list 过滤空项。"""
        monkeypatch.setenv("MODELS", "gpt-4,,claude-3,")
        result = get_list("MODELS")
        assert result == ["gpt-4", "claude-3"]

    def test_get_list_with_special_characters(self, monkeypatch):
        """Test that get_list handles values with special characters.
        测试 get_list 处理含特殊字符的值。"""
        monkeypatch.setenv("ALLOWED_TOOLS", "read-file,write_file,execute.cmd")
        result = get_list("ALLOWED_TOOLS")
        assert result == ["read-file", "write_file", "execute.cmd"]

    def test_get_list_continuum_prefix_priority(self, monkeypatch):
        """Test that CONTINUUM_ prefix takes priority for lists.
        测试列表的 CONTINUUM_ 前缀优先级。"""
        monkeypatch.setenv("MODELS", "model-a,model-b")
        monkeypatch.setenv("CONTINUUM_MODELS", "model-x,model-y")
        result = get_list("MODELS")
        assert result == ["model-x", "model-y"]


class TestContinuumPrefixPriority:
    """Tests for CONTINUUM_ prefix priority behavior.
    CONTINUUM_ 前缀优先级行为测试。"""

    def test_prefix_takes_priority_over_base_name(self, monkeypatch):
        """Test that CONTINUUM_ prefix consistently takes priority over base name.
        测试 CONTINUUM_ 前缀始终优先于基本名称。"""
        # Set both prefixed and non-prefixed versions
        monkeypatch.setenv("API_KEY", "base-api-key")
        monkeypatch.setenv("CONTINUUM_API_KEY", "prefix-api-key")

        monkeypatch.setenv("BASE_URL", "https://base.example.com")
        monkeypatch.setenv("CONTINUUM_BASE_URL", "https://prefix.example.com")

        monkeypatch.setenv("TIMEOUT", "10")
        monkeypatch.setenv("CONTINUUM_TIMEOUT", "20")

        monkeypatch.setenv("DEBUG", "false")
        monkeypatch.setenv("CONTINUUM_DEBUG", "true")

        monkeypatch.setenv("MODELS", "model-a")
        monkeypatch.setenv("CONTINUUM_MODELS", "model-b")

        # Verify prefix takes priority for all types
        assert get_str("API_KEY") == "prefix-api-key"
        assert get_str("BASE_URL") == "https://prefix.example.com"
        assert get_int("TIMEOUT") == 20
        assert get_bool("DEBUG") is True
        assert get_list("MODELS") == ["model-b"]

    def test_fallback_to_base_name_when_prefix_not_set(self, monkeypatch):
        """Test fallback to base name when CONTINUUM_ prefix is not set.
        测试 CONTINUUM_ 前缀未设置时回退到基本名称。"""
        # Only set non-prefixed versions
        monkeypatch.delenv("CONTINUUM_API_KEY", raising=False)
        monkeypatch.setenv("API_KEY", "base-api-key")

        monkeypatch.delenv("CONTINUUM_DEBUG", raising=False)
        monkeypatch.setenv("DEBUG", "true")

        # Verify fallback works
        assert get_str("API_KEY") == "base-api-key"
        assert get_bool("DEBUG") is True


class TestDocumentedVars:
    """Tests for documented variables whitelist.
    文档化变量白名单测试。"""

    def test_documented_names_are_accessible(self, monkeypatch):
        """Test that all documented names can be accessed.
        测试所有文档化名称都可访问。"""
        # Test a few representative names from the whitelist
        test_names = ["API_KEY", "BASE_URL", "TIMEOUT", "DEBUG", "MODELS"]
        for name in test_names:
            monkeypatch.setenv(name, "test-value")
            # Should not raise
            result = get_str(name)
            assert result == "test-value"

    def test_documented_vars_is_frozenset(self):
        """Test that DOCUMENTED_ENV_VARS is a frozenset for immutability.
        测试 DOCUMENTED_ENV_VARS 是 frozenset 以保证不可变性。"""
        assert isinstance(DOCUMENTED_ENV_VARS, frozenset)

    def test_documented_vars_not_empty(self):
        """Test that whitelist is not empty.
        测试白名单不为空。"""
        assert len(DOCUMENTED_ENV_VARS) > 0

    def test_documented_vars_contains_expected_variables(self):
        """Test that whitelist contains all expected variables.
        测试白名单包含所有预期的变量。"""
        expected_vars = {
            # Core configuration
            "API_KEY",
            "BASE_URL",
            "PROVIDER",
            "MODEL",
            "SMALL_MODEL",
            "DEFAULT_MODEL",
            "API_FORMAT",
            # Runtime settings
            "LOG_LEVEL",
            "MAX_TOKENS",
            "TIMEOUT",
            "MAX_ITERATIONS",
            "TEMPERATURE",
            "EFFORT_LEVEL",
            # Feature flags
            "DEBUG",
            "VERBOSE",
            "DISABLE_TRAFFIC",
            "AUDIT_ENABLED",
            # Paths
            "WORKTREES_DIR",
            "PLUGINS_DIR",
            "AUDIT_LOG_PATH",
            "THEME_CONFIG",
            # List-type
            "MODELS",
            "ALLOWED_TOOLS",
            "BLOCKED_TOOLS",
            "EXTRA_HEADERS",
            # Retention
            "AUDIT_RETENTION",
            # Test support
            "USE_REAL_API",
        }
        assert expected_vars.issubset(DOCUMENTED_ENV_VARS)


class TestConstants:
    """Tests for module constants.
    模块常量测试。"""

    def test_env_prefix_value(self):
        """Test that ENV_PREFIX has the correct value.
        测试 ENV_PREFIX 具有正确的值。"""
        assert ENV_PREFIX == "CONTINUUM_"


class TestDefensiveCodePaths:
    """Tests for defensive code paths that require mocking.
    测试需要 mock 的防御性代码路径。"""

    def test_get_int_defensive_none_after_resolve(self, monkeypatch):
        """Test get_int handles None from os.environ.get after successful resolve.
        测试 get_int 在解析成功后处理 os.environ.get 返回 None 的情况。"""
        # Set the environment variable
        monkeypatch.setenv("TIMEOUT", "42")

        # Mock os.environ.get to return None even though _resolve_name found the key
        original_get = os.environ.get

        def mock_get(key, default=None):
            if key in ("TIMEOUT", "CONTINUUM_TIMEOUT"):
                return None  # Simulate the key disappeared between checks
            return original_get(key, default)

        monkeypatch.setattr(os.environ, "get", mock_get)

        # Should return default due to defensive check
        result = get_int("TIMEOUT", default=99)
        assert result == 99

    def test_get_bool_defensive_none_after_resolve(self, monkeypatch):
        """Test get_bool handles None from os.environ.get after successful resolve.
        测试 get_bool 在解析成功后处理 os.environ.get 返回 None 的情况。"""
        # Set the environment variable
        monkeypatch.setenv("DEBUG", "true")

        # Mock os.environ.get to return None even though _resolve_name found the key
        original_get = os.environ.get

        def mock_get(key, default=None):
            if key in ("DEBUG", "CONTINUUM_DEBUG"):
                return None
            return original_get(key, default)

        monkeypatch.setattr(os.environ, "get", mock_get)

        # Should return default due to defensive check
        result = get_bool("DEBUG", default=False)
        assert result is False

    def test_get_list_defensive_none_after_resolve(self, monkeypatch):
        """Test get_list handles None from os.environ.get after successful resolve.
        测试 get_list 在解析成功后处理 os.environ.get 返回 None 的情况。"""
        # Set the environment variable
        monkeypatch.setenv("MODELS", "gpt-4")

        # Mock os.environ.get to return None even though _resolve_name found the key
        original_get = os.environ.get

        def mock_get(key, default=None):
            if key in ("MODELS", "CONTINUUM_MODELS"):
                return None
            return original_get(key, default)

        monkeypatch.setattr(os.environ, "get", mock_get)

        # Should return default due to defensive check
        result = get_list("MODELS", default=["default"])
        assert result == ["default"]