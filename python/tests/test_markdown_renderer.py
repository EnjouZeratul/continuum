"""Markdown Renderer Tests"""

import os
import sys

import pytest


class TestMarkdownRenderer:
    """MarkdownRenderer Tests"""

    def test_import(self):
        """Test module import"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            assert MarkdownRenderer is not None
        except ImportError:
            pytest.skip("rich library not available")

    def test_renderer_creation(self):
        """Test renderer creation"""
        try:
            from continuum_sdk.render import MarkdownRenderer, CodeTheme
            renderer = MarkdownRenderer(theme=CodeTheme.MONOKAI)
            assert renderer is not None
            assert renderer.theme == CodeTheme.MONOKAI
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_simple(self):
        """Test simple markdown rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            # Should not raise
            renderer.render("# Hello World", console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_to_string(self):
        """Test render to string"""
        try:
            from continuum_sdk.render import MarkdownRenderer

            renderer = MarkdownRenderer()
            result = renderer.render_to_string("# Test")
            assert len(result) > 0
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_code_block(self):
        """Test code block rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            code = "def hello():\n    print('Hello')"
            renderer.render_code_block(code, "python", console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_table(self):
        """Test table rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            headers = ["Name", "Value"]
            rows = [["test", "123"], ["foo", "bar"]]
            renderer.render_table(headers, rows, console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_heading(self):
        """Test heading rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            for level in range(1, 7):
                renderer.render_heading(f"Heading {level}", level, console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_list(self):
        """Test list rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            items = ["Item 1", "Item 2", "Item 3"]
            renderer.render_list(items, ordered=False, console=console)
            renderer.render_list(items, ordered=True, console=console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_blockquote(self):
        """Test blockquote rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            renderer.render_blockquote("This is a quote", console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_link(self):
        """Test link rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            renderer.render_link("Click here", "https://example.com", console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_code_theme_enum(self):
        """Test CodeTheme enum"""
        try:
            from continuum_sdk.render import CodeTheme

            assert CodeTheme.MONOKAI.value == "monokai"
            assert CodeTheme.GITHUB_DARK.value == "github-dark"
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_options(self):
        """Test RenderOptions"""
        try:
            from continuum_sdk.render import RenderOptions, CodeTheme

            options = RenderOptions(
                code_theme=CodeTheme.MONOKAI,
                show_line_numbers=True,
                word_wrap=False,
            )
            assert options.code_theme == CodeTheme.MONOKAI
            assert options.show_line_numbers is True
            assert options.word_wrap is False
        except ImportError:
            pytest.skip("rich library not available")

    def test_set_theme(self):
        """Test theme setter"""
        try:
            from continuum_sdk.render import MarkdownRenderer, CodeTheme

            renderer = MarkdownRenderer(theme=CodeTheme.MONOKAI)
            assert renderer.theme == CodeTheme.MONOKAI

            renderer.theme = CodeTheme.GITHUB_DARK
            assert renderer.theme == CodeTheme.GITHUB_DARK
        except ImportError:
            pytest.skip("rich library not available")

    def test_set_options(self):
        """Test set_options method"""
        try:
            from continuum_sdk.render import MarkdownRenderer

            renderer = MarkdownRenderer()
            renderer.set_options(show_line_numbers=True, word_wrap=False)

            assert renderer.options.show_line_numbers is True
            assert renderer.options.word_wrap is False
        except ImportError:
            pytest.skip("rich library not available")

    def test_render_markdown_function(self):
        """Test convenience function"""
        try:
            from continuum_sdk.render import render_markdown
            from rich.console import Console

            console = Console()
            render_markdown("# Test", console=console)
        except ImportError:
            pytest.skip("rich library not available")

    def test_complex_markdown(self):
        """Test complex markdown rendering"""
        try:
            from continuum_sdk.render import MarkdownRenderer
            from rich.console import Console

            renderer = MarkdownRenderer()
            console = Console()

            complex_md = """
# Main Title

This is a paragraph with **bold** and *italic* text.

## Subheading

- Item 1
- Item 2
- Item 3

### Code Example

```python
def hello():
    print("Hello, World!")
```

> This is a blockquote

[Link to example](https://example.com)
"""
            renderer.render(complex_md, console)
        except ImportError:
            pytest.skip("rich library not available")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
