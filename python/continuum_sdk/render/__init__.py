"""Markdown Renderer - TUI Terminal Rendering

Uses rich library to render Markdown to terminal.

Features:
    - Header rendering (H1-H6)
    - Paragraphs and lists
    - Code block syntax highlighting
    - Table rendering
    - Link display
    - Block quotes

Quick Start:
    >>> from continuum_sdk.render import MarkdownRenderer
    >>> from rich.console import Console
    >>>
    >>> renderer = MarkdownRenderer()
    >>> console = Console()
    >>>
    >>> markdown = "# Hello\\n\\nThis is **bold** text."
    >>> renderer.render(markdown, console)

Custom Theme:
    >>> renderer = MarkdownRenderer(theme="monokai")
    >>> renderer.render(markdown, console)
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from enum import Enum
from typing import Any

logger = logging.getLogger(__name__)

# Try to import rich
try:
    from rich.console import Console
    from rich.markdown import Markdown
    from rich.style import Style
    from rich.syntax import Syntax
    from rich.table import Table
    from rich.text import Text
    from rich.theme import Theme

    RICH_AVAILABLE = True
except ImportError:
    RICH_AVAILABLE = False
    Console = None
    Markdown = None
    Syntax = None
    Table = None
    Text = None
    Theme = None
    Style = None


class CodeTheme(Enum):
    """Code highlighting theme"""

    MONOKAI = "monokai"
    RICH_DEFAULT = "rich-default"
    GITHUB_DARK = "github-dark"
    ONE_DARK = "one-dark"
    VIM = "vim"
    NATIVE = "native"


@dataclass
class RenderOptions:
    """Render options

    Attributes:
        code_theme: Code highlighting theme
        show_line_numbers: Whether to show line numbers
        word_wrap: Whether to word wrap
        enable_hyperlinks: Whether to enable hyperlinks
        highlight_code: Whether to highlight code
    """

    code_theme: CodeTheme = CodeTheme.MONOKAI
    show_line_numbers: bool = False
    word_wrap: bool = True
    enable_hyperlinks: bool = True
    highlight_code: bool = True


class MarkdownRenderer:
    """Markdown renderer

    Uses rich library to render Markdown to terminal.

    Example:
        >>> renderer = MarkdownRenderer()
        >>> renderer.render("# Hello World", Console())
    """

    def __init__(
        self,
        theme: CodeTheme = CodeTheme.MONOKAI,
        options: RenderOptions | None = None,
    ):
        """Initialize Markdown renderer

        Args:
            theme: Code highlighting theme
            options: Render options
        """
        if not RICH_AVAILABLE:
            raise ImportError(
                "rich library is required for MarkdownRenderer. "
                "Install with: pip install rich"
            )

        self._theme = theme
        self._options = options or RenderOptions(code_theme=theme)

    @property
    def theme(self) -> CodeTheme:
        """Get current theme"""
        return self._theme

    @theme.setter
    def theme(self, value: CodeTheme) -> None:
        """Set theme"""
        self._theme = value
        self._options.code_theme = value

    @property
    def options(self) -> RenderOptions:
        """Get render options"""
        return self._options

    def render(
        self,
        markdown: str,
        console: Console | None = None,
    ) -> None:
        """Render Markdown to console

        Args:
            markdown: Markdown text
            console: Console instance (creates new one if None)
        """
        if console is None:
            console = Console()

        # Use rich's built-in Markdown rendering
        md = Markdown(
            markdown,
            code_theme=self._options.code_theme.value,
        )
        console.print(md)

    def render_to_string(self, markdown: str) -> str:
        """Render Markdown to string

        Args:
            markdown: Markdown text

        Returns:
            Rendered string (with ANSI escape codes)
        """
        console = Console(force_terminal=True)
        with console.capture() as capture:
            self.render(markdown, console)
        return capture.get()

    def render_code_block(
        self,
        code: str,
        language: str,
        console: Console | None = None,
    ) -> None:
        """Render standalone code block

        Args:
            code: Code content
            language: Programming language
            console: Console instance
        """
        if console is None:
            console = Console()

        if self._options.highlight_code:
            syntax = Syntax(
                code,
                language,
                theme=self._options.code_theme.value,
                line_numbers=self._options.show_line_numbers,
                word_wrap=self._options.word_wrap,
            )
            console.print(syntax)
        else:
            console.print(code)

    def render_table(
        self,
        headers: list[str],
        rows: list[list[str]],
        console: Console | None = None,
        title: str | None = None,
    ) -> None:
        """Render table

        Args:
            headers: Table headers
            rows: Data rows
            console: Console instance
            title: Table title
        """
        if console is None:
            console = Console()

        table = Table(title=title, show_header=True, header_style="bold")
        for header in headers:
            table.add_column(header)

        for row in rows:
            table.add_row(*row)

        console.print(table)

    def render_heading(
        self,
        text: str,
        level: int,
        console: Console | None = None,
    ) -> None:
        """Render heading

        Args:
            text: Heading text
            level: Heading level (1-6)
            console: Console instance
        """
        if console is None:
            console = Console()

        styles = {
            1: Style(bold=True, color="bright_red", underline=True),
            2: Style(bold=True, color="bright_green"),
            3: Style(bold=True, color="bright_yellow"),
            4: Style(bold=True, color="bright_blue"),
            5: Style(bold=True, color="bright_magenta"),
            6: Style(bold=True, color="bright_cyan"),
        }

        style = styles.get(level, Style(bold=True))
        heading = Text(text, style=style)
        console.print(heading)

    def render_list(
        self,
        items: list[str],
        ordered: bool = False,
        console: Console | None = None,
    ) -> None:
        """Render list

        Args:
            items: List items
            ordered: Whether ordered list
            console: Console instance
        """
        if console is None:
            console = Console()

        for i, item in enumerate(items, 1):
            if ordered:
                console.print(f"  {i}. {item}")
            else:
                console.print(f"  • {item}")

    def render_blockquote(
        self,
        text: str,
        console: Console | None = None,
    ) -> None:
        """Render blockquote

        Args:
            text: Quote text
            console: Console instance
        """
        if console is None:
            console = Console()

        style = Style(color="yellow", italic=True)
        quote = Text(f"│ {text}", style=style)
        console.print(quote)

    def render_link(
        self,
        text: str,
        url: str,
        console: Console | None = None,
    ) -> None:
        """Render link

        Args:
            text: Link text
            url: Link URL
            console: Console instance
        """
        if console is None:
            console = Console()

        if self._options.enable_hyperlinks:
            link = Text(text, style=Style(color="blue", underline=True, link=url))
        else:
            link = Text(f"{text} ({url})", style=Style(color="blue"))

        console.print(link)

    def set_options(self, **kwargs: Any) -> None:
        """Set render options

        Args:
            **kwargs: Option key-value pairs
        """
        for key, value in kwargs.items():
            if hasattr(self._options, key):
                setattr(self._options, key, value)

    def is_rich_available(self) -> bool:
        """Check if rich is available

        Returns:
            Whether available
        """
        return RICH_AVAILABLE


def render_markdown(
    text: str,
    theme: CodeTheme = CodeTheme.MONOKAI,
    console: Console | None = None,
) -> None:
    """Convenience function: Render Markdown

    Args:
        text: Markdown text
        theme: Code highlighting theme
        console: Console instance
    """
    renderer = MarkdownRenderer(theme=theme)
    renderer.render(text, console)
