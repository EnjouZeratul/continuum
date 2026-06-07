"""
Tests for MCP Adapter integration.

Tests MCPToolRegistry, MCPTool, and ContinuumMCPAdapter.
Requires mcpadapt library for real tests.
"""

from unittest.mock import MagicMock, Mock, patch

import pytest

# Test imports
from continuum_sdk.tools.types import ToolCategory, ToolMeta, ToolResult


class TestMCPToolWithoutLibrary:
    """Test MCPTool without requiring mcpadapt library."""

    def test_mcp_tool_creation(self):
        """Test creating MCPTool manually."""
        # We need to import the module structure
        # Mock the mcpadapt dependency
        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
            },
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            # Create a mock call function
            mock_call = Mock(return_value=Mock(content=[Mock(text="result")]))

            tool = MCPTool(
                name="test_tool",
                description="Test tool",
                parameters={"type": "object"},
                _call_func=mock_call,
                category=ToolCategory.FILE_OPS,
            )

            assert tool.name == "test_tool"
            assert tool.description == "Test tool"
            assert tool.category == ToolCategory.FILE_OPS
            assert not tool.is_dangerous

    def test_mcp_tool_to_meta(self):
        """Test converting MCPTool to ToolMeta."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            mock_call = Mock()
            tool = MCPTool(
                name="read_file",
                description="Read file contents",
                parameters={
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                },
                _call_func=mock_call,
                category=ToolCategory.FILE_OPS,
            )

            meta = tool.to_meta()

            assert isinstance(meta, ToolMeta)
            assert meta.name == "read_file"
            assert meta.description == "Read file contents"
            assert meta.category == ToolCategory.FILE_OPS

    def test_mcp_tool_execute(self):
        """Test MCPTool execute method."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            # Create mock result
            mock_result = Mock()
            mock_result.content = [Mock(text="file contents here")]

            mock_call = Mock(return_value=mock_result)
            tool = MCPTool(
                name="read_file",
                description="Read file",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute({"path": "/test/file.txt"})

            assert isinstance(result, ToolResult)
            assert result.name == "read_file"
            assert result.content == "file contents here"
            assert not result.is_error

    def test_mcp_tool_execute_error(self):
        """Test MCPTool execute with error."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            # 使用具体异常类型（RuntimeError），而不是通用 Exception
            mock_call = Mock(side_effect=RuntimeError("File not found"))
            tool = MCPTool(
                name="read_file",
                description="Read file",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute({"path": "/nonexistent.txt"})

            assert result.is_error
            assert "File not found" in result.content

    def test_mcp_tool_dangerous_flag(self):
        """Test dangerous tools are marked correctly."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            tool = MCPTool(
                name="delete_file",
                description="Delete file",
                parameters={},
                _call_func=Mock(),
                is_dangerous=True,
                requires_confirmation=True,
            )

            assert tool.is_dangerous
            assert tool.requires_confirmation

    def test_mcp_tool_execute_without_content_attribute(self):
        """Test execute when result has no content attribute."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            mock_call = Mock(return_value="plain string result")
            tool = MCPTool(
                name="simple_tool",
                description="Simple tool",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute({})

            assert not result.is_error
            assert result.content == "plain string result"

    def test_mcp_tool_execute_empty_content(self):
        """Test execute with empty content list."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            mock_result = Mock()
            mock_result.content = []

            mock_call = Mock(return_value=mock_result)
            tool = MCPTool(
                name="empty_tool",
                description="Returns empty",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute({})

            assert not result.is_error
            assert "content=" in result.content or "Mock" in result.content

    def test_mcp_tool_execute_content_without_text(self):
        """Test execute when content item has no text attribute."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            mock_result = Mock()
            mock_result.content = [Mock(spec=[])]  # No text attribute

            mock_call = Mock(return_value=mock_result)
            tool = MCPTool(
                name="no_text_tool",
                description="No text",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute({})

            assert not result.is_error

    def test_mcp_tool_execute_none_arguments(self):
        """Test execute with None arguments."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            mock_result = Mock()
            mock_result.content = [Mock(text="ok")]

            mock_call = Mock(return_value=mock_result)
            tool = MCPTool(
                name="test_tool",
                description="Test",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute(None)

            mock_call.assert_called_once_with(None)
            assert not result.is_error

    def test_mcp_tool_duration_tracking(self):
        """Test that execute tracks duration."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            mock_result = Mock()
            mock_result.content = [Mock(text="result")]

            mock_call = Mock(return_value=mock_result)
            tool = MCPTool(
                name="timed_tool",
                description="Timed",
                parameters={},
                _call_func=mock_call,
            )

            result = tool.execute({})

            assert result.duration_ms >= 0


class TestMCPToolAsync:
    """Test MCPTool async functionality."""

    @pytest.mark.asyncio
    async def test_aexecute_success(self):
        """Test async execute success."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            # Create async mock
            async def async_call(arguments):
                mock_result = Mock()
                mock_result.content = [Mock(text="async result")]
                return mock_result

            tool = MCPTool(
                name="async_tool",
                description="Async tool",
                parameters={},
                _call_func=async_call,
            )

            result = await tool.aexecute({"key": "value"})

            assert isinstance(result, ToolResult)
            assert result.name == "async_tool"
            assert result.content == "async result"
            assert not result.is_error

    @pytest.mark.asyncio
    async def test_aexecute_error(self):
        """Test async execute with error."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            async def async_error(arguments):
                raise ValueError("Async error")

            tool = MCPTool(
                name="async_error_tool",
                description="Async error tool",
                parameters={},
                _call_func=async_error,
            )

            result = await tool.aexecute({})

            assert result.is_error
            assert "Async error" in result.content

    @pytest.mark.asyncio
    async def test_aexecute_none_arguments(self):
        """Test async execute with None arguments."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            async def async_call(arguments):
                mock_result = Mock()
                mock_result.content = [Mock(text="ok")]
                return mock_result

            tool = MCPTool(
                name="async_tool",
                description="Async",
                parameters={},
                _call_func=async_call,
            )

            result = await tool.aexecute(None)

            assert not result.is_error

    @pytest.mark.asyncio
    async def test_aexecute_content_without_text(self):
        """Test async execute when content has no text."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            async def async_call(arguments):
                mock_result = Mock()
                mock_result.content = [Mock(spec=[])]
                return mock_result

            tool = MCPTool(
                name="async_tool",
                description="Async",
                parameters={},
                _call_func=async_call,
            )

            result = await tool.aexecute({})

            assert not result.is_error

    @pytest.mark.asyncio
    async def test_aexecute_duration_tracking(self):
        """Test that async execute tracks duration."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            async def async_call(arguments):
                mock_result = Mock()
                mock_result.content = [Mock(text="result")]
                return mock_result

            tool = MCPTool(
                name="async_timed_tool",
                description="Async timed",
                parameters={},
                _call_func=async_call,
            )

            result = await tool.aexecute({})

            assert result.duration_ms >= 0

    @pytest.mark.asyncio
    async def test_aexecute_without_content_attribute(self):
        """Test async execute when result has no content attribute."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            async def async_call(arguments):
                return "plain string result"

            tool = MCPTool(
                name="async_string_tool",
                description="Async string tool",
                parameters={},
                _call_func=async_call,
            )

            result = await tool.aexecute({})

            assert not result.is_error
            assert result.content == "plain string result"

    @pytest.mark.asyncio
    async def test_aexecute_empty_content_list(self):
        """Test async execute with empty content list."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool

            async def async_call(arguments):
                mock_result = Mock()
                mock_result.content = []
                return mock_result

            tool = MCPTool(
                name="async_empty_tool",
                description="Async empty tool",
                parameters={},
                _call_func=async_call,
            )

            result = await tool.aexecute({})

            assert not result.is_error


class TestContinuumMCPAdapter:
    """Test ContinuumMCPAdapter."""

    def test_adapter_creation(self):
        """Test creating adapter."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            # Mock MCPAdapt and ToolAdapter
            mock_mcpadapt = MagicMock()
            mock_tool_adapter = MagicMock()
            mock_mcpadapt.MCPAdapt = mock_mcpadapt
            mock_mcpadapt.ToolAdapter = mock_tool_adapter

            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter(
                category=ToolCategory.FILE_OPS,
                requires_confirmation=False,
            )

            assert adapter.category == ToolCategory.FILE_OPS
            assert not adapter.requires_confirmation
            assert "delete_file" in adapter.dangerous_tools

    def test_adapter_dangerous_tools_default(self):
        """Test default dangerous tools set."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter()

            expected_dangerous = {
                "delete_file",
                "execute_command",
                "run_shell",
                "write_file",
            }
            assert adapter.dangerous_tools == expected_dangerous

    def test_adapter_custom_dangerous_tools(self):
        """Test adapter with custom dangerous tools."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            custom_dangerous = {"rm_file", "destroy_data"}
            adapter = ContinuumMCPAdapter(dangerous_tools=custom_dangerous)

            assert adapter.dangerous_tools == custom_dangerous

    def test_adapter_adapt_basic(self):
        """Test basic adapt method."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.return_value = {
            "type": "object",
            "properties": {"path": {"type": "string"}},
        }

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter, MCPTool

            adapter = ContinuumMCPAdapter(category=ToolCategory.FILE_OPS)

            # Create mock MCP tool
            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "read_file"
            mock_mcp_tool.description = "Read a file"
            mock_mcp_tool.inputSchema = {
                "type": "object",
                "properties": {"path": {"type": "string"}},
            }

            mock_func = Mock()
            result = adapter.adapt(mock_func, mock_mcp_tool)

            assert isinstance(result, MCPTool)
            assert result.name == "read_file"
            assert result.description == "Read a file"
            assert result.category == ToolCategory.FILE_OPS

    def test_adapter_adapt_dangerous_tool(self):
        """Test adapt marks dangerous tools correctly."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.return_value = {"type": "object"}

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter()

            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "delete_file"  # Dangerous!
            mock_mcp_tool.description = "Delete a file"
            mock_mcp_tool.inputSchema = {"type": "object"}

            result = adapter.adapt(Mock(), mock_mcp_tool)

            assert result.is_dangerous
            assert result.requires_confirmation

    def test_adapter_adapt_case_insensitive_dangerous(self):
        """Test adapt checks dangerous tools case-insensitively."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.return_value = {"type": "object"}

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter()

            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "DELETE_FILE"  # Uppercase
            mock_mcp_tool.description = "Delete a file"
            mock_mcp_tool.inputSchema = {"type": "object"}

            result = adapter.adapt(Mock(), mock_mcp_tool)

            assert result.is_dangerous

    def test_adapter_adapt_with_json_schema_refs(self):
        """Test adapt resolves JSON schema references."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.return_value = {
            "type": "object",
            "properties": {"ref": {"type": "string"}},
            "$defs": {"should_be_removed": True},
        }

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter()

            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "test_tool"
            mock_mcp_tool.description = "Test"
            mock_mcp_tool.inputSchema = {"$ref": "#/definitions/some"}

            result = adapter.adapt(Mock(), mock_mcp_tool)

            # $defs should be filtered out
            assert "$defs" not in result.parameters

    def test_adapter_adapt_jsonref_error(self):
        """Test adapt handles JSON schema resolution errors gracefully."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.side_effect = ValueError("Invalid ref")

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter()

            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "test_tool"
            mock_mcp_tool.description = "Test"
            mock_mcp_tool.inputSchema = {"type": "object"}

            # Should not raise, just use original schema
            result = adapter.adapt(Mock(), mock_mcp_tool)

            assert result.name == "test_tool"

    def test_adapter_adapt_empty_description(self):
        """Test adapt handles empty description."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.return_value = {"type": "object"}

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter

            adapter = ContinuumMCPAdapter()

            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "test_tool"
            mock_mcp_tool.description = None
            mock_mcp_tool.inputSchema = {"type": "object"}

            result = adapter.adapt(Mock(), mock_mcp_tool)

            assert result.description == ""

    @pytest.mark.asyncio
    async def test_adapter_async_adapt(self):
        """Test async_adapt method."""
        mock_jsonref = MagicMock()
        mock_jsonref.replace_refs.return_value = {"type": "object"}

        with patch.dict(
            "sys.modules",
            {
                "mcp": MagicMock(),
                "mcpadapt": MagicMock(),
                "mcpadapt.core": MagicMock(),
                "jsonref": mock_jsonref,
            },
        ):
            from continuum_sdk.tools.mcp_adapter import ContinuumMCPAdapter, MCPTool

            adapter = ContinuumMCPAdapter()

            mock_mcp_tool = MagicMock()
            mock_mcp_tool.name = "async_tool"
            mock_mcp_tool.description = "Async tool"
            mock_mcp_tool.inputSchema = {"type": "object"}

            async def async_func(args):
                return Mock(content=[Mock(text="result")])

            result = await adapter.async_adapt(async_func, mock_mcp_tool)

            assert isinstance(result, MCPTool)
            assert result.name == "async_tool"


class TestMCPToolRegistry:
    """Test MCPToolRegistry."""

    def test_registry_creation(self):
        """Test creating registry."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            registry = MCPToolRegistry(timeout=30)

            assert registry.timeout == 30
            assert registry._connections == {}
            assert registry._tools == {}

    def test_registry_get_tools_empty(self):
        """Test get_tools when no connections."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            registry = MCPToolRegistry()
            tools = registry.get_tools()

            assert tools == []

    def test_registry_close_empty(self):
        """Test closing empty registry."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            registry = MCPToolRegistry()
            registry.close()  # Should not raise

            assert registry._connections == {}
            assert registry._tools == {}

    def test_registry_context_manager(self):
        """Test registry as context manager."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            with MCPToolRegistry() as registry:
                assert registry is not None

            # Should be closed after context
            assert registry._connections == {}

    def test_registry_connect_stdio(self):
        """Test connecting to stdio MCP server."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            # Setup mocks
            mock_mcp = MagicMock()
            mock_mcpadapt = MagicMock()

            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="read_file",
                description="Read file",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), mock_mcp)

                registry = MCPToolRegistry()
                tools = registry.connect_stdio(
                    name="test_server",
                    command="uvx",
                    args=["mcp-server-test"],
                    category=ToolCategory.FILE_OPS,
                )

                assert len(tools) == 1
                assert tools[0].name == "read_file"
                assert "test_server" in registry._connections

    def test_registry_connect_stdio_with_env(self):
        """Test connecting to stdio with environment variables."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            mock_mcp = MagicMock()
            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_client.tools.return_value = []
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), mock_mcp)

                registry = MCPToolRegistry()
                registry.connect_stdio(
                    name="test_server",
                    command="uvx",
                    args=["mcp-server-test"],
                    env={"CUSTOM_VAR": "value"},
                )

                # Verify StdioServerParameters was called with merged env
                mock_mcp.StdioServerParameters.assert_called_once()

    def test_registry_connect_sse(self):
        """Test connecting to SSE MCP server."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="sse_tool",
                description="SSE tool",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                tools = registry.connect_sse(
                    name="sse_server",
                    url="http://localhost:8080/sse",
                    category=ToolCategory.NETWORK,
                )

                assert len(tools) == 1
                assert tools[0].name == "sse_tool"
                assert "sse_server" in registry._connections

                # Verify params passed correctly
                call_args = mock_mcpadapt.call_args
                assert call_args[0][0]["url"] == "http://localhost:8080/sse"
                assert call_args[0][0]["transport"] == "sse"

    def test_registry_connect_websocket(self):
        """Test connecting to WebSocket MCP server."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="ws_tool",
                description="WebSocket tool",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                tools = registry.connect_websocket(
                    name="ws_server",
                    url="ws://localhost:8080/ws",
                    category=ToolCategory.NETWORK,
                )

                assert len(tools) == 1
                assert tools[0].name == "ws_tool"

                # Verify params passed correctly
                call_args = mock_mcpadapt.call_args
                assert call_args[0][0]["url"] == "ws://localhost:8080/ws"
                assert call_args[0][0]["transport"] == "ws"

    def test_registry_get_tools_with_connection(self):
        """Test get_tools with specific connection filter."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="tool1",
                description="Tool 1",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server1", "cmd", [])
                registry.connect_stdio("server2", "cmd", [])

                # Get all tools
                all_tools = registry.get_tools()
                assert len(all_tools) == 2

                # Get specific connection tools
                server1_tools = registry.get_tools("server1")
                assert len(server1_tools) == 1

                # Non-existent connection
                empty_tools = registry.get_tools("nonexistent")
                assert empty_tools == []

    def test_registry_get_tool_metas(self):
        """Test get_tool_metas method."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="meta_tool",
                description="Meta tool",
                parameters={"type": "object"},
                _call_func=Mock(),
                category=ToolCategory.FILE_OPS,
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server", "cmd", [])

                metas = registry.get_tool_metas()

                assert len(metas) == 1
                assert isinstance(metas[0], ToolMeta)
                assert metas[0].name == "meta_tool"

    def test_registry_refresh_tools(self):
        """Test refresh_tools method."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool1 = MCPTool(
                name="tool1",
                description="Tool 1",
                parameters={},
                _call_func=Mock(),
            )
            mock_tool2 = MCPTool(
                name="tool2",
                description="Tool 2",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool1]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server", "cmd", [])

                # Change what tools() returns
                mock_client.tools.return_value = [mock_tool1, mock_tool2]

                registry.refresh_tools("server")

                tools = registry.get_tools("server")
                assert len(tools) == 2

    def test_registry_refresh_tools_all_connections(self):
        """Test refresh_tools for all connections."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="tool",
                description="Tool",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server1", "cmd", [])
                registry.connect_stdio("server2", "cmd", [])

                registry.refresh_tools()  # Refresh all

                assert mock_client.tools.call_count >= 4  # Initial + refresh

    def test_registry_refresh_tools_error(self):
        """Test refresh_tools handles errors gracefully."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_tool = MCPTool(
                name="tool",
                description="Tool",
                parameters={},
                _call_func=Mock(),
            )
            mock_client.tools.return_value = [mock_tool]
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server", "cmd", [])

                # Make tools() raise error on refresh
                mock_client.tools.side_effect = ConnectionError("Failed")

                # Should not raise
                registry.refresh_tools("server")

    def test_registry_close_specific_connection(self):
        """Test closing a specific connection."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPTool, MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client1 = MagicMock()
            mock_client2 = MagicMock()
            mock_tool = MCPTool(
                name="tool",
                description="Tool",
                parameters={},
                _call_func=Mock(),
            )
            mock_client1.tools.return_value = [mock_tool]
            mock_client2.tools.return_value = [mock_tool]
            mock_mcpadapt.side_effect = [mock_client1, mock_client2]

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server1", "cmd", [])
                registry.connect_stdio("server2", "cmd", [])

                registry.close("server1")

                assert "server1" not in registry._connections
                assert "server1" not in registry._tools
                assert "server2" in registry._connections
                mock_client1.close.assert_called_once()

    def test_registry_close_nonexistent_connection(self):
        """Test closing a non-existent connection does nothing."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            registry = MCPToolRegistry()

            # Should not raise
            registry.close("nonexistent")

    def test_registry_close_all_handles_errors(self):
        """Test close handles errors from individual clients."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_client.tools.return_value = []
            mock_client.close.side_effect = RuntimeError("Close failed")
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                registry = MCPToolRegistry()
                registry.connect_stdio("server", "cmd", [])

                # Should not raise
                registry.close()

                assert registry._connections == {}

    @pytest.mark.asyncio
    async def test_registry_async_context_manager(self):
        """Test registry as async context manager."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

            mock_mcpadapt = MagicMock()
            mock_client = MagicMock()
            mock_client.tools.return_value = []
            mock_mcpadapt.return_value = mock_client

            with patch(
                "continuum_sdk.tools.mcp_adapter._ensure_mcpadapt"
            ) as mock_ensure:
                mock_ensure.return_value = ((mock_mcpadapt, MagicMock()), MagicMock())

                async with MCPToolRegistry() as registry:
                    assert registry is not None
                    registry.connect_stdio("server", "cmd", [])

                # Should be closed after context
                assert registry._connections == {}


class TestPredefinedServers:
    """Test predefined MCP server configurations."""

    def test_predefined_servers_exist(self):
        """Test predefined servers are available."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import PREDEFINED_MCP_SERVERS

            expected_servers = [
                "filesystem",
                "github",
                "puppeteer",
                "slack",
                "postgres",
                "memory",
            ]
            for server in expected_servers:
                assert server in PREDEFINED_MCP_SERVERS
                assert "command" in PREDEFINED_MCP_SERVERS[server]
                assert "args" in PREDEFINED_MCP_SERVERS[server]
                assert "description" in PREDEFINED_MCP_SERVERS[server]

    def test_fileserver_config(self):
        """Test filesystem server configuration."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import PREDEFINED_MCP_SERVERS

            fs_config = PREDEFINED_MCP_SERVERS["filesystem"]

            assert fs_config["command"] == "uvx"
            assert "mcp-server-filesystem" in fs_config["args"]
            assert fs_config["category"] == ToolCategory.FILE_OPS


class TestCreateMCPRegistry:
    """Test create_mcp_registry helper function."""

    def test_create_registry_default(self):
        """Test create_registry with defaults."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import create_mcp_registry

            # Mock the registry creation
            with patch(
                "continuum_sdk.tools.mcp_adapter.MCPToolRegistry"
            ) as mock_registry_class:
                mock_registry = MagicMock()
                mock_registry_class.return_value = mock_registry
                mock_registry.connect_stdio = Mock(return_value=[])

                create_mcp_registry()

                # Should default to filesystem
                mock_registry.connect_stdio.assert_called_once()

    def test_create_registry_custom_servers(self):
        """Test create_registry with custom servers."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import create_mcp_registry

            with patch(
                "continuum_sdk.tools.mcp_adapter.MCPToolRegistry"
            ) as mock_registry_class:
                mock_registry = MagicMock()
                mock_registry_class.return_value = mock_registry
                mock_registry.connect_stdio = Mock(return_value=[])

                create_mcp_registry(["filesystem", "github"])

                # Should call connect_stdio twice
                assert mock_registry.connect_stdio.call_count == 2

    def test_create_registry_unknown_server(self):
        """Test create_registry with unknown server name."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import create_mcp_registry

            with patch(
                "continuum_sdk.tools.mcp_adapter.MCPToolRegistry"
            ) as mock_registry_class:
                mock_registry = MagicMock()
                mock_registry_class.return_value = mock_registry
                mock_registry.connect_stdio = Mock(return_value=[])

                create_mcp_registry(["unknown_server"])

                # Should not call connect_stdio for unknown server
                mock_registry.connect_stdio.assert_not_called()

    def test_create_registry_with_root_path(self):
        """Test create_registry with root_path for filesystem."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import create_mcp_registry

            with patch(
                "continuum_sdk.tools.mcp_adapter.MCPToolRegistry"
            ) as mock_registry_class:
                mock_registry = MagicMock()
                mock_registry_class.return_value = mock_registry
                mock_registry.connect_stdio = Mock(return_value=[])

                create_mcp_registry(servers=["filesystem"], root_path="/custom/root")

                # Check that --root was added to args
                call_kwargs = mock_registry.connect_stdio.call_args
                assert "--root" in call_kwargs[1]["args"]
                assert "/custom/root" in call_kwargs[1]["args"]

    def test_create_registry_connection_error(self):
        """Test create_registry handles connection errors."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import create_mcp_registry

            with patch(
                "continuum_sdk.tools.mcp_adapter.MCPToolRegistry"
            ) as mock_registry_class:
                mock_registry = MagicMock()
                mock_registry_class.return_value = mock_registry
                mock_registry.connect_stdio.side_effect = ConnectionError("Failed")

                # Should not raise, just log error
                registry = create_mcp_registry(["filesystem"])

                assert registry is not None

    def test_create_registry_with_env_vars(self, monkeypatch):
        """Test create_registry with environment variables from predefined config."""
        with patch.dict(
            "sys.modules",
            {"mcp": MagicMock(), "mcpadapt": MagicMock(), "mcpadapt.core": MagicMock()},
        ):
            from continuum_sdk.tools.mcp_adapter import create_mcp_registry

            # Set env var for github server
            monkeypatch.setenv("GITHUB_TOKEN", "test_token")

            with patch(
                "continuum_sdk.tools.mcp_adapter.MCPToolRegistry"
            ) as mock_registry_class:
                mock_registry = MagicMock()
                mock_registry_class.return_value = mock_registry
                mock_registry.connect_stdio = Mock(return_value=[])

                create_mcp_registry(["github"])

                # Check that env was passed
                call_kwargs = mock_registry.connect_stdio.call_args
                if call_kwargs[1].get("env"):
                    assert "GITHUB_TOKEN" in call_kwargs[1]["env"]


class TestModuleExports:
    """Test module exports."""

    def test_tools_init_exports(self):
        """Test tools module exports MCP classes."""
        from continuum_sdk.tools import (
            _MCP_AVAILABLE,
            PREDEFINED_MCP_SERVERS,
            ContinuumMCPAdapter,
            MCPTool,
            MCPToolRegistry,
            create_mcp_registry,
        )

        # If mcpadapt is installed, these should be available
        # If not, they should be None
        if _MCP_AVAILABLE:
            assert MCPToolRegistry is not None
            assert MCPTool is not None
            assert ContinuumMCPAdapter is not None
            assert create_mcp_registry is not None
            assert PREDEFINED_MCP_SERVERS is not None


# Integration tests (require mcpadapt library)
@pytest.mark.skipif(
    not pytest.importorskip("mcpadapt", reason="mcpadapt not installed"),
    reason="Integration tests require mcpadapt library",
)
class TestMCPIntegration:
    """Integration tests with real MCP library."""

    def test_real_connect_stdio(self):
        """Test connecting to real MCP server."""
        # This would test with a real MCP server if available
        # Skipped in CI environment
        pass

    def test_real_tool_execution(self):
        """Test executing real MCP tool."""
        # This would test tool execution with real MCP server
        pass


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
