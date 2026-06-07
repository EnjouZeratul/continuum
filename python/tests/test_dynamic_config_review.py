"""
全量审查动态配置实现

审查范围:
1. 环境变量优先级 (CONTINUUM_MODEL 等)
2. Fallback 机制
3. env.py 安全访问
4. 类型转换和错误处理
"""

import os
import sys
import warnings
from pathlib import Path

import pytest


class TestEnvVariablePriority:
    """测试环境变量优先级"""

    def test_continuum_model_priority(self, monkeypatch):
        """验证 CONTINUUM_MODEL 环境变量优先"""
        from continuum_sdk.config.providers import get_default_model

        # 设置 CONTINUUM_MODEL
        monkeypatch.setenv("CONTINUUM_MODEL", "test-model-from-env")

        # 无论 provider 是什么，都应该返回环境变量的值
        result = get_default_model("anthropic")
        assert result == "test-model-from-env", \
            f"Expected CONTINUUM_MODEL to take priority, got: {result}"

        result = get_default_model("openai")
        assert result == "test-model-from-env", \
            f"Expected CONTINUUM_MODEL to take priority, got: {result}"

        print("[PASS] CONTINUUM_MODEL 环境变量优先级正确")

    def test_continuum_model_override_provider_config(self, monkeypatch):
        """验证 CONTINUUM_MODEL 覆盖 provider 内置配置"""
        from continuum_sdk.config.providers import get_default_model, BUILTIN_PROVIDERS

        # 不设置 CONTINUUM_MODEL 时，应返回 provider 内置配置
        anthropic_default = BUILTIN_PROVIDERS["anthropic"].default_model

        # 清理 CONTINUUM_MODEL
        monkeypatch.delenv("CONTINUUM_MODEL", raising=False)

        result = get_default_model("anthropic")
        assert result == anthropic_default, \
            f"Expected provider default model {anthropic_default}, got: {result}"

        # 设置 CONTINUUM_MODEL 后，应覆盖
        monkeypatch.setenv("CONTINUUM_MODEL", "custom-model")
        result = get_default_model("anthropic")
        assert result == "custom-model", \
            f"Expected custom-model from env, got: {result}"

        print("[PASS] CONTINUUM_MODEL 正确覆盖 provider 内置配置")

    def test_max_iterations_env_variable(self, monkeypatch):
        """验证 CONTINUUM_MAX_ITERATIONS 正确读取"""
        from continuum_sdk.env import get_int

        # 不设置时返回默认值
        monkeypatch.delenv("CONTINUUM_MAX_ITERATIONS", raising=False)
        monkeypatch.delenv("MAX_ITERATIONS", raising=False)

        result = get_int("MAX_ITERATIONS", default=10)
        assert result == 10, f"Expected default 10, got: {result}"

        # 设置 CONTINUUM_MAX_ITERATIONS
        monkeypatch.setenv("CONTINUUM_MAX_ITERATIONS", "50")
        result = get_int("MAX_ITERATIONS", default=10)
        assert result == 50, f"Expected 50 from env, got: {result}"

        print("[PASS] CONTINUUM_MAX_ITERATIONS 正确读取")

    def test_provider_specific_env_fallback(self, monkeypatch):
        """验证 provider-specific 环境变量回退"""
        from continuum_sdk.config.loader import Config

        # 设置 provider
        monkeypatch.setenv("CONTINUUM_PROVIDER", "openai")

        # 不设置 CONTINUUM_API_KEY，设置 OPENAI_API_KEY
        monkeypatch.delenv("CONTINUUM_API_KEY", raising=False)

        monkeypatch.setenv("OPENAI_API_KEY", "test-openai-key")

        config = Config.from_env()
        assert config.api_key == "test-openai-key", \
            f"Expected OPENAI_API_KEY fallback, got: {config.api_key}"

        print("[PASS] provider-specific 环境变量回退正确")


class TestFallbackMechanism:
    """测试 Fallback 机制"""

    def test_fallback_provider_order(self, monkeypatch):
        """验证 FALLBACK_PROVIDER_ORDER 顺序"""
        from continuum_sdk.config.providers import (
            FALLBACK_PROVIDER_ORDER,
            get_default_model,
            BUILTIN_PROVIDERS,
        )

        # 清理 CONTINUUM_MODEL
        monkeypatch.delenv("CONTINUUM_MODEL", raising=False)

        # 验证 fallback 顺序
        expected_order = ["anthropic", "openai", "google", "deepseek", "qwen"]
        assert FALLBACK_PROVIDER_ORDER == expected_order, \
            f"FALLBACK_PROVIDER_ORDER should be {expected_order}, got: {FALLBACK_PROVIDER_ORDER}"

        print("[PASS] FALLBACK_PROVIDER_ORDER 顺序正确")

    def test_fallback_logs_friendly(self, monkeypatch):
        """验证 fallback 日志友好"""
        from continuum_sdk.config.providers import get_default_model
        import logging

        # 清理 CONTINUUM_MODEL
        monkeypatch.delenv("CONTINUUM_MODEL", raising=False)

        # 使用一个不存在的 provider 来触发 fallback
        with self._capture_logs("continuum_sdk.config.providers") as logs:
            try:
                result = get_default_model("unknown_provider_xyz")
                # 应该返回 fallback provider 的默认模型
                assert result is not None, "Should return a fallback model"
                # 检查日志是否包含友好的信息
                log_messages = [r.getMessage() for r in logs]
                has_fallback_log = any("Fallback" in msg for msg in log_messages)
                assert has_fallback_log, f"Should log fallback info, got: {log_messages}"
            except RuntimeError as e:
                # 如果所有 provider 都不可用，应该抛出包含配置指引的异常
                assert "CONTINUUM_MODEL" in str(e), \
                    f"RuntimeError should contain config guidance, got: {e}"

        print("[PASS] fallback 日志友好")

    def test_runtime_error_contains_guidance(self, monkeypatch):
        """验证最终 RuntimeError 包含配置指引"""
        from continuum_sdk.config.providers import get_default_model, BUILTIN_PROVIDERS

        # 临时清空 BUILTIN_PROVIDERS 来测试边界情况
        # (注意：这是破坏性测试，实际使用中不会发生)
        original_providers = BUILTIN_PROVIDERS.copy()
        BUILTIN_PROVIDERS.clear()

        try:
            with pytest.raises(RuntimeError) as exc_info:
                get_default_model("anthropic")

            error_msg = str(exc_info.value)
            assert "CONTINUUM_MODEL" in error_msg, \
                f"Error should mention CONTINUUM_MODEL, got: {error_msg}"
            assert "configure" in error_msg.lower(), \
                f"Error should mention configuration, got: {error_msg}"

            print("[PASS] RuntimeError 包含配置指引")
        finally:
            # 恢复 BUILTIN_PROVIDERS
            BUILTIN_PROVIDERS.update(original_providers)

    def test_fallback_to_first_available_provider(self, monkeypatch):
        """验证回退到第一个可用的 provider"""
        from continuum_sdk.config.providers import get_default_model, BUILTIN_PROVIDERS

        monkeypatch.delenv("CONTINUUM_MODEL", raising=False)

        # 传入一个不存在的 provider
        result = get_default_model("nonexistent_provider")

        # 应该返回 fallback 顺序中第一个 provider 的默认模型
        first_fallback_provider = BUILTIN_PROVIDERS.get("anthropic")
        assert first_fallback_provider is not None
        expected = first_fallback_provider.default_model

        assert result == expected, \
            f"Expected fallback to anthropic default model {expected}, got: {result}"

        print("[PASS] 正确回退到第一个可用的 provider")

    class _capture_logs:
        """捕获日志的上下文管理器"""
        def __init__(self, logger_name):
            self.logger_name = logger_name
            self.records = []

        def __enter__(self):
            import logging
            self.logger = logging.getLogger(self.logger_name)
            self.handler = logging.Handler()
            self.handler.emit = lambda record: self.records.append(record)
            self.logger.addHandler(self.handler)
            self.logger.setLevel(logging.DEBUG)
            return self.records

        def __exit__(self, *args):
            self.logger.removeHandler(self.handler)


class TestEnvSafety:
    """测试 env.py 安全访问"""

    def test_whitelist_enforced(self):
        """验证白名单强制执行"""
        from continuum_sdk.env import get_str, ALLOWED_ENV_BASE_NAMES

        # 尝试访问不在白名单中的变量应该抛出 ValueError
        with pytest.raises(ValueError) as exc_info:
            get_str("NOT_IN_WHITELIST_VAR")

        assert "not in the allowed list" in str(exc_info.value)
        print("[PASS] 白名单强制执行正确")

    def test_whitelist_reasonable(self):
        """验证白名单合理性 - 包含核心配置"""
        from continuum_sdk.env import ALLOWED_ENV_BASE_NAMES

        # 核心配置应该在白名单中
        essential_vars = {
            "API_KEY",
            "BASE_URL",
            "PROVIDER",
            "MODEL",
            "MAX_ITERATIONS",
            "LOG_LEVEL",
            "DEBUG",
        }

        for var in essential_vars:
            assert var in ALLOWED_ENV_BASE_NAMES, \
                f"{var} should be in whitelist"

        print("[PASS] 白名单包含核心配置")

    def test_type_conversion_safe(self, monkeypatch):
        """验证类型转换安全"""
        from continuum_sdk.env import get_int, get_bool, get_list

        # 设置无效的整数值
        monkeypatch.setenv("CONTINUUM_MAX_TOKENS", "not_a_number")

        # 应该返回默认值，不抛出异常
        result = get_int("MAX_TOKENS", default=4096)
        assert result == 4096, f"Should return default on invalid int, got: {result}"

        # 设置有效的布尔值
        monkeypatch.setenv("CONTINUUM_DEBUG", "yes")
        result = get_bool("DEBUG", default=False)
        assert result is True, f"Should parse 'yes' as True, got: {result}"

        # 设置无效的布尔值
        monkeypatch.setenv("CONTINUUM_VERBOSE", "maybe")
        result = get_bool("VERBOSE", default=False)
        assert result is False, f"Should return default on invalid bool, got: {result}"

        print("[PASS] 类型转换安全")

    def test_list_parsing(self, monkeypatch):
        """验证列表解析"""
        from continuum_sdk.env import get_list

        # 设置列表值
        monkeypatch.setenv("CONTINUUM_MODELS", "gpt-4, claude-3, gemini-pro")
        result = get_list("MODELS")
        assert result == ["gpt-4", "claude-3", "gemini-pro"], \
            f"Should parse comma-separated list, got: {result}"

        # 空字符串
        monkeypatch.setenv("CONTINUUM_MODELS", "")
        result = get_list("MODELS", default=["default"])
        assert result == [], f"Empty string should return empty list, got: {result}"

        print("[PASS] 列表解析正确")

    def test_continuum_prefix_handling(self, monkeypatch):
        """验证 CONTINUUM_ 前缀正确处理"""
        from continuum_sdk.env import get_str, ENV_PREFIX

        # 验证前缀
        assert ENV_PREFIX == "CONTINUUM_", f"Expected CONTINUUM_ prefix, got: {ENV_PREFIX}"

        # 设置带前缀的变量
        monkeypatch.setenv("CONTINUUM_API_KEY", "test_key_1")
        monkeypatch.setenv("API_KEY", "test_key_2")

        # 应该优先返回带前缀的
        result = get_str("API_KEY")
        assert result == "test_key_1", \
            f"Should prioritize CONTINUUM_ prefix, got: {result}"

        print("[PASS] CONTINUUM_ 前缀正确处理")


class TestConfigLoaderSecurity:
    """测试 Config loader 安全性"""

    def test_env_var_whitelist_in_loader(self):
        """验证 loader 中的环境变量白名单"""
        from continuum_sdk.config.loader import ALLOWED_ENV_VARS, _get_env

        # 尝试访问不在白名单中的变量
        # 新行为：静默返回 None，不发出警告
        result = _get_env("NOT_ALLOWED_VARIABLE_XYZ")
        assert result is None, "Should return None for non-whitelisted var"

        # 验证白名单变量可以正常访问
        result = _get_env("CONTINUUM_API_KEY")
        # 即使变量不存在，也应该返回默认值而不是抛出异常
        assert result is None, "Should return None for unset whitelisted var"

        print("[PASS] loader 白名单正确（静默拒绝非白名单变量）")

    def test_env_expansion_security(self):
        """验证环境变量展开安全"""
        from continuum_sdk.config.loader import Config

        # 创建一个包含环境变量引用的字典
        data = {"api_key": "${ANTHROPIC_API_KEY}"}

        # 展开时应该通过白名单检查
        # 如果 ANTHROPIC_API_KEY 在白名单中，应该被展开
        # 否则保持原样
        expanded = Config._expand_env_vars(data)

        # ANTHROPIC_API_KEY 在白名单中
        # 如果环境变量存在，应该展开
        if "ANTHROPIC_API_KEY" in os.environ:
            expected = os.environ["ANTHROPIC_API_KEY"]
            assert expanded["api_key"] == expected, \
                f"Should expand ANTHROPIC_API_KEY, got: {expanded['api_key']}"

        print("[PASS] 环境变量展开安全")


class TestIntegrationScenarios:
    """集成测试场景"""

    def test_full_config_flow(self, monkeypatch):
        """测试完整配置流程"""
        from continuum_sdk.config.loader import Config

        # 设置环境变量
        monkeypatch.setenv("CONTINUUM_PROVIDER", "openai")
        monkeypatch.setenv("CONTINUUM_MODEL", "gpt-4-test")
        monkeypatch.setenv("OPENAI_API_KEY", "test-key")

        # 从环境变量加载配置
        config = Config.from_env()

        assert config.provider == "openai"
        assert config.model == "gpt-4-test"
        assert config.api_key == "test-key"

        print("[PASS] 完整配置流程正确")

    def test_config_model_property_with_fallback(self, monkeypatch):
        """测试 Config.model 属性的 fallback 行为"""
        from continuum_sdk.config.loader import Config

        # 不设置任何模型
        monkeypatch.delenv("CONTINUUM_MODEL", raising=False)

        # 创建配置，不指定 model
        config = Config(provider="openai", api_key="test")

        # model 属性应该返回 provider 的默认模型
        model = config.model
        assert model is not None, "Should return default model"
        # 应该是 openai 的默认模型或 fallback 模型
        assert model in ["gpt-5.5", "claude-sonnet-4-6"] or model.startswith("gpt"), \
            f"Expected openai default model, got: {model}"

        print("[PASS] Config.model 属性 fallback 正确")

    def test_multiple_provider_configs(self):
        """测试多 provider 配置"""
        from continuum_sdk.config.loader import Config

        config = Config(provider="anthropic", api_key="key1")

        # 添加其他 provider 配置
        config.add_provider("openai", api_key="key2", model="gpt-4")
        config.add_provider("google", api_key="key3", model="gemini-pro")

        # 切换 provider
        config.use("openai")

        assert config.provider == "openai"
        # 注意：use() 不会自动设置 api_key，除非在 _providers 中配置了

        print("[PASS] 多 provider 配置正确")


def run_all_tests():
    """运行所有测试"""
    print("\n" + "="*60)
    print("全量审查动态配置实现")
    print("="*60 + "\n")

    # 运行 pytest
    exit_code = pytest.main([
        __file__,
        "-v",
        "--tb=short",
        "-x",  # 首次失败即停止
    ])

    return exit_code


if __name__ == "__main__":
    sys.exit(run_all_tests())
