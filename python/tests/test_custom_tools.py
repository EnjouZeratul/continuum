"""Tests for continuum_sdk.tools.custom module.

Tests cover:
- ToolRegistry class
- Tool registration and unregistration
- Tool execution
- Parameter Schema generation
- Error handling
"""

import pytest

from continuum_sdk.tools.custom import (
    CustomTool,
    ToolRegistry,
    default_registry,
    get_registry,
    register_tool,
    tool,
)


class SimpleTestTool(CustomTool):
    """Simple test tool for testing."""

    @property
    def name(self) -> str:
        return "simple_test"

    @property
    def description(self) -> str:
        return "A simple test tool"

    def parameters_schema(self) -> dict:
        return {
            "type": "object",
            "properties": {"input": {"type": "string"}},
            "required": ["input"],
        }

    async def execute(self, **kwargs) -> str:
        return f"Processed: {kwargs.get('input', '')}"


class DangerousTestTool(CustomTool):
    """Dangerous test tool for testing."""

    @property
    def name(self) -> str:
        return "dangerous_test"

    @property
    def description(self) -> str:
        return "A dangerous test tool"

    def parameters_schema(self) -> dict:
        return {"type": "object", "properties": {}}

    async def execute(self, **kwargs) -> str:
        return "Dangerous operation executed"

    @property
    def requires_confirmation(self) -> bool:
        return True

    @property
    def is_dangerous(self) -> bool:
        return True


class TestCustomToolBase:
    """Tests for CustomTool base class."""

    def test_custom_tool_properties(self):
        """Test CustomTool property access."""
        tool_instance = SimpleTestTool()
        assert tool_instance.name == "simple_test"
        assert tool_instance.description == "A simple test tool"
        assert tool_instance.category == "other"
        assert tool_instance.requires_confirmation is False
        assert tool_instance.is_dangerous is False

    def test_custom_tool_parameters_schema(self):
        """Test parameters_schema returns correct schema."""
        tool_instance = SimpleTestTool()
        schema = tool_instance.parameters_schema()
        assert schema["type"] == "object"
        assert "input" in schema["properties"]
        assert "input" in schema["required"]

    def test_custom_tool_to_meta(self):
        """Test to_meta returns complete metadata."""
        tool_instance = SimpleTestTool()
        meta = tool_instance.to_meta()
        assert meta["name"] == "simple_test"
        assert meta["description"] == "A simple test tool"
        assert meta["category"] == "other"
        assert meta["requires_confirmation"] is False
        assert meta["is_dangerous"] is False
        assert "parameters" in meta

    def test_dangerous_tool_properties(self):
        """Test dangerous tool properties."""
        tool_instance = DangerousTestTool()
        assert tool_instance.requires_confirmation is True
        assert tool_instance.is_dangerous is True
        meta = tool_instance.to_meta()
        assert meta["requires_confirmation"] is True
        assert meta["is_dangerous"] is True


class TestToolDecorator:
    """Tests for the @tool decorator."""

    def test_basic_decorator(self):
        """Test basic tool decorator usage."""

        @tool(name="add", description="Add two numbers")
        def add(a: int, b: int) -> int:
            return a + b

        assert isinstance(add, CustomTool)
        assert add.name == "add"
        assert add.description == "Add two numbers"

    def test_decorator_with_defaults(self):
        """Test decorator with default parameters."""

        @tool(name="greet", description="Say hello")
        def greet(name: str, greeting: str = "Hello") -> str:
            return f"{greeting}, {name}!"

        assert greet.name == "greet"
        schema = greet.parameters_schema()
        assert "name" in schema["required"]
        assert "greeting" not in schema["required"]

    def test_decorator_with_custom_schema(self):
        """Test decorator with custom parameter schema."""
        custom_schema = {
            "type": "object",
            "properties": {"value": {"type": "number"}},
            "required": ["value"],
        }

        @tool(
            name="custom_schema_tool",
            description="Custom schema",
            parameters=custom_schema,
        )
        def custom_tool(value: float) -> float:
            return value * 2

        schema = custom_tool.parameters_schema()
        assert schema == custom_schema

    def test_decorator_async_function(self):
        """Test decorator with async function."""

        @tool(name="async_tool", description="Async tool")
        async def async_tool(value: str) -> str:
            return f"async: {value}"

        assert async_tool.name == "async_tool"

    def test_decorator_type_inference_string(self):
        """Test parameter type inference for string."""

        @tool(name="string_tool", description="String tool")
        def string_tool(text: str) -> str:
            return text

        schema = string_tool.parameters_schema()
        assert schema["properties"]["text"]["type"] == "string"

    def test_decorator_type_inference_int(self):
        """Test parameter type inference for int."""

        @tool(name="int_tool", description="Int tool")
        def int_tool(count: int) -> int:
            return count

        schema = int_tool.parameters_schema()
        assert schema["properties"]["count"]["type"] == "integer"

    def test_decorator_type_inference_float(self):
        """Test parameter type inference for float."""

        @tool(name="float_tool", description="Float tool")
        def float_tool(value: float) -> float:
            return value

        schema = float_tool.parameters_schema()
        assert schema["properties"]["value"]["type"] == "number"

    def test_decorator_type_inference_bool(self):
        """Test parameter type inference for bool."""

        @tool(name="bool_tool", description="Bool tool")
        def bool_tool(flag: bool) -> bool:
            return flag

        schema = bool_tool.parameters_schema()
        assert schema["properties"]["flag"]["type"] == "boolean"

    def test_decorator_type_inference_list(self):
        """Test parameter type inference for list."""

        @tool(name="list_tool", description="List tool")
        def list_tool(items: list) -> list:
            return items

        schema = list_tool.parameters_schema()
        assert schema["properties"]["items"]["type"] == "array"

    def test_decorator_type_inference_dict(self):
        """Test parameter type inference for dict."""

        @tool(name="dict_tool", description="Dict tool")
        def dict_tool(data: dict) -> dict:
            return data

        schema = dict_tool.parameters_schema()
        assert schema["properties"]["data"]["type"] == "object"

    def test_decorator_requires_confirmation(self):
        """Test decorator with requires_confirmation flag."""

        @tool(
            name="confirm_tool",
            description="Needs confirmation",
            requires_confirmation=True,
        )
        def confirm_tool(action: str) -> str:
            return action

        assert confirm_tool.requires_confirmation is True

    def test_decorator_is_dangerous(self):
        """Test decorator with is_dangerous flag."""

        @tool(name="danger_tool", description="Dangerous", is_dangerous=True)
        def danger_tool(action: str) -> str:
            return action

        assert danger_tool.is_dangerous is True


class TestToolRegistry:
    """Tests for ToolRegistry class."""

    def test_registry_creation(self):
        """Test creating a new registry."""
        registry = ToolRegistry()
        assert registry is not None
        assert registry.list() == []

    def test_register_tool(self):
        """Test registering a tool."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        assert registry.has_tool("simple_test")
        assert registry.get("simple_test") == tool_instance

    def test_register_decorator_tool(self):
        """Test registering a decorated function."""

        @tool(name="decorated", description="Decorated tool")
        def decorated_func(x: int) -> int:
            return x * 2

        registry = ToolRegistry()
        registry.register(decorated_func)

        assert registry.has_tool("decorated")

    def test_unregister_tool(self):
        """Test unregistering a tool."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        assert registry.unregister("simple_test") is True
        assert not registry.has_tool("simple_test")

    def test_unregister_nonexistent_tool(self):
        """Test unregistering a tool that doesn't exist."""
        registry = ToolRegistry()
        result = registry.unregister("nonexistent")
        assert result is False

    def test_get_tool(self):
        """Test getting a tool by name."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        retrieved = registry.get("simple_test")
        assert retrieved == tool_instance

    def test_get_nonexistent_tool(self):
        """Test getting a tool that doesn't exist."""
        registry = ToolRegistry()
        result = registry.get("nonexistent")
        assert result is None

    def test_list_tools(self):
        """Test listing all tools."""
        registry = ToolRegistry()
        tool1 = SimpleTestTool()
        tool2 = DangerousTestTool()
        registry.register(tool1)
        registry.register(tool2)

        tools = registry.list()
        assert len(tools) == 2
        assert tool1 in tools
        assert tool2 in tools

    def test_list_names(self):
        """Test listing tool names."""
        registry = ToolRegistry()
        tool1 = SimpleTestTool()
        tool2 = DangerousTestTool()
        registry.register(tool1)
        registry.register(tool2)

        names = registry.list_names()
        assert len(names) == 2
        assert "simple_test" in names
        assert "dangerous_test" in names

    def test_has_tool(self):
        """Test checking if tool exists."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        assert registry.has_tool("simple_test") is True
        assert registry.has_tool("nonexistent") is False

    def test_get_meta(self):
        """Test getting tool metadata."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        meta = registry.get_meta("simple_test")
        assert meta is not None
        assert meta["name"] == "simple_test"
        assert meta["description"] == "A simple test tool"

    def test_get_meta_nonexistent(self):
        """Test getting metadata for nonexistent tool."""
        registry = ToolRegistry()
        meta = registry.get_meta("nonexistent")
        assert meta is None


class TestToolRegistryExecute:
    """Tests for tool execution."""

    @pytest.mark.asyncio
    async def test_execute_tool(self):
        """Test executing a tool."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        result = await registry.execute("simple_test", input="test_value")
        assert result == "Processed: test_value"

    @pytest.mark.asyncio
    async def test_execute_nonexistent_tool(self):
        """Test executing a tool that doesn't exist."""
        registry = ToolRegistry()
        with pytest.raises(ValueError, match="Tool not found"):
            await registry.execute("nonexistent", x=1)

    @pytest.mark.asyncio
    async def test_execute_decorated_sync_function(self):
        """Test executing a decorated sync function."""

        @tool(name="sync_add", description="Add two numbers")
        def sync_add(a: int, b: int) -> int:
            return a + b

        registry = ToolRegistry()
        registry.register(sync_add)

        result = await registry.execute("sync_add", a=5, b=3)
        assert result == "8"

    @pytest.mark.asyncio
    async def test_execute_decorated_async_function(self):
        """Test executing a decorated async function."""

        @tool(name="async_multiply", description="Multiply two numbers")
        async def async_multiply(a: int, b: int) -> int:
            return a * b

        registry = ToolRegistry()
        registry.register(async_multiply)

        result = await registry.execute("async_multiply", a=4, b=7)
        assert result == "28"


class TestDefaultRegistry:
    """Tests for default registry functionality."""

    def test_default_registry_exists(self):
        """Test that default registry exists."""
        assert default_registry is not None
        assert isinstance(default_registry, ToolRegistry)

    def test_get_registry(self):
        """Test getting default registry."""
        registry = get_registry()
        assert registry is default_registry

    def test_register_tool_to_default(self):
        """Test registering tool to default registry."""
        tool_instance = SimpleTestTool()
        register_tool(tool_instance)

        assert default_registry.has_tool("simple_test")
        # Cleanup
        default_registry.unregister("simple_test")


class TestParameterSchemaGeneration:
    """Tests for parameter schema generation."""

    def test_schema_no_parameters(self):
        """Test schema generation for function with no parameters."""

        @tool(name="no_params", description="No parameters")
        def no_params() -> str:
            return "result"

        schema = no_params.parameters_schema()
        assert schema["type"] == "object"
        assert schema["properties"] == {}
        assert schema["required"] == []

    def test_schema_all_required(self):
        """Test schema generation with all required parameters."""

        @tool(name="all_required", description="All required")
        def all_required(a: int, b: str, c: bool) -> str:
            return f"{a}{b}{c}"

        schema = all_required.parameters_schema()
        assert set(schema["required"]) == {"a", "b", "c"}

    def test_schema_with_defaults(self):
        """Test schema generation with default parameters."""

        @tool(name="with_defaults", description="With defaults")
        def with_defaults(required: str, optional: int = 10) -> str:
            return f"{required}{optional}"

        schema = with_defaults.parameters_schema()
        assert "required" in schema["required"]
        assert "optional" not in schema["required"]

    def test_schema_ignores_self(self):
        """Test that self parameter is ignored in schema."""

        class MyClass:
            @tool(name="method_tool", description="Method tool")
            def my_method(self, value: str) -> str:
                return value

        # Create instance and get the decorated tool
        instance = MyClass()
        method_tool = instance.my_method

        schema = method_tool.parameters_schema()
        assert "self" not in schema["properties"]
        assert "value" in schema["properties"]


class TestErrorHandling:
    """Tests for error handling scenarios."""

    @pytest.mark.asyncio
    async def test_execute_with_wrong_parameters(self):
        """Test executing with wrong parameters."""
        registry = ToolRegistry()
        tool_instance = SimpleTestTool()
        registry.register(tool_instance)

        # The tool expects 'input', but we pass something else
        # This should work but the value will be empty string
        result = await registry.execute("simple_test", wrong_param="value")
        assert result == "Processed: "

    @pytest.mark.asyncio
    async def test_tool_execution_error(self):
        """Test tool that raises an error during execution."""

        class ErrorTool(CustomTool):
            @property
            def name(self) -> str:
                return "error_tool"

            @property
            def description(self) -> str:
                return "An error tool"

            def parameters_schema(self) -> dict:
                return {"type": "object", "properties": {}}

            async def execute(self, **kwargs) -> str:
                raise RuntimeError("Intentional error")

        registry = ToolRegistry()
        registry.register(ErrorTool())

        with pytest.raises(RuntimeError, match="Intentional error"):
            await registry.execute("error_tool")

    def test_register_same_name_overwrites(self):
        """Test that registering with same name overwrites previous tool."""
        registry = ToolRegistry()

        class Tool1(CustomTool):
            @property
            def name(self) -> str:
                return "same_name"

            @property
            def description(self) -> str:
                return "Tool 1"

            def parameters_schema(self) -> dict:
                return {"type": "object", "properties": {}}

            async def execute(self, **kwargs) -> str:
                return "tool1"

        class Tool2(CustomTool):
            @property
            def name(self) -> str:
                return "same_name"

            @property
            def description(self) -> str:
                return "Tool 2"

            def parameters_schema(self) -> dict:
                return {"type": "object", "properties": {}}

            async def execute(self, **kwargs) -> str:
                return "tool2"

        registry.register(Tool1())
        registry.register(Tool2())

        tool = registry.get("same_name")
        assert tool.description == "Tool 2"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
