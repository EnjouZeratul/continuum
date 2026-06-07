"""
Python Implementation Adapters

Pure Python fallback implementations for unified API compatibility.
This module is used when Rust binding is not available.
"""

from __future__ import annotations

import logging
from collections.abc import Callable
from datetime import datetime
from typing import Any

from .config.providers import get_default_model
from .utils import generate_short_id

logger = logging.getLogger(__name__)


class PythonAgent:
    """Pure Python Agent implementation."""

    def __init__(
        self, name: str = "default", model: str | None = None, **kwargs: Any
    ):
        self._name = name
        self._model = model or get_default_model("anthropic")
        self._tools: dict[str, Callable] = {}
        self._sessions: dict[str, PythonSession] = {}
        self._state = "idle"

        # Import Python implementations
        from .agent.runtime import Agent as InternalAgent
        from .config import Config

        base_config = kwargs.get("config") or Config.from_env()
        self._config = Config.from_dict(base_config.to_dict())
        overrides = {
            key: value
            for key, value in {
                "provider": kwargs.get("provider"),
                "api_key": kwargs.get("api_key"),
                "model": model,
            }.items()
            if value is not None
        }
        self._config.update(overrides)
        self._model = self._config.model
        self._internal_agent = InternalAgent(config=self._config, _use_rust=False)

    def run(self, task: str, **kwargs: Any) -> str:
        """Execute task synchronously."""
        self._state = "running"
        try:
            result = self._internal_agent.run(task)
            self._state = "idle"
            return result
        except Exception as e:
            self._state = "error"
            logger.error(f"Agent execution failed: {e}")
            raise

    async def arun(self, task: str, **kwargs: Any) -> str:
        """Execute task asynchronously."""
        # Use internal agent's async support
        return await self._internal_agent.execute_async(task)

    def register_tool(
        self,
        name: str,
        func: Callable,
        description: str = "",
        parameters: dict | None = None,
    ) -> None:
        """Register a custom tool."""
        self._tools[name] = func
        self._internal_agent.register_tool(name, func, description, parameters)

    def create_session(self) -> PythonSession:
        """Create a new session."""
        session_id = generate_short_id()
        session = PythonSession(session_id=session_id)
        self._sessions[session_id] = session
        return session

    @property
    def state(self) -> str:
        """Get agent state."""
        return self._state


class PythonSession:
    """Pure Python Session implementation."""

    def __init__(self, session_id: str | None = None):
        self._id = session_id or generate_short_id()
        self._messages: list[dict[str, str]] = []
        self._created_at = datetime.now()
        self._metadata: dict[str, Any] = {}

    def add_message(self, role: str, content: str) -> None:
        """Add a message."""
        self._messages.append({
            "role": role,
            "content": content,
            "timestamp": datetime.now().isoformat(),
        })

    def get_messages(self) -> list[dict[str, str]]:
        """Get all messages."""
        return [{"role": m["role"], "content": m["content"]} for m in self._messages]

    def save(self) -> str:
        """Save session."""
        data = {
            "id": self._id,
            "created_at": self._created_at.isoformat(),
            "messages": self._messages,
            "metadata": self._metadata,
        }
        return json.dumps(data)

    def load(self, data: str) -> None:
        """Load from saved data."""
        parsed = json.loads(data)
        self._id = parsed.get("id", self._id)
        self._messages = parsed.get("messages", [])
        self._metadata = parsed.get("metadata", {})

    @property
    def id(self) -> str:
        """Get session ID."""
        return self._id

    def set_metadata(self, key: str, value: Any) -> None:
        """Set metadata."""
        self._metadata[key] = value

    def get_metadata(self, key: str) -> Any:
        """Get metadata."""
        return self._metadata.get(key)


class PythonBuiltinTools:
    """Pure Python BuiltinTools implementation."""

    def __init__(self):
        # Import Python implementations
        from .tools.builtin import BuiltinTools as InternalTools

        self._tools = InternalTools()

    def read_file(
        self, path: str, offset: int | None = None, limit: int | None = None
    ) -> str:
        """Read file."""
        return self._tools.read_file(path, offset, limit)

    def write_file(self, path: str, content: str) -> str:
        """Write file."""
        return self._tools.write_file(path, content)

    def edit_file(self, path: str, old: str, new: str) -> str:
        """Edit file."""
        return self._tools.edit_file(path, old, new)

    def grep(
        self, pattern: str, path: str | None = None, glob: str | None = None
    ) -> str:
        """Search content."""
        return self._tools.grep(pattern, path, glob)

    def glob(self, pattern: str, path: str | None = None) -> str:
        """Find files."""
        return self._tools.glob(pattern, path)

    def bash(
        self,
        command: str,
        timeout_ms: int | None = None,
        working_dir: str | None = None,
    ) -> str:
        """Execute command."""
        return self._tools.bash(command, timeout_ms, working_dir)

    def list_tools(self) -> list[dict[str, str]]:
        """List tools."""
        tools = self._tools.list_tools()
        return [{"name": t.name, "description": t.description} for t in tools]

    def is_available(self, name: str) -> bool:
        """Check tool availability."""
        return self._tools.is_available(name)

    def execute(self, name: str, args: dict[str, Any]) -> str:
        """Execute tool by name."""
        return self._tools.execute(name, args)

import json
import re
from pathlib import Path

__all__ = [
    "PythonAgent",
    "PythonSession",
    "PythonBuiltinTools",
    "PythonQueryEngine",
    "PythonMemorySystem",
    "PythonMultimodalHandler",
    "PythonPermissionManager",
    "PythonPermission",
    "PythonRole",
    "ImageInput",
    "TierProxy",
]


class PythonPermission:
    """Pure Python Permission implementation."""

    def __init__(self, resource: str, action: str):
        self._resource = resource
        self._action = action

    @property
    def resource(self) -> str:
        return self._resource

    @property
    def action(self) -> str:
        return self._action

    def __repr__(self) -> str:
        return f"Permission(resource='{self.resource}', action='{self.action}')"

    def __eq__(self, other: object) -> bool:
        if isinstance(other, PythonPermission):
            return self.resource == other.resource and self.action == other.action
        return False

    def __hash__(self) -> int:
        return hash((self.resource, self.action))


class PythonRole:
    """Pure Python Role implementation."""

    def __init__(self, name: str, permissions: list[PythonPermission] | None = None):
        self._name = name
        self._permissions = permissions or []

    @property
    def name(self) -> str:
        return self._name

    @property
    def permissions(self) -> list[PythonPermission]:
        return self._permissions

    def __repr__(self) -> str:
        return f"Role(name='{self.name}', permissions={len(self.permissions)})"


class PythonPermissionManager:
    """Pure Python PermissionManager implementation.

    RBAC permission management system.

    Default roles:
    - admin: All permissions (*, *)
    - user: session:read, session:write, tool:execute, agent:run
    - guest: session:read only

    Example:
        >>> pm = PythonPermissionManager()
        >>> pm.grant("user1", "admin")
        >>> pm.check("user1", "session", "read")
        True
        >>> pm.revoke("user1", "admin")
    """

    def __init__(self):
        self._roles: dict[str, set[tuple[str, str]]] = self._default_roles()
        self._user_roles: dict[str, set[str]] = {}

    def _default_roles(self) -> dict[str, set[tuple[str, str]]]:
        """Create default roles."""
        return {
            "admin": {("*", "*")},
            "user": {
                ("session", "read"),
                ("session", "write"),
                ("tool", "execute"),
                ("agent", "run"),
            },
            "guest": {("session", "read")},
        }

    def check(self, user_id: str, resource: str, action: str) -> bool:
        """Check if user has permission for resource:action."""
        roles = self._user_roles.get(user_id, {"guest"})

        for role_name in roles:
            if role_name in self._roles:
                for r, a in self._roles[role_name]:
                    if (r == "*" or r == resource) and (a == "*" or a == action):
                        return True

        return False

    def grant(self, user_id: str, role_name: str) -> None:
        """Grant role to user."""
        if user_id not in self._user_roles:
            self._user_roles[user_id] = set()
        self._user_roles[user_id].add(role_name)

    def revoke(self, user_id: str, role_name: str) -> None:
        """Revoke role from user."""
        if user_id in self._user_roles:
            self._user_roles[user_id].discard(role_name)
            if not self._user_roles[user_id]:
                del self._user_roles[user_id]

    def create_role(self, role: PythonRole) -> None:
        """Create custom role."""
        perms = {(p.resource, p.action) for p in role.permissions}
        self._roles[role.name] = perms

    def get_permissions(self, user_id: str) -> list[dict[str, str]]:
        """Get all permissions for user."""
        roles = self._user_roles.get(user_id, {"guest"})
        perms: set[tuple[str, str]] = set()

        for role_name in roles:
            if role_name in self._roles:
                perms.update(self._roles[role_name])

        return [{"resource": r, "action": a} for r, a in perms]

    def is_admin(self, user_id: str) -> bool:
        """Check if user has admin privileges."""
        return self.check(user_id, "*", "*")

    def get_user_roles(self, user_id: str) -> list[str]:
        """Get user's roles."""
        return list(self._user_roles.get(user_id, {"guest"}))


class PythonQueryEngine:
    """Pure Python QueryEngine implementation.

    Provides code analysis capabilities using regex-based parsing.
    For full LSP support, use the Rust binding (RustQueryEngine).
    """

    def __init__(self):
        self._initialized_languages: set[str] = set()
        self._root_paths: dict[str, Path] = {}

    def initialize(self, language: str, root_path: str) -> bool:
        """Initialize query engine for a language.

        Args:
            language: Language identifier (python, rust, typescript, etc.)
            root_path: Root directory for analysis

        Returns:
            True if initialization successful
        """
        lang = language.lower()
        path = Path(root_path).expanduser().resolve()

        if not path.exists():
            raise ValueError(f"Root path does not exist: {path}")

        self._initialized_languages.add(lang)
        self._root_paths[lang] = path
        return True

    def go_to_definition(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
    ) -> list[dict[str, Any]]:
        """Find definition of symbol at position.

        Args:
            language: Language identifier
            file_path: File path
            line: Line number (1-based)
            column: Column number (1-based)

        Returns:
            List of definition locations with 'uri', 'line', 'column'
        """
        from .tools.lsp import go_to_definition
        try:
            result = go_to_definition(
                file_path, line, column,
                search_dir=str(self._root_paths.get(language.lower(), Path(file_path).parent))
            )
            if result.metadata and "file" in result.metadata:
                return [{
                    "uri": result.metadata["file"],
                    "line": result.metadata.get("line", 1),
                    "column": 1,
                }]
            return []
        except (OSError, json.JSONDecodeError):
            return []

    def find_references(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
        include_declaration: bool = True,
    ) -> list[dict[str, Any]]:
        """Find all references to symbol at position.

        Args:
            language: Language identifier
            file_path: File path
            line: Line number (1-based)
            column: Column number (1-based)
            include_declaration: Include declaration in results

        Returns:
            List of reference locations
        """
        from .tools.lsp import find_references
        try:
            result = find_references(
                file_path, line, column,
                search_dir=str(self._root_paths.get(language.lower(), Path(file_path).parent)),
                include_declaration=include_declaration
            )
            if result.content:
                refs = []
                for line_content in result.content.split("\n"):
                    if ":" in line_content:
                        parts = line_content.split(":")
                        if len(parts) >= 3:
                            refs.append({
                                "uri": parts[0],
                                "line": int(parts[1]),
                                "column": int(parts[2].split()[0]),
                            })
                return refs
            return []
        except (OSError, json.JSONDecodeError):
            return []

    def hover(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
    ) -> str | None:
        """Get hover information for symbol at position.

        Args:
            language: Language identifier
            file_path: File path
            line: Line number (1-based)
            column: Column number (1-based)

        Returns:
            Hover text or None
        """
        from .tools.lsp import get_hover
        try:
            result = get_hover(file_path, line, column)
            return result.content if result.content else None
        except OSError:
            return None

    def shutdown(self, language: str) -> None:
        """Shutdown query engine for language."""
        self._initialized_languages.discard(language.lower())
        self._root_paths.pop(language.lower(), None)

    def is_connected(self, language: str) -> bool:
        """Check if query engine is connected for language."""
        return language.lower() in self._initialized_languages

    def full_symbol_info(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
    ) -> dict[str, Any]:
        """Get complete symbol information.

        Args:
            language: Language identifier
            file_path: File path
            line: Line number (1-based)
            column: Column number (1-based)

        Returns:
            Dict with symbol name, kind, definition, references, hover
        """
        symbol_info = {
            "symbol": None,
            "kind": None,
            "definition": None,
            "references": [],
            "hover": None,
        }

        # Get hover for symbol extraction
        hover_text = self.hover(language, file_path, line, column)
        if hover_text:
            symbol_info["hover"] = hover_text
            # Extract symbol name from hover
            import re
            match = re.search(r"\*\*(\w+)\*\*", hover_text)
            if match:
                symbol_info["symbol"] = match.group(1)

        # Get definition
        defs = self.go_to_definition(language, file_path, line, column)
        if defs:
            symbol_info["definition"] = defs[0]

        # Get references
        refs = self.find_references(language, file_path, line, column)
        symbol_info["references"] = refs

        # Determine symbol kind from hover text
        if hover_text:
            kind_map = {
                "function": "function",
                "class": "class",
                "struct": "struct",
                "enum": "enum",
                "interface": "interface",
                "variable": "variable",
                "const": "constant",
            }
            for key, kind in kind_map.items():
                if f"({key})" in hover_text.lower() or key in hover_text.lower():
                    symbol_info["kind"] = kind
                    break

        return symbol_info

    def get_document_symbols(
        self,
        language: str,
        file_path: str,
    ) -> list[dict[str, Any]]:
        """Get all symbols in a document.

        Args:
            language: Language identifier
            file_path: File path

        Returns:
            List of symbols with name, kind, line, column
        """
        try:
            path = Path(file_path).expanduser().resolve()
            if not path.exists():
                return []

            content = path.read_text(encoding="utf-8", errors="replace")
            lines = content.split("\n")

            symbols = []
            file_ext = path.suffix.lower()

            # Definition patterns by language
            patterns = {
                ".py": [(r"def\s+(\w+)\s*\(", "function"), (r"class\s+(\w+)\s*[:\(]", "class")],
                ".rs": [(r"fn\s+(\w+)\s*[<(]", "function"), (r"struct\s+(\w+)\s*[{{<]", "struct"), (r"enum\s+(\w+)\s*[{{<]", "enum")],
                ".ts": [(r"function\s+(\w+)\s*[<(]", "function"), (r"class\s+(\w+)\s*[{{<]", "class"), (r"interface\s+(\w+)\s*[{{<]", "interface")],
                ".go": [(r"func\s+(\w+)\s*\(", "function"), (r"type\s+(\w+)\s+struct", "struct")],
            }

            lang_patterns = patterns.get(file_ext, [])

            for i, line_content in enumerate(lines, start=1):
                for pattern, kind in lang_patterns:
                    for match in re.finditer(pattern, line_content):
                        symbols.append({
                            "name": match.group(1),
                            "kind": kind,
                            "line": i,
                            "column": match.start(1) + 1,
                        })

            return symbols
        except (OSError, json.JSONDecodeError):
            return []

    def rename_symbol(
        self,
        language: str,
        file_path: str,
        line: int,
        column: int,
        new_name: str,
    ) -> dict[str, Any]:
        """Rename a symbol across all references.

        Args:
            language: Language identifier
            file_path: File path
            line: Line number (1-based)
            column: Column number (1-based)
            new_name: New symbol name

        Returns:
            Dict with changed_files count and preview of changes
        """
        # Get all references
        refs = self.find_references(language, file_path, line, column)

        if not refs:
            return {"changed_files": 0, "changes": []}

        # Group by file
        file_refs: dict[str, list[dict[str, Any]]] = {}
        for ref in refs:
            uri = ref["uri"]
            if uri not in file_refs:
                file_refs[uri] = []
            file_refs[uri].append(ref)

        changes = []
        for uri, positions in file_refs.items():
            try:
                path = Path(uri)
                content = path.read_text(encoding="utf-8", errors="replace")
                lines = content.split("\n")

                # Extract old symbol name
                old_name = None
                for ref in positions:
                    ref_line = ref["line"] - 1
                    if 0 <= ref_line < len(lines):
                        line_content = lines[ref_line]
                        col = ref["column"] - 1
                        # Extract identifier
                        import re
                        match = re.search(r"\b(\w+)\b", line_content[col:])
                        if match:
                            old_name = match.group(1)
                            break

                if old_name:
                    changes.append({
                        "file": uri,
                        "old_name": old_name,
                        "new_name": new_name,
                        "locations": positions,
                    })
            except (OSError, PermissionError, UnicodeDecodeError):
                continue

        return {
            "changed_files": len(changes),
            "changes": changes,
            "preview": f"Would rename in {len(changes)} files",
        }

    def reconnect(self, language: str) -> bool:
        """Attempt to reconnect after error.

        Args:
            language: Language to reconnect

        Returns:
            True if reconnection successful
        """
        if language.lower() in self._root_paths:
            path = self._root_paths[language.lower()]
            # Reinitialize
            self._initialized_languages.discard(language.lower())
            return self.initialize(language, str(path))
        return False

    def get_connection_pool_status(self) -> dict[str, Any]:
        """Get connection pool status.

        Returns:
            Dict with connected languages and status
        """
        return {
            "connected_languages": list(self._initialized_languages),
            "total_connections": len(self._initialized_languages),
            "root_paths": {lang: str(path) for lang, path in self._root_paths.items()},
        }


class TierProxy:
    """Tier proxy for convenient tier access.

    Usage:
        memory.working().add("content")
        memory.working().search("query")
    """

    def __init__(self, memory_system: PythonMemorySystem, tier: str):
        self._memory = memory_system
        self._tier = tier

    def add(self, content: str, metadata: dict[str, Any] | None = None) -> str:
        """Add memory to tier."""
        return self._memory.store(self._tier, content)

    def search(self, query: str, limit: int = 10) -> list[dict[str, Any]]:
        """Search memories in tier."""
        return self._memory.query(query, tier=self._tier, limit=limit)

    def get(self, memory_id: str) -> dict[str, Any] | None:
        """Get memory by ID."""
        return self._memory.get(self._tier, memory_id)

    def remove(self, memory_id: str) -> bool:
        """Remove memory by ID."""
        return self._memory.delete(self._tier, memory_id)

    def clear(self) -> int:
        """Clear all memories in tier."""
        return self._memory.clear(self._tier)

    def count(self) -> int:
        """Get memory count in tier."""
        stats = self._memory.stats()
        return stats.get(self._tier, 0)


class PythonMemorySystem:
    """Pure Python MemorySystem implementation.

    Unified memory system with working/session/project/long-term tiers.
    """

    def __init__(self, session_id: str | None = None):
        """Initialize memory system.

        Args:
            session_id: Session identifier (auto-generated if None)
        """
        self._session_id = session_id or generate_short_id()
        self._memories: dict[str, dict[str, Any]] = {
            "working": {},
            "session": {},
            "project": {},
            "longterm": {},
        }

    def store(self, tier: str, content: str) -> str:
        """Store memory in tier.

        Args:
            tier: Memory tier (working, session, project, longterm)
            content: Memory content

        Returns:
            Memory ID
        """
        tier_key = self._normalize_tier(tier)
        memory_id = generate_short_id()

        self._memories[tier_key][memory_id] = {
            "id": memory_id,
            "content": content,
            "created_at": datetime.now().isoformat(),
            "access_count": 0,
        }

        return memory_id

    def query(
        self,
        query: str,
        tier: str | None = None,
        limit: int = 10,
    ) -> list[dict[str, Any]]:
        """Query memories.

        Args:
            query: Search query
            tier: Optional tier filter
            limit: Maximum results

        Returns:
            List of matching memories
        """
        results = []
        tiers = [self._normalize_tier(tier)] if tier else list(self._memories.keys())

        query_lower = query.lower()

        for t in tiers:
            for _memory_id, memory in self._memories.get(t, {}).items():
                if query_lower in memory["content"].lower():
                    memory["access_count"] = memory.get("access_count", 0) + 1
                    results.append(memory.copy())
                    if len(results) >= limit:
                        return results

        return results

    def get(self, tier: str, memory_id: str) -> dict[str, Any] | None:
        """Get specific memory.

        Args:
            tier: Memory tier
            memory_id: Memory identifier

        Returns:
            Memory dict or None
        """
        tier_key = self._normalize_tier(tier)
        return self._memories.get(tier_key, {}).get(memory_id)

    def stats(self) -> dict[str, int]:
        """Get memory statistics.

        Returns:
            Dict mapping tier names to counts
        """
        return {
            "working": len(self._memories["working"]),
            "session": len(self._memories["session"]),
            "project": len(self._memories["project"]),
            "longterm": len(self._memories["longterm"]),
        }

    def clear(self, tier: str) -> int:
        """Clear all memories in tier.

        Args:
            tier: Memory tier

        Returns:
            Number of memories cleared
        """
        tier_key = self._normalize_tier(tier)
        count = len(self._memories[tier_key])
        self._memories[tier_key] = {}
        return count

    def delete(self, tier: str, memory_id: str) -> bool:
        """Delete a specific memory.

        Args:
            tier: Memory tier
            memory_id: Memory identifier

        Returns:
            True if deleted, False if not found
        """
        tier_key = self._normalize_tier(tier)
        if memory_id in self._memories[tier_key]:
            del self._memories[tier_key][memory_id]
            return True
        return False

    def working(self) -> TierProxy:
        """Get working memory tier proxy."""
        return TierProxy(self, "working")

    def session(self) -> TierProxy:
        """Get session memory tier proxy."""
        return TierProxy(self, "session")

    def project(self) -> TierProxy:
        """Get project memory tier proxy."""
        return TierProxy(self, "project")

    def long_term(self) -> TierProxy:
        """Get long-term memory tier proxy."""
        return TierProxy(self, "longterm")

    def get_project_backend(self) -> dict[str, Any]:
        """Get project memory backend info."""
        return {
            "type": "memory",
            "tier": "project",
            "count": len(self._memories["project"]),
        }

    def get_long_term_backend(self) -> dict[str, Any]:
        """Get long-term memory backend info."""
        return {
            "type": "memory",
            "tier": "longterm",
            "count": len(self._memories["longterm"]),
        }

    def persist(self, path: str | None = None) -> bool:
        """Persist memory to storage.

        Args:
            path: Optional path for persistence

        Returns:
            True if successful
        """
        if path is None:
            path = str(Path.home() / ".continuum" / "memory" / f"{self._session_id}.json")

        storage_path = Path(path)
        storage_path.parent.mkdir(parents=True, exist_ok=True)

        data = {
            "session_id": self._session_id,
            "memories": self._memories,
            "persisted_at": datetime.now().isoformat(),
        }

        with open(storage_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

        return True

    def load(self, path: str) -> bool:
        """Load memory from storage.

        Args:
            path: Path to load from

        Returns:
            True if successful
        """
        storage_path = Path(path)
        if not storage_path.exists():
            return False

        with open(storage_path, encoding="utf-8") as f:
            data = json.load(f)

        self._session_id = data.get("session_id", self._session_id)
        self._memories = data.get("memories", self._memories)

        return True

    def _normalize_tier(self, tier: str) -> str:
        """Normalize tier name."""
        tier_map = {
            "working": "working",
            "session": "session",
            "project": "project",
            "longterm": "longterm",
            "long_term": "longterm",
            "long-term": "longterm",
        }
        key = tier.lower().replace("-", "_")
        if key not in tier_map:
            raise ValueError(f"Invalid tier: {tier}. Must be working, session, project, or longterm")
        return tier_map[key]


class PythonMultimodalHandler:
    """Pure Python multimodal content handler.

    Handles text, images, audio, and document content for LLM interactions.
    """

    SUPPORTED_IMAGE_TYPES = {"image/jpeg", "image/png", "image/gif", "image/webp"}
    SUPPORTED_AUDIO_TYPES = {"audio/mp3", "audio/wav", "audio/ogg", "audio/m4a"}
    SUPPORTED_DOC_TYPES = {"application/pdf", "text/plain", "text/markdown"}

    def __init__(self):
        """Initialize multimodal handler."""
        self._content_cache: dict[str, dict[str, Any]] = {}

    def encode_image(self, image_path: str, media_type: str | None = None) -> dict[str, Any]:
        """Encode image for LLM consumption.

        Args:
            image_path: Path to image file
            media_type: MIME type (auto-detected if None)

        Returns:
            Content dict with type 'image' and base64 data
        """
        import base64

        path = Path(image_path).expanduser().resolve()
        if not path.exists():
            raise FileNotFoundError(f"Image not found: {path}")

        # Auto-detect media type
        if media_type is None:
            ext = path.suffix.lower()
            type_map = {
                ".jpg": "image/jpeg",
                ".jpeg": "image/jpeg",
                ".png": "image/png",
                ".gif": "image/gif",
                ".webp": "image/webp",
            }
            media_type = type_map.get(ext, "image/jpeg")

        if media_type not in self.SUPPORTED_IMAGE_TYPES:
            raise ValueError(f"Unsupported image type: {media_type}")

        with open(path, "rb") as f:
            data = base64.b64encode(f.read()).decode("utf-8")

        content_id = generate_short_id()
        self._content_cache[content_id] = {
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": media_type,
                "data": data,
            },
            "path": str(path),
        }

        return {
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": media_type,
                "data": data,
            },
        }

    def encode_document(self, doc_path: str, media_type: str | None = None) -> dict[str, Any]:
        """Encode document for LLM consumption.

        Args:
            doc_path: Path to document
            media_type: MIME type (auto-detected if None)

        Returns:
            Content dict with document data
        """
        import base64

        path = Path(doc_path).expanduser().resolve()
        if not path.exists():
            raise FileNotFoundError(f"Document not found: {path}")

        # Auto-detect media type
        if media_type is None:
            ext = path.suffix.lower()
            type_map = {
                ".pdf": "application/pdf",
                ".txt": "text/plain",
                ".md": "text/markdown",
                ".markdown": "text/markdown",
            }
            media_type = type_map.get(ext, "application/octet-stream")

        with open(path, "rb") as f:
            content = f.read()

        if media_type == "text/plain" or media_type == "text/markdown":
            # Return as text for text files
            return {
                "type": "text",
                "text": content.decode("utf-8"),
            }
        else:
            # Return as base64 for binary files
            data = base64.b64encode(content).decode("utf-8")
            return {
                "type": "document",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": data,
                },
            }

    def create_message(
        self,
        role: str,
        content: str | list[dict[str, Any]],
    ) -> dict[str, Any]:
        """Create a multimodal message.

        Args:
            role: Message role (user, assistant, system)
            content: Text content or list of content parts

        Returns:
            Message dict
        """
        if isinstance(content, str):
            return {
                "role": role,
                "content": content,
            }

        return {
            "role": role,
            "content": content,
        }

    def create_image_message(
        self,
        role: str,
        text: str,
        image_paths: list[str],
    ) -> dict[str, Any]:
        """Create a message with text and images.

        Args:
            role: Message role
            text: Text content
            image_paths: List of image paths

        Returns:
            Message dict with multimodal content
        """
        content = [{"type": "text", "text": text}]

        for path in image_paths:
            content.append(self.encode_image(path))

        return {
            "role": role,
            "content": content,
        }

    def extract_text(self, message: dict[str, Any]) -> str:
        """Extract text from message.

        Args:
            message: Message dict

        Returns:
            Extracted text content
        """
        content = message.get("content", "")

        if isinstance(content, str):
            return content

        if isinstance(content, list):
            texts = []
            for part in content:
                if part.get("type") == "text":
                    texts.append(part.get("text", ""))
            return "\n".join(texts)

        return ""

    def list_images(self, message: dict[str, Any]) -> list[dict[str, Any]]:
        """Extract images from message.

        Args:
            message: Message dict

        Returns:
            List of image content parts
        """
        content = message.get("content", [])

        if isinstance(content, str):
            return []

        return [part for part in content if part.get("type") == "image"]

    def _is_private_ip(self, ip: str) -> bool:
        """Check if IP address is private/internal.

        Blocks:
        - Private ranges: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
        - Localhost: 127.0.0.0/8, 0.0.0.0/8
        - Link-local: 169.254.0.0/16
        - IPv6 loopback: ::1
        - IPv6 link-local: fe80::/10
        - IPv6 private: fc00::/7 (ULA), ::ffff:0:0/96 (IPv4-mapped)
        """
        import ipaddress

        try:
            addr = ipaddress.ip_address(ip)
        except ValueError:
            return True  # Invalid IP is treated as private for safety

        # IPv4 private ranges
        private_networks_v4 = [
            ipaddress.ip_network('10.0.0.0/8'),
            ipaddress.ip_network('172.16.0.0/12'),
            ipaddress.ip_network('192.168.0.0/16'),
            ipaddress.ip_network('127.0.0.0/8'),
            ipaddress.ip_network('0.0.0.0/8'),
            ipaddress.ip_network('169.254.0.0/16'),
        ]

        # IPv6 private ranges
        private_networks_v6 = [
            ipaddress.ip_network('::1/128'),           # Loopback
            ipaddress.ip_network('fe80::/10'),         # Link-local
            ipaddress.ip_network('fc00::/7'),          # ULA
            ipaddress.ip_network('::ffff:0:0/96'),     # IPv4-mapped
        ]

        if addr.version == 4:
            return any(addr in net for net in private_networks_v4)
        else:
            return any(addr in net for net in private_networks_v6)

    def _validate_url_for_ssrf(self, url: str) -> str:
        """Validate URL for SSRF protection.

        Args:
            url: URL to validate

        Returns:
            The validated hostname

        Raises:
            ValueError: If URL is invalid or points to private IP
        """
        import socket
        from urllib.parse import urlparse

        # Parse URL
        parsed = urlparse(url)

        # Only allow http/https schemes
        if parsed.scheme not in ('http', 'https'):
            raise ValueError(f"Invalid URL scheme: {parsed.scheme}. Only http and https are allowed.")

        hostname = parsed.hostname
        if not hostname:
            raise ValueError("Invalid URL: missing hostname")

        # Block localhost and similar hostnames
        blocked_hostnames = {'localhost', 'localhost.localdomain', 'local'}
        if hostname.lower() in blocked_hostnames:
            raise ValueError(f"Blocked hostname: {hostname}")

        # DNS rebinding protection: resolve hostname and check resolved IP
        try:
            # Get all IP addresses for the hostname
            addr_info = socket.getaddrinfo(hostname, parsed.port or (443 if parsed.scheme == 'https' else 80))
            for _family, _socktype, _proto, _canonname, sockaddr in addr_info:
                ip = sockaddr[0]
                if self._is_private_ip(ip):
                    raise ValueError(f"URL resolves to private/internal IP: {hostname} -> {ip}")
        except socket.gaierror as e:
            raise ValueError(f"Failed to resolve hostname {hostname}: {e}")

        return hostname

    def _follow_redirects_safely(self, url: str, timeout: int, max_redirects: int = 5) -> tuple[bytes, str]:
        """Follow redirects safely, validating each redirect target.

        Args:
            url: Initial URL
            timeout: Request timeout
            max_redirects: Maximum redirects to follow

        Returns:
            Tuple of (data, content_type)

        Raises:
            ValueError: If redirect target is invalid or points to private IP
        """
        import urllib.error
        import urllib.request

        current_url = url
        redirect_count = 0

        while redirect_count <= max_redirects:
            # Validate current URL before each request
            self._validate_url_for_ssrf(current_url)

            request = urllib.request.Request(
                current_url,
                headers={"User-Agent": "Mozilla/5.0 (compatible; ContinuumSDK/1.0)"},
            )

            try:
                with urllib.request.urlopen(request, timeout=timeout) as response:
                    # Check for redirect
                    if response.geturl() != current_url:
                        redirect_count += 1
                        if redirect_count > max_redirects:
                            raise ValueError(f"Too many redirects (max {max_redirects})")
                        current_url = response.geturl()
                        continue

                    # No more redirects, return data
                    data = response.read()
                    content_type = response.headers.get("Content-Type", "image/jpeg")
                    media_type = content_type.split(";")[0].strip()
                    return (data, media_type)

            except urllib.error.HTTPError as e:
                if e.code in (301, 302, 303, 307, 308) and 'Location' in e.headers:
                    redirect_count += 1
                    if redirect_count > max_redirects:
                        raise ValueError(f"Too many redirects (max {max_redirects})")
                    current_url = e.headers['Location']
                    continue
                raise ValueError(f"HTTP error {e.code}: {e.reason}")
            except urllib.error.URLError as e:
                raise ValueError(f"Failed to fetch URL: {e}")

        raise ValueError(f"Too many redirects (max {max_redirects})")  # pragma: no cover - hard to trigger in tests

    def encode_image_from_url(self, url: str, timeout: int = 30) -> dict[str, Any]:
        """Fetch and encode image from URL with SSRF protection.

        Args:
            url: Image URL
            timeout: Request timeout in seconds

        Returns:
            Content dict with type 'image' and base64 data

        Raises:
            ValueError: If URL is invalid, blocked, or fetch fails
        """
        import base64

        # Validate URL and get hostname
        self._validate_url_for_ssrf(url)

        try:
            # Follow redirects safely with SSRF protection
            data, media_type = self._follow_redirects_safely(url, timeout)

            if media_type not in self.SUPPORTED_IMAGE_TYPES:
                # Try to guess from URL extension
                ext = url.lower().split("?")[0].rsplit(".", 1)[-1]
                type_map = {
                    "jpg": "image/jpeg",
                    "jpeg": "image/jpeg",
                    "png": "image/png",
                    "gif": "image/gif",
                    "webp": "image/webp",
                }
                media_type = type_map.get(ext, "image/jpeg")

            encoded_data = base64.b64encode(data).decode("utf-8")

            return {
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": encoded_data,
                },
                "url": url,
            }
        except ValueError:
            raise  # Re-raise SSRF validation errors
        except Exception as e:
            raise ValueError(f"Failed to fetch image from URL: {e}")

    def encode_image_url_direct(self, url: str) -> dict[str, Any]:
        """Encode image URL directly (without fetching).

        Some LLM APIs (like GPT-4 Vision) support direct URL references.

        Args:
            url: Image URL

        Returns:
            Content dict with type 'image_url'
        """
        return {
            "type": "image_url",
            "image_url": {"url": url},
        }

    def to_openai_format(self, content: dict[str, Any]) -> dict[str, Any]:
        """Convert content to OpenAI API format.

        Args:
            content: Content dict from encode_image/encode_document

        Returns:
            OpenAI-compatible content dict
        """
        if content.get("type") == "image":
            source = content.get("source", {})
            if source.get("type") == "base64":
                return {
                    "type": "image_url",
                    "image_url": {
                        "url": f"data:{source.get('media_type', 'image/jpeg')};base64,{source.get('data', '')}"
                    },
                }
        elif content.get("type") == "image_url":
            return content
        elif content.get("type") == "text":
            return content

        return content

    def create_openai_vision_message(
        self,
        role: str,
        text: str,
        images: list[str | dict[str, Any]],
        detail: str = "auto",
    ) -> dict[str, Any]:
        """Create OpenAI Vision API compatible message.

        Args:
            role: Message role
            text: Text content
            images: List of image paths, URLs, or encoded content
            detail: Image detail level ("low", "high", "auto")

        Returns:
            OpenAI-compatible message dict
        """
        content = [{"type": "text", "text": text}]

        for img in images:
            if isinstance(img, str):
                if img.startswith(("http://", "https://")):
                    # URL - use direct reference for OpenAI
                    content.append({
                        "type": "image_url",
                        "image_url": {"url": img, "detail": detail},
                    })
                else:
                    # File path - encode as base64
                    encoded = self.encode_image(img)
                    openai_img = self.to_openai_format(encoded)
                    openai_img["image_url"]["detail"] = detail
                    content.append(openai_img)
            else:
                # Already encoded content
                openai_img = self.to_openai_format(img)
                if "image_url" in openai_img:
                    openai_img["image_url"]["detail"] = detail
                content.append(openai_img)

        return {"role": role, "content": content}

    def create_anthropic_vision_message(
        self,
        role: str,
        text: str,
        images: list[str | dict[str, Any]],
    ) -> dict[str, Any]:
        """Create Anthropic Vision API compatible message.

        Args:
            role: Message role
            text: Text content
            images: List of image paths, URLs, or encoded content

        Returns:
            Anthropic-compatible message dict
        """
        content = [{"type": "text", "text": text}]

        for img in images:
            if isinstance(img, str):
                if img.startswith(("http://", "https://")):
                    # Anthropic doesn't support direct URLs - must fetch
                    encoded = self.encode_image_from_url(img)
                    content.append(encoded)
                else:
                    # File path
                    content.append(self.encode_image(img))
            elif isinstance(img, dict):  # pragma: no cover - dict input is a fallback
                content.append(img)

        return {"role": role, "content": content}


class ImageInput:
    """Image input type supporting multiple formats.

    Supports:
    - Local file paths
    - URLs (http/https)
    - Base64-encoded data
    - PIL Image objects (lazy loaded)
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
        """Initialize image input.

        Args:
            source: Universal source (path, URL, or base64 data)
            path: Explicit file path
            url: Explicit URL
            base64_data: Explicit base64 data
            media_type: MIME type (auto-detected if None)
        """
        self._path: str | None = None
        self._url: str | None = None
        self._base64_data: str | None = None
        self._media_type: str | None = media_type
        self._bytes_data: bytes | None = None
        self._lazy_loaded = False

        # Parse source
        if source is not None:
            if isinstance(source, bytes):
                self._bytes_data = source
            elif source.startswith(("http://", "https://")):
                self._url = source
            elif source.startswith("data:"):
                # Data URL
                import re
                match = re.match(r"data:([^;]+);base64,(.+)", source)
                if match:
                    self._media_type = match.group(1)
                    self._base64_data = match.group(2)
            else:
                # Assume file path
                self._path = source

        # Apply explicit parameters
        if path is not None:
            self._path = path
        if url is not None:
            self._url = url
        if base64_data is not None:
            self._base64_data = base64_data

    @classmethod
    def from_path(cls, path: str, media_type: str | None = None) -> ImageInput:
        """Create from file path."""
        return cls(path=path, media_type=media_type)

    @classmethod
    def from_url(cls, url: str) -> ImageInput:
        """Create from URL."""
        return cls(url=url)

    @classmethod
    def from_base64(
        cls, data: str, media_type: str = "image/jpeg"
    ) -> ImageInput:
        """Create from base64 data."""
        return cls(base64_data=data, media_type=media_type)

    @classmethod
    def from_bytes(cls, data: bytes, media_type: str = "image/jpeg") -> ImageInput:
        """Create from raw bytes."""
        instance = cls(media_type=media_type)
        instance._bytes_data = data
        return instance

    def to_base64(self) -> str:
        """Get base64-encoded data.

        Lazy loads from path/URL on first access.

        Returns:
            Base64-encoded string
        """
        import base64

        if self._base64_data is not None:
            return self._base64_data

        if self._bytes_data is not None:
            self._base64_data = base64.b64encode(self._bytes_data).decode("utf-8")
            return self._base64_data

        if self._path is not None:
            path = Path(self._path).expanduser().resolve()
            with open(path, "rb") as f:
                self._bytes_data = f.read()
            # Auto-detect media type
            if self._media_type is None:
                ext = path.suffix.lower()
                type_map = {
                    ".jpg": "image/jpeg",
                    ".jpeg": "image/jpeg",
                    ".png": "image/png",
                    ".gif": "image/gif",
                    ".webp": "image/webp",
                }
                self._media_type = type_map.get(ext, "image/jpeg")
            self._base64_data = base64.b64encode(self._bytes_data).decode("utf-8")  # pragma: no cover - branch when media_type is already set
            self._lazy_loaded = True
            return self._base64_data

        if self._url is not None:
            # Fetch from URL
            import urllib.request
            request = urllib.request.Request(
                self._url,
                headers={"User-Agent": "Mozilla/5.0 (compatible; ContinuumSDK/1.0)"},
            )
            with urllib.request.urlopen(request, timeout=30) as response:
                self._bytes_data = response.read()
                if self._media_type is None:
                    content_type = response.headers.get("Content-Type", "image/jpeg")
                    self._media_type = content_type.split(";")[0].strip()
            self._base64_data = base64.b64encode(self._bytes_data).decode("utf-8")  # pragma: no cover - branch when media_type is already set
            self._lazy_loaded = True
            return self._base64_data

        raise ValueError("No image source available")

    @property
    def media_type(self) -> str:
        """Get MIME type."""
        if self._media_type is not None:
            return self._media_type

        # Auto-detect from path
        if self._path is not None:
            ext = Path(self._path).suffix.lower()
            type_map = {
                ".jpg": "image/jpeg",
                ".jpeg": "image/jpeg",
                ".png": "image/png",
                ".gif": "image/gif",
                ".webp": "image/webp",
            }
            return type_map.get(ext, "image/jpeg")

        return "image/jpeg"

    def to_anthropic_format(self) -> dict[str, Any]:
        """Convert to Anthropic API format."""
        return {
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": self.media_type,
                "data": self.to_base64(),
            },
        }

    def to_openai_format(self, detail: str = "auto") -> dict[str, Any]:
        """Convert to OpenAI API format."""
        return {
            "type": "image_url",
            "image_url": {
                "url": f"data:{self.media_type};base64,{self.to_base64()}",
                "detail": detail,
            },
        }

    @property
    def source_type(self) -> str:
        """Get source type (path, url, base64, bytes)."""
        if self._path is not None:
            return "path"
        if self._url is not None:
            return "url"
        if self._base64_data is not None:
            return "base64"
        if self._bytes_data is not None:
            return "bytes"
        return "unknown"