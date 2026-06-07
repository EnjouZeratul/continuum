"""Continuum SDK Memory Module

Tiered memory system for conversation context management.

Features:
    - Short-term memory (recent messages)
    - Long-term memory (important facts)
    - Episodic memory (session history)
    - Semantic memory (knowledge base)
    - Automatic tier promotion
    - Multiple storage backends (memory, file, sqlite)

Tier Levels:
    - Tier 0: Immediate context (last N messages)
    - Tier 1: Short-term (active session)
    - Tier 2: Long-term (persisted facts)
    - Tier 3: Archive (historical data)

Storage Backends:
    - MemoryStorage: In-memory storage (default, no persistence)
    - FileStorage: JSON file-based persistence
    - SQLiteStorage: SQLite database persistence (recommended for production)

Quick Start:
    >>> from continuum_sdk.memory import Memory, MemoryTier
    >>>
    >>> # In-memory storage (default)
    >>> memory = Memory(session_id="session-123")
    >>>
    >>> # File-based persistence
    >>> memory = Memory.create_with_file_storage("session-123")
    >>>
    >>> # SQLite persistence (recommended)
    >>> memory = Memory.create_with_sqlite_storage("session-123")
    >>>
    >>> # Add entry
    >>> memory.remember("User prefers Python", tier=MemoryTier.LONG_TERM)
    >>>
    >>> # Retrieve relevant context
    >>> context = memory.recall("Python")
    >>> print(context[0].content)  # "User prefers Python"
"""

# First import base types from storage
# Then import high-level API from layers
from .layers import Memory, MemoryQuery, TierProxy
from .storage import (
    FileStorage,
    MemoryEntry,
    MemoryStorage,
    MemoryTier,
    SQLiteStorage,
    StorageBackend,
)

__all__ = [
    # Core API
    "Memory",
    "MemoryTier",
    "MemoryEntry",
    "MemoryQuery",
    "TierProxy",
    # Storage backends
    "StorageBackend",
    "MemoryStorage",
    "FileStorage",
    "SQLiteStorage",
]
