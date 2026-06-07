"""Change Previewer - Change Preview and Confirmation

Preview changes and request user confirmation before executing high-risk operations.

Features:
    - Change preview: Show change content before operation
    - Diff display: Visualize file changes
    - Risk assessment: Automatically assess operation risk level
    - User confirmation: High-risk operations require user confirmation
    - Batch confirmation: Support batch operation confirmation
    - Skip configuration: Configurable skip confirmation for operation types

Risk Levels:
    - LOW: Low risk (e.g., reading files)
    - MEDIUM: Medium risk (e.g., creating new files)
    - HIGH: High risk (e.g., modifying existing files)
    - CRITICAL: Critical risk (e.g., deleting files)

Quick Start:
    >>> from continuum_sdk.security import ChangePreviewer, ChangeType
    >>>
    >>> previewer = ChangePreviewer()
    >>>
    >>> # Preview change
    >>> change = previewer.create_change(
    ...     change_type=ChangeType.WRITE,
    ...     path="/project/src/main.py",
    ...     content="new content"
    ... )
    >>>
    >>> # Show preview
    >>> print(previewer.preview(change))
    >>>
    >>> # Request confirmation
    >>> if previewer.confirm(change):
    ...     # Execute operation
    ...     pass

Diff Preview:
    >>> # Show file diff
    >>> diff = previewer.diff(
    ...     old_content="original",
    ...     new_content="modified"
    ... )
    >>> print(diff)

Batch Confirmation:
    >>> # Batch operation confirmation
    >>> changes = [
    ...     Change(ChangeType.WRITE, "/file1.py", "content1"),
    ...     Change(ChangeType.DELETE, "/file2.py"),
    ... ]
    >>> approved = previewer.confirm_batch(changes)
"""

from __future__ import annotations

import difflib
import logging
import os
import threading
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Callable

logger = logging.getLogger(__name__)


class ChangeType(Enum):
    """Change types"""

    CREATE = "create"  # Create new file
    WRITE = "write"  # Write/overwrite file
    EDIT = "edit"  # Edit file (precise replacement)
    DELETE = "delete"  # Delete file
    MOVE = "move"  # Move file
    COPY = "copy"  # Copy file
    RENAME = "rename"  # Rename file
    READ = "read"  # Read file (low risk)
    LIST = "list"  # List directory (low risk)
    APPEND = "append"  # Append content


class RiskLevel(Enum):
    """Risk levels"""

    LOW = "low"  # Low risk
    MEDIUM = "medium"  # Medium risk
    HIGH = "high"  # High risk
    CRITICAL = "critical"  # Critical risk


@dataclass
class Change:
    """Change description

    Attributes:
        change_type: Change type
        path: Target path
        content: New content (if applicable)
        old_content: Original content (if available)
        source_path: Source path (MOVE/COPY)
        metadata: Additional information
        risk_level: Risk level
        reason: Change reason
    """

    change_type: ChangeType
    path: str
    content: str | bytes | None = None
    old_content: str | bytes | None = None
    source_path: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    risk_level: RiskLevel | None = None
    reason: str | None = None

    def __post_init__(self):
        # Automatically assess risk level
        if self.risk_level is None:
            self.risk_level = self._assess_risk()

    def _assess_risk(self) -> RiskLevel:
        """Assess risk level"""
        path = Path(self.path)

        # Check if file exists
        exists = path.exists()

        # Check if sensitive file
        sensitive_patterns = [
            ".env", ".git", ".ssh", "config", "secret", "key", "credential"
        ]
        is_sensitive = any(p in self.path.lower() for p in sensitive_patterns)

        # Check change type
        if self.change_type == ChangeType.DELETE:
            return RiskLevel.CRITICAL

        if self.change_type in (ChangeType.MOVE, ChangeType.RENAME):
            return RiskLevel.HIGH if exists else RiskLevel.MEDIUM

        if self.change_type in (ChangeType.WRITE, ChangeType.EDIT):
            if is_sensitive:
                return RiskLevel.CRITICAL
            return RiskLevel.HIGH if exists else RiskLevel.MEDIUM

        if self.change_type == ChangeType.CREATE:
            return RiskLevel.MEDIUM

        if self.change_type == ChangeType.COPY:
            return RiskLevel.LOW

        if self.change_type in (ChangeType.APPEND, ChangeType.READ):
            return RiskLevel.LOW

        return RiskLevel.MEDIUM

    def to_dict(self) -> dict[str, Any]:
        return {
            "change_type": self.change_type.value,
            "path": self.path,
            "content_preview": self._content_preview(),
            "risk_level": self.risk_level.value if self.risk_level else None,
            "reason": self.reason,
            "metadata": self.metadata,
        }

    def _content_preview(self) -> str:
        """Content preview"""
        if self.content is None:
            return "None"
        if isinstance(self.content, bytes):
            return f"<binary: {len(self.content)} bytes>"
        preview = self.content[:100]
        if len(self.content) > 100:
            preview += "..."
        return preview


@dataclass
class ConfirmationResult:
    """Confirmation result

    Attributes:
        approved: Whether approved
        change: Change description
        reason: Reason
        timestamp: Timestamp
        user_response: User response
    """

    approved: bool
    change: Change
    reason: str
    timestamp: datetime = field(default_factory=datetime.now)
    user_response: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "approved": self.approved,
            "change": self.change.to_dict(),
            "reason": self.reason,
            "timestamp": self.timestamp.isoformat(),
            "user_response": self.user_response,
        }


class ChangePreviewer:
    """Change previewer

    Example:
        >>> previewer = ChangePreviewer()
        >>> change = previewer.create_change(ChangeType.WRITE, "/file.py", "content")
        >>> if previewer.confirm(change):
        ...     with open("/file.py", "w") as f:
        ...         f.write("content")
    """

    # Default skip confirmation operation types
    DEFAULT_SKIP_TYPES = {ChangeType.READ, ChangeType.LIST}

    # Default skip confirmation risk levels
    DEFAULT_SKIP_RISK = {RiskLevel.LOW}

    def __init__(
        self,
        auto_confirm_low: bool = True,
        require_confirmation: set[RiskLevel] | None = None,
        skip_types: set[ChangeType] | None = None,
        custom_confirmer: Callable[[Change], bool] | None = None,
        project_root: str | Path | None = None,
    ):
        """Initialize change previewer

        Args:
            auto_confirm_low: Auto-confirm low risk operations
            require_confirmation: Risk levels requiring confirmation
            skip_types: Change types to skip confirmation
            custom_confirmer: Custom confirmation function
            project_root: Project root directory
        """
        self._auto_confirm_low = auto_confirm_low
        self._require_confirmation = require_confirmation or {
            RiskLevel.MEDIUM,
            RiskLevel.HIGH,
            RiskLevel.CRITICAL,
        }
        self._skip_types = skip_types or self.DEFAULT_SKIP_TYPES
        self._custom_confirmer = custom_confirmer
        self._project_root = Path(project_root or os.getcwd()).resolve()

        self._confirmation_history: list[ConfirmationResult] = []
        self._lock = threading.RLock()

    def create_change(
        self,
        change_type: ChangeType,
        path: str | Path,
        content: str | bytes | None = None,
        old_content: str | bytes | None = None,
        source_path: str | Path | None = None,
        reason: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> Change:
        """Create change description

        Args:
            change_type: Change type
            path: Target path
            content: New content
            old_content: Original content
            source_path: Source path
            reason: Change reason
            metadata: Additional information

        Returns:
            Change object
        """
        # Read original content (if needed and not provided)
        if old_content is None and change_type in (
            ChangeType.EDIT,
            ChangeType.WRITE,
        ):
            path_obj = Path(path)
            if path_obj.exists():
                try:
                    old_content = path_obj.read_text(encoding="utf-8")
                except (OSError, IOError, PermissionError, UnicodeDecodeError) as e:
                    # Silent handling on read failure, old_content stays None
                    # diff will show as "new content" instead of "modified"
                    logger.debug(f"Failed to read old content for {path}: {e}")

        return Change(
            change_type=change_type,
            path=str(path),
            content=content,
            old_content=old_content,
            source_path=str(source_path) if source_path else None,
            reason=reason,
            metadata=metadata or {},
        )

    def preview(self, change: Change) -> str:
        """Generate change preview

        Args:
            change: Change description

        Returns:
            Preview text
        """
        lines = []
        lines.append(f"=== Change Preview ===")
        lines.append(f"Type: {change.change_type.value}")
        lines.append(f"Path: {change.path}")
        lines.append(f"Risk: {change.risk_level.value if change.risk_level else 'unknown'}")

        if change.reason:
            lines.append(f"Reason: {change.reason}")

        # Show diff
        if change.change_type in (ChangeType.WRITE, ChangeType.EDIT):
            diff = self.diff(
                change.old_content or "",
                change.content or "",
                change.path,
            )
            lines.append("")
            lines.append("Diff:")
            lines.append(diff)

        elif change.change_type == ChangeType.CREATE:
            lines.append("")
            lines.append("New content:")
            if change.content:
                if isinstance(change.content, bytes):
                    lines.append(f"<binary: {len(change.content)} bytes>")
                else:
                    preview = change.content[:500]
                    if len(change.content) > 500:
                        preview += "..."
                    lines.append(preview)

        elif change.change_type == ChangeType.DELETE:
            lines.append("")
            lines.append("Warning: This will DELETE the file!")
            if change.old_content:
                lines.append(f"File size: {len(change.old_content)} characters")

        elif change.change_type in (ChangeType.MOVE, ChangeType.COPY):
            lines.append(f"Source: {change.source_path}")

        lines.append("")
        lines.append(f"=== End Preview ===")

        return "\n".join(lines)

    def diff(
        self,
        old_content: str | bytes | None,
        new_content: str | bytes | None,
        path: str | None = None,
    ) -> str:
        """Generate diff display

        Args:
            old_content: Original content
            new_content: New content
            path: File path (for display)

        Returns:
            Diff text
        """
        if old_content is None:
            old_content = ""
        if new_content is None:
            new_content = ""

        if isinstance(old_content, bytes) or isinstance(new_content, bytes):
            return "<binary content - cannot diff>"

        old_lines = old_content.splitlines(keepends=True)
        new_lines = new_content.splitlines(keepends=True)

        diff_lines = list(
            difflib.unified_diff(
                old_lines,
                new_lines,
                fromfile=f"{path} (original)" if path else "original",
                tofile=f"{path} (modified)" if path else "modified",
            )
        )

        return "".join(diff_lines)

    def confirm(
        self,
        change: Change,
        force_confirm: bool = False,
        auto_approve: bool = False,
    ) -> bool:
        """Request change confirmation

        Args:
            change: Change description
            force_confirm: Force confirmation
            auto_approve: Auto-approve (skip confirmation)

        Returns:
            Whether approved
        """
        # Auto-approve
        if auto_approve:
            result = ConfirmationResult(
                approved=True,
                change=change,
                reason="Auto-approved",
            )
            self._record_result(result)
            return True

        # Check if skip
        if not force_confirm:
            if change.change_type in self._skip_types:
                result = ConfirmationResult(
                    approved=True,
                    change=change,
                    reason="Skipped (change type in skip list)",
                )
                self._record_result(result)
                return True

            if change.risk_level and change.risk_level not in self._require_confirmation:
                result = ConfirmationResult(
                    approved=True,
                    change=change,
                    reason=f"Skipped (risk level: {change.risk_level.value})",
                )
                self._record_result(result)
                return True

        # Use custom confirmer
        if self._custom_confirmer:
            try:
                approved = self._custom_confirmer(change)
                result = ConfirmationResult(
                    approved=approved,
                    change=change,
                    reason="Custom confirmer",
                )
                self._record_result(result)
                return approved
            except (TypeError, ValueError, RuntimeError) as e:
                logger.warning(f"Custom confirmer failed: {e}")

        # Default interactive confirmation (returns False in non-interactive environment)
        result = self._interactive_confirm(change)
        self._record_result(result)
        return result.approved

    def confirm_batch(
        self,
        changes: list[Change],
        show_preview: bool = True,
    ) -> list[ConfirmationResult]:
        """Batch confirm changes

        Args:
            changes: Change list
            show_preview: Whether to show preview

        Returns:
            Confirmation result list
        """
        results = []

        # Show batch preview
        if show_preview:
            print("\n=== Batch Change Preview ===")
            print(f"Total changes: {len(changes)}")

            # Group by risk level
            by_risk: dict[RiskLevel, list[Change]] = {}
            for c in changes:
                level = c.risk_level or RiskLevel.MEDIUM
                if level not in by_risk:
                    by_risk[level] = []
                by_risk[level].append(c)

            for level in [RiskLevel.CRITICAL, RiskLevel.HIGH, RiskLevel.MEDIUM, RiskLevel.LOW]:
                if level in by_risk:
                    print(f"\n{level.value.upper()} risk ({len(by_risk[level])} changes):")
                    for c in by_risk[level]:
                        print(f"  - {c.change_type.value}: {c.path}")

        # Confirm individually
        for change in changes:
            result = ConfirmationResult(
                approved=self.confirm(change),
                change=change,
                reason="Individual confirmation",
            )
            results.append(result)
            self._record_result(result)

        return results

    def _interactive_confirm(self, change: Change) -> ConfirmationResult:
        """Interactive confirmation

        Args:
            change: Change description

        Returns:
            Confirmation result
        """
        # Show preview
        print("\n" + self.preview(change))

        # Prompt for confirmation
        print(f"\nRisk level: {change.risk_level.value if change.risk_level else 'unknown'}")
        print("Approve this change? [y/N/a(vendor)/q(uit)]")

        try:
            response = input("> ").strip().lower()
        except (EOFError, KeyboardInterrupt):
            return ConfirmationResult(
                approved=False,
                change=change,
                reason="User interrupted",
                user_response="interrupt",
            )

        # Parse response
        if response in ("y", "yes"):
            return ConfirmationResult(
                approved=True,
                change=change,
                reason="User approved",
                user_response=response,
            )
        elif response == "a":
            # Approve all (auto-confirm subsequent)
            self._require_confirmation = set()  # Clear items requiring confirmation
            return ConfirmationResult(
                approved=True,
                change=change,
                reason="User approved all",
                user_response=response,
            )
        elif response == "q":
            # Quit
            raise KeyboardInterrupt("User requested quit")
        else:
            return ConfirmationResult(
                approved=False,
                change=change,
                reason="User rejected",
                user_response=response,
            )

    def _record_result(self, result: ConfirmationResult) -> None:
        """Record confirmation result"""
        with self._lock:
            self._confirmation_history.append(result)

    def get_history(self) -> list[ConfirmationResult]:
        """Get confirmation history

        Returns:
            Confirmation result list
        """
        with self._lock:
            return list(self._confirmation_history)

    def get_statistics(self) -> dict[str, Any]:
        """Get statistics

        Returns:
            Statistics dictionary
        """
        with self._lock:
            total = len(self._confirmation_history)
            if total == 0:
                return {"total": 0}

            approved = sum(1 for r in self._confirmation_history if r.approved)
            rejected = total - approved

            by_risk: dict[str, int] = {}
            for r in self._confirmation_history:
                level = r.change.risk_level.value if r.change.risk_level else "unknown"
                by_risk[level] = by_risk.get(level, 0) + 1

            return {
                "total": total,
                "approved": approved,
                "rejected": rejected,
                "approval_rate": approved / total if total > 0 else 0,
                "by_risk_level": by_risk,
            }

    def set_custom_confirmer(self, confirmer: Callable[[Change], bool]) -> None:
        """Set custom confirmation function

        Args:
            confirmer: Confirmation function
        """
        self._custom_confirmer = confirmer

    def reset_settings(self) -> None:
        """Reset settings to default"""
        self._require_confirmation = {
            RiskLevel.MEDIUM,
            RiskLevel.HIGH,
            RiskLevel.CRITICAL,
        }
        self._skip_types = self.DEFAULT_SKIP_TYPES
        self._custom_confirmer = None

    def __repr__(self) -> str:
        return f"ChangePreviewer(project_root={self._project_root})"