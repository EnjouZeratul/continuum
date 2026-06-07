"""Custom Tools Example

Demonstrates how to create and register custom tools.
"""

import asyncio

from continuum_sdk.tools import (
    CustomTool,
    get_registry,
    register_tool,
    tool,
)

# ==================== Method 1: Inherit from CustomTool ====================


class CalculatorTool(CustomTool):
    """Calculator tool"""

    @property
    def name(self) -> str:
        return "calculator"

    @property
    def description(self) -> str:
        return "Perform basic mathematical operations"

    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "Operation type",
                },
                "a": {"type": "number", "description": "First number"},
                "b": {"type": "number", "description": "Second number"},
            },
            "required": ["operation", "a", "b"],
        }

    @property
    def category(self) -> str:
        return "math"

    async def execute(self, **kwargs) -> str:
        op = kwargs["operation"]
        a, b = kwargs["a"], kwargs["b"]

        ops = {
            "add": lambda x, y: x + y,
            "subtract": lambda x, y: x - y,
            "multiply": lambda x, y: x * y,
            "divide": lambda x, y: x / y if y != 0 else "Error: division by zero",
        }

        result = ops[op](a, b)
        return f"{a} {op} {b} = {result}"


# ==================== Method 2: Use @tool Decorator ====================


@tool(name="greet", description="Generate greeting message", requires_confirmation=False)
async def greet_user(name: str, greeting: str = "Hello") -> str:
    """Generate personalized greeting"""
    return f"{greeting}, {name}!"


@tool(name="format_json", description="Format JSON string")
async def format_json_string(data: str, indent: int = 2) -> str:
    """Format JSON"""
    import json

    try:
        parsed = json.loads(data)
        return json.dumps(parsed, indent=indent, ensure_ascii=False)
    except json.JSONDecodeError as e:
        return f"Error: {e}"


# ==================== Method 3: Dangerous Tool (Requires Confirmation) ====================


@tool(
    name="delete_temp_files",
    description="Delete temporary files",
    is_dangerous=True,
    requires_confirmation=True,
)
async def delete_temp_files(pattern: str) -> str:
    """Delete temporary files matching pattern"""
    # Actual implementation requires file operations
    return f"[Simulated] Deleted temporary files matching '{pattern}'"


# ==================== Demo ====================


async def main():
    print("=== Custom Tools Example ===\n")

    # Get registry
    registry = get_registry()

    # 1. Register tools
    print("1. Register tools")
    register_tool(CalculatorTool())  # Use default registry
    registry.register(greet_user)  # Decorator already created instance
    registry.register(format_json_string)
    registry.register(delete_temp_files)
    print(f"   Registered {len(registry.list_names())} tools\n")

    # 2. List all tools
    print("2. Registered tools list:")
    for t in registry.list():
        danger_flag = " [Dangerous]" if t.is_dangerous else ""
        confirm_flag = " [Requires Confirmation]" if t.requires_confirmation else ""
        print(f"   - {t.name}: {t.description}{danger_flag}{confirm_flag}")
    print()

    # 3. Execute tools
    print("3. Execute tools")

    # Calculator
    result = await registry.execute("calculator", operation="add", a=10, b=5)
    print(f"   calculator: {result}")

    # Greeting
    result = await registry.execute("greet", name="World", greeting="Hi")
    print(f"   greet: {result}")

    # JSON formatting
    result = await registry.execute("format_json", data='{"name":"test","value":123}')
    print(f"   format_json:\n{result}")

    # 4. Get tool metadata
    print("\n4. Tool metadata:")
    meta = registry.get_meta("calculator")
    if meta:
        print(f"   Name: {meta['name']}")
        print(f"   Description: {meta['description']}")
        print(f"   Category: {meta['category']}")
        print(f"   Parameters: {meta['parameters']['required']}")


if __name__ == "__main__":
    asyncio.run(main())