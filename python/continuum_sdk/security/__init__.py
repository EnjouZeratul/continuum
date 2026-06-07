"""Continuum SDK Security Module

Security module providing file operation safeguards.

Features:
    - Path validation: Prevent access to directories outside project
    - Permission checking: Read/write/execute permission verification
    - Audit logging: Operation history recording
    - Change preview: High-risk operation confirmation

Components:
    - PathValidator: Path boundary validation
    - PermissionChecker: Permission checking
    - AuditLogger: Audit logging
    - ChangePreviewer: Change preview and confirmation

Quick Start:
    >>> from continuum_sdk.security import (
    ...     PathValidator,
    ...     PermissionChecker,
    ...     AuditLogger,
    ...     ChangePreviewer,
    ...     Permission,
    ...     AuditOperation,
    ...     ChangeType,
    ... )
    >>>
    >>> # Path validation
    >>> validator = PathValidator(project_root="/project")
    >>> if validator.is_valid("src/main.py"):
    ...     # Safe operation
    ...     pass
    >>>
    >>> # Permission checking
    >>> checker = PermissionChecker()
    >>> if checker.can_write("/project/src/main.py"):
    ...     # Can write
    ...     pass
    >>>
    >>> # Audit logging
    >>> audit = AuditLogger()
    >>> audit.log(AuditOperation.WRITE, "/file.py", AuditResult.SUCCESS)
    >>>
    >>> # Change preview
    >>> previewer = ChangePreviewer()
    >>> change = previewer.create_change(ChangeType.WRITE, "/file.py", "content")
    >>> if previewer.confirm(change):
    ...     # Execute operation
    ...     pass

Security Pipeline:
    >>> # Complete security check flow
    >>> path = "/project/src/main.py"
    >>>
    >>> # 1. Validate path
    >>> validator = PathValidator(project_root="/project")
    >>> result = validator.validate(path)
    >>> if not result.is_valid:
    ...     raise SecurityError(result.reason)
    >>>
    >>> # 2. Check permission
    >>> checker = PermissionChecker()
    >>> if not checker.can_write(path):
    ...     raise PermissionError("No write permission")
    >>>
    >>> # 3. Preview change
    >>> previewer = ChangePreviewer()
    >>> change = previewer.create_change(ChangeType.WRITE, path, content)
    >>> if not previewer.confirm(change):
    ...     raise CancelledError("User cancelled")
    >>>
    >>> # 4. Execute operation (omitted)
    >>>
    >>> # 5. Log audit
    >>> audit = AuditLogger()
    >>> audit.log(AuditOperation.WRITE, path, AuditResult.SUCCESS)
"""

from .audit_logger import (
    AuditLogger,
    AuditOperation,
    AuditRecord,
    AuditResult,
)
from .change_previewer import (
    Change,
    ChangePreviewer,
    ChangeType,
    ConfirmationResult,
    RiskLevel,
)
from .secure_file_ops import (
    safe_open_read,
    safe_read_with_retry,
    safe_write_atomic,
)
from .path_validator import (
    PathValidationResult,
    PathValidator,
    ValidationResult,
)
from .permission_checker import (
    Permission,
    PermissionChecker,
    PermissionResult,
)

__all__ = [
    # Path Validation
    "PathValidator",
    "PathValidationResult",
    "ValidationResult",
    # Permission
    "PermissionChecker",
    "Permission",
    "PermissionResult",
    # Audit
    "AuditLogger",
    "AuditOperation",
    "AuditRecord",
    "AuditResult",
    # Change Preview
    "ChangePreviewer",
    "Change",
    "ChangeType",
    "RiskLevel",
    "ConfirmationResult",
    # Secure File Operations (TOCTOU Protection)
    "safe_open_read",
    "safe_write_atomic",
    "safe_read_with_retry",
]
