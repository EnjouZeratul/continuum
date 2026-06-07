"""Theme Manager - Theme System Configuration.

Manages TUI color themes and style configurations.

Features:
    - Preset themes (dark, light, monokai, etc.)
    - Custom color configuration
    - Runtime dynamic switching
    - TOML config persistence
    - Environment variable support

Preset Themes:
    - dark: Dark theme (default)
    - light: Light theme
    - monokai: Monokai style
    - github: GitHub style
    - nord: Nord style
    - dracula: Dracula style

Quick Start:
    >>> from continuum_sdk.config import ThemeManager
    >>>
    >>> # Use default theme
    >>> manager = ThemeManager()
    >>> manager.apply("monokai")
    >>>
    >>> # Custom colors
    >>> manager.set_color("primary", "#FF5500")
    >>> manager.save()
    >>>
    >>> # Switch theme
    >>> manager.apply("light")

Config File (~/.continuum/theme.toml):
    [theme]
    name = "monokai"

    [colors]
    primary = "#FF5500"
    secondary = "#00AAFF"
    background = "#1E1E1E"
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass, replace
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

# TOML support
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None

try:
    import tomli_w
except ImportError:
    tomli_w = None


class PresetTheme(Enum):
    """Preset theme."""

    DARK = "dark"
    LIGHT = "light"
    MONOKAI = "monokai"
    GITHUB = "github"
    NORD = "nord"
    DRACULA = "dracula"
    SOLARIZED = "solarized"
    GRUVBOX = "gruvbox"


@dataclass
class ColorScheme:
    """
    Color scheme.

    Attributes:
        name: Scheme name.
        primary: Primary color.
        secondary: Secondary color.
        accent: Accent color.
        background: Background color.
        foreground: Foreground color (text).
        error: Error color.
        warning: Warning color.
        success: Success color.
        info: Info color.
        muted: Muted color.
        border: Border color.
    """

    name: str = "default"
    primary: str = "#4A90D9"
    secondary: str = "#6C7A89"
    accent: str = "#FF5500"
    background: str = "#1E1E1E"
    foreground: str = "#E0E0E0"
    error: str = "#E53935"
    warning: str = "#FFA726"
    success: str = "#43A047"
    info: str = "#29B6F6"
    muted: str = "#757575"
    border: str = "#424242"

    def to_dict(self) -> dict[str, str]:
        return {
            "name": self.name,
            "primary": self.primary,
            "secondary": self.secondary,
            "accent": self.accent,
            "background": self.background,
            "foreground": self.foreground,
            "error": self.error,
            "warning": self.warning,
            "success": self.success,
            "info": self.info,
            "muted": self.muted,
            "border": self.border,
        }

    @classmethod
    def from_dict(cls, data: dict[str, str]) -> ColorScheme:
        return cls(
            name=data.get("name", "custom"),
            primary=data.get("primary", "#4A90D9"),
            secondary=data.get("secondary", "#6C7A89"),
            accent=data.get("accent", "#FF5500"),
            background=data.get("background", "#1E1E1E"),
            foreground=data.get("foreground", "#E0E0E0"),
            error=data.get("error", "#E53935"),
            warning=data.get("warning", "#FFA726"),
            success=data.get("success", "#43A047"),
            info=data.get("info", "#29B6F6"),
            muted=data.get("muted", "#757575"),
            border=data.get("border", "#424242"),
        )


# Preset color schemes
PRESET_THEMES: dict[str, ColorScheme] = {
    "dark": ColorScheme(
        name="dark",
        primary="#4A90D9",
        secondary="#6C7A89",
        accent="#FF5500",
        background="#1E1E1E",
        foreground="#E0E0E0",
        error="#E53935",
        warning="#FFA726",
        success="#43A047",
        info="#29B6F6",
        muted="#757575",
        border="#424242",
    ),
    "light": ColorScheme(
        name="light",
        primary="#1976D2",
        secondary="#546E7A",
        accent="#E65100",
        background="#FAFAFA",
        foreground="#212121",
        error="#D32F2F",
        warning="#F57C00",
        success="#388E3C",
        info="#0288D1",
        muted="#9E9E9E",
        border="#E0E0E0",
    ),
    "monokai": ColorScheme(
        name="monokai",
        primary="#66D9EF",
        secondary="#75715E",
        accent="#FF5500",
        background="#272822",
        foreground="#F8F8F2",
        error="#F92672",
        warning="#FD971F",
        success="#A6E22E",
        info="#66D9EF",
        muted="#75715E",
        border="#49483E",
    ),
    "github": ColorScheme(
        name="github",
        primary="#0366D6",
        secondary="#586069",
        accent="#D73A49",
        background="#FFFFFF",
        foreground="#24292E",
        error="#CB2431",
        warning="#F9A825",
        success="#28A745",
        info="#0366D6",
        muted="#6A737D",
        border="#E1E4E8",
    ),
    "nord": ColorScheme(
        name="nord",
        primary="#88C0D0",
        secondary="#81A1C1",
        accent="#BF616A",
        background="#2E3440",
        foreground="#ECEFF4",
        error="#BF616A",
        warning="#EBCB8B",
        success="#A3BE8C",
        info="#88C0D0",
        muted="#D8DEE9",
        border="#4C566A",
    ),
    "dracula": ColorScheme(
        name="dracula",
        primary="#BD93F9",
        secondary="#6272A4",
        accent="#FF79C6",
        background="#282A36",
        foreground="#F8F8F2",
        error="#FF5555",
        warning="#F1FA8C",
        success="#50FA7B",
        info="#8BE9FD",
        muted="#6272A4",
        border="#44475A",
    ),
    "solarized": ColorScheme(
        name="solarized",
        primary="#268BD2",
        secondary="#839496",
        accent="#CB4B16",
        background="#002B36",
        foreground="#839496",
        error="#DC322F",
        warning="#B58900",
        success="#859900",
        info="#2AA198",
        muted="#586E75",
        border="#073642",
    ),
    "gruvbox": ColorScheme(
        name="gruvbox",
        primary="#83A598",
        secondary="#928374",
        accent="#FE8019",
        background="#282828",
        foreground="#EBDBB2",
        error="#FB4934",
        warning="#FABD2F",
        success="#B8BB26",
        info="#83A598",
        muted="#928374",
        border="#3C3836",
    ),
}


class ThemeManager:
    """
    Theme Manager.

    Example:
        >>> manager = ThemeManager()
        >>> manager.apply("monokai")
        >>> manager.set_color("primary", "#FF0000")
        >>> manager.save()
    """

    DEFAULT_CONFIG_NAME = "theme.toml"
    DEFAULT_CONFIG_DIR = ".continuum"

    def __init__(
        self,
        config_path: str | Path | None = None,
        auto_load: bool = True,
    ):
        """
        Initialize theme manager.

        Args:
            config_path: Config file path (default ~/.continuum/theme.toml)
            auto_load: Whether to auto-load config
        """
        self._config_path = self._resolve_config_path(config_path)
        self._current_theme: ColorScheme = replace(PRESET_THEMES["dark"])
        self._custom_colors: dict[str, str] = {}
        self._listeners: list[callable] = []

        if auto_load:
            self.load()

    def _resolve_config_path(self, config_path: str | Path | None) -> Path:
        """Resolve config file path."""
        if config_path:
            return Path(config_path)

        # Check environment variable
        env_path = os.environ.get("CONTINUUM_THEME_CONFIG")
        if env_path:
            return Path(env_path)

        # Default path
        return Path.home() / self.DEFAULT_CONFIG_DIR / self.DEFAULT_CONFIG_NAME

    @property
    def config_path(self) -> Path:
        """Get config file path."""
        return self._config_path

    @property
    def current_theme(self) -> ColorScheme:
        """Get current color scheme."""
        return self._current_theme

    def apply(self, theme: str | PresetTheme) -> bool:
        """
        Apply preset theme.

        Args:
            theme: Theme name or enum value

        Returns:
            Whether successfully applied
        """
        if isinstance(theme, PresetTheme):
            theme = theme.value

        if theme not in PRESET_THEMES:
            logger.warning(f"Unknown theme: {theme}")
            return False

        self._current_theme = replace(PRESET_THEMES[theme])
        self._apply_custom_colors()
        self._notify_listeners()
        logger.info(f"Applied theme: {theme}")
        return True

    def set_color(self, name: str, color: str) -> None:
        """
        Set custom color.

        Args:
            name: Color name (e.g., primary, secondary)
            color: Color value (e.g., #FF5500 or red)
        """
        if not hasattr(self._current_theme, name):
            logger.warning(f"Unknown color name: {name}")
            return

        self._custom_colors[name] = color
        setattr(self._current_theme, name, color)
        self._notify_listeners()

    def get_color(self, name: str) -> str | None:
        """
        Get color value.

        Args:
            name: Color name

        Returns:
            Color value
        """
        return getattr(self._current_theme, name, None)

    def reset_color(self, name: str) -> None:
        """
        Reset color to default value.

        Args:
            name: Color name
        """
        # Get preset theme's default value
        theme_name = self._current_theme.name
        if theme_name in PRESET_THEMES:
            default_value = getattr(PRESET_THEMES[theme_name], name, None)
            if default_value:
                setattr(self._current_theme, name, default_value)
                self._custom_colors.pop(name, None)
                self._notify_listeners()

    def reset_all(self) -> None:
        """Reset all custom colors."""
        self._custom_colors.clear()
        theme_name = self._current_theme.name
        if theme_name in PRESET_THEMES:
            self._current_theme = replace(PRESET_THEMES[theme_name])
        self._notify_listeners()

    def _apply_custom_colors(self) -> None:
        """Apply custom colors to current theme."""
        for name, color in self._custom_colors.items():
            if hasattr(self._current_theme, name):
                setattr(self._current_theme, name, color)

    def load(self) -> bool:
        """
        Load theme from config file.

        Returns:
            Whether successfully loaded
        """
        if not self._config_path.exists():
            logger.debug(f"Theme config not found: {self._config_path}")
            return False

        if tomllib is None:
            logger.warning("TOML support not available")
            return False

        try:
            with open(self._config_path, "rb") as f:
                data = tomllib.load(f)

            # Load theme name
            theme_name = data.get("theme", {}).get("name", "dark")
            if theme_name in PRESET_THEMES:
                self._current_theme = replace(PRESET_THEMES[theme_name])

            # Load custom colors
            colors = data.get("colors", {})
            for name, color in colors.items():
                self._custom_colors[name] = color

            self._apply_custom_colors()
            logger.info(f"Loaded theme from: {self._config_path}")
            return True

        except Exception as e:
            # Catch OSError, IOError, FileNotFoundError, and TOMLDecodeError
            logger.error(f"Failed to load theme: {e}")
            return False

    def save(self) -> bool:
        """
        Save theme to config file.

        Returns:
            Whether successfully saved
        """
        if tomli_w is None:
            logger.warning("tomli_w not available for writing")
            return False

        try:
            # Ensure directory exists
            self._config_path.parent.mkdir(parents=True, exist_ok=True)

            # Build config data
            data = {
                "theme": {"name": self._current_theme.name},
                "colors": self._custom_colors,
            }

            with open(self._config_path, "wb") as f:
                tomli_w.dump(data, f)

            logger.info(f"Saved theme to: {self._config_path}")
            return True

        except (OSError, PermissionError) as e:
            logger.error(f"Failed to save theme: {e}")
            return False

    def list_themes(self) -> list[str]:
        """
        List all available themes.

        Returns:
            Theme name list
        """
        return list(PRESET_THEMES.keys())

    def get_theme_preview(self, theme: str) -> ColorScheme | None:
        """
        Get theme preview (without applying).

        Args:
            theme: Theme name

        Returns:
            Color scheme
        """
        return PRESET_THEMES.get(theme)

    def on_theme_change(self, callback: callable) -> None:
        """
        Register theme change listener.

        Args:
            callback: Callback function, receives ColorScheme parameter
        """
        self._listeners.append(callback)

    def _notify_listeners(self) -> None:
        """Notify all listeners."""
        for callback in self._listeners:
            try:
                callback(self._current_theme)
            except (TypeError, ValueError, RuntimeError) as e:
                logger.warning(f"Theme listener error: {e}")

    def to_rich_style(self) -> dict[str, Any]:
        """
        Convert to rich style dictionary.

        Returns:
            rich-compatible style dictionary
        """
        return {
            "primary": self._current_theme.primary,
            "secondary": self._current_theme.secondary,
            "accent": self._current_theme.accent,
            "background": self._current_theme.background,
            "foreground": self._current_theme.foreground,
            "error": self._current_theme.error,
            "warning": self._current_theme.warning,
            "success": self._current_theme.success,
            "info": self._current_theme.info,
            "muted": self._current_theme.muted,
            "border": self._current_theme.border,
        }

    def __repr__(self) -> str:
        return f"ThemeManager(theme={self._current_theme.name}, path={self._config_path})"
