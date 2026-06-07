"""Memory Storage Backend

Provides storage backend abstraction and implementations.

[STABILITY: STABLE] Storage interface is stable
"""

from __future__ import annotations

import json
import logging
import sqlite3
import threading
from abc import ABC, abstractmethod
from datetime import datetime, timezone
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    pass

logger = logging.getLogger(__name__)


def _utc_now() -> datetime:
    """Get UTC time (timezone-aware)"""
    return datetime.now(timezone.utc)


class MemoryTier(Enum):
    """Memory tier"""

    WORKING = "working"  # Current conversation context
    SESSION = "session"  # Session memory
    PROJECT = "project"  # Project knowledge base
    LONG_TERM = "long_term"  # Cross-project knowledge


@dataclass
class MemoryEntry:
    """Memory entry"""

    id: str
    tier: MemoryTier
    content: str
    metadata: dict[str, Any] = field(default_factory=dict)
    created_at: datetime = field(default_factory=_utc_now)
    last_accessed: datetime = field(default_factory=_utc_now)
    access_count: int = 0
    importance: float = 0.5

    def touch(self) -> None:
        """Update access time and count"""
        self.last_accessed = _utc_now()
        self.access_count += 1


class StorageBackend(ABC):
    """Storage backend abstract class

    Supports multiple storage backends:
    - MemoryStorage: In-memory storage (default, no persistence)
    - FileStorage: JSON file storage
    """

    @abstractmethod
    def save(self, tier: MemoryTier, entry: MemoryEntry) -> None:
        """Save memory entry"""
        pass  # pragma: no cover

    @abstractmethod
    def load(self, tier: MemoryTier, entry_id: str) -> MemoryEntry | None:
        """Load memory entry"""
        pass  # pragma: no cover

    @abstractmethod
    def load_all(self, tier: MemoryTier) -> list[MemoryEntry]:
        """Load all entries in specified tier"""
        pass  # pragma: no cover

    @abstractmethod
    def delete(self, tier: MemoryTier, entry_id: str) -> bool:
        """Delete memory entry"""
        pass  # pragma: no cover

    @abstractmethod
    def clear(self, tier: MemoryTier) -> int:
        """Clear specified tier"""
        pass  # pragma: no cover

    @abstractmethod
    def count(self, tier: MemoryTier) -> int:
        """Get entry count"""
        pass  # pragma: no cover

    @abstractmethod
    def search(
        self, tier: MemoryTier, query: str, limit: int = 10
    ) -> list[MemoryEntry]:
        """Search entries"""
        pass  # pragma: no cover

    @abstractmethod
    def flush(self) -> None:
        """Flush cache to storage"""
        pass  # pragma: no cover

    @abstractmethod
    def close(self) -> None:
        """Close storage"""
        pass  # pragma: no cover


class MemoryStorage(StorageBackend):
    """In-memory storage backend

    No persistence, data is only kept in memory.
    """

    def __init__(self):
        self._storage: dict[MemoryTier, dict[str, MemoryEntry]] = {
            MemoryTier.WORKING: {},
            MemoryTier.SESSION: {},
            MemoryTier.PROJECT: {},
            MemoryTier.LONG_TERM: {},
        }
        self._lock = threading.RLock()

    def save(self, tier: MemoryTier, entry: MemoryEntry) -> None:
        with self._lock:
            self._storage[tier][entry.id] = entry

    def load(self, tier: MemoryTier, entry_id: str) -> MemoryEntry | None:
        with self._lock:
            return self._storage[tier].get(entry_id)

    def load_all(self, tier: MemoryTier) -> list[MemoryEntry]:
        with self._lock:
            return list(self._storage[tier].values())

    def delete(self, tier: MemoryTier, entry_id: str) -> bool:
        with self._lock:
            if entry_id in self._storage[tier]:
                del self._storage[tier][entry_id]
                return True
            return False

    def clear(self, tier: MemoryTier) -> int:
        with self._lock:
            count = len(self._storage[tier])
            self._storage[tier].clear()
            return count

    def count(self, tier: MemoryTier) -> int:
        with self._lock:
            return len(self._storage[tier])

    def search(
        self, tier: MemoryTier, query: str, limit: int = 10
    ) -> list[MemoryEntry]:
        with self._lock:
            results = []
            for entry in self._storage[tier].values():
                if query.lower() in entry.content.lower():
                    entry.touch()
                    results.append(entry)
                    if len(results) >= limit:
                        break
            return results

    def flush(self) -> None:
        """No-op for memory storage"""
        logger.debug("MemoryStorage.flush: no-op")

    def close(self) -> None:
        self._storage.clear()


class FileStorage(StorageBackend):
    """JSON file storage backend

    Each tier is stored as a separate JSON file.
    Supports auto-save and manual save.
    """

    def __init__(
        self,
        base_path: str | Path,
        auto_save: bool = True,
        session_id: str = "default",
    ):
        """Initialize file storage

        Args:
            base_path: Storage directory
            auto_save: Whether to auto-save (save after each modification)
            session_id: Session ID
        """
        self._base_path = Path(base_path)
        self._auto_save = auto_save
        self._session_id = session_id

        self._storage: dict[MemoryTier, dict[str, MemoryEntry]] = {
            MemoryTier.WORKING: {},
            MemoryTier.SESSION: {},
            MemoryTier.PROJECT: {},
            MemoryTier.LONG_TERM: {},
        }
        self._lock = threading.RLock()
        self._dirty = False

        self._ensure_dir()
        self._load_all()

    def _ensure_dir(self) -> None:
        """Ensure storage directory exists"""
        self._base_path.mkdir(parents=True, exist_ok=True)

    def _get_file_path(self, tier: MemoryTier) -> Path:
        """Get tier file path"""
        tier_names = {
            MemoryTier.WORKING: "working",
            MemoryTier.SESSION: "session",
            MemoryTier.PROJECT: "project",
            MemoryTier.LONG_TERM: "long_term",
        }
        return self._base_path / f"{self._session_id}_{tier_names[tier]}.json"

    def _entry_to_dict(self, entry: MemoryEntry) -> dict[str, Any]:
        """Convert entry to dictionary"""
        return {
            "id": entry.id,
            "tier": entry.tier.value,
            "content": entry.content,
            "metadata": entry.metadata,
            "created_at": entry.created_at.isoformat(),
            "last_accessed": entry.last_accessed.isoformat(),
            "access_count": entry.access_count,
            "importance": entry.importance,
        }

    def _dict_to_entry(self, data: dict[str, Any]) -> MemoryEntry:
        """Convert dictionary to entry"""
        return MemoryEntry(
            id=data["id"],
            tier=MemoryTier(data["tier"]),
            content=data["content"],
            metadata=data.get("metadata", {}),
            created_at=datetime.fromisoformat(data["created_at"]),
            last_accessed=datetime.fromisoformat(data["last_accessed"]),
            access_count=data.get("access_count", 0),
            importance=data.get("importance", 0.5),
        )

    def _save_tier(self, tier: MemoryTier) -> None:
        """Save tier to file"""
        file_path = self._get_file_path(tier)
        entries = [self._entry_to_dict(e) for e in self._storage[tier].values()]
        data = {
            "tier": tier.value,
            "session_id": self._session_id,
            "count": len(entries),
            "entries": entries,
            "updated_at": _utc_now().isoformat(),
        }

        with open(file_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

    def _load_tier(self, tier: MemoryTier) -> None:
        """Load tier from file"""
        file_path = self._get_file_path(tier)
        if not file_path.exists():
            return

        try:
            with open(file_path, encoding="utf-8") as f:
                data = json.load(f)

            for entry_data in data.get("entries", []):
                entry = self._dict_to_entry(entry_data)
                self._storage[tier][entry.id] = entry
        except (json.JSONDecodeError, KeyError) as e:
            logger.warning(f"Failed to load {tier} from {file_path}: {e}")

    def _load_all(self) -> None:
        """Load all tiers"""
        for tier in MemoryTier:
            self._load_tier(tier)

    def save(self, tier: MemoryTier, entry: MemoryEntry) -> None:
        with self._lock:
            self._storage[tier][entry.id] = entry
            self._dirty = True
            if self._auto_save:
                self._save_tier(tier)

    def load(self, tier: MemoryTier, entry_id: str) -> MemoryEntry | None:
        with self._lock:
            entry = self._storage[tier].get(entry_id)
            if entry:
                entry.touch()
            return entry

    def load_all(self, tier: MemoryTier) -> list[MemoryEntry]:  # pragma: no cover
        with self._lock:
            return list(self._storage[tier].values())

    def delete(self, tier: MemoryTier, entry_id: str) -> bool:
        with self._lock:
            if entry_id in self._storage[tier]:
                del self._storage[tier][entry_id]
                self._dirty = True
                if self._auto_save:  # pragma: no branch
                    self._save_tier(tier)
                return True
            return False  # pragma: no cover

    def clear(self, tier: MemoryTier) -> int:
        with self._lock:
            count = len(self._storage[tier])
            self._storage[tier].clear()
            self._dirty = True
            if self._auto_save:
                self._save_tier(tier)
            return count

    def count(self, tier: MemoryTier) -> int:
        with self._lock:
            return len(self._storage[tier])

    def search(
        self, tier: MemoryTier, query: str, limit: int = 10
    ) -> list[MemoryEntry]:
        with self._lock:
            results = []
            for entry in self._storage[tier].values():
                if query.lower() in entry.content.lower():
                    entry.touch()
                    results.append(entry)
                    if len(results) >= limit:
                        break
            return results

    def flush(self) -> None:
        """Save all modifications"""
        with self._lock:
            if self._dirty:
                for tier in MemoryTier:
                    self._save_tier(tier)
                self._dirty = False

    def close(self) -> None:
        """Close storage and save all data"""
        self.flush()
        self._storage.clear()

    def save_all(self) -> None:
        """Save all tiers"""
        self.flush()

    @staticmethod
    def get_default_storage_path() -> Path:
        """Get default storage path"""
        return Path.home() / ".continuum" / "memory"


class SQLiteStorage(StorageBackend):
    """SQLite storage backend

    Uses SQLite database for persistent storage.
    Supports efficient queries and transactions.

    Features:
        - Single-file database, easy to manage
        - Full-text search (FTS) support
        - Transaction support
        - Efficient batch operations
    """

    def __init__(
        self,
        db_path: str | Path,
        session_id: str = "default",
        auto_commit: bool = True,
        enable_fts: bool = True,
    ):
        """Initialize SQLite storage

        Args:
            db_path: Database file path
            session_id: Session ID
            auto_commit: Whether to auto-commit
            enable_fts: Whether to enable full-text search
        """
        import sqlite3

        self._db_path = Path(db_path)
        self._session_id = session_id
        self._auto_commit = auto_commit
        self._enable_fts = enable_fts
        self._lock = threading.RLock()

        # Ensure directory exists
        self._db_path.parent.mkdir(parents=True, exist_ok=True)

        # Connect to database
        self._conn = sqlite3.connect(
            str(self._db_path),
            check_same_thread=False,
        )
        self._conn.row_factory = sqlite3.Row

        # Initialize table structure
        self._init_tables()

        logger.info(f"SQLiteStorage initialized: {self._db_path}")

    def _init_tables(self) -> None:
        """Initialize database tables"""
        cursor = self._conn.cursor()

        # Main memory table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                tier TEXT NOT NULL,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                access_count INTEGER DEFAULT 0,
                importance REAL DEFAULT 0.5
            )
        """)

        # Indexes
        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_tier_session
            ON memories(tier, session_id)
        """)

        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_created_at
            ON memories(created_at)
        """)

        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_importance
            ON memories(importance DESC)
        """)

        # Full-text search table (optional)
        if self._enable_fts:
            cursor.execute("""
                CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts
                USING fts5(id, content, metadata)
            """)

        self._conn.commit()

    def save(self, tier: MemoryTier, entry: MemoryEntry) -> None:
        """Save memory entry"""
        import json

        with self._lock:
            cursor = self._conn.cursor()

            # Insert or replace
            cursor.execute("""
                INSERT OR REPLACE INTO memories
                (id, tier, session_id, content, metadata, created_at, last_accessed, access_count, importance)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                entry.id,
                tier.value,
                self._session_id,
                entry.content,
                json.dumps(entry.metadata),
                entry.created_at.isoformat(),
                entry.last_accessed.isoformat(),
                entry.access_count,
                entry.importance,
            ))

            # Update full-text search index
            if self._enable_fts:
                cursor.execute("""
                    INSERT OR REPLACE INTO memories_fts (id, content, metadata)
                    VALUES (?, ?, ?)
                """, (
                    entry.id,
                    entry.content,
                    json.dumps(entry.metadata),
                ))

            if self._auto_commit:
                self._conn.commit()

    def load(self, tier: MemoryTier, entry_id: str) -> MemoryEntry | None:
        """Load memory entry"""
        with self._lock:
            cursor = self._conn.cursor()
            cursor.execute("""
                SELECT * FROM memories
                WHERE tier = ? AND session_id = ? AND id = ?
            """, (tier.value, self._session_id, entry_id))

            row = cursor.fetchone()
            if row:
                # Update access info
                cursor.execute("""
                    UPDATE memories
                    SET last_accessed = ?, access_count = access_count + 1
                    WHERE id = ?
                """, (_utc_now().isoformat(), entry_id))

                if self._auto_commit:  # pragma: no branch
                    self._conn.commit()

                return self._row_to_entry(row)
            return None  # pragma: no cover

    def load_all(self, tier: MemoryTier) -> list[MemoryEntry]:  # pragma: no cover
        """Load all entries in specified tier"""
        with self._lock:
            cursor = self._conn.cursor()
            cursor.execute("""
                SELECT * FROM memories
                WHERE tier = ? AND session_id = ?
                ORDER BY created_at DESC
            """, (tier.value, self._session_id))

            return [self._row_to_entry(row) for row in cursor.fetchall()]

    def delete(self, tier: MemoryTier, entry_id: str) -> bool:
        """Delete memory entry"""
        with self._lock:
            cursor = self._conn.cursor()

            # Check if exists
            cursor.execute("""
                SELECT id FROM memories
                WHERE tier = ? AND session_id = ? AND id = ?
            """, (tier.value, self._session_id, entry_id))

            if not cursor.fetchone():
                return False

            # Delete
            cursor.execute("""
                DELETE FROM memories WHERE id = ?
            """, (entry_id,))

            # Delete full-text search index
            if self._enable_fts:  # pragma: no branch
                cursor.execute("""
                    DELETE FROM memories_fts WHERE id = ?
                """, (entry_id,))

            if self._auto_commit:  # pragma: no branch
                self._conn.commit()

            return True

    def clear(self, tier: MemoryTier) -> int:
        """Clear specified tier"""
        import json

        with self._lock:
            cursor = self._conn.cursor()

            # Get list of IDs to delete
            cursor.execute("""
                SELECT id FROM memories
                WHERE tier = ? AND session_id = ?
            """, (tier.value, self._session_id))

            ids = [row["id"] for row in cursor.fetchall()]
            count = len(ids)

            if count == 0:
                return 0

            # Delete main table records
            placeholders = ",".join("?" * len(ids))
            cursor.execute(f"""
                DELETE FROM memories WHERE id IN ({placeholders})
            """, ids)

            # Delete full-text search index
            if self._enable_fts:  # pragma: no branch
                cursor.execute(f"""
                    DELETE FROM memories_fts WHERE id IN ({placeholders})
                """, ids)

            if self._auto_commit:  # pragma: no branch
                self._conn.commit()

            return count

    def count(self, tier: MemoryTier) -> int:
        """Get entry count"""
        with self._lock:
            cursor = self._conn.cursor()
            cursor.execute("""
                SELECT COUNT(*) FROM memories
                WHERE tier = ? AND session_id = ?
            """, (tier.value, self._session_id))
            return cursor.fetchone()[0]

    def search(
        self, tier: MemoryTier, query: str, limit: int = 10
    ) -> list[MemoryEntry]:
        """Search entries"""
        import json

        with self._lock:
            cursor = self._conn.cursor()

            if self._enable_fts:
                # Use full-text search
                cursor.execute("""
                    SELECT m.* FROM memories m
                    JOIN memories_fts fts ON m.id = fts.id
                    WHERE m.tier = ? AND m.session_id = ?
                    AND memories_fts MATCH ?
                    ORDER BY m.importance DESC
                    LIMIT ?
                """, (tier.value, self._session_id, query, limit))
            else:
                # Use LIKE search
                cursor.execute("""
                    SELECT * FROM memories
                    WHERE tier = ? AND session_id = ?
                    AND content LIKE ?
                    ORDER BY importance DESC
                    LIMIT ?
                """, (tier.value, self._session_id, f"%{query}%", limit))

            return [self._row_to_entry(row) for row in cursor.fetchall()]

    def flush(self) -> None:
        """Flush cache to storage"""
        with self._lock:
            self._conn.commit()

    def close(self) -> None:
        """Close storage"""
        with self._lock:
            self._conn.commit()
            self._conn.close()

    def _row_to_entry(self, row: sqlite3.Row) -> MemoryEntry:
        """Convert database row to MemoryEntry"""
        import json

        return MemoryEntry(
            id=row["id"],
            tier=MemoryTier(row["tier"]),
            content=row["content"],
            metadata=json.loads(row["metadata"]) if row["metadata"] else {},
            created_at=datetime.fromisoformat(row["created_at"]),
            last_accessed=datetime.fromisoformat(row["last_accessed"]),
            access_count=row["access_count"],
            importance=row["importance"],
        )

    def vacuum(self) -> None:
        """Clean up database and reclaim space"""
        with self._lock:
            self._conn.execute("VACUUM")
            self._conn.commit()

    def get_stats(self) -> dict[str, Any]:
        """Get database statistics"""
        with self._lock:
            cursor = self._conn.cursor()

            stats = {}

            # Total record count
            cursor.execute("SELECT COUNT(*) FROM memories")
            stats["total_records"] = cursor.fetchone()[0]

            # Record count per tier
            cursor.execute("""
                SELECT tier, COUNT(*) as count
                FROM memories
                WHERE session_id = ?
                GROUP BY tier
            """, (self._session_id,))

            stats["by_tier"] = {
                row["tier"]: row["count"] for row in cursor.fetchall()
            }

            # Database size
            stats["db_size_bytes"] = self._db_path.stat().st_size if self._db_path.exists() else 0

            return stats
