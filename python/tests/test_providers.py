"""
Tests for provider configuration module.

Tests cover:
- BUILTIN_PROVIDERS configuration
- FALLBACK_PROVIDER_ORDER
- ProviderType and ApiFormat enums
- get_default_model function
- get_default_small_model function
- get_default_base_url function
- get_api_format function
- list_models function
- Provider info query functions
"""

import os
from unittest.mock import patch

import pytest

from continuum_sdk.config.providers import (
    BUILTIN_PROVIDERS,
    FALLBACK_PROVIDER_ORDER,
    ApiFormat,
    ProviderInfo,
    ProviderType,
    get_api_format,
    get_default_base_url,
    get_default_model,
    get_default_small_model,
    get_env_key_name,
    get_provider_info,
    list_models,
    list_providers,
)


class TestProviderTypeEnum:
    """Tests for ProviderType enum."""

    def test_provider_type_values(self):
        """Test ProviderType enum has expected values."""
        assert ProviderType.ANTHROPIC.value == "anthropic"
        assert ProviderType.OPENAI.value == "openai"
        assert ProviderType.GOOGLE.value == "google"
        assert ProviderType.GEMINI.value == "gemini"
        assert ProviderType.AZURE.value == "azure"
        assert ProviderType.BEDROCK.value == "bedrock"
        assert ProviderType.OLLAMA.value == "ollama"
        assert ProviderType.CUSTOM.value == "custom"

    def test_provider_type_count(self):
        """Test ProviderType enum has all expected members."""
        expected_types = [
            "ANTHROPIC",
            "OPENAI",
            "GOOGLE",
            "GEMINI",
            "AZURE",
            "BEDROCK",
            "OLLAMA",
            "CUSTOM",
        ]
        actual_types = [pt.name for pt in ProviderType]
        assert set(expected_types) == set(actual_types)


class TestApiFormatEnum:
    """Tests for ApiFormat enum."""

    def test_api_format_values(self):
        """Test ApiFormat enum has expected values."""
        assert ApiFormat.ANTHROPIC.value == "anthropic"
        assert ApiFormat.OPENAI.value == "openai"
        assert ApiFormat.GOOGLE.value == "google"

    def test_api_format_count(self):
        """Test ApiFormat enum has all expected members."""
        expected_formats = ["ANTHROPIC", "OPENAI", "GOOGLE"]
        actual_formats = [af.name for af in ApiFormat]
        assert set(expected_formats) == set(actual_formats)


class TestProviderInfo:
    """Tests for ProviderInfo dataclass."""

    def test_provider_info_creation(self):
        """Test ProviderInfo can be created with all fields."""
        info = ProviderInfo(
            name="test_provider",
            display_name="Test Provider",
            default_model="test-model",
            default_small_model="test-small-model",
            default_base_url="https://api.test.com",
            env_key_name="TEST_API_KEY",
            models=["model-a", "model-b"],
            api_format=ApiFormat.OPENAI,
        )
        assert info.name == "test_provider"
        assert info.display_name == "Test Provider"
        assert info.default_model == "test-model"
        assert info.default_small_model == "test-small-model"
        assert info.default_base_url == "https://api.test.com"
        assert info.env_key_name == "TEST_API_KEY"
        assert info.models == ["model-a", "model-b"]
        assert info.api_format == ApiFormat.OPENAI

    def test_provider_info_defaults(self):
        """Test ProviderInfo default values."""
        info = ProviderInfo(
            name="test",
            display_name="Test",
            default_model="default",
        )
        assert info.default_small_model is None
        assert info.default_base_url is None
        assert info.env_key_name is None
        assert info.models == []
        assert info.api_format == ApiFormat.OPENAI

    def test_provider_info_partial_fields(self):
        """Test ProviderInfo with some optional fields."""
        info = ProviderInfo(
            name="partial",
            display_name="Partial Provider",
            default_model="partial-model",
            default_base_url="https://partial.com",
        )
        assert info.default_small_model is None
        assert info.env_key_name is None
        assert info.models == []


class TestBuiltinProviders:
    """Tests for BUILTIN_PROVIDERS configuration."""

    def test_builtin_providers_is_dict(self):
        """Test BUILTIN_PROVIDERS is a dictionary."""
        assert isinstance(BUILTIN_PROVIDERS, dict)

    def test_builtin_providers_not_empty(self):
        """Test BUILTIN_PROVIDERS has providers configured."""
        assert len(BUILTIN_PROVIDERS) > 0

    def test_builtin_providers_contains_anthropic(self):
        """Test Anthropic provider exists in BUILTIN_PROVIDERS."""
        assert "anthropic" in BUILTIN_PROVIDERS
        info = BUILTIN_PROVIDERS["anthropic"]
        assert info.name == "anthropic"
        assert info.api_format == ApiFormat.ANTHROPIC
        assert info.env_key_name == "ANTHROPIC_API_KEY"

    def test_builtin_providers_contains_openai(self):
        """Test OpenAI provider exists in BUILTIN_PROVIDERS."""
        assert "openai" in BUILTIN_PROVIDERS
        info = BUILTIN_PROVIDERS["openai"]
        assert info.name == "openai"
        assert info.api_format == ApiFormat.OPENAI
        assert info.env_key_name == "OPENAI_API_KEY"

    def test_builtin_providers_contains_google(self):
        """Test Google provider exists in BUILTIN_PROVIDERS."""
        assert "google" in BUILTIN_PROVIDERS
        info = BUILTIN_PROVIDERS["google"]
        assert info.name == "google"
        assert info.api_format == ApiFormat.GOOGLE
        assert info.env_key_name == "GOOGLE_API_KEY"

    def test_builtin_providers_anthropic_has_models(self):
        """Test Anthropic provider has model list."""
        info = BUILTIN_PROVIDERS["anthropic"]
        assert len(info.models) > 0
        assert "claude-sonnet-4-6" in info.models

    def test_builtin_providers_ollama_no_api_key(self):
        """Test Ollama provider has no API key requirement."""
        info = BUILTIN_PROVIDERS["ollama"]
        assert info.env_key_name is None
        assert info.default_base_url == "http://localhost:11434"

    def test_builtin_providers_all_have_required_fields(self):
        """Test all providers have required fields."""
        for name, info in BUILTIN_PROVIDERS.items():
            assert info.name == name
            assert info.display_name is not None
            assert len(info.display_name) > 0
            assert info.default_model is not None
            assert len(info.default_model) > 0 or name == "huggingface"
            assert isinstance(info.api_format, ApiFormat)


class TestFallbackProviderOrder:
    """Tests for FALLBACK_PROVIDER_ORDER."""

    def test_fallback_order_is_list(self):
        """Test FALLBACK_PROVIDER_ORDER is a list."""
        assert isinstance(FALLBACK_PROVIDER_ORDER, list)

    def test_fallback_order_not_empty(self):
        """Test FALLBACK_PROVIDER_ORDER has entries."""
        assert len(FALLBACK_PROVIDER_ORDER) > 0

    def test_fallback_order_starts_with_anthropic(self):
        """Test Anthropic is first in fallback order."""
        assert FALLBACK_PROVIDER_ORDER[0] == "anthropic"

    def test_fallback_order_contains_primary_providers(self):
        """Test primary providers are in fallback order."""
        assert "anthropic" in FALLBACK_PROVIDER_ORDER
        assert "openai" in FALLBACK_PROVIDER_ORDER
        assert "google" in FALLBACK_PROVIDER_ORDER

    def test_fallback_providers_exist_in_builtin(self):
        """Test all fallback providers exist in BUILTIN_PROVIDERS."""
        for provider in FALLBACK_PROVIDER_ORDER:
            assert provider in BUILTIN_PROVIDERS


class TestGetProviderInfo:
    """Tests for get_provider_info function."""

    def test_get_provider_info_for_known_provider(self):
        """Test get_provider_info returns info for known provider."""
        result = get_provider_info("anthropic")
        assert result is not None
        assert result.name == "anthropic"
        assert result.display_name == "Anthropic (Claude)"

    def test_get_provider_info_for_unknown_provider(self):
        """Test get_provider_info returns None for unknown provider."""
        result = get_provider_info("nonexistent_provider")
        assert result is None

    def test_get_provider_info_for_all_builtin(self):
        """Test get_provider_info works for all builtin providers."""
        for name in BUILTIN_PROVIDERS:
            result = get_provider_info(name)
            assert result is not None
            assert result.name == name


class TestListProviders:
    """Tests for list_providers function."""

    def test_list_providers_returns_list(self):
        """Test list_providers returns a list."""
        result = list_providers()
        assert isinstance(result, list)

    def test_list_providers_not_empty(self):
        """Test list_providers returns non-empty list."""
        result = list_providers()
        assert len(result) > 0

    def test_list_providers_contains_all_builtin(self):
        """Test list_providers contains all builtin provider names."""
        result = list_providers()
        assert set(result) == set(BUILTIN_PROVIDERS.keys())

    def test_list_providers_contains_expected_providers(self):
        """Test list_providers contains expected providers."""
        result = list_providers()
        assert "anthropic" in result
        assert "openai" in result
        assert "google" in result


class TestGetDefaultModel:
    """Tests for get_default_model function."""

    def test_default_model_env_variable_priority(self):
        """Test CONTINUUM_MODEL env var takes highest priority."""
        with patch.dict(os.environ, {"CONTINUUM_MODEL": "custom-model"}, clear=False):
            result = get_default_model("anthropic")
            assert result == "custom-model"

    def test_default_model_from_builtin_config(self):
        """Test default model from builtin config when no env var."""
        with patch.dict(os.environ, {}, clear=True):
            from importlib import reload

            import continuum_sdk.config.providers as providers_module

            reload(providers_module)

            result = providers_module.get_default_model("anthropic")
            expected = providers_module.BUILTIN_PROVIDERS["anthropic"].default_model
            assert result == expected

    def test_default_model_for_openai(self):
        """Test default model for OpenAI provider."""
        with patch.dict(os.environ, {}, clear=True):
            from importlib import reload

            import continuum_sdk.config.providers as providers_module

            reload(providers_module)

            result = providers_module.get_default_model("openai")
            expected = providers_module.BUILTIN_PROVIDERS["openai"].default_model
            assert result == expected

    def test_default_model_fallback_for_unknown_provider(self):
        """Test fallback for unknown provider."""
        with patch.dict(os.environ, {}, clear=True):
            result = get_default_model("unknown_provider_xyz")
            # Should get first fallback provider's default model
            first_fallback = FALLBACK_PROVIDER_ORDER[0]
            expected = BUILTIN_PROVIDERS[first_fallback].default_model
            assert result == expected

    def test_default_model_fallback_follows_order(self):
        """Test fallback follows FALLBACK_PROVIDER_ORDER."""
        with patch.dict(os.environ, {}, clear=True):
            result = get_default_model("nonexistent")
            # Should use first provider in fallback order
            first_fallback = FALLBACK_PROVIDER_ORDER[0]
            expected = BUILTIN_PROVIDERS[first_fallback].default_model
            assert result == expected

    def test_default_model_fallback_to_first_builtin(self):
        """Test fallback to first builtin provider."""
        with patch.dict(os.environ, {}, clear=True):
            # Mock fallback order to be empty to test the second fallback path
            with patch("continuum_sdk.config.providers.FALLBACK_PROVIDER_ORDER", []):
                result = get_default_model("unknown")
                # Should get first provider in BUILTIN_PROVIDERS
                assert result is not None
                assert isinstance(result, str)

    def test_default_model_raises_when_no_config(self):
        """Test RuntimeError when no config available."""
        with patch.dict(os.environ, {}, clear=True):
            with patch("continuum_sdk.config.providers.BUILTIN_PROVIDERS", {}):
                with pytest.raises(RuntimeError) as exc_info:
                    get_default_model("any_provider")
                assert "Unable to get any default model config" in str(exc_info.value)


class TestGetDefaultSmallModel:
    """Tests for get_default_small_model function."""

    def test_get_default_small_model_for_anthropic(self):
        """Test get_default_small_model for Anthropic."""
        result = get_default_small_model("anthropic")
        assert result is not None
        assert result == "claude-haiku-4-5"

    def test_get_default_small_model_for_openai(self):
        """Test get_default_small_model for OpenAI."""
        result = get_default_small_model("openai")
        assert result is not None
        assert result == "gpt-4.1-mini"

    def test_get_default_small_model_for_provider_without_small_model(self):
        """Test get_default_small_model for provider without small model."""
        # Some providers may not have a small model configured
        result = get_default_small_model("huggingface")
        assert result is None  # HuggingFace has no small model configured

    def test_get_default_small_model_for_unknown_provider(self):
        """Test get_default_small_model returns None for unknown provider."""
        result = get_default_small_model("nonexistent_provider")
        assert result is None

    def test_get_default_small_model_returns_none_when_not_set(self):
        """Test get_default_small_model returns None when provider has no small model."""
        # Create a provider info without small model
        with patch.dict(
            BUILTIN_PROVIDERS,
            {
                "test_no_small": ProviderInfo(
                    name="test_no_small",
                    display_name="Test No Small",
                    default_model="default",
                    default_small_model=None,
                )
            },
        ):
            result = get_default_small_model("test_no_small")
            assert result is None


class TestGetEnvKeyName:
    """Tests for get_env_key_name function."""

    def test_get_env_key_name_for_anthropic(self):
        """Test get_env_key_name for Anthropic."""
        result = get_env_key_name("anthropic")
        assert result == "ANTHROPIC_API_KEY"

    def test_get_env_key_name_for_openai(self):
        """Test get_env_key_name for OpenAI."""
        result = get_env_key_name("openai")
        assert result == "OPENAI_API_KEY"

    def test_get_env_key_name_for_google(self):
        """Test get_env_key_name for Google."""
        result = get_env_key_name("google")
        assert result == "GOOGLE_API_KEY"

    def test_get_env_key_name_for_deepseek(self):
        """Test get_env_key_name for DeepSeek."""
        result = get_env_key_name("deepseek")
        assert result == "DEEPSEEK_API_KEY"

    def test_get_env_key_name_for_ollama(self):
        """Test get_env_key_name for Ollama (no key required)."""
        result = get_env_key_name("ollama")
        assert result is None

    def test_get_env_key_name_for_unknown_provider(self):
        """Test get_env_key_name returns None for unknown provider."""
        result = get_env_key_name("nonexistent")
        assert result is None


class TestGetDefaultBaseUrl:
    """Tests for get_default_base_url function."""

    def test_get_default_base_url_for_anthropic(self):
        """Test get_default_base_url for Anthropic."""
        result = get_default_base_url("anthropic")
        assert result == "https://api.anthropic.com"

    def test_get_default_base_url_for_openai(self):
        """Test get_default_base_url for OpenAI."""
        result = get_default_base_url("openai")
        assert result == "https://api.openai.com/v1"

    def test_get_default_base_url_for_google(self):
        """Test get_default_base_url for Google."""
        result = get_default_base_url("google")
        assert result == "https://generativelanguage.googleapis.com/v1beta"

    def test_get_default_base_url_for_ollama(self):
        """Test get_default_base_url for Ollama."""
        result = get_default_base_url("ollama")
        assert result == "http://localhost:11434"

    def test_get_default_base_url_for_unknown_provider(self):
        """Test get_default_base_url returns None for unknown provider."""
        result = get_default_base_url("nonexistent_provider")
        assert result is None


class TestGetApiFormat:
    """Tests for get_api_format function."""

    def test_get_api_format_for_anthropic(self):
        """Test get_api_format for Anthropic returns ANTHROPIC format."""
        result = get_api_format("anthropic")
        assert result.value == ApiFormat.ANTHROPIC.value

    def test_get_api_format_for_openai(self):
        """Test get_api_format for OpenAI returns OPENAI format."""
        result = get_api_format("openai")
        assert result.value == ApiFormat.OPENAI.value

    def test_get_api_format_for_google(self):
        """Test get_api_format for Google returns GOOGLE format."""
        result = get_api_format("google")
        assert result.value == ApiFormat.GOOGLE.value

    def test_get_api_format_for_openai_compatible_provider(self):
        """Test get_api_format for OpenAI-compatible providers."""
        result = get_api_format("deepseek")
        assert result.value == ApiFormat.OPENAI.value

        result = get_api_format("groq")
        assert result.value == ApiFormat.OPENAI.value

    def test_get_api_format_for_unknown_provider_returns_openai(self):
        """Test get_api_format returns OPENAI for unknown provider (default)."""
        result = get_api_format("unknown_provider")
        assert result.value == ApiFormat.OPENAI.value


class TestListModels:
    """Tests for list_models function."""

    def test_list_models_for_anthropic(self):
        """Test list_models for Anthropic returns model list."""
        result = list_models("anthropic")
        assert isinstance(result, list)
        assert len(result) > 0
        assert "claude-sonnet-4-6" in result

    def test_list_models_for_openai(self):
        """Test list_models for OpenAI returns model list."""
        result = list_models("openai")
        assert isinstance(result, list)
        assert len(result) > 0
        assert "gpt-5.5" in result

    def test_list_models_returns_copy(self):
        """Test list_models returns a copy (not mutable reference)."""
        result = list_models("anthropic")
        # Verify it's a copy by checking it's a new list
        original = BUILTIN_PROVIDERS["anthropic"].models
        assert result == original
        # Modify the returned list should not affect original
        result.append("fake-model")
        assert "fake-model" not in original

    def test_list_models_for_provider_with_empty_models(self):
        """Test list_models for provider with empty model list."""
        result = list_models("ollama")
        assert isinstance(result, list)
        assert len(result) == 0

    def test_list_models_for_unknown_provider(self):
        """Test list_models returns empty list for unknown provider."""
        result = list_models("nonexistent_provider")
        assert isinstance(result, list)
        assert len(result) == 0


class TestProviderSpecificConfigurations:
    """Tests for specific provider configurations."""

    def test_anthropic_has_correct_config(self):
        """Test Anthropic provider has correct configuration."""
        info = BUILTIN_PROVIDERS["anthropic"]
        assert info.name == "anthropic"
        assert info.display_name == "Anthropic (Claude)"
        assert info.default_model == "claude-sonnet-4-6"
        assert info.default_small_model == "claude-haiku-4-5"
        assert info.api_format == ApiFormat.ANTHROPIC
        assert info.env_key_name == "ANTHROPIC_API_KEY"

    def test_google_and_gemini_share_config(self):
        """Test Google and Gemini providers share similar config."""
        google = BUILTIN_PROVIDERS["google"]
        gemini = BUILTIN_PROVIDERS["gemini"]
        assert google.env_key_name == gemini.env_key_name
        assert google.default_model == gemini.default_model
        assert google.api_format == ApiFormat.GOOGLE
        assert gemini.api_format == ApiFormat.GOOGLE

    def test_chinese_providers_config(self):
        """Test Chinese provider configurations."""
        # GLM
        glm = BUILTIN_PROVIDERS["glm"]
        assert glm.env_key_name == "GLM_API_KEY"
        assert glm.api_format == ApiFormat.OPENAI

        # Qwen
        qwen = BUILTIN_PROVIDERS["qwen"]
        assert qwen.env_key_name == "QWEN_API_KEY"
        assert qwen.api_format == ApiFormat.OPENAI

        # Kimi
        kimi = BUILTIN_PROVIDERS["kimi"]
        assert kimi.env_key_name == "MOONSHOT_API_KEY"
        assert kimi.api_format == ApiFormat.OPENAI

    def test_bedrock_config(self):
        """Test AWS Bedrock configuration."""
        info = BUILTIN_PROVIDERS["bedrock"]
        assert info.env_key_name == "AWS_ACCESS_KEY_ID"
        assert info.api_format == ApiFormat.OPENAI
        assert len(info.models) == 0

    def test_azure_config(self):
        """Test Azure OpenAI configuration."""
        info = BUILTIN_PROVIDERS["azure"]
        assert info.env_key_name == "AZURE_OPENAI_API_KEY"
        assert info.api_format == ApiFormat.OPENAI
        assert len(info.models) == 0


class TestEdgeCases:
    """Tests for edge cases and error handling."""

    def test_empty_provider_name(self):
        """Test functions handle empty provider name."""
        assert get_provider_info("") is None
        assert get_default_small_model("") is None
        assert get_env_key_name("") is None
        assert get_default_base_url("") is None
        assert get_api_format("").value == ApiFormat.OPENAI.value
        assert list_models("") == []

    def test_case_sensitivity(self):
        """Test provider lookup is case-sensitive."""
        # lowercase should work
        assert get_provider_info("anthropic") is not None
        # uppercase should not work
        assert get_provider_info("ANTHROPIC") is None
        # mixed case should not work
        assert get_provider_info("Anthropic") is None

    def test_whitespace_in_provider_name(self):
        """Test functions handle whitespace in provider name."""
        assert get_provider_info("  anthropic  ") is None
        assert get_provider_info("anthropic ") is None

    def test_special_characters_in_provider_name(self):
        """Test functions handle special characters in provider name."""
        assert get_provider_info("anthropic-test") is None
        assert get_provider_info("anthropic.test") is None
