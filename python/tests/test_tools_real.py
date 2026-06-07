"""Tool Tests

Unit tests for real tool implementations.
"""

import asyncio
import os
import tempfile
from pathlib import Path

import pytest

from continuum_sdk.tools import (
    BashTool,
    EditTool,
    GlobTool,
    GrepTool,
    ReadTool,
    ToolError,
    ToolResult,
    WriteTool,
)


class TestBashTool:
    """BashTool tests"""

    def test_bash_echo(self):
        """Test basic echo command"""
        bash = BashTool()
        result = bash.run("echo hello")
        assert result.is_error is False
        assert "hello" in result.content

    def test_bash_with_timeout(self):
        """Test timeout handling"""
        bash = BashTool(default_timeout=1.0)
        with pytest.raises(ToolError):
            bash.run("sleep 5", timeout=0.5)

    def test_bash_command_not_found(self):
        """Test command not found error"""
        bash = BashTool()
        with pytest.raises(ToolError) as exc_info:
            bash.run("nonexistent_command_xyz_12345")
        assert "Command not found" in str(exc_info.value)
        assert "nonexistent_command_xyz_12345" in str(exc_info.value)

    def test_bash_async(self):
        """Test async execution"""
        bash = BashTool()
        result = asyncio.run(bash.run_async("echo async"))
        assert "async" in result.content

    def test_bash_working_dir(self):
        """Test working directory"""
        bash = BashTool()
        with tempfile.TemporaryDirectory() as tmpdir:
            result = bash.run("pwd", working_dir=tmpdir)
            # Windows may return Unix-style path in Git Bash, just check it succeeded
            assert result.is_error is False

    def test_bash_dangerous_validation(self):
        """Test dangerous command validation"""
        from continuum_sdk.tools.bash import validate_command

        # Blocked commands
        assert validate_command("sudo rm -rf /") is not None
        assert validate_command("eval 'code'") is not None


class TestReadTool:
    """ReadTool tests"""

    def test_read_file(self):
        """Test basic file reading"""
        reader = ReadTool()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("Hello, World!\nLine 2\nLine 3")
            filepath = f.name
        try:
            result = reader.read(filepath)
            assert "Hello, World!" in result.content
            assert result.is_error is False
            assert "lines_read" in result.metadata
        finally:
            # Close file handle before unlink on Windows
            import time

            time.sleep(0.1)
            os.unlink(filepath)

    def test_read_with_limit(self):
        """Test reading with limit"""
        reader = ReadTool()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("Line 1\nLine 2\nLine 3\n")
            filepath = f.name
        try:
            result = reader.read(filepath, limit=2)
            assert result.metadata["lines_read"] == 2
        finally:
            import time

            time.sleep(0.1)
            os.unlink(filepath)

    def test_read_with_offset(self):
        """Test reading with offset"""
        reader = ReadTool()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("Line 1\nLine 2\nLine 3\n")
            filepath = f.name
        try:
            result = reader.read(filepath, offset=2, limit=1)
            assert "Line 2" in result.content
        finally:
            import time

            time.sleep(0.1)
            os.unlink(filepath)

    def test_read_nonexistent_file(self):
        """Test reading nonexistent file"""
        reader = ReadTool()
        with pytest.raises(ToolError):
            reader.read("/nonexistent/file/path.txt")

    def test_read_with_line_numbers(self):
        """Test reading with line numbers"""
        reader = ReadTool(show_line_numbers=True)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("Line 1\nLine 2\n")
            filepath = f.name
        try:
            result = reader.read(filepath)
            assert "1\t" in result.content or "     1\t" in result.content
        finally:
            import time

            time.sleep(0.1)
            os.unlink(filepath)


class TestWriteTool:
    """WriteTool tests"""

    def test_write_new_file(self):
        """Test writing new file"""
        writer = WriteTool(backup=False)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            result = writer.write(filepath, "Test content")
            assert result.is_error is False
            assert os.path.exists(filepath)
            with open(filepath) as f:
                assert "Test content" in f.read()

    def test_write_append(self):
        """Test appending to file"""
        writer = WriteTool(backup=False)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            writer.write(filepath, "Line 1")
            writer.append(filepath, "Line 2")
            with open(filepath) as f:
                content = f.read()
                assert "Line 1" in content
                assert "Line 2" in content

    def test_write_with_backup(self):
        """Test writing with backup"""
        writer = WriteTool(backup=True)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            # Write initial content
            with open(filepath, "w") as f:
                f.write("Original content")
            # Overwrite with backup
            result = writer.write(filepath, "New content")
            assert result.is_error is False
            # Check backup exists
            backup_path = filepath + ".bak"
            assert os.path.exists(backup_path)


class TestEditTool:
    """EditTool tests"""

    def test_edit_file(self):
        """Test basic file editing"""
        editor = EditTool(backup=False)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w") as f:
                f.write("Hello old world")
            result = editor.edit(filepath, "old", "new")
            assert result.is_error is False
            with open(filepath) as f:
                assert "Hello new world" in f.read()

    def test_edit_not_found(self):
        """Test editing with string not found"""
        editor = EditTool(backup=False)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w") as f:
                f.write("Hello world")
            with pytest.raises(ToolError):
                editor.edit(filepath, "nonexistent", "replacement")

    def test_edit_replace_all(self):
        """Test replacing all occurrences"""
        editor = EditTool(backup=False)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w") as f:
                f.write("foo bar foo bar foo")
            result = editor.replace_all(filepath, "foo", "baz")
            assert result.is_error is False
            with open(filepath) as f:
                content = f.read()
                assert content.count("baz") == 3
                assert "foo" not in content

    def test_edit_has_diff(self):
        """Test that edit produces diff in metadata"""
        editor = EditTool(backup=False)
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w") as f:
                f.write("Hello world")
            result = editor.edit(filepath, "world", "universe")
            assert "diff" in result.metadata
            assert result.metadata["replacements"] == 1


class TestGrepTool:
    """GrepTool tests"""

    def test_grep_find_pattern(self):
        """Test finding pattern in files"""
        grep_tool = GrepTool()
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.py")
            with open(filepath, "w") as f:
                f.write("def hello():\n    print('hello')\n\ndef world():\n    pass")
            result = grep_tool.search("def ", path=tmpdir)
            assert "def hello" in result.content or "def world" in result.content

    def test_grep_files_with_matches(self):
        """Test files_with_matches mode"""
        grep_tool = GrepTool()
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w") as f:
                f.write("pattern here")
            result = grep_tool.search(
                "pattern", path=tmpdir, output_mode="files_with_matches"
            )
            assert filepath in result.content or "test.txt" in result.content

    def test_grep_case_insensitive(self):
        """Test case insensitive search"""
        grep_tool = GrepTool()
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w") as f:
                f.write("PATTERN HERE")
            result = grep_tool.search("pattern", path=tmpdir, case_sensitive=False)
            assert "PATTERN" in result.content


class TestGlobTool:
    """GlobTool tests"""

    def test_glob_find_files(self):
        """Test finding files with glob pattern"""
        glob_tool = GlobTool()
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create some files
            for name in ["test.py", "test.txt", "other.py"]:
                Path(tmpdir, name).write_text("content")
            result = glob_tool.find("*.py", path=tmpdir)
            assert "test.py" in result.content or "other.py" in result.content
            assert ".txt" not in result.content

    def test_glob_recursive(self):
        """Test recursive glob"""
        glob_tool = GlobTool()
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create nested structure
            subdir = Path(tmpdir, "subdir")
            subdir.mkdir()
            Path(subdir, "nested.py").write_text("content")
            result = glob_tool.find("**/*.py", path=tmpdir)
            assert "nested.py" in result.content

    def test_glob_metadata(self):
        """Test glob result metadata"""
        glob_tool = GlobTool()
        result = glob_tool.find("*.py", path=".")
        assert "count" in result.metadata


class TestToolTypes:
    """Tool type tests"""

    def test_tool_result_str(self):
        """Test ToolResult string representation"""
        result = ToolResult(
            call_id="abc123",
            name="test",
            content="Success",
            is_error=False,
            duration_ms=100,
        )
        s = str(result)
        assert "[OK]" in s
        assert "test" in s
        assert "100ms" in s

    def test_tool_error(self):
        """Test ToolError exception"""
        error = ToolError(
            call_id="xyz789",
            name="bash",
            message="Command failed",
        )
        assert "[bash]" in str(error)
        assert error.call_id == "xyz789"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
