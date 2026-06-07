"""
MCP Adapter for Continuum

Integrates MCP (Model Context Protocol) tools into Continuum's tool system.
Uses MCPAdapt library to connect to 1000+ MCP servers.

Supported MCP servers: filesystem, puppeteer, github, slack, postgres, etc.
Transport protocols: stdio, SSE, WebSocket, Streamable HTTP

Usage:
    from continuum_sdk.tools.mcp_adapter import MCPToolRegistry

    # Connect to filesystem MCP server
    registry = MCPToolRegistry()
    registry.connect_stdio("filesystem", "uvx", ["mcp-server-filesystem"])

    # Get tools
    tools = registry.get_tools()

    # Use in agent
    agent = continuum.Agent(tools=tools)
"""

from __future__ import annotations

import asyncio
import json
import logging
import time
from dataclasses import dataclass, field
from functools import partial
from typing import Any, Callable

from continuum_sdk.utils import generate_short_id
from .types import ToolCategory, ToolError, ToolMeta, ToolResult

logger = logging.getLogger(__name__)

# Lazy imports for optional dependencies
_mcpadapt = None
_mcp = None


def _ensure_mcpadapt():
    """Ensure MCPAdapt is installed."""
    global _mcpadapt, _mcp
    if _mcpadapt is None:
        try:
            import mcp
            from mcpadapt.core import MCPAdapt, ToolAdapter

            _mcpadapt = (MCPAdapt, ToolAdapter)
            _mcp = mcp
        except ImportError as e:  # pragma: no cover - depends on whether mcpadapt is installed
            raise ImportError(
                "MCPAdapt is required for MCP tool integration. "
                "Install with: pip install mcpadapt"
            ) from e
    return _mcpadapt, _mcp


@dataclass
class MCPTool:
    """
    Adapted MCP tool for Continuum.

    Wraps an MCP tool call into Continuum's ToolMeta interface.
    """

    name: str
    description: str
    parameters: dict[str, Any]
    _call_func: Callable[[dict | None], Any]
    category: ToolCategory = ToolCategory.OTHER
    requires_confirmation: bool = False
    is_dangerous: bool = False

    def to_meta(self) -> ToolMeta:
        """Convert to ToolMeta."""
        return ToolMeta(
            name=self.name,
            description=self.description,
            category=self.category,
            requires_confirmation=self.requires_confirmation,
            is_dangerous=self.is_dangerous,
            parameters=self.parameters,
        )

    def execute(self, arguments: dict[str, Any] | None = None) -> ToolResult:
        """Execute the MCP tool synchronously."""
        call_id = generate_short_id()
        start_time = time.time()

        try:
            result = self._call_func(arguments)

            # Extract text content from MCP result
            if hasattr(result, "content") and result.content:
                content = result.content[0]
                if hasattr(content, "text"):
                    text = content.text
                else:
                    text = str(content)
            else:
                text = str(result)

            duration_ms = int((time.time() - start_time) * 1000)
            return ToolResult(
                call_id=call_id,
                name=self.name,
                content=text,
                is_error=False,
                duration_ms=duration_ms,
            )
        except Exception as e:
            # Tool execution may raise any exception, must catch for friendly error message
            # This is a public API design decision: don't propagate unhandled exceptions to callers
            logger.debug("MCP tool %s execution failed: %s", self.name, e)
            duration_ms = int((time.time() - start_time) * 1000)
            return ToolResult(
                call_id=call_id,
                name=self.name,
                content=str(e),
                is_error=True,
                duration_ms=duration_ms,
            )

    async def aexecute(self, arguments: dict[str, Any] | None = None) -> ToolResult:
        """Execute the MCP tool asynchronously."""
        call_id = generate_short_id()
        start_time = time.time()

        try:
            result = await self._call_func(arguments)

            # Extract text content from MCP result
            if hasattr(result, "content") and result.content:
                content = result.content[0]
                if hasattr(content, "text"):
                    text = content.text
                else:
                    text = str(content)
            else:
                text = str(result)

            duration_ms = int((time.time() - start_time) * 1000)
            return ToolResult(
                call_id=call_id,
                name=self.name,
                content=text,
                is_error=False,
                duration_ms=duration_ms,
            )
        except Exception as e:
            # Async tool execution may raise any exception, must catch for friendly error message
            logger.debug("MCP tool %s async execution failed: %s", self.name, e)
            duration_ms = int((time.time() - start_time) * 1000)
            return ToolResult(
                call_id=call_id,
                name=self.name,
                content=str(e),
                is_error=True,
                duration_ms=duration_ms,
            )


class ContinuumMCPAdapter:
    """
    MCPAdapt adapter for Continuum.

    Converts MCP tools to Continuum's ToolMeta format.
    """

    def __init__(
        self,
        category: ToolCategory = ToolCategory.OTHER,
        requires_confirmation: bool = False,
        dangerous_tools: set[str] | None = None,
    ):
        """
        Initialize the adapter.

        Args:
            category: Default category for MCP tools
            requires_confirmation: Whether tools require confirmation by default
            dangerous_tools: Set of tool names that are considered dangerous
        """
        (MCPAdapt, ToolAdapter) = _ensure_mcpadapt()[0]
        self._ToolAdapter = ToolAdapter
        self.category = category
        self.requires_confirmation = requires_confirmation
        self.dangerous_tools = dangerous_tools or {
            "delete_file",
            "execute_command",
            "run_shell",
            "write_file",
        }

    def adapt(self, func: Callable, mcp_tool: Any) -> MCPTool:
        """
        Adapt an MCP tool to Continuum format.

        Args:
            func: The MCP tool call function
            mcp_tool: The MCP tool definition

        Returns:
            MCPTool ready for Continuum use
        """
        import jsonref

        # Resolve JSON schema references
        input_schema = dict(mcp_tool.inputSchema)
        try:
            input_schema = {
                k: v
                for k, v in jsonref.replace_refs(input_schema).items()
                if k != "$defs"
            }
        except (ValueError, TypeError, KeyError, json.JSONDecodeError) as e:
            logger.debug("Failed to resolve JSON schema refs: %s", e)

        # Determine if tool is dangerous
        is_dangerous = mcp_tool.name.lower() in self.dangerous_tools

        return MCPTool(
            name=mcp_tool.name,
            description=mcp_tool.description or "",
            parameters=input_schema,
            _call_func=func,
            category=self.category,
            requires_confirmation=is_dangerous or self.requires_confirmation,
            is_dangerous=is_dangerous,
        )

    async def async_adapt(self, afunc: Callable, mcp_tool: Any) -> MCPTool:
        """Adapt an async MCP tool."""
        return self.adapt(afunc, mcp_tool)


class MCPToolRegistry:
    """
    Registry for MCP tools.

    Manages connections to MCP servers and provides access to adapted tools.

    Example:
        registry = MCPToolRegistry()

        # Connect to stdio-based MCP server
        registry.connect_stdio(
            "filesystem",
            command="uvx",
            args=["mcp-server-filesystem", "--root", "/path/to/project"]
        )

        # Connect to SSE-based MCP server
        registry.connect_sse("remote-tools", url="http://localhost:8000/sse")

        # Get all tools
        tools = registry.get_tools()

        # Close connections
        registry.close()
    """

    def __init__(self, timeout: int = 30):
        """
        Initialize the registry.

        Args:
            timeout: Connection timeout in seconds
        """
        (MCPAdapt, _) = _ensure_mcpadapt()[0]
        self._MCPAdapt = MCPAdapt
        self._mcp = _ensure_mcpadapt()[1]
        self.timeout = timeout
        self._connections: dict[str, Any] = {}
        self._tools: dict[str, list[MCPTool]] = {}

    def connect_stdio(
        self,
        name: str,
        command: str,
        args: list[str] | None = None,
        env: dict[str, str] | None = None,
        category: ToolCategory = ToolCategory.OTHER,
    ) -> list[MCPTool]:
        """
        Connect to a stdio-based MCP server.

        Args:
            name: Connection name
            command: Command to run
            args: Command arguments
            env: Environment variables
            category: Category for tools from this server

        Returns:
            List of adapted tools
        """
        import os

        args = args or []
        merged_env = {**os.environ, **(env or {})}

        params = self._mcp.StdioServerParameters(
            command=command,
            args=args,
            env=merged_env,
        )

        adapter = ContinuumMCPAdapter(category=category)
        client = self._MCPAdapt(params, adapter, connect_timeout=self.timeout)

        # Start the client
        client.start()
        self._connections[name] = client

        # Get tools
        tools = client.tools()
        mcp_tools = [t for t in tools if isinstance(t, MCPTool)]
        self._tools[name] = mcp_tools

        logger.info(
            f"Connected to MCP server '{name}' with {len(mcp_tools)} tools"
        )
        return mcp_tools

    def connect_sse(
        self,
        name: str,
        url: str,
        category: ToolCategory = ToolCategory.OTHER,
    ) -> list[MCPTool]:
        """
        Connect to an SSE-based MCP server.

        Args:
            name: Connection name
            url: SSE endpoint URL
            category: Category for tools from this server

        Returns:
            List of adapted tools
        """
        params = {"url": url, "transport": "sse"}

        adapter = ContinuumMCPAdapter(category=category)
        client = self._MCPAdapt(params, adapter, connect_timeout=self.timeout)

        client.start()
        self._connections[name] = client

        tools = client.tools()
        mcp_tools = [t for t in tools if isinstance(t, MCPTool)]
        self._tools[name] = mcp_tools

        logger.info(
            f"Connected to SSE MCP server '{name}' with {len(mcp_tools)} tools"
        )
        return mcp_tools

    def connect_websocket(
        self,
        name: str,
        url: str,
        category: ToolCategory = ToolCategory.OTHER,
    ) -> list[MCPTool]:
        """
        Connect to a WebSocket-based MCP server.

        Args:
            name: Connection name
            url: WebSocket endpoint URL
            category: Category for tools from this server

        Returns:
            List of adapted tools
        """
        params = {"url": url, "transport": "ws"}

        adapter = ContinuumMCPAdapter(category=category)
        client = self._MCPAdapt(params, adapter, connect_timeout=self.timeout)

        client.start()
        self._connections[name] = client

        tools = client.tools()
        mcp_tools = [t for t in tools if isinstance(t, MCPTool)]
        self._tools[name] = mcp_tools

        logger.info(
            f"Connected to WebSocket MCP server '{name}' with {len(mcp_tools)} tools"
        )
        return mcp_tools

    def get_tools(self, connection: str | None = None) -> list[MCPTool]:
        """
        Get all tools or tools from a specific connection.

        Args:
            connection: Optional connection name to filter tools

        Returns:
            List of MCPTool instances
        """
        if connection:
            return self._tools.get(connection, [])

        all_tools = []
        for tools in self._tools.values():
            all_tools.extend(tools)
        return all_tools

    def get_tool_metas(self, connection: str | None = None) -> list[ToolMeta]:
        """
        Get ToolMeta for all tools.

        Args:
            connection: Optional connection name to filter

        Returns:
            List of ToolMeta instances
        """
        return [t.to_meta() for t in self.get_tools(connection)]

    def refresh_tools(self, connection: str | None = None) -> None:
        """
        Refresh tools from MCP servers.

        Args:
            connection: Optional specific connection to refresh
        """
        connections = (
            {connection: self._connections[connection]}
            if connection
            else self._connections
        )

        for name, client in connections.items():
            try:
                tools = client.tools()
                mcp_tools = [t for t in tools if isinstance(t, MCPTool)]
                self._tools[name] = mcp_tools
                logger.debug(f"Refreshed {len(mcp_tools)} tools from '{name}'")
            except (ValueError, TypeError, KeyError, RuntimeError, ConnectionError) as e:
                logger.warning(f"Failed to refresh tools from '{name}': {e}")

    def close(self, connection: str | None = None) -> None:
        """
        Close MCP server connections.

        Args:
            connection: Optional specific connection to close
        """
        if connection:
            if connection in self._connections:
                self._connections[connection].close()
                del self._connections[connection]
                del self._tools[connection]
                logger.info(f"Closed MCP connection '{connection}'")
        else:
            for name, client in self._connections.items():
                try:
                    client.close()
                    logger.info(f"Closed MCP connection '{name}'")
                except (ValueError, TypeError, RuntimeError) as e:
                    logger.warning(f"Error closing '{name}': {e}")
            self._connections.clear()
            self._tools.clear()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        self.close()


# ============================================================================
# Predefined common MCP server configurations
# ============================================================================

PREDEFINED_MCP_SERVERS = {
    "filesystem": {
        "command": "uvx",
        "args": ["mcp-server-filesystem"],
        "category": ToolCategory.FILE_OPS,
        "description": "File system operations (read, write, list, search)",
    },
    "github": {
        "command": "uvx",
        "args": ["mcp-server-github"],
        "category": ToolCategory.OTHER,
        "description": "GitHub API operations (repos, issues, PRs)",
        "env": {"GITHUB_TOKEN": None},  # Will use env var if set
    },
    "puppeteer": {
        "command": "uvx",
        "args": ["mcp-server-puppeteer"],
        "category": ToolCategory.NETWORK,
        "description": "Browser automation and web scraping",
    },
    "slack": {
        "command": "uvx",
        "args": ["mcp-server-slack"],
        "category": ToolCategory.OTHER,
        "description": "Slack messaging and channel operations",
        "env": {"SLACK_BOT_TOKEN": None},
    },
    "postgres": {
        "command": "uvx",
        "args": ["mcp-server-postgres"],
        "category": ToolCategory.OTHER,
        "description": "PostgreSQL database operations",
        "env": {"DATABASE_URL": None},
    },
    "memory": {
        "command": "uvx",
        "args": ["mcp-server-memory"],
        "category": ToolCategory.MEMORY,
        "description": "Persistent memory storage for agents",
    },
}


def create_mcp_registry(
    servers: list[str] | None = None,
    root_path: str | None = None,
) -> MCPToolRegistry:
    """
    Create an MCP tool registry with predefined servers.

    Args:
        servers: List of server names to connect (default: ["filesystem"])
        root_path: Root path for filesystem server

    Returns:
        Configured MCPToolRegistry

    Example:
        # Connect to filesystem MCP server
        registry = create_mcp_registry(["filesystem"], "/path/to/project")
        tools = registry.get_tools()

        # Connect to multiple servers
        registry = create_mcp_registry(["filesystem", "github"])
    """
    import os

    servers = servers or ["filesystem"]
    registry = MCPToolRegistry()

    for server_name in servers:
        if server_name not in PREDEFINED_MCP_SERVERS:
            logger.warning(f"Unknown MCP server: {server_name}")
            continue

        config = PREDEFINED_MCP_SERVERS[server_name].copy()

        # Handle environment variables
        env = {}
        if "env" in config:
            for key, value in config["env"].items():
                if value is None:  # pragma: no cover - env vars typically set in tests
                    value = os.environ.get(key)
                if value:
                    env[key] = value
            del config["env"]

        # Special handling for filesystem
        if server_name == "filesystem" and root_path:
            config["args"] = config["args"] + ["--root", root_path]

        category = config.pop("category", ToolCategory.OTHER)
        config.pop("description", None)

        try:
            registry.connect_stdio(
                name=server_name,
                env=env if env else None,
                category=category,
                **config,
            )
        except (ValueError, TypeError, KeyError, RuntimeError, ConnectionError) as e:
            logger.error(f"Failed to connect to {server_name}: {e}")

    return registry


__all__ = [
    "MCPTool",
    "MCPToolRegistry",
    "ContinuumMCPAdapter",
    "create_mcp_registry",
    "PREDEFINED_MCP_SERVERS",
]
