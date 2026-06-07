"""Path Validator - Path Validation and Security Boundary Checking

Ensures all file operations are within project directory, preventing unauthorized access.

Features:
    - Project boundary checking: Prevent access to files outside project directory
    - Path normalization: Handle symlinks, relative paths, absolute paths
    - Dangerous path detection: Identify sensitive system directories
    - Configurable allow list: Whitelist specific directories
    - Path traversal attack protection: Prevent ../../../ etc. path traversal

Security Rules:
    1. All paths must be within project root directory
    2. Symlinks must remain within boundary after resolution
    3. Prevent access to system sensitive directories (/etc, ~/.ssh, etc)
    4. Relative paths are automatically converted to absolute paths for checking

Quick Start:
    >>> from continuum_sdk.security import PathValidator
    >>>
    >>> validator = PathValidator(project_root="/home/user/project")
    >>>
    >>> # Validate path
    >>> result = validator.validate("/home/user/project/src/file.py")
    >>> print(result.is_valid)  # True
    >>>
    >>> # Detect out-of-bound path
    >>> result = validator.validate("/etc/passwd")
    >>> print(result.is_valid)  # False
    >>> print(result.reason)    # "Path outside project boundary"

Dangerous Path Examples:
    >>> # Path traversal attack
    >>> validator.validate("../../../etc/passwd")
    >>> # Symlink escape
    >>> validator.validate("/home/user/project/link_to_secret")

Configuration:
    >>> validator = PathValidator(
    ...     project_root="/project",
    ...     allowed_paths=["/tmp/cache"],  # Whitelist
    ...     denied_paths=[".env", ".git"],  # Blacklist
    ...     follow_symlinks=True
    ... )
"""

from __future__ import annotations

import logging
import os
import re
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class PathValidationResult(Enum):
    """Path validation result types"""

    VALID = "valid"  # Path is valid and safe
    OUT_OF_BOUND = "out_of_bound"  # Outside project boundary
    DENIED_PATH = "denied_path"  # In blacklist
    SYMLINK_ESCAPE = "symlink_escape"  # Symlink escape
    DANGEROUS_PATH = "dangerous_path"  # System sensitive directory
    INVALID_PATH = "invalid_path"  # Invalid path format
    NOT_ALLOWED = "not_allowed"  # Not in allow list


@dataclass
class ValidationResult:
    """Validation result

    Attributes:
        is_valid: Whether valid
        result_type: Result type
        reason: Detailed reason
        original_path: Original path
        resolved_path: Resolved absolute path
        metadata: Additional information
    """

    is_valid: bool
    result_type: PathValidationResult
    reason: str
    original_path: str
    resolved_path: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "is_valid": self.is_valid,
            "result_type": self.result_type.value,
            "reason": self.reason,
            "original_path": self.original_path,
            "resolved_path": self.resolved_path,
            "metadata": self.metadata,
        }


class PathValidator:
    """Path validator

    Ensures all file operations are within security boundary.

    Example:
        >>> validator = PathValidator(project_root="/home/user/project")
        >>> result = validator.validate("src/main.py")
        >>> if not result.is_valid:
        ...     raise SecurityError(result.reason)
    """

    # System sensitive directories (prohibited access)
    DANGEROUS_DIRS = {
        # Unix/Linux
        "/etc",
        "/root",
        "/var/log",
        "/var/run",
        "/proc",
        "/sys",
        "/dev",
        "/boot",
        "/usr/bin",
        "/usr/sbin",
        "/bin",
        "/sbin",
        # Windows
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\Program Files (x86)",
        "C:\\System32",
        # Home sensitive directories
        ".ssh",
        ".gnupg",
        ".password-store",
        ".config/ssh",
    }

    # Sensitive file patterns
    SENSITIVE_PATTERNS = [
        r"\.env$",
        r"\.env\.",
        r"id_rsa",
        r"id_ed25519",
        r"\.pem$",
        r"\.key$",
        r"\.p12$",
        r"credentials",
        r"secrets",
        r"\.htpasswd",
        r"passwd$",
        r"shadow$",
    ]

    def __init__(
        self,
        project_root: str | Path | None = None,
        allowed_paths: list[str] | None = None,
        denied_paths: list[str] | None = None,
        follow_symlinks: bool = True,
        allow_sensitive_files: bool = False,
    ):
        """Initialize path validator

        Args:
            project_root: Project root directory (default current working directory)
            allowed_paths: Whitelist paths (directories outside project allowed for access)
            denied_paths: Blacklist paths (directories inside project prohibited for access)
            follow_symlinks: Whether to resolve symlinks
            allow_sensitive_files: Whether to allow access to sensitive files
        """
        self._project_root = Path(project_root or os.getcwd()).resolve()
        self._allowed_paths = [Path(p).resolve() for p in (allowed_paths or [])]
        self._denied_paths = [Path(p) for p in (denied_paths or [])]
        self._follow_symlinks = follow_symlinks
        self._allow_sensitive_files = allow_sensitive_files

        # Ensure project root exists
        if not self._project_root.exists():
            logger.warning(
                f"Project root does not exist: {self._project_root}"
            )

    @property
    def project_root(self) -> Path:
        """Get project root directory"""
        return self._project_root

    def validate(self, path: str | Path) -> ValidationResult:
        """Validate if path is safe

        Args:
            path: Path to validate

        Returns:
            ValidationResult containing validation result and reason
        """
        original_path = str(path)

        # 1. Path format check
        try:
            path_obj = Path(path)
        except (ValueError, TypeError, OSError) as e:
            return ValidationResult(
                is_valid=False,
                result_type=PathValidationResult.INVALID_PATH,
                reason=f"Invalid path format: {e}",
                original_path=original_path,
            )

        # 2. Resolve absolute path
        if not path_obj.is_absolute():
            # Relative to project root
            path_obj = self._project_root / path_obj

        try:
            if self._follow_symlinks:
                resolved = path_obj.resolve()
            else:
                resolved = path_obj.absolute()
        except (OSError, IOError, RuntimeError) as e:
            return ValidationResult(
                is_valid=False,
                result_type=PathValidationResult.INVALID_PATH,
                reason=f"Cannot resolve path: {e}",
                original_path=original_path,
            )

        resolved_str = str(resolved)

        # 3. Check blacklist paths
        for denied in self._denied_paths:
            # Blacklist can be relative or absolute path
            denied_abs = denied if denied.is_absolute() else (self._project_root / denied)
            if self._path_contains(resolved, denied_abs):
                return ValidationResult(
                    is_valid=False,
                    result_type=PathValidationResult.DENIED_PATH,
                    reason=f"Path matches denied pattern: {denied}",
                    original_path=original_path,
                    resolved_path=resolved_str,
                )

        # 4. Check sensitive files
        if not self._allow_sensitive_files:
            for pattern in self.SENSITIVE_PATTERNS:
                if re.search(pattern, resolved_str, re.IGNORECASE):
                    return ValidationResult(
                        is_valid=False,
                        result_type=PathValidationResult.DANGEROUS_PATH,
                        reason=f"Path matches sensitive file pattern: {pattern}",
                        original_path=original_path,
                        resolved_path=resolved_str,
                        metadata={"pattern": pattern},
                    )

        # 5. Check system sensitive directories
        for dangerous in self.DANGEROUS_DIRS:
            dangerous_path = Path(dangerous)
            if self._path_contains(resolved, dangerous_path):
                return ValidationResult(
                    is_valid=False,
                    result_type=PathValidationResult.DANGEROUS_PATH,
                    reason=f"Path in system sensitive directory: {dangerous}",
                    original_path=original_path,
                    resolved_path=resolved_str,
                )

        # 6. Check if within project boundary
        in_project = self._path_contains(resolved, self._project_root)

        if in_project:
            return ValidationResult(
                is_valid=True,
                result_type=PathValidationResult.VALID,
                reason="Path is within project boundary",
                original_path=original_path,
                resolved_path=resolved_str,
            )

        # 7. Check whitelist
        for allowed in self._allowed_paths:
            if self._path_contains(resolved, allowed):
                return ValidationResult(
                    is_valid=True,
                    result_type=PathValidationResult.VALID,
                    reason="Path is in allowed list",
                    original_path=original_path,
                    resolved_path=resolved_str,
                    metadata={"allowed_path": str(allowed)},
                )

        # 8. Out of boundary
        return ValidationResult(
            is_valid=False,
            result_type=PathValidationResult.OUT_OF_BOUND,
            reason=f"Path outside project boundary: {self._project_root}",
            original_path=original_path,
            resolved_path=resolved_str,
        )

    def validate_batch(self, paths: list[str | Path]) -> list[ValidationResult]:
        """Batch validate paths

        Args:
            paths: Path list

        Returns:
            Validation result list
        """
        return [self.validate(p) for p in paths]

    def is_valid(self, path: str | Path) -> bool:
        """Quickly check if path is valid

        Args:
            path: Path to check

        Returns:
            Whether valid
        """
        return self.validate(path).is_valid

    def get_safe_path(self, path: str | Path) -> Path | None:
        """Get safe resolved path

        Args:
            path: Original path

        Returns:
            Resolved path (if valid), otherwise None
        """
        result = self.validate(path)
        if result.is_valid and result.resolved_path:
            return Path(result.resolved_path)
        return None

    def _path_contains(self, child: Path, parent: Path) -> bool:
        """Check if path is contained within another path

        Args:
            child: Child path
            parent: Parent path

        Returns:
            Whether contained
        """
        try:
            # Windows path comparison needs case handling
            if os.name == "nt":
                child_str = str(child).lower()
                parent_str = str(parent).lower()
            else:
                child_str = str(child)
                parent_str = str(parent)

            parent_str_bare = parent_str.rstrip(os.sep)

            # Ensure parent has trailing separator to avoid partial match
            # Note: str(Path(...)) never ends with separator, so this branch is always True
            if not parent_str.endswith(os.sep):  # pragma: no branch
                parent_str += os.sep

            return child_str.startswith(parent_str) or child_str == parent_str_bare
        except (OSError, IOError, ValueError):  # pragma: no cover - defensive code
            return False

    def add_allowed_path(self, path: str | Path) -> None:
        """Add whitelist path

        Args:
            path: Path allowed for access
        """
        resolved = Path(path).resolve()
        if resolved not in self._allowed_paths:
            self._allowed_paths.append(resolved)
            logger.info(f"Added allowed path: {resolved}")

    def add_denied_path(self, path: str | Path) -> None:
        """Add blacklist path

        Args:
            path: Path prohibited for access
        """
        denied = Path(path)
        if denied not in self._denied_paths:
            self._denied_paths.append(denied)
            logger.info(f"Added denied path: {denied}")

    def get_config(self) -> dict[str, Any]:
        """Get current configuration

        Returns:
            Configuration dictionary
        """
        return {
            "project_root": str(self._project_root),
            "allowed_paths": [str(p) for p in self._allowed_paths],
            "denied_paths": [str(p) for p in self._denied_paths],
            "follow_symlinks": self._follow_symlinks,
            "allow_sensitive_files": self._allow_sensitive_files,
        }

    def __repr__(self) -> str:
        return f"PathValidator(project_root={self._project_root})"