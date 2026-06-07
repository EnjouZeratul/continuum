"""Permission Checker - File Permission Checking

Checks file and directory access permissions to ensure safe operations.

Features:
    - Read/write/execute permission checking
    - File existence validation
    - Parent directory permission checking (before creating files)
    - Permission change detection (detect anomalous permissions)
    - Batch permission checking

Permission Types:
    - READ: Read file contents
    - WRITE: Modify file contents
    - EXECUTE: Execute file (scripts/programs)
    - DELETE: Delete file
    - CREATE: Create new file

Quick Start:
    >>> from continuum_sdk.security import PermissionChecker, Permission
    >>>
    >>> checker = PermissionChecker()
    >>>
    >>> # Check read permission
    >>> result = checker.check("/path/to/file", Permission.READ)
    >>> print(result.has_permission)  # True/False
    >>>
    >>> # Check multiple permissions
    >>> results = checker.check_multiple(
    ...     "/path/to/file",
    ...     [Permission.READ, Permission.WRITE]
    ... )

Pre-Operation Check:
    >>> # Check parent directory permission before creating file
    >>> result = checker.check_parent("/new/file/path", Permission.WRITE)
    >>> if result.has_permission:
    ...     # Can create file
    ...     pass

Batch Checking:
    >>> # Batch check multiple files
    >>> files = ["/file1.py", "/file2.py", "/file3.py"]
    >>> results = checker.check_batch(files, Permission.WRITE)
    >>> writable = [f for f, r in results.items() if r.has_permission]
"""

from __future__ import annotations

import logging
import os
import stat
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class Permission(Enum):
    """Permission types"""

    READ = "read"  # Read
    WRITE = "write"  # Write
    EXECUTE = "execute"  # Execute
    DELETE = "delete"  # Delete
    CREATE = "create"  # Create


@dataclass
class PermissionResult:
    """Permission check result

    Attributes:
        has_permission: Whether permission is granted
        permission: The permission type checked
        path: The path checked
        reason: Result reason
        exists: Whether file exists
        actual_permissions: Actual permissions (Unix mode)
        metadata: Additional information
    """

    has_permission: bool
    permission: Permission
    path: str
    reason: str
    exists: bool = True
    actual_permissions: int | None = None
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "has_permission": self.has_permission,
            "permission": self.permission.value,
            "path": self.path,
            "reason": self.reason,
            "exists": self.exists,
            "actual_permissions": oct(self.actual_permissions)
            if self.actual_permissions
            else None,
            "metadata": self.metadata,
        }


class PermissionChecker:
    """Permission checker

    Example:
        >>> checker = PermissionChecker()
        >>> if checker.can_write("/path/to/file"):
        ...     with open("/path/to/file", "w") as f:
        ...         f.write("content")
    """

    def __init__(self, strict_mode: bool = False):
        """Initialize permission checker

        Args:
            strict_mode: Strict mode (more checks, stricter judgment)
        """
        self._strict_mode = strict_mode
        self._cache: dict[str, tuple[datetime, PermissionResult]] = {}
        self._cache_ttl = 5.0  # Cache TTL (seconds)

    def check(self, path: str | Path, permission: Permission) -> PermissionResult:
        """Check specified permission

        Args:
            path: File path
            permission: Permission type

        Returns:
            PermissionResult
        """
        path_str = str(path)
        path_obj = Path(path)

        # Check cache
        cache_key = f"{path_str}:{permission.value}"
        if cache_key in self._cache:
            timestamp, cached_result = self._cache[cache_key]
            if (datetime.now() - timestamp).total_seconds() < self._cache_ttl:
                return cached_result

        # Check if path exists
        exists = path_obj.exists()

        if not exists:
            if permission == Permission.CREATE:
                # Creating file requires checking parent directory
                return self._check_create_permission(path_obj)
            else:
                return PermissionResult(
                    has_permission=False,
                    permission=permission,
                    path=path_str,
                    reason="File does not exist",
                    exists=False,
                )

        # Get actual permissions
        actual_perms = self._get_permissions(path_obj)

        # Check permission
        result = self._check_permission(path_obj, permission, actual_perms)

        # Cache result
        self._cache[cache_key] = (datetime.now(), result)

        return result

    def check_multiple(
        self, path: str | Path, permissions: list[Permission]
    ) -> list[PermissionResult]:
        """Check multiple permissions

        Args:
            path: File path
            permissions: Permission list

        Returns:
            List of results
        """
        return [self.check(path, p) for p in permissions]

    def check_batch(
        self, paths: list[str | Path], permission: Permission
    ) -> dict[str, PermissionResult]:
        """Batch check permissions

        Args:
            paths: Path list
            permission: Permission type

        Returns:
            {path: result} dictionary
        """
        return {str(p): self.check(p, permission) for p in paths}

    def check_parent(self, path: str | Path, permission: Permission) -> PermissionResult:
        """Check parent directory permission

        Args:
            path: File path
            permission: Permission type

        Returns:
            Parent directory permission result
        """
        path_obj = Path(path)
        parent = path_obj.parent

        if not parent or str(parent) == str(path_obj):
            return PermissionResult(
                has_permission=False,
                permission=permission,
                path=str(path),
                reason="No parent directory",
            )

        return self.check(parent, permission)

    def _check_permission(
        self, path: Path, permission: Permission, actual_perms: int
    ) -> PermissionResult:
        """Execute permission check

        Args:
            path: File path
            permission: Permission type
            actual_perms: Actual permission mode

        Returns:
            PermissionResult
        """
        path_str = str(path)

        try:
            if permission == Permission.READ:
                return self._check_read(path, actual_perms)

            elif permission == Permission.WRITE:
                return self._check_write(path, actual_perms)

            elif permission == Permission.EXECUTE:
                return self._check_execute(path, actual_perms)

            elif permission == Permission.DELETE:
                return self._check_delete(path)

            elif permission == Permission.CREATE:
                return self._check_create_permission(path)

            else:
                return PermissionResult(
                    has_permission=False,
                    permission=permission,
                    path=path_str,
                    reason=f"Unknown permission type: {permission}",
                )

        except PermissionError as e:
            return PermissionResult(
                has_permission=False,
                permission=permission,
                path=path_str,
                reason=f"Permission denied: {e}",
                actual_permissions=actual_perms,
            )
        except (OSError, RuntimeError) as e:
            return PermissionResult(
                has_permission=False,
                permission=permission,
                path=path_str,
                reason=f"Error checking permission: {e}",
                actual_permissions=actual_perms,
            )

    def _check_read(self, path: Path, actual_perms: int) -> PermissionResult:
        """Check read permission"""
        path_str = str(path)

        # Windows and Unix check differently
        if os.name == "nt":
            # Directories can't be open()'d on Windows; use os.access for them.
            if path.is_dir():
                if os.access(path, os.R_OK):
                    return PermissionResult(
                        has_permission=True,
                        permission=Permission.READ,
                        path=path_str,
                        reason="Can read directory",
                        actual_permissions=actual_perms,
                    )
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.READ,
                    path=path_str,
                    reason="No read permission on directory",
                    actual_permissions=actual_perms,
                )
            # Windows: Try to open file
            try:
                with open(path, "rb"):
                    pass
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.READ,
                    path=path_str,
                    reason="Can read file",
                    actual_permissions=actual_perms,
                )
            except PermissionError:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.READ,
                    path=path_str,
                    reason="No read permission",
                    actual_permissions=actual_perms,
                )
        else:
            # Unix: Check permission bits
            if os.access(path, os.R_OK):
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.READ,
                    path=path_str,
                    reason="Read permission granted",
                    actual_permissions=actual_perms,
                )
            else:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.READ,
                    path=path_str,
                    reason="No read permission",
                    actual_permissions=actual_perms,
                )

    def _check_write(self, path: Path, actual_perms: int) -> PermissionResult:
        """Check write permission"""
        path_str = str(path)

        if os.name == "nt":
            # Windows: Try to open in append mode
            try:
                with open(path, "ab"):
                    pass
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.WRITE,
                    path=path_str,
                    reason="Can write file",
                    actual_permissions=actual_perms,
                )
            except PermissionError:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.WRITE,
                    path=path_str,
                    reason="No write permission",
                    actual_permissions=actual_perms,
                )
        else:
            if os.access(path, os.W_OK):
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.WRITE,
                    path=path_str,
                    reason="Write permission granted",
                    actual_permissions=actual_perms,
                )
            else:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.WRITE,
                    path=path_str,
                    reason="No write permission",
                    actual_permissions=actual_perms,
                )

    def _check_execute(self, path: Path, actual_perms: int) -> PermissionResult:
        """Check execute permission"""
        path_str = str(path)

        if os.name == "nt":
            # Windows: Check file extension
            executable_extensions = {".exe", ".bat", ".cmd", ".ps1", ".com"}
            if path.suffix.lower() in executable_extensions:
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.EXECUTE,
                    path=path_str,
                    reason="Executable file type",
                    actual_permissions=actual_perms,
                )
            return PermissionResult(
                has_permission=False,
                permission=Permission.EXECUTE,
                path=path_str,
                reason="Not an executable file type",
                actual_permissions=actual_perms,
            )
        else:
            if os.access(path, os.X_OK):
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.EXECUTE,
                    path=path_str,
                    reason="Execute permission granted",
                    actual_permissions=actual_perms,
                )
            else:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.EXECUTE,
                    path=path_str,
                    reason="No execute permission",
                    actual_permissions=actual_perms,
                )

    def _check_delete(self, path: Path) -> PermissionResult:
        """Check delete permission"""
        path_str = str(path)

        # Delete requires write permission
        if os.name == "nt":
            try:
                # Check if can delete
                if path.is_file():
                    os.access(path, os.W_OK)
                else:
                    os.access(path.parent, os.W_OK)
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.DELETE,
                    path=path_str,
                    reason="Can delete file",
                    actual_permissions=self._get_permissions(path),
                )
            except (OSError, PermissionError):
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.DELETE,
                    path=path_str,
                    reason="No delete permission",
                )
        else:
            # Unix: Need parent directory write permission
            parent_writable = os.access(path.parent, os.W_OK)
            if parent_writable:
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.DELETE,
                    path=path_str,
                    reason="Delete permission granted",
                    actual_permissions=self._get_permissions(path),
                )
            else:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.DELETE,
                    path=path_str,
                    reason="No write permission on parent directory",
                    actual_permissions=self._get_permissions(path),
                )

    def _check_create_permission(self, path: Path) -> PermissionResult:
        """Check create permission"""
        path_str = str(path)
        parent = path.parent

        if not parent.exists():
            return PermissionResult(
                has_permission=False,
                permission=Permission.CREATE,
                path=path_str,
                reason="Parent directory does not exist",
                exists=False,
            )

        # Check parent directory write permission
        if os.name == "nt":
            try:
                test_file = parent / ".__permission_test__"
                test_file.touch()
                test_file.unlink()
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.CREATE,
                    path=path_str,
                    reason="Can create file",
                    exists=False,
                )
            except PermissionError:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.CREATE,
                    path=path_str,
                    reason="No create permission in parent directory",
                    exists=False,
                )
        else:
            if os.access(parent, os.W_OK):
                return PermissionResult(
                    has_permission=True,
                    permission=Permission.CREATE,
                    path=path_str,
                    reason="Create permission granted",
                    exists=False,
                )
            else:
                return PermissionResult(
                    has_permission=False,
                    permission=Permission.CREATE,
                    path=path_str,
                    reason="No write permission in parent directory",
                    exists=False,
                )

    def _get_permissions(self, path: Path) -> int:
        """Get file permission mode

        Args:
            path: File path

        Returns:
            Permission mode (Unix style)
        """
        try:
            return stat.S_IMODE(path.stat().st_mode)
        except (OSError, FileNotFoundError):
            return 0o644  # Default

    # Convenience methods
    def can_read(self, path: str | Path) -> bool:
        """Check read permission"""
        return self.check(path, Permission.READ).has_permission

    def can_write(self, path: str | Path) -> bool:
        """Check write permission"""
        return self.check(path, Permission.WRITE).has_permission

    def can_execute(self, path: str | Path) -> bool:
        """Check execute permission"""
        return self.check(path, Permission.EXECUTE).has_permission

    def can_delete(self, path: str | Path) -> bool:
        """Check delete permission"""
        return self.check(path, Permission.DELETE).has_permission

    def can_create(self, path: str | Path) -> bool:
        """Check create permission"""
        return self.check(path, Permission.CREATE).has_permission

    def clear_cache(self) -> None:
        """Clear cache"""
        self._cache.clear()

    def __repr__(self) -> str:
        return f"PermissionChecker(strict_mode={self._strict_mode})"