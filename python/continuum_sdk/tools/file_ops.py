"""
Read Tool

File reading with large file support, encoding detection, and pagination.

Features:
    - Read file content
    - Large file pagination (offset/limit)
    - Encoding detection
    - Line number formatting
    - TOCTOU-safe reading (secure_mode)
"""

import difflib
import shutil
import time
from pathlib import Path
from typing import Any

from ..security import AuditOperation, Permission
from ..utils import generate_short_id
from ._security import (
    enforce_path,
    record_audit,
    resolve_security,
    secure_file_read,
    secure_file_write,
)
from .types import ToolError, ToolResult


def detect_encoding(file_path: Path) -> str:
    """
    Detect file encoding.

    Args:
        file_path: Path to file

    Returns:
        Encoding name (utf-8, gbk, etc.)
    """
    # Read first 4KB for encoding detection
    try:
        with open(file_path, "rb") as f:
            raw = f.read(4096)

        # Try UTF-8 first
        try:
            raw.decode("utf-8")
            return "utf-8"
        except UnicodeDecodeError:
            pass

        # Try common encodings
        for encoding in ["gbk", "gb2312", "gb18030", "shift-jis", "euc-kr", "latin-1"]:
            try:
                raw.decode(encoding)
                return encoding
            except (UnicodeDecodeError, LookupError):
                continue

        # Fallback to utf-8 with errors='replace'
        return "utf-8"  # pragma: no cover - defensive fallback, all paths return earlier
    except (OSError, UnicodeDecodeError):
        return "utf-8"


def read_file(
    path: str,
    offset: int | None = None,
    limit: int | None = None,
    show_line_numbers: bool = False,
    secure_mode: bool = False,
    *,
    workspace: str | Path | None = None,
    security_config: dict[str, Any] | None = None,
) -> ToolResult:
    """
    Read file content.

    Args:
        path: File path
        offset: Starting line number (1-based, optional)
        limit: Number of lines to read (optional)
        show_line_numbers: Whether to show line numbers (default False)
        secure_mode: Enable TOCTOU-safe reading (default False)
        workspace: Optional workspace root for security enforcement
        security_config: Optional explicit security components

    Returns:
        ToolResult with file content

    Raises:
        ToolError: If file doesn't exist, can't be read, or fails security checks
    """
    call_id = generate_short_id()
    start_time = time.time()

    sec = resolve_security(workspace, security_config, "read_file")

    if secure_mode and sec.enabled:
        # TOCTOU-safe read with fd verification
        try:
            with secure_file_read(
                sec, path, Permission.READ, AuditOperation.READ, call_id, "read"
            ) as raw_content:
                # Decode content
                encoding = detect_encoding_from_bytes(raw_content)
                content_str = raw_content.decode(encoding, errors="replace")

                # Process lines
                lines = content_str.splitlines(keepends=True)
                start_line = (offset or 1) - 1
                if start_line < 0:
                    start_line = 0
                end_line = start_line + (limit or len(lines))
                if end_line > len(lines):
                    end_line = len(lines)
                selected_lines = lines[start_line:end_line]

                if show_line_numbers:
                    output_lines = []
                    for i, line in enumerate(selected_lines, start=(offset or 1)):
                        line = line.rstrip("\n\r")
                        output_lines.append(f"{i:6}\t{line}")
                    content = "\n".join(output_lines)
                else:
                    content = "".join(selected_lines).rstrip("\n\r")

                duration_ms = int((time.time() - start_time) * 1000)
                metadata = {
                    "path": str(Path(path).resolve()),
                    "encoding": encoding,
                    "total_lines": len(lines),
                    "lines_read": len(selected_lines),
                    "secure_mode": True,
                }

                record_audit(
                    sec, AuditOperation.READ, Path(path).resolve(), success=True,
                    metadata={"lines_read": len(selected_lines), "encoding": encoding, "secure_mode": True},
                )

                return ToolResult(
                    call_id=call_id,
                    name="read",
                    content=content,
                    is_error=False,
                    duration_ms=duration_ms,
                    metadata=metadata,
                )
        except ToolError:
            raise
        except (OSError, UnicodeDecodeError) as e:
            file_path = Path(path).resolve()
            record_audit(sec, AuditOperation.READ, file_path, success=False, details=str(e))
            raise ToolError(
                call_id=call_id,
                name="read",
                message=f"Failed to read file: {e}",
            )
    else:
        # Legacy read mode
        file_path = enforce_path(
            sec, path, Permission.READ, AuditOperation.READ, call_id, "read"
        )

        # Check if file exists
        if not file_path.exists():
            record_audit(sec, AuditOperation.READ, file_path, success=False,
                         details="file not found")
            raise ToolError(
                call_id=call_id,
                name="read",
                message=f"File not found: {file_path}",
            )

        # Check if it's a file
        if not file_path.is_file():
            record_audit(sec, AuditOperation.READ, file_path, success=False,
                         details="not a file")
            raise ToolError(
                call_id=call_id,
                name="read",
                message=f"Not a file: {file_path}",
            )

        # Detect encoding
        encoding = detect_encoding(file_path)

        try:
            # Read file
            with open(file_path, encoding=encoding, errors="replace") as f:
                lines = f.readlines()

            # Calculate line range
            start_line = (offset or 1) - 1  # 0-based index
            if start_line < 0:
                start_line = 0

            end_line = start_line + (limit or len(lines))
            if end_line > len(lines):
                end_line = len(lines)

            # Extract lines
            selected_lines = lines[start_line:end_line]

            # Format output
            if show_line_numbers:
                # Format with line numbers like cat -n
                output_lines = []
                for i, line in enumerate(selected_lines, start=(offset or 1)):
                    line = line.rstrip("\n\r")
                    output_lines.append(f"{i:6}\t{line}")
                content = "\n".join(output_lines)
            else:
                content = "".join(selected_lines).rstrip("\n\r")

            duration_ms = int((time.time() - start_time) * 1000)

            # Add metadata
            metadata = {
                "path": str(file_path),
                "encoding": encoding,
                "total_lines": len(lines),
                "lines_read": len(selected_lines),
            }

            record_audit(
                sec, AuditOperation.READ, file_path, success=True,
                metadata={"lines_read": len(selected_lines), "encoding": encoding},
            )

            return ToolResult(
                call_id=call_id,
                name="read",
                content=content,
                is_error=False,
                duration_ms=duration_ms,
                metadata=metadata,
            )

        except PermissionError:
            record_audit(sec, AuditOperation.READ, file_path, success=False,
                         details="permission denied")
            raise ToolError(
                call_id=call_id,
                name="read",
                message=f"Permission denied: {file_path}",
            )
        except (OSError, UnicodeDecodeError) as e:
            record_audit(sec, AuditOperation.READ, file_path, success=False,
                         details=str(e))
            raise ToolError(
                call_id=call_id,
                name="read",
                message=f"Failed to read file: {e}",
            )


def detect_encoding_from_bytes(raw: bytes) -> str:
    """
    Detect encoding from raw bytes.

    Args:
        raw: Raw bytes content

    Returns:
        Encoding name (utf-8, gbk, etc.)
    """
    # Try UTF-8 first
    try:
        raw.decode("utf-8")
        return "utf-8"
    except UnicodeDecodeError:
        pass

    # Try common encodings
    for encoding in ["gbk", "gb2312", "gb18030", "shift-jis", "euc-kr", "latin-1"]:
        try:
            raw.decode(encoding)
            return encoding
        except (UnicodeDecodeError, LookupError):
            continue

    return "utf-8"


class ReadTool:
    """
    Read tool wrapper for convenient usage.

    Example:
        >>> from continuum_sdk.tools import ReadTool
        >>> reader = ReadTool()
        >>> result = reader.read("src/main.rs", limit=50)
        >>> print(result.content)
    """

    def __init__(self, show_line_numbers: bool = False):
        self.show_line_numbers = show_line_numbers

    def read(
        self,
        path: str,
        offset: int | None = None,
        limit: int | None = None,
    ) -> ToolResult:
        """Read file content."""
        return read_file(path, offset, limit, self.show_line_numbers)

    def __call__(self, path: str, **kwargs) -> ToolResult:
        """Allow calling instance directly."""
        return self.read(path, **kwargs)


"""
Write Tool

Safe file writing with backup and permission checking.

Features:
    - Safe write with backup
    - Permission check
    - Append mode
    - Atomic write (write to temp, then rename)
"""


def write_file(
    path: str,
    content: str,
    backup: bool = True,
    append: bool = False,
    create_dirs: bool = True,
    encoding: str = "utf-8",
    secure_mode: bool = False,
    *,
    workspace: str | Path | None = None,
    security_config: dict[str, Any] | None = None,
) -> ToolResult:
    """
    Write content to file.

    Args:
        path: File path
        content: Content to write
        backup: Create backup before overwriting (default True)
        append: Append to file instead of overwriting (default False)
        create_dirs: Create parent directories if needed (default True)
        encoding: File encoding (default utf-8)
        secure_mode: Enable TOCTOU-safe atomic write (default False)
        workspace: Optional workspace root for security enforcement
        security_config: Optional explicit security components

    Returns:
        ToolResult indicating success

    Raises:
        ToolError: If write fails or security checks fail
    """
    call_id = generate_short_id()
    start_time = time.time()

    sec = resolve_security(workspace, security_config, "write_file")

    if secure_mode and sec.enabled and not append:
        # TOCTOU-safe atomic write
        try:
            file_path = secure_file_write(
                sec, path, content.encode(encoding),
                Permission.CREATE, AuditOperation.CREATE, call_id, "write",
                create_dirs=create_dirs
            )

            duration_ms = int((time.time() - start_time) * 1000)

            metadata = {
                "path": str(file_path),
                "bytes_written": len(content.encode(encoding)),
                "secure_mode": True,
            }

            record_audit(
                sec, AuditOperation.WRITE if file_path.exists() else AuditOperation.CREATE,
                file_path, success=True,
                metadata={"bytes_written": metadata["bytes_written"], "secure_mode": True},
            )

            return ToolResult(
                call_id=call_id,
                name="write",
                content=f"Successfully wrote to {file_path}",
                is_error=False,
                duration_ms=duration_ms,
                metadata=metadata,
            )
        except ToolError:
            raise
        except (OSError, UnicodeEncodeError) as e:
            file_path = Path(path).resolve()
            record_audit(sec, AuditOperation.WRITE, file_path, success=False, details=str(e))
            raise ToolError(
                call_id=call_id,
                name="write",
                message=f"Failed to write file: {e}",
            )
    else:
        # Legacy write mode
        # Use CREATE permission so enforce_path skips the exists check for new files.
        # After validation we determine the correct audit_op based on actual existence.
        file_path = enforce_path(
            sec, path, Permission.CREATE, AuditOperation.CREATE, call_id, "write"
        )

        file_exists = file_path.exists()
        audit_op = AuditOperation.WRITE if file_exists else AuditOperation.CREATE

        # Create parent directories if needed
        if create_dirs and not file_path.parent.exists():
            file_path.parent.mkdir(parents=True, exist_ok=True)

        # Create backup if needed
        backup_path = None
        if backup and file_exists and not append:
            backup_path = file_path.with_suffix(file_path.suffix + ".bak")
            shutil.copy2(file_path, backup_path)

        try:
            mode = "a" if append else "w"
            with open(file_path, mode, encoding=encoding, errors="replace") as f:
                f.write(content)
                if not content.endswith("\n"):
                    f.write("\n")

            duration_ms = int((time.time() - start_time) * 1000)

            metadata = {
                "path": str(file_path),
                "bytes_written": len(content.encode(encoding)),
                "backup": str(backup_path) if backup_path else None,
            }

            record_audit(
                sec, audit_op, file_path, success=True,
                metadata={"bytes_written": metadata["bytes_written"], "append": append},
            )

            return ToolResult(
                call_id=call_id,
                name="write",
                content=f"Successfully wrote to {file_path}",
                is_error=False,
                duration_ms=duration_ms,
                metadata=metadata,
            )

        except PermissionError:
            record_audit(sec, audit_op, file_path, success=False,
                         details="permission denied")
            raise ToolError(
                call_id=call_id,
                name="write",
                message=f"Permission denied: {file_path}",
            )
        except (OSError, UnicodeEncodeError) as e:
            # Restore from backup if write failed
            if backup_path and backup_path.exists():
                shutil.move(backup_path, file_path)
            record_audit(sec, audit_op, file_path, success=False, details=str(e))
            raise ToolError(
                call_id=call_id,
                name="write",
                message=f"Failed to write file: {e}",
            )


class WriteTool:
    """
    Write tool wrapper.

    Example:
        >>> from continuum_sdk.tools import WriteTool
        >>> writer = WriteTool()
        >>> result = writer.write("output.txt", "Hello, World!")
    """

    def __init__(self, backup: bool = True):
        self.backup = backup

    def write(
        self,
        path: str,
        content: str,
        append: bool = False,
    ) -> ToolResult:
        """Write content to file."""
        return write_file(path, content, backup=self.backup, append=append)

    def append(self, path: str, content: str) -> ToolResult:
        """Append content to file."""
        return self.write(path, content, append=True)

    def __call__(self, path: str, content: str, **kwargs) -> ToolResult:
        """Allow calling instance directly."""
        return self.write(path, content, **kwargs)


"""
Edit Tool

Precise string replacement in files.

Features:
    - Exact string matching
    - Multiple occurrences (replace_all)
    - Preview changes
    - Backup before edit
"""


def edit_file(
    path: str,
    old: str,
    new: str,
    replace_all: bool = False,
    backup: bool = True,
    *,
    workspace: str | Path | None = None,
    security_config: dict[str, Any] | None = None,
) -> ToolResult:
    """
    Edit file by replacing exact string.

    Args:
        path: File path
        old: Text to find (must be exact match)
        new: Text to replace with
        replace_all: Replace all occurrences (default False)
        backup: Create backup before editing (default True)
        workspace: Optional workspace root for security enforcement
        security_config: Optional explicit security components

    Returns:
        ToolResult indicating changes made

    Raises:
        ToolError: If edit fails or string not found
    """
    call_id = generate_short_id()
    start_time = time.time()

    sec = resolve_security(workspace, security_config, "edit_file")
    file_path = enforce_path(
        sec, path, Permission.WRITE, AuditOperation.MODIFY, call_id, "edit"
    )

    # Check file exists
    if not file_path.exists():
        record_audit(sec, AuditOperation.MODIFY, file_path, success=False,
                     details="file not found")
        raise ToolError(
            call_id=call_id,
            name="edit",
            message=f"File not found: {file_path}",
        )

    # Read file
    encoding = detect_encoding(file_path)
    try:
        with open(file_path, encoding=encoding, errors="replace") as f:
            content = f.read()
    except (OSError, PermissionError, UnicodeDecodeError) as e:
        record_audit(sec, AuditOperation.MODIFY, file_path, success=False,
                     details=f"read failed: {e}")
        raise ToolError(
            call_id=call_id,
            name="edit",
            message=f"Failed to read file: {e}",
        )

    # Check if old string exists
    if old not in content:
        record_audit(sec, AuditOperation.MODIFY, file_path, success=False,
                     details="search string not found")
        raise ToolError(
            call_id=call_id,
            name="edit",
            message=f"String not found: {old[:100]}...",
        )

    # Count occurrences
    count = content.count(old)

    # Create backup if needed
    if backup:
        backup_path = file_path.with_suffix(file_path.suffix + ".bak")
        shutil.copy2(file_path, backup_path)

    # Perform replacement
    if replace_all:
        new_content = content.replace(old, new)
        replacements = count
    else:
        new_content = content.replace(old, new, 1)
        replacements = 1

    # Write back
    try:
        with open(file_path, "w", encoding=encoding) as f:
            f.write(new_content)
    except (OSError, PermissionError, UnicodeEncodeError) as e:  # pragma: no cover - hard to trigger write errors
        # Restore from backup
        if backup:
            shutil.move(backup_path, file_path)
        record_audit(sec, AuditOperation.MODIFY, file_path, success=False,
                     details=f"write failed: {e}")
        raise ToolError(
            call_id=call_id,
            name="edit",
            message=f"Failed to write file: {e}",
        )

    duration_ms = int((time.time() - start_time) * 1000)

    # Generate diff for metadata
    diff = list(
        difflib.unified_diff(
            content.splitlines(keepends=True),
            new_content.splitlines(keepends=True),
            fromfile=str(file_path),
            tofile=str(file_path),
        )
    )

    metadata = {
        "path": str(file_path),
        "replacements": replacements,
        "total_occurrences": count,
        "diff": "".join(diff),
    }

    record_audit(
        sec, AuditOperation.MODIFY, file_path, success=True,
        metadata={"replacements": replacements},
    )

    return ToolResult(
        call_id=call_id,
        name="edit",
        content=f"Replaced {replacements} occurrence(s) in {file_path}",
        is_error=False,
        duration_ms=duration_ms,
        metadata=metadata,
    )


class EditTool:
    """
    Edit tool wrapper.

    Example:
        >>> from continuum_sdk.tools import EditTool
        >>> editor = EditTool()
        >>> result = editor.edit("config.py", "old_value", "new_value")
    """

    def __init__(self, backup: bool = True):
        self.backup = backup

    def edit(
        self,
        path: str,
        old: str,
        new: str,
        replace_all: bool = False,
    ) -> ToolResult:
        """Edit file by replacing string."""
        return edit_file(path, old, new, replace_all, self.backup)

    def replace_all(self, path: str, old: str, new: str) -> ToolResult:
        """Replace all occurrences in file."""
        return self.edit(path, old, new, replace_all=True)

    def __call__(self, path: str, old: str, new: str, **kwargs) -> ToolResult:
        """Allow calling instance directly."""
        return self.edit(path, old, new, **kwargs)


"""
List Directory Tool

Directory listing with security enforcement.
"""


def list_directory(
    path: str,
    *,
    workspace: str | Path | None = None,
    security_config: dict[str, Any] | None = None,
) -> ToolResult:
    """
    List entries in a directory.

    Args:
        path: Directory path
        workspace: Optional workspace root for security enforcement
        security_config: Optional explicit security components

    Returns:
        ToolResult whose metadata["entries"] is a list of {name, type, path}.

    Raises:
        ToolError: If path is not a directory or fails security checks.
    """
    call_id = generate_short_id()
    start_time = time.time()

    sec = resolve_security(workspace, security_config, "list_directory")
    dir_path = enforce_path(
        sec, path, Permission.READ, AuditOperation.LIST, call_id, "list_directory"
    )

    if not dir_path.exists():
        record_audit(sec, AuditOperation.LIST, dir_path, success=False,
                     details="directory not found")
        raise ToolError(
            call_id=call_id,
            name="list_directory",
            message=f"Directory not found: {dir_path}",
        )

    if not dir_path.is_dir():
        record_audit(sec, AuditOperation.LIST, dir_path, success=False,
                     details="not a directory")
        raise ToolError(
            call_id=call_id,
            name="list_directory",
            message=f"Not a directory: {dir_path}",
        )

    try:
        entries = []
        for entry in dir_path.iterdir():
            entries.append({
                "name": entry.name,
                "type": "dir" if entry.is_dir() else "file",
                "path": str(entry),
            })
        entries.sort(key=lambda e: (e["type"], e["name"]))
    except PermissionError:
        record_audit(sec, AuditOperation.LIST, dir_path, success=False,
                     details="permission denied")
        raise ToolError(
            call_id=call_id,
            name="list_directory",
            message=f"Permission denied: {dir_path}",
        )
    except OSError as e:
        record_audit(sec, AuditOperation.LIST, dir_path, success=False,
                     details=str(e))
        raise ToolError(
            call_id=call_id,
            name="list_directory",
            message=f"Failed to list directory: {e}",
        )

    duration_ms = int((time.time() - start_time) * 1000)

    record_audit(
        sec, AuditOperation.LIST, dir_path, success=True,
        metadata={"entry_count": len(entries)},
    )

    summary_lines = [f"{e['type']}\t{e['name']}" for e in entries]
    return ToolResult(
        call_id=call_id,
        name="list_directory",
        content="\n".join(summary_lines),
        is_error=False,
        duration_ms=duration_ms,
        metadata={"path": str(dir_path), "entries": entries, "count": len(entries)},
    )


class ListDirectoryTool:
    """List directory tool wrapper."""

    def list(
        self,
        path: str,
        *,
        workspace: str | Path | None = None,
        security_config: dict[str, Any] | None = None,
    ) -> ToolResult:
        return list_directory(path, workspace=workspace, security_config=security_config)

    def __call__(self, path: str, **kwargs) -> ToolResult:
        return self.list(path, **kwargs)
