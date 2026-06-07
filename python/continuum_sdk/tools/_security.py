"""Security integration helpers for tool functions.

Provides a unified pipeline (path validation -> permission check -> audit log)
for file/shell tools without duplicating boilerplate across each function.

The security pipeline is opt-in via a per-call ``workspace`` or ``security_config``
argument. When neither is provided, a single warning is emitted and the operation
proceeds unchecked (backwards-compatible transition).
"""

from __future__ import annotations

import logging
import os
from collections.abc import Generator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from ..security import (
    AuditLogger,
    AuditOperation,
    AuditResult,
    PathValidator,
    Permission,
    PermissionChecker,
)
from .types import ToolError

logger = logging.getLogger(__name__)

_WARNED_NO_WORKSPACE: set[str] = set()


@dataclass
class SecurityContext:
    """Resolved security components for a single tool invocation."""

    validator: PathValidator | None
    checker: PermissionChecker | None
    auditor: AuditLogger | None
    enforced: bool

    @property
    def enabled(self) -> bool:
        return self.enforced and self.validator is not None


def resolve_security(
    workspace: str | Path | None,
    security_config: dict[str, Any] | None,
    tool_name: str,
) -> SecurityContext:
    """Build a SecurityContext from caller-provided arguments.

    Behaviour:
      - If ``security_config`` is provided, use its components verbatim.
      - Else if ``workspace`` is provided, construct default components rooted at it.
      - Else return a disabled context and warn once per tool name.
    """
    if security_config:
        return SecurityContext(
            validator=security_config.get("validator"),
            checker=security_config.get("checker"),
            auditor=security_config.get("auditor"),
            enforced=True,
        )

    if workspace is not None:
        return SecurityContext(
            validator=PathValidator(project_root=workspace),
            checker=PermissionChecker(),
            auditor=AuditLogger(),
            enforced=True,
        )

    if tool_name not in _WARNED_NO_WORKSPACE:
        logger.warning(
            "security disabled for tool '%s': no workspace configured. "
            "Pass workspace=... or security_config=... to enable path "
            "validation, permission checks, and audit logging.",
            tool_name,
        )
        _WARNED_NO_WORKSPACE.add(tool_name)

    return SecurityContext(validator=None, checker=None, auditor=None, enforced=False)


def enforce_path(
    ctx: SecurityContext,
    path: str | Path,
    permission: Permission,
    audit_op: AuditOperation,
    call_id: str,
    tool_name: str,
) -> Path:
    """Run validator + permission check; record DENIED audit on failure.

    Returns the resolved absolute Path when security is disabled or the
    resolved path from the validator when enabled.

    Raises:
        ToolError: when validation or permission check fails.
    """
    resolved = Path(path).expanduser().resolve()

    if not ctx.enabled:
        return resolved

    assert ctx.validator is not None
    result = ctx.validator.validate(path)
    if not result.is_valid:
        if ctx.auditor is not None:
            ctx.auditor.log(
                operation=audit_op,
                path=str(path),
                result=AuditResult.DENIED,
                details=f"path validation failed: {result.reason}",
            )
        raise ToolError(
            call_id=call_id,
            name=tool_name,
            message=f"path validation failed: {result.reason}",
        )

    if (
        result.resolved_path
    ):  # pragma: no cover - path validator always returns resolved_path when is_valid
        resolved = Path(result.resolved_path)

    if ctx.checker is not None:
        perm_result = ctx.checker.check(resolved, permission)
        permission_required = permission != Permission.CREATE or perm_result.exists
        if not perm_result.has_permission and permission_required:
            if ctx.auditor is not None:
                ctx.auditor.log(
                    operation=audit_op,
                    path=str(resolved),
                    result=AuditResult.DENIED,
                    details=f"permission check failed: {perm_result.reason}",
                )
            raise ToolError(
                call_id=call_id,
                name=tool_name,
                message=f"permission denied ({permission.value}): {perm_result.reason}",
            )

    return resolved


def record_audit(
    ctx: SecurityContext,
    audit_op: AuditOperation,
    path: str | Path,
    success: bool,
    details: str | None = None,
    metadata: dict[str, Any] | None = None,
) -> None:
    """Record a success/failure audit entry if auditing is enabled."""
    if ctx.auditor is None:
        return
    ctx.auditor.log(
        operation=audit_op,
        path=str(path),
        result=AuditResult.SUCCESS if success else AuditResult.FAILURE,
        details=details,
        metadata=metadata,
    )


@contextmanager
def secure_file_read(
    ctx: SecurityContext,
    path: str | Path,
    permission: Permission,
    audit_op: AuditOperation,
    call_id: str,
    tool_name: str,
) -> Generator[bytes, None, None]:
    """
    TOCTOU-safe file read with fd-based verification.

    Opens file by fd, validates real path, then yields content.
    Prevents symlink race condition attacks.

    Args:
        ctx: Security context with validator
        path: File path to read
        permission: Required permission
        audit_op: Audit operation type
        call_id: Tool call ID
        tool_name: Tool name for error messages

    Yields:
        File content as bytes

    Raises:
        ToolError: On security violation or file access error
    """
    file_path = Path(path).expanduser().resolve()

    # If security is disabled, do simple read
    if not ctx.enabled or ctx.validator is None:
        try:
            with open(file_path, "rb") as f:
                yield f.read()
        except FileNotFoundError:
            raise ToolError(
                call_id=call_id,
                name=tool_name,
                message=f"File not found: {file_path}",
            )
        except PermissionError:
            raise ToolError(
                call_id=call_id,
                name=tool_name,
                message=f"Permission denied: {file_path}",
            )
        return

    # TOCTOU-safe read with fd verification
    try:
        fd = os.open(str(file_path), os.O_RDONLY)
    except FileNotFoundError:
        if ctx.auditor is not None:
            ctx.auditor.log(
                operation=audit_op,
                path=str(file_path),
                result=AuditResult.DENIED,
                details="file not found",
            )
        raise ToolError(
            call_id=call_id,
            name=tool_name,
            message=f"File not found: {file_path}",
        )
    except PermissionError:
        if ctx.auditor is not None:
            ctx.auditor.log(
                operation=audit_op,
                path=str(file_path),
                result=AuditResult.DENIED,
                details="permission denied",
            )
        raise ToolError(
            call_id=call_id,
            name=tool_name,
            message=f"Permission denied: {file_path}",
        )

    try:
        # Linux: verify real path through /proc/self/fd
        if os.name != "nt":
            try:
                real_path = Path(os.readlink(f"/proc/self/fd/{fd}"))
            except (OSError, FileNotFoundError):
                real_path = file_path
        else:
            real_path = file_path

        # Validate the real path
        result = ctx.validator.validate(real_path)
        if not result.is_valid:
            if ctx.auditor is not None:
                ctx.auditor.log(
                    operation=audit_op,
                    path=str(real_path),
                    result=AuditResult.DENIED,
                    details=f"TOCTOU protection: {result.reason}",
                )
            raise ToolError(
                call_id=call_id,
                name=tool_name,
                message=f"Security violation: {result.reason}",
            )

        # Read content through fd
        with os.fdopen(fd, "rb") as f:
            content = f.read()

        yield content

    finally:
        # fd is closed by fdopen, nothing to do
        pass


def secure_file_write(
    ctx: SecurityContext,
    path: str | Path,
    content: bytes,
    permission: Permission,
    audit_op: AuditOperation,
    call_id: str,
    tool_name: str,
    create_dirs: bool = True,
) -> Path:
    """
    TOCTOU-safe atomic file write.

    Writes to temp file then atomically renames.
    Validates path before writing.

    Args:
        ctx: Security context with validator
        path: Target file path
        content: Content to write
        permission: Required permission
        audit_op: Audit operation type
        call_id: Tool call ID
        tool_name: Tool name for error messages
        create_dirs: Create parent directories if needed

    Returns:
        Path to written file

    Raises:
        ToolError: On security violation or write error
    """
    file_path = Path(path).expanduser().resolve()

    # If security is disabled, do simple write
    if not ctx.enabled or ctx.validator is None:
        if create_dirs and not file_path.parent.exists():
            file_path.parent.mkdir(parents=True, exist_ok=True)

        temp_path = file_path.with_suffix(file_path.suffix + ".tmp")
        try:
            with open(temp_path, "wb") as f:
                f.write(content)
            if os.name == "nt":
                os.replace(temp_path, file_path)
            else:
                os.rename(temp_path, file_path)
        except OSError as e:
            temp_path.unlink(missing_ok=True)
            raise ToolError(
                call_id=call_id,
                name=tool_name,
                message=f"Failed to write file: {e}",
            )
        return file_path

    # Validate target path
    result = ctx.validator.validate(file_path)
    if not result.is_valid:
        if ctx.auditor is not None:
            ctx.auditor.log(
                operation=audit_op,
                path=str(file_path),
                result=AuditResult.DENIED,
                details=f"path validation failed: {result.reason}",
            )
        raise ToolError(
            call_id=call_id,
            name=tool_name,
            message=f"Security violation: {result.reason}",
        )

    # Create parent directories
    if create_dirs and not file_path.parent.exists():
        file_path.parent.mkdir(parents=True, exist_ok=True)

    # Atomic write: temp file -> rename
    temp_path = file_path.with_suffix(file_path.suffix + ".tmp")
    try:
        with open(temp_path, "wb") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())

        if os.name == "nt":
            os.replace(temp_path, file_path)
        else:
            os.rename(temp_path, file_path)

        logger.debug(f"Atomically wrote {len(content)} bytes to {file_path}")

    except (OSError, PermissionError) as e:
        temp_path.unlink(missing_ok=True)
        if ctx.auditor is not None:
            ctx.auditor.log(
                operation=audit_op,
                path=str(file_path),
                result=AuditResult.FAILURE,
                details=f"write failed: {e}",
            )
        raise ToolError(
            call_id=call_id,
            name=tool_name,
            message=f"Failed to write file: {e}",
        )
    finally:
        temp_path.unlink(missing_ok=True)

    return file_path
