"""
Continuum SDK Tools Module

Real tool implementations for file operations, search, and shell execution.

Tools:
    - BashTool: Safe command execution with timeout and output capture
    - ReadTool: File reading with pagination and encoding detection
    - WriteTool: Safe file writing with backup
    - EditTool: Precise string replacement in files
    - GrepTool: Regex content search
    - GlobTool: File pattern matching
    - WebSearchTool: Web search with multiple engines

Quick Start:
    >>> from continuum_sdk.tools import BashTool, ReadTool, WriteTool, WebSearchTool
    >>>
    >>> # Bash
    >>> bash = BashTool()
    >>> result = bash.run("echo hello")
    >>>
    >>> # Read
    >>> reader = ReadTool()
    >>> content = reader.read("config.toml")
    >>>
    >>> # Write
    >>> writer = WriteTool()
    >>> writer.write("output.txt", "Hello!")
    >>>
    >>> # Web Search
    >>> search = WebSearchTool()
    >>> results = search.search("Python async programming")
"""

# Tool types
# Real tool implementations
from .bash import BashTool, bash_execute, bash_execute_sync, validate_command

# Legacy compatibility (custom tools)
from .custom import CustomTool, ToolRegistry, get_registry, register_tool, tool
from .file_ops import (
    EditTool,
    ReadTool,
    WriteTool,
    detect_encoding,
    edit_file,
    read_file,
    write_file,
)
from .search import (
    GlobTool,
    GrepTool,
    glob,
    grep,
)
from .types import ToolCategory, ToolError, ToolMeta, ToolNotAvailableError, ToolResult

# Web Search
from .web import (
    WebSearchTool,
    SearchEngine,
    SearchResult,
    SearchResponse,
    web_search,
    duckduckgo,
    google,
    bing,
)

# MCP Adapter (optional, requires mcpadapt)
try:
    from .mcp_adapter import (
        PREDEFINED_MCP_SERVERS,
        MCPTool,
        MCPToolRegistry,
        ContinuumMCPAdapter,
        create_mcp_registry,
    )
    _MCP_AVAILABLE = True
except ImportError:
    _MCP_AVAILABLE = False
    MCPToolRegistry = None  # type: ignore
    MCPTool = None  # type: ignore
    ContinuumMCPAdapter = None  # type: ignore
    create_mcp_registry = None  # type: ignore
    PREDEFINED_MCP_SERVERS = {}  # type: ignore

# BuiltinTools (unified API)
from .builtin import BuiltinTools, get_builtin_tools

__all__ = [
    # Types
    "ToolResult",
    "ToolError",
    "ToolNotAvailableError",
    "ToolMeta",
    "ToolCategory",
    # Bash
    "BashTool",
    "bash_execute",
    "bash_execute_sync",
    "validate_command",
    # Read
    "ReadTool",
    "read_file",
    "detect_encoding",
    # Write
    "WriteTool",
    "write_file",
    # Edit
    "EditTool",
    "edit_file",
    # File Search
    "GrepTool",
    "GlobTool",
    "grep",
    "glob",
    # Web Search
    "WebSearchTool",
    "SearchEngine",
    "SearchResult",
    "SearchResponse",
    "web_search",
    "duckduckgo",
    "google",
    "bing",
    # Custom tools
    "CustomTool",
    "ToolRegistry",
    "tool",
    "register_tool",
    "get_registry",
    # BuiltinTools (unified)
    "BuiltinTools",
    "get_builtin_tools",
    # MCP (optional)
    "_MCP_AVAILABLE",
    "MCPToolRegistry",
    "MCPTool",
    "ContinuumMCPAdapter",
    "create_mcp_registry",
    "PREDEFINED_MCP_SERVERS",
]
