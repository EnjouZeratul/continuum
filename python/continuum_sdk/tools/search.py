"""
Search Tools

Grep and Glob for file content and pattern matching.

Features:
    - Grep: Regex search in file contents
    - Glob: File pattern matching
    - Results filtering
"""

import re
import time
from pathlib import Path
from re import Pattern
from typing import Any

from ..utils import generate_short_id
from .file_ops import detect_encoding
from .types import ToolError, ToolResult


def grep(
    pattern: str,
    path: str | None = None,
    glob_pattern: str | None = None,
    case_sensitive: bool = False,
    output_mode: str = "content",
    head_limit: int = 250,
    include_line_numbers: bool = True,
) -> ToolResult:
    """
    Search file contents with regex pattern.

    Args:
        pattern: Regex pattern to search
        path: Search directory (default current)
        glob_pattern: File filter pattern (e.g., "*.py")
        case_sensitive: Case sensitive search (default False)
        output_mode: "content" or "files_with_matches" or "count"
        head_limit: Max results to return (default 250)
        include_line_numbers: Show line numbers (default True)

    Returns:
        ToolResult with matches

    Raises:
        ToolError: If search fails
    """
    call_id = generate_short_id()
    start_time = time.time()

    # Prepare regex
    flags = re.MULTILINE
    if not case_sensitive:
        flags |= re.IGNORECASE

    try:
        regex: Pattern = re.compile(pattern, flags)
    except re.error as e:
        raise ToolError(
            call_id=call_id,
            name="grep",
            message=f"Invalid regex pattern: {e}",
        )

    # Prepare search path
    search_path = Path(path or ".").expanduser().resolve()
    if not search_path.exists():
        raise ToolError(
            call_id=call_id,
            name="grep",
            message=f"Path not found: {search_path}",
        )

    # Find files to search
    files: list[Path] = []
    if search_path.is_file():
        files = [search_path]
    elif search_path.is_dir():  # pragma: no branch - else branch covered by pragma below
        if glob_pattern:
            files = list(search_path.glob(glob_pattern))
        else:  # pragma: no cover - rglob("*") is slow, tests use glob_pattern
            # Search all files (excluding hidden and binary)
            files = [
                f
                for f in search_path.rglob("*")
                if f.is_file() and not f.name.startswith(".")
            ]

    # Search files
    results: list[dict[str, Any]] = []
    matched_files: list[str] = []
    total_matches = 0

    for file_path in files[:1000]:  # Limit to 1000 files
        try:
            encoding = detect_encoding(file_path)
            with open(file_path, encoding=encoding, errors="replace") as f:
                content = f.read()

            matches = list(regex.finditer(content))

            if matches:
                matched_files.append(str(file_path))
                total_matches += len(matches)

                if output_mode == "content":
                    for match in matches[:head_limit]:
                        # Find line number
                        line_num = content[: match.start()].count("\n") + 1
                        line_content = content.split("\n")[line_num - 1]

                        results.append(
                            {
                                "file": str(file_path),
                                "line": line_num,
                                "content": line_content,
                                "match": match.group(),
                                "start": match.start(),
                                "end": match.end(),
                            }
                        )

                elif output_mode == "count":
                    results.append(
                        {
                            "file": str(file_path),
                            "count": len(matches),
                        }
                    )

                # Stop if we have enough results
                if output_mode == "content" and len(results) >= head_limit:
                    break

        except (OSError, IOError, PermissionError, UnicodeDecodeError):
            continue  # Skip files that can't be read

    duration_ms = int((time.time() - start_time) * 1000)

    # Format output
    if output_mode == "files_with_matches":
        content_output = "\n".join(matched_files)
    elif output_mode == "count":
        content_output = "\n".join(
            f"{r['file']}: {r['count']} matches" for r in results
        )
    else:
        # Content mode with line numbers
        output_lines = []
        for r in results[:head_limit]:
            if include_line_numbers:
                output_lines.append(f"{r['file']}:{r['line']}:\t{r['content']}")
            else:
                output_lines.append(f"{r['file']}:\t{r['content']}")
        content_output = "\n".join(output_lines)

    metadata = {
        "pattern": pattern,
        "path": str(search_path),
        "files_searched": len(files),
        "files_matched": len(matched_files),
        "total_matches": total_matches,
        "output_mode": output_mode,
    }

    return ToolResult(
        call_id=call_id,
        name="grep",
        content=content_output or "(no matches)",
        is_error=False,
        duration_ms=duration_ms,
        metadata=metadata,
    )


def glob(
    pattern: str,
    path: str | None = None,
) -> ToolResult:
    """
    Find files matching glob pattern.

    Args:
        pattern: Glob pattern (e.g., "**/*.py")
        path: Search directory (default current)

    Returns:
        ToolResult with matched file paths

    Raises:
        ToolError: If search fails
    """
    call_id = generate_short_id()
    start_time = time.time()

    search_path = Path(path or ".").expanduser().resolve()
    if not search_path.exists():
        raise ToolError(
            call_id=call_id,
            name="glob",
            message=f"Path not found: {search_path}",
        )

    try:
        # Use pathlib glob
        if pattern.startswith("**/"):
            matches = list(search_path.rglob(pattern[3:]))
        else:
            matches = list(search_path.glob(pattern))

        # Filter out directories, sort by modification time
        files = sorted(
            [f for f in matches if f.is_file()],
            key=lambda f: f.stat().st_mtime,
            reverse=True,
        )

        duration_ms = int((time.time() - start_time) * 1000)

        # Format output
        content_output = "\n".join(str(f) for f in files)

        metadata = {
            "pattern": pattern,
            "path": str(search_path),
            "count": len(files),
        }

        return ToolResult(
            call_id=call_id,
            name="glob",
            content=content_output or "(no matches)",
            is_error=False,
            duration_ms=duration_ms,
            metadata=metadata,
        )

    except (OSError, IOError, PermissionError, ValueError) as e:
        raise ToolError(
            call_id=call_id,
            name="glob",
            message=f"Failed to search: {e}",
        )


class GrepTool:
    """
    Grep tool wrapper.

    Example:
        >>> from continuum_sdk.tools import GrepTool
        >>> grep = GrepTool()
        >>> result = grep.search("def\\s+\\w+", glob_pattern="*.py")
    """

    def search(
        self,
        pattern: str,
        path: str | None = None,
        glob_pattern: str | None = None,
        **kwargs,
    ) -> ToolResult:
        """Search files for pattern."""
        return grep(pattern, path, glob_pattern, **kwargs)

    def __call__(self, pattern: str, **kwargs) -> ToolResult:
        """Allow calling instance directly."""
        return self.search(pattern, **kwargs)


class GlobTool:
    """
    Glob tool wrapper.

    Example:
        >>> from continuum_sdk.tools import GlobTool
        >>> glob = GlobTool()
        >>> result = glob.find("**/*.py")
    """

    def find(
        self,
        pattern: str,
        path: str | None = None,
    ) -> ToolResult:
        """Find files matching pattern."""
        return glob(pattern, path)

    def __call__(self, pattern: str, **kwargs) -> ToolResult:
        """Allow calling instance directly."""
        return self.find(pattern, **kwargs)
