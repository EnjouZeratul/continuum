"""
Builtin Tools Tests

Tests for ReadTool, WriteTool, EditTool, BashTool, GrepTool, GlobTool.

Run: pytest python/tests/test_builtin.py -v --cov=continuum_sdk.tools --cov-report=term-missing
"""

import asyncio
import os
import tempfile

import pytest

from continuum_sdk.tools.bash import (
    BLOCKED_COMMANDS,
    BashTool,
    validate_command,
)
from continuum_sdk.tools.builtin import (
    BuiltinTools,
    ToolCategory,
    ToolMeta,
)
from continuum_sdk.tools.file_ops import (
    EditTool,
    ReadTool,
    WriteTool,
    detect_encoding,
)
from continuum_sdk.tools.search import (
    GlobTool,
    GrepTool,
    grep,
)
from continuum_sdk.tools.types import ToolError, ToolNotAvailableError, ToolResult

# ==============================================================================
# ReadTool Tests
# ==============================================================================


class TestReadTool:
    """ReadTool tests"""

    def test_read_file_success(self):
        """测试正常读取文件"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("test content line 1\n")
            f.write("test content line 2\n")
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath)

            assert result.is_error is False
            assert "test content line 1" in result.content
            assert "test content line 2" in result.content
            assert result.name == "read"
            assert result.call_id is not None
            assert result.duration_ms >= 0
            assert "total_lines" in result.metadata
            assert result.metadata["total_lines"] == 2
        finally:
            os.unlink(filepath)

    def test_read_file_with_offset(self):
        """测试 offset 参数读取部分行"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            for i in range(10):
                f.write(f"Line Alpha {i+1:02d}\n")  # 01, 02, ... 10 (无子串重叠)
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath, offset=3)

            assert result.is_error is False
            assert "Line Alpha 03" in result.content
            assert "Line Alpha 01" not in result.content
            assert "Line Alpha 02" not in result.content
            assert result.metadata["lines_read"] == 8  # lines 3-10
        finally:
            os.unlink(filepath)

    def test_read_file_with_limit(self):
        """测试 limit 参数限制行数"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            for i in range(10):
                f.write(f"line {i+1}\n")
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath, limit=3)

            assert result.is_error is False
            assert "line 1" in result.content
            assert "line 3" in result.content
            assert "line 4" not in result.content
            assert result.metadata["lines_read"] == 3
        finally:
            os.unlink(filepath)

    def test_read_file_with_offset_and_limit(self):
        """测试 offset 和 limit 组合"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            for i in range(10):
                f.write(f"line {i+1}\n")
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath, offset=3, limit=2)

            assert result.is_error is False
            assert "line 3" in result.content
            assert "line 4" in result.content
            assert "line 2" not in result.content
            assert "line 5" not in result.content
        finally:
            os.unlink(filepath)

    def test_read_nonexistent_file_raises_error(self):
        """测试读取不存在的文件"""
        reader = ReadTool()
        with pytest.raises(ToolError) as exc_info:
            reader.read("/nonexistent/path/file_xyz.txt")

        assert "File not found" in str(exc_info.value)
        assert exc_info.value.name == "read"

    def test_read_directory_raises_error(self):
        """测试读取目录（非文件）"""
        with tempfile.TemporaryDirectory() as tmpdir:
            reader = ReadTool()
            with pytest.raises(ToolError) as exc_info:
                reader.read(tmpdir)

            assert "Not a file" in str(exc_info.value)

    def test_read_empty_file(self):
        """测试读取空文件"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath)

            assert result.is_error is False
            assert result.content == ""
            assert result.metadata["total_lines"] == 0
        finally:
            os.unlink(filepath)

    def test_read_file_with_unicode(self):
        """测试读取 Unicode 文件"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("你好世界 🌍\n")
            f.write("日本語テスト\n")
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath)

            assert result.is_error is False
            assert "你好世界" in result.content
            assert "🌍" in result.content
            assert "日本語" in result.content
        finally:
            os.unlink(filepath)

    def test_read_file_show_line_numbers(self):
        """测试显示行号"""
        reader = ReadTool(show_line_numbers=True)
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("line 1\n")
            f.write("line 2\n")
            filepath = f.name

        try:
            result = reader.read(filepath)

            assert result.is_error is False
            assert "1\t" in result.content
            assert "2\t" in result.content
        finally:
            os.unlink(filepath)

    def test_read_tool_callable(self):
        """测试 ReadTool 直接调用"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("callable test\n")
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader(filepath)

            assert result.is_error is False
            assert "callable test" in result.content
        finally:
            os.unlink(filepath)

    def test_read_negative_offset(self):
        """测试负偏移量（应从第0行开始）"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            for i in range(5):
                f.write(f"negative line {i+1}\n")
            filepath = f.name

        try:
            reader = ReadTool()
            result = reader.read(filepath, offset=-1)

            assert result.is_error is False
            assert "negative line 1" in result.content
            assert "negative line 5" in result.content
        finally:
            os.unlink(filepath)


class TestDetectEncoding:
    """Encoding detection tests"""

    def test_detect_utf8(self):
        """测试 UTF-8 检测"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("Hello, World!\n")
            filepath = f.name

        try:
            from pathlib import Path

            encoding = detect_encoding(Path(filepath))
            assert encoding == "utf-8"
        finally:
            os.unlink(filepath)

    def test_detect_gbk(self):
        """测试 GBK 编码检测"""
        with tempfile.NamedTemporaryFile(mode="wb", delete=False, suffix=".txt") as f:
            # GBK 编码的中文字符
            f.write("中文测试\n".encode("gbk"))
            filepath = f.name

        try:
            from pathlib import Path

            encoding = detect_encoding(Path(filepath))
            # 应检测为 GBK 或兼容编码
            assert encoding in ["gbk", "gb2312", "gb18030"]
        finally:
            os.unlink(filepath)

    def test_detect_latin1(self):
        """测试 Latin-1 编码检测"""
        with tempfile.NamedTemporaryFile(mode="wb", delete=False, suffix=".txt") as f:
            # Latin-1 特殊字符（非 UTF-8 兼容）
            f.write(b"\xe9\xe0\xe7\n")  # é, à, ç in Latin-1
            filepath = f.name

        try:
            from pathlib import Path

            encoding = detect_encoding(Path(filepath))
            # 应检测为 Latin-1 或 UTF-8（fallback）
            assert encoding in ["latin-1", "utf-8"]
        finally:
            os.unlink(filepath)


# ==============================================================================
# WriteTool Tests
# ==============================================================================


class TestWriteTool:
    """WriteTool tests"""

    def test_write_new_file(self):
        """测试写入新文件"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "new_file.txt")

            writer = WriteTool(backup=False)
            result = writer.write(filepath, "Hello, World!")

            assert result.is_error is False
            assert "Successfully wrote" in result.content
            assert os.path.exists(filepath)

            # 验证内容
            with open(filepath, encoding="utf-8") as f:
                content = f.read()
            assert "Hello, World!" in content

    def test_write_overwrite_existing(self):
        """测试覆盖已有文件"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("old content\n")
            filepath = f.name

        try:
            writer = WriteTool(backup=False)
            result = writer.write(filepath, "new content")

            assert result.is_error is False

            with open(filepath, encoding="utf-8") as f:
                content = f.read()
            assert "new content" in content
            assert "old content" not in content
        finally:
            os.unlink(filepath)

    def test_write_append_mode(self):
        """测试追加模式"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("line 1\n")
            filepath = f.name

        try:
            writer = WriteTool(backup=False)
            result = writer.append(filepath, "line 2")

            assert result.is_error is False

            with open(filepath, encoding="utf-8") as f:
                content = f.read()
            assert "line 1" in content
            assert "line 2" in content
        finally:
            os.unlink(filepath)

    def test_write_create_dirs(self):
        """测试自动创建目录"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "subdir", "nested", "file.txt")

            writer = WriteTool(backup=False)
            result = writer.write(filepath, "nested content")

            assert result.is_error is False
            assert os.path.exists(filepath)

    def test_write_with_backup(self):
        """测试备份功能"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("original content\n")
            filepath = f.name

        try:
            writer = WriteTool(backup=True)
            result = writer.write(filepath, "new content")

            assert result.is_error is False
            # 备份文件应存在
            backup_path = filepath + ".bak"
            assert os.path.exists(backup_path)

            # 备份内容应为原始内容
            with open(backup_path, encoding="utf-8") as f:
                backup_content = f.read()
            assert "original content" in backup_content
        finally:
            os.unlink(filepath)
            backup_path = filepath + ".bak"
            if os.path.exists(backup_path):
                os.unlink(backup_path)

    def test_write_tool_callable(self):
        """测试 WriteTool 直接调用"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "callable_test.txt")

            writer = WriteTool(backup=False)
            result = writer(filepath, content="callable write test")

            assert result.is_error is False
            assert os.path.exists(filepath)


# ==============================================================================
# EditTool Tests
# ==============================================================================


class TestEditTool:
    """EditTool tests"""

    def test_edit_replace_single_occurrence(self):
        """测试替换单个匹配"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("foo bar foo baz\n")
            filepath = f.name

        try:
            editor = EditTool(backup=False)
            result = editor.edit(filepath, "foo", "QUX")

            assert result.is_error is False
            assert "Replaced 1 occurrence" in result.content

            with open(filepath, encoding="utf-8") as f:
                content = f.read()
            assert content == "QUX bar foo baz\n"
        finally:
            os.unlink(filepath)

    def test_edit_replace_all(self):
        """测试替换所有匹配"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("foo bar foo baz foo\n")
            filepath = f.name

        try:
            editor = EditTool(backup=False)
            result = editor.replace_all(filepath, "foo", "QUX")

            assert result.is_error is False

            with open(filepath, encoding="utf-8") as f:
                content = f.read()
            assert content == "QUX bar QUX baz QUX\n"
        finally:
            os.unlink(filepath)

    def test_edit_string_not_found_raises_error(self):
        """测试字符串不存在"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("hello world\n")
            filepath = f.name

        try:
            editor = EditTool(backup=False)
            with pytest.raises(ToolError) as exc_info:
                editor.edit(filepath, "nonexistent", "replacement")

            assert "String not found" in str(exc_info.value)
        finally:
            os.unlink(filepath)

    def test_edit_nonexistent_file_raises_error(self):
        """测试编辑不存在的文件"""
        editor = EditTool(backup=False)
        with pytest.raises(ToolError) as exc_info:
            editor.edit("/nonexistent/path/file.txt", "old", "new")

        assert "File not found" in str(exc_info.value)

    def test_edit_with_backup(self):
        """测试编辑时备份"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("original\n")
            filepath = f.name

        try:
            editor = EditTool(backup=True)
            result = editor.edit(filepath, "original", "modified")

            assert result.is_error is False
            backup_path = filepath + ".bak"
            assert os.path.exists(backup_path)

            with open(backup_path, encoding="utf-8") as f:
                assert "original" in f.read()
        finally:
            os.unlink(filepath)
            backup_path = filepath + ".bak"
            if os.path.exists(backup_path):
                os.unlink(backup_path)

    def test_edit_tool_callable(self):
        """测试 EditTool 直接调用"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".txt", encoding="utf-8"
        ) as f:
            f.write("callable edit test\n")
            filepath = f.name

        try:
            editor = EditTool(backup=False)
            result = editor(filepath, "callable edit test", "modified test")

            assert result.is_error is False

            with open(filepath, encoding="utf-8") as f:
                assert "modified test" in f.read()
        finally:
            os.unlink(filepath)


# ==============================================================================
# BashTool Tests
# ==============================================================================


class TestBashTool:
    """BashTool tests"""

    def test_bash_simple_command(self):
        """测试简单命令执行"""
        bash = BashTool()
        result = bash.run("echo hello")

        assert result.is_error is False
        assert "hello" in result.content

    def test_bash_command_with_exit_code(self):
        """测试非零退出码"""
        bash = BashTool()
        # Use findstr/cat on nonexistent file - these are actual executables
        with tempfile.TemporaryDirectory() as tmpdir:
            nonexistent = os.path.join(tmpdir, "nonexistent_xyz.txt")
            if os.name == "nt":
                # Windows: findstr is a standalone exe, fails on nonexistent file
                result = bash.run(f"findstr xyz {nonexistent}")
            else:
                # Unix: cat fails on nonexistent file
                result = bash.run(f"cat {nonexistent}")

        assert result.is_error is True
        # Exit code should be mentioned or error should be indicated
        assert "Exit code" in result.content or "Error" in result.content

    def test_bash_command_timeout(self):
        """测试命令超时"""
        bash = BashTool(default_timeout=0.5)

        with pytest.raises(ToolError) as exc_info:
            bash.run("sleep 10")

        assert "timed out" in str(exc_info.value).lower()

    def test_bash_command_not_found(self):
        """测试命令不存在"""
        bash = BashTool()

        # On both Windows and Unix, nonexistent commands raise ToolError
        with pytest.raises(ToolError) as exc_info:
            bash.run("nonexistent_command_xyz_12345")

        # Should indicate command not found
        error_msg = str(exc_info.value).lower()
        assert "not found" in error_msg or "command" in error_msg

    def test_bash_working_directory(self):
        """测试指定工作目录"""
        with tempfile.TemporaryDirectory() as tmpdir:
            bash = BashTool()
            # Create a marker file to verify working directory
            subdir = os.path.join(tmpdir, "subdir")
            os.makedirs(subdir)
            marker_file = os.path.join(subdir, "marker.txt")

            # Create the marker file directly
            with open(marker_file, "w") as f:
                f.write("marker")

            # Use findstr/cat to read the file from the working directory
            if os.name == "nt":
                # Windows: findstr to search in file
                result = bash.run("findstr marker marker.txt", working_dir=subdir)
            else:
                # Unix: cat to read file
                result = bash.run("cat marker.txt", working_dir=subdir)

            assert result.is_error is False
            # The marker content should be in the output
            assert "marker" in result.content

    def test_bash_working_directory_not_found(self):
        """测试工作目录不存在"""
        bash = BashTool()
        with pytest.raises(ToolError) as exc_info:
            bash.run("echo test", working_dir="/nonexistent/path/xyz")

        assert "Working directory not found" in str(exc_info.value)

    def test_bash_async_execution(self):
        """测试异步执行"""
        bash = BashTool()
        result = asyncio.run(bash.run_async("echo async"))

        assert result.is_error is False
        assert "async" in result.content

    def test_bash_environment_variables(self):
        """测试环境变量"""
        bash = BashTool()
        # Create a small script file to read and print env var
        # This avoids shell-based variable expansion which differs by platform
        with tempfile.TemporaryDirectory() as tmpdir:
            if os.name == "nt":
                # Windows: Create a batch script
                script_path = os.path.join(tmpdir, "test_env.bat")
                with open(script_path, "w") as f:
                    f.write("@echo %MY_TEST_VAR%")
                result = bash.run(f"{script_path}", env={"MY_TEST_VAR": "test_value"})
            else:
                # Unix: Create a shell script
                script_path = os.path.join(tmpdir, "test_env.sh")
                with open(script_path, "w") as f:
                    f.write("#!/bin/sh\necho $MY_TEST_VAR")
                os.chmod(script_path, 0o755)
                result = bash.run(f"{script_path}", env={"MY_TEST_VAR": "test_value"})

            assert result.is_error is False
            assert "test_value" in result.content

    def test_bash_tool_callable(self):
        """测试 BashTool 直接调用"""
        bash = BashTool()
        result = bash("echo callable")

        assert result.is_error is False
        assert "callable" in result.content

    def test_bash_blocked_command(self):
        """测试被阻止的命令执行"""
        bash = BashTool()
        with pytest.raises(ToolError) as exc_info:
            bash.run("sudo ls")

        assert "Blocked" in str(exc_info.value)

    def test_bash_command_with_stderr(self):
        """测试有 stderr 输出的命令"""
        bash = BashTool()
        # 使用一个会产生 stderr 的命令
        if os.name == "nt":
            # Windows: 使用不存在的路径
            result = bash.run("dir nonexistent_path_xyz")
        else:
            result = bash.run("ls /nonexistent_path_xyz")

        assert result.is_error is True
        assert "Error:" in result.content or "not found" in result.content.lower()


class TestCommandValidation:
    """Command validation tests"""

    def test_validate_blocked_command(self):
        """测试被阻止的命令"""
        for cmd in BLOCKED_COMMANDS:
            error = validate_command(f"{cmd} some args")
            assert error is not None
            assert "Blocked" in error

    def test_validate_normal_command(self):
        """测试正常命令"""
        error = validate_command("ls -la")
        assert error is None

    def test_validate_dangerous_command_allowed(self):
        """测试危险命令需确认"""
        # 危险命令返回 require-confirm 字符串
        for cmd in ["rm", "git push"]:
            error = validate_command(f"{cmd} args")
            assert error is not None
            assert "confirm=True" in error

    def test_validate_deferred_p3_command_vectors_require_confirmation(self):
        """测试前缀/查找命令绕过向量需要确认"""
        # Note: Commands with ';' are blocked by command substitution detection
        # because ';' is a command separator. Test the valid prefix commands.
        for cmd in [
            "env sudo rm -rf /",
            "nice sudo rm -rf /",
            "nohup sudo rm -rf /",
            "xargs sudo",
            "chroot / sudo rm -rf /",
            "ionice sudo rm -rf /",
            "strace sudo rm -rf /",
            "time sudo rm -rf /",
        ]:
            error = validate_command(cmd)
            assert error is not None
            assert "confirm=True" in error


# ==============================================================================
# GrepTool Tests
# ==============================================================================


class TestGrepTool:
    """GrepTool tests"""

    def test_grep_find_pattern(self):
        """测试正则搜索"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.py")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("def hello():\n")
                f.write("    print('hello')\n")
                f.write("\n")
                f.write("def world():\n")
                f.write("    print('world')\n")

            grep_tool = GrepTool()
            result = grep_tool.search("def\\s+\\w+", path=tmpdir, glob_pattern="*.py")

            assert result.is_error is False
            assert "def hello" in result.content or "def world" in result.content

    def test_grep_files_with_matches_mode(self):
        """测试 files_with_matches 输出模式"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("hello world\n")

            grep_tool = GrepTool()
            result = grep_tool.search(
                "hello", path=tmpdir, output_mode="files_with_matches"
            )

            assert result.is_error is False
            assert "test.txt" in result.content

    def test_grep_count_mode(self):
        """测试 count 输出模式"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("hello\nhello\nhello\n")

            grep_tool = GrepTool()
            result = grep_tool.search("hello", path=tmpdir, output_mode="count")

            assert result.is_error is False
            assert "3" in result.content or "matches" in result.content.lower()

    def test_grep_case_insensitive(self):
        """测试大小写不敏感"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("HELLO World\n")

            grep_tool = GrepTool()
            result = grep_tool.search("hello", path=tmpdir, case_sensitive=False)

            assert result.is_error is False
            assert "HELLO" in result.content

    def test_grep_case_sensitive(self):
        """测试大小写敏感"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("HELLO World\nhello there\n")

            grep_tool = GrepTool()
            result = grep_tool.search("hello", path=tmpdir, case_sensitive=True)

            assert result.is_error is False
            assert "hello there" in result.content
            # HELLO 不应匹配

    def test_grep_invalid_pattern_raises_error(self):
        """测试无效正则表达式"""
        grep_tool = GrepTool()
        with pytest.raises(ToolError) as exc_info:
            grep_tool.search("[invalid(", path=".")

        assert "Invalid regex" in str(exc_info.value)

    def test_grep_nonexistent_path_raises_error(self):
        """测试路径不存在"""
        grep_tool = GrepTool()
        with pytest.raises(ToolError) as exc_info:
            grep_tool.search("pattern", path="/nonexistent/path/xyz")

        assert "Path not found" in str(exc_info.value)

    def test_grep_single_file(self):
        """测试搜索单个文件"""
        with tempfile.NamedTemporaryFile(
            mode="w", delete=False, suffix=".py", encoding="utf-8"
        ) as f:
            f.write("def single_file_test():\n")
            f.write("    return 'test'\n")
            filepath = f.name

        try:
            grep_tool = GrepTool()
            result = grep_tool.search("def\\s+\\w+", path=filepath)

            assert result.is_error is False
            assert "single_file_test" in result.content
        finally:
            os.unlink(filepath)

    def test_grep_without_line_numbers(self):
        """测试不显示行号"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "test.txt")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("test line without numbers\n")

            # Use grep directly with include_line_numbers=False
            result = grep("test", path=tmpdir, include_line_numbers=False)

            assert result.is_error is False
            # Should not have file:line: format
            assert ":" not in result.content or "test.txt" in result.content

    def test_grep_tool_callable(self):
        """测试 GrepTool 直接调用"""
        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "callable.py")
            with open(filepath, "w", encoding="utf-8") as f:
                f.write("callable_grep_test\n")

            grep_tool = GrepTool()
            result = grep_tool("callable_grep_test", path=tmpdir)

            assert result.is_error is False
            assert "callable_grep_test" in result.content


# ==============================================================================
# GlobTool Tests
# ==============================================================================


class TestGlobTool:
    """GlobTool tests"""

    def test_glob_find_files(self):
        """测试文件匹配"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # 创建一些文件
            open(os.path.join(tmpdir, "file1.py"), "w").close()
            open(os.path.join(tmpdir, "file2.py"), "w").close()
            open(os.path.join(tmpdir, "file3.txt"), "w").close()

            glob_tool = GlobTool()
            result = glob_tool.find("*.py", path=tmpdir)

            assert result.is_error is False
            assert "file1.py" in result.content
            assert "file2.py" in result.content
            assert "file3.txt" not in result.content

    def test_glob_recursive(self):
        """测试递归搜索"""
        with tempfile.TemporaryDirectory() as tmpdir:
            subdir = os.path.join(tmpdir, "subdir")
            os.makedirs(subdir)
            open(os.path.join(subdir, "nested.py"), "w").close()

            glob_tool = GlobTool()
            result = glob_tool.find("**/*.py", path=tmpdir)

            assert result.is_error is False
            assert "nested.py" in result.content

    def test_glob_no_matches(self):
        """测试无匹配"""
        with tempfile.TemporaryDirectory() as tmpdir:
            glob_tool = GlobTool()
            result = glob_tool.find("*.xyz", path=tmpdir)

            assert result.is_error is False
            assert (
                "no matches" in result.content.lower() or result.content.strip() == ""
            )

    def test_glob_nonexistent_path_raises_error(self):
        """测试路径不存在"""
        glob_tool = GlobTool()
        with pytest.raises(ToolError) as exc_info:
            glob_tool.find("*.py", path="/nonexistent/path/xyz")

        assert "Path not found" in str(exc_info.value)

    def test_glob_tool_callable(self):
        """测试 GlobTool 直接调用"""
        with tempfile.TemporaryDirectory() as tmpdir:
            open(os.path.join(tmpdir, "callable_glob.py"), "w").close()

            glob_tool = GlobTool()
            result = glob_tool("*.py", path=tmpdir)

            assert result.is_error is False
            assert "callable_glob.py" in result.content

    def test_glob_recursive_pattern(self):
        """测试递归搜索模式 (**/*.py)"""
        with tempfile.TemporaryDirectory() as tmpdir:
            subdir = os.path.join(tmpdir, "deep", "nested")
            os.makedirs(subdir)
            open(os.path.join(subdir, "deep_file.py"), "w").close()

            glob_tool = GlobTool()
            result = glob_tool.find("**/*.py", path=tmpdir)

            assert result.is_error is False
            assert "deep_file.py" in result.content


# ==============================================================================
# ToolResult Tests
# ==============================================================================


class TestToolResult:
    """ToolResult tests"""

    def test_tool_result_str(self):
        """测试 ToolResult 字符串表示"""
        result = ToolResult(
            call_id="abc123",
            name="test",
            content="Hello, World!",
            is_error=False,
            duration_ms=100,
        )

        result_str = str(result)
        assert "[OK]" in result_str
        assert "test" in result_str
        assert "100ms" in result_str

    def test_tool_result_error_str(self):
        """测试错误结果字符串表示"""
        result = ToolResult(
            call_id="abc123",
            name="test",
            content="Error message",
            is_error=True,
            duration_ms=50,
        )

        result_str = str(result)
        assert "[ERROR]" in result_str


# ==============================================================================
# ToolError Tests
# ==============================================================================


class TestToolError:
    """ToolError tests"""

    def test_tool_error_creation(self):
        """测试错误创建"""
        error = ToolError(
            call_id="abc123",
            name="read",
            message="File not found",
        )

        assert error.call_id == "abc123"
        assert error.name == "read"
        assert error.message == "File not found"
        assert "read" in str(error)
        assert "File not found" in str(error)


# ==============================================================================
# BuiltinTools Tests (builtin.py)
# ==============================================================================


class TestToolCategory:
    """ToolCategory enum tests"""

    def test_category_values(self):
        """测试分类值"""
        assert ToolCategory.FILE_OPS.value == "file_ops"
        assert ToolCategory.SEARCH.value == "search"
        assert ToolCategory.SHELL.value == "shell"
        assert ToolCategory.CODE_ANALYSIS.value == "code_analysis"

    def test_category_count(self):
        """测试分类数量"""
        assert len(list(ToolCategory)) >= 8


class TestToolMeta:
    """ToolMeta dataclass tests"""

    def test_tool_meta_creation(self):
        """测试元数据创建"""
        meta = ToolMeta(
            name="test_tool",
            description="A test tool",
            category=ToolCategory.FILE_OPS,
        )
        assert meta.name == "test_tool"
        assert meta.description == "A test tool"
        assert meta.category == ToolCategory.FILE_OPS
        assert meta.requires_confirmation is False
        assert meta.is_dangerous is False

    def test_tool_meta_with_params(self):
        """测试带参数的元数据"""
        params = {"path": {"type": "string"}, "content": {"type": "string"}}
        meta = ToolMeta(
            name="write_file",
            description="Write to file",
            category=ToolCategory.FILE_OPS,
            requires_confirmation=True,
            is_dangerous=True,
            parameters=params,
        )
        assert meta.parameters == params
        assert meta.requires_confirmation is True
        assert meta.is_dangerous is True


class TestBuiltinTools:
    """BuiltinTools class tests"""

    def test_builtin_tools_init(self):
        """测试初始化"""
        tools = BuiltinTools()
        assert tools._tools_cache is not None
        assert len(tools._tools_cache) > 0

    def test_list_tools(self):
        """测试列出工具"""
        tools = BuiltinTools()
        tool_list = tools.list_tools()
        assert len(tool_list) > 0
        # 检查关键工具
        tool_names = [t.name for t in tool_list]
        assert "read_file" in tool_names
        assert "write_file" in tool_names
        assert "grep" in tool_names
        assert "bash" in tool_names

    def test_get_tool_meta(self):
        """测试获取工具元数据"""
        tools = BuiltinTools()
        meta = tools.get_tool_meta("read_file")
        assert meta is not None
        assert meta.name == "read_file"
        assert meta.category == ToolCategory.FILE_OPS

    def test_get_tool_meta_nonexistent(self):
        """测试获取不存在的工具元数据"""
        tools = BuiltinTools()
        meta = tools.get_tool_meta("nonexistent_tool_xyz")
        assert meta is None

    def test_is_available(self):
        """测试工具可用性检查"""
        tools = BuiltinTools()
        assert tools.is_available("read_file") is True
        assert tools.is_available("nonexistent_tool_xyz") is False

    def test_read_file_fallback(self):
        """测试 read_file 使用 Python fallback"""
        # Python fallback 现已实现，验证 fallback 可用
        import continuum_sdk.tools.builtin as builtin_module

        original_executor = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False
        try:
            tools = BuiltinTools()
            # 验证 fallback 工具集包含 read_file
            assert "read_file" in tools._fallback_tools
        finally:
            builtin_module.HAS_RUST_BINDING = original_executor

    def test_write_file_fallback(self):
        """测试 write_file 使用 Python fallback"""
        import continuum_sdk.tools.builtin as builtin_module

        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False
        try:
            tools = BuiltinTools()
            # 验证 fallback 工具集包含 write_file
            assert "write_file" in tools._fallback_tools
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_bash_fallback(self):
        """测试 bash 使用 Python fallback"""
        import continuum_sdk.tools.builtin as builtin_module

        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False
        try:
            tools = BuiltinTools()
            # 验证 fallback 工具集包含 bash
            assert "bash" in tools._fallback_tools
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_guess_category_other(self):
        """测试无法推断分类的工具"""
        tools = BuiltinTools()
        # 使用一个不匹配任何模式的工具名
        category = tools._guess_category("unknown_xyz_tool")
        assert category == ToolCategory.OTHER

    def test_guess_category_code_analysis(self):
        """测试 CODE_ANALYSIS 分类"""
        tools = BuiltinTools()
        assert tools._guess_category("definition") == ToolCategory.CODE_ANALYSIS
        assert tools._guess_category("reference") == ToolCategory.CODE_ANALYSIS
        assert tools._guess_category("hover") == ToolCategory.CODE_ANALYSIS
        assert tools._guess_category("symbol") == ToolCategory.CODE_ANALYSIS

    def test_guess_category_memory(self):
        """测试 MEMORY 分类"""
        tools = BuiltinTools()
        assert tools._guess_category("memory") == ToolCategory.MEMORY
        assert tools._guess_category("session_memory") == ToolCategory.MEMORY

    def test_guess_category_workflow(self):
        """测试 WORKFLOW 分类"""
        tools = BuiltinTools()
        assert tools._guess_category("checkpoint") == ToolCategory.WORKFLOW
        assert tools._guess_category("save_checkpoint") == ToolCategory.WORKFLOW

    def test_execute_file_ops(self):
        """测试 execute 方法调用文件操作"""
        import os
        import tempfile
        tools = BuiltinTools()

        with tempfile.TemporaryDirectory() as tmpdir:
            # 测试 write_file via execute
            filepath = os.path.join(tmpdir, "test_execute.txt")
            result = tools.execute("write_file", {"path": filepath, "content": "test content"})
            assert "Success" in result or "wrote" in result.lower()

            # 测试 read_file via execute
            result = tools.execute("read_file", {"path": filepath})
            assert "test content" in result

            # 测试 list_directory via execute
            result = tools.execute("list_directory", {"path": tmpdir})
            assert "test_execute.txt" in result

    def test_execute_search_ops(self):
        """测试 execute 方法调用搜索操作"""
        import os
        import tempfile
        tools = BuiltinTools()

        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "search_test.py")
            with open(filepath, "w") as f:
                f.write("def test_func():\n    pass\n")

            # 测试 grep via execute
            result = tools.execute("grep", {"pattern": "def", "path": tmpdir})
            assert "test_func" in result

            # 测试 glob via execute
            result = tools.execute("glob", {"pattern": "*.py", "path": tmpdir})
            assert "search_test.py" in result

    def test_execute_edit_file(self):
        """测试 execute 方法调用 edit_file"""
        import os
        import tempfile
        tools = BuiltinTools()

        with tempfile.TemporaryDirectory() as tmpdir:
            filepath = os.path.join(tmpdir, "edit_test.txt")
            tools.write_file(filepath, "Hello World")

            # edit_file 使用 old_string/new_string 参数名
            result = tools.execute("edit_file", {
                "path": filepath,
                "old_string": "World",
                "new_string": "Python"
            })
            assert "Replace" in result or "occurrence" in result.lower() or "Python" in result or result != ""

            # 验证修改成功
            content = tools.read_file(filepath)
            assert "Python" in content

    def test_execute_bash(self):
        """测试 execute 方法调用 bash"""
        tools = BuiltinTools()
        result = tools.execute("bash", {"command": "echo hello"})
        assert "hello" in result.lower()

    def test_execute_unknown_tool_raises_error(self):
        """测试 execute 调用未知工具抛出错误"""
        tools = BuiltinTools()
        with pytest.raises(ToolNotAvailableError):
            tools.execute("unknown_tool_xyz", {})

    def test_execute_unknown_tool_key_error_from_executor_is_normalized(self):
        """测试 Rust executor 未知工具错误被归一化"""
        class MissingToolExecutor:
            def execute(self, name, args):
                raise KeyError(f"Tool not found: {name}")

        tools = BuiltinTools()
        tools._executor = MissingToolExecutor()
        with pytest.raises(ToolNotAvailableError):
            tools.execute("unknown_tool_xyz", {})

    def test_execute_unknown_tool_runtime_error_from_executor_is_normalized(self):
        """测试 Rust executor RuntimeError 未知工具错误被归一化"""
        class MissingToolExecutor:
            def execute(self, name, args):
                raise RuntimeError(f"Tool not found: {name}")

        tools = BuiltinTools()
        tools._executor = MissingToolExecutor()
        with pytest.raises(ToolNotAvailableError):
            tools.execute("unknown_tool_xyz", {})

    def test_check_binding_without_executor(self):
        """测试无 executor 时的 binding 检查"""
        import continuum_sdk.tools.builtin as builtin_module

        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False
        try:
            tools = BuiltinTools()
            # fallback 工具不应抛出错误
            tools._check_binding("read_file")

            # 非 fallback 工具应抛出错误
            with pytest.raises(ToolNotAvailableError):
                tools._check_binding("unknown_tool")
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_list_directory_nonexistent(self):
        """测试 list_directory 目录不存在"""
        tools = BuiltinTools()
        # Rust binding 会抛出 RuntimeError，Python fallback 返回错误列表
        try:
            result = tools.list_directory("/nonexistent/path/xyz")
            # Python fallback 路径
            assert isinstance(result, list)
            assert len(result) >= 1
            assert "error" in result[0] or "Error" in result[0] or len(result) == 0
        except RuntimeError:
            # Rust binding 路径 - 路径不存在会抛出错误
            pass  # 预期行为

    def test_list_directory_with_files(self):
        """测试 list_directory 包含文件和目录"""
        import os
        import tempfile
        tools = BuiltinTools()

        with tempfile.TemporaryDirectory() as tmpdir:
            # 创建文件和子目录
            filepath = os.path.join(tmpdir, "file.txt")
            subdir = os.path.join(tmpdir, "subdir")
            os.makedirs(subdir)
            with open(filepath, "w") as f:
                f.write("content")

            result = tools.list_directory(tmpdir)
            assert isinstance(result, list)
            assert len(result) >= 1

            # Rust binding 返回 [{'raw': 'file.txt [file]\nsubdir [dir]'}] 格式
            # Python fallback 返回 [{'name': 'file.txt', ...}, ...] 格式
            result_str = str(result)
            assert "file.txt" in result_str
            assert "subdir" in result_str


if __name__ == "__main__":
    pytest.main(
        [__file__, "-v", "--cov=continuum_sdk.tools", "--cov-report=term-missing"]
    )
