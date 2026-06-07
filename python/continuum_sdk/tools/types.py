"""
Tool Types

Shared type definitions for all tools.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class ToolCategory(Enum):
    """Tool category classification."""

    FILE_OPS = "file_ops"
    SEARCH = "search"
    SHELL = "shell"
    NETWORK = "network"
    CODE_ANALYSIS = "code_analysis"
    MEMORY = "memory"
    WORKFLOW = "workflow"
    SYSTEM = "system"
    OTHER = "other"


@dataclass
class ToolResult:
    """
    Tool execution result.

    Attributes:
        call_id: Unique call identifier
        name: Tool name
        content: Result content (text)
        is_error: Whether the result is an error
        duration_ms: Execution duration in milliseconds
        metadata: Additional metadata
    """

    call_id: str
    name: str
    content: str
    is_error: bool = False
    duration_ms: int = 0
    metadata: dict[str, Any] = field(default_factory=dict)

    def __str__(self) -> str:
        status = "ERROR" if self.is_error else "OK"
        return f"[{status}] {self.name} ({self.duration_ms}ms): {self.content[:200]}"


class ToolError(Exception):
    """
    Tool execution error.

    Attributes:
        call_id: Call identifier that caused the error
        name: Tool name
        message: Error message
    """

    def __init__(self, call_id: str, name: str, message: str):
        super().__init__(f"[{name}] {message}")
        self.call_id = call_id
        self.name = name
        self.message = message


class ToolNotAvailableError(Exception):
    """Raised when a tool is unavailable in the current mode."""

    def __init__(self, message: str):
        super().__init__(message)
        self.message = message


@dataclass
class ToolMeta:
    """
    Tool metadata.

    Attributes:
        name: Tool name
        description: Tool description
        category: Tool category
        requires_confirmation: Whether user confirmation is required
        is_dangerous: Whether the tool is dangerous
        parameters: JSON Schema for parameters
    """

    name: str
    description: str
    category: ToolCategory
    requires_confirmation: bool = False
    is_dangerous: bool = False
    parameters: dict[str, Any] = field(default_factory=dict)
