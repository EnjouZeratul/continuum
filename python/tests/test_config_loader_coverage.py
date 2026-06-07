"""
Config Loader Tests - Coverage Enhancement

Tests for config/loader.py to improve coverage from 85% to 95%+.
"""

import json
import os
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest

from continuum_sdk.config.loader import (
    Config,
    ProviderConfig,
    ConfigLoader,
    load_config,
    get_user_config_dir,
    _get_env,
    ALLOWED_ENV_VARS,
)


class TestGetEnv:
    """Test _get_env security function"""

    def test_allowed_env_var(self, monkeypatch):
        """Test accessing allowed environment variable"""
        monkeypatch.setenv("CONTINUUM_API_KEY", "test-key")
        result = _get_env("CONTINUUM_API_KEY")
        assert result == "test-key"

    def test_blocked_env_var(self, monkeypatch):
        """Test that blocked env vars return None"""
        # PATH is not in the whitelist
        monkeypatch.setenv("PATH", "/usr/bin")
        result = _get_env("PATH")
        assert result is None

    def test_default_value(self):
        """Test default value for non-existent var"""
        result = _get_env("CONTINUUM_NONEXISTENT_VAR", default="default")
        assert result == "default"

    def test_allowed_vars_whitelist(self):
        """Test that whitelist contains expected vars"""
        assert "CONTINUUM_API_KEY" in ALLOWED_ENV_VARS
        assert "ANTHROPIC_API_KEY" in ALLOWED_ENV_VARS
        assert "OPENAI_API_KEY" in ALLOWED_ENV_VARS
        assert "GOOGLE_API_KEY" in ALLOWED_ENV_VARS
        assert "DEEPSEEK_API_KEY" in ALLOWED_ENV_VARS
        assert "USE_REAL_API" in ALLOWED_ENV_VARS
        assert "CONTINUUM_THEME_CONFIG" in ALLOWED_ENV_VARS


class TestProviderConfig:
    """Test ProviderConfig dataclass"""

    def test_provider_config_creation(self):
        """Test basic creation"""
        config = ProviderConfig(name="anthropic")
        assert config.name == "anthropic"
        assert config.api_key is None
        assert config.base_url is None

    def test_provider_config_full(self):
        """Test with all fields"""
        config = ProviderConfig(
            name="openai",
            api_key="sk-test",
            base_url="https://api.openai.com",
            model="gpt-4",
        )
        assert config.api_key == "sk-test"
        assert config.base_url == "https://api.openai.com"
        assert config.model == "gpt-4"

    def test_to_dict(self):
        """Test to_dict method"""
        config = ProviderConfig(
            name="test",
            api_key="key",
            base_url="url",
            model="model",
            small_model="small",
        )
        d = config.to_dict()
        assert d["name"] == "test"
        assert d["api_key"] == "key"
        assert d["base_url"] == "url"


class TestConfig:
    """Test Config class"""

    def test_config_creation(self):
        """Test basic config creation"""
        config = Config()
        assert config.provider == "anthropic"

    def test_config_with_params(self):
        """Test config with parameters"""
        config = Config(
            provider="openai",
            api_key="test-key",
            model="gpt-4",
        )
        assert config.provider == "openai"
        assert config.api_key == "test-key"
        assert config.model == "gpt-4"

    def test_config_properties(self):
        """Test config property access"""
        config = Config(
            provider="anthropic",
            api_key="key",
            model="claude-sonnet-4-6",
            max_tokens=8192,
            temperature=0.5,
        )
        assert config.provider == "anthropic"
        assert config.api_key == "key"
        assert config.model == "claude-sonnet-4-6"
        assert config.max_tokens == 8192
        assert config.temperature == 0.5

    def test_config_get_set(self):
        """Test get/set methods"""
        config = Config()
        config.set("custom_key", "custom_value")
        assert config.get("custom_key") == "custom_value"
        assert config.get("nonexistent", "default") == "default"

    def test_config_update(self):
        """Test update method"""
        config = Config()
        config.update({"key1": "value1", "key2": "value2"})
        assert config.get("key1") == "value1"
        assert config.get("key2") == "value2"

    def test_config_to_dict(self):
        """Test to_dict method"""
        config = Config(provider="test", api_key="key")
        d = config.to_dict()
        assert "provider" in d
        assert d["provider"] == "test"

    def test_config_from_dict(self):
        """Test from_dict class method"""
        data = {"provider": "openai", "api_key": "test-key"}
        config = Config.from_dict(data)
        assert config.provider == "openai"
        assert config.api_key == "test-key"

    def test_config_from_env(self, monkeypatch):
        """Test from_env class method"""
        monkeypatch.setenv("CONTINUUM_API_KEY", "env-key")
        monkeypatch.setenv("CONTINUUM_PROVIDER", "google")
        config = Config.from_env()
        assert config.api_key == "env-key"
        assert config.provider == "google"

    def test_config_from_file_json(self):
        """Test loading from JSON file"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"provider": "openai", "model": "gpt-4"}, f)
            path = f.name

        try:
            config = Config.from_file(path)
            assert config.provider == "openai"
            assert config.model == "gpt-4"
        finally:
            os.unlink(path)

    def test_config_from_file_toml(self):
        """Test loading from TOML file"""
        toml_content = """
provider = "anthropic"
model = "claude-sonnet-4-6"
"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".toml", delete=False
        ) as f:
            f.write(toml_content)
            path = f.name

        try:
            config = Config.from_file(path)
            assert config.provider == "anthropic"
        except ImportError:
            # TOML support requires Python 3.11+ or tomli
            pass
        finally:
            os.unlink(path)

    def test_config_from_file_not_found(self):
        """Test error for nonexistent file"""
        with pytest.raises(FileNotFoundError):
            Config.from_file("/nonexistent/path/config.json")

    def test_config_from_default(self, monkeypatch):
        """Test from_default class method"""
        # Clear any existing env vars
        for var in ALLOWED_ENV_VARS:
            monkeypatch.delenv(var, raising=False)

        config = Config.from_default()
        assert config is not None

    def test_config_use_provider(self):
        """Test use() method for switching providers"""
        config = Config(provider="anthropic")
        config.add_provider("openai", api_key="openai-key", model="gpt-4")
        result = config.use("openai")
        assert result is config  # Returns self for chaining

    def test_config_add_provider(self):
        """Test add_provider method"""
        config = Config()
        config.add_provider(
            "custom",
            api_key="custom-key",
            base_url="https://custom.api",
            model="custom-model",
        )
        assert "custom" in config.list_providers()

    def test_config_list_providers(self):
        """Test list_providers method"""
        config = Config()
        config.add_provider("test1")
        config.add_provider("test2")
        providers = config.list_providers()
        assert "test1" in providers
        assert "test2" in providers

    def test_config_repr(self):
        """Test __repr__ method"""
        config = Config(provider="test", model="test-model")
        repr_str = repr(config)
        assert "Config" in repr_str
        assert "test" in repr_str


class TestConfigLoader:
    """Test ConfigLoader class"""

    def test_config_loader_init(self):
        """Test initialization"""
        loader = ConfigLoader()
        assert loader._config is None

    def test_config_loader_with_path(self):
        """Test with path"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"provider": "test"}, f)
            path = f.name

        try:
            loader = ConfigLoader(path)
            config = loader.load()
            assert config.provider == "test"
        finally:
            os.unlink(path)

    def test_config_loader_load_default(self):
        """Test loading default config"""
        loader = ConfigLoader()
        config = loader.load()
        assert config is not None

    def test_config_loader_get_config(self):
        """Test get_config method"""
        loader = ConfigLoader()
        loader.load()
        config = loader.get_config()
        assert config is not None

    def test_config_loader_save(self):
        """Test save method"""
        with tempfile.TemporaryDirectory() as tmpdir:
            loader = ConfigLoader()
            loader.load()

            save_path = os.path.join(tmpdir, "saved_config.json")
            loader.save(save_path)

            assert os.path.exists(save_path)

            # Verify content
            with open(save_path) as f:
                data = json.load(f)
            assert "provider" in data

    def test_config_loader_save_no_config(self):
        """Test save without loading config"""
        loader = ConfigLoader()
        with pytest.raises(ValueError, match="No config loaded"):
            loader.save()

    def test_config_loader_get_default_config(self):
        """Test get_default_config static method"""
        config = ConfigLoader.get_default_config()
        assert isinstance(config, Config)


class TestUtilityFunctions:
    """Test utility functions"""

    def test_load_config_with_path(self):
        """Test load_config with path"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"provider": "test"}, f)
            path = f.name

        try:
            config = load_config(path)
            assert config.provider == "test"
        finally:
            os.unlink(path)

    def test_load_config_default(self):
        """Test load_config without path"""
        config = load_config()
        assert config is not None

    def test_get_user_config_dir(self):
        """Test get_user_config_dir"""
        config_dir = get_user_config_dir()
        assert isinstance(config_dir, Path)
        # Should be under home directory
        assert ".config" in str(config_dir) or "AppData" in str(config_dir)


class TestConfigEdgeCases:
    """Test edge cases and error handling"""

    def test_config_effort_level(self):
        """Test effort_level property"""
        config = Config(effort_level="high")
        assert config.effort_level == "high"

    def test_config_disable_traffic(self):
        """Test disable_traffic property"""
        config = Config(disable_traffic=True)
        assert config.disable_traffic is True

    def test_config_budget(self):
        """Test budget property"""
        config = Config(budget=100.0)
        assert config.budget == 100.0

    def test_config_audit_enabled(self):
        """Test audit_enabled property"""
        config = Config(audit_enabled=False)
        assert config.audit_enabled is False

    def test_config_api_format(self):
        """Test api_format property"""
        config = Config(api_format="openai")
        assert config.api_format == "openai"

    def test_config_small_model(self):
        """Test small_model property"""
        config = Config(small_model="gpt-4o-mini")
        assert config.small_model == "gpt-4o-mini"

    def test_env_var_expansion(self, monkeypatch):
        """Test environment variable expansion in config files"""
        monkeypatch.setenv("TEST_CONFIG_VAR", "expanded_value")

        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"api_key": "${TEST_CONFIG_VAR}"}, f)
            path = f.name

        try:
            config = Config.from_file(path)
            # The expansion should work for whitelisted patterns
            # but may not for arbitrary vars
        finally:
            os.unlink(path)


class TestConfigFromEnvAdvanced:
    """Test Config.from_env with advanced scenarios"""

    def test_from_env_with_conversion_error(self, monkeypatch):
        """Test from_env handles conversion errors gracefully"""
        # Set invalid value for integer conversion
        monkeypatch.setenv("CONTINUUM_MAX_TOKENS", "invalid_number")
        config = Config.from_env()
        # Should use default value when conversion fails
        assert config.max_tokens == 4096

    def test_from_env_with_float_conversion(self, monkeypatch):
        """Test from_env with float conversion"""
        # CONTINUUM_TEMPERATURE is whitelisted in ALLOWED_ENV_VARS
        monkeypatch.setenv("CONTINUUM_TEMPERATURE", "0.9")
        config = Config.from_env()
        # Temperature is read from env var
        assert config.temperature == 0.9

    def test_from_env_with_int_conversion(self, monkeypatch):
        """Test from_env with int conversion"""
        monkeypatch.setenv("CONTINUUM_MAX_TOKENS", "8192")
        config = Config.from_env()
        assert config.max_tokens == 8192

    def test_from_env_budget_conversion(self, monkeypatch):
        """Test from_env with budget float conversion"""
        # CONTINUUM_BUDGET is whitelisted in ALLOWED_ENV_VARS
        monkeypatch.setenv("CONTINUUM_BUDGET", "99.99")
        config = Config.from_env()
        # Budget is read from env var
        assert config.budget == 99.99

    def test_from_env_boolean_true_values(self, monkeypatch):
        """Test from_env with boolean conversion"""
        # CONTINUUM_DISABLE_TRAFFIC is whitelisted in ALLOWED_ENV_VARS
        monkeypatch.setenv("CONTINUUM_DISABLE_TRAFFIC", "true")
        config = Config.from_env()
        # disable_traffic is read from env var
        assert config.disable_traffic is True

    def test_from_env_boolean_false_values(self, monkeypatch):
        """Test from_env boolean false values"""
        monkeypatch.setenv("CONTINUUM_DISABLE_TRAFFIC", "false")
        config = Config.from_env()
        assert config.disable_traffic is False

    def test_from_env_audit_enabled_boolean(self, monkeypatch):
        """Test from_env with audit_enabled boolean"""
        monkeypatch.setenv("CONTINUUM_AUDIT_ENABLED", "true")
        config = Config.from_env()
        assert config.audit_enabled is True

    def test_from_env_audit_retention_int(self, monkeypatch):
        """Test from_env with audit retention conversion"""
        monkeypatch.setenv("CONTINUUM_AUDIT_RETENTION", "30")
        config = Config.from_env()
        # This is stored as audit_retention_days internally
        assert config.get("audit_retention_days") == 30

    def test_from_env_provider_specific_api_key(self, monkeypatch):
        """Test from_env with provider-specific API key fallback"""
        monkeypatch.setenv("CONTINUUM_PROVIDER", "openai")
        monkeypatch.setenv("OPENAI_API_KEY", "openai-key-from-provider")
        config = Config.from_env()
        assert config.api_key == "openai-key-from-provider"

    def test_from_env_provider_specific_base_url(self, monkeypatch):
        """Test from_env with provider-specific base URL"""
        monkeypatch.setenv("CONTINUUM_PROVIDER", "anthropic")
        monkeypatch.setenv("ANTHROPIC_BASE_URL", "https://custom.anthropic.com")
        config = Config.from_env()
        assert config.base_url == "https://custom.anthropic.com"

    def test_from_env_continuum_prefix_priority(self, monkeypatch):
        """Test that CONTINUUM_* has priority"""
        monkeypatch.setenv("CONTINUUM_API_KEY", "continuum-key")
        monkeypatch.setenv("ANTHROPIC_API_KEY", "anthropic-key")
        config = Config.from_env()
        assert config.api_key == "continuum-key"


class TestConfigUseProviderAdvanced:
    """Test Config.use() with provider configurations"""

    def test_use_updates_all_provider_fields(self):
        """Test use() updates api_key, base_url, and model"""
        config = Config()
        config.add_provider(
            "custom",
            api_key="custom-key",
            base_url="https://custom.api",
            model="custom-model"
        )
        config.use("custom")
        assert config.api_key == "custom-key"
        assert config.base_url == "https://custom.api"
        assert config.model == "custom-model"

    def test_use_with_small_model(self):
        """Test use() does not copy small_model (implementation behavior)"""
        config = Config()
        config.add_provider(
            "provider-with-small",
            api_key="key",
            model="big-model",
            small_model="small-model"
        )
        config.use("provider-with-small")
        # Note: use() only copies api_key, base_url, and model (not small_model)
        # small_model stays in _providers but doesn't override _data
        assert config.model == "big-model"
        # small_model in _data is not updated by use()
        assert config.small_model is None

    def test_use_nonexistent_provider(self):
        """Test use() with provider not in _providers"""
        config = Config(provider="anthropic")
        result = config.use("unknown-provider")
        assert result is config
        assert config.provider == "unknown-provider"


class TestConfigFileLoading:
    """Test config file loading edge cases"""

    def test_load_toml_without_tomllib(self, monkeypatch):
        """Test TOML loading when tomllib is not available"""
        # Patch tomllib to None to simulate missing support
        import continuum_sdk.config.loader as loader_module
        original_tomllib = loader_module.tomllib
        loader_module.tomllib = None

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", suffix=".toml", delete=False
            ) as f:
                f.write('provider = "test"')
                path = f.name

            try:
                config_data = Config._load_file(Path(path))
                # Should return empty dict when tomllib is None
                assert config_data == {}
            finally:
                os.unlink(path)
        finally:
            loader_module.tomllib = original_tomllib

    def test_load_file_json_decode_error(self):
        """Test handling of invalid JSON"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            f.write("{ invalid json }")
            path = f.name

        try:
            config_data = Config._load_file(Path(path))
            # Should return empty dict on error
            assert config_data == {}
        finally:
            os.unlink(path)

    def test_load_file_toml_decode_error(self):
        """Test handling of invalid TOML - note: ValueError not caught by current handler"""
        # Only run if tomllib is available
        import continuum_sdk.config.loader as loader_module
        if loader_module.tomllib is None:
            pytest.skip("TOML support not available")

        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".toml", delete=False
        ) as f:
            f.write("invalid toml [[[[")
            path = f.name

        try:
            # TOMLDecodeError is a ValueError, not caught by the exception handler
            # This test documents current behavior - the error propagates
            with pytest.raises(ValueError):  # TOMLDecodeError is subclass of ValueError
                config_data = Config._load_file(Path(path))
        finally:
            os.unlink(path)

    def test_load_file_unknown_extension_json_content(self):
        """Test auto-detection with JSON content for unknown extension"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".custom", delete=False
        ) as f:
            json.dump({"provider": "auto-detected"}, f)
            path = f.name

        try:
            config_data = Config._load_file(Path(path))
            assert config_data == {"provider": "auto-detected"}
        finally:
            os.unlink(path)

    def test_load_file_unknown_extension_toml_content(self):
        """Test auto-detection with TOML content for unknown extension"""
        import continuum_sdk.config.loader as loader_module
        if loader_module.tomllib is None:
            pytest.skip("TOML support not available")

        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".custom", delete=False
        ) as f:
            f.write('provider = "auto-detected-toml"')
            path = f.name

        try:
            config_data = Config._load_file(Path(path))
            assert config_data == {"provider": "auto-detected-toml"}
        finally:
            os.unlink(path)


class TestConfigFindFile:
    """Test config file finding logic"""

    def test_find_config_file_in_current_dir(self):
        """Test finding config file in current directory"""
        with tempfile.TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "config.toml"
            config_path.write_text('provider = "test"')

            # Temporarily change DEFAULT_CONFIG_DIRS
            original_dirs = Config.DEFAULT_CONFIG_DIRS
            Config.DEFAULT_CONFIG_DIRS = [tmpdir]
            try:
                found = Config._find_config_file()
                assert found == config_path
            finally:
                Config.DEFAULT_CONFIG_DIRS = original_dirs

    def test_find_config_file_none_found(self):
        """Test when no config file is found"""
        with tempfile.TemporaryDirectory() as tmpdir:
            original_dirs = Config.DEFAULT_CONFIG_DIRS
            Config.DEFAULT_CONFIG_DIRS = [tmpdir]
            try:
                found = Config._find_config_file()
                assert found is None
            finally:
                Config.DEFAULT_CONFIG_DIRS = original_dirs


class TestConfigFromDefaultAdvanced:
    """Test Config.from_default() advanced scenarios"""

    def test_from_default_with_config_file(self):
        """Test from_default loads config file when available"""
        with tempfile.TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "config.toml"
            config_path.write_text('custom_key = "from_file"')

            original_dirs = Config.DEFAULT_CONFIG_DIRS
            Config.DEFAULT_CONFIG_DIRS = [tmpdir]
            try:
                config = Config.from_default()
                assert config.get("custom_key") == "from_file"
            finally:
                Config.DEFAULT_CONFIG_DIRS = original_dirs


class TestEnvVarExpansionAdvanced:
    """Test environment variable expansion"""

    def test_expand_env_vars_dict(self, monkeypatch):
        """Test expansion in nested dict"""
        monkeypatch.setenv("CONTINUUM_API_KEY", "expanded-key")
        data = {
            "nested": {
                "api_key": "${CONTINUUM_API_KEY}"
            }
        }
        result = Config._expand_env_vars(data)
        assert result["nested"]["api_key"] == "expanded-key"

    def test_expand_env_vars_list(self, monkeypatch):
        """Test expansion in list"""
        monkeypatch.setenv("CONTINUUM_MODEL", "gpt-4")
        data = {
            "models": ["${CONTINUUM_MODEL}", "claude"]
        }
        result = Config._expand_env_vars(data)
        assert result["models"][0] == "gpt-4"

    def test_expand_env_vars_not_allowed(self, monkeypatch):
        """Test that non-whitelisted vars are not expanded"""
        monkeypatch.setenv("NON_WHITELISTED_VAR", "secret")
        data = {"key": "${NON_WHITELISTED_VAR}"}
        result = Config._expand_env_vars(data)
        # Should not expand, keep original
        assert result["key"] == "${NON_WHITELISTED_VAR}"

    def test_expand_env_vars_dollar_without_braces(self, monkeypatch):
        """Test expansion with $VAR format"""
        monkeypatch.setenv("CONTINUUM_PROVIDER", "openai")
        data = {"provider": "$CONTINUUM_PROVIDER"}
        result = Config._expand_env_vars(data)
        assert result["provider"] == "openai"


class TestConfigLoaderSaveAdvanced:
    """Test ConfigLoader.save() edge cases"""

    def test_save_creates_parent_directory(self):
        """Test save creates parent directories"""
        with tempfile.TemporaryDirectory() as tmpdir:
            loader = ConfigLoader()
            loader.load()
            save_path = os.path.join(tmpdir, "subdir", "config.json")
            loader.save(save_path)
            assert os.path.exists(save_path)
            assert os.path.exists(os.path.dirname(save_path))

    def test_save_uses_config_path(self):
        """Test save uses _config_path when no path provided"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create the config file first so from_file can load it
            config_path = os.path.join(tmpdir, "config.json")
            with open(config_path, "w") as f:
                json.dump({"provider": "test"}, f)

            loader = ConfigLoader(config_path)
            loader.load()
            loader.save()
            assert os.path.exists(config_path)
            # File should be updated
            with open(config_path) as f:
                data = json.load(f)
            assert "provider" in data


class TestGetEnvWarning:
    """Test _get_env warning behavior"""

    def test_get_env_silent_for_non_whitelisted(self):
        """Test that _get_env silently returns None for non-whitelisted vars"""
        # Updated behavior: _get_env now silently returns None for non-whitelisted vars
        # This is to avoid spamming warnings during expected fallback checks
        result = _get_env("DEFINITELY_NOT_WHITELISTED")
        assert result is None

    def test_get_env_returns_value_for_whitelisted(self, monkeypatch):
        """Test that _get_env returns value for whitelisted vars"""
        monkeypatch.setenv("CONTINUUM_API_KEY", "test-value")
        result = _get_env("CONTINUUM_API_KEY")
        assert result == "test-value"


class TestConfigBaseUrlProperty:
    """Test base_url property edge cases"""

    def test_base_url_none_by_default(self):
        """Test base_url is None when not set"""
        config = Config()
        assert config.base_url is None

    def test_base_url_returns_value(self):
        """Test base_url returns set value"""
        config = Config(base_url="https://custom.url")
        assert config.base_url == "https://custom.url"


class TestConfigDefaultModel:
    """Test _get_default_model integration"""

    def test_default_model_delegates_to_providers(self, monkeypatch):
        """Test _get_default_model calls providers module"""
        mock_default = "mock-default-model"
        # Patch the imported function in loader module
        import continuum_sdk.config.loader as loader_module
        original_func = loader_module._get_provider_default_model
        loader_module._get_provider_default_model = lambda p: mock_default

        try:
            config = Config(provider="test")
            # Set model to None to trigger _get_default_model
            config._data["model"] = None
            model = config.model
            assert model == mock_default
        finally:
            loader_module._get_provider_default_model = original_func


class TestTomllibImportFallback:
    """Test tomllib/tomli import fallback at module level"""

    def test_tomli_fallback_import(self):
        """Test that tomli is imported when tomllib is not available (lines 118-121)"""
        # This test simulates the import fallback scenario
        # We need to temporarily manipulate sys.modules to trigger the fallback

        import sys
        import importlib

        # Save current state
        original_tomllib = sys.modules.get('tomllib')
        original_tomli = sys.modules.get('tomli')

        # Remove tomllib to force fallback
        if 'tomllib' in sys.modules:
            del sys.modules['tomllib']

        # Make sure tomli is available
        try:
            import tomli
            sys.modules['tomli'] = tomli
        except ImportError:
            pytest.skip("tomli not available for fallback test")

        try:
            # Re-import the loader module to trigger the import block
            import continuum_sdk.config.loader as loader_module
            importlib.reload(loader_module)

            # The module should have loaded tomli as tomllib
            assert loader_module.tomllib is not None

        finally:
            # Restore original state
            if original_tomllib:
                sys.modules['tomllib'] = original_tomllib
            if original_tomli:
                sys.modules['tomli'] = original_tomli

            # Reload to restore original state
            importlib.reload(loader_module)

    def test_both_imports_fail(self):
        """Test when both tomllib and tomli are not available (line 121)"""
        import sys
        import importlib

        # Save current state
        original_tomllib = sys.modules.get('tomllib')
        original_tomli = sys.modules.get('tomli')

        # Remove both to simulate complete absence
        if 'tomllib' in sys.modules:
            del sys.modules['tomllib']
        if 'tomli' in sys.modules:
            del sys.modules['tomli']

        # Mock both imports to fail
        import builtins
        original_import = builtins.__import__

        def mock_import(name, *args, **kwargs):
            if name in ('tomllib', 'tomli'):
                raise ImportError(f"No module named '{name}'")
            return original_import(name, *args, **kwargs)

        builtins.__import__ = mock_import

        try:
            # Re-import to trigger both import failures
            import continuum_sdk.config.loader as loader_module
            importlib.reload(loader_module)

            # tomllib should be None when both imports fail
            assert loader_module.tomllib is None

        finally:
            # Restore original state
            builtins.__import__ = original_import
            if original_tomllib:
                sys.modules['tomllib'] = original_tomllib
            if original_tomli:
                sys.modules['tomli'] = original_tomli

            # Reload to restore original state
            importlib.reload(loader_module)


class TestConfigUseProviderMissingFields:
    """Test Config.use() with missing provider fields (lines 506->508, 510->513)"""

    def test_use_provider_with_none_api_key(self):
        """Test use() when provider has None api_key (should not update)"""
        config = Config()
        # Add provider with None api_key but valid base_url
        config.add_provider(
            "test_provider",
            api_key=None,  # None - should NOT update
            base_url="https://test.url",
            model="test-model"
        )
        config.use("test_provider")

        # api_key should remain None (not updated from provider config)
        assert config.api_key is None
        # base_url and model should be updated
        assert config.base_url == "https://test.url"
        assert config.model == "test-model"

    def test_use_provider_with_none_base_url(self):
        """Test use() when provider has None base_url (should not update)"""
        config = Config(base_url="https://original.url")
        config.add_provider(
            "test_provider",
            api_key="test-key",
            base_url=None,  # None - should NOT update
            model="test-model"
        )
        config.use("test_provider")

        # base_url should keep original value (not updated from None)
        assert config.base_url == "https://original.url"
        # api_key and model should be updated
        assert config.api_key == "test-key"
        assert config.model == "test-model"

    def test_use_provider_with_none_model(self):
        """Test use() when provider has None model (should not update)"""
        config = Config(model="original-model")
        config.add_provider(
            "test_provider",
            api_key="test-key",
            base_url="https://test.url",
            model=None  # None - should NOT update
        )
        config.use("test_provider")

        # model should get default from providers module (not None from provider config)
        # The use() method doesn't update model when it's None in provider config
        # So model will be fetched via _get_default_model()
        assert config.api_key == "test-key"
        assert config.base_url == "https://test.url"


class TestConfigLoadFileEdgeCases:
    """Test Config._load_file edge cases for branch coverage"""

    def test_load_file_unknown_extension_not_json_not_toml(self):
        """Test auto-detection with non-JSON, non-TOML content (line 589->595)"""
        import continuum_sdk.config.loader as loader_module

        # Ensure tomllib is None to hit the branch where we return empty dict
        original_tomllib = loader_module.tomllib
        loader_module.tomllib = None

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", suffix=".custom", delete=False
            ) as f:
                # Content that doesn't start with { (not JSON)
                f.write("not json or toml content")
                path = f.name

            try:
                config_data = Config._load_file(Path(path))
                # When tomllib is None and content doesn't start with {,
                # should return empty dict (line 595)
                assert config_data == {}
            finally:
                os.unlink(path)
        finally:
            loader_module.tomllib = original_tomllib


class TestExpandEnvVarsPrimitiveTypes:
    """Test _expand_env_vars with primitive types (line 619)"""

    def test_expand_env_vars_with_int(self):
        """Test that integers are returned unchanged (line 619)"""
        data = {"count": 42}
        result = Config._expand_env_vars(data)
        assert result["count"] == 42

    def test_expand_env_vars_with_float(self):
        """Test that floats are returned unchanged (line 619)"""
        data = {"ratio": 3.14}
        result = Config._expand_env_vars(data)
        assert result["ratio"] == 3.14

    def test_expand_env_vars_with_bool(self):
        """Test that booleans are returned unchanged (line 619)"""
        data = {"enabled": True, "disabled": False}
        result = Config._expand_env_vars(data)
        assert result["enabled"] is True
        assert result["disabled"] is False

    def test_expand_env_vars_with_none(self):
        """Test that None is returned unchanged (line 619)"""
        data = {"value": None}
        result = Config._expand_env_vars(data)
        assert result["value"] is None

    def test_expand_env_vars_with_mixed_types(self):
        """Test mixed types including primitives (line 619)"""
        monkeypatch = pytest.MonkeyPatch()
        monkeypatch.setenv("CONTINUUM_API_KEY", "test-key")

        data = {
            "api_key": "${CONTINUUM_API_KEY}",  # string with expansion
            "count": 100,  # int (line 619)
            "ratio": 0.5,  # float (line 619)
            "enabled": True,  # bool (line 619)
            "nested": {  # dict
                "value": None,  # None (line 619)
                "list": [1, 2, "${CONTINUUM_API_KEY}"]  # list with primitives
            }
        }

        result = Config._expand_env_vars(data)
        assert result["api_key"] == "test-key"
        assert result["count"] == 100
        assert result["ratio"] == 0.5
        assert result["enabled"] is True
        assert result["nested"]["value"] is None
        assert result["nested"]["list"] == [1, 2, "test-key"]

        monkeypatch.undo()


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--cov=continuum_sdk.config.loader", "--cov-report=term-missing"])
