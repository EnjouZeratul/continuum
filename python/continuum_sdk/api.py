"""
Continuum Unified API

This module provides a unified entry point for Continuum SDK,
abstracting over Rust bindings and Python implementations.

Usage:
    from continuum_sdk.api import Agent, Session, BuiltinTools

    # Create agent (auto-selects best implementation)
    agent = Agent()
    result = agent.run("task")

Implementation Selection:
    - If Rust binding (sh_python) available → use high-performance Rust implementation
    - Otherwise → fallback to pure Python implementation
"""

from __future__ import annotations

import logging
import os
from collections.abc import Callable
from typing import Any

logger = logging.getLogger(__name__)

# Detect Rust binding availability
try:
    from . import _continuum as _rust_binding  # pragma: no cover

    HAS_RUST_BINDING = True  # pragma: no cover
    logger.debug("Rust binding (continuum_sdk._continuum) available")  # pragma: no cover
except ImportError:  # pragma: no cover
    try:
        import sh_python as _rust_binding  # pragma: no cover

        HAS_RUST_BINDING = True  # pragma: no cover
        logger.debug("Rust binding (sh_python) available")  # pragma: no cover
    except ImportError:
        _rust_binding = None  # pragma: no cover
        HAS_RUST_BINDING = False  # pragma: no cover
        logger.debug("Rust binding not available - using pure Python mode")  # pragma: no cover

from .agent.session import Session


def get_implementation_preference() -> str:
    """
    Get the current implementation preference.

    Returns:
        "rust" if Rust binding available, otherwise "python"
    """
    # Allow override via environment variable
    override = os.environ.get("CONTINUUM_IMPL", "").lower()
    if override == "rust" and HAS_RUST_BINDING:
        return "rust"
    elif override == "python":
        return "python"
    else:
        return "python"


# ============================================================================
# Unified API Classes
# ============================================================================


class Agent:
    """
    Unified Agent API.

    Automatically selects the best available implementation:
    - Rust binding (sh_python) for high performance
    - Pure Python fallback for compatibility

    Example:
        >>> agent = Agent()
        >>> result = agent.run("Write a hello world program")
    """

    def __init__(
        self,
        name: str | None = None,
        model: str | None = None,
        provider: str | None = None,
        api_key: str | None = None,
        config: Any | None = None,
        *,
        impl: str | None = None,
        **kwargs: Any,
    ):
        """
        Initialize Agent.

        Args:
            name: Agent name (optional)
            model: Model identifier (optional, uses default)
            provider: Provider name (optional)
            api_key: API key (optional)
            config: Config object (optional)
            impl: Force implementation ("rust" or "python")
            **kwargs: Additional configuration
        """
        self._impl_type = impl or get_implementation_preference()
        self._name = name or "default"
        self._model = model

        kwargs.update({"config": config, "provider": provider, "api_key": api_key})

        if self._impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustAgent

            self._agent = RustAgent(name=self._name, model=self._model, **kwargs)
        else:
            from .python_impl import PythonAgent

            self._agent = PythonAgent(name=self._name, model=self._model, **kwargs)

    def run(self, task: str, **kwargs: Any) -> str:
        """Execute a task synchronously."""
        return self._agent.run(task, **kwargs)

    async def arun(self, task: str, **kwargs: Any) -> str:
        """Execute a task asynchronously."""
        return await self._agent.arun(task, **kwargs)

    def register_tool(
        self,
        name: str,
        func: Callable,
        description: str = "",
        parameters: dict | None = None,
    ) -> None:
        """Register a custom tool."""
        return self._agent.register_tool(name, func, description, parameters)

    def create_session(self, session_id: str | None = None) -> Session:
        """Create a new session."""
        return Session(id=session_id)

    @property
    def implementation(self) -> str:
        """Get current implementation type."""
        return self._impl_type


class BuiltinTools:
    """
    Unified Built-in Tools API.

    Provides access to file operations, search, and shell commands.
    """

    def __init__(self, *, impl: str | None = None):
        self._impl_type = impl or get_implementation_preference()

        if self._impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustBuiltinTools

            self._tools = RustBuiltinTools()
        else:
            from .python_impl import PythonBuiltinTools

            self._tools = PythonBuiltinTools()

    def read_file(
        self, path: str, offset: int | None = None, limit: int | None = None
    ) -> str:
        """Read file contents."""
        return self._tools.read_file(path, offset, limit)

    def write_file(self, path: str, content: str) -> str:
        """Write content to file."""
        return self._tools.write_file(path, content)

    def edit_file(self, path: str, old: str, new: str) -> str:
        """Edit file by replacing text."""
        return self._tools.edit_file(path, old, new)

    def grep(
        self, pattern: str, path: str | None = None, glob: str | None = None
    ) -> str:
        """Search file contents."""
        return self._tools.grep(pattern, path, glob)

    def glob(self, pattern: str, path: str | None = None) -> str:
        """Find files matching pattern."""
        return self._tools.glob(pattern, path)

    def bash(
        self,
        command: str,
        timeout_ms: int | None = None,
        working_dir: str | None = None,
    ) -> str:
        """Execute shell command."""
        return self._tools.bash(command, timeout_ms, working_dir)

    def list_tools(self) -> list[dict[str, str]]:
        """List available tools."""
        return self._tools.list_tools()

    @property
    def implementation(self) -> str:
        """Get current implementation type."""
        return self._impl_type


class QueryEngine:
    """
    Unified Query Engine API.

    Provides code analysis capabilities:
    - Go to definition
    - Find references
    - Hover information

    Example:
        >>> engine = QueryEngine()
        >>> engine.initialize("python", "/path/to/project")
        >>> refs = engine.find_references("python", "src/main.py", 10, 5)
    """

    def __init__(self, *, impl: str | None = None):
        self._impl_type = impl or get_implementation_preference()

        if self._impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustQueryEngine

            self._engine = RustQueryEngine()
        else:
            from .python_impl import PythonQueryEngine

            self._engine = PythonQueryEngine()

    def initialize(self, language: str, root_path: str) -> bool:
        """Initialize query engine for a language."""
        return self._engine.initialize(language, root_path)

    def go_to_definition(
        self, language: str, file_path: str, line: int, column: int
    ) -> list[dict[str, Any]]:
        """Find definition of symbol at position."""
        return self._engine.go_to_definition(language, file_path, line, column)

    def find_references(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
        include_declaration: bool = True,
    ) -> list[dict[str, Any]]:
        """Find all references to symbol."""
        return self._engine.find_references(
            language, file_path, line, column, include_declaration
        )

    def hover(
        self, language: str, file_path: str, line: int, column: int
    ) -> str | None:
        """Get hover information."""
        return self._engine.hover(language, file_path, line, column)

    def shutdown(self, language: str) -> None:
        """Shutdown engine for language."""
        return self._engine.shutdown(language)

    def is_connected(self, language: str) -> bool:
        """Check if connected."""
        return self._engine.is_connected(language)

    def full_symbol_info(
        self, language: str, file_path: str, line: int, column: int
    ) -> dict[str, Any]:
        """Get complete symbol information."""
        return self._engine.full_symbol_info(language, file_path, line, column)

    def get_document_symbols(self, language: str, file_path: str) -> list[dict[str, Any]]:
        """Get all symbols in a document."""
        return self._engine.get_document_symbols(language, file_path)

    def rename_symbol(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
        new_name: str,
    ) -> dict[str, Any]:
        """Rename a symbol across all references."""
        return self._engine.rename_symbol(language, file_path, line, column, new_name)

    def reconnect(self, language: str) -> bool:
        """Reconnect after error."""
        return self._engine.reconnect(language)

    def get_connection_pool_status(self) -> dict[str, Any]:
        """Get connection pool status."""
        return self._engine.get_connection_pool_status()


class MemorySystem:
    """
    Unified Memory System API.

    Tiered memory storage:
    - Working: Current conversation context
    - Session: Session-level facts
    - Project: Project knowledge
    - Long-term: Cross-project knowledge

    Example:
        >>> memory = MemorySystem()
        >>> id = memory.store("working", "User prefers Python")
        >>> results = memory.query("Python")
    """

    def __init__(self, session_id: str | None = None, *, impl: str | None = None):
        self._impl_type = impl or get_implementation_preference()

        if self._impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustMemorySystem

            self._memory = RustMemorySystem(session_id=session_id)
        else:
            from .python_impl import PythonMemorySystem

            self._memory = PythonMemorySystem(session_id=session_id)

    def store(self, tier: str, content: str) -> str:
        """Store memory, returns memory ID."""
        return self._memory.store(tier, content)

    def query(
        self, query: str, tier: str | None = None, limit: int = 10
    ) -> list[dict[str, Any]]:
        """Query memories."""
        return self._memory.query(query, tier, limit)

    def get(self, tier: str, memory_id: str) -> dict[str, Any] | None:
        """Get specific memory."""
        return self._memory.get(tier, memory_id)

    def stats(self) -> dict[str, int]:
        """Get memory statistics."""
        return self._memory.stats()

    def clear(self, tier: str) -> int:
        """Clear tier, returns count."""
        return self._memory.clear(tier)

    def delete(self, tier: str, memory_id: str) -> bool:
        """Delete a specific memory."""
        return self._memory.delete(tier, memory_id)

    def working(self):
        """Get working memory tier proxy."""
        return self._memory.working()

    def session(self):
        """Get session memory tier proxy."""
        return self._memory.session()

    def project(self):
        """Get project memory tier proxy."""
        return self._memory.project()

    def long_term(self):
        """Get long-term memory tier proxy."""
        return self._memory.long_term()

    def persist(self, path: str | None = None) -> bool:
        """Persist memory to storage."""
        return self._memory.persist(path)

    def load_from_storage(self, path: str) -> bool:
        """Load memory from storage."""
        return self._memory.load(path)


class MultimodalHandler:
    """
    Unified Multimodal Content Handler.

    Handles text, images, audio, and documents for LLM interactions.

    Example:
        >>> handler = MultimodalHandler()
        >>> msg = handler.create_image_message("user", "What's this?", ["photo.jpg"])
        >>> text = handler.extract_text(msg)
    """

    def __init__(self, *, impl: str | None = None):
        self._impl_type = impl or get_implementation_preference()

        if self._impl_type == "rust" and HAS_RUST_BINDING:
            # Rust binding doesn't have multimodal yet, use Python
            from .python_impl import PythonMultimodalHandler

            self._handler = PythonMultimodalHandler()
        else:
            from .python_impl import PythonMultimodalHandler

            self._handler = PythonMultimodalHandler()

    def encode_image(self, image_path: str, media_type: str | None = None) -> dict[str, Any]:
        """Encode image for LLM."""
        return self._handler.encode_image(image_path, media_type)

    def encode_document(self, doc_path: str, media_type: str | None = None) -> dict[str, Any]:
        """Encode document for LLM."""
        return self._handler.encode_document(doc_path, media_type)

    def create_message(self, role: str, content: str | list[dict[str, Any]]) -> dict[str, Any]:
        """Create multimodal message."""
        return self._handler.create_message(role, content)

    def create_image_message(
        self, role: str, text: str, image_paths: list[str]
    ) -> dict[str, Any]:
        """Create message with text and images."""
        return self._handler.create_image_message(role, text, image_paths)

    def extract_text(self, message: dict[str, Any]) -> str:
        """Extract text from message."""
        return self._handler.extract_text(message)

    def list_images(self, message: dict[str, Any]) -> list[dict[str, Any]]:
        """Extract images from message."""
        return self._handler.list_images(message)

    def encode_image_from_url(self, url: str, timeout: int = 30) -> dict[str, Any]:
        """Fetch and encode image from URL."""
        return self._handler.encode_image_from_url(url, timeout)

    def encode_image_url_direct(self, url: str) -> dict[str, Any]:
        """Encode image URL directly (for GPT-4 Vision)."""
        return self._handler.encode_image_url_direct(url)

    def to_openai_format(self, content: dict[str, Any]) -> dict[str, Any]:
        """Convert content to OpenAI API format."""
        return self._handler.to_openai_format(content)

    def create_openai_vision_message(
        self, role: str, text: str, images: list, detail: str = "auto"
    ) -> dict[str, Any]:
        """Create OpenAI Vision API compatible message."""
        return self._handler.create_openai_vision_message(role, text, images, detail)

    def create_anthropic_vision_message(
        self, role: str, text: str, images: list
    ) -> dict[str, Any]:
        """Create Anthropic Vision API compatible message."""
        return self._handler.create_anthropic_vision_message(role, text, images)


class ImageInput:
    """
    Unified Image Input type.

    Supports multiple formats:
    - Local file paths
    - URLs (http/https)
    - Base64-encoded data
    - Raw bytes

    Example:
        >>> from continuum_sdk.api import ImageInput
        >>> img = ImageInput.from_path("photo.jpg")
        >>> openai_msg = img.to_openai_format()
        >>> anthropic_msg = img.to_anthropic_format()
    """

    def __init__(
        self,
        source: str | bytes | None = None,
        *,
        path: str | None = None,
        url: str | None = None,
        base64_data: str | None = None,
        media_type: str | None = None,
    ):
        """Initialize image input."""
        from .python_impl import ImageInput as PyImageInput

        self._impl = PyImageInput(
            source=source,
            path=path,
            url=url,
            base64_data=base64_data,
            media_type=media_type,
        )

    @classmethod
    def from_path(cls, path: str, media_type: str | None = None) -> ImageInput:
        """Create from file path."""
        instance = cls.__new__(cls)
        from .python_impl import ImageInput as PyImageInput
        instance._impl = PyImageInput.from_path(path, media_type)
        return instance

    @classmethod
    def from_url(cls, url: str) -> ImageInput:
        """Create from URL."""
        instance = cls.__new__(cls)
        from .python_impl import ImageInput as PyImageInput
        instance._impl = PyImageInput.from_url(url)
        return instance

    @classmethod
    def from_base64(cls, data: str, media_type: str = "image/jpeg") -> ImageInput:
        """Create from base64 data."""
        instance = cls.__new__(cls)
        from .python_impl import ImageInput as PyImageInput
        instance._impl = PyImageInput.from_base64(data, media_type)
        return instance

    @classmethod
    def from_bytes(cls, data: bytes, media_type: str = "image/jpeg") -> ImageInput:
        """Create from raw bytes."""
        instance = cls.__new__(cls)
        from .python_impl import ImageInput as PyImageInput
        instance._impl = PyImageInput.from_bytes(data, media_type)
        return instance

    def to_base64(self) -> str:
        """Get base64-encoded data (lazy load)."""
        return self._impl.to_base64()

    @property
    def media_type(self) -> str:
        """Get MIME type."""
        return self._impl.media_type

    def to_anthropic_format(self) -> dict[str, Any]:
        """Convert to Anthropic API format."""
        return self._impl.to_anthropic_format()

    def to_openai_format(self, detail: str = "auto") -> dict[str, Any]:
        """Convert to OpenAI API format."""
        return self._impl.to_openai_format(detail)

    @property
    def source_type(self) -> str:
        """Get source type (path, url, base64, bytes)."""
        return self._impl.source_type


# ============================================================================
# Module exports
# ============================================================================

__all__ = [
    "Agent",
    "Session",
    "BuiltinTools",
    "QueryEngine",
    "MemorySystem",
    "MultimodalHandler",
    "ImageInput",
    "PermissionManager",
    "Permission",
    "Role",
    "HAS_RUST_BINDING",
    "get_implementation_preference",
]


class PermissionManager:
    """
    Unified Permission Manager API.

    RBAC permission management with default roles:
    - admin: All permissions
    - user: Basic permissions (session, tool, agent)
    - guest: Read-only session access

    Example:
        >>> pm = PermissionManager()
        >>> pm.grant("user1", "admin")
        >>> pm.check("user1", "session", "read")
        True
    """

    def __init__(self, *, impl: str | None = None):
        self._impl_type = impl or get_implementation_preference()

        if self._impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustPermissionManager

            self._manager = RustPermissionManager()
        else:
            from .python_impl import PythonPermissionManager

            self._manager = PythonPermissionManager()

    def check(self, user_id: str, resource: str, action: str) -> bool:
        """Check if user has permission."""
        return self._manager.check(user_id, resource, action)

    def grant(self, user_id: str, role_name: str) -> None:
        """Grant role to user."""
        return self._manager.grant(user_id, role_name)

    def revoke(self, user_id: str, role_name: str) -> None:
        """Revoke role from user."""
        return self._manager.revoke(user_id, role_name)

    def create_role(self, role: Role) -> None:
        """Create custom role."""
        return self._manager.create_role(role)

    def get_permissions(self, user_id: str) -> list[dict[str, str]]:
        """Get all permissions for user."""
        return self._manager.get_permissions(user_id)

    def is_admin(self, user_id: str) -> bool:
        """Check if user has admin privileges."""
        return self._manager.is_admin(user_id)

    def get_user_roles(self, user_id: str) -> list[str]:
        """Get user's roles."""
        return self._manager.get_user_roles(user_id)


class Permission:
    """
    Unified Permission type.

    Example:
        >>> p = Permission("session", "read")
        >>> p.resource
        'session'
    """

    def __init__(self, resource: str, action: str, *, impl: str | None = None):
        impl_type = impl or get_implementation_preference()

        if impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustPermission

            self._permission = RustPermission(resource, action)
        else:
            from .python_impl import PythonPermission

            self._permission = PythonPermission(resource, action)

    @property
    def resource(self) -> str:
        return self._permission.resource

    @property
    def action(self) -> str:
        return self._permission.action

    def __repr__(self) -> str:
        return f"Permission(resource='{self.resource}', action='{self.action}')"


class Role:
    """
    Unified Role type.

    Example:
        >>> r = Role("custom", [Permission("resource", "read")])
        >>> r.name
        'custom'
    """

    def __init__(
        self, name: str, permissions: list[Permission] | None = None, *, impl: str | None = None
    ):
        impl_type = impl or get_implementation_preference()

        if impl_type == "rust" and HAS_RUST_BINDING:
            from .rust_impl import RustPermission, RustRole

            perms = permissions or []
            rust_perms = [RustPermission(p.resource, p.action) for p in perms]
            self._role = RustRole(name, rust_perms)
        else:
            from .python_impl import PythonPermission, PythonRole

            perms = permissions or []
            py_perms = [PythonPermission(p.resource, p.action) for p in perms]
            self._role = PythonRole(name, py_perms)

    @property
    def name(self) -> str:
        return self._role.name

    @property
    def permissions(self) -> list[Permission]:
        """Get permissions as unified Permission objects."""
        return [Permission(p.resource, p.action) for p in self._role.permissions]

    def __repr__(self) -> str:
        return f"Role(name='{self.name}', permissions={len(self.permissions)})"
