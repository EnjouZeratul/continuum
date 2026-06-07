"""Tiered Memory API

Provides Working -> Session -> Project -> LongTerm four-tier memory.

[STABILITY: STABLE] Core API is stable
Supports multiple storage backends: MemoryStorage, FileStorage
"""

from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from continuum_sdk.utils import generate_short_id
from .storage import FileStorage, MemoryStorage, StorageBackend, MemoryEntry, MemoryTier


@dataclass
class MemoryQuery:
    """Memory query"""

    query: str
    tier: MemoryTier | None = None
    limit: int = 10
    time_range: tuple | None = None


class TierProxy:
    """Tier proxy

    Usage:
        memory.working().add("content")
        memory.working().search("query")
        memory.working().clear()
    """

    def __init__(self, memory: "Memory", tier: MemoryTier):
        self._memory = memory
        self._tier = tier

    def add(
        self,
        content: str,
        metadata: dict[str, Any] | None = None,
        importance: float = 0.5,
    ) -> str:
        """Add memory"""
        return self._memory.remember(
            content, tier=self._tier, metadata=metadata, importance=importance
        )

    def search(self, query: str, limit: int = 10) -> list[MemoryEntry]:
        """Search memories"""
        return self._memory.recall(query, tier=self._tier, limit=limit)

    def get(self, memory_id: str) -> MemoryEntry | None:
        """Get memory"""
        return self._memory.get(self._tier, memory_id)

    def remove(self, memory_id: str) -> bool:
        """Remove memory"""
        return self._memory.forget(self._tier, memory_id)

    def clear(self) -> int:
        """Clear tier"""
        return self._memory.clear(self._tier)

    def count(self) -> int:
        """Get count"""
        stats = self._memory.stats()
        return stats.get(self._tier, 0)


class Memory:
    """Tiered memory system

    Usage:
        from continuum_sdk.memory import Memory, MemoryTier

        # Create in-memory storage (default)
        memory = Memory(session_id="session-123")

        # Create with file persistence
        memory = Memory(
            session_id="session-123",
            storage=FileStorage("~/.continuum/memory", session_id="session-123")
        )

        # Store memory
        memory.store("working", "Important fact")
        memory.store("project", "Project config")

        # Query memory
        results = memory.query("fact")

        # Get specific tier
        working = memory.get_tier(MemoryTier.WORKING)

        # Statistics
        stats = memory.stats()

        # Convenience access
        memory.working().add("temporary info")
        results = memory.working().search("keyword")

        # Persistence operations
        memory.save()  # Save to file
        memory.close()  # Close and save
    """

    def __init__(
        self,
        session_id: str,
        storage: StorageBackend | None = None,
        auto_persist: bool = False,
    ):
        """Initialize memory system

        Args:
            session_id: Session ID
            storage: Storage backend (None means use memory storage)
            auto_persist: Whether to auto-persist (only effective for FileStorage)
        """
        self._session_id = session_id
        self._auto_persist = auto_persist

        # Use provided storage backend, or default to in-memory storage
        if storage is not None:
            self._backend = storage
        else:
            self._backend = MemoryStorage()

        # Working memory size limit
        self._working_limit = 100

    @property
    def session_id(self) -> str:
        """Get session ID"""
        return self._session_id

    def remember(
        self,
        content: str,
        tier: MemoryTier = MemoryTier.WORKING,
        metadata: dict[str, Any] | None = None,
        importance: float = 0.5,
    ) -> str:
        """Store memory

        Args:
            content: Memory content
            tier: Memory tier
            metadata: Metadata (optional)
            importance: Importance score (0.0-1.0)

        Returns:
            Memory ID
        """
        entry = MemoryEntry(
            id=generate_short_id(),
            tier=tier,
            content=content,
            metadata=metadata or {},
            importance=importance,
        )

        # Save to storage backend
        self._backend.save(tier, entry)

        # Enforce working memory size limit
        if tier == MemoryTier.WORKING:
            count = self._backend.count(tier)
            if count > self._working_limit:
                entries = self._backend.load_all(tier)
                if entries:
                    oldest = min(entries, key=lambda e: e.created_at)
                    self._backend.delete(tier, oldest.id)

        return entry.id

    def recall(
        self, query: str, tier: MemoryTier | None = None, limit: int = 10
    ) -> list[MemoryEntry]:
        """Query memory

        Args:
            query: Query text
            tier: Restrict tier (optional)
            limit: Result count limit

        Returns:
            List of matching memory entries
        """
        results = []

        # Search order: Working -> Session -> Project -> LongTerm
        tiers = (
            [tier]
            if tier
            else [
                MemoryTier.WORKING,
                MemoryTier.SESSION,
                MemoryTier.PROJECT,
                MemoryTier.LONG_TERM,
            ]
        )

        for t in tiers:
            entries = self._backend.search(t, query, limit - len(results))
            results.extend(entries)
            if len(results) >= limit:
                break

        return results

    def get(self, tier: MemoryTier, memory_id: str) -> MemoryEntry | None:
        """Get specific memory

        Args:
            tier: Memory tier
            memory_id: Memory ID

        Returns:
            Memory entry (if exists)
        """
        return self._backend.load(tier, memory_id)

    def forget(self, tier: MemoryTier, memory_id: str) -> bool:
        """Delete memory

        Args:
            tier: Memory tier
            memory_id: Memory ID

        Returns:
            Whether deletion was successful
        """
        return self._backend.delete(tier, memory_id)

    def clear(self, tier: MemoryTier) -> int:
        """Clear specified tier

        Args:
            tier: Memory tier

        Returns:
            Number of deleted memories
        """
        return self._backend.clear(tier)

    def stats(self) -> dict[MemoryTier, int]:
        """Get tier statistics

        Returns:
            Memory count per tier
        """
        return {tier: self._backend.count(tier) for tier in MemoryTier}

    # ==================== Persistence Methods ====================

    def save(self) -> None:
        """Save all memories to storage"""
        self._backend.flush()

    def close(self) -> None:
        """Close memory system and save all data"""
        self._backend.close()

    @staticmethod
    def get_default_storage_path() -> Path:
        """Get default storage path"""
        return FileStorage.get_default_storage_path()

    @classmethod
    def create_with_file_storage(
        cls,
        session_id: str,
        storage_path: str | Path | None = None,
        auto_persist: bool = True,
    ) -> "Memory":
        """Create memory system with file storage

        Args:
            session_id: Session ID
            storage_path: Storage path (default ~/.continuum/memory)
            auto_persist: Whether to auto-persist

        Returns:
            Memory instance
        """
        path = Path(storage_path) if storage_path else cls.get_default_storage_path()
        storage = FileStorage(path, auto_save=auto_persist, session_id=session_id)
        return cls(session_id=session_id, storage=storage, auto_persist=auto_persist)

    @classmethod
    def create_with_sqlite_storage(
        cls,
        session_id: str,
        db_path: str | Path | None = None,
        enable_fts: bool = True,
    ) -> "Memory":
        """Create memory system with SQLite storage (recommended)

        SQLite storage provides better performance and query capabilities, suitable for production.

        Args:
            session_id: Session ID
            db_path: Database path (default ~/.continuum/memory/memory.db)
            enable_fts: Whether to enable full-text search

        Returns:
            Memory instance
        """
        from .storage import SQLiteStorage

        if db_path is None:
            db_path = cls.get_default_storage_path() / "memory.db"
        else:
            db_path = Path(db_path)

        storage = SQLiteStorage(db_path, session_id=session_id, enable_fts=enable_fts)
        return cls(session_id=session_id, storage=storage, auto_persist=True)

    # ==================== Convenience Methods ====================

    def working(self) -> TierProxy:
        """Get working memory proxy"""
        return TierProxy(self, MemoryTier.WORKING)

    def session(self) -> TierProxy:
        """Get session memory proxy"""
        return TierProxy(self, MemoryTier.SESSION)

    def project(self) -> TierProxy:
        """Get project memory proxy"""
        return TierProxy(self, MemoryTier.PROJECT)

    def long_term(self) -> TierProxy:
        """Get long-term memory proxy"""
        return TierProxy(self, MemoryTier.LONG_TERM)

    # ==================== Serialization ====================

    def to_dict(self) -> dict[str, Any]:
        """Export as dictionary"""
        return {
            "session_id": self._session_id,
            "stats": {tier.value: count for tier, count in self.stats().items()},
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Memory":
        """Create from dictionary"""
        return cls(session_id=data.get("session_id", ""))
