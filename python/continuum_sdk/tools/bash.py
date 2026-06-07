"""
Bash Tool

Safe command execution with timeout control, output capture, and security sandbox.

Features:
    - Token-level command policy (no startswith bypass)
    - Minimised default environment (PATH/HOME/USER/TEMP only)
    - Optional workspace-rooted working directory validation
    - Audit logging
    - Confirmation required for dangerous commands
"""

from __future__ import annotations

import asyncio
import logging
import os
import shlex
import subprocess
import time
from pathlib import Path
from typing import Any

from continuum_sdk.utils import generate_short_id

from ..security import AuditOperation, Permission
from ._security import enforce_path, record_audit, resolve_security
from .types import ToolError, ToolResult

logger = logging.getLogger(__name__)

# Commands that may never run.
BLOCKED_COMMANDS = {
    # Privilege escalation
    "sudo",
    "su",
    "doas",
    # Shell interpreters (shell-of-shell bypass)
    "bash",
    "sh",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "cmd",
    "cmd.exe",
    "powershell",
    "pwsh",
    # Shell-builtins for arbitrary code execution
    "eval",
    "exec",
    # Network backdoors / data exfiltration
    "mkfifo",
    "nc",
    "ncat",
    "netcat",
    "telnet",
    "curl",
    "wget",
    # Script execution (out-of-band code)
    "python",
    "python3",
    "perl",
    "ruby",
    "node",
    "php",
    # Encoding / obfuscation
    "base64",
    "openssl",
    "xxd",
    # Container escape
    "docker",
    "kubectl",
    # Process manipulation
    "kill",
    "pkill",
    "killall",
}

# Commands allowed only with explicit `confirm=True`.
DANGEROUS_COMMANDS = {
    "rm",
    "rmdir",
    "del",
    "format",
    "mkfs",
    "dd",
    "shutdown",
    "reboot",
    "poweroff",
    "chmod",
    "chown",
    "git",  # git push / git reset / git checkout etc. — require confirm
    "npm",  # npm publish
    "pip",  # pip upload / pip install
    "ssh-keygen",
    "env",
    "nice",
    "nohup",
    "xargs",
    "chroot",
    "ionice",
    "strace",
    "time",
    "find",
}

# Environment variables propagated to subprocesses by default.
SAFE_ENV_KEYS = {
    "PATH",
    "HOME",
    "USER",
    "USERNAME",
    "LOGNAME",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "SYSTEMROOT",
    "WINDIR",  # required for Windows subprocess to find DLLs
    "COMSPEC",
    "PATHEXT",
}

# Shell metacharacters that introduce a new command token list.
_COMMAND_SEPARATORS = {"|", "||", "&", "&&", ";", "\n"}

# Patterns that indicate command substitution or injection — always blocked.
import re as _re

# Injection patterns - always forbidden (even with allow_shell=True)
_INJECTION_RE = _re.compile(
    r"\$\("  # $(...) command substitution - injection
    r"|`"  # `...` backtick substitution - injection
    r"|<\("  # <(...) process substitution - injection
    r"|>\("  # >(...) process substitution - injection
    r"|%0[aA]"  # %0a URL-encoded newline - injection
    r"|%00"  # %00 URL-encoded null byte - injection
)

# Shell operators - conditionally forbidden (only when allow_shell=False)
_SHELL_OPERATOR_RE = _re.compile(
    r"\|\s*\|"  # || operator
    r"|&&"  # && operator
    r"|\|"  # | operator (must come after ||)
    r"|;"  # ; separator
)

# Backward compat: _SUBSTITUTION_RE is now the injection pattern
# (used by tests, deprecated in favor of _INJECTION_RE)
_SUBSTITUTION_RE = _INJECTION_RE

# Dangerous environment variable names that can alter program behavior
DANGEROUS_ENV_VARS = {
    # Dynamic library injection
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    # Python interpreter manipulation
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONSTARTUP",
    # Other interpreter manipulation
    "PERL5LIB",
    "PERLLIB",
    "NODE_PATH",
    "RUBYLIB",
    # Shell manipulation
    "BASH_ENV",
    "ZDOTDIR",
    "ENV",
    # Debug/trace manipulation
    "LD_DEBUG",
    "LD_AUDIT",
}


def _basename(token: str) -> str:
    name = os.path.basename(token)
    # Strip leading/trailing shell metacharacters: (sudo → sudo, sudo) → sudo
    name = name.strip("()'\"")
    if os.name == "nt" and name.lower().endswith((".exe", ".bat", ".cmd", ".com")):
        name = name.rsplit(".", 1)[0]
    return name.lower()


def _split_glued_separators(tokens: list[str]) -> list[str]:
    """Split tokens where a separator is glued to the start or end.

    shlex.split("echo a; sudo rm -rf /") produces ['echo', 'a;', 'sudo', ...].
    This function splits 'a;' into ['a', ';'] so the separator is recognized.
    Only splits at token edges — a separator in the middle of a token (e.g.
    from quoted content like ``fix; add feature``) is left intact.
    """
    result: list[str] = []
    for tok in tokens:
        # Only handle separators at the start or end of a token, not the middle.
        # "a;" → ["a", ";"], ";sudo" → [";", "sudo"], "a&&b" → leave as-is
        prefix_sep = ""
        suffix_sep = ""

        # Check two-char prefix
        if len(tok) >= 2 and tok[:2] in ("&&", "||"):
            prefix_sep = tok[:2]
            tok = tok[2:]
        elif tok and tok[0] in ("|", "&", ";", "\n"):
            prefix_sep = tok[0]
            tok = tok[1:]

        # Check two-char suffix
        if len(tok) >= 2 and tok[-2:] in ("&&", "||"):
            suffix_sep = tok[-2:]
            tok = tok[:-2]
        elif tok and tok[-1] in ("|", "&", ";", "\n"):
            suffix_sep = tok[-1]
            tok = tok[:-1]

        if prefix_sep:
            result.append(prefix_sep)
        if tok:
            result.append(tok)
        if suffix_sep:
            result.append(suffix_sep)
    return result


def _extract_commands(command: str) -> list[list[str]]:
    """Split a shell command line into individual command token lists.

    Splits on pipes, &&, ||, ;, and newlines so policy applies to every
    subcommand, not just the first one.

    Raises:
        ToolError: if command substitution (``$(...)``, `` `...` ``, ``<(...)``)
            is detected — these allow arbitrary code hidden inside arguments.
    """
    if _SUBSTITUTION_RE.search(command):
        raise ToolError(
            call_id="policy",
            name="bash",
            message="Command substitution ($(), `<backtick>`, <()) not allowed",
        )

    try:
        tokens = shlex.split(command, posix=(os.name != "nt"))
    except ValueError:
        # Unparseable command (e.g. unbalanced quote) — treat as a single
        # opaque token list so the caller can still apply policy on the head.
        tokens = command.strip().split()

    # Split glued separators: "a;" → ["a", ";"] etc.
    tokens = _split_glued_separators(tokens)

    commands: list[list[str]] = []
    current: list[str] = []
    for tok in tokens:
        if tok in _COMMAND_SEPARATORS:
            if (
                current
            ):  # pragma: no branch - defensive check, separators always come after tokens
                commands.append(current)
                current = []
        else:
            current.append(tok)
    if current:
        commands.append(current)
    return commands


def validate_command(command: str, confirm: bool = False) -> str | None:
    """Token-level command policy check (back-compat signature).

    Returns:
        None when the command passes policy, otherwise a human-readable error
        string. Command substitution raises ToolError directly.

    Dangerous commands return None (allowed but flagged) unless the caller
    wants to enforce confirmation — callers should use
    :func:`validate_command_tokens` for the structured result.
    """
    try:
        blocked, dangerous = validate_command_tokens(command)
    except ToolError:
        raise  # command substitution — propagate
    if blocked:
        return f"Blocked command: {sorted(set(blocked))[0]}"
    if dangerous and confirm is False:
        return f"Dangerous command token(s) require confirm=True: {', '.join(sorted(set(dangerous)))}"
    return None


def validate_command_tokens(command: str) -> tuple[list[str], list[str]]:
    """Structured token-level policy check.

    Returns:
        (blocked_reasons, dangerous_reasons). ``blocked`` is fatal;
        ``dangerous`` requires ``confirm=True`` at execution time.

    Raises:
        ToolError: if command substitution is detected.
    """
    blocked: list[str] = []
    dangerous: list[str] = []

    for tokens in _extract_commands(command):
        if not tokens:
            continue
        head = _basename(tokens[0])
        if head in BLOCKED_COMMANDS:
            blocked.append(head)
            continue
        if head in DANGEROUS_COMMANDS:
            dangerous.append(head)

    return blocked, dangerous


def _build_safe_env(user_env: dict[str, str] | None, inherit: bool) -> dict[str, str]:
    if inherit:
        env = os.environ.copy()
    else:
        env = {k: os.environ[k] for k in SAFE_ENV_KEYS if k in os.environ}
    if user_env:
        # Filter out dangerous environment variables from user-provided env
        for key, value in user_env.items():
            if key.upper() in DANGEROUS_ENV_VARS:
                logger.warning(f"Blocked dangerous environment variable: {key}")
                continue
            env[key] = value
    return env


def _can_parse_for_exec(command: str) -> bool:
    """Check if command can be safely parsed for exec mode.

    Returns True if shlex.split produces valid tokens without shell features
    (no pipes, chains, glob patterns, or variable expansion).
    """
    try:
        tokens = shlex.split(command, posix=(os.name != "nt"))
        if not tokens:
            return False
        # Check for shell operators that need shell interpretation
        for token in tokens:
            if token in _COMMAND_SEPARATORS:
                # Pipes, chains, etc. need shell
                return False
        # Check for remaining shell metacharacters that need shell interpretation
        for token in tokens:
            # These indicate shell features not handled by exec
            if any(sep in token for sep in ("*", "?", "[", "]", "~")):
                # Glob patterns need shell expansion
                return False
            if token.startswith("$"):
                # Variable expansion needs shell
                return False
        return True
    except ValueError:
        return False


def _validate_command(command: str, allow_shell: bool) -> tuple[bool, str]:
    """
    Validate command security.

    Injection patterns ($(), ``, <(), >(), %0a, %00) are always blocked.
    Shell operators (|, &&, ||, ;) are blocked only when allow_shell=False.

    Returns:
        (is_valid, error_message)
    """
    # 1. Always block injection patterns
    if _INJECTION_RE.search(command):
        return False, (
            "Command contains forbidden injection patterns: "
            "$(), ``, <(), >(), %0a, %00. "
            "These are always blocked for security."
        )

    # 2. Block shell operators when allow_shell=False
    if not allow_shell and _SHELL_OPERATOR_RE.search(command):
        return False, (
            "Command contains shell operators (|, &&, ||, ;) "
            "but allow_shell=False. "
            "Set allow_shell=True to enable shell features."
        )

    return True, ""


def _validate_shell_mode(command: str) -> tuple[bool, str]:
    """Validate if shell=True is necessary and safe.

    Returns:
        (allowed, reason) - allowed=True if shell mode is acceptable,
        reason explains why shell is needed or why it's denied.
    """
    # First check for injection patterns (always blocked)
    if _INJECTION_RE.search(command):
        return (
            False,
            "Command contains forbidden injection patterns ($(), ``, <(), >(), %0a, %00)",
        )

    # Check if command can be parsed for exec
    if _can_parse_for_exec(command):
        return False, "Command can be parsed for exec mode - shell=True not allowed"

    # Shell is needed for features like globbing, pipes, etc.
    # But we still need to validate it doesn't contain injection
    return True, "Shell mode required for complex command features"


async def bash_execute(
    command: str,
    timeout: float = 120.0,
    working_dir: str | None = None,
    env: dict[str, str] | None = None,
    *,
    confirm: bool = False,
    inherit_env: bool = False,
    workspace: str | Path | None = None,
    security_config: dict[str, Any] | None = None,
    allow_shell: bool = False,
) -> ToolResult:
    """
    Execute a bash command asynchronously.

    Args:
        command: The command to execute
        timeout: Timeout in seconds (default 120)
        working_dir: Working directory (default current)
        env: Extra environment variables (merged after safe defaults)
        confirm: Required True for DANGEROUS_COMMANDS tokens
        inherit_env: If True, inherit full parent env (default False = minimised)
        workspace: Workspace root for security enforcement of working_dir
        security_config: Explicit security components
        allow_shell: If True, allows shell features (pipes, chains, etc.).
                     Default False uses exec mode. Injection patterns ($(), ``,
                     <(), >(), %0a, %00) are always blocked.

    Returns:
        ToolResult with execution result

    Raises:
        ToolError: If command fails policy, fails to start, or times out.
    """
    call_id = generate_short_id()
    start_time = time.time()

    sec = resolve_security(workspace, security_config, "bash")

    # Command validation (injection + shell operators)
    is_valid, error = _validate_command(command, allow_shell)
    if not is_valid:
        record_audit(
            sec,
            AuditOperation.EXECUTE,
            command,
            success=False,
            details=f"validation error: {error}",
        )
        raise ToolError(call_id=call_id, name="bash", message=error)

    # Policy check (blocked/dangerous commands)
    try:
        blocked, dangerous = validate_command_tokens(command)
    except ToolError as e:
        e.call_id = call_id
        record_audit(
            sec,
            AuditOperation.EXECUTE,
            command,
            success=False,
            details=f"parse error: {e.message}",
        )
        raise

    if blocked:
        msg = f"Blocked command token(s): {', '.join(sorted(set(blocked)))}"
        record_audit(sec, AuditOperation.EXECUTE, command, success=False, details=msg)
        raise ToolError(call_id=call_id, name="bash", message=msg)

    if dangerous and not confirm:
        msg = (
            f"Dangerous command token(s) require confirm=True: "
            f"{', '.join(sorted(set(dangerous)))}"
        )
        raise ToolError(call_id=call_id, name="bash", message=msg)

    # Determine execution mode
    # allow_shell=True: use shell mode (pipes, chains allowed)
    # allow_shell=False: use exec mode (simple commands only)
    if allow_shell:
        # Shell mode already validated by _validate_command (no injection patterns)
        # Check if exec mode would work (for logging)
        shell_allowed, shell_reason = _validate_shell_mode(command)
        if not shell_allowed:
            # This shouldn't happen since _validate_command already passed,
            # but handle it defensively
            record_audit(
                sec,
                AuditOperation.EXECUTE,
                command,
                success=False,
                details=f"shell mode denied: {shell_reason}",
            )
            raise ToolError(
                call_id=call_id,
                name="bash",
                message=f"Shell mode denied: {shell_reason}",
            )
        logger.info(f"Shell mode enabled: {shell_reason}")

    # Resolve working directory (validated against workspace if configured)
    if working_dir is not None:
        if sec.enabled:
            cwd_path = enforce_path(
                sec,
                working_dir,
                Permission.READ,
                AuditOperation.EXECUTE,
                call_id,
                "bash",
            )
        else:
            cwd_path = Path(working_dir).expanduser().resolve()
    else:
        cwd_path = Path.cwd()

    if not cwd_path.exists():
        record_audit(
            sec,
            AuditOperation.EXECUTE,
            command,
            success=False,
            details=f"cwd not found: {cwd_path}",
        )
        raise ToolError(
            call_id=call_id,
            name="bash",
            message=f"Working directory not found: {cwd_path}",
        )

    exec_env = _build_safe_env(env, inherit_env)

    try:
        if allow_shell:
            proc = await asyncio.create_subprocess_shell(
                command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=str(cwd_path),
                env=exec_env,
            )
        else:
            args = shlex.split(command, posix=(os.name != "nt"))
            proc = await asyncio.create_subprocess_exec(
                args[0],
                *args[1:],
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=str(cwd_path),
                env=exec_env,
            )

        try:
            stdout, stderr = await asyncio.wait_for(
                proc.communicate(),
                timeout=timeout,
            )
        except asyncio.TimeoutError:
            proc.kill()
            record_audit(
                sec,
                AuditOperation.EXECUTE,
                command,
                success=False,
                details=f"timed out after {timeout}s",
            )
            raise ToolError(
                call_id=call_id,
                name="bash",
                message=f"Command timed out after {timeout}s",
            )

        duration_ms = int((time.time() - start_time) * 1000)

        stdout_str = stdout.decode("utf-8", errors="replace")
        stderr_str = stderr.decode("utf-8", errors="replace")

        record_audit(
            sec,
            AuditOperation.EXECUTE,
            command,
            success=(proc.returncode == 0),
            details=f"exit_code={proc.returncode}",
            metadata={
                "cwd": str(cwd_path),
                "exit_code": proc.returncode,
                "duration_ms": duration_ms,
            },
        )

        if proc.returncode != 0:
            content = f"Exit code: {proc.returncode}\n"
            if stderr_str:
                content += f"Error: {stderr_str}\n"
            if stdout_str:
                content += f"Output: {stdout_str}"
            return ToolResult(
                call_id=call_id,
                name="bash",
                content=content.strip(),
                is_error=True,
                duration_ms=duration_ms,
            )

        return ToolResult(
            call_id=call_id,
            name="bash",
            content=stdout_str or "(no output)",
            is_error=False,
            duration_ms=duration_ms,
        )

    except FileNotFoundError as e:
        record_audit(
            sec,
            AuditOperation.EXECUTE,
            command,
            success=False,
            details=f"command not found: {e}",
        )
        raise ToolError(
            call_id=call_id,
            name="bash",
            message=f"Command not found: {command.split()[0]}",
        ) from e
    except (
        subprocess.SubprocessError,
        OSError,
        PermissionError,
    ) as e:  # pragma: no cover - hard to trigger these errors in tests
        record_audit(
            sec, AuditOperation.EXECUTE, command, success=False, details=str(e)
        )
        raise ToolError(
            call_id=call_id,
            name="bash",
            message=f"Execution failed: {e}",
        ) from e


def bash_execute_sync(
    command: str,
    timeout: float = 120.0,
    working_dir: str | None = None,
    env: dict[str, str] | None = None,
    *,
    confirm: bool = False,
    inherit_env: bool = False,
    workspace: str | Path | None = None,
    security_config: dict[str, Any] | None = None,
    allow_shell: bool = False,
) -> ToolResult:
    """Execute a bash command synchronously."""
    return asyncio.run(
        bash_execute(
            command,
            timeout,
            working_dir,
            env,
            confirm=confirm,
            inherit_env=inherit_env,
            workspace=workspace,
            security_config=security_config,
            allow_shell=allow_shell,
        )
    )


class BashTool:
    """
    Bash tool wrapper for convenient usage.

    Example:
        >>> from continuum_sdk.tools import BashTool
        >>> bash = BashTool(workspace="/project")
        >>> result = bash.run("echo hello")
        >>> print(result.content)
        'hello'
    """

    def __init__(
        self,
        default_timeout: float = 120.0,
        default_working_dir: str | None = None,
        *,
        workspace: str | Path | None = None,
        security_config: dict[str, Any] | None = None,
        inherit_env: bool = False,
    ):
        self.default_timeout = default_timeout
        self.default_working_dir = default_working_dir
        self.workspace = workspace
        self.security_config = security_config
        self.inherit_env = inherit_env

    async def run_async(
        self,
        command: str,
        timeout: float | None = None,
        working_dir: str | None = None,
        env: dict[str, str] | None = None,
        *,
        confirm: bool = False,
        allow_shell: bool = False,
    ) -> ToolResult:
        return await bash_execute(
            command=command,
            timeout=timeout or self.default_timeout,
            working_dir=working_dir or self.default_working_dir,
            env=env,
            confirm=confirm,
            inherit_env=self.inherit_env,
            workspace=self.workspace,
            security_config=self.security_config,
            allow_shell=allow_shell,
        )

    def run(
        self,
        command: str,
        timeout: float | None = None,
        working_dir: str | None = None,
        env: dict[str, str] | None = None,
        *,
        confirm: bool = False,
        allow_shell: bool = False,
    ) -> ToolResult:
        return bash_execute_sync(
            command=command,
            timeout=timeout or self.default_timeout,
            working_dir=working_dir or self.default_working_dir,
            env=env,
            confirm=confirm,
            inherit_env=self.inherit_env,
            workspace=self.workspace,
            security_config=self.security_config,
            allow_shell=allow_shell,
        )

    def __call__(self, command: str, **kwargs) -> ToolResult:
        return self.run(command, **kwargs)
