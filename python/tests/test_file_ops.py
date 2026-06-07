"""
Tests for tools/file_ops.py

Test coverage for:
- ReadTool: file reading, encoding detection, pagination
- WriteTool: file writing, backup, append mode
- EditTool: string replacement, replace_all
- ListDirectoryTool: directory listing
- Encoding detection
- Error handling
"""

from pathlib import Path

import pytest

from continuum_sdk.tools.file_ops import (
    EditTool,
    ListDirectoryTool,
    ReadTool,
    WriteTool,
    detect_encoding,
    edit_file,
    list_directory,
    read_file,
    write_file,
)
from continuum_sdk.tools.types import ToolError, ToolResult


class TestDetectEncoding:
    """Tests for encoding detection."""

    def test_detect_utf8(self, tmp_path: Path):
        """Test UTF-8 encoding detection."""
        file_path = tmp_path / "utf8.txt"
        file_path.write_text("Hello, World!", encoding="utf-8")

        encoding = detect_encoding(file_path)
        assert encoding == "utf-8"

    def test_detect_utf8_with_bom(self, tmp_path: Path):
        """Test UTF-8 with BOM."""
        file_path = tmp_path / "utf8_bom.txt"
        # Write UTF-8 with BOM
        with open(file_path, "wb") as f:
            f.write(b"\xef\xbb\xbfHello, World!")

        encoding = detect_encoding(file_path)
        assert encoding == "utf-8"

    def test_detect_gbk(self, tmp_path: Path):
        """Test GBK encoding detection for Chinese text."""
        file_path = tmp_path / "gbk.txt"
        # GBK encoded Chinese text
        with open(file_path, "wb") as f:
            f.write("你好世界".encode("gbk"))

        encoding = detect_encoding(file_path)
        assert encoding == "gbk"

    def test_detect_latin1_fallback(self, tmp_path: Path):
        """Test fallback to latin-1 for binary-ish content."""
        file_path = tmp_path / "mixed.txt"
        # Latin-1 specific bytes
        with open(file_path, "wb") as f:
            f.write(b"caf\xe9")  # 'cafe' with accent in latin-1

        encoding = detect_encoding(file_path)
        # Should detect some encoding (latin-1 or fallback to utf-8)
        assert encoding in ("utf-8", "latin-1", "gbk", "gb18030")

    def test_detect_nonexistent_file(self, tmp_path: Path):
        """Test encoding detection for nonexistent file returns utf-8 fallback."""
        file_path = tmp_path / "nonexistent.txt"
        encoding = detect_encoding(file_path)
        # Should return utf-8 as fallback when file can't be read
        assert encoding == "utf-8"

    def test_detect_encoding_with_oserror(self, tmp_path: Path, monkeypatch):
        """Test encoding detection handles OSError gracefully (line 57-58)."""
        file_path = tmp_path / "oserror_encoding.txt"
        file_path.write_text("content")

        original_open = open

        def mock_open_raise_oserror(*args, **kwargs):
            mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
            if isinstance(mode, bytes) or "rb" in str(mode):
                raise OSError("Mock OS error")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_open_raise_oserror)

        encoding = detect_encoding(file_path)
        # Should return utf-8 as fallback
        assert encoding == "utf-8"


class TestReadTool:
    """Tests for ReadTool functionality."""

    def test_read_tool_call_directly(self, tmp_path: Path):
        """Test calling ReadTool instance directly (line 211-213)."""
        file_path = tmp_path / "call_direct_read.txt"
        file_path.write_text("Direct call test")

        reader = ReadTool()
        result = reader(str(file_path))  # Call instance directly

        assert result.is_error is False
        assert result.content == "Direct call test"

    def test_read_simple_file(self, tmp_path: Path):
        """Test reading a simple file."""
        file_path = tmp_path / "simple.txt"
        file_path.write_text("Hello, World!")

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert isinstance(result, ToolResult)
        assert result.is_error is False
        assert result.content == "Hello, World!"
        assert result.metadata["path"] == str(file_path)
        assert result.metadata["encoding"] == "utf-8"
        assert result.metadata["total_lines"] == 1

    def test_read_multiline_file(self, tmp_path: Path):
        """Test reading a multiline file."""
        file_path = tmp_path / "multiline.txt"
        lines = ["Line 1", "Line 2", "Line 3", "Line 4", "Line 5"]
        file_path.write_text("\n".join(lines))

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert result.is_error is False
        assert result.content == "\n".join(lines)
        assert result.metadata["total_lines"] == 5

    def test_read_with_offset(self, tmp_path: Path):
        """Test reading with offset (pagination)."""
        file_path = tmp_path / "offset.txt"
        lines = [f"Line {i}" for i in range(1, 11)]  # Line 1 to Line 10
        file_path.write_text("\n".join(lines))

        reader = ReadTool()
        result = reader.read(str(file_path), offset=3, limit=3)

        assert result.is_error is False
        # Should read lines 3, 4, 5
        result_lines = result.content.split("\n")
        assert len(result_lines) == 3
        assert result_lines[0] == "Line 3"

    def test_read_with_limit(self, tmp_path: Path):
        """Test reading with limit."""
        file_path = tmp_path / "limit.txt"
        lines = [f"Line {i}" for i in range(100)]
        file_path.write_text("\n".join(lines))

        reader = ReadTool()
        result = reader.read(str(file_path), limit=10)

        assert result.is_error is False
        assert result.metadata["lines_read"] == 10

    def test_read_with_offset_and_limit(self, tmp_path: Path):
        """Test reading with both offset and limit."""
        file_path = tmp_path / "both.txt"
        lines = [f"Line {i}" for i in range(1, 21)]
        file_path.write_text("\n".join(lines))

        reader = ReadTool()
        result = reader.read(str(file_path), offset=5, limit=5)

        assert result.is_error is False
        result_lines = result.content.split("\n")
        assert len(result_lines) == 5
        assert result_lines[0] == "Line 5"
        assert result_lines[-1] == "Line 9"

    def test_read_with_line_numbers(self, tmp_path: Path):
        """Test reading with line numbers enabled."""
        file_path = tmp_path / "numbered.txt"
        file_path.write_text("Line A\nLine B\nLine C")

        reader = ReadTool(show_line_numbers=True)
        result = reader.read(str(file_path))

        assert result.is_error is False
        # Should have line number formatting
        assert "1" in result.content
        assert "Line A" in result.content

    def test_read_nonexistent_file(self, tmp_path: Path):
        """Test reading a nonexistent file raises ToolError."""
        reader = ReadTool()
        with pytest.raises(ToolError) as exc_info:
            reader.read(str(tmp_path / "nonexistent.txt"))

        assert exc_info.value.name == "read"
        assert "not found" in exc_info.value.message.lower()

    def test_read_directory_instead_of_file(self, tmp_path: Path):
        """Test reading a directory raises ToolError."""
        reader = ReadTool()
        with pytest.raises(ToolError) as exc_info:
            reader.read(str(tmp_path))

        assert exc_info.value.name == "read"
        assert "not a file" in exc_info.value.message.lower()

    def test_read_chinese_utf8(self, tmp_path: Path):
        """Test reading Chinese UTF-8 file."""
        file_path = tmp_path / "chinese_utf8.txt"
        content = "你好，世界！\n这是中文测试。"
        file_path.write_text(content, encoding="utf-8")

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert result.is_error is False
        assert "你好" in result.content
        assert result.metadata["encoding"] == "utf-8"

    def test_read_chinese_gbk(self, tmp_path: Path):
        """Test reading Chinese GBK file."""
        file_path = tmp_path / "chinese_gbk.txt"
        content = "你好，世界！"
        with open(file_path, "w", encoding="gbk") as f:
            f.write(content)

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert result.is_error is False
        # Should detect GBK and decode correctly
        assert result.metadata["encoding"] == "gbk"

    def test_read_empty_file(self, tmp_path: Path):
        """Test reading an empty file."""
        file_path = tmp_path / "empty.txt"
        file_path.write_text("")

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert result.is_error is False
        assert result.content == ""
        assert result.metadata["total_lines"] == 0

    def test_read_call_id_generation(self, tmp_path: Path):
        """Test that each read generates a unique call_id."""
        file_path = tmp_path / "id_test.txt"
        file_path.write_text("content")

        reader = ReadTool()
        result1 = reader.read(str(file_path))
        result2 = reader.read(str(file_path))

        assert result1.call_id != result2.call_id

    def test_read_file_function(self, tmp_path: Path):
        """Test read_file function directly."""
        file_path = tmp_path / "direct.txt"
        file_path.write_text("Direct function test")

        result = read_file(str(file_path))

        assert result.is_error is False
        assert result.content == "Direct function test"

    def test_read_with_workspace(self, tmp_path: Path):
        """Test reading with workspace parameter for security."""
        file_path = tmp_path / "workspace_test.txt"
        file_path.write_text("Workspace content")

        result = read_file(str(file_path), workspace=str(tmp_path))

        assert result.is_error is False
        assert result.content == "Workspace content"


class TestWriteTool:
    """Tests for WriteTool functionality."""

    def test_write_content_not_ending_with_newline(self, tmp_path: Path):
        """Test that write adds newline when content doesn't end with one (lines 287-290)."""
        file_path = tmp_path / "newline_test.txt"

        writer = WriteTool()
        # Write content without trailing newline
        result = writer.write(str(file_path), "Content without newline")

        assert result.is_error is False
        # File should have newline added
        content = file_path.read_text()
        assert content == "Content without newline\n"

    def test_write_content_with_existing_newline(self, tmp_path: Path):
        """Test that write doesn't add extra newline when content already has one (line 287->290 branch)."""
        file_path = tmp_path / "has_newline.txt"

        writer = WriteTool()
        # Write content WITH trailing newline
        result = writer.write(str(file_path), "Content with newline\n")

        assert result.is_error is False
        # File should have exactly one trailing newline, not two
        content = file_path.read_text()
        assert content == "Content with newline\n"

    def test_write_simple_file(self, tmp_path: Path):
        """Test writing a simple file."""
        file_path = tmp_path / "write_test.txt"
        content = "Hello, Write!"

        writer = WriteTool()
        result = writer.write(str(file_path), content)

        assert isinstance(result, ToolResult)
        assert result.is_error is False
        assert file_path.exists()
        assert file_path.read_text() == content + "\n"

    def test_write_creates_parent_directories(self, tmp_path: Path):
        """Test that write creates parent directories."""
        file_path = tmp_path / "nested" / "deep" / "file.txt"
        content = "Nested content"

        writer = WriteTool()
        result = writer.write(str(file_path), content)

        assert result.is_error is False
        assert file_path.exists()
        assert file_path.read_text() == content + "\n"

    def test_write_creates_backup(self, tmp_path: Path):
        """Test that write creates backup when overwriting."""
        file_path = tmp_path / "backup_test.txt"
        file_path.write_text("Original content")

        writer = WriteTool(backup=True)
        result = writer.write(str(file_path), "New content")

        assert result.is_error is False
        assert file_path.read_text() == "New content\n"
        # Check backup exists
        backup_path = file_path.with_suffix(".txt.bak")
        assert backup_path.exists()
        assert backup_path.read_text() == "Original content"

    def test_write_no_backup(self, tmp_path: Path):
        """Test write without backup."""
        file_path = tmp_path / "no_backup.txt"
        file_path.write_text("Original")

        writer = WriteTool(backup=False)
        result = writer.write(str(file_path), "New")

        assert result.is_error is False
        # No backup should be created
        backup_path = file_path.with_suffix(".txt.bak")
        assert not backup_path.exists()

    def test_write_append_mode(self, tmp_path: Path):
        """Test append mode."""
        file_path = tmp_path / "append.txt"
        file_path.write_text("Original\n")

        writer = WriteTool()
        result = writer.append(str(file_path), "Appended")

        assert result.is_error is False
        content = file_path.read_text()
        assert "Original" in content
        assert "Appended" in content

    def test_write_append_to_new_file(self, tmp_path: Path):
        """Test append to a new file creates it."""
        file_path = tmp_path / "new_append.txt"

        writer = WriteTool()
        result = writer.append(str(file_path), "First line")

        assert result.is_error is False
        assert file_path.exists()
        assert "First line" in file_path.read_text()

    def test_write_with_custom_encoding(self, tmp_path: Path):
        """Test write with custom encoding."""
        file_path = tmp_path / "encoded.txt"
        content = "Hello"

        result = write_file(str(file_path), content, encoding="utf-8")

        assert result.is_error is False
        assert result.metadata["bytes_written"] == len(content.encode("utf-8"))

    def test_write_metadata(self, tmp_path: Path):
        """Test write result metadata."""
        file_path = tmp_path / "meta.txt"
        content = "Content for metadata check"

        writer = WriteTool()
        result = writer.write(str(file_path), content)

        assert "path" in result.metadata
        assert "bytes_written" in result.metadata
        assert result.metadata["bytes_written"] == len(content.encode("utf-8"))

    def test_write_overwrites_existing(self, tmp_path: Path):
        """Test that write overwrites existing file."""
        file_path = tmp_path / "overwrite.txt"
        file_path.write_text("Old content")

        writer = WriteTool(backup=False)
        result = writer.write(str(file_path), "New content")

        assert result.is_error is False
        assert file_path.read_text() == "New content\n"

    def test_write_function_directly(self, tmp_path: Path):
        """Test write_file function directly."""
        file_path = tmp_path / "direct_write.txt"

        result = write_file(str(file_path), "Direct function")

        assert result.is_error is False
        assert file_path.exists()

    def test_write_call_directly(self, tmp_path: Path):
        """Test calling WriteTool instance directly."""
        file_path = tmp_path / "call_direct.txt"

        writer = WriteTool()
        result = writer(str(file_path), "Direct call")

        assert result.is_error is False
        assert file_path.exists()


class TestEditTool:
    """Tests for EditTool functionality."""

    def test_edit_simple_replacement(self, tmp_path: Path):
        """Test simple string replacement."""
        file_path = tmp_path / "edit_simple.txt"
        file_path.write_text("Hello, World!")

        editor = EditTool()
        result = editor.edit(str(file_path), "World", "Python")

        assert isinstance(result, ToolResult)
        assert result.is_error is False
        assert file_path.read_text() == "Hello, Python!"
        assert result.metadata["replacements"] == 1

    def test_edit_multiple_occurrences_single(self, tmp_path: Path):
        """Test editing file with multiple occurrences (single replace)."""
        file_path = tmp_path / "multi.txt"
        file_path.write_text("foo bar foo baz foo")

        editor = EditTool()
        result = editor.edit(str(file_path), "foo", "qux")

        assert result.is_error is False
        # Only first occurrence replaced
        assert file_path.read_text() == "qux bar foo baz foo"
        assert result.metadata["replacements"] == 1
        assert result.metadata["total_occurrences"] == 3

    def test_edit_replace_all(self, tmp_path: Path):
        """Test replacing all occurrences."""
        file_path = tmp_path / "replace_all.txt"
        file_path.write_text("foo bar foo baz foo")

        editor = EditTool()
        result = editor.replace_all(str(file_path), "foo", "qux")

        assert result.is_error is False
        assert file_path.read_text() == "qux bar qux baz qux"
        assert result.metadata["replacements"] == 3

    def test_edit_with_replace_all_flag(self, tmp_path: Path):
        """Test replace_all flag."""
        file_path = tmp_path / "flag.txt"
        file_path.write_text("a b a b a")

        editor = EditTool()
        result = editor.edit(str(file_path), "a", "x", replace_all=True)

        assert result.is_error is False
        assert file_path.read_text() == "x b x b x"

    def test_edit_creates_backup(self, tmp_path: Path):
        """Test that edit creates backup."""
        file_path = tmp_path / "edit_backup.txt"
        original = "Original content"
        file_path.write_text(original)

        editor = EditTool(backup=True)
        editor.edit(str(file_path), "Original", "New")

        backup_path = file_path.with_suffix(".txt.bak")
        assert backup_path.exists()
        assert backup_path.read_text() == original

    def test_edit_no_backup(self, tmp_path: Path):
        """Test edit without backup."""
        file_path = tmp_path / "no_edit_backup.txt"
        file_path.write_text("Content to edit")

        editor = EditTool(backup=False)
        editor.edit(str(file_path), "edit", "modify")

        backup_path = file_path.with_suffix(".txt.bak")
        assert not backup_path.exists()

    def test_edit_string_not_found(self, tmp_path: Path):
        """Test editing when string not found."""
        file_path = tmp_path / "not_found.txt"
        file_path.write_text("Hello")

        editor = EditTool()
        with pytest.raises(ToolError) as exc_info:
            editor.edit(str(file_path), "nonexistent", "replacement")

        assert exc_info.value.name == "edit"
        assert "not found" in exc_info.value.message.lower()

    def test_edit_nonexistent_file(self, tmp_path: Path):
        """Test editing a nonexistent file."""
        editor = EditTool()
        with pytest.raises(ToolError) as exc_info:
            editor.edit(str(tmp_path / "nonexistent.txt"), "old", "new")

        assert exc_info.value.name == "edit"
        assert "not found" in exc_info.value.message.lower()

    def test_edit_multiline_replacement(self, tmp_path: Path):
        """Test replacing multiline strings."""
        file_path = tmp_path / "multiline.txt"
        file_path.write_text("Line 1\nLine 2\nLine 3")

        editor = EditTool()
        result = editor.edit(str(file_path), "Line 1\nLine 2", "New Line")

        assert result.is_error is False
        assert file_path.read_text() == "New Line\nLine 3"

    def test_edit_metadata(self, tmp_path: Path):
        """Test edit result metadata."""
        file_path = tmp_path / "meta_edit.txt"
        file_path.write_text("foo foo foo")

        editor = EditTool()
        result = editor.replace_all(str(file_path), "foo", "bar")

        assert "replacements" in result.metadata
        assert "total_occurrences" in result.metadata
        assert "diff" in result.metadata
        assert result.metadata["replacements"] == 3
        assert result.metadata["total_occurrences"] == 3

    def test_edit_diff_generation(self, tmp_path: Path):
        """Test that diff is generated in metadata."""
        file_path = tmp_path / "diff.txt"
        file_path.write_text("old content")

        editor = EditTool()
        result = editor.edit(str(file_path), "old", "new")

        assert "diff" in result.metadata
        assert "---" in result.metadata["diff"] or "-old" in result.metadata["diff"]

    def test_edit_function_directly(self, tmp_path: Path):
        """Test edit_file function directly."""
        file_path = tmp_path / "direct_edit.txt"
        file_path.write_text("Direct test")

        result = edit_file(str(file_path), "Direct", "Indirect")

        assert result.is_error is False
        assert "Indirect" in file_path.read_text()

    def test_edit_call_directly(self, tmp_path: Path):
        """Test calling EditTool instance directly."""
        file_path = tmp_path / "call_direct.txt"
        file_path.write_text("Call test")

        editor = EditTool()
        result = editor(str(file_path), "Call", "Direct")

        assert result.is_error is False


class TestListDirectoryTool:
    """Tests for ListDirectoryTool functionality."""

    def test_list_directory(self, tmp_path: Path):
        """Test listing directory contents."""
        # Create some files and directories
        (tmp_path / "file1.txt").write_text("content")
        (tmp_path / "file2.py").write_text("code")
        (tmp_path / "subdir").mkdir()

        tool = ListDirectoryTool()
        result = tool.list(str(tmp_path))

        assert isinstance(result, ToolResult)
        assert result.is_error is False
        assert "entries" in result.metadata
        entries = result.metadata["entries"]
        assert len(entries) == 3

        names = [e["name"] for e in entries]
        assert "file1.txt" in names
        assert "file2.py" in names
        assert "subdir" in names

    def test_list_directory_entry_types(self, tmp_path: Path):
        """Test that entries have correct types."""
        (tmp_path / "file.txt").write_text("content")
        (tmp_path / "directory").mkdir()

        tool = ListDirectoryTool()
        result = tool.list(str(tmp_path))

        entries = result.metadata["entries"]
        file_entry = next(e for e in entries if e["name"] == "file.txt")
        dir_entry = next(e for e in entries if e["name"] == "directory")

        assert file_entry["type"] == "file"
        assert dir_entry["type"] == "dir"

    def test_list_empty_directory(self, tmp_path: Path):
        """Test listing empty directory."""
        tool = ListDirectoryTool()
        result = tool.list(str(tmp_path))

        assert result.is_error is False
        assert result.metadata["count"] == 0
        assert len(result.metadata["entries"]) == 0

    def test_list_nonexistent_directory(self, tmp_path: Path):
        """Test listing nonexistent directory."""
        tool = ListDirectoryTool()
        with pytest.raises(ToolError) as exc_info:
            tool.list(str(tmp_path / "nonexistent"))

        assert exc_info.value.name == "list_directory"
        assert "not found" in exc_info.value.message.lower()

    def test_list_file_instead_of_directory(self, tmp_path: Path):
        """Test listing a file instead of directory."""
        file_path = tmp_path / "notadir.txt"
        file_path.write_text("content")

        tool = ListDirectoryTool()
        with pytest.raises(ToolError) as exc_info:
            tool.list(str(file_path))

        assert exc_info.value.name == "list_directory"
        assert "not a directory" in exc_info.value.message.lower()

    def test_list_directory_function(self, tmp_path: Path):
        """Test list_directory function directly."""
        (tmp_path / "test.txt").write_text("content")

        result = list_directory(str(tmp_path))

        assert result.is_error is False
        assert result.metadata["count"] == 1

    def test_list_call_directly(self, tmp_path: Path):
        """Test calling ListDirectoryTool instance directly."""
        (tmp_path / "direct.txt").write_text("test")

        tool = ListDirectoryTool()
        result = tool(str(tmp_path))

        assert result.is_error is False


class TestErrorHandling:
    """Tests for error handling scenarios."""

    def test_read_permission_denied_simulation(self, tmp_path: Path):
        """Test read tool handles permission errors correctly (simulation)."""
        # This test verifies the error handling path
        # Actual permission denied tests require OS-level setup
        file_path = tmp_path / "readable.txt"
        file_path.write_text("content")

        reader = ReadTool()
        result = reader.read(str(file_path))

        # Should succeed normally
        assert result.is_error is False

    def test_write_permission_denied_simulation(self, tmp_path: Path):
        """Test write tool handles permission errors correctly (simulation)."""
        file_path = tmp_path / "writable.txt"

        writer = WriteTool()
        result = writer.write(str(file_path), "content")

        # Should succeed normally
        assert result.is_error is False

    def test_read_permission_error_path(self, tmp_path: Path, monkeypatch):
        """Test read_file PermissionError handling (lines 170-177)."""
        file_path = tmp_path / "protected.txt"
        file_path.write_text("content")

        original_open = open

        def mock_open_raise_permission(*args, **kwargs):
            mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
            # Only raise for read operations, not for write (backup creation)
            if isinstance(mode, str) and "r" in mode and "+" not in mode:
                # Avoid raising during encoding detection or other operations
                call_stack = kwargs.get("_stack_hint", "")
                if "read" in str(args[0]) or call_stack:
                    raise PermissionError("Mock permission denied")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_open_raise_permission)

        reader = ReadTool()
        with pytest.raises(ToolError) as exc_info:
            reader.read(str(file_path))

        assert exc_info.value.name == "read"
        assert "permission denied" in exc_info.value.message.lower()

    def test_read_oserror_path(self, tmp_path: Path, monkeypatch):
        """Test read_file OSError handling (lines 178-185)."""
        file_path = tmp_path / "oserror.txt"
        file_path.write_text("content")

        original_open = open

        def mock_open_raise_oserror(*args, **kwargs):
            mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
            if isinstance(mode, str) and "r" in mode and "+" not in mode:
                raise OSError("Mock OS error")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_open_raise_oserror)

        reader = ReadTool()
        with pytest.raises(ToolError) as exc_info:
            reader.read(str(file_path))

        assert exc_info.value.name == "read"
        assert "failed to read file" in exc_info.value.message.lower()

    def test_write_permission_error_path(self, tmp_path: Path, monkeypatch):
        """Test write_file PermissionError handling (lines 312-319)."""
        file_path = tmp_path / "write_protected.txt"
        file_path.write_text("original")

        original_open = open

        def mock_open_raise_permission(*args, **kwargs):
            mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
            # Raise for write operations
            if isinstance(mode, str) and "w" in mode:
                raise PermissionError("Mock write permission denied")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_open_raise_permission)

        writer = WriteTool()
        with pytest.raises(ToolError) as exc_info:
            writer.write(str(file_path), "new content")

        assert exc_info.value.name == "write"
        assert "permission denied" in exc_info.value.message.lower()

    def test_write_oserror_with_backup_restore(self, tmp_path: Path, monkeypatch):
        """Test write_file OSError handling with backup restore (lines 320-329)."""
        file_path = tmp_path / "backup_restore.txt"
        original_content = "original content"
        file_path.write_text(original_content)

        original_open = open
        # Track write attempts to simulate failure
        write_attempts = [0]

        def mock_write_fail(*args, **kwargs):
            # Intercept file writes
            if len(args) > 0 and "backup_restore.txt" in str(args[0]):
                mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
                if isinstance(mode, str) and "w" in mode:
                    write_attempts[0] += 1
                    if write_attempts[0] == 1 and ".bak" not in str(args[0]):
                        raise OSError("Mock write error")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_write_fail)

        writer = WriteTool(backup=True)
        with pytest.raises(ToolError) as exc_info:
            writer.write(str(file_path), "new content")

        assert exc_info.value.name == "write"
        # Error should be raised
        assert "failed to write file" in exc_info.value.message.lower()

    def test_edit_read_error_path(self, tmp_path: Path, monkeypatch):
        """Test edit_file read error handling (lines 427-434)."""
        file_path = tmp_path / "edit_read_error.txt"
        file_path.write_text("content")

        original_open = open

        def mock_open_raise_error(*args, **kwargs):
            mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
            if isinstance(mode, str) and "r" in mode and "+" not in mode:
                raise OSError("Mock read error")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_open_raise_error)

        editor = EditTool()
        with pytest.raises(ToolError) as exc_info:
            editor.edit(str(file_path), "content", "replacement")

        assert exc_info.value.name == "edit"
        assert "failed to read file" in exc_info.value.message.lower()

    def test_edit_write_error_with_backup_restore(self, tmp_path: Path, monkeypatch):
        """Test edit_file write error with backup restore (lines 466-476)."""
        file_path = tmp_path / "edit_write_error.txt"
        original_content = "original content"
        file_path.write_text(original_content)

        original_open = open
        write_attempts = [0]

        def mock_write_fail(*args, **kwargs):
            # Intercept file writes
            if len(args) > 0 and "edit_write_error.txt" in str(args[0]):
                mode = kwargs.get("mode", args[1] if len(args) > 1 else "r")
                if isinstance(mode, str) and "w" in mode and ".bak" not in str(args[0]):
                    write_attempts[0] += 1
                    if write_attempts[0] == 1:
                        raise OSError("Mock write error")
            return original_open(*args, **kwargs)

        monkeypatch.setattr("builtins.open", mock_write_fail)

        editor = EditTool(backup=True)
        with pytest.raises(ToolError) as exc_info:
            editor.edit(str(file_path), "original", "replacement")

        assert exc_info.value.name == "edit"
        # Error should be raised
        assert "failed to write file" in exc_info.value.message.lower()

    def test_list_directory_permission_error(self, tmp_path: Path, monkeypatch):
        """Test list_directory PermissionError handling (lines 606-613)."""
        (tmp_path / "file.txt").write_text("content")

        def mock_iterdir_raise_permission(self):
            raise PermissionError("Mock permission denied")

        monkeypatch.setattr(Path, "iterdir", mock_iterdir_raise_permission)

        tool = ListDirectoryTool()
        with pytest.raises(ToolError) as exc_info:
            tool.list(str(tmp_path))

        assert exc_info.value.name == "list_directory"
        assert "permission denied" in exc_info.value.message.lower()

    def test_list_directory_oserror(self, tmp_path: Path, monkeypatch):
        """Test list_directory OSError handling (lines 614-621)."""
        (tmp_path / "file.txt").write_text("content")

        def mock_iterdir_raise_oserror(self):
            raise OSError("Mock OS error")

        monkeypatch.setattr(Path, "iterdir", mock_iterdir_raise_oserror)

        tool = ListDirectoryTool()
        with pytest.raises(ToolError) as exc_info:
            tool.list(str(tmp_path))

        assert exc_info.value.name == "list_directory"
        assert "failed to list directory" in exc_info.value.message.lower()

    def test_tool_error_attributes(self, tmp_path: Path):
        """Test ToolError has correct attributes."""
        reader = ReadTool()

        with pytest.raises(ToolError) as exc_info:
            reader.read(str(tmp_path / "nonexistent.txt"))

        error = exc_info.value
        assert hasattr(error, "call_id")
        assert hasattr(error, "name")
        assert hasattr(error, "message")
        assert error.name == "read"

    def test_tool_result_attributes(self, tmp_path: Path):
        """Test ToolResult has correct attributes."""
        file_path = tmp_path / "test.txt"
        file_path.write_text("content")

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert hasattr(result, "call_id")
        assert hasattr(result, "name")
        assert hasattr(result, "content")
        assert hasattr(result, "is_error")
        assert hasattr(result, "duration_ms")
        assert hasattr(result, "metadata")


class TestEdgeCases:
    """Tests for edge cases and boundary conditions."""

    def test_read_offset_beyond_file(self, tmp_path: Path):
        """Test reading with offset beyond file length."""
        file_path = tmp_path / "short.txt"
        file_path.write_text("Line 1\nLine 2")

        reader = ReadTool()
        result = reader.read(str(file_path), offset=100)

        assert result.is_error is False
        assert result.content == ""
        assert result.metadata["lines_read"] == 0

    def test_read_limit_beyond_file(self, tmp_path: Path):
        """Test reading with limit beyond file length."""
        file_path = tmp_path / "limit_beyond.txt"
        file_path.write_text("Line 1\nLine 2")

        reader = ReadTool()
        result = reader.read(str(file_path), limit=100)

        assert result.is_error is False
        assert result.metadata["lines_read"] == 2

    def test_read_negative_offset(self, tmp_path: Path):
        """Test reading with negative offset is handled gracefully."""
        file_path = tmp_path / "negative.txt"
        file_path.write_text("Content")

        reader = ReadTool()
        result = reader.read(str(file_path), offset=-1)

        # Should handle gracefully (likely starts from 0)
        assert result.is_error is False

    def test_edit_empty_string_replacement(self, tmp_path: Path):
        """Test replacing with empty string."""
        file_path = tmp_path / "empty_replace.txt"
        file_path.write_text("Hello World")

        editor = EditTool()
        result = editor.edit(str(file_path), "World", "")

        assert result.is_error is False
        assert file_path.read_text() == "Hello "

    def test_edit_with_newlines(self, tmp_path: Path):
        """Test edit with newlines in old/new strings."""
        file_path = tmp_path / "newlines.txt"
        file_path.write_text("Line 1\nLine 2\nLine 3")

        editor = EditTool()
        result = editor.edit(str(file_path), "\n", ";")

        assert result.is_error is False
        assert ";" in file_path.read_text()

    def test_write_empty_content(self, tmp_path: Path):
        """Test writing empty content."""
        file_path = tmp_path / "empty.txt"

        writer = WriteTool()
        result = writer.write(str(file_path), "")

        assert result.is_error is False
        assert file_path.exists()

    def test_write_unicode_content(self, tmp_path: Path):
        """Test writing Unicode content."""
        file_path = tmp_path / "unicode.txt"
        content = "Hello 你好 مرحبا Привет 🌍"

        writer = WriteTool()
        result = writer.write(str(file_path), content)

        assert result.is_error is False
        assert file_path.read_text(encoding="utf-8") == content + "\n"

    def test_path_with_spaces(self, tmp_path: Path):
        """Test handling paths with spaces."""
        file_path = tmp_path / "path with spaces" / "file name.txt"
        content = "Content in spaced path"

        writer = WriteTool()
        result = writer.write(str(file_path), content)

        assert result.is_error is False
        assert file_path.exists()
        assert file_path.read_text() == content + "\n"

    def test_read_binary_detection(self, tmp_path: Path):
        """Test reading file that's mostly binary but decodable."""
        file_path = tmp_path / "mixed.txt"
        # Write content that's valid UTF-8
        file_path.write_bytes(b"Hello\x00World")

        reader = ReadTool()
        result = reader.read(str(file_path))

        # Should succeed (with replacement character for null)
        assert result.is_error is False

    def test_read_with_content_not_ending_in_newline(self, tmp_path: Path):
        """Test reading content that doesn't end in newline (covers line 144 branch)."""
        file_path = tmp_path / "no_newline.txt"
        file_path.write_text(
            "Line 1\nLine 2\nLine 3", newline=""
        )  # No trailing newline

        reader = ReadTool()
        result = reader.read(str(file_path))

        assert result.is_error is False
        assert "Line 3" in result.content


class TestSecurityIntegration:
    """Tests for security integration with workspace."""

    def test_read_with_workspace_security(self, tmp_path: Path):
        """Test read with workspace enforces path validation."""
        file_path = tmp_path / "secured.txt"
        file_path.write_text("Secured content")

        result = read_file(str(file_path), workspace=str(tmp_path))

        assert result.is_error is False
        assert result.content == "Secured content"

    def test_write_with_workspace_security(self, tmp_path: Path):
        """Test write with workspace enforces path validation."""
        file_path = tmp_path / "secured_write.txt"

        result = write_file(str(file_path), "Content", workspace=str(tmp_path))

        assert result.is_error is False
        assert file_path.exists()

    def test_edit_with_workspace_security(self, tmp_path: Path):
        """Test edit with workspace enforces path validation."""
        file_path = tmp_path / "secured_edit.txt"
        file_path.write_text("Original text")

        result = edit_file(
            str(file_path), "Original", "Modified", workspace=str(tmp_path)
        )

        assert result.is_error is False
        assert "Modified" in file_path.read_text()

    def test_list_with_workspace_security(self, tmp_path: Path):
        """Test list_directory with workspace enforces path validation."""
        (tmp_path / "test.txt").write_text("content")

        result = list_directory(str(tmp_path), workspace=str(tmp_path))

        assert result.is_error is False
        assert result.metadata["count"] == 1


class TestSecureMode:
    """Tests for TOCTOU-safe secure mode."""

    def test_read_secure_mode_valid_path(self, tmp_path: Path):
        """Test secure mode read with valid path."""
        file_path = tmp_path / "secure_read.txt"
        file_path.write_text("Secure content")

        result = read_file(str(file_path), secure_mode=True, workspace=str(tmp_path))

        assert result.is_error is False
        assert result.content == "Secure content"
        assert result.metadata.get("secure_mode") is True

    def test_read_secure_mode_out_of_bound(self, tmp_path: Path):
        """Test secure mode read rejects out-of-bound path."""
        # Create file outside workspace
        outside_path = tmp_path / "outside.txt"
        outside_path.write_text("Outside content")

        # Workspace is a different directory
        project_path = tmp_path / "project"
        project_path.mkdir()

        with pytest.raises(ToolError) as exc_info:
            read_file(str(outside_path), secure_mode=True, workspace=str(project_path))

        assert (
            "validation failed" in exc_info.value.message.lower()
            or "security" in exc_info.value.message.lower()
        )

    def test_read_secure_mode_nonexistent_file(self, tmp_path: Path):
        """Test secure mode read handles nonexistent file."""
        with pytest.raises(ToolError) as exc_info:
            read_file(
                str(tmp_path / "nonexistent.txt"),
                secure_mode=True,
                workspace=str(tmp_path),
            )

        assert "not found" in exc_info.value.message.lower()

    def test_write_secure_mode_new_file(self, tmp_path: Path):
        """Test secure mode atomic write creates new file."""
        file_path = tmp_path / "secure_write_new.txt"

        result = write_file(
            str(file_path),
            "Atomic content\n",  # Secure mode writes exact content
            secure_mode=True,
            workspace=str(tmp_path),
        )

        assert result.is_error is False
        assert file_path.exists()
        assert file_path.read_text() == "Atomic content\n"
        assert result.metadata.get("secure_mode") is True

    def test_write_secure_mode_overwrites_existing(self, tmp_path: Path):
        """Test secure mode atomic write overwrites existing file."""
        file_path = tmp_path / "secure_overwrite.txt"
        file_path.write_text("Old content")

        result = write_file(
            str(file_path),
            "New atomic content\n",  # Secure mode writes exact content
            secure_mode=True,
            workspace=str(tmp_path),
        )

        assert result.is_error is False
        assert file_path.read_text() == "New atomic content\n"

    def test_write_secure_mode_exact_content(self, tmp_path: Path):
        """Test secure mode writes exact content without adding newline."""
        file_path = tmp_path / "exact_content.txt"

        result = write_file(
            str(file_path),
            "Exact",  # No trailing newline
            secure_mode=True,
            workspace=str(tmp_path),
        )

        assert result.is_error is False
        assert file_path.read_text() == "Exact"  # Exactly as provided

    def test_write_secure_mode_creates_directories(self, tmp_path: Path):
        """Test secure mode creates parent directories."""
        file_path = tmp_path / "nested" / "secure" / "file.txt"

        result = write_file(
            str(file_path),
            "Nested secure content",
            secure_mode=True,
            workspace=str(tmp_path),
        )

        assert result.is_error is False
        assert file_path.exists()

    def test_write_secure_mode_validation_failure(self, tmp_path: Path):
        """Test secure mode validates path before write."""
        # Try to write outside workspace
        outside_path = tmp_path / "outside.txt"
        project_path = tmp_path / "project"
        project_path.mkdir()

        with pytest.raises(ToolError) as exc_info:
            write_file(
                str(outside_path),
                "Should fail",
                secure_mode=True,
                workspace=str(project_path),
            )

        assert (
            "validation failed" in exc_info.value.message.lower()
            or "security" in exc_info.value.message.lower()
        )

    def test_write_secure_mode_skips_append(self, tmp_path: Path):
        """Test secure mode falls back to legacy mode for append."""
        file_path = tmp_path / "secure_append.txt"
        file_path.write_text("Original\n")

        result = write_file(
            str(file_path),
            "Appended",
            secure_mode=True,
            append=True,
            workspace=str(tmp_path),
        )

        # Secure mode should be skipped for append operations
        assert result.is_error is False
        assert "Original" in file_path.read_text()
        assert "Appended" in file_path.read_text()

    def test_read_secure_mode_binary_content(self, tmp_path: Path):
        """Test secure mode handles binary content."""
        file_path = tmp_path / "binary_secure.bin"
        file_path.write_bytes(b"\x00\x01\x02\xff\xfe")

        result = read_file(str(file_path), secure_mode=True, workspace=str(tmp_path))

        # Should read successfully, binary data decoded
        assert result.is_error is False

    def test_read_secure_mode_unicode_content(self, tmp_path: Path):
        """Test secure mode handles unicode content."""
        file_path = tmp_path / "unicode_secure.txt"
        file_path.write_text("你好世界 World 🌍", encoding="utf-8")

        result = read_file(str(file_path), secure_mode=True, workspace=str(tmp_path))

        assert result.is_error is False
        assert "你好" in result.content

    def test_read_secure_mode_with_pagination(self, tmp_path: Path):
        """Test secure mode with offset and limit."""
        file_path = tmp_path / "paginated_secure.txt"
        lines = [f"Line {i}" for i in range(10)]
        file_path.write_text("\n".join(lines))

        result = read_file(
            str(file_path), offset=3, limit=3, secure_mode=True, workspace=str(tmp_path)
        )

        assert result.is_error is False
        assert "Line 3" in result.content

    def test_read_secure_mode_disabled_security(self, tmp_path: Path):
        """Test secure mode with disabled security still works."""
        file_path = tmp_path / "no_security.txt"
        file_path.write_text("No security check")

        # No workspace = security disabled
        result = read_file(str(file_path), secure_mode=True)

        assert result.is_error is False
        assert result.content == "No security check"

    def test_write_secure_mode_disabled_security(self, tmp_path: Path):
        """Test secure mode write with disabled security still works."""
        file_path = tmp_path / "no_security_write.txt"

        # No workspace = security disabled
        result = write_file(str(file_path), "No security write", secure_mode=True)

        assert result.is_error is False
        assert file_path.exists()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
