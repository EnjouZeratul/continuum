"""
LSP Tools - Code Analysis

Python fallback implementation for LSP-like code analysis tools.

Features:
    - go_to_definition: Find symbol definition
    - find_references: Find all references to a symbol
    - get_hover: Get type/hover information
    - symbol_search: Search for symbols in project

Note: This is a basic implementation using regex/grep.
For full LSP support, use the Rust binding which includes
LSP client integration.
"""

import re
import time
from pathlib import Path
from typing import Any

from ..utils import generate_short_id
from .types import ToolError, ToolResult


# Definition patterns by file extension
DEFINITION_PATTERNS = {
    ".py": [
        # Function definition
        r"def\s+{symbol}\s*\(",
        # Class definition
        r"class\s+{symbol}\s*[:\(]",
        # Variable assignment
        r"^{symbol}\s*=",
    ],
    ".rs": [
        r"fn\s+{symbol}\s*[<(]",
        r"struct\s+{symbol}\s*[{{<]",
        r"enum\s+{symbol}\s*[{{<]",
        r"trait\s+{symbol}\s*[{{<]",
        r"type\s+{symbol}\s*=",
        r"const\s+{symbol}\s*:",
        r"static\s+{symbol}\s*:",
        r"let\s+{symbol}\s*=",
    ],
    ".ts": [".tsx"],
    ".tsx": [
        r"function\s+{symbol}\s*[<(]",
        r"const\s+{symbol}\s*[=<(]",
        r"let\s+{symbol}\s*=",
        r"var\s+{symbol}\s*=",
        r"class\s+{symbol}\s*[{{<]",
        r"interface\s+{symbol}\s*[{{<]",
        r"type\s+{symbol}\s*=",
        r"enum\s+{symbol}\s*[{{<]",
    ],
    ".js": [".jsx"],
    ".jsx": [
        r"function\s+{symbol}\s*\(",
        r"const\s+{symbol}\s*=",
        r"let\s+{symbol}\s*=",
        r"var\s+{symbol}\s*=",
        r"class\s+{symbol}\s*[{{]",
    ],
    ".go": [
        r"func\s+{symbol}\s*\(",
        r"func\s+\([^)]+\)\s*{symbol}\s*\(",
        r"type\s+{symbol}\s+(struct|interface)",
        r"var\s+{symbol}\s+",
        r"const\s+{symbol}\s+",
    ],
    ".java": [
        r"(public|private|protected)?\s*(static)?\s*\w+\s+{symbol}\s*\(",
        r"class\s+{symbol}\s*[{{<]",
        r"interface\s+{symbol}\s*[{{<]",
        r"enum\s+{symbol}\s*[{{]",
    ],
    ".c": [".cpp", ".cc", ".cxx", ".h", ".hpp"],
    ".cpp": [
        r"\w+\s+{symbol}\s*\(",
        r"class\s+{symbol}\s*[{{:]",
        r"struct\s+{symbol}\s*[{{:]",
        r"enum\s+{symbol}\s*[{{:]",
        r"typedef\s+.*\s+{symbol}\s*;",
    ],
}


def get_definition_patterns(file_ext: str, symbol: str) -> list[str]:
    """Get definition patterns for file type and symbol."""
    # Handle aliases
    ext_map = {
        ".tsx": ".ts",
        ".jsx": ".js",
        ".cpp": ".c",
        ".cc": ".c",
        ".cxx": ".c",
        ".h": ".c",
        ".hpp": ".c",
    }

    ext = ext_map.get(file_ext, file_ext)
    patterns = DEFINITION_PATTERNS.get(ext, [])

    return [p.format(symbol=re.escape(symbol)) for p in patterns]


def go_to_definition(
    file_path: str,
    line: int,
    column: int,
    symbol: str | None = None,
    search_dir: str | None = None,
) -> ToolResult:
    """
    Find the definition of a symbol.

    Args:
        file_path: File path
        line: Line number (1-based)
        column: Column number (1-based)
        symbol: Optional symbol name (if not provided, extracted from position)
        search_dir: Optional directory to search (default: same as file)

    Returns:
        ToolResult with definition location

    Raises:
        ToolError: If file not found or definition not found
    """
    call_id = generate_short_id()
    start_time = time.time()

    path = Path(file_path).expanduser().resolve()

    if not path.exists():
        raise ToolError(
            call_id=call_id,
            name="go_to_definition",
            message=f"File not found: {path}",
        )

    # Read file content
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
        lines = content.split("\n")
    except (OSError, IOError, PermissionError, UnicodeDecodeError) as e:
        raise ToolError(
            call_id=call_id,
            name="go_to_definition",
            message=f"Failed to read file: {e}",
        )

    # Extract symbol from position if not provided
    if not symbol:
        if 1 <= line <= len(lines):
            line_content = lines[line - 1]
            # Find word at column
            if 1 <= column <= len(line_content):
                # Extract identifier around column
                match = re.search(r"\b\w+\b", line_content[column - 1 :])
                if match:
                    symbol = match.group()
                else:
                    # Try before column
                    match = re.search(r"\b(\w+)\b\s*$", line_content[:column])
                    if match:
                        symbol = match.group(1)

    if not symbol:
        duration_ms = int((time.time() - start_time) * 1000)
        return ToolResult(
            call_id=call_id,
            name="go_to_definition",
            content=f"No symbol found at {file_path}:{line}:{column}",
            is_error=False,
            duration_ms=duration_ms,
        )

    # Get file extension
    file_ext = path.suffix.lower()
    patterns = get_definition_patterns(file_ext, symbol)

    if not patterns:
        duration_ms = int((time.time() - start_time) * 1000)
        return ToolResult(
            call_id=call_id,
            name="go_to_definition",
            content=f"No definition patterns for file type: {file_ext}",
            is_error=False,
            duration_ms=duration_ms,
        )

    # Search in current file first
    for i, line_content in enumerate(lines, start=1):
        for pattern in patterns:
            if re.search(pattern, line_content):
                duration_ms = int((time.time() - start_time) * 1000)
                return ToolResult(
                    call_id=call_id,
                    name="go_to_definition",
                    content=f"Definition found: {file_path}:{i}",
                    is_error=False,
                    duration_ms=duration_ms,
                    metadata={
                        "file": str(path),
                        "line": i,
                        "symbol": symbol,
                    },
                )

    # Search in same directory
    search_path = Path(search_dir) if search_dir else path.parent

    for py_file in search_path.glob(f"*{file_ext}"):
        if py_file == path:
            continue

        try:
            content = py_file.read_text(encoding="utf-8", errors="replace")
            lines = content.split("\n")

            for i, line_content in enumerate(lines, start=1):
                for pattern in patterns:
                    if re.search(pattern, line_content):
                        duration_ms = int((time.time() - start_time) * 1000)
                        return ToolResult(
                            call_id=call_id,
                            name="go_to_definition",
                            content=f"Definition found: {py_file}:{i}",
                            is_error=False,
                            duration_ms=duration_ms,
                            metadata={
                                "file": str(py_file),
                                "line": i,
                                "symbol": symbol,
                            },
                        )
        except (OSError, IOError, PermissionError):
            continue

    duration_ms = int((time.time() - start_time) * 1000)
    return ToolResult(
        call_id=call_id,
        name="go_to_definition",
        content=f"No definition found for symbol: {symbol}",
        is_error=False,
        duration_ms=duration_ms,
    )


def find_references(
    file_path: str,
    line: int,
    column: int,
    symbol: str | None = None,
    search_dir: str | None = None,
    include_declaration: bool = True,
) -> ToolResult:
    """
    Find all references to a symbol.

    Args:
        file_path: File path
        line: Line number (1-based)
        column: Column number (1-based)
        symbol: Optional symbol name
        search_dir: Directory to search (default: same as file's directory)
        include_declaration: Include the declaration in results

    Returns:
        ToolResult with list of references
    """
    call_id = generate_short_id()
    start_time = time.time()

    path = Path(file_path).expanduser().resolve()

    if not path.exists():
        raise ToolError(
            call_id=call_id,
            name="find_references",
            message=f"File not found: {path}",
        )

    # Read file and extract symbol
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
        lines = content.split("\n")
    except (OSError, IOError, PermissionError, UnicodeDecodeError) as e:
        raise ToolError(
            call_id=call_id,
            name="find_references",
            message=f"Failed to read file: {e}",
        )

    # Extract symbol from position
    if not symbol:
        if 1 <= line <= len(lines):
            line_content = lines[line - 1]
            if 1 <= column <= len(line_content):
                match = re.search(r"\b\w+\b", line_content[column - 1 :])
                if match:
                    symbol = match.group()

    if not symbol:
        duration_ms = int((time.time() - start_time) * 1000)
        return ToolResult(
            call_id=call_id,
            name="find_references",
            content=f"No symbol found at {file_path}:{line}:{column}",
            is_error=False,
            duration_ms=duration_ms,
        )

    # Build reference pattern
    ref_pattern = re.compile(rf"\b{re.escape(symbol)}\b")

    references = []
    search_path = Path(search_dir) if search_dir else path.parent

    # Search in all files in directory
    file_ext = path.suffix.lower()
    for search_file in search_path.glob(f"**/*{file_ext}"):
        try:
            content = search_file.read_text(encoding="utf-8", errors="replace")
            lines = content.split("\n")

            for i, line_content in enumerate(lines, start=1):
                if ref_pattern.search(line_content):
                    # Check if this is a declaration
                    is_declaration = False
                    patterns = get_definition_patterns(file_ext, symbol)
                    for p in patterns:
                        if re.search(p, line_content):
                            is_declaration = True
                            break

                    if include_declaration or not is_declaration:
                        references.append({
                            "file": str(search_file),
                            "line": i,
                            "column": line_content.find(symbol) + 1,
                            "is_declaration": is_declaration,
                        })
        except (OSError, IOError, PermissionError):
            continue

    duration_ms = int((time.time() - start_time) * 1000)

    if not references:
        return ToolResult(
            call_id=call_id,
            name="find_references",
            content=f"No references found for symbol: {symbol}",
            is_error=False,
            duration_ms=duration_ms,
        )

    # Format output
    output_lines = []
    for ref in references:
        decl_marker = " [declaration]" if ref["is_declaration"] else ""
        output_lines.append(f"{ref['file']}:{ref['line']}:{ref['column']}{decl_marker}")

    return ToolResult(
        call_id=call_id,
        name="find_references",
        content="\n".join(output_lines),
        is_error=False,
        duration_ms=duration_ms,
        metadata={
            "symbol": symbol,
            "total_references": len(references),
            "declarations": sum(1 for r in references if r["is_declaration"]),
        },
    )


def get_hover(
    file_path: str,
    line: int,
    column: int,
) -> ToolResult:
    """
    Get hover/type information for a symbol.

    Args:
        file_path: File path
        line: Line number (1-based)
        column: Column number (1-based)

    Returns:
        ToolResult with hover information

    Note: This is a basic implementation that returns the line content.
    For full type information, use the Rust binding with LSP integration.
    """
    call_id = generate_short_id()
    start_time = time.time()

    path = Path(file_path).expanduser().resolve()

    if not path.exists():
        raise ToolError(
            call_id=call_id,
            name="get_hover",
            message=f"File not found: {path}",
        )

    try:
        content = path.read_text(encoding="utf-8", errors="replace")
        lines = content.split("\n")
    except (OSError, IOError, PermissionError, UnicodeDecodeError) as e:
        raise ToolError(
            call_id=call_id,
            name="get_hover",
            message=f"Failed to read file: {e}",
        )

    if not (1 <= line <= len(lines)):
        raise ToolError(
            call_id=call_id,
            name="get_hover",
            message=f"Line {line} out of range (file has {len(lines)} lines)",
        )

    line_content = lines[line - 1]

    # Find symbol at column
    symbol = None
    if 1 <= column <= len(line_content):
        match = re.search(r"\b\w+\b", line_content[column - 1 :])
        if match:
            symbol = match.group()
        else:
            match = re.search(r"\b(\w+)\b", line_content)
            if match:
                symbol = match.group(1)

    duration_ms = int((time.time() - start_time) * 1000)

    if not symbol:
        return ToolResult(
            call_id=call_id,
            name="get_hover",
            content=f"Line {line}: {line_content.strip()}",
            is_error=False,
            duration_ms=duration_ms,
        )

    # Try to find definition for context
    file_ext = path.suffix.lower()
    patterns = get_definition_patterns(file_ext, symbol)

    # Check if current line is a definition
    is_definition = False
    definition_type = None

    for pattern in patterns:
        if re.search(pattern, line_content):
            is_definition = True
            # Determine type
            if "fn " in pattern or "def " in pattern or "function " in pattern:
                definition_type = "function"
            elif "struct " in pattern or "class " in pattern:
                definition_type = "class/struct"
            elif "enum " in pattern:
                definition_type = "enum"
            elif "interface " in pattern:
                definition_type = "interface"
            elif "const " in pattern or "let " in pattern or "var " in pattern:
                definition_type = "variable"
            break

    hover_text = f"**{symbol}**"
    if is_definition and definition_type:
        hover_text += f" ({definition_type})"
    hover_text += f"\n\n```\n{line_content.strip()}\n```"

    return ToolResult(
        call_id=call_id,
        name="get_hover",
        content=hover_text,
        is_error=False,
        duration_ms=duration_ms,
        metadata={
            "symbol": symbol,
            "line": line,
            "is_definition": is_definition,
            "definition_type": definition_type,
        },
    )


def symbol_search(
    pattern: str,
    search_dir: str | None = None,
    file_pattern: str | None = None,
) -> ToolResult:
    """
    Search for symbols matching a pattern.

    Args:
        pattern: Symbol pattern to search (regex supported)
        search_dir: Directory to search
        file_pattern: File pattern filter (e.g., "*.py")

    Returns:
        ToolResult with matching symbols
    """
    call_id = generate_short_id()
    start_time = time.time()

    search_path = Path(search_dir or ".").expanduser().resolve()

    if not search_path.exists():
        raise ToolError(
            call_id=call_id,
            name="symbol_search",
            message=f"Directory not found: {search_path}",
        )

    try:
        regex = re.compile(pattern, re.IGNORECASE)
    except re.error as e:
        raise ToolError(
            call_id=call_id,
            name="symbol_search",
            message=f"Invalid regex pattern: {e}",
        )

    matches = []

    # Get file extensions to search
    extensions = [".py", ".rs", ".ts", ".tsx", ".js", ".jsx", ".go", ".java", ".c", ".cpp", ".h"]

    for ext in extensions:
        glob_pattern = file_pattern or f"**/*{ext}"
        for file_path in search_path.glob(glob_pattern):
            if not file_path.is_file():
                continue

            try:
                content = file_path.read_text(encoding="utf-8", errors="replace")
                lines = content.split("\n")

                for i, line_content in enumerate(lines, start=1):
                    for match in regex.finditer(line_content):
                        symbol = match.group()
                        matches.append({
                            "file": str(file_path),
                            "line": i,
                            "column": match.start() + 1,
                            "symbol": symbol,
                            "context": line_content.strip(),
                        })
            except (OSError, IOError, PermissionError, UnicodeDecodeError):
                continue

    duration_ms = int((time.time() - start_time) * 1000)

    if not matches:
        return ToolResult(
            call_id=call_id,
            name="symbol_search",
            content=f"No symbols found matching: {pattern}",
            is_error=False,
            duration_ms=duration_ms,
        )

    # Format output
    output_lines = []
    for m in matches[:100]:  # Limit to 100 results
        output_lines.append(f"{m['file']}:{m['line']}:{m['column']}: {m['symbol']}")

    return ToolResult(
        call_id=call_id,
        name="symbol_search",
        content="\n".join(output_lines),
        is_error=False,
        duration_ms=duration_ms,
        metadata={
            "pattern": pattern,
            "total_matches": len(matches),
            "shown": min(len(matches), 100),
        },
    )


class LspTools:
    """
    LSP tools wrapper for convenient usage.

    Example:
        >>> from continuum_sdk.tools import LspTools
        >>> lsp = LspTools()
        >>> result = lsp.go_to_definition("src/main.rs", 10, 5)
    """

    def go_to_definition(
        self,
        file_path: str,
        line: int,
        column: int,
        symbol: str | None = None,
        search_dir: str | None = None,
    ) -> ToolResult:
        """Find symbol definition."""
        return go_to_definition(file_path, line, column, symbol, search_dir)

    def find_references(
        self,
        file_path: str,
        line: int,
        column: int,
        symbol: str | None = None,
        search_dir: str | None = None,
    ) -> ToolResult:
        """Find all references to symbol."""
        return find_references(file_path, line, column, symbol, search_dir)

    def get_hover(self, file_path: str, line: int, column: int) -> ToolResult:
        """Get hover information."""
        return get_hover(file_path, line, column)

    def symbol_search(
        self,
        pattern: str,
        search_dir: str | None = None,
        file_pattern: str | None = None,
    ) -> ToolResult:
        """Search for symbols."""
        return symbol_search(pattern, search_dir, file_pattern)
