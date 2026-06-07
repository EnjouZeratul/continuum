"""
Builtin Tools Additional Coverage Tests

Tests to improve builtin.py coverage from 81% to 95%+.

Missing coverage areas:
1. Rust executor paths (mock executor tests)
2. LSP tools fallback (go_to_definition, find_references, get_hover, symbol_search)
3. execute() method for LSP tools
4. JSON decode error in list_directory
5. RuntimeError re-raise in execute()
"""

import json
import os
import tempfile

import pytest

from continuum_sdk.tools.builtin import (
    BuiltinTools,
    ToolCategory,
    ToolMeta,
    get_builtin_tools,
)
from continuum_sdk.tools.types import ToolNotAvailableError


class TestBuiltinToolsFallback:
    """Test fallback mode when Rust binding unavailable"""

    def test_fallback_mode_init(self, monkeypatch):
        """Test initialization in fallback mode"""
        # Force fallback mode
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            assert tools._executor is None
            assert len(tools._tools_cache) > 0
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_list_tools(self, monkeypatch):
        """Test list_tools in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            tool_list = tools.list_tools()
            tool_names = [t.name for t in tool_list]
            assert "read_file" in tool_names
            assert "write_file" in tool_names
            assert "grep" in tool_names
            assert "glob" in tool_names
            assert "bash" in tool_names
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_is_available(self, monkeypatch):
        """Test is_available in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            assert tools.is_available("read_file") is True
            assert tools.is_available("write_file") is True
            assert tools.is_available("nonexistent") is False
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_read_file(self, monkeypatch):
        """Test read_file in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".txt"
            ) as f:
                f.write("test content for fallback")
                path = f.name

            tools = BuiltinTools()
            content = tools.read_file(path)
            assert "test content" in content

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_write_file(self, monkeypatch):
        """Test write_file in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                path = os.path.join(tmpdir, "test_write.txt")

                tools = BuiltinTools()
                result = tools.write_file(path, "fallback write test")

                assert "Successfully" in result or "wrote" in result.lower()
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_edit_file(self, monkeypatch):
        """Test edit_file in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("old_text = 1\n")
                path = f.name

            tools = BuiltinTools()
            result = tools.edit_file(path, "old_text", "new_text")

            # Verify edit happened
            with open(path) as f:
                content = f.read()
            assert "new_text" in content

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_list_directory(self, monkeypatch):
        """Test list_directory in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                # Create some files
                open(os.path.join(tmpdir, "file1.txt"), "w").close()
                open(os.path.join(tmpdir, "file2.py"), "w").close()

                tools = BuiltinTools()
                entries = tools.list_directory(tmpdir)

                assert len(entries) >= 2
                names = [e["name"] for e in entries]
                assert "file1.txt" in names
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_list_directory_not_found(self, monkeypatch):
        """Test list_directory with nonexistent path"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            result = tools.list_directory("/nonexistent/path/xyz")
            assert "error" in result[0] or result == []
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_grep(self, monkeypatch):
        """Test grep in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                path = os.path.join(tmpdir, "test.py")
                with open(path, "w") as f:
                    f.write("def test_function():\n    pass\n")

                tools = BuiltinTools()
                result = tools.grep("def\\s+\\w+", path=path)

                assert "test_function" in result or result.strip() != ""
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_glob(self, monkeypatch):
        """Test glob in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                # Create files
                open(os.path.join(tmpdir, "file.py"), "w").close()
                open(os.path.join(tmpdir, "file.txt"), "w").close()

                tools = BuiltinTools()
                result = tools.glob("*.py", path=tmpdir)

                assert "file.py" in result
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_bash(self, monkeypatch):
        """Test bash in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            result = tools.bash("echo hello")

            assert "hello" in result.lower()
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestBuiltinToolsExecute:
    """Test execute method"""

    def test_execute_read_file(self, monkeypatch):
        """Test execute for read_file"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".txt"
            ) as f:
                f.write("execute test content")
                path = f.name

            tools = BuiltinTools()
            result = tools.execute("read_file", {"path": path})

            assert "execute test" in result

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_write_file(self, monkeypatch):
        """Test execute for write_file"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                path = os.path.join(tmpdir, "exec_write.txt")

                tools = BuiltinTools()
                result = tools.execute("write_file", {"path": path, "content": "test"})

                assert os.path.exists(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_edit_file(self, monkeypatch):
        """Test execute for edit_file"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".txt"
            ) as f:
                f.write("original text")
                path = f.name

            tools = BuiltinTools()
            result = tools.execute("edit_file", {
                "path": path,
                "old": "original",
                "new": "modified",
            })

            with open(path) as f:
                assert "modified" in f.read()

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_list_directory(self, monkeypatch):
        """Test execute for list_directory"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                open(os.path.join(tmpdir, "file.txt"), "w").close()

                tools = BuiltinTools()
                result = tools.execute("list_directory", {"path": tmpdir})

                # Result should be JSON array
                data = json.loads(result)
                assert len(data) >= 1
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_grep(self, monkeypatch):
        """Test execute for grep"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                path = os.path.join(tmpdir, "test.py")
                with open(path, "w") as f:
                    f.write("def grep_target():\n    pass\n")

                tools = BuiltinTools()
                result = tools.execute("grep", {
                    "pattern": "grep_target",
                    "path": tmpdir,
                })

                assert "grep_target" in result or result.strip() != ""
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_glob(self, monkeypatch):
        """Test execute for glob"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                open(os.path.join(tmpdir, "test.py"), "w").close()

                tools = BuiltinTools()
                result = tools.execute("glob", {
                    "pattern": "*.py",
                    "path": tmpdir,
                })

                assert "test.py" in result
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_bash(self, monkeypatch):
        """Test execute for bash"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            result = tools.execute("bash", {"command": "echo execute_bash_test"})

            assert "execute_bash_test" in result
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_unknown_tool(self, monkeypatch):
        """Test execute for unknown tool"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            with pytest.raises(ToolNotAvailableError, match="not available"):
                tools.execute("unknown_tool_xyz", {})
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestToolCategory:
    """Test ToolCategory enum"""

    def test_all_categories(self):
        """Test all category values"""
        assert ToolCategory.FILE_OPS.value == "file_ops"
        assert ToolCategory.SEARCH.value == "search"
        assert ToolCategory.SHELL.value == "shell"
        assert ToolCategory.NETWORK.value == "network"
        assert ToolCategory.CODE_ANALYSIS.value == "code_analysis"
        assert ToolCategory.MEMORY.value == "memory"
        assert ToolCategory.WORKFLOW.value == "workflow"
        assert ToolCategory.SYSTEM.value == "system"
        assert ToolCategory.OTHER.value == "other"


class TestToolMeta:
    """Test ToolMeta dataclass"""

    def test_tool_meta_minimal(self):
        """Test minimal ToolMeta"""
        meta = ToolMeta(name="test", description="A test tool", category=ToolCategory.OTHER)
        assert meta.name == "test"
        assert meta.requires_confirmation is False
        assert meta.is_dangerous is False
        assert meta.parameters == {}

    def test_tool_meta_full(self):
        """Test full ToolMeta"""
        params = {"path": {"type": "string"}}
        meta = ToolMeta(
            name="write",
            description="Write file",
            category=ToolCategory.FILE_OPS,
            requires_confirmation=True,
            is_dangerous=True,
            parameters=params,
        )
        assert meta.requires_confirmation is True
        assert meta.is_dangerous is True
        assert meta.parameters == params


class TestBuiltinToolsSingleton:
    """Test get_builtin_tools singleton"""

    def test_get_builtin_tools_singleton(self):
        """Test singleton creation"""
        from continuum_sdk.tools.builtin import get_builtin_tools, _builtin_tools

        # Reset singleton
        import continuum_sdk.tools.builtin as builtin_module
        builtin_module._builtin_tools = None

        tools1 = get_builtin_tools()
        tools2 = get_builtin_tools()

        assert tools1 is tools2
        assert isinstance(tools1, BuiltinTools)

    def test_singleton_persistence(self):
        """Test singleton persists across calls"""
        from continuum_sdk.tools.builtin import get_builtin_tools

        tools1 = get_builtin_tools()
        tools2 = get_builtin_tools()

        # Same instance
        assert id(tools1) == id(tools2)


class TestBuiltinToolsCategoryGuess:
    """Test _guess_category edge cases"""

    def test_guess_category_search_patterns(self):
        """Test SEARCH category detection"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            assert tools._guess_category("grep") == ToolCategory.SEARCH
            assert tools._guess_category("glob_tool") == ToolCategory.SEARCH
            # Note: "search_files" contains "file" so it matches FILE_OPS first
            assert tools._guess_category("search_content") == ToolCategory.SEARCH
            assert tools._guess_category("find_pattern") == ToolCategory.SEARCH
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_guess_category_shell_patterns(self):
        """Test SHELL category detection"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            assert tools._guess_category("bash_execute") == ToolCategory.SHELL
            assert tools._guess_category("run_bash") == ToolCategory.SHELL
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_guess_category_file_ops_patterns(self):
        """Test FILE_OPS category detection"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            assert tools._guess_category("file_reader") == ToolCategory.FILE_OPS
            assert tools._guess_category("directory_list") == ToolCategory.FILE_OPS
            assert tools._guess_category("list_files") == ToolCategory.FILE_OPS
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestBuiltinToolsReadWithParameters:
    """Test read_file with various parameters"""

    def test_fallback_read_file_with_offset_limit(self):
        """Test read_file with offset and limit"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".txt"
            ) as f:
                for i in range(10):
                    f.write(f"line {i+1}\n")
                path = f.name

            tools = BuiltinTools()
            content = tools.read_file(path, offset=3, limit=2)

            assert "line 3" in content
            assert "line 4" in content
            assert "line 1" not in content

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_read_file_empty_result(self):
        """Test read_file returns empty for offset beyond file"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".txt"
            ) as f:
                f.write("short file\n")
                path = f.name

            tools = BuiltinTools()
            content = tools.read_file(path, offset=100, limit=10)

            # Should return empty or minimal content
            assert content.strip() == "" or "short" not in content

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestBuiltinToolsWriteVariations:
    """Test write_file variations"""

    def test_fallback_write_empty_content(self):
        """Test write_file with empty content"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                path = os.path.join(tmpdir, "empty.txt")

                tools = BuiltinTools()
                result = tools.write_file(path, "")

                # File should be created (empty or with newline)
                assert os.path.exists(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestBuiltinToolsBashVariations:
    """Test bash variations"""

    def test_fallback_bash_with_timeout(self):
        """Test bash with timeout parameter"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            result = tools.bash("echo timeout_test", timeout_ms=5000)

            assert "timeout_test" in result.lower()
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_fallback_bash_with_working_dir(self):
        """Test bash with working_dir parameter"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                tools = BuiltinTools()
                result = tools.bash("echo wd_test", working_dir=tmpdir)

                assert "wd_test" in result.lower()
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestBuiltinToolsGlobVariations:
    """Test glob variations"""

    def test_fallback_glob_with_path(self):
        """Test glob with path parameter"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                open(os.path.join(tmpdir, "file1.py"), "w").close()
                open(os.path.join(tmpdir, "file2.txt"), "w").close()

                tools = BuiltinTools()
                result = tools.glob("*.py", path=tmpdir)

                assert "file1.py" in result
                assert "file2.txt" not in result
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestBuiltinToolsGrepVariations:
    """Test grep variations"""

    def test_fallback_grep_with_glob(self):
        """Test grep with glob pattern"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                py_file = os.path.join(tmpdir, "test.py")
                txt_file = os.path.join(tmpdir, "test.txt")

                with open(py_file, "w") as f:
                    f.write("python_pattern_match\n")
                with open(txt_file, "w") as f:
                    f.write("python_pattern_match\n")

                tools = BuiltinTools()
                result = tools.grep("python_pattern", path=tmpdir, glob="*.py")

                assert "python_pattern" in result
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestEditFileVariations:
    """Test edit_file variations"""

    def test_fallback_edit_file_args_key_mapping(self):
        """Test edit_file with different argument keys"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".txt"
            ) as f:
                f.write("original_text\n")
                path = f.name

            tools = BuiltinTools()
            # Test via execute with correct parameter keys (old/new for fallback)
            result = tools.execute("edit_file", {
                "path": path,
                "old": "original",
                "new": "modified",
            })

            with open(path) as f:
                content = f.read()
            assert "modified" in content

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original


# ==============================================================================
# Rust Executor Mock Tests
# ==============================================================================


class TestRustExecutorPaths:
    """Test Rust executor paths with mock executor"""

    def test_edit_file_rust_executor(self):
        """Test edit_file with mock Rust executor"""
        class MockExecutor:
            def execute(self, name, args):
                return f"Mock execute result for {name}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.edit_file("test.py", "old", "new")
        assert "Mock execute result" in result

    def test_grep_rust_executor(self):
        """Test grep with mock Rust executor"""
        class MockExecutor:
            def grep(self, pattern, path, glob):
                return f"Mock grep: {pattern}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.grep("def test", path="src/", glob="*.py")
        assert "Mock grep" in result

    def test_glob_rust_executor(self):
        """Test glob with mock Rust executor"""
        class MockExecutor:
            def glob(self, pattern, path):
                return f"Mock glob: {pattern}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.glob("**/*.py", path="src/")
        assert "Mock glob" in result

    def test_bash_rust_executor(self):
        """Test bash with mock Rust executor"""
        class MockExecutor:
            def bash(self, command, timeout_ms, working_dir):
                return f"Mock bash: {command}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.bash("echo test", timeout_ms=5000, working_dir="/tmp")
        assert "Mock bash" in result

    def test_list_directory_json_decode_error(self):
        """Test list_directory when executor returns non-JSON"""
        class MockExecutor:
            def execute(self, name, args):
                return "Not JSON data"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.list_directory("/tmp")
        # Should return [{"raw": "Not JSON data"}] on decode error
        assert isinstance(result, list)
        assert len(result) == 1
        assert result[0]["raw"] == "Not JSON data"

    def test_go_to_definition_rust_executor(self):
        """Test go_to_definition with mock Rust executor"""
        class MockExecutor:
            def execute(self, name, args):
                return f"Mock definition: {args}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.go_to_definition("test.py", 10, 5)
        assert "Mock definition" in result

    def test_find_references_rust_executor(self):
        """Test find_references with mock Rust executor"""
        class MockExecutor:
            def execute(self, name, args):
                return f"Mock references: {args}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.find_references("test.py", 10, 5)
        assert "Mock references" in result

    def test_get_hover_rust_executor(self):
        """Test get_hover with mock Rust executor"""
        class MockExecutor:
            def execute(self, name, args):
                return f"Mock hover: {args}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.get_hover("test.py", 10, 5)
        assert "Mock hover" in result

    def test_symbol_search_rust_executor(self):
        """Test symbol_search with mock Rust executor"""
        class MockExecutor:
            def execute(self, name, args):
                return f"Mock symbol_search: {args}"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.symbol_search("test_func")
        assert "Mock symbol_search" in result

    def test_execute_runtime_error_re_raise(self):
        """Test execute re-raises RuntimeError when not 'Tool not found'"""
        class MockExecutor:
            def execute(self, name, args):
                raise RuntimeError("Internal error: something went wrong")

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        with pytest.raises(RuntimeError, match="Internal error"):
            tools.execute("read_file", {"path": "test.txt"})


# ==============================================================================
# LSP Tools Fallback Tests
# ==============================================================================


class TestLSPToolsFallback:
    """Test LSP tools in Python fallback mode"""

    def test_go_to_definition_fallback(self):
        """Test go_to_definition fallback"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("def my_func():\n    pass\n")
                path = f.name

            tools = BuiltinTools()
            # LSP tools return results even without LSP server
            result = tools.go_to_definition(path, 1, 5)
            # Should return some content (even if "No LSP server" message)
            assert isinstance(result, str)

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_find_references_fallback(self):
        """Test find_references fallback"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("def my_func():\n    pass\n")
                path = f.name

            tools = BuiltinTools()
            result = tools.find_references(path, 1, 5)
            assert isinstance(result, str)

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_get_hover_fallback(self):
        """Test get_hover fallback"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("def my_func():\n    pass\n")
                path = f.name

            tools = BuiltinTools()
            result = tools.get_hover(path, 1, 5)
            assert isinstance(result, str)

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_symbol_search_fallback(self):
        """Test symbol_search fallback"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            result = tools.symbol_search("test_func")
            assert isinstance(result, str)
        finally:
            builtin_module.HAS_RUST_BINDING = original


class TestExecuteLSPTools:
    """Test execute method for LSP tools in fallback mode"""

    def test_execute_go_to_definition(self):
        """Test execute for go_to_definition"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("def my_func():\n    pass\n")
                path = f.name

            tools = BuiltinTools()
            result = tools.execute("go_to_definition", {
                "file": path,
                "line": 1,
                "column": 5,
            })
            assert isinstance(result, str)

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_find_references(self):
        """Test execute for find_references"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("def my_func():\n    pass\n")
                path = f.name

            tools = BuiltinTools()
            result = tools.execute("find_references", {
                "file": path,
                "line": 1,
                "column": 5,
            })
            assert isinstance(result, str)

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_get_hover(self):
        """Test execute for get_hover"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            with tempfile.NamedTemporaryFile(
                mode="w", delete=False, suffix=".py"
            ) as f:
                f.write("def my_func():\n    pass\n")
                path = f.name

            tools = BuiltinTools()
            result = tools.execute("get_hover", {
                "file": path,
                "line": 1,
                "column": 5,
            })
            assert isinstance(result, str)

            os.unlink(path)
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_execute_symbol_search(self):
        """Test execute for symbol_search"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            result = tools.execute("symbol_search", {
                "pattern": "test_func",
            })
            assert isinstance(result, str)
        finally:
            builtin_module.HAS_RUST_BINDING = original


# ==============================================================================
# Edge Cases and Error Handling
# ==============================================================================


class TestBuiltinToolsEdgeCases:
    """Test edge cases and error handling"""

    def test_read_file_with_executor(self):
        """Test read_file with mock Rust executor"""
        class MockExecutor:
            def read_file(self, path, offset, limit):
                return "Mock read content"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.read_file("test.txt", offset=5, limit=10)
        assert "Mock read content" in result

    def test_write_file_with_executor(self):
        """Test write_file with mock Rust executor"""
        class MockExecutor:
            def write_file(self, path, content):
                return "Mock write success"

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.write_file("test.txt", "content")
        assert "Mock write success" in result

    def test_list_directory_with_executor(self):
        """Test list_directory with mock Rust executor returning JSON"""
        class MockExecutor:
            def execute(self, name, args):
                return json.dumps([{"name": "file.txt", "type": "file"}])

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        result = tools.list_directory("/tmp")
        assert isinstance(result, list)
        assert result[0]["name"] == "file.txt"

    def test_is_available_with_executor(self):
        """Test is_available with mock Rust executor"""
        class MockExecutor:
            def is_available(self, name):
                return name in ["read_file", "write_file"]

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        assert tools.is_available("read_file") is True
        assert tools.is_available("bash") is False

    def test_fallback_tools_property(self):
        """Test _fallback_tools property"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            fallback = tools._fallback_tools
            assert "read_file" in fallback
            assert "write_file" in fallback
            assert "go_to_definition" in fallback
            assert "symbol_search" in fallback
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_check_binding_unavailable_in_fallback(self):
        """Test _check_binding raises for unavailable tool in fallback mode"""
        import continuum_sdk.tools.builtin as builtin_module
        original = builtin_module.HAS_RUST_BINDING
        builtin_module.HAS_RUST_BINDING = False

        try:
            tools = BuiltinTools()
            # Tool not in fallback should raise
            with pytest.raises(ToolNotAvailableError, match="not available"):
                tools._check_binding("rust_only_tool_xyz")
        finally:
            builtin_module.HAS_RUST_BINDING = original

    def test_get_tool_meta_returns_none_for_unknown(self):
        """Test get_tool_meta returns None for unknown tool"""
        tools = BuiltinTools()
        result = tools.get_tool_meta("completely_unknown_tool_xyz")
        assert result is None

    def test_execute_with_executor_key_error(self):
        """Test execute handles KeyError from executor"""
        class MockExecutor:
            def execute(self, name, args):
                raise KeyError(f"Tool not found: {name}")

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        with pytest.raises(ToolNotAvailableError):
            tools.execute("unknown_tool", {})

    def test_execute_with_executor_tool_not_found_runtime_error(self):
        """Test execute handles RuntimeError with 'Tool not found'"""
        class MockExecutor:
            def execute(self, name, args):
                raise RuntimeError("Tool not found: xyz")

        tools = BuiltinTools()
        tools._executor = MockExecutor()

        with pytest.raises(ToolNotAvailableError):
            tools.execute("unknown_tool", {})


class TestModuleLevelFunctions:
    """Test module-level functions"""

    def test_get_builtin_tools_creates_instance(self):
        """Test get_builtin_tools creates instance"""
        import continuum_sdk.tools.builtin as builtin_module
        # Reset singleton
        builtin_module._builtin_tools = None

        tools = get_builtin_tools()
        assert isinstance(tools, BuiltinTools)
        assert len(tools._tools_cache) > 0


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--cov=continuum_sdk.tools.builtin", "--cov-report=term-missing"])
