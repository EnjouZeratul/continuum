"""Audit Logger - Operation Audit Logging

Records all file operation history, supporting traceability and compliance checks.

Features:
    - Operation recording: Complete history of all file operations
    - Timestamp: Operation time precise to milliseconds
    - Operation types: READ/WRITE/CREATE/DELETE/MOVE/COPY
    - Result recording: Success/failure and reasons
    - User tracking: Record operation initiator
    - Query interface: Support query by time/type/path
    - Rate limiting: Prevent audit log flooding (DoS protection)
    - Sensitive data filtering: Redact API keys, tokens, passwords
    - Log rotation: Automatic log file rotation

Audit Record:
    - timestamp: Operation time
    - operation: Operation type
    - path: Operation path
    - result: Operation result
    - user: Operation user
    - details: Operation details

Quick Start:
    >>> from continuum_sdk.security import AuditLogger, AuditOperation
    >>>
    >>> logger = AuditLogger()
    >>>
    >>> # Log operation
    >>> logger.log(
    ...     operation=AuditOperation.READ,
    ...     path="/project/src/main.py",
    ...     result="success"
    ... )
    >>>
    >>> # Query history
    >>> records = logger.query(path="/project/src/")
    >>> for record in records:
    ...     print(f"{record.timestamp}: {record.operation.value} {record.path}")

Query Operations:
    >>> # Query by time range
    >>> from datetime import datetime, timedelta
    >>> start = datetime.now() - timedelta(hours=1)
    >>> records = logger.query(start_time=start)
    >>>
    >>> # Query by operation type
    >>> records = logger.query(operation=AuditOperation.WRITE)
    >>>
    >>> # Combined query
    >>> records = logger.query(
    ...     path="/project/",
    ...     operation=AuditOperation.DELETE,
    ...     start_time=start
    ... )

Export:
    >>> # Export as JSON
    >>> logger.export_json("/path/to/audit.json")
    >>>
    >>> # Export as CSV
    >>> logger.export_csv("/path/to/audit.csv")
"""

from __future__ import annotations

import csv
import json
import logging
import os
import re
import threading
from collections import deque
from collections.abc import Iterator
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


# ==================== Rate Limiting ====================


class RateLimiter:
    """Sliding window rate limiter for audit logs.

    Prevents DoS attacks by limiting the number of audit records
    that can be logged within a time window.
    """

    def __init__(self, max_records: int = 100, window_seconds: float = 1.0):
        """Initialize rate limiter.

        Args:
            max_records: Maximum records allowed within the window
            window_seconds: Window duration in seconds
        """
        self._max_records = max_records
        self._window = timedelta(seconds=window_seconds)
        self._timestamps: deque[datetime] = deque()
        self._lock = threading.Lock()
        self._dropped_count = 0

    def allow(self) -> bool:
        """Check if a new record is allowed.

        Returns:
            True if allowed, False if rate limited
        """
        with self._lock:
            now = datetime.now()
            cutoff = now - self._window

            # Clean up expired timestamps
            while self._timestamps and self._timestamps[0] < cutoff:
                self._timestamps.popleft()

            if len(self._timestamps) >= self._max_records:
                self._dropped_count += 1
                if self._dropped_count == 1:
                    logger.warning(
                        f"Audit log rate limit exceeded. "
                        f"Max {self._max_records} records per {self._window.total_seconds()}s. "
                        f"Subsequent drops will be silent."
                    )
                return False

            self._timestamps.append(now)
            return True

    @property
    def dropped_count(self) -> int:
        """Get count of dropped records due to rate limiting."""
        return self._dropped_count


# ==================== Sensitive Data Filtering ====================


# Patterns for sensitive data detection and replacement
SENSITIVE_PATTERNS: list[tuple[str, str]] = [
    # API Keys
    (r'(api[_-]?key["\s:=]+)["\']?[\w-]{20,}["\']?', r"\1[REDACTED]"),
    (r'(apikey["\s:=]+)["\']?[\w-]{20,}["\']?', r"\1[REDACTED]"),
    # Tokens
    (r'(token["\s:=]+)["\']?[\w-]{20,}["\']?', r"\1[REDACTED]"),
    (r'(access_token["\s:=]+)["\']?[\w-]{20,}["\']?', r"\1[REDACTED]"),
    (r'(refresh_token["\s:=]+)["\']?[\w-]{20,}["\']?', r"\1[REDACTED]"),
    # Passwords
    (r'(password["\s:=]+)["\']?[\w-]+["\']?', r"\1[REDACTED]"),
    (r'(passwd["\s:=]+)["\']?[\w-]+["\']?', r"\1[REDACTED]"),
    # AWS
    (r"AKIA[A-Z0-9]{16}", r"AKIA[REDACTED]"),
    (r'(aws_secret_access_key["\s:=]+)["\']?[\w/+=]{40}["\']?', r"\1[REDACTED]"),
    # Private Keys
    (r"-----BEGIN[^-]*PRIVATE KEY-----", r"-----BEGIN [REDACTED] PRIVATE KEY-----"),
    (r"-----END[^-]*PRIVATE KEY-----", r"-----END [REDACTED] PRIVATE KEY-----"),
    # Bearer tokens
    (r"Bearer[\s]+[\w-]+\.[\w-]+\.[\w-]+", r"Bearer [REDACTED]"),
]

_SENSITIVE_REGEX: list[tuple[re.Pattern, str]] = [
    (re.compile(pattern, re.IGNORECASE), replacement)
    for pattern, replacement in SENSITIVE_PATTERNS
]


def sanitize_string(text: str) -> str:
    """Filter sensitive data from string.

    Args:
        text: String to sanitize

    Returns:
        Sanitized string with sensitive data replaced
    """
    result = text
    for pattern, replacement in _SENSITIVE_REGEX:
        result = pattern.sub(replacement, result)
    return result


def sanitize_metadata(metadata: dict[str, Any]) -> dict[str, Any]:
    """Filter sensitive data from metadata dictionary.

    Recursively processes dictionaries and lists, sanitizing all string values.

    Args:
        metadata: Dictionary to sanitize

    Returns:
        Sanitized dictionary with sensitive data replaced
    """
    result: dict[str, Any] = {}
    for key, value in metadata.items():
        # Sanitize key name
        safe_key = sanitize_string(key) if isinstance(key, str) else key

        # Sanitize value based on type
        if isinstance(value, str):
            result[safe_key] = sanitize_string(value)
        elif isinstance(value, dict):
            result[safe_key] = sanitize_metadata(value)
        elif isinstance(value, list):
            result[safe_key] = [
                (
                    sanitize_metadata(item)
                    if isinstance(item, dict)
                    else sanitize_string(item) if isinstance(item, str) else item
                )
                for item in value
            ]
        else:
            result[safe_key] = value

    return result


# ==================== Log Rotation ====================


# Default rotation settings
MAX_LOG_SIZE_MB = 100
MAX_BACKUP_COUNT = 5


def _rotate_if_needed(log_path: Path) -> None:
    """Check and rotate log file if needed.

    Rotates when file exceeds MAX_LOG_SIZE_MB.
    Keeps MAX_BACKUP_COUNT rotated files.

    Args:
        log_path: Path to log file
    """
    if not log_path.exists():
        return

    size_mb = log_path.stat().st_size / (1024 * 1024)
    if size_mb <= MAX_LOG_SIZE_MB:
        return

    # Rotate log file
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_path = log_path.with_suffix(f".{timestamp}.log")

    logger.info(f"Rotating audit log: {log_path} -> {backup_path}")
    log_path.rename(backup_path)

    # Clean up old backups
    backup_pattern = f"{log_path.stem}.*.log"
    backups = sorted(log_path.parent.glob(backup_pattern), reverse=True)

    for old_backup in backups[MAX_BACKUP_COUNT:]:
        logger.debug(f"Removing old backup: {old_backup}")
        old_backup.unlink()


class AuditOperation(Enum):
    """Audit operation types"""

    READ = "read"  # Read file
    WRITE = "write"  # Write file
    CREATE = "create"  # Create file
    DELETE = "delete"  # Delete file
    MOVE = "move"  # Move file
    COPY = "copy"  # Copy file
    RENAME = "rename"  # Rename file
    MODIFY = "modify"  # Modify file content
    ACCESS = "access"  # Access (e.g., check existence)
    LIST = "list"  # List directory
    EXECUTE = "execute"  # Execute file


class AuditResult(Enum):
    """Audit results"""

    SUCCESS = "success"  # Success
    FAILURE = "failure"  # Failure
    DENIED = "denied"  # Denied
    ERROR = "error"  # Error


@dataclass
class AuditRecord:
    """Audit record

    Attributes:
        id: Record ID
        timestamp: Operation time
        operation: Operation type
        path: Operation path
        result: Operation result
        user: Operation user
        process_id: Process ID
        details: Operation details
        metadata: Additional information
    """

    id: str
    timestamp: datetime
    operation: AuditOperation
    path: str
    result: AuditResult
    user: str | None = None
    process_id: int | None = None
    details: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "timestamp": self.timestamp.isoformat(),
            "operation": self.operation.value,
            "path": self.path,
            "result": self.result.value,
            "user": self.user,
            "process_id": self.process_id,
            "details": self.details,
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> AuditRecord:
        return cls(
            id=data["id"],
            timestamp=datetime.fromisoformat(data["timestamp"]),
            operation=AuditOperation(data["operation"]),
            path=data["path"],
            result=AuditResult(data["result"]),
            user=data.get("user"),
            process_id=data.get("process_id"),
            details=data.get("details"),
            metadata=data.get("metadata", {}),
        )


class AuditLogger:
    """Audit logger with security enhancements.

    Security features:
    - Rate limiting to prevent log flooding
    - Sensitive data filtering (API keys, tokens, passwords)
    - Log rotation to prevent disk exhaustion

    Example:
        >>> audit = AuditLogger()
        >>> audit.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        >>> records = audit.query(operation=AuditOperation.READ)
    """

    def __init__(
        self,
        log_file: str | Path | None = None,
        max_records: int = 10000,
        auto_flush: bool = True,
        flush_interval: float = 5.0,
        rate_limit: int = 100,
        rate_window: float = 1.0,
    ):
        """Initialize audit logger

        Args:
            log_file: Log file path (None means memory only)
            max_records: Maximum records (memory)
            auto_flush: Whether to auto-flush to file
            flush_interval: Flush interval (seconds)
            rate_limit: Max records per rate window (DoS protection)
            rate_window: Rate limit window in seconds
        """
        self._log_file = Path(log_file) if log_file else None
        self._max_records = max_records
        self._auto_flush = auto_flush
        self._flush_interval = flush_interval

        self._records: list[AuditRecord] = []
        self._lock = threading.RLock()
        self._counter = 0
        self._last_flush = datetime.now()

        # Rate limiter for DoS protection
        self._rate_limiter = RateLimiter(
            max_records=rate_limit, window_seconds=rate_window
        )

        # Ensure log directory exists
        if self._log_file:
            self._log_file.parent.mkdir(parents=True, exist_ok=True)
            self._load_existing_records()

    def _generate_id(self) -> str:
        """Generate record ID"""
        self._counter += 1
        return f"audit-{datetime.now().strftime('%Y%m%d%H%M%S')}-{self._counter:06d}"

    def log(
        self,
        operation: AuditOperation,
        path: str | Path,
        result: AuditResult,
        user: str | None = None,
        details: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> AuditRecord | None:
        """Log operation with rate limiting and sensitive data filtering.

        Args:
            operation: Operation type
            path: Operation path
            result: Operation result
            user: Operation user
            details: Operation details
            metadata: Additional information

        Returns:
            Audit record, or None if rate limited
        """
        # 1. Rate limit check
        if not self._rate_limiter.allow():
            return None

        # 2. Sanitize sensitive data
        safe_metadata = sanitize_metadata(metadata) if metadata else {}
        safe_details = sanitize_string(details) if details else None
        safe_path = sanitize_string(str(path))

        # 3. Check log rotation
        if self._log_file:
            _rotate_if_needed(self._log_file)

        # 4. Create record
        record = AuditRecord(
            id=self._generate_id(),
            timestamp=datetime.now(),
            operation=operation,
            path=safe_path,
            result=result,
            user=user or self._get_current_user(),
            process_id=os.getpid(),
            details=safe_details,
            metadata=safe_metadata,
        )

        with self._lock:
            self._records.append(record)

            # Limit memory records
            if len(self._records) > self._max_records:
                self._records = self._records[-self._max_records :]

            # Auto-flush
            if self._auto_flush and self._log_file:
                elapsed = (datetime.now() - self._last_flush).total_seconds()
                if elapsed >= self._flush_interval:
                    self._append_to_file(record)

        logger.debug(f"Audit: {operation.value} {safe_path} -> {result.value}")

        return record

    @property
    def dropped_records(self) -> int:
        """Get count of records dropped due to rate limiting."""
        return self._rate_limiter.dropped_count

    def query(
        self,
        path: str | None = None,
        operation: AuditOperation | None = None,
        result: AuditResult | None = None,
        user: str | None = None,
        start_time: datetime | None = None,
        end_time: datetime | None = None,
        limit: int = 100,
    ) -> list[AuditRecord]:
        """Query audit records

        Args:
            path: Path prefix filter
            operation: Operation type filter
            result: Result filter
            user: User filter
            start_time: Start time
            end_time: End time
            limit: Result count limit

        Returns:
            Matching audit records
        """
        with self._lock:
            results = []

            for record in reversed(self._records):  # Newest first
                # Path filter
                if path and not record.path.startswith(path):
                    continue

                # Operation type filter
                if operation and record.operation != operation:
                    continue

                # Result filter
                if result and record.result != result:
                    continue

                # User filter
                if user and record.user != user:
                    continue

                # Time range filter
                if start_time and record.timestamp < start_time:
                    continue
                if end_time and record.timestamp > end_time:
                    continue

                results.append(record)

                if len(results) >= limit:
                    break

            return results

    def get_by_path(self, path: str) -> list[AuditRecord]:
        """Get all records for specified path

        Args:
            path: File path

        Returns:
            Audit record list
        """
        return self.query(path=path, limit=self._max_records)

    def get_by_time_range(self, start: datetime, end: datetime) -> list[AuditRecord]:
        """Get records within time range

        Args:
            start: Start time
            end: End time

        Returns:
            Audit record list
        """
        return self.query(start_time=start, end_time=end, limit=self._max_records)

    def get_recent(self, count: int = 10) -> list[AuditRecord]:
        """Get recent records

        Args:
            count: Count

        Returns:
            Audit record list
        """
        with self._lock:
            return list(reversed(self._records[-count:]))

    def get_statistics(self) -> dict[str, Any]:
        """Get audit statistics

        Returns:
            Statistics info
        """
        with self._lock:
            total = len(self._records)

            if total == 0:
                return {
                    "total": 0,
                    "operations": {},
                    "results": {},
                    "paths": set(),
                }

            operations: dict[str, int] = {}
            results: dict[str, int] = {}
            paths: set[str] = set()

            for record in self._records:
                op = record.operation.value
                operations[op] = operations.get(op, 0) + 1

                res = record.result.value
                results[res] = results.get(res, 0) + 1

                paths.add(record.path)

            return {
                "total": total,
                "operations": operations,
                "results": results,
                "unique_paths": len(paths),
                "first_record": (
                    self._records[0].timestamp.isoformat() if self._records else None
                ),
                "last_record": (
                    self._records[-1].timestamp.isoformat() if self._records else None
                ),
            }

    def export_json(self, path: str | Path) -> int:
        """Export as JSON file

        Args:
            path: Export path

        Returns:
            Exported record count
        """
        with self._lock:
            data = [r.to_dict() for r in self._records]

        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

        return len(data)

    def export_csv(self, path: str | Path) -> int:
        """Export as CSV file

        Args:
            path: Export path

        Returns:
            Exported record count
        """
        with self._lock:
            records = self._records.copy()

        with open(path, "w", encoding="utf-8", newline="") as f:
            writer = csv.writer(f)
            writer.writerow(
                [
                    "id",
                    "timestamp",
                    "operation",
                    "path",
                    "result",
                    "user",
                    "process_id",
                    "details",
                ]
            )

            for r in records:
                writer.writerow(
                    [
                        r.id,
                        r.timestamp.isoformat(),
                        r.operation.value,
                        r.path,
                        r.result.value,
                        r.user or "",
                        r.process_id or "",
                        r.details or "",
                    ]
                )

        return len(records)

    def clear(self) -> int:
        """Clear audit records

        Returns:
            Cleared record count
        """
        with self._lock:
            count = len(self._records)
            self._records.clear()
            self._counter = 0

        return count

    def flush(self) -> None:
        """Manually flush to file"""
        if not self._log_file:
            return

        with self._lock:
            with open(self._log_file, "a", encoding="utf-8") as f:
                for record in self._records:
                    f.write(json.dumps(record.to_dict()) + "\n")

            self._last_flush = datetime.now()

    def _append_to_file(self, record: AuditRecord) -> None:
        """Append single record to file"""
        if not self._log_file:
            return

        with open(self._log_file, "a", encoding="utf-8") as f:
            f.write(json.dumps(record.to_dict()) + "\n")

        self._last_flush = datetime.now()

    def _load_existing_records(self) -> None:
        """Load existing records from file"""
        if not self._log_file or not self._log_file.exists():
            return

        try:
            with open(self._log_file, encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line:
                        try:
                            data = json.loads(line)
                            record = AuditRecord.from_dict(data)
                            self._records.append(record)
                        except (json.JSONDecodeError, KeyError):
                            continue

            logger.info(
                f"Loaded {len(self._records)} audit records from {self._log_file}"
            )

        except (OSError, FileNotFoundError) as e:
            logger.warning(f"Failed to load audit records: {e}")

    def _get_current_user(self) -> str:
        """Get current user"""
        try:
            return os.getlogin()
        except (OSError, PermissionError):
            return "unknown"

    def __iter__(self) -> Iterator[AuditRecord]:
        """Iterate all records"""
        with self._lock:
            return iter(self._records.copy())

    def __len__(self) -> int:
        """Get record count"""
        return len(self._records)

    def __repr__(self) -> str:
        return f"AuditLogger(records={len(self._records)}, file={self._log_file})"


# ==================== Exports ====================

__all__ = [
    "AuditLogger",
    "AuditOperation",
    "AuditResult",
    "AuditRecord",
    "RateLimiter",
    "sanitize_metadata",
    "sanitize_string",
    "MAX_LOG_SIZE_MB",
    "MAX_BACKUP_COUNT",
]
