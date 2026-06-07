"""Custom Tool API

For creating and registering custom tools.
"""

import asyncio
import builtins
import inspect
from abc import ABC, abstractmethod
from collections.abc import Callable
from typing import Any, get_type_hints


class CustomTool(ABC):
    """Base class for custom tools

    Usage:
        from continuum_sdk.tools import CustomTool

        class MyTool(CustomTool):
            @property
            def name(self) -> str:
                return "my_tool"

            @property
            def description(self) -> str:
                return "My custom tool"

            def parameters_schema(self) -> Dict[str, Any]:
                return {
                    "type": "object",
                    "properties": {
                        "input": {"type": "string"}
                    },
                    "required": ["input"]
                }

            async def execute(self, **kwargs) -> str:
                return f"Processed: {kwargs['input']}"

        # Register
        registry.register(MyTool())
    """

    @property
    @abstractmethod
    def name(self) -> str:
        """Tool name"""
        ...  # pragma: no cover

    @property
    @abstractmethod
    def description(self) -> str:
        """Tool description"""
        ...  # pragma: no cover

    @abstractmethod
    def parameters_schema(self) -> dict[str, Any]:
        """Parameter JSON Schema"""
        ...  # pragma: no cover

    @abstractmethod
    async def execute(self, **kwargs) -> str:
        """Execute tool"""
        ...  # pragma: no cover

    @property
    def category(self) -> str:
        """Tool category"""
        return "other"

    @property
    def requires_confirmation(self) -> bool:
        """Whether user confirmation is required"""
        return False

    @property
    def is_dangerous(self) -> bool:
        """Whether this is a dangerous operation"""
        return False

    def to_meta(self) -> dict[str, Any]:
        """Convert to metadata dictionary"""
        return {
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters_schema(),
            "category": self.category,
            "requires_confirmation": self.requires_confirmation,
            "is_dangerous": self.is_dangerous,
        }


def tool(
    name: str,
    description: str,
    parameters: dict[str, Any] | None = None,
    requires_confirmation: bool = False,
    is_dangerous: bool = False,
) -> Callable:
    """Tool decorator

    Args:
        name: Tool name
        description: Tool description
        parameters: Parameter Schema (optional, auto-inferred)
        requires_confirmation: Whether confirmation is required
        is_dangerous: Whether dangerous

    Usage:
        from continuum_sdk.tools import tool

        @tool(name="add", description="Add two numbers")
        async def add(a: int, b: int) -> int:
            return a + b

        @tool(name="greet", description="Say hello")
        async def greet(name: str, greeting: str = "Hello") -> str:
            return f"{greeting}, {name}!"
    """

    def decorator(func: Callable) -> CustomTool:
        # Auto-infer parameter Schema
        inferred_params = parameters
        if inferred_params is None:
            hints = get_type_hints(func)
            sig = inspect.signature(func)

            properties = {}
            required = []

            for param_name, param in sig.parameters.items():
                if param_name == "self":
                    continue

                param_type = hints.get(param_name, str)
                prop = {"type": "string"}

                if param_type is int:
                    prop = {"type": "integer"}
                elif param_type is float:
                    prop = {"type": "number"}
                elif param_type is bool:
                    prop = {"type": "boolean"}
                elif param_type is list:
                    prop = {"type": "array"}
                elif param_type is dict:
                    prop = {"type": "object"}

                properties[param_name] = prop

                if param.default == inspect.Parameter.empty:
                    required.append(param_name)

            inferred_params = {
                "type": "object",
                "properties": properties,
                "required": required,
            }

        # Create dynamic class
        class DecoratedTool(CustomTool):
            @property
            def name(self) -> str:
                return name

            @property
            def description(self) -> str:
                return description

            def parameters_schema(self) -> dict[str, Any]:
                return inferred_params

            @property
            def requires_confirmation(self) -> bool:
                return requires_confirmation

            @property
            def is_dangerous(self) -> bool:
                return is_dangerous

            async def execute(self, **kwargs) -> str:
                result = func(**kwargs)
                if asyncio.iscoroutine(result):
                    result = await result
                return str(result)

        return DecoratedTool()

    return decorator


class ToolRegistry:
    """Tool registry

    Usage:
        from continuum_sdk.tools import ToolRegistry, CustomTool

        registry = ToolRegistry()

        # Register custom tool
        registry.register(MyTool())

        # List all tools
        tools = registry.list()

        # Execute tool
        result = await registry.execute("my_tool", input="test")
    """

    def __init__(self):
        """Initialize registry"""
        self._tools: dict[str, CustomTool] = {}

    def register(self, tool: CustomTool) -> None:
        """Register tool

        Args:
            tool: Custom tool instance
        """
        self._tools[tool.name] = tool

    def unregister(self, name: str) -> bool:
        """Unregister tool"""
        if name in self._tools:
            del self._tools[name]
            return True
        return False

    def get(self, name: str) -> CustomTool | None:
        """Get tool"""
        return self._tools.get(name)

    def list(self) -> list[CustomTool]:
        """List all tools"""
        return list(self._tools.values())

    def list_names(self) -> builtins.list[str]:
        """List all tool names"""
        return list(self._tools.keys())

    async def execute(self, name: str, **kwargs) -> str:
        """Execute tool

        Args:
            name: Tool name
            **kwargs: Tool parameters

        Returns:
            Execution result
        """
        tool = self.get(name)
        if tool is None:
            raise ValueError(f"Tool not found: {name}")
        return await tool.execute(**kwargs)

    def has_tool(self, name: str) -> bool:
        """Check if tool exists"""
        return name in self._tools

    def get_meta(self, name: str) -> dict[str, Any] | None:
        """Get tool metadata"""
        tool = self.get(name)
        return tool.to_meta() if tool else None


# Default registry instance
default_registry = ToolRegistry()


def register_tool(tool: CustomTool) -> None:
    """Register tool to default registry"""
    default_registry.register(tool)


def get_registry() -> ToolRegistry:
    """Get default registry"""
    return default_registry
