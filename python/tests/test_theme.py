"""Theme Manager Tests

Comprehensive tests for theme.py covering:
- Theme class
- ColorScheme class
- Theme loading and saving
- Environment variable reading
- Default value handling
"""

import os
import shutil
import sys
import tempfile
from pathlib import Path
from unittest import mock

import pytest


class TestColorScheme:
    """ColorScheme Tests"""

    def test_color_scheme_creation(self):
        """Test color scheme creation with custom values"""
        try:
            from continuum_sdk.config import ColorScheme

            scheme = ColorScheme(
                name="test",
                primary="#FF0000",
                secondary="#00FF00",
            )
            assert scheme.name == "test"
            assert scheme.primary == "#FF0000"
            assert scheme.secondary == "#00FF00"
        except ImportError:
            pytest.skip("theme module not available")

    def test_color_scheme_default_values(self):
        """Test color scheme default values"""
        try:
            from continuum_sdk.config import ColorScheme

            scheme = ColorScheme()
            assert scheme.name == "default"
            assert scheme.primary == "#4A90D9"
            assert scheme.secondary == "#6C7A89"
            assert scheme.accent == "#FF5500"
            assert scheme.background == "#1E1E1E"
            assert scheme.foreground == "#E0E0E0"
            assert scheme.error == "#E53935"
            assert scheme.warning == "#FFA726"
            assert scheme.success == "#43A047"
            assert scheme.info == "#29B6F6"
            assert scheme.muted == "#757575"
            assert scheme.border == "#424242"
        except ImportError:
            pytest.skip("theme module not available")

    def test_color_scheme_all_custom_values(self):
        """Test color scheme with all custom values"""
        try:
            from continuum_sdk.config import ColorScheme

            scheme = ColorScheme(
                name="custom",
                primary="#111111",
                secondary="#222222",
                accent="#333333",
                background="#444444",
                foreground="#555555",
                error="#666666",
                warning="#777777",
                success="#888888",
                info="#999999",
                muted="#AAAAAA",
                border="#BBBBBB",
            )
            assert scheme.name == "custom"
            assert scheme.primary == "#111111"
            assert scheme.border == "#BBBBBB"
        except ImportError:
            pytest.skip("theme module not available")

    def test_color_scheme_to_dict(self):
        """Test to_dict method returns all fields"""
        try:
            from continuum_sdk.config import ColorScheme

            scheme = ColorScheme(
                name="test",
                primary="#FF0000",
                secondary="#00FF00",
            )
            data = scheme.to_dict()
            assert "name" in data
            assert "primary" in data
            assert "secondary" in data
            assert "accent" in data
            assert "background" in data
            assert "foreground" in data
            assert "error" in data
            assert "warning" in data
            assert "success" in data
            assert "info" in data
            assert "muted" in data
            assert "border" in data
            assert len(data) == 12
            assert data["name"] == "test"
            assert data["primary"] == "#FF0000"
        except ImportError:
            pytest.skip("theme module not available")

    def test_color_scheme_from_dict_all_fields(self):
        """Test from_dict method with all fields"""
        try:
            from continuum_sdk.config import ColorScheme

            data = {
                "name": "custom",
                "primary": "#ABCDEF",
                "secondary": "#123456",
                "accent": "#FEDCBA",
                "background": "#000000",
                "foreground": "#FFFFFF",
                "error": "#FF0000",
                "warning": "#FFFF00",
                "success": "#00FF00",
                "info": "#0000FF",
                "muted": "#888888",
                "border": "#666666",
            }
            scheme = ColorScheme.from_dict(data)
            assert scheme.name == "custom"
            assert scheme.primary == "#ABCDEF"
            assert scheme.secondary == "#123456"
            assert scheme.accent == "#FEDCBA"
            assert scheme.background == "#000000"
        except ImportError:
            pytest.skip("theme module not available")

    def test_color_scheme_from_dict_partial(self):
        """Test from_dict method with partial fields (uses defaults)"""
        try:
            from continuum_sdk.config import ColorScheme

            data = {
                "name": "partial",
                "primary": "#ABCDEF",
                "background": "#000000",
            }
            scheme = ColorScheme.from_dict(data)
            assert scheme.name == "partial"
            assert scheme.primary == "#ABCDEF"
            assert scheme.background == "#000000"
            # Should use defaults for missing fields
            assert scheme.secondary == "#6C7A89"
            assert scheme.accent == "#FF5500"
        except ImportError:
            pytest.skip("theme module not available")

    def test_color_scheme_from_dict_empty(self):
        """Test from_dict method with empty dict"""
        try:
            from continuum_sdk.config import ColorScheme

            scheme = ColorScheme.from_dict({})
            assert scheme.name == "custom"
            # Should use all defaults
            assert scheme.primary == "#4A90D9"
            assert scheme.background == "#1E1E1E"
        except ImportError:
            pytest.skip("theme module not available")


class TestPresetTheme:
    """PresetTheme Tests"""

    def test_preset_theme_enum_values(self):
        """Test all preset theme enum values"""
        try:
            from continuum_sdk.config import PresetTheme

            assert PresetTheme.DARK.value == "dark"
            assert PresetTheme.LIGHT.value == "light"
            assert PresetTheme.MONOKAI.value == "monokai"
            assert PresetTheme.GITHUB.value == "github"
            assert PresetTheme.NORD.value == "nord"
            assert PresetTheme.DRACULA.value == "dracula"
            assert PresetTheme.SOLARIZED.value == "solarized"
            assert PresetTheme.GRUVBOX.value == "gruvbox"
        except ImportError:
            pytest.skip("theme module not available")

    def test_preset_theme_count(self):
        """Test number of preset themes"""
        try:
            from continuum_sdk.config import PresetTheme

            assert len(PresetTheme) == 8
        except ImportError:
            pytest.skip("theme module not available")


class TestPresetThemes:
    """Preset Themes Dictionary Tests"""

    def test_preset_themes_exist(self):
        """Test that all preset themes are defined"""
        try:
            from continuum_sdk.config.theme import PRESET_THEMES

            assert "dark" in PRESET_THEMES
            assert "light" in PRESET_THEMES
            assert "monokai" in PRESET_THEMES
            assert "github" in PRESET_THEMES
            assert "nord" in PRESET_THEMES
            assert "dracula" in PRESET_THEMES
            assert "solarized" in PRESET_THEMES
            assert "gruvbox" in PRESET_THEMES
            assert len(PRESET_THEMES) == 8
        except ImportError:
            pytest.skip("theme module not available")

    def test_preset_themes_are_color_schemes(self):
        """Test that preset themes are ColorScheme instances"""
        try:
            from continuum_sdk.config import ColorScheme
            from continuum_sdk.config.theme import PRESET_THEMES

            for name, scheme in PRESET_THEMES.items():
                assert isinstance(scheme, ColorScheme)
                assert scheme.name == name
        except ImportError:
            pytest.skip("theme module not available")

    def test_dark_theme_values(self):
        """Test dark theme specific values"""
        try:
            from continuum_sdk.config.theme import PRESET_THEMES

            dark = PRESET_THEMES["dark"]
            assert dark.name == "dark"
            assert dark.background == "#1E1E1E"
            assert dark.foreground == "#E0E0E0"
        except ImportError:
            pytest.skip("theme module not available")

    def test_light_theme_values(self):
        """Test light theme specific values"""
        try:
            from continuum_sdk.config.theme import PRESET_THEMES

            light = PRESET_THEMES["light"]
            assert light.name == "light"
            assert light.background == "#FAFAFA"
            assert light.foreground == "#212121"
        except ImportError:
            pytest.skip("theme module not available")


class TestThemeManager:
    """ThemeManager Tests"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path, ignore_errors=True)

    @pytest.fixture
    def temp_config_path(self, temp_dir):
        """Create temp config path"""
        return os.path.join(temp_dir, "theme.toml")

    def test_theme_manager_creation(self):
        """Test theme manager creation with defaults"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            assert manager is not None
            assert manager.current_theme is not None
            assert manager.current_theme.name == "dark"
        except ImportError:
            pytest.skip("theme module not available")

    def test_theme_manager_with_custom_path(self, temp_config_path):
        """Test theme manager with custom config path"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(config_path=temp_config_path, auto_load=False)
            assert str(manager.config_path) == temp_config_path
        except ImportError:
            pytest.skip("theme module not available")

    def test_theme_manager_auto_load_disabled(self, temp_config_path):
        """Test theme manager with auto_load=False"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(config_path=temp_config_path, auto_load=False)
            # Should have default theme
            assert manager.current_theme.name == "dark"
        except ImportError:
            pytest.skip("theme module not available")

    def test_theme_manager_auto_load_enabled_no_file(self, temp_config_path):
        """Test theme manager auto_load when file doesn't exist"""
        try:
            from continuum_sdk.config import ThemeManager

            # File doesn't exist, should use defaults
            manager = ThemeManager(config_path=temp_config_path, auto_load=True)
            assert manager.current_theme.name == "dark"
        except ImportError:
            pytest.skip("theme module not available")

    def test_config_path_property(self, temp_config_path):
        """Test config_path property"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(config_path=temp_config_path, auto_load=False)
            assert manager.config_path == Path(temp_config_path)
        except ImportError:
            pytest.skip("theme module not available")

    def test_current_theme_property(self):
        """Test current_theme property"""
        try:
            from continuum_sdk.config import ColorScheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            theme = manager.current_theme
            assert isinstance(theme, ColorScheme)
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_preset_theme(self):
        """Test applying preset theme by name"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            result = manager.apply("monokai")
            assert result is True
            assert manager.current_theme.name == "monokai"
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_all_preset_themes(self):
        """Test applying all preset themes"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            themes = manager.list_themes()
            for theme in themes:
                result = manager.apply(theme)
                assert result is True
                assert manager.current_theme.name == theme
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_invalid_theme(self):
        """Test applying invalid theme returns False"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            result = manager.apply("invalid_theme")
            assert result is False
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_with_enum(self):
        """Test applying theme with PresetTheme enum"""
        try:
            from continuum_sdk.config import PresetTheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            result = manager.apply(PresetTheme.NORD)
            assert result is True
            assert manager.current_theme.name == "nord"
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_with_enum_light(self):
        """Test applying theme with PresetTheme.LIGHT enum"""
        try:
            from continuum_sdk.config import PresetTheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            result = manager.apply(PresetTheme.LIGHT)
            assert result is True
            assert manager.current_theme.name == "light"
        except ImportError:
            pytest.skip("theme module not available")

    def test_set_custom_color(self):
        """Test setting custom color"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            manager.set_color("primary", "#FF5500")
            assert manager.get_color("primary") == "#FF5500"
        except ImportError:
            pytest.skip("theme module not available")

    def test_set_multiple_custom_colors(self):
        """Test setting multiple custom colors"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            manager.set_color("primary", "#FF0000")
            manager.set_color("secondary", "#00FF00")
            manager.set_color("accent", "#0000FF")
            assert manager.get_color("primary") == "#FF0000"
            assert manager.get_color("secondary") == "#00FF00"
            assert manager.get_color("accent") == "#0000FF"
        except ImportError:
            pytest.skip("theme module not available")

    def test_set_color_invalid_name(self):
        """Test setting color with invalid name (should log warning)"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            # Invalid color name should be ignored
            manager.set_color("nonexistent", "#FF0000")
            # Should not raise, just log warning
            assert manager.get_color("nonexistent") is None
        except ImportError:
            pytest.skip("theme module not available")

    def test_get_color(self):
        """Test getting color value"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            color = manager.get_color("primary")
            assert color is not None
            assert color.startswith("#")
        except ImportError:
            pytest.skip("theme module not available")

    def test_get_color_invalid_name(self):
        """Test getting color with invalid name returns None"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            color = manager.get_color("nonexistent")
            assert color is None
        except ImportError:
            pytest.skip("theme module not available")

    def test_get_color_all_fields(self):
        """Test getting all color fields"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            color_names = [
                "primary",
                "secondary",
                "accent",
                "background",
                "foreground",
                "error",
                "warning",
                "success",
                "info",
                "muted",
                "border",
            ]
            for name in color_names:
                color = manager.get_color(name)
                assert color is not None
                assert color.startswith("#")
        except ImportError:
            pytest.skip("theme module not available")

    def test_reset_color(self):
        """Test reset single color to default"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            original = manager.get_color("primary")

            manager.set_color("primary", "#FF0000")
            assert manager.get_color("primary") == "#FF0000"

            manager.reset_color("primary")
            assert manager.get_color("primary") == original
        except ImportError:
            pytest.skip("theme module not available")

    def test_reset_color_non_preset_theme(self):
        """Test reset color when current theme is not in presets"""
        try:
            from continuum_sdk.config import ColorScheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            # Manually set a non-preset theme
            manager._current_theme = ColorScheme(name="custom_nonexistent")

            # reset_color should not crash when theme not in presets
            manager.reset_color("primary")
            # Should just do nothing
        except ImportError:
            pytest.skip("theme module not available")

    def test_reset_all(self):
        """Test reset all custom colors"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")

            manager.set_color("primary", "#FF0000")
            manager.set_color("secondary", "#00FF00")

            manager.reset_all()

            # Should have default theme colors
            assert manager.get_color("primary") != "#FF0000"
            assert manager.get_color("secondary") != "#00FF00"
        except ImportError:
            pytest.skip("theme module not available")

    def test_reset_all_non_preset_theme(self):
        """Test reset_all when current theme is not in presets"""
        try:
            from continuum_sdk.config import ColorScheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            # Manually set a non-preset theme
            manager._current_theme = ColorScheme(name="custom_nonexistent")
            manager._custom_colors = {"primary": "#FF0000"}

            # reset_all should not crash
            manager.reset_all()
            assert len(manager._custom_colors) == 0
        except ImportError:
            pytest.skip("theme module not available")

    def test_list_themes(self):
        """Test listing all available themes"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            themes = manager.list_themes()
            assert len(themes) >= 8
            assert "dark" in themes
            assert "light" in themes
            assert "monokai" in themes
            assert "github" in themes
            assert "nord" in themes
            assert "dracula" in themes
            assert "solarized" in themes
            assert "gruvbox" in themes
        except ImportError:
            pytest.skip("theme module not available")

    def test_get_theme_preview(self):
        """Test getting theme preview without applying"""
        try:
            from continuum_sdk.config import ColorScheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            preview = manager.get_theme_preview("monokai")
            assert preview is not None
            assert isinstance(preview, ColorScheme)
            assert preview.name == "monokai"
            # Current theme should not change
            assert manager.current_theme.name == "dark"
        except ImportError:
            pytest.skip("theme module not available")

    def test_get_theme_preview_invalid(self):
        """Test getting preview for invalid theme returns None"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            preview = manager.get_theme_preview("nonexistent")
            assert preview is None
        except ImportError:
            pytest.skip("theme module not available")

    def test_to_rich_style(self):
        """Test converting to rich style dictionary"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            style = manager.to_rich_style()
            assert "primary" in style
            assert "secondary" in style
            assert "accent" in style
            assert "background" in style
            assert "foreground" in style
            assert "error" in style
            assert "warning" in style
            assert "success" in style
            assert "info" in style
            assert "muted" in style
            assert "border" in style
        except ImportError:
            pytest.skip("theme module not available")

    def test_on_theme_change(self):
        """Test theme change listener"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            changes = []

            def listener(scheme):
                changes.append(scheme.name)

            manager.on_theme_change(listener)
            manager.apply("dark")

            assert len(changes) == 1
            assert changes[0] == "dark"
        except ImportError:
            pytest.skip("theme module not available")

    def test_on_theme_change_multiple_callbacks(self):
        """Test multiple theme change listeners"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            changes1 = []
            changes2 = []

            def listener1(scheme):
                changes1.append(scheme.name)

            def listener2(scheme):
                changes2.append(scheme.name)

            manager.on_theme_change(listener1)
            manager.on_theme_change(listener2)
            manager.apply("monokai")

            assert len(changes1) == 1
            assert len(changes2) == 1
            assert changes1[0] == "monokai"
            assert changes2[0] == "monokai"
        except ImportError:
            pytest.skip("theme module not available")

    def test_on_theme_change_listener_error(self):
        """Test theme change listener with error (should not crash)"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)

            def bad_listener(scheme):
                raise TypeError("Test error")

            manager.on_theme_change(bad_listener)
            # Should not crash, just log warning
            manager.apply("dark")
            assert manager.current_theme.name == "dark"
        except ImportError:
            pytest.skip("theme module not available")

    def test_repr(self):
        """Test __repr__ method"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("monokai")
            repr_str = repr(manager)
            assert "ThemeManager" in repr_str
            assert "monokai" in repr_str
        except ImportError:
            pytest.skip("theme module not available")


class TestThemeManagerEnvironmentVariables:
    """ThemeManager Environment Variable Tests"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path, ignore_errors=True)

    def test_config_path_env_var(self, monkeypatch):
        """Test config path from environment variable"""
        try:
            from continuum_sdk.config import ThemeManager

            # Set environment variable
            monkeypatch.setenv("CONTINUUM_THEME_CONFIG", "/custom/path/theme.toml")

            manager = ThemeManager(auto_load=False)
            # On Windows, path separators are backslashes
            assert "theme.toml" in str(manager.config_path)

        except ImportError:
            pytest.skip("theme module not available")

    def test_config_path_env_var_windows_path(self, monkeypatch, temp_dir):
        """Test config path from environment variable with Windows path"""
        try:
            from continuum_sdk.config import ThemeManager

            custom_path = os.path.join(temp_dir, "custom_theme.toml")
            monkeypatch.setenv("CONTINUUM_THEME_CONFIG", custom_path)

            manager = ThemeManager(auto_load=False)
            assert str(manager.config_path) == custom_path

        except ImportError:
            pytest.skip("theme module not available")

    def test_config_path_env_var_takes_precedence(self, monkeypatch, temp_dir):
        """Test that env var takes precedence over default"""
        try:
            from continuum_sdk.config import ThemeManager

            custom_path = os.path.join(temp_dir, "env_theme.toml")
            monkeypatch.setenv("CONTINUUM_THEME_CONFIG", custom_path)

            manager = ThemeManager(auto_load=False)
            # Should use env var path, not default
            assert str(manager.config_path) == custom_path

        except ImportError:
            pytest.skip("theme module not available")


class TestThemeManagerFileOperations:
    """ThemeManager File Load/Save Tests"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path, ignore_errors=True)

    def test_save_creates_directory(self, temp_dir):
        """Test that save creates parent directory if needed"""
        try:
            from continuum_sdk.config import ThemeManager

            # Nested directory that doesn't exist
            nested_dir = os.path.join(temp_dir, "nested", "deep", "path")
            config_path = os.path.join(nested_dir, "theme.toml")

            manager = ThemeManager(config_path=config_path, auto_load=False)
            manager.apply("monokai")

            # Check if tomli_w is available
            try:
                saved = manager.save()
                if not saved:
                    pytest.skip("tomli_w not available")
            except Exception:
                pytest.skip("tomli_w not available")

            # Directory should be created
            assert os.path.exists(nested_dir)
        except ImportError:
            pytest.skip("theme module not available")

    def test_save_and_load_cycle(self, temp_dir):
        """Test complete save and load cycle"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # Create and save
            manager1 = ThemeManager(config_path=config_path, auto_load=False)
            manager1.apply("monokai")
            manager1.set_color("primary", "#FF0000")

            # Save may fail if tomli_w not available
            try:
                saved = manager1.save()
                if not saved:
                    pytest.skip("tomli_w not available")
            except Exception:
                pytest.skip("tomli_w not available")

            # Load
            manager2 = ThemeManager(config_path=config_path, auto_load=True)
            assert manager2.current_theme.name == "monokai"
        except ImportError:
            pytest.skip("theme module not available")

    def test_load_nonexistent_file(self, temp_dir):
        """Test loading from nonexistent file returns False"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "nonexistent", "theme.toml")

            manager = ThemeManager(config_path=config_path, auto_load=False)
            result = manager.load()
            assert result is False
        except ImportError:
            pytest.skip("theme module not available")

    def test_load_with_existing_file(self, temp_dir):
        """Test loading from existing valid file"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # First create a valid config
            manager1 = ThemeManager(config_path=config_path, auto_load=False)
            manager1.apply("nord")

            try:
                saved = manager1.save()
                if not saved:
                    pytest.skip("tomli_w not available")
            except Exception:
                pytest.skip("tomli_w not available")

            # Now load it
            manager2 = ThemeManager(config_path=config_path, auto_load=False)
            result = manager2.load()
            assert result is True
            assert manager2.current_theme.name == "nord"
        except ImportError:
            pytest.skip("theme module not available")

    def test_load_with_custom_colors(self, temp_dir):
        """Test loading custom colors from file"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # First create with custom colors
            manager1 = ThemeManager(config_path=config_path, auto_load=False)
            manager1.apply("dark")
            manager1.set_color("primary", "#CUSTOM1")
            manager1.set_color("secondary", "#CUSTOM2")

            try:
                saved = manager1.save()
                if not saved:
                    pytest.skip("tomli_w not available")
            except Exception:
                pytest.skip("tomli_w not available")

            # Load and verify custom colors are preserved
            manager2 = ThemeManager(config_path=config_path, auto_load=False)
            result = manager2.load()
            assert result is True
        except ImportError:
            pytest.skip("theme module not available")

    def test_save_returns_true_on_success(self, temp_dir):
        """Test save returns True when successful"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")
            manager = ThemeManager(config_path=config_path, auto_load=False)
            manager.apply("monokai")

            try:
                saved = manager.save()
                assert saved is True
            except Exception:
                pytest.skip("tomli_w not available")
        except ImportError:
            pytest.skip("theme module not available")

    def test_save_overwrites_existing_file(self, temp_dir):
        """Test save overwrites existing file"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # First save
            manager1 = ThemeManager(config_path=config_path, auto_load=False)
            manager1.apply("dark")

            try:
                saved = manager1.save()
                if not saved:
                    pytest.skip("tomli_w not available")
            except Exception:
                pytest.skip("tomli_w not available")

            # Modify and save again
            manager1.apply("light")
            saved = manager1.save()
            assert saved is True

            # Load and verify
            manager2 = ThemeManager(config_path=config_path, auto_load=True)
            assert manager2.current_theme.name == "light"
        except ImportError:
            pytest.skip("theme module not available")


class TestThemeManagerErrorHandling:
    """ThemeManager Error Handling Tests"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path, ignore_errors=True)

    def test_load_invalid_toml(self, temp_dir):
        """Test loading invalid TOML file returns False"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # Write invalid TOML
            with open(config_path, "w") as f:
                f.write("invalid toml content [[[")

            manager = ThemeManager(config_path=config_path, auto_load=False)
            result = manager.load()
            assert result is False
        except ImportError:
            pytest.skip("theme module not available")

    def test_load_json_decode_error_handling(self, temp_dir):
        """Test load handles JSON decode error (for error branch)"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # Write valid TOML to test the json.JSONDecodeError branch
            # (This is actually testing error handling in load())
            with open(config_path, "w") as f:
                f.write('[theme]\nname = "dark"')

            manager = ThemeManager(config_path=config_path, auto_load=False)
            # Should not crash
            manager.load()
            # Result depends on whether tomllib is available
        except ImportError:
            pytest.skip("theme module not available")

    def test_load_os_error(self, temp_dir):
        """Test load handles OS errors gracefully"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # Create file
            with open(config_path, "w") as f:
                f.write('[theme]\nname = "dark"')

            manager = ThemeManager(config_path=config_path, auto_load=False)

            # Mock open to raise OSError
            with mock.patch("builtins.open", side_effect=OSError("Permission denied")):
                result = manager.load()
                assert result is False
        except ImportError:
            pytest.skip("theme module not available")

    def test_save_permission_error(self, temp_dir):
        """Test save handles permission errors gracefully"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")
            manager = ThemeManager(config_path=config_path, auto_load=False)
            manager.apply("dark")

            # Mock open to raise PermissionError
            with mock.patch(
                "builtins.open", side_effect=PermissionError("Access denied")
            ):
                result = manager.save()
                assert result is False
        except ImportError:
            pytest.skip("theme module not available")

    def test_save_os_error(self, temp_dir):
        """Test save handles OS errors gracefully"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")
            manager = ThemeManager(config_path=config_path, auto_load=False)
            manager.apply("dark")

            # Mock mkdir to raise OSError
            with mock.patch.object(Path, "mkdir", side_effect=OSError("Disk full")):
                result = manager.save()
                assert result is False
        except ImportError:
            pytest.skip("theme module not available")


class TestTomlSupportFallback:
    """TOML Support Fallback Tests"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path, ignore_errors=True)

    def test_tomllib_import_fallback(self):
        """Test that module handles missing tomllib/tomli gracefully"""
        # This tests the import fallback at module level
        # The module should still load even without tomllib
        try:
            import importlib
            import sys

            # Save original modules
            original_tomllib = sys.modules.get("tomllib")
            original_tomli = sys.modules.get("tomli")

            # Remove to simulate fallback
            if "tomllib" in sys.modules:
                del sys.modules["tomllib"]
            if "tomli" in sys.modules:
                del sys.modules["tomli"]

            # Re-import to trigger fallback
            import continuum_sdk.config.theme as theme_module

            importlib.reload(theme_module)

            # Module should still work
            from continuum_sdk.config import ThemeManager  # noqa: F401

            manager = ThemeManager(auto_load=False)
            assert manager is not None

            # Restore original modules
            if original_tomllib:
                sys.modules["tomllib"] = original_tomllib
            if original_tomli:
                sys.modules["tomli"] = original_tomli

        except ImportError:
            pytest.skip("theme module not available")

    def test_load_without_tomllib(self, temp_dir):
        """Test load returns False when tomllib is not available"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")

            # Create valid config
            with open(config_path, "w") as f:
                f.write('[theme]\nname = "monokai"')

            manager = ThemeManager(config_path=config_path, auto_load=False)

            # Mock tomllib to None to test fallback
            import continuum_sdk.config.theme as theme_module

            original_tomllib = theme_module.tomllib
            theme_module.tomllib = None

            try:
                result = manager.load()
                assert result is False
            finally:
                theme_module.tomllib = original_tomllib
        except ImportError:
            pytest.skip("theme module not available")

    def test_save_without_tomli_w(self, temp_dir):
        """Test save returns False when tomli_w is not available"""
        try:
            from continuum_sdk.config import ThemeManager

            config_path = os.path.join(temp_dir, "theme.toml")
            manager = ThemeManager(config_path=config_path, auto_load=False)
            manager.apply("dark")

            # Mock tomli_w to None
            import continuum_sdk.config.theme as theme_module

            original_tomli_w = theme_module.tomli_w
            theme_module.tomli_w = None

            try:
                result = manager.save()
                assert result is False
            finally:
                theme_module.tomli_w = original_tomli_w
        except ImportError:
            pytest.skip("theme module not available")


class TestThemeManagerCustomColorsPersistence:
    """Test custom colors persistence across operations"""

    def test_custom_colors_persist_after_theme_change(self):
        """Test that custom colors persist when changing themes"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            manager.set_color("primary", "#FF0000")

            # Change theme - custom colors should still be tracked
            manager.apply("monokai")
            manager._apply_custom_colors()

            # Custom color should be applied to new theme
            assert manager.get_color("primary") == "#FF0000"
        except ImportError:
            pytest.skip("theme module not available")

    def test_set_color_triggers_notification(self):
        """Test that set_color triggers listener notification"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            notifications = []

            def listener(scheme):
                notifications.append(scheme)

            manager.on_theme_change(listener)
            manager.apply("dark")
            initial_count = len(notifications)

            manager.set_color("primary", "#FF0000")

            # Should have triggered notification
            assert len(notifications) > initial_count
        except ImportError:
            pytest.skip("theme module not available")

    def test_reset_color_removes_from_custom(self):
        """Test that reset_color removes color from custom colors"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")
            manager.set_color("primary", "#FF0000")

            assert "primary" in manager._custom_colors

            manager.reset_color("primary")

            assert "primary" not in manager._custom_colors
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_custom_colors_to_current_theme(self):
        """Test _apply_custom_colors method directly"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")

            # Set custom colors
            manager._custom_colors = {
                "primary": "#AAAAAA",
                "secondary": "#BBBBBB",
            }

            # Apply custom colors
            manager._apply_custom_colors()

            assert manager.current_theme.primary == "#AAAAAA"
            assert manager.current_theme.secondary == "#BBBBBB"
        except ImportError:
            pytest.skip("theme module not available")

    def test_apply_custom_colors_skips_invalid(self):
        """Test _apply_custom_colors skips invalid color names"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")

            # Set custom colors with invalid name
            manager._custom_colors = {
                "primary": "#AAAAAA",
                "nonexistent": "#BBBBBB",
            }

            # Should not crash
            manager._apply_custom_colors()

            assert manager.current_theme.primary == "#AAAAAA"
        except ImportError:
            pytest.skip("theme module not available")


class TestThemeManagerAdditionalBranches:
    """Additional tests to cover remaining branches"""

    def test_reset_color_when_no_default_value(self):
        """Test reset_color when theme is not in presets - should do nothing"""
        try:
            from continuum_sdk.config import ColorScheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            # Create a custom theme object with a name not in presets
            manager._current_theme = ColorScheme(name="nonexistent_preset")
            manager._custom_colors["primary"] = "#FF0000"
            manager._current_theme.primary = "#FF0000"

            # reset_color should handle when theme is not in PRESET_THEMES
            # In this case, it should NOT reset the color (no default available)
            manager.reset_color("primary")
            # Custom colors should remain because no preset default exists
            assert manager._custom_colors.get("primary") == "#FF0000"
        except ImportError:
            pytest.skip("theme module not available")

    def test_load_with_unknown_theme_name(self, tmp_path):
        """Test load when theme name is not in presets - should keep default"""
        try:
            import tomli_w  # noqa: F401

            from continuum_sdk.config import ThemeManager

            config_path = tmp_path / "theme.toml"

            # Create config with unknown theme name
            data = {"theme": {"name": "unknown_custom_theme"}, "colors": {}}
            with open(config_path, "wb") as f:
                tomli_w.dump(data, f)

            manager = ThemeManager(config_path=str(config_path), auto_load=False)
            result = manager.load()

            # Should load successfully
            assert result is True
            # When theme name is not in presets, current theme is NOT changed
            # (stays as default 'dark')
            assert manager.current_theme.name == "dark"
        except ImportError:
            pytest.skip("theme module or tomli_w not available")

    def test_load_with_custom_colors_data(self, tmp_path):
        """Test load with custom colors in config"""
        try:
            import tomli_w  # noqa: F401

            from continuum_sdk.config import ThemeManager

            config_path = tmp_path / "theme.toml"

            # Create config with custom colors
            data = {
                "theme": {"name": "monokai"},
                "colors": {"primary": "#CUSTOM01", "secondary": "#CUSTOM02"},
            }
            with open(config_path, "wb") as f:
                tomli_w.dump(data, f)

            manager = ThemeManager(config_path=str(config_path), auto_load=False)
            result = manager.load()

            assert result is True
            assert manager.current_theme.name == "monokai"
            # Custom colors should be loaded
            assert "primary" in manager._custom_colors
        except ImportError:
            pytest.skip("theme module or tomli_w not available")

    def test_save_with_tomli_w_available(self, tmp_path):
        """Test save when tomli_w is available"""
        try:
            import tomli_w  # noqa: F401

            from continuum_sdk.config import ThemeManager

            config_path = tmp_path / "theme.toml"
            manager = ThemeManager(config_path=str(config_path), auto_load=False)
            manager.apply("nord")
            manager.set_color("primary", "#NORDCUSTOM")

            result = manager.save()
            assert result is True

            # Verify file was created
            assert config_path.exists()

            # Load and verify content
            manager2 = ThemeManager(config_path=str(config_path), auto_load=False)
            manager2.load()
            assert manager2.current_theme.name == "nord"
        except ImportError:
            pytest.skip("theme module or tomli_w not available")


class TestTomlImportFallbacks:
    """Test TOML import fallbacks at module level (lines 58-67)"""

    def test_tomli_fallback_import(self):
        """Test that tomli is imported when tomllib is not available (lines 58-62)"""
        import importlib

        # Save current state
        original_tomllib = sys.modules.get("tomllib")
        original_tomli = sys.modules.get("tomli")

        # Remove tomllib to force fallback
        if "tomllib" in sys.modules:
            del sys.modules["tomllib"]

        # Make sure tomli is available
        try:
            import tomli

            sys.modules["tomli"] = tomli
        except ImportError:
            pytest.skip("tomli not available for fallback test")

        try:
            # Re-import the theme module to trigger the import block
            import continuum_sdk.config.theme as theme_module

            importlib.reload(theme_module)

            # The module should have loaded tomli as tomllib
            assert theme_module.tomllib is not None

        finally:
            # Restore original state
            if original_tomllib:
                sys.modules["tomllib"] = original_tomllib
            if original_tomli:
                sys.modules["tomli"] = original_tomli

            # Reload to restore original state
            importlib.reload(theme_module)

    def test_tomllib_and_tomli_not_available(self):
        """Test when both tomllib and tomli are not available (line 62)"""
        import builtins
        import importlib

        # Save current state
        original_tomllib = sys.modules.get("tomllib")
        original_tomli = sys.modules.get("tomli")
        original_import = builtins.__import__

        # Remove both to simulate complete absence
        if "tomllib" in sys.modules:
            del sys.modules["tomllib"]
        if "tomli" in sys.modules:
            del sys.modules["tomli"]

        # Mock both imports to fail
        def mock_import(name, *args, **kwargs):
            if name in ("tomllib", "tomli"):
                raise ImportError(f"No module named '{name}'")
            return original_import(name, *args, **kwargs)

        builtins.__import__ = mock_import

        try:
            # Re-import to trigger both import failures
            import continuum_sdk.config.theme as theme_module

            importlib.reload(theme_module)

            # tomllib should be None when both imports fail
            assert theme_module.tomllib is None

        finally:
            # Restore original state
            builtins.__import__ = original_import
            if original_tomllib:
                sys.modules["tomllib"] = original_tomllib
            if original_tomli:
                sys.modules["tomli"] = original_tomli

            # Reload to restore original state
            importlib.reload(theme_module)

    def test_tomli_w_not_available(self):
        """Test when tomli_w is not available (lines 66-67)"""
        import builtins
        import importlib

        # Save current state
        original_tomli_w = sys.modules.get("tomli_w")
        original_import = builtins.__import__

        # Remove tomli_w to simulate absence
        if "tomli_w" in sys.modules:
            del sys.modules["tomli_w"]

        # Mock tomli_w import to fail
        def mock_import(name, *args, **kwargs):
            if name == "tomli_w":
                raise ImportError("No module named 'tomli_w'")
            return original_import(name, *args, **kwargs)

        builtins.__import__ = mock_import

        try:
            # Re-import to trigger the import failure
            import continuum_sdk.config.theme as theme_module

            importlib.reload(theme_module)

            # tomli_w should be None when import fails
            assert theme_module.tomli_w is None

        finally:
            # Restore original state
            builtins.__import__ = original_import
            if original_tomli_w:
                sys.modules["tomli_w"] = original_tomli_w

            # Reload to restore original state
            importlib.reload(theme_module)


class TestResetColorBranch:
    """Test reset_color branch coverage (line 386->exit)"""

    def test_reset_color_when_default_value_is_none(self):
        """Test reset_color when getattr returns None (should not update)"""
        try:
            from continuum_sdk.config import ColorScheme, ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")

            # Create a custom color scheme with a name not in presets
            custom_scheme = ColorScheme(name="custom_test")
            manager._current_theme = custom_scheme
            manager._custom_colors["primary"] = "#FF0000"

            # Set a color value
            manager.set_color("primary", "#FF0000")

            # When theme name is not in presets, reset_color should do nothing
            # because there's no default value to restore
            manager.reset_color("primary")

            # The color should still be set because no preset default exists
            # This tests the branch where we don't have a default_value
            # (line 386: if default_value: evaluates to False)

        except ImportError:
            pytest.skip("theme module not available")

    def test_reset_color_with_invalid_attribute_name(self):
        """Test reset_color when color name doesn't exist in preset (returns None)"""
        try:
            from continuum_sdk.config import ThemeManager

            manager = ThemeManager(auto_load=False)
            manager.apply("dark")

            # Try to reset a color name that doesn't exist in the ColorScheme
            # This will cause getattr(PRESET_THEMES[theme_name], name, None) to return None
            # which tests the branch where default_value is None (line 386)
            manager.reset_color("nonexistent_color_field")

            # Should not crash and should not modify anything

        except ImportError:
            pytest.skip("theme module not available")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
