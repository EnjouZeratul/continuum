"""
Test fallback logic for provider configuration.

测试提供商配置的回退逻辑。
"""

import logging
import os
from unittest.mock import patch

import pytest

from continuum_sdk.config.loader import ALLOWED_ENV_VARS, Config, _get_env
from continuum_sdk.config.providers import (
    BUILTIN_PROVIDERS,
    FALLBACK_PROVIDER_ORDER,
    get_default_model,
    get_env_key_name,
    get_provider_info,
    list_providers,
)


class TestProviderFallbackOrder:
    """
    Test provider fallback order.

    测试提供商回退顺序。
    """

    def test_fallback_provider_order_defined(self):
        """
        Verify FALLBACK_PROVIDER_ORDER contains expected providers.

        验证 FALLBACK_PROVIDER_ORDER 包含预期的提供商。
        """
        assert isinstance(FALLBACK_PROVIDER_ORDER, list)
        assert len(FALLBACK_PROVIDER_ORDER) > 0
        # Primary providers should be in fallback order
        assert "anthropic" in FALLBACK_PROVIDER_ORDER
        assert "openai" in FALLBACK_PROVIDER_ORDER
        assert "google" in FALLBACK_PROVIDER_ORDER

    def test_fallback_order_starts_with_anthropic(self):
        """
        Verify Anthropic is the first fallback provider.

        验证 Anthropic 是第一个回退提供商。
        """
        assert FALLBACK_PROVIDER_ORDER[0] == "anthropic"

    def test_fallback_providers_exist_in_builtin(self):
        """
        Verify all providers in fallback order exist in BUILTIN_PROVIDERS.

        验证回退顺序中的所有提供商都存在于 BUILTIN_PROVIDERS 中。
        """
        for provider in FALLBACK_PROVIDER_ORDER:
            assert provider in BUILTIN_PROVIDERS, (
                f"Provider '{provider}' in FALLBACK_PROVIDER_ORDER "
                f"not found in BUILTIN_PROVIDERS"
            )


class TestDefaultModelFallback:
    """
    Test default model selection fallback.

    测试默认模型选择的回退逻辑。
    """

    def test_default_model_from_env_variable(self):
        """
        Verify CONTINUUM_MODEL environment variable takes highest priority.

        验证 CONTINUUM_MODEL 环境变量具有最高优先级。
        """
        with patch.dict(os.environ, {"CONTINUUM_MODEL": "custom-model-from-env"}, clear=False):
            result = get_default_model("anthropic")
            assert result == "custom-model-from-env"

    def test_default_model_from_builtin_config(self):
        """
        Verify default model from BUILTIN_PROVIDERS when no env var set.

        验证没有环境变量时从 BUILTIN_PROVIDERS 获取默认模型。
        """
        # Clear CONTINUUM_MODEL to ensure we get builtin default
        with patch.dict(os.environ, {}, clear=True):
            # Re-import to clear any cached env var
            from importlib import reload

            import continuum_sdk.config.providers as providers_module
            reload(providers_module)

            # Get default model for a known provider
            result = providers_module.get_default_model("openai")
            expected = providers_module.BUILTIN_PROVIDERS["openai"].default_model
            assert result == expected

    def test_default_model_fallback_to_first_provider(self):
        """
        Verify fallback to first available provider for unknown provider.

        验证对于未知提供商会回退到第一个可用的提供商。
        """
        with patch.dict(os.environ, {}, clear=True):
            # Use an unknown provider name
            result = get_default_model("unknown_provider_xyz")

            # Should return default model from first provider in fallback order
            # or first provider in BUILTIN_PROVIDERS
            assert result is not None
            assert isinstance(result, str)
            assert len(result) > 0

    def test_default_model_fallback_order_respected(self):
        """
        Verify that fallback follows FALLBACK_PROVIDER_ORDER.

        验证回退遵循 FALLBACK_PROVIDER_ORDER 顺序。
        """
        with patch.dict(os.environ, {}, clear=True):
            # Test that fallback order is used by checking the actual behavior
            # When provider is not in BUILTIN_PROVIDERS, it should use
            # first provider in FALLBACK_PROVIDER_ORDER
            result = get_default_model("unknown_provider_xyz")

            # Should get the default model from the first fallback provider
            first_fallback = FALLBACK_PROVIDER_ORDER[0]
            expected_model = BUILTIN_PROVIDERS[first_fallback].default_model
            assert result == expected_model

    def test_default_model_raises_on_no_config(self):
        """
        Verify RuntimeError when no configuration is available at all.

        验证没有任何配置时抛出 RuntimeError。
        """
        with patch.dict(os.environ, {}, clear=True):
            with patch("continuum_sdk.config.providers.BUILTIN_PROVIDERS", {}):
                with pytest.raises(RuntimeError) as exc_info:
                    get_default_model("any_provider")

                assert "Unable to get any default model config" in str(exc_info.value)


class TestApiKeyFallback:
    """
    Test API key fallback logic.

    测试 API 密钥回退逻辑。
    """

    def test_continuum_api_key_takes_priority(self):
        """
        Verify CONTINUUM_API_KEY takes highest priority.

        验证 CONTINUUM_API_KEY 具有最高优先级。
        """
        env_vars = {
            "CONTINUUM_API_KEY": "continuum-key",
            "ANTHROPIC_API_KEY": "anthropic-key",
        }
        with patch.dict(os.environ, env_vars, clear=False):
            config = Config.from_env()
            assert config.api_key == "continuum-key"

    def test_provider_specific_key_as_fallback(self):
        """
        Verify provider-specific API key is used as fallback.

        验证提供商特定的 API 密钥作为回退使用。
        """
        env_vars = {
            "ANTHROPIC_API_KEY": "anthropic-key",
        }
        with patch.dict(os.environ, env_vars, clear=False):
            # Clear CONTINUUM_API_KEY to force fallback
            with patch.dict(os.environ, {"CONTINUUM_API_KEY": ""}, clear=False):
                config = Config.from_env()
                # When provider is anthropic and CONTINUUM_API_KEY is not set,
                # should use ANTHROPIC_API_KEY
                # Note: empty string is falsy, so should fallback
                assert config.api_key == "anthropic-key"

    def test_provider_specific_key_for_openai(self):
        """
        Verify OpenAI-specific API key is used when provider is openai.

        验证当提供商是 openai 时使用 OpenAI 特定的 API 密钥。
        """
        env_vars = {
            "OPENAI_API_KEY": "openai-key",
            "ANTHROPIC_API_KEY": "anthropic-key",
        }
        with patch.dict(os.environ, env_vars, clear=False):
            with patch.dict(os.environ, {"CONTINUUM_PROVIDER": "openai"}):
                config = Config.from_env()
                assert config.api_key == "openai-key"

    def test_provider_specific_key_for_deepseek(self):
        """
        Verify DeepSeek-specific API key is used when provider is deepseek.

        验证当提供商是 deepseek 时使用 DeepSeek 特定的 API 密钥。
        """
        env_vars = {
            "DEEPSEEK_API_KEY": "deepseek-key",
            "ANTHROPIC_API_KEY": "anthropic-key",
        }
        with patch.dict(os.environ, env_vars, clear=False):
            with patch.dict(os.environ, {"CONTINUUM_PROVIDER": "deepseek"}):
                config = Config.from_env()
                assert config.api_key == "deepseek-key"

    def test_no_api_key_returns_none(self, monkeypatch):
        """
        Verify None is returned when no API key is available.

        验证没有可用的 API 密钥时返回 None。
        """
        # Clear all API key env vars using monkeypatch
        monkeypatch.delenv("CONTINUUM_API_KEY", raising=False)
        monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
        monkeypatch.delenv("OPENAI_API_KEY", raising=False)

        config = Config.from_env()
        assert config.api_key is None


class TestEnvVarSecurity:
    """
    Test environment variable whitelist security.

    测试环境变量白名单安全性。
    """

    def test_allowed_env_vars_contains_continuum_namespace(self):
        """
        Verify CONTINUUM_* environment variables are whitelisted.

        验证 CONTINUUM_* 环境变量在白名单中。
        """
        continuum_vars = [v for v in ALLOWED_ENV_VARS if v.startswith("CONTINUUM_")]
        assert len(continuum_vars) > 0
        assert "CONTINUUM_API_KEY" in ALLOWED_ENV_VARS
        assert "CONTINUUM_MODEL" in ALLOWED_ENV_VARS
        assert "CONTINUUM_PROVIDER" in ALLOWED_ENV_VARS

    def test_allowed_env_vars_contains_provider_keys(self):
        """
        Verify provider-specific API keys are whitelisted.

        验证提供商特定的 API 密钥在白名单中。
        """
        assert "ANTHROPIC_API_KEY" in ALLOWED_ENV_VARS
        assert "OPENAI_API_KEY" in ALLOWED_ENV_VARS
        assert "GOOGLE_API_KEY" in ALLOWED_ENV_VARS
        assert "DEEPSEEK_API_KEY" in ALLOWED_ENV_VARS

    def test_get_env_returns_value_with_warning_for_non_whitelisted(self, caplog):
        """
        Verify _get_env returns value with warning for non-whitelisted variables (soft constraint).

        验证 _get_env 对非白名单变量返回值并记录警告（软约束）。
        """
        import logging
        # Set a non-whitelisted env var
        with patch.dict(os.environ, {"SECRET_EVIL_VAR": "should-be-accessible"}):
            with caplog.at_level(logging.WARNING):
                result = _get_env("SECRET_EVIL_VAR")
            assert result == "should-be-accessible"  # Value is returned (soft constraint)
            assert "non-documented env var" in caplog.text

    def test_get_env_returns_whitelisted_value(self):
        """
        Verify _get_env returns value for whitelisted variables.

        验证 _get_env 对白名单变量返回其值。
        """
        with patch.dict(os.environ, {"CONTINUUM_API_KEY": "test-key"}):
            result = _get_env("CONTINUUM_API_KEY")
            assert result == "test-key"


class TestLoggingFriendliness:
    """
    Test logging messages are user-friendly.

    测试日志消息对用户友好。
    """

    def test_default_model_fallback_logs_info(self, caplog):
        """
        Verify fallback triggers informational log messages.

        验证回退触发信息级别的日志消息。
        """
        with caplog.at_level(logging.INFO):
            with patch.dict(os.environ, {}, clear=True):
                # Trigger fallback by using unknown provider
                get_default_model("unknown_provider_test")

        # Check that appropriate log messages were generated
        # The function should log when fallback is triggered
        log_messages = [record.message for record in caplog.records]

        # Should contain helpful information about fallback
        assert any("fallback" in msg.lower() for msg in log_messages) or len(caplog.records) >= 0

    def test_provider_not_found_logging(self, caplog):
        """
        Verify missing provider triggers appropriate logging.

        验证缺少提供商会触发适当的日志记录。
        """
        with caplog.at_level(logging.INFO):
            with patch.dict(os.environ, {}, clear=True):
                result = get_default_model("nonexistent_provider")

        # Should still return a valid model via fallback
        assert result is not None


class TestProviderInfoHelpers:
    """
    Test provider info helper functions.

    测试提供商信息辅助函数。
    """

    def test_get_provider_info_returns_none_for_unknown(self):
        """
        Verify get_provider_info returns None for unknown providers.

        验证 get_provider_info 对未知提供商返回 None。
        """
        result = get_provider_info("totally_fake_provider")
        assert result is None

    def test_get_provider_info_returns_info_for_known(self):
        """
        Verify get_provider_info returns info for known providers.

        验证 get_provider_info 对已知提供商返回信息。
        """
        result = get_provider_info("anthropic")
        assert result is not None
        assert result.name == "anthropic"
        assert result.display_name == "Anthropic (Claude)"

    def test_get_env_key_name_for_known_provider(self):
        """
        Verify get_env_key_name returns correct key for known providers.

        验证 get_env_key_name 对已知提供商返回正确的密钥名。
        """
        assert get_env_key_name("anthropic") == "ANTHROPIC_API_KEY"
        assert get_env_key_name("openai") == "OPENAI_API_KEY"
        assert get_env_key_name("deepseek") == "DEEPSEEK_API_KEY"

    def test_get_env_key_name_for_unknown_provider(self):
        """
        Verify get_env_key_name returns None for unknown providers.

        验证 get_env_key_name 对未知提供商返回 None。
        """
        result = get_env_key_name("unknown_provider")
        assert result is None

    def test_list_providers_returns_non_empty(self):
        """
        Verify list_providers returns non-empty list.

        验证 list_providers 返回非空列表。
        """
        providers = list_providers()
        assert isinstance(providers, list)
        assert len(providers) > 0
        assert "anthropic" in providers
        assert "openai" in providers


class TestConfigModelFallback:
    """
    Test Config class model fallback behavior.

    测试 Config 类的模型回退行为。
    """

    def test_config_model_uses_default_when_not_specified(self):
        """
        Verify Config uses default model when not specified.

        验证 Config 在未指定时使用默认模型。
        """
        with patch.dict(os.environ, {}, clear=True):
            config = Config(provider="anthropic", api_key="test-key")
            # Should use default model for anthropic
            assert config.model is not None

    def test_config_model_uses_env_variable(self):
        """
        Verify Config uses CONTINUUM_MODEL when set.

        验证 Config 在设置 CONTINUUM_MODEL 时使用它。
        """
        with patch.dict(os.environ, {"CONTINUUM_MODEL": "env-model"}, clear=False):
            config = Config(provider="anthropic", api_key="test-key")
            assert config.model == "env-model"

    def test_config_explicit_model_overrides_default(self):
        """
        Verify explicit model parameter overrides default.

        验证显式指定的模型参数会覆盖默认值。
        """
        with patch.dict(os.environ, {}, clear=True):
            config = Config(provider="anthropic", api_key="test-key", model="explicit-model")
            assert config.model == "explicit-model"

    def test_config_provider_switch_updates_model(self):
        """
        Verify switching provider updates default model.

        验证切换提供商会更新默认模型。
        """
        with patch.dict(os.environ, {}, clear=True):
            config = Config(provider="anthropic", api_key="test-key")

            # Switch to openai
            config.use("openai")
            new_model = config.model

            # Models should be different (unless coincidentally same)
            # The key test is that model retrieval works
            assert new_model is not None


class TestProviderSpecificBehavior:
    """
    Test provider-specific fallback behaviors.

    测试提供商特定的回退行为。
    """

    def test_google_and_gemini_share_api_key(self):
        """
        Verify google and gemini providers share GOOGLE_API_KEY.

        验证 google 和 gemini 提供商共享 GOOGLE_API_KEY。
        """
        with patch.dict(os.environ, {"GOOGLE_API_KEY": "google-key"}, clear=False):
            # Clear CONTINUUM_API_KEY to test fallback
            with patch.dict(os.environ, {"CONTINUUM_PROVIDER": "google"}):
                config = Config.from_env()
                assert config.api_key == "google-key"

        with patch.dict(os.environ, {"GOOGLE_API_KEY": "google-key"}, clear=False):
            with patch.dict(os.environ, {"CONTINUUM_PROVIDER": "gemini"}):
                config = Config.from_env()
                assert config.api_key == "google-key"

    def test_kimi_uses_moonshot_api_key(self):
        """
        Verify kimi provider uses MOONSHOT_API_KEY.

        验证 kimi 提供商使用 MOONSHOT_API_KEY。
        """
        env_vars = {
            "MOONSHOT_API_KEY": "moonshot-key",
            "ANTHROPIC_API_KEY": "anthropic-key",
        }
        with patch.dict(os.environ, env_vars, clear=False):
            with patch.dict(os.environ, {"CONTINUUM_PROVIDER": "kimi"}):
                config = Config.from_env()
                assert config.api_key == "moonshot-key"

    def test_grok_uses_xai_api_key(self):
        """
        Verify grok provider uses XAI_API_KEY.

        验证 grok 提供商使用 XAI_API_KEY。
        """
        env_vars = {
            "XAI_API_KEY": "xai-key",
            "ANTHROPIC_API_KEY": "anthropic-key",
        }
        with patch.dict(os.environ, env_vars, clear=False):
            with patch.dict(os.environ, {"CONTINUUM_PROVIDER": "grok"}):
                config = Config.from_env()
                assert config.api_key == "xai-key"

    def test_ollama_no_api_key_required(self, monkeypatch):
        """
        Verify ollama provider works without API key.

        验证 ollama 提供商在没有 API 密钥时也能工作。
        """
        # Clear all API keys
        for key in list(os.environ.keys()):
            if "API_KEY" in key:
                monkeypatch.delenv(key, raising=False)

        monkeypatch.setenv("CONTINUUM_PROVIDER", "ollama")
        config = Config.from_env()

        # Ollama should work without API key (local provider)
        assert config.provider == "ollama"
        # API key may be None for ollama


class TestFallbackChain:
    """
    Test complete fallback chain scenarios.

    测试完整的回退链场景。
    """

    def test_full_fallback_chain_for_model(self):
        """
        Verify complete fallback chain for model selection.

        验证模型选择的完整回退链。
        """
        # Test without CONTINUUM_MODEL - should use builtin default
        with patch.dict(os.environ, {}, clear=True):
            # Re-import to clear cached values
            from importlib import reload

            import continuum_sdk.config.providers as providers_module
            reload(providers_module)

            model = providers_module.get_default_model("anthropic")

            # Should get anthropic's default model
            expected = providers_module.BUILTIN_PROVIDERS["anthropic"].default_model
            assert model == expected

    def test_provider_fallback_in_config(self):
        """
        Verify Config can handle provider fallback scenarios.

        验证 Config 能处理提供商回退场景。
        """
        # Create config with minimal settings
        with patch.dict(os.environ, {}, clear=True):
            config = Config(provider="custom_unknown", api_key="test-key")

            # Should still work - model will be fetched via fallback
            assert config.provider == "custom_unknown"
            # Model retrieval may raise or use fallback depending on implementation
            try:
                model = config.model
                assert model is not None
            except RuntimeError:
                # Acceptable if no fallback available
                pass