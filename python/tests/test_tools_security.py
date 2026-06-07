"""
Tests for continuum_sdk.tools._security module.

This module provides tests for the security integration helpers
including SecurityContext, resolve_security, enforce_path, and record_audit.
"""

import logging
import os
import shutil
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from continuum_sdk.security import (
    AuditLogger,
    AuditOperation,
    AuditResult,
    PathValidator,
    Permission,
    PermissionChecker,
)
from continuum_sdk.tools._security import (
    SecurityContext,
    _WARNED_NO_WORKSPACE,
    enforce_path,
    record_audit,
    resolve_security,
)
from continuum_sdk.tools.types import ToolError


class TestSecurityContext:
    """Tests for SecurityContext dataclass."""

    def test_security_context_creation(self):
        """Test creating a SecurityContext instance."""
        validator = PathValidator(project_root="/tmp")
        checker = PermissionChecker()
        auditor = AuditLogger()

        ctx = SecurityContext(
            validator=validator,
            checker=checker,
            auditor=auditor,
            enforced=True,
        )

        assert ctx.validator is validator
        assert ctx.checker is checker
        assert ctx.auditor is auditor
        assert ctx.enforced is True

    def test_security_context_enabled_property(self):
        """Test the enabled property of SecurityContext."""
        # When enforced=True and validator is set
        ctx = SecurityContext(
            validator=PathValidator(project_root="/tmp"),
            checker=None,
            auditor=None,
            enforced=True,
        )
        assert ctx.enabled is True

    def test_security_context_disabled_when_not_enforced(self):
        """Test enabled is False when enforced=False."""
        ctx = SecurityContext(
            validator=PathValidator(project_root="/tmp"),
            checker=None,
            auditor=None,
            enforced=False,
        )
        assert ctx.enabled is False

    def test_security_context_disabled_when_validator_none(self):
        """Test enabled is False when validator is None."""
        ctx = SecurityContext(
            validator=None,
            checker=None,
            auditor=None,
            enforced=True,
        )
        assert ctx.enabled is False

    def test_security_context_enabled_requires_both(self):
        """Test enabled requires both enforced=True and validator set."""
        # Both conditions met -> enabled
        ctx1 = SecurityContext(
            validator=PathValidator(project_root="/tmp"),
            checker=None,
            auditor=None,
            enforced=True,
        )
        assert ctx1.enabled is True

        # Missing validator -> disabled
        ctx2 = SecurityContext(
            validator=None,
            checker=None,
            auditor=None,
            enforced=True,
        )
        assert ctx2.enabled is False

        # enforced=False -> disabled
        ctx3 = SecurityContext(
            validator=PathValidator(project_root="/tmp"),
            checker=None,
            auditor=None,
            enforced=False,
        )
        assert ctx3.enabled is False


class TestResolveSecurity:
    """Tests for resolve_security function."""

    @pytest.fixture(autouse=True)
    def reset_warning_cache(self):
        """Reset the warning cache before each test."""
        _WARNED_NO_WORKSPACE.clear()
        yield

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory for tests."""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_resolve_security_with_security_config(self, temp_dir):
        """Test resolve_security with security_config parameter."""
        validator = PathValidator(project_root=temp_dir)
        checker = PermissionChecker()
        auditor = AuditLogger()

        security_config = {
            "validator": validator,
            "checker": checker,
            "auditor": auditor,
        }

        ctx = resolve_security(
            workspace=None,
            security_config=security_config,
            tool_name="test_tool",
        )

        assert ctx.validator is validator
        assert ctx.checker is checker
        assert ctx.auditor is auditor
        assert ctx.enforced is True

    def test_resolve_security_with_security_config_partial(self):
        """Test resolve_security with partial security_config."""
        validator = PathValidator(project_root="/tmp")

        security_config = {
            "validator": validator,
            # No checker or auditor
        }

        ctx = resolve_security(
            workspace=None,
            security_config=security_config,
            tool_name="test_tool",
        )

        assert ctx.validator is validator
        assert ctx.checker is None
        assert ctx.auditor is None
        assert ctx.enforced is True

    def test_resolve_security_with_workspace_string(self, temp_dir):
        """Test resolve_security with workspace as string."""
        ctx = resolve_security(
            workspace=temp_dir,
            security_config=None,
            tool_name="test_tool",
        )

        assert ctx.validator is not None
        assert ctx.checker is not None
        assert ctx.auditor is not None
        assert ctx.enforced is True
        assert ctx.enabled is True

    def test_resolve_security_with_workspace_path(self, temp_dir):
        """Test resolve_security with workspace as Path."""
        ctx = resolve_security(
            workspace=Path(temp_dir),
            security_config=None,
            tool_name="test_tool",
        )

        assert ctx.validator is not None
        assert ctx.checker is not None
        assert ctx.auditor is not None
        assert ctx.enforced is True
        assert ctx.enabled is True

    def test_resolve_security_without_workspace_warns_once(self, caplog):
        """Test resolve_security warns once per tool when no workspace."""
        with caplog.at_level(logging.WARNING):
            # First call should warn
            ctx1 = resolve_security(
                workspace=None,
                security_config=None,
                tool_name="tool_a",
            )

            assert ctx1.validator is None
            assert ctx1.checker is None
            assert ctx1.auditor is None
            assert ctx1.enforced is False
            assert ctx1.enabled is False

            # Should have logged a warning
            assert any("security disabled for tool 'tool_a'" in r.message for r in caplog.records)

            caplog.clear()

            # Second call for same tool should not warn again
            ctx2 = resolve_security(
                workspace=None,
                security_config=None,
                tool_name="tool_a",
            )

            assert ctx2.enforced is False
            assert len(caplog.records) == 0

    def test_resolve_security_different_tools_separate_warnings(self, caplog):
        """Test different tools get separate warnings."""
        with caplog.at_level(logging.WARNING):
            resolve_security(
                workspace=None,
                security_config=None,
                tool_name="tool_x",
            )
            resolve_security(
                workspace=None,
                security_config=None,
                tool_name="tool_y",
            )

            # Both tools should have logged
            messages = [r.message for r in caplog.records]
            assert any("tool_x" in m for m in messages)
            assert any("tool_y" in m for m in messages)

    def test_resolve_security_security_config_takes_precedence(self, temp_dir):
        """Test security_config takes precedence over workspace."""
        custom_validator = PathValidator(project_root=temp_dir)

        security_config = {
            "validator": custom_validator,
        }

        ctx = resolve_security(
            workspace="/different/path",
            security_config=security_config,
            tool_name="test_tool",
        )

        # Should use the validator from security_config, not create new one
        assert ctx.validator is custom_validator


class TestEnforcePath:
    """Tests for enforce_path function."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory for tests."""
        dir_path = tempfile.mkdtemp()
        # Create a test file
        Path(dir_path, "test_file.txt").write_text("test content")
        yield dir_path
        shutil.rmtree(dir_path)

    @pytest.fixture
    def security_context(self, temp_dir):
        """Create a security context with all components."""
        return SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=PermissionChecker(),
            auditor=AuditLogger(),
            enforced=True,
        )

    @pytest.fixture
    def disabled_context(self):
        """Create a disabled security context."""
        return SecurityContext(
            validator=None,
            checker=None,
            auditor=None,
            enforced=False,
        )

    def test_enforce_path_disabled_context(self, disabled_context):
        """Test enforce_path with disabled security returns resolved path."""
        result = enforce_path(
            ctx=disabled_context,
            path="/some/arbitrary/path/file.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-123",
            tool_name="read_file",
        )

        assert result == Path("/some/arbitrary/path/file.txt").expanduser().resolve()

    def test_enforce_path_valid_path(self, security_context, temp_dir):
        """Test enforce_path with a valid path inside workspace."""
        test_file = Path(temp_dir, "test_file.txt")

        result = enforce_path(
            ctx=security_context,
            path="test_file.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-123",
            tool_name="read_file",
        )

        assert result == test_file.resolve()

    def test_enforce_path_absolute_path_inside_project(self, security_context, temp_dir):
        """Test enforce_path with absolute path inside project."""
        test_file = Path(temp_dir, "test_file.txt")

        result = enforce_path(
            ctx=security_context,
            path=str(test_file),
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-123",
            tool_name="read_file",
        )

        assert result == test_file.resolve()

    def test_enforce_path_path_validation_failure(self, security_context):
        """Test enforce_path raises ToolError on path validation failure."""
        with pytest.raises(ToolError) as exc_info:
            enforce_path(
                ctx=security_context,
                path="/etc/passwd",
                permission=Permission.READ,
                audit_op=AuditOperation.READ,
                call_id="call-456",
                tool_name="read_file",
            )

        assert exc_info.value.call_id == "call-456"
        assert exc_info.value.name == "read_file"
        assert "path validation failed" in exc_info.value.message

    def test_enforce_path_path_validation_failure_logs_audit(self, security_context):
        """Test enforce_path logs DENIED audit on path validation failure."""
        auditor = security_context.auditor

        with pytest.raises(ToolError):
            enforce_path(
                ctx=security_context,
                path="/etc/passwd",
                permission=Permission.READ,
                audit_op=AuditOperation.READ,
                call_id="call-789",
                tool_name="read_file",
            )

        # Check audit log
        records = auditor.query(result=AuditResult.DENIED)
        assert len(records) >= 1
        assert records[0].operation == AuditOperation.READ
        assert records[0].result == AuditResult.DENIED
        assert "path validation failed" in records[0].details

    def test_enforce_path_permission_denied(self, security_context, temp_dir):
        """Test enforce_path raises ToolError on permission denial."""
        # Create a file that's inside project but we'll mock permission check
        test_file = Path(temp_dir, "readonly.txt")
        test_file.write_text("readonly content")

        # Create a context with mocked checker that denies permission
        mock_checker = MagicMock()
        mock_result = MagicMock()
        mock_result.has_permission = False
        mock_result.exists = True
        mock_result.reason = "Mocked permission denial"
        mock_checker.check.return_value = mock_result

        ctx = SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=mock_checker,
            auditor=AuditLogger(),
            enforced=True,
        )

        with pytest.raises(ToolError) as exc_info:
            enforce_path(
                ctx=ctx,
                path="readonly.txt",
                permission=Permission.WRITE,
                audit_op=AuditOperation.WRITE,
                call_id="call-denied",
                tool_name="write_file",
            )

        assert exc_info.value.call_id == "call-denied"
        assert exc_info.value.name == "write_file"
        assert "permission denied" in exc_info.value.message.lower()

    def test_enforce_path_permission_denied_logs_audit(self, temp_dir):
        """Test enforce_path logs DENIED audit on permission denial."""
        mock_checker = MagicMock()
        mock_result = MagicMock()
        mock_result.has_permission = False
        mock_result.exists = True
        mock_result.reason = "Access denied by policy"
        mock_checker.check.return_value = mock_result

        auditor = AuditLogger()
        ctx = SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=mock_checker,
            auditor=auditor,
            enforced=True,
        )

        with pytest.raises(ToolError):
            enforce_path(
                ctx=ctx,
                path="some_file.txt",
                permission=Permission.WRITE,
                audit_op=AuditOperation.WRITE,
                call_id="call-perm-denied",
                tool_name="write_file",
            )

        records = auditor.query(result=AuditResult.DENIED)
        assert len(records) >= 1
        assert "permission check failed" in records[0].details

    def test_enforce_path_permission_create_nonexistent_file(self, security_context, temp_dir):
        """Test enforce_path with CREATE permission for nonexistent file."""
        # CREATE permission for nonexistent file should be allowed
        # (parent directory permission check)
        result = enforce_path(
            ctx=security_context,
            path="new_file.txt",
            permission=Permission.CREATE,
            audit_op=AuditOperation.CREATE,
            call_id="call-create",
            tool_name="create_file",
        )

        # Should succeed because parent directory is writable
        expected = (Path(temp_dir) / "new_file.txt").resolve()
        assert result == expected

    def test_enforce_path_permission_create_existing_file(self, security_context, temp_dir):
        """Test enforce_path with CREATE permission for existing file."""
        # Create a file
        test_file = Path(temp_dir, "existing.txt")
        test_file.write_text("existing content")

        # CREATE on existing file checks permission
        result = enforce_path(
            ctx=security_context,
            path="existing.txt",
            permission=Permission.CREATE,
            audit_op=AuditOperation.CREATE,
            call_id="call-create-existing",
            tool_name="create_file",
        )

        assert result == test_file.resolve()

    def test_enforce_path_resolved_path_from_validator(self, temp_dir):
        """Test enforce_path uses resolved_path from validator."""
        # Create a validator that returns a resolved path
        ctx = SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=None,  # No permission checker
            auditor=AuditLogger(),
            enforced=True,
        )

        result = enforce_path(
            ctx=ctx,
            path="subdir/file.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-resolved",
            tool_name="read_file",
        )

        expected = (Path(temp_dir) / "subdir" / "file.txt").resolve()
        assert result == expected

    def test_enforce_path_without_checker(self, temp_dir):
        """Test enforce_path works without a permission checker."""
        ctx = SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=None,
            auditor=AuditLogger(),
            enforced=True,
        )

        result = enforce_path(
            ctx=ctx,
            path="test.txt",
            permission=Permission.WRITE,
            audit_op=AuditOperation.WRITE,
            call_id="call-no-checker",
            tool_name="write_file",
        )

        assert result == (Path(temp_dir) / "test.txt").resolve()

    def test_enforce_path_without_auditor(self, temp_dir):
        """Test enforce_path works without an auditor."""
        mock_checker = MagicMock()
        mock_result = MagicMock()
        mock_result.has_permission = True
        mock_result.exists = True
        mock_checker.check.return_value = mock_result

        ctx = SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=mock_checker,
            auditor=None,
            enforced=True,
        )

        # Should not raise, even without auditor
        result = enforce_path(
            ctx=ctx,
            path="test.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-no-auditor",
            tool_name="read_file",
        )

        assert result == (Path(temp_dir) / "test.txt").resolve()

    def test_enforce_path_with_path_object(self, security_context, temp_dir):
        """Test enforce_path with Path object as input."""
        test_path = Path(temp_dir) / "test_file.txt"

        result = enforce_path(
            ctx=security_context,
            path=test_path,
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-path-obj",
            tool_name="read_file",
        )

        assert result == test_path.resolve()

    def test_enforce_path_expands_user(self, disabled_context):
        """Test enforce_path expands user home directory."""
        # With disabled context, path resolution should still work
        result = enforce_path(
            ctx=disabled_context,
            path="~/some_file.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-expand",
            tool_name="read_file",
        )

        # Result should be expanded (not start with ~)
        assert not str(result).startswith("~")
        assert "some_file.txt" in str(result)


class TestRecordAudit:
    """Tests for record_audit function."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory for tests."""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    @pytest.fixture
    def security_context(self, temp_dir):
        """Create a security context with auditor."""
        return SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=PermissionChecker(),
            auditor=AuditLogger(),
            enforced=True,
        )

    @pytest.fixture
    def context_without_auditor(self, temp_dir):
        """Create a security context without auditor."""
        return SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=PermissionChecker(),
            auditor=None,
            enforced=True,
        )

    def test_record_audit_success(self, security_context):
        """Test record_audit records a successful operation."""
        record_audit(
            ctx=security_context,
            audit_op=AuditOperation.WRITE,
            path="/test/file.py",
            success=True,
            details="File written successfully",
        )

        records = security_context.auditor.query(
            operation=AuditOperation.WRITE,
            result=AuditResult.SUCCESS,
        )

        assert len(records) == 1
        assert records[0].path == "/test/file.py"
        assert records[0].details == "File written successfully"

    def test_record_audit_failure(self, security_context):
        """Test record_audit records a failed operation."""
        record_audit(
            ctx=security_context,
            audit_op=AuditOperation.DELETE,
            path="/test/file.py",
            success=False,
            details="File not found",
        )

        records = security_context.auditor.query(
            operation=AuditOperation.DELETE,
            result=AuditResult.FAILURE,
        )

        assert len(records) == 1
        assert records[0].path == "/test/file.py"
        assert records[0].result == AuditResult.FAILURE

    def test_record_audit_with_metadata(self, security_context):
        """Test record_audit with additional metadata."""
        metadata = {
            "size": 1024,
            "mode": "overwrite",
        }

        record_audit(
            ctx=security_context,
            audit_op=AuditOperation.WRITE,
            path="/test/file.py",
            success=True,
            details="Written with metadata",
            metadata=metadata,
        )

        records = security_context.auditor.query(
            operation=AuditOperation.WRITE,
        )

        assert len(records) == 1
        assert records[0].metadata == metadata

    def test_record_audit_without_auditor(self, context_without_auditor):
        """Test record_audit does nothing when no auditor."""
        # Should not raise
        record_audit(
            ctx=context_without_auditor,
            audit_op=AuditOperation.READ,
            path="/test/file.py",
            success=True,
        )

        # No exception means success

    def test_record_audit_with_path_object(self, security_context, temp_dir):
        """Test record_audit with Path object."""
        test_path = Path(temp_dir) / "test.txt"

        record_audit(
            ctx=security_context,
            audit_op=AuditOperation.READ,
            path=test_path,
            success=True,
        )

        records = security_context.auditor.query(
            operation=AuditOperation.READ,
        )

        assert len(records) == 1
        assert records[0].path == str(test_path)

    def test_record_audit_none_details(self, security_context):
        """Test record_audit with None details."""
        record_audit(
            ctx=security_context,
            audit_op=AuditOperation.READ,
            path="/test/file.py",
            success=True,
            details=None,
        )

        records = security_context.auditor.query(
            operation=AuditOperation.READ,
        )

        assert len(records) == 1
        assert records[0].details is None

    def test_record_audit_none_metadata(self, security_context):
        """Test record_audit with None metadata."""
        record_audit(
            ctx=security_context,
            audit_op=AuditOperation.READ,
            path="/test/file.py",
            success=True,
            metadata=None,
        )

        records = security_context.auditor.query(
            operation=AuditOperation.READ,
        )

        assert len(records) == 1
        assert records[0].metadata == {}


class TestSecurityIntegration:
    """Integration tests for the full security pipeline."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory for tests."""
        dir_path = tempfile.mkdtemp()
        Path(dir_path, "existing.txt").write_text("content")
        yield dir_path
        shutil.rmtree(dir_path)

    def test_full_security_pipeline_success(self, temp_dir):
        """Test complete security pipeline for a valid operation."""
        # 1. Resolve security context
        ctx = resolve_security(
            workspace=temp_dir,
            security_config=None,
            tool_name="read_file",
        )

        assert ctx.enabled is True

        # 2. Enforce path
        resolved = enforce_path(
            ctx=ctx,
            path="existing.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-1",
            tool_name="read_file",
        )

        assert resolved.exists()

        # 3. Record audit
        record_audit(
            ctx=ctx,
            audit_op=AuditOperation.READ,
            path=resolved,
            success=True,
            details="File read successfully",
        )

        # 4. Verify audit log
        records = ctx.auditor.query(result=AuditResult.SUCCESS)
        assert len(records) == 1

    def test_security_pipeline_with_denied_path(self, temp_dir):
        """Test security pipeline blocks access to denied path."""
        # Create denied directory
        denied_dir = Path(temp_dir) / "blocked"
        denied_dir.mkdir()

        validator = PathValidator(
            project_root=temp_dir,
            denied_paths=["blocked"],
        )

        ctx = SecurityContext(
            validator=validator,
            checker=PermissionChecker(),
            auditor=AuditLogger(),
            enforced=True,
        )

        with pytest.raises(ToolError) as exc_info:
            enforce_path(
                ctx=ctx,
                path="blocked/file.txt",
                permission=Permission.READ,
                audit_op=AuditOperation.READ,
                call_id="call-blocked",
                tool_name="read_file",
            )

        assert "path validation failed" in exc_info.value.message

    def test_security_pipeline_disabled_no_workspace(self, caplog):
        """Test pipeline with disabled security (no workspace)."""
        _WARNED_NO_WORKSPACE.clear()

        with caplog.at_level(logging.WARNING):
            ctx = resolve_security(
                workspace=None,
                security_config=None,
                tool_name="test_tool",
            )

        assert ctx.enabled is False

        # Should still be able to resolve paths
        result = enforce_path(
            ctx=ctx,
            path="/any/path/file.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-1",
            tool_name="test_tool",
        )

        # Path is resolved but not validated
        assert result == Path("/any/path/file.txt").resolve()

    def test_security_pipeline_with_custom_config(self, temp_dir):
        """Test pipeline with custom security_config."""
        custom_validator = PathValidator(project_root=temp_dir)
        custom_auditor = AuditLogger()

        security_config = {
            "validator": custom_validator,
            "auditor": custom_auditor,
        }

        ctx = resolve_security(
            workspace=None,
            security_config=security_config,
            tool_name="custom_tool",
        )

        assert ctx.validator is custom_validator
        assert ctx.auditor is custom_auditor
        assert ctx.enforced is True


class TestEdgeCases:
    """Tests for edge cases and error handling."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory for tests."""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_enforce_path_empty_path(self, temp_dir):
        """Test enforce_path with empty path string."""
        ctx = resolve_security(
            workspace=temp_dir,
            security_config=None,
            tool_name="test",
        )

        # Empty path resolves to project root
        result = enforce_path(
            ctx=ctx,
            path="",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-1",
            tool_name="test",
        )

        assert result == Path(temp_dir).resolve()

    def test_enforce_path_with_special_characters(self, temp_dir):
        """Test enforce_path with special characters in path."""
        special_dir = Path(temp_dir) / "special chars & symbols!"
        special_dir.mkdir()

        ctx = resolve_security(
            workspace=temp_dir,
            security_config=None,
            tool_name="test",
        )

        result = enforce_path(
            ctx=ctx,
            path="special chars & symbols!/file.txt",
            permission=Permission.CREATE,
            audit_op=AuditOperation.CREATE,
            call_id="call-1",
            tool_name="test",
        )

        assert special_dir.name in str(result)

    def test_enforce_path_unicode(self, temp_dir):
        """Test enforce_path with unicode characters."""
        try:
            unicode_dir = Path(temp_dir) / "unicode"
            unicode_dir.mkdir()
            # Create the file so permission check passes
            unicode_file = unicode_dir / "test.txt"
            unicode_file.write_text("content")
        except OSError:
            pytest.skip("Unicode path creation not supported")

        ctx = resolve_security(
            workspace=temp_dir,
            security_config=None,
            tool_name="test",
        )

        result = enforce_path(
            ctx=ctx,
            path="unicode/test.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-1",
            tool_name="test",
        )

        assert result.exists()

    def test_multiple_audits_same_path(self, temp_dir):
        """Test multiple audit records for same path."""
        ctx = resolve_security(
            workspace=temp_dir,
            security_config=None,
            tool_name="test",
        )

        # Record multiple operations
        for i in range(3):
            record_audit(
                ctx=ctx,
                audit_op=AuditOperation.READ,
                path="/test/file.py",
                success=True,
                details=f"Operation {i+1}",
            )

        records = ctx.auditor.query(path="/test/file.py")
        assert len(records) == 3


if __name__ == "__main__":
    pytest.main([__file__, "-v"])


class TestMissingCoverage:
    """Tests for missing coverage branches in continuum_sdk.tools._security."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory for tests."""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_enforce_path_validation_failure_without_auditor(self, temp_dir):
        """Test enforce_path with validation failure and no auditor (line 113->120)."""
        validator = PathValidator(project_root=temp_dir)
        ctx = SecurityContext(
            validator=validator,
            checker=None,
            auditor=None,  # No auditor
            enforced=True,
        )

        with pytest.raises(ToolError) as exc_info:
            enforce_path(
                ctx=ctx,
                path="/etc/passwd",
                permission=Permission.READ,
                audit_op=AuditOperation.READ,
                call_id="call-123",
                tool_name="read_file",
            )

        assert "path validation failed" in exc_info.value.message

    def test_enforce_path_resolved_path_from_validator(self, temp_dir):
        """Test enforce_path uses resolved_path from validator (line 126->129)."""
        validator = PathValidator(project_root=temp_dir)
        ctx = SecurityContext(
            validator=validator,
            checker=None,
            auditor=None,
            enforced=True,
        )

        # Create a subdirectory
        subdir = Path(temp_dir) / "subdir"
        subdir.mkdir()

        result = enforce_path(
            ctx=ctx,
            path="subdir/file.txt",
            permission=Permission.READ,
            audit_op=AuditOperation.READ,
            call_id="call-123",
            tool_name="read_file",
        )

        # Should return the resolved path
        assert result == (subdir / "file.txt").resolve()

    def test_enforce_path_permission_denied_without_auditor(self, temp_dir):
        """Test enforce_path with permission denied and no auditor (line 133->140)."""
        mock_checker = MagicMock()
        mock_result = MagicMock()
        mock_result.has_permission = False
        mock_result.exists = True
        mock_result.reason = "Access denied by policy"
        mock_checker.check.return_value = mock_result

        ctx = SecurityContext(
            validator=PathValidator(project_root=temp_dir),
            checker=mock_checker,
            auditor=None,  # No auditor
            enforced=True,
        )

        with pytest.raises(ToolError) as exc_info:
            enforce_path(
                ctx=ctx,
                path="test.txt",
                permission=Permission.WRITE,
                audit_op=AuditOperation.WRITE,
                call_id="call-456",
                tool_name="write_file",
            )

        assert "permission denied" in exc_info.value.message.lower()
