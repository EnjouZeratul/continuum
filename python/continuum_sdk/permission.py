"""Permission System

Interactive permission system for secure agent execution.

Provides capability-based security with:
- Interactive confirmation prompts
- Permission caching and "remember choice" functionality
- Security policy configuration
- Audit logging integration
"""

from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Any

# Try to import Rust bindings if available
try:
    from sh_python import (
        InteractivePermissionManager as RustInteractivePermissionManager,
    )
    from sh_python import (
        PermissionAction as RustPermissionAction,
    )
    from sh_python import (
        PermissionDecision as RustPermissionDecision,
    )
    from sh_python import (
        PermissionManager as RustPermissionManagerBinding,
    )
    from sh_python import (
        PermissionPolicy as RustPermissionPolicyBinding,
    )
    from sh_python import (
        SecurityLevel as RustSecurityLevel,
    )
    HAS_BINDING = True
except ImportError:
    HAS_BINDING = False
    RustPermissionManagerBinding = None
    RustPermissionPolicyBinding = None
    RustSecurityLevel = None
    RustPermissionDecision = None
    RustPermissionAction = None
    RustInteractivePermissionManager = None


class SecurityLevel(Enum):
    """Security level for permission policies."""

    TRUSTED = "trusted"
    STANDARD = "standard"
    STRICT = "strict"
    PARANOID = "paranoid"


class PermissionDecision(Enum):
    """Decision made for a permission request."""

    ALLOW = "allow"
    DENY = "deny"
    ALLOW_ONCE = "allow_once"
    DENY_ONCE = "deny_once"

    def is_allowed(self) -> bool:
        """Check if this decision allows the action."""
        return self in (PermissionDecision.ALLOW, PermissionDecision.ALLOW_ONCE)

    def should_remember(self) -> bool:
        """Check if this decision should be cached."""
        return self in (PermissionDecision.ALLOW, PermissionDecision.DENY)


@dataclass
class PermissionAction:
    """Action that requires permission."""

    action_type: str
    details: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def command_execute(cls, command: str, args: list[str] | None = None) -> PermissionAction:
        """Create a command execution action."""
        return cls(action_type="command_execute", details={"command": command, "args": args or []})

    @classmethod
    def file_read(cls, path: str) -> PermissionAction:
        """Create a file read action."""
        return cls(action_type="file_read", details={"path": path})

    @classmethod
    def file_write(cls, path: str, content_preview: str | None = None) -> PermissionAction:
        """Create a file write action."""
        return cls(action_type="file_write", details={"path": path, "content_preview": content_preview})

    @classmethod
    def file_delete(cls, path: str) -> PermissionAction:
        """Create a file delete action."""
        return cls(action_type="file_delete", details={"path": path})

    @classmethod
    def network_request(cls, url: str, method: str = "GET") -> PermissionAction:
        """Create a network request action."""
        return cls(action_type="network_request", details={"url": url, "method": method})

    @classmethod
    def env_access(cls, names: list[str]) -> PermissionAction:
        """Create an environment variable access action."""
        return cls(action_type="env_access", details={"names": names})

    @classmethod
    def package_install(cls, packages: list[str]) -> PermissionAction:
        """Create a package install action."""
        return cls(action_type="package_install", details={"packages": packages})

    @classmethod
    def custom(cls, description: str) -> PermissionAction:
        """Create a custom action."""
        return cls(action_type="custom", details={"description": description})

    def description(self) -> str:
        """Get a human-readable description of the action."""
        if self.action_type == "command_execute":
            cmd = self.details.get("command", "")
            args = " ".join(self.details.get("args", []))
            return f"Execute command: {cmd} {args}"
        elif self.action_type == "file_read":
            return f"Read file: {self.details.get('path', '')}"
        elif self.action_type == "file_write":
            return f"Write to file: {self.details.get('path', '')}"
        elif self.action_type == "file_delete":
            return f"Delete file: {self.details.get('path', '')}"
        elif self.action_type == "network_request":
            return f"{self.details.get('method', 'GET')} request to: {self.details.get('url', '')}"
        elif self.action_type == "env_access":
            return f"Access environment variables: {', '.join(self.details.get('names', []))}"
        elif self.action_type == "package_install":
            return f"Install packages: {', '.join(self.details.get('packages', []))}"
        elif self.action_type == "system_access":
            return f"Access system resource: {self.details.get('resource', '')}"
        else:
            return self.details.get("description", "Unknown action")


@dataclass
class PermissionRequest:
    """A request for permission."""

    id: str
    action: PermissionAction
    context: dict[str, Any] = field(default_factory=dict)
    batchable: bool = False
    timestamp: datetime = field(default_factory=datetime.utcnow)


@dataclass
class PermissionResponse:
    """Response to a permission request."""

    request_id: str
    decision: PermissionDecision
    reason: str | None = None
    timestamp: datetime = field(default_factory=datetime.utcnow)

    def is_allowed(self) -> bool:
        """Check if this response allows the action."""
        return self.decision in (PermissionDecision.ALLOW, PermissionDecision.ALLOW_ONCE)

    def should_remember(self) -> bool:
        """Check if this decision should be remembered."""
        return self.decision in (PermissionDecision.ALLOW, PermissionDecision.DENY)


@dataclass
class PermissionPolicy:
    """Security policy configuration."""

    level: SecurityLevel = SecurityLevel.STANDARD
    trusted_paths: list[str] = field(default_factory=list)
    blocked_paths: list[str] = field(default_factory=lambda: [
        ".env", ".env.local", "**/credentials.json", "**/secrets.json",
        "~/.ssh/id_rsa", "~/.ssh/id_ed25519", "/etc/shadow", "/etc/passwd",
    ])
    trusted_urls: list[str] = field(default_factory=list)
    blocked_urls: list[str] = field(default_factory=list)
    trusted_commands: list[str] = field(default_factory=list)
    blocked_commands: list[str] = field(default_factory=lambda: [
        "rm -rf /", "rm -rf ~", "mkfs", "dd if=/dev/zero",
    ])
    enable_cache: bool = True
    cache_expire_seconds: int = 3600
    audit_enabled: bool = True
    max_audit_entries: int = 10000

    @classmethod
    def trusted(cls) -> PermissionPolicy:
        """Create a trusted policy (no prompts)."""
        return cls(level=SecurityLevel.TRUSTED)

    @classmethod
    def standard(cls) -> PermissionPolicy:
        """Create a standard policy (prompt for dangerous actions)."""
        return cls(level=SecurityLevel.STANDARD)

    @classmethod
    def strict(cls) -> PermissionPolicy:
        """Create a strict policy (prompt for everything)."""
        return cls(level=SecurityLevel.STRICT)

    @classmethod
    def paranoid(cls) -> PermissionPolicy:
        """Create a paranoid policy (prompt for everything, log everything)."""
        return cls(level=SecurityLevel.PARANOID, audit_enabled=True)

    def is_path_blocked(self, path: str) -> bool:
        """Check if a path is blocked."""
        for blocked in self.blocked_paths:
            if path.startswith(blocked) or path == blocked:
                return True
        return False

    def is_path_trusted(self, path: str) -> bool:
        """Check if a path is trusted."""
        for trusted in self.trusted_paths:
            if path.startswith(trusted) or path == trusted:
                return True
        return False


class PermissionManager:
    """Central manager for the permission system."""

    def __init__(self, policy: PermissionPolicy | None = None):
        """Initialize the permission manager with the given policy."""
        self._policy = policy or PermissionPolicy()
        self._cache: dict[str, tuple[PermissionDecision, datetime]] = {}
        self._audit_log: list[dict[str, Any]] = []
        self._prompt_callback: Callable[[PermissionRequest], PermissionResponse] | None = None

    def set_policy(self, policy: PermissionPolicy) -> None:
        """Update the security policy."""
        self._policy = policy

    def get_policy(self) -> PermissionPolicy:
        """Get the current security policy."""
        return self._policy

    def set_prompt_callback(
        self, callback: Callable[[PermissionRequest], PermissionResponse]
    ) -> None:
        """Set the prompt callback for interactive mode."""
        self._prompt_callback = callback

    def clear_prompt_callback(self) -> None:
        """Clear the prompt callback."""
        self._prompt_callback = None

    def check_permission(self, request: PermissionRequest) -> PermissionResponse:
        """Check if an action is allowed, prompting if necessary."""

        # Check if path is blocked
        if request.action.action_type in ("file_read", "file_write", "file_delete"):
            path = request.action.details.get("path", "")
            if self._policy.is_path_blocked(path):
                return PermissionResponse(
                    request_id=request.id,
                    decision=PermissionDecision.DENY,
                    reason=f"Path '{path}' is blocked by security policy",
                )

        # Check if command is blocked
        if request.action.action_type == "command_execute":
            command = request.action.details.get("command", "")
            for blocked in self._policy.blocked_commands:
                if command.startswith(blocked) or command == blocked:
                    return PermissionResponse(
                        request_id=request.id,
                        decision=PermissionDecision.DENY,
                        reason=f"Command '{command}' is blocked by security policy",
                    )

        # Check if URL is blocked
        if request.action.action_type == "network_request":
            url = request.action.details.get("url", "")
            for blocked in self._policy.blocked_urls:
                if blocked in url:
                    return PermissionResponse(
                        request_id=request.id,
                        decision=PermissionDecision.DENY,
                        reason=f"URL '{url}' is blocked by security policy",
                    )

        # Auto-approve for trusted level
        if self._policy.level == SecurityLevel.TRUSTED:
            response = PermissionResponse(request_id=request.id, decision=PermissionDecision.ALLOW)
            self._log_audit(request, response, from_cache=False)
            return response

        # Check cache
        if self._policy.enable_cache:
            cache_key = self._cache_key(request.action)
            if cache_key in self._cache:
                decision, cached_at = self._cache[cache_key]
                age = (datetime.utcnow() - cached_at).total_seconds()
                if age < self._policy.cache_expire_seconds:
                    response = PermissionResponse(
                        request_id=request.id,
                        decision=decision,
                        reason="From cache",
                    )
                    self._log_audit(request, response, from_cache=True)
                    return response

        # Prompt user if callback is set
        if self._prompt_callback:
            response = self._prompt_callback(request)

            # Cache if should remember
            if response.should_remember() and self._policy.enable_cache:
                cache_key = self._cache_key(request.action)
                self._cache[cache_key] = (response.decision, datetime.utcnow())

            # Log audit
            self._log_audit(request, response, from_cache=False)

            return response
        else:
            # No callback - deny in non-trusted mode
            # Note: TRUSTED level is handled earlier at lines 287-291, so we only reach here
            # for non-TRUSTED levels, which require a callback.
            raise RuntimeError("No permission prompt callback configured")

    def _cache_key(self, action: PermissionAction) -> str:
        """Generate cache key for an action."""
        if action.action_type == "command_execute":
            return f"cmd:{action.details.get('command', '')}"
        elif action.action_type == "file_read":
            return f"read:{action.details.get('path', '')}"
        elif action.action_type == "file_write":
            return f"write:{action.details.get('path', '')}"
        elif action.action_type == "file_delete":
            return f"delete:{action.details.get('path', '')}"
        elif action.action_type == "network_request":
            return f"net:{action.details.get('method', '')}:{action.details.get('url', '')}"
        else:
            return f"{action.action_type}:{json.dumps(action.details, sort_keys=True)}"

    def _log_audit(
        self, request: PermissionRequest, response: PermissionResponse, from_cache: bool
    ) -> None:
        """Log an audit entry."""
        if not self._policy.audit_enabled:
            return

        entry = {
            "id": request.id,
            "action": request.action.action_type,
            "action_details": request.action.details,
            "decision": response.decision.value,
            "reason": response.reason,
            "from_cache": from_cache,
            "timestamp": datetime.utcnow().isoformat(),
        }

        self._audit_log.append(entry)

        # Trim if exceeds max
        if len(self._audit_log) > self._policy.max_audit_entries:
            excess = len(self._audit_log) - self._policy.max_audit_entries
            self._audit_log = self._audit_log[excess:]

    def get_audit_log(self) -> list[dict[str, Any]]:
        """Get audit log entries."""
        return self._audit_log.copy()

    def clear_audit_log(self) -> None:
        """Clear audit log."""
        self._audit_log.clear()

    def clear_cache(self) -> None:
        """Clear permission cache."""
        self._cache.clear()

    def cache_stats(self) -> tuple[int, int]:
        """Get cache statistics (total entries, valid entries)."""
        return (len(self._cache), len(self._cache))

    # Convenience methods for creating requests
    def request_command(self, command: str, args: list[str] = None) -> PermissionRequest:
        """Create a permission request for command execution."""
        import uuid

        return PermissionRequest(
            id=str(uuid.uuid4()),
            action=PermissionAction(
                action_type="command_execute",
                details={"command": command, "args": args or []},
            ),
        )

    def request_file_read(self, path: str) -> PermissionRequest:
        """Create a permission request for file read."""
        import uuid

        return PermissionRequest(
            id=str(uuid.uuid4()),
            action=PermissionAction(action_type="file_read", details={"path": path}),
        )

    def request_file_write(self, path: str, content_preview: str | None = None) -> PermissionRequest:
        """Create a permission request for file write."""
        import uuid

        return PermissionRequest(
            id=str(uuid.uuid4()),
            action=PermissionAction(
                action_type="file_write",
                details={"path": path, "content_preview": content_preview},
            ),
        )

    def request_file_delete(self, path: str) -> PermissionRequest:
        """Create a permission request for file delete."""
        import uuid

        return PermissionRequest(
            id=str(uuid.uuid4()),
            action=PermissionAction(action_type="file_delete", details={"path": path}),
        )

    def request_network(self, url: str, method: str = "GET") -> PermissionRequest:
        """Create a permission request for network request."""
        import uuid

        return PermissionRequest(
            id=str(uuid.uuid4()),
            action=PermissionAction(
                action_type="network_request",
                details={"url": url, "method": method},
            ),
        )


__all__ = [
    "SecurityLevel",
    "PermissionDecision",
    "PermissionAction",
    "PermissionRequest",
    "PermissionResponse",
    "PermissionPolicy",
    "PermissionManager",
    "InteractivePermissionManager",  # Alias for PermissionManager
    "HAS_BINDING",
]

# Alias for clarity
InteractivePermissionManager = PermissionManager
