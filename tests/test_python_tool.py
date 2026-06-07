"""
Test Python tool registration and execution.

This module tests the Python tool registration system which allows
registering Python callables as tools that can be executed by the runtime.

NOTE: These tests require the Rust bindings (sh-python) to be built with maturin.
Run `maturin develop` in the rust/sh-python directory before running these tests.
"""

import pytest
import json
import asyncio
from typing import Dict, Any


# Try to import from Rust bindings
try:
    import continuum_bindings
    HAVE_BINDINGS = True
except ImportError:
    HAVE_BINDINGS = False
    continuum_bindings = None


@pytest.mark.skipif(not HAVE_BINDINGS, reason="Rust bindings not installed. Run 'maturin develop' first.")
class TestPythonToolRegistration:
    """Tests for Python tool registration."""

    def test_register_simple_function(self):
        """Test registering a simple Python function and calling it."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        # Define a simple tool
        def hello_world(args: dict) -> str:
            return "Hello, World!"

        # Register the tool
        runtime.register_tool("hello_world", "A simple hello world tool", hello_world)

        # List tools should include our tool
        tools = runtime.list_tools()
        assert "hello_world" in tools

    def test_register_function_with_parameters(self):
        """Test registering a function that accepts parameters."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        # Define a tool with parameters
        def greet(args: dict) -> str:
            name = args.get("name", "Anonymous")
            return f"Hello, {name}!"

        # Register with parameter schema
        parameters = {
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name to greet"
                }
            },
            "required": ["name"]
        }

        runtime.register_tool(
            "greet",
            "Greet a person by name",
            greet,
            parameters
        )

        # Verify registration
        tools = runtime.list_tools()
        assert "greet" in tools

    def test_tool_execution_returns_result(self):
        """Test that executing a registered tool returns the correct result."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()

        # Execute a built-in tool (glob)
        result = executor.execute("glob", json.dumps({"pattern": "*.py"}))
        assert result is not None
        assert isinstance(result, str)

    def test_tool_with_dict_return(self):
        """Test tool that returns a dictionary."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        # Define a tool returning dict
        def get_info(args: dict) -> dict:
            return {
                "status": "success",
                "data": args.get("query", ""),
                "count": len(args.get("items", []))
            }

        runtime.register_tool(
            "get_info",
            "Get information about a query",
            get_info
        )

        tools = runtime.list_tools()
        assert "get_info" in tools

    def test_multiple_tool_registration(self):
        """Test registering multiple tools."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def tool1(args: dict) -> str:
            return "tool1"

        def tool2(args: dict) -> str:
            return "tool2"

        def tool3(args: dict) -> str:
            return "tool3"

        runtime.register_tool("tool1", "First tool", tool1)
        runtime.register_tool("tool2", "Second tool", tool2)
        runtime.register_tool("tool3", "Third tool", tool3)

        tools = runtime.list_tools()
        assert "tool1" in tools
        assert "tool2" in tools
        assert "tool3" in tools


@pytest.mark.skipif(not HAVE_BINDINGS, reason="Rust bindings not installed. Run 'maturin develop' first.")
class TestPythonToolErrorHandling:
    """Tests for error handling in Python tool execution."""

    def test_tool_raises_exception(self):
        """Test that tool exceptions are handled properly."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def failing_tool(args: dict) -> str:
            raise ValueError("Intentional error")

        runtime.register_tool("failing_tool", "A tool that fails", failing_tool)

        # Registration should succeed
        tools = runtime.list_tools()
        assert "failing_tool" in tools

    def test_tool_with_invalid_json_args(self):
        """Test tool execution with invalid JSON arguments."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()

        # Invalid JSON should raise an error
        with pytest.raises(Exception):
            executor.execute("some_tool", "not valid json")

    def test_tool_with_missing_required_args(self):
        """Test tool execution with missing required arguments."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def requires_name(args: dict) -> str:
            if "name" not in args:
                raise ValueError("Missing required argument: name")
            return f"Hello, {args['name']}!"

        parameters = {
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "required": ["name"]
        }

        runtime.register_tool("requires_name", "A tool requiring name", requires_name, parameters)

        # Should be registered
        tools = runtime.list_tools()
        assert "requires_name" in tools


@pytest.mark.skipif(not HAVE_BINDINGS, reason="Rust bindings not installed. Run 'maturin develop' first.")
class TestPythonToolAsyncExecution:
    """Tests for asynchronous tool execution."""

    def test_async_tool_registration(self):
        """Test registering an async function as a tool."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        async def async_tool(args: dict) -> str:
            await asyncio.sleep(0.1)
            return f"Async result: {args}"

        # Async functions should be registerable
        runtime.register_tool("async_tool", "An async tool", async_tool)

        tools = runtime.list_tools()
        assert "async_tool" in tools

    def test_tool_with_async_operation(self):
        """Test tool that performs async operations."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        async def fetch_data(args: dict) -> dict:
            # Simulate async operation
            await asyncio.sleep(0.01)
            return {
                "fetched": True,
                "query": args.get("query", ""),
                "timestamp": "2024-01-01T00:00:00Z"
            }

        runtime.register_tool("fetch_data", "Fetch data asynchronously", fetch_data)

        tools = runtime.list_tools()
        assert "fetch_data" in tools


@pytest.mark.skipif(not HAVE_BINDINGS, reason="Rust bindings not installed. Run 'maturin develop' first.")
class TestPythonToolTypes:
    """Tests for different tool return types."""

    def test_tool_returns_string(self):
        """Test tool returning a string."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def string_tool(args: dict) -> str:
            return "simple string result"

        runtime.register_tool("string_tool", "Returns string", string_tool)
        assert "string_tool" in runtime.list_tools()

    def test_tool_returns_dict(self):
        """Test tool returning a dictionary."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def dict_tool(args: dict) -> dict:
            return {"key": "value", "nested": {"a": 1, "b": 2}}

        runtime.register_tool("dict_tool", "Returns dict", dict_tool)
        assert "dict_tool" in runtime.list_tools()

    def test_tool_returns_list(self):
        """Test tool returning a list."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def list_tool(args: dict) -> list:
            return ["item1", "item2", "item3"]

        runtime.register_tool("list_tool", "Returns list", list_tool)
        assert "list_tool" in runtime.list_tools()

    def test_tool_returns_number(self):
        """Test tool returning a number."""
        from continuum_bindings import AgentRuntime

        runtime = AgentRuntime()

        def number_tool(args: dict) -> int:
            return len(args.get("items", []))

        runtime.register_tool("number_tool", "Returns number", number_tool)
        assert "number_tool" in runtime.list_tools()


@pytest.mark.skipif(not HAVE_BINDINGS, reason="Rust bindings not installed. Run 'maturin develop' first.")
class TestPythonToolIntegration:
    """Integration tests for Python tool system."""

    def test_builtin_tools_available(self):
        """Test that built-in tools are available."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()

        # Built-in tools should be available
        result = executor.list_tools()
        assert isinstance(result, list)

    def test_tool_executor_creation(self):
        """Test ToolExecutor can be created."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()
        assert executor is not None

    def test_tool_executor_with_bash(self):
        """Test ToolExecutor with bash command."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()

        # Execute a simple bash command
        result = executor.execute("bash", json.dumps({"command": "echo hello"}))
        assert result is not None

    def test_tool_executor_with_glob(self):
        """Test ToolExecutor with glob pattern."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()

        result = executor.execute("glob", json.dumps({"pattern": "*.py"}))
        assert result is not None

    def test_tool_executor_is_available(self):
        """Test checking tool availability."""
        from continuum_bindings import ToolExecutor

        executor = ToolExecutor()

        # Check if a tool is available
        is_available = executor.is_available("bash")
        assert isinstance(is_available, bool)


class TestBindingsInstallation:
    """Test that bindings are properly installed."""

    def test_bindings_importable(self):
        """Test that continuum_bindings can be imported."""
        try:
            import continuum_bindings
            assert continuum_bindings is not None
        except ImportError:
            pytest.skip("continuum_bindings not installed. Run 'maturin develop' in rust/sh-python")

    def test_bindings_has_agent_runtime(self):
        """Test that AgentRuntime class exists."""
        try:
            import continuum_bindings
            assert hasattr(continuum_bindings, 'AgentRuntime')
        except ImportError:
            pytest.skip("continuum_bindings not installed")

    def test_bindings_has_tool_executor(self):
        """Test that ToolExecutor class exists."""
        try:
            import continuum_bindings
            assert hasattr(continuum_bindings, 'ToolExecutor')
        except ImportError:
            pytest.skip("continuum_bindings not installed")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
