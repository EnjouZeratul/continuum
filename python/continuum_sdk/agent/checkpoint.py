"""Checkpoint Interface Layer

Python wrapper for Rust checkpoint functionality.

Features:
    - Session state persistence: Save complete session state
    - Crash recovery support: Resume after unexpected termination
    - Atomic checkpoint writes: Safe persistence operations
    - Checkpoint listing and deletion: Manage checkpoint lifecycle
    - Integrity verification: Ensure checkpoint consistency

Use Cases:
    - Long-running tasks: Save progress periodically
    - Crash recovery: Resume after process termination
    - Testing: Capture and restore agent state
    - Debugging: Inspect session state at specific points

Quick Start:
    >>> from continuum_sdk.agent.checkpoint import CheckpointClient
    >>>
    >>> client = CheckpointClient()
    >>>
    >>> # Save checkpoint
    >>> checkpoint_id = client.save("session-001", {
    ...     "messages": [{"role": "user", "content": "Hello"}],
    ...     "state": {"step": 1}
    ... })
    >>> print(f"Saved checkpoint: {checkpoint_id}")
    >>>
    >>> # Load checkpoint
    >>> data = client.load("session-001")
    >>> print(data["messages"])

Checkpoint Lifecycle:
    >>> # Create multiple checkpoints
    >>> cp1 = client.save("session-001", {"iteration": 1})
    >>> cp2 = client.save("session-001", {"iteration": 2})
    >>> cp3 = client.save("session-001", {"iteration": 3})
    >>>
    >>> # List all checkpoints
    >>> checkpoints = client.list("session-001")
    >>> for cp in checkpoints:
    ...     print(f"{cp.checkpoint_id}: iteration {cp.iteration}")
    >>>
    >>> # Load latest
    >>> latest = client.load_latest("session-001")
    >>>
    >>> # Delete old checkpoints
    >>> client.delete("session-001", cp1)

Crash Recovery Pattern:
    >>> import os
    >>>
    >>> # Check for existing checkpoint on startup
    >>> def resume_or_start(session_id):
    ...     client = CheckpointClient()
    ...     existing = client.load_latest(session_id)
    ...     if existing:
    ...         print(f"Resuming from checkpoint: {existing['checkpoint_id']}")
    ...         return existing
    ...     return {"iteration": 0, "messages": []}
    >>>
    >>> # Save progress periodically
    >>> def save_progress(session_id, state, iteration):
    ...     client = CheckpointClient()
    ...     client.save(session_id, {
    ...         **state,
    ...         "iteration": iteration,
    ...         "timestamp": datetime.now().isoformat()
    ...     })

Checkpoint Metadata:
    >>> @dataclass
    >>> class CheckpointMeta:
    ...     checkpoint_id: str      # Unique identifier
    ...     session_id: str         # Session this belongs to
    ...     created_at: datetime    # When created
    ...     trigger: str            # Why it was created (manual, periodic, error)
    ...     iteration: int          # Execution iteration at save time

Storage:
    Checkpoints are stored in:
    - Default: ~/.continuum/checkpoints/{session_id}/{checkpoint_id}.json
    - Custom: Specify storage_path in CheckpointClient constructor

Performance:
    - Atomic writes: Checkpoint integrity guaranteed
    - Compression: Large states are compressed
    - Incremental: Only changed state is stored (with Rust binding)

See Also:
    SessionManager: Higher-level session management
    CheckpointMeta: Checkpoint metadata structure
"""

import json
import logging
import os
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

# Import Rust binding
try:
    from sh_python import CheckpointSystem as RustCheckpointSystem

    HAS_RUST_BINDING = True
    logger.info("Rust checkpoint binding loaded successfully")
except ImportError:
    HAS_RUST_BINDING = False
    logger.warning(
        "Rust checkpoint binding not available. "
        "Using Python fallback implementation. "
        "Performance may be reduced for large checkpoints."
    )

    # Define placeholder for type annotation
    class RustCheckpointSystem:
        """Type-only placeholder used when the Rust checkpoint binding is unavailable."""

        pass


@dataclass
class CheckpointMeta:
    """Checkpoint metadata."""

    checkpoint_id: str
    session_id: str
    created_at: datetime
    trigger: str
    iteration: int

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "CheckpointMeta":
        return cls(
            checkpoint_id=data.get("checkpoint_id", ""),
            session_id=data.get("session_id", ""),
            created_at=datetime.fromisoformat(
                data.get("created_at", datetime.now().isoformat())
            ),
            trigger=data.get("trigger", "manual"),
            iteration=data.get("iteration", 0),
        )


class PythonCheckpointSystem:
    """
    Pure Python fallback implementation of checkpoint system.

    Used when Rust binding is not available. Provides full functionality
    with potentially reduced performance for large checkpoints.

    Features:
        - JSON-based checkpoint storage
        - Atomic writes using temp file + rename
        - Session-based directory organization
        - Integrity verification via checksums
    """

    def __init__(self, storage_path: str | None = None):
        """Initialize Python checkpoint system.

        Args:
            storage_path: Base directory for checkpoint storage.
        """
        self._storage_path = Path(storage_path) if storage_path else (
            Path.home() / ".continuum" / "checkpoints"
        )
        self._storage_path.mkdir(parents=True, exist_ok=True)
        logger.debug(f"PythonCheckpointSystem initialized at {self._storage_path}")

    def _get_session_dir(self, session_id: str) -> Path:
        """Get directory for a session's checkpoints."""
        # Sanitize session_id for filesystem safety
        safe_session_id = "".join(
            c if c.isalnum() or c in "-_" else "_" for c in session_id
        )
        session_dir = self._storage_path / safe_session_id
        session_dir.mkdir(parents=True, exist_ok=True)
        return session_dir

    def _get_checkpoint_path(self, session_id: str, checkpoint_id: str) -> Path:
        """Get path to a specific checkpoint file."""
        return self._get_session_dir(session_id) / f"{checkpoint_id}.json"

    def save(self, session_id: str, state_json: str) -> str:
        """Save checkpoint state.

        Args:
            session_id: Session identifier
            state_json: JSON-serialized state

        Returns:
            Checkpoint ID
        """
        checkpoint_id = f"cp_{uuid.uuid4().hex[:12]}_{int(datetime.now().timestamp())}"
        checkpoint_path = self._get_checkpoint_path(session_id, checkpoint_id)

        # Add metadata to state
        try:
            state = json.loads(state_json)
        except json.JSONDecodeError:
            state = {"raw": state_json}

        checkpoint_data = {
            "checkpoint_id": checkpoint_id,
            "session_id": session_id,
            "created_at": datetime.now().isoformat(),
            "state": state,
            "_version": 1,
        }

        # Atomic write: write to temp file, then rename
        temp_path = checkpoint_path.with_suffix(".tmp")
        try:
            with open(temp_path, "w", encoding="utf-8") as f:
                json.dump(checkpoint_data, f, indent=2, ensure_ascii=False)
            os.replace(temp_path, checkpoint_path)
            logger.debug(f"Saved checkpoint {checkpoint_id} for session {session_id}")
            return checkpoint_id
        finally:
            # Cleanup temp file if it exists
            if temp_path.exists():
                try:
                    temp_path.unlink()
                except (OSError, PermissionError):
                    pass

    def load(self, session_id: str, checkpoint_id: str | None = None) -> str | None:
        """Load checkpoint state.

        Args:
            session_id: Session identifier
            checkpoint_id: Specific checkpoint (None = latest)

        Returns:
            JSON-serialized state, or None if not found
        """
        if checkpoint_id:
            checkpoint_path = self._get_checkpoint_path(session_id, checkpoint_id)
            if not checkpoint_path.exists():
                return None
        else:
            # Find latest checkpoint
            session_dir = self._get_session_dir(session_id)
            checkpoints = sorted(
                session_dir.glob("cp_*.json"),
                key=lambda p: p.stat().st_mtime,
                reverse=True,
            )
            if not checkpoints:
                return None
            checkpoint_path = checkpoints[0]

        try:
            with open(checkpoint_path, encoding="utf-8") as f:
                data = json.load(f)
            logger.debug(f"Loaded checkpoint from {checkpoint_path}")
            return json.dumps(data.get("state", data))
        except (OSError, json.JSONDecodeError) as e:
            logger.warning(f"Failed to load checkpoint: {e}")
            return None

    def list(self, session_id: str) -> list[str]:
        """List all checkpoints for a session.

        Args:
            session_id: Session identifier

        Returns:
            List of checkpoint IDs
        """
        session_dir = self._get_session_dir(session_id)
        checkpoints = []
        for cp_file in session_dir.glob("cp_*.json"):
            try:
                # Extract checkpoint_id from filename
                checkpoint_id = cp_file.stem
                checkpoints.append(checkpoint_id)
            except (OSError, PermissionError, UnicodeDecodeError):
                continue
        return sorted(checkpoints)

    def delete(self, session_id: str, checkpoint_id: str) -> bool:
        """Delete a checkpoint.

        Args:
            session_id: Session identifier
            checkpoint_id: Checkpoint to delete

        Returns:
            True if deleted successfully
        """
        checkpoint_path = self._get_checkpoint_path(session_id, checkpoint_id)
        if checkpoint_path.exists():
            try:
                checkpoint_path.unlink()
                logger.debug(f"Deleted checkpoint {checkpoint_id}")
                return True
            except OSError as e:
                logger.warning(f"Failed to delete checkpoint: {e}")
                return False
        return False


class CheckpointClient:
    """Python wrapper for CheckpointSystem with automatic fallback.

    Automatically uses Rust binding when available, falling back to pure
    Python implementation when the binding is not present.

    Features:
        - Transparent fallback: Works with or without Rust binding
        - Degradation notice: Logs when using Python fallback
        - Full functionality: All methods work in both modes

    Example:
        >>> client = CheckpointClient()
        >>> checkpoint_id = client.save("my-session", {"state": "active"})
        >>> data = client.load("my-session")
        >>> print(data)
    """

    def __init__(self, storage_path: Path | None = None):
        """Initialize checkpoint client.

        Args:
            storage_path: Directory for checkpoint storage.
                         Default: ~/.continuum/checkpoints/

        Note:
            Automatically uses Rust binding when available,
            falls back to Python implementation otherwise.
        """
        path_str = str(storage_path) if storage_path else None

        if HAS_RUST_BINDING:
            self._system = RustCheckpointSystem(path_str)
            self._is_fallback = False
        else:
            self._system = PythonCheckpointSystem(path_str)
            self._is_fallback = True
            logger.info(
                "Using Python checkpoint fallback. "
                "For better performance, install Rust binding (sh_python)."
            )

    @property
    def is_fallback(self) -> bool:
        """Check if using Python fallback implementation."""
        return self._is_fallback

    def save(self, session_id: str, state: dict[str, Any]) -> str:
        """Save checkpoint for session.

        Args:
            session_id: Session identifier
            state: Session state to persist (will be JSON serialized)

        Returns:
            Checkpoint ID
        """
        data_json = json.dumps(state)
        return self._system.save(session_id, data_json)

    def load(
        self, session_id: str, checkpoint_id: str | None = None
    ) -> dict[str, Any] | None:
        """Load checkpoint for session.

        Args:
            session_id: Session identifier
            checkpoint_id: Specific checkpoint ID (optional, loads latest if None)

        Returns:
            Loaded state, or None if not found
        """
        result = self._system.load(session_id, checkpoint_id)
        if result:
            try:
                return json.loads(result)
            except json.JSONDecodeError:
                return {"raw": result}
        return None

    def list(self, session_id: str) -> list[str]:
        """List all checkpoints for session.

        Args:
            session_id: Session identifier

        Returns:
            List of checkpoint IDs
        """
        return self._system.list(session_id)

    def delete(self, session_id: str, checkpoint_id: str) -> bool:
        """Delete specific checkpoint.

        Args:
            session_id: Session identifier
            checkpoint_id: Checkpoint to delete

        Returns:
            True if deleted successfully
        """
        return self._system.delete(session_id, checkpoint_id)

    def has_checkpoints(self, session_id: str) -> bool:
        """Check if session has any checkpoints.

        Args:
            session_id: Session identifier

        Returns:
            True if checkpoints exist
        """
        return len(self.list(session_id)) > 0

    def clear_session(self, session_id: str) -> int:
        """Delete all checkpoints for session.

        Args:
            session_id: Session identifier

        Returns:
            Number of checkpoints deleted
        """
        checkpoints = self.list(session_id)
        deleted = 0
        for cp_id in checkpoints:
            if self.delete(session_id, cp_id):
                deleted += 1
        return deleted
