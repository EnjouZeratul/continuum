"""
Session Module

Session management for Continuum SDK.

A Session represents a single conversation thread between a user and an Agent.
It maintains the message history, metadata, and tracks usage statistics.

Key Features:
    - Message history with roles (user, assistant, system, tool)
    - Metadata storage for custom session data
    - Tool usage tracking
    - Cost and token tracking
    - Export/import for persistence
    - File-based persistence (save/load)

Quick Start:
    >>> from continuum_sdk.agent.session import Session
    >>>
    >>> session = Session(id="my-session")
    >>> session.add_user_message("Hello")
    >>> session.add_assistant_message("Hi there!")
    >>> print(session.message_count)  # 2

Message Types:
    >>> # User message
    >>> session.add_user_message("What is Python?")
    >>>
    >>> # Assistant message
    >>> session.add_assistant_message("Python is a programming language.")
    >>>
    >>> # System message (instructions)
    >>> session.add_system_message("You are a helpful assistant.")
    >>>
    >>> # Tool message (function result)
    >>> session.add_tool_message("search", '{"result": "found"}')

Message History:
    >>> for msg in session.get_messages():
    ...     print(f"[{msg.role.value}]: {msg.content[:50]}...")
    >>>
    >>> # Get last N messages
    >>> recent = session.get_messages(limit=10)

Cost Tracking:
    >>> session.update_cost(cost=0.05, tokens=1000)
    >>> print(f"Total cost: ${session.total_cost:.4f}")
    >>> print(f"Total tokens: {session.total_tokens}")

Tool Usage:
    >>> session.record_tool_use("search")
    >>> session.record_tool_use("read_file")
    >>> print(session.tool_calls)  # {"search": 1, "read_file": 1}

Persistence:
    >>> # Save session
    >>> session.save("~/.continuum/sessions/my-session.json")
    >>>
    >>> # Load session
    >>> loaded = Session.load("~/.continuum/sessions/my-session.json")
    >>>
    >>> # Export as dict
    >>> data = session.to_dict()
    >>>
    >>> # Import from dict
    >>> session2 = Session.from_dict(data)

Metadata:
    >>> session.set_metadata("user_id", "12345")
    >>> session.set_metadata("preferences", {"language": "en"})
    >>> print(session.get_metadata("user_id"))  # "12345"

Integration with Agent:
    >>> agent = Agent()
    >>> session = agent.create_session("conversation-1")
    >>> agent.set_session(session)
    >>>
    >>> result = agent.execute("Hello")
    >>> # Messages automatically recorded in session

See Also:
    Agent: Uses Session for conversation management
    Message: Message container class
    MessageRole: Role enumeration
"""

import json
import logging
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

# Import Rust bindings
try:
    from sh_core import Session as RustSession

    HAS_RUST_BINDINGS = True
except ImportError:
    HAS_RUST_BINDINGS = False


class MessageRole(Enum):
    """Message role enumeration."""

    USER = "user"
    ASSISTANT = "assistant"
    SYSTEM = "system"
    TOOL = "tool"


class Message:
    """Session message container."""

    def __init__(
        self,
        role: MessageRole,
        content: str,
        timestamp: datetime | None = None,
        metadata: dict[str, Any] | None = None,
    ):
        self.role = role
        self.content = content
        self.timestamp = timestamp or datetime.now()
        self.metadata = metadata or {}

    def to_dict(self) -> dict[str, Any]:
        return {
            "role": self.role.value,
            "content": self.content,
            "timestamp": self.timestamp.isoformat(),
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Message":
        return cls(
            role=MessageRole(data["role"]),
            content=data["content"],
            timestamp=datetime.fromisoformat(data["timestamp"]),
            metadata=data.get("metadata", {}),
        )


class Session:
    """
    Continuum Session class.

    Manages a single conversation thread's state and history.

    A Session maintains:
        - Message history with timestamps
        - Metadata (key-value store)
        - Tool usage tracking
        - Cost and token statistics

    Sessions can be exported to JSON and restored later, enabling
    conversation persistence and resumption.

    Attributes:
        id: Unique session identifier
        created_at: Session creation timestamp
        message_count: Number of messages in history
        cost: Total accumulated cost
        tokens: Total token count

    Example:
        >>> session = Session(id="chat-001")
        >>> session.add_user_message("What is Python?")
        >>> session.add_assistant_message("Python is a programming language.")
        >>> print(session.get_last_message().content)
        'Python is a programming language.'
    """

    def __init__(self, id: str | None = None):
        """
        Create a new Session.

        Args:
            id: Optional session identifier. Auto-generated if not provided.
        """
        self._id = id or "default-session"
        self._messages: list[Message] = []
        self._created_at = datetime.now()
        self._metadata: dict[str, Any] = {}
        self._tools_used: list[str] = []
        self._cost: float = 0.0
        self._token_count: int = 0

        # Rust bindings
        if HAS_RUST_BINDINGS:
            self._rust_session = RustSession(self._id)
        else:
            self._rust_session = None

    @property
    def id(self) -> str:
        """Session ID."""
        if self._rust_session:
            # Rust binding uses #[getter], so id is a property not a method
            return self._rust_session.id
        return self._id

    @property
    def created_at(self) -> datetime:
        """Creation timestamp."""
        if self._rust_session:
            # Rust binding uses #[getter], so created_at is a property not a method
            return datetime.fromisoformat(self._rust_session.created_at)
        return self._created_at

    @property
    def message_count(self) -> int:
        """Message count."""
        if self._rust_session:
            # message_count is a method in Rust binding
            return self._rust_session.message_count()
        return len(self._messages)

    @property
    def cost(self) -> float:
        """Total cost."""
        return self._cost

    @property
    def tokens(self) -> int:
        """Token count."""
        return self._token_count

    def add_message(
        self,
        role: MessageRole,
        content: str,
        metadata: dict[str, Any] | None = None,
    ) -> Message:
        """
        Add message.

        Args:
            role: Message role
            content: Message content
            metadata: Metadata

        Returns:
            The added message
        """
        message = Message(role=role, content=content, metadata=metadata)
        self._messages.append(message)

        # Sync to Rust
        if self._rust_session:
            if role == MessageRole.USER:
                self._rust_session.add_user_message(content)
            elif role == MessageRole.ASSISTANT:  # pragma: no branch
                self._rust_session.add_assistant_message(content)

        return message

    def add_user_message(self, content: str) -> Message:
        """Add user message."""
        return self.add_message(MessageRole.USER, content)

    def add_assistant_message(self, content: str) -> Message:
        """Add assistant message."""
        return self.add_message(MessageRole.ASSISTANT, content)

    def add_system_message(self, content: str) -> Message:
        """Add system message."""
        return self.add_message(MessageRole.SYSTEM, content)

    def get_messages(self, limit: int | None = None) -> list[Message]:
        """Get messages."""
        if self._rust_session:
            rust_messages = self._rust_session.get_messages()
            messages = [
                Message(
                    role=MessageRole(r[0]),
                    content=r[1],
                )
                for r in rust_messages
            ]
        else:
            messages = self._messages.copy()

        if limit is None:
            return messages
        return messages[-limit:]

    def get_last_message(self) -> Message | None:
        """Get last message."""
        if not self._messages:
            return None
        return self._messages[-1]

    def clear_messages(self) -> None:
        """Clear message history."""
        self._messages.clear()
        if self._rust_session:
            self._rust_session.clear_messages()

    def set_metadata(self, key: str, value: Any) -> None:
        """Set metadata."""
        self._metadata[key] = value

    def get_metadata(self, key: str) -> Any | None:
        """Get metadata."""
        return self._metadata.get(key)

    def record_tool_use(self, tool_name: str) -> None:
        """Record tool usage."""
        self._tools_used.append(tool_name)

    def get_tools_used(self) -> list[str]:
        """Get list of tools used."""
        return self._tools_used.copy()

    def update_cost(self, cost: float, tokens: int) -> None:
        """Update cost."""
        self._cost += cost
        self._token_count += tokens

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "created_at": self.created_at.isoformat(),
            "messages": [m.to_dict() for m in self.get_messages()],
            "metadata": self._metadata,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Session":
        session = cls(id=data["id"])
        session._created_at = datetime.fromisoformat(data["created_at"])
        session._messages = [Message.from_dict(m) for m in data.get("messages", [])]
        session._metadata = data.get("metadata", {})
        return session

    def export(self) -> str:
        """Export session as JSON."""
        if self._rust_session:
            return self._rust_session.export()

        data = {
            "id": self._id,
            "created_at": self._created_at.isoformat(),
            "messages": [m.to_dict() for m in self._messages],
            "metadata": self._metadata,
            "tools_used": self._tools_used,
            "cost": self._cost,
            "tokens": self._token_count,
        }
        return json.dumps(data, indent=2)

    @classmethod
    def from_export(cls, export_data: str) -> "Session":
        """Restore session from exported data."""
        data = json.loads(export_data)
        session = cls(id=data["id"])
        session._created_at = datetime.fromisoformat(data["created_at"])
        session._messages = [Message.from_dict(m) for m in data["messages"]]
        session._metadata = data.get("metadata", {})
        session._tools_used = data.get("tools_used", [])
        session._cost = data.get("cost", 0.0)
        session._token_count = data.get("tokens", 0)
        return session

    def __repr__(self) -> str:
        return f"Session(id={self._id}, messages={len(self._messages)})"

    def save(self, path: str | Path) -> Path:
        """
        Save session to file.

        Args:
            path: File path to save to (JSON format)
        """
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)

        data = {
            "id": self._id,
            "created_at": self._created_at.isoformat(),
            "messages": [m.to_dict() for m in self._messages],
            "metadata": self._metadata,
            "tools_used": self._tools_used,
            "cost": self._cost,
            "tokens": self._token_count,
            "version": "1.0",
        }

        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

        return path

    @classmethod
    def load(cls, path: str | Path) -> "Session":
        """
        Load session from file.

        Args:
            path: File path to load from

        Returns:
            Restored Session instance

        Raises:
            FileNotFoundError: If file doesn't exist
            ValueError: If file format is invalid
        """
        path = Path(path)
        if not path.exists():
            raise FileNotFoundError(f"Session file not found: {path}")

        with open(path, encoding="utf-8") as f:
            data = json.load(f)

        session = cls(id=data["id"])
        session._created_at = datetime.fromisoformat(data["created_at"])
        session._messages = [Message.from_dict(m) for m in data.get("messages", [])]
        session._metadata = data.get("metadata", {})
        session._tools_used = data.get("tools_used", [])
        session._cost = data.get("cost", 0.0)
        session._token_count = data.get("tokens", 0)

        return session

    def delete(self, path: str | Path) -> None:
        """
        Delete session file.

        Args:
            path: File path to delete
        """
        path = Path(path)
        if path.exists():
            path.unlink()

    @staticmethod
    def get_default_session_dir() -> Path:
        """
        Get default session storage directory.

        Returns:
            Path to ~/.continuum/sessions/
        """
        home = Path.home()
        return home / ".continuum" / "sessions"

    def save_to_default(self) -> Path:
        """
        Save session to default directory.

        Returns:
            Path where session was saved
        """
        session_dir = self.get_default_session_dir()
        path = session_dir / f"{self._id}.json"
        self.save(path)
        return path

    @classmethod
    def load_from_default(cls, session_id: str) -> "Session":
        """
        Load session from default directory.

        Args:
            session_id: Session ID to load

        Returns:
            Restored Session instance
        """
        session_dir = cls.get_default_session_dir()
        path = session_dir / f"{session_id}.json"
        return cls.load(path)

    @classmethod
    def list_saved_sessions(cls) -> list[str]:
        """
        List all saved session IDs in default directory.

        Returns:
            List of session IDs
        """
        session_dir = cls.get_default_session_dir()
        if not session_dir.exists():
            return []

        return [f.stem for f in session_dir.glob("*.json")]  # pragma: no cover

    @classmethod
    def recover(cls, checkpoint_path: str | Path) -> "Session":
        """
        Recover a session from a checkpoint file.

        从检查点文件恢复会话状态。

        Args:
            checkpoint_path: Path to the checkpoint file (JSON format).
                检查点文件路径（JSON格式）。

        Returns:
            Session: Restored Session instance.
                恢复的 Session 实例。

        Raises:
            FileNotFoundError: If the checkpoint file does not exist.
                检查点文件不存在时抛出。
            ValueError: If the checkpoint file format is invalid.
                检查点文件格式无效时抛出。

        Example:
            >>> session = Session.recover("~/.continuum/checkpoints/session-001.json")
            >>> print(f"Recovered session: {session.id}")
        """
        checkpoint_path = Path(checkpoint_path)

        logger.info("Recovering session from checkpoint: %s", checkpoint_path)

        if not checkpoint_path.exists():
            raise FileNotFoundError(
                f"Checkpoint file not found: {checkpoint_path}. "
                "Please ensure the checkpoint path is correct and the file exists."
            )

        try:
            with open(checkpoint_path, encoding="utf-8") as f:
                data = json.load(f)
        except json.JSONDecodeError as e:
            raise ValueError(
                f"Invalid checkpoint file format: {checkpoint_path}. "
                f"The file is not valid JSON. Error: {e}"
            ) from e

        # Validate required fields
        required_fields = ["id", "created_at"]
        missing_fields = [field for field in required_fields if field not in data]
        if missing_fields:
            raise ValueError(
                f"Invalid checkpoint file: missing required fields {missing_fields}. "
                "Please ensure the checkpoint file contains a valid session export."
            )

        logger.debug(
            "Checkpoint data loaded: id=%s, messages=%d",
            data.get("id"),
            len(data.get("messages", [])),
        )

        session = cls(id=data["id"])
        session._created_at = datetime.fromisoformat(data["created_at"])
        session._messages = [
            Message.from_dict(m) for m in data.get("messages", [])
        ]
        session._metadata = data.get("metadata", {})
        session._tools_used = data.get("tools_used", [])
        session._cost = data.get("cost", 0.0)
        session._token_count = data.get("tokens", 0)

        logger.info(
            "Session recovered successfully: id=%s, messages=%d, cost=%.4f",
            session.id,
            session.message_count,
            session.cost,
        )

        return session


def create_session(id: str | None = None) -> Session:
    """
    Convenience function to create a Session.

    Args:
        id: Session ID

    Returns:
        Session instance
    """
    return Session(id=id)
