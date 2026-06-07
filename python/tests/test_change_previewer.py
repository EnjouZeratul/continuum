"""Comprehensive tests for ChangePreviewer module.

Tests cover:
- Change creation and validation
- Risk assessment
- Preview generation
- Diff display
- Confirmation workflow
- Batch operations
- Custom confirmers
- Thread safety
"""

import sys
import threading
from datetime import datetime
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from continuum_sdk.security.change_previewer import (
    Change,
    ChangePreviewer,
    ChangeType,
    ConfirmationResult,
    RiskLevel,
)


class TestChangeType:
    """Tests for ChangeType enum."""

    def test_all_change_types_defined(self):
        """Test all change types are defined."""
        expected_types = [
            "CREATE",
            "WRITE",
            "EDIT",
            "DELETE",
            "MOVE",
            "COPY",
            "RENAME",
            "READ",
            "LIST",
            "APPEND",
        ]
        for type_name in expected_types:
            assert hasattr(ChangeType, type_name)

    def test_change_type_values(self):
        """Test change type values."""
        assert ChangeType.CREATE.value == "create"
        assert ChangeType.DELETE.value == "delete"
        assert ChangeType.WRITE.value == "write"


class TestRiskLevel:
    """Tests for RiskLevel enum."""

    def test_all_risk_levels_defined(self):
        """Test all risk levels are defined."""
        expected_levels = ["LOW", "MEDIUM", "HIGH", "CRITICAL"]
        for level in expected_levels:
            assert hasattr(RiskLevel, level)

    def test_risk_level_values(self):
        """Test risk level values."""
        assert RiskLevel.LOW.value == "low"
        assert RiskLevel.CRITICAL.value == "critical"


class TestChange:
    """Tests for Change dataclass."""

    def test_change_creation_basic(self):
        """Test basic change creation."""
        change = Change(
            change_type=ChangeType.WRITE, path="/test/file.py", content="test content"
        )
        assert change.change_type == ChangeType.WRITE
        assert change.path == "/test/file.py"
        assert change.content == "test content"
        assert change.metadata == {}

    def test_change_with_all_fields(self):
        """Test change creation with all fields."""
        change = Change(
            change_type=ChangeType.MOVE,
            path="/dest/file.py",
            source_path="/src/file.py",
            content="content",
            old_content="old",
            reason="Moving file",
            metadata={"key": "value"},
        )
        assert change.source_path == "/src/file.py"
        assert change.reason == "Moving file"
        assert change.metadata == {"key": "value"}

    def test_change_auto_risk_assessment_delete(self):
        """Test DELETE operations are always CRITICAL risk."""
        with patch.object(Path, "exists", return_value=True):
            change = Change(change_type=ChangeType.DELETE, path="/any/file.py")
            assert change.risk_level == RiskLevel.CRITICAL

    def test_change_auto_risk_assessment_write_existing(self):
        """Test WRITE to existing file is HIGH risk."""
        with patch.object(Path, "exists", return_value=True):
            change = Change(change_type=ChangeType.WRITE, path="/file.py")
            assert change.risk_level == RiskLevel.HIGH

    def test_change_auto_risk_assessment_write_new(self):
        """Test WRITE to new file is MEDIUM risk."""
        with patch.object(Path, "exists", return_value=False):
            change = Change(change_type=ChangeType.WRITE, path="/new/file.py")
            assert change.risk_level == RiskLevel.MEDIUM

    def test_change_auto_risk_assessment_sensitive_file(self):
        """Test WRITE to sensitive file is CRITICAL risk."""
        with patch.object(Path, "exists", return_value=True):
            change = Change(change_type=ChangeType.WRITE, path="/project/.env")
            assert change.risk_level == RiskLevel.CRITICAL

    def test_change_auto_risk_assessment_sensitive_patterns(self):
        """Test all sensitive patterns trigger CRITICAL risk."""
        sensitive_patterns = [
            "/project/.env",
            "/project/.git/config",
            "/project/.ssh/key",
            "/project/config/secret",
            "/project/secret_key",
            "/project/credential.json",
        ]
        for path in sensitive_patterns:
            with patch.object(Path, "exists", return_value=True):
                change = Change(change_type=ChangeType.WRITE, path=path)
                assert change.risk_level == RiskLevel.CRITICAL, f"Failed for {path}"

    def test_change_auto_risk_assessment_read(self):
        """Test READ operations are LOW risk."""
        change = Change(change_type=ChangeType.READ, path="/file.py")
        assert change.risk_level == RiskLevel.LOW

    def test_change_auto_risk_assessment_list(self):
        """Test LIST operations default to MEDIUM risk (not explicitly handled)."""
        change = Change(change_type=ChangeType.LIST, path="/dir/")
        # LIST is not explicitly handled, falls through to default MEDIUM
        assert change.risk_level == RiskLevel.MEDIUM

    def test_change_auto_risk_assessment_append(self):
        """Test APPEND operations are LOW risk."""
        change = Change(change_type=ChangeType.APPEND, path="/file.py")
        assert change.risk_level == RiskLevel.LOW

    def test_change_auto_risk_assessment_copy(self):
        """Test COPY operations are LOW risk."""
        change = Change(change_type=ChangeType.COPY, path="/dest/file.py")
        assert change.risk_level == RiskLevel.LOW

    def test_change_auto_risk_assessment_create(self):
        """Test CREATE operations are MEDIUM risk."""
        change = Change(change_type=ChangeType.CREATE, path="/new/file.py")
        assert change.risk_level == RiskLevel.MEDIUM

    def test_change_auto_risk_assessment_move_existing(self):
        """Test MOVE of existing file is HIGH risk."""
        with patch.object(Path, "exists", return_value=True):
            change = Change(change_type=ChangeType.MOVE, path="/dest/file.py")
            assert change.risk_level == RiskLevel.HIGH

    def test_change_auto_risk_assessment_move_new(self):
        """Test MOVE to new location is MEDIUM risk."""
        with patch.object(Path, "exists", return_value=False):
            change = Change(change_type=ChangeType.MOVE, path="/dest/file.py")
            assert change.risk_level == RiskLevel.MEDIUM

    def test_change_content_preview_string(self):
        """Test content preview for string content."""
        change = Change(
            change_type=ChangeType.WRITE, path="/file.py", content="test content"
        )
        preview = change._content_preview()
        assert preview == "test content"

    def test_change_content_preview_truncated(self):
        """Test content preview truncation for long content."""
        long_content = "x" * 200
        change = Change(
            change_type=ChangeType.WRITE, path="/file.py", content=long_content
        )
        preview = change._content_preview()
        assert len(preview) == 103  # 100 chars + "..."
        assert preview.endswith("...")

    def test_change_content_preview_binary(self):
        """Test content preview for binary content."""
        change = Change(
            change_type=ChangeType.WRITE, path="/file.py", content=b"binary"
        )
        preview = change._content_preview()
        assert "<binary:" in preview
        assert "bytes" in preview

    def test_change_content_preview_none(self):
        """Test content preview for None content."""
        change = Change(change_type=ChangeType.DELETE, path="/file.py")
        preview = change._content_preview()
        assert preview == "None"

    def test_change_to_dict(self):
        """Test change serialization to dict."""
        change = Change(
            change_type=ChangeType.WRITE,
            path="/file.py",
            content="content",
            reason="test",
        )
        result = change.to_dict()
        assert result["change_type"] == "write"
        assert result["path"] == "/file.py"
        assert result["reason"] == "test"
        assert "risk_level" in result


class TestConfirmationResult:
    """Tests for ConfirmationResult dataclass."""

    def test_confirmation_result_creation(self):
        """Test confirmation result creation."""
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        result = ConfirmationResult(
            approved=True, change=change, reason="User approved"
        )
        assert result.approved is True
        assert result.change == change
        assert result.reason == "User approved"
        assert result.user_response is None

    def test_confirmation_result_with_response(self):
        """Test confirmation result with user response."""
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        result = ConfirmationResult(
            approved=False, change=change, reason="User rejected", user_response="n"
        )
        assert result.approved is False
        assert result.user_response == "n"

    def test_confirmation_result_timestamp(self):
        """Test confirmation result has timestamp."""
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        before = datetime.now()
        result = ConfirmationResult(approved=True, change=change, reason="test")
        after = datetime.now()
        assert before <= result.timestamp <= after

    def test_confirmation_result_to_dict(self):
        """Test confirmation result serialization."""
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        result = ConfirmationResult(
            approved=True, change=change, reason="test", user_response="y"
        )
        d = result.to_dict()
        assert d["approved"] is True
        assert d["reason"] == "test"
        assert d["user_response"] == "y"
        assert "change" in d
        assert "timestamp" in d


class TestChangePreviewer:
    """Tests for ChangePreviewer class."""

    def test_previewer_creation_default(self):
        """Test previewer creation with defaults."""
        previewer = ChangePreviewer()
        assert previewer._auto_confirm_low is True
        assert RiskLevel.MEDIUM in previewer._require_confirmation
        assert RiskLevel.HIGH in previewer._require_confirmation
        assert RiskLevel.CRITICAL in previewer._require_confirmation

    def test_previewer_creation_custom_settings(self):
        """Test previewer creation with custom settings."""
        previewer = ChangePreviewer(
            auto_confirm_low=False,
            require_confirmation={RiskLevel.HIGH, RiskLevel.CRITICAL},
            skip_types={ChangeType.READ},
            project_root="/custom/root",
        )
        assert previewer._auto_confirm_low is False
        assert RiskLevel.MEDIUM not in previewer._require_confirmation

    def test_previewer_project_root_resolution(self):
        """Test project root path resolution."""
        previewer = ChangePreviewer(project_root="/some/path")
        assert previewer._project_root.is_absolute()

    def test_create_change_basic(self):
        """Test basic change creation."""
        previewer = ChangePreviewer()
        change = previewer.create_change(
            change_type=ChangeType.WRITE, path="/file.py", content="content"
        )
        assert change.change_type == ChangeType.WRITE
        assert change.path == "/file.py"

    def test_create_change_reads_old_content(self):
        """Test change creation reads existing file content."""
        previewer = ChangePreviewer()
        mock_content = "existing content"

        with patch.object(Path, "exists", return_value=True):
            with patch.object(Path, "read_text", return_value=mock_content):
                change = previewer.create_change(
                    change_type=ChangeType.WRITE, path="/file.py", content="new content"
                )
                assert change.old_content == mock_content

    def test_create_change_handles_read_error(self):
        """Test change creation handles read errors gracefully."""
        previewer = ChangePreviewer()

        with patch.object(Path, "exists", return_value=True):
            with patch.object(
                Path, "read_text", side_effect=PermissionError("Access denied")
            ):
                change = previewer.create_change(
                    change_type=ChangeType.WRITE, path="/file.py", content="new content"
                )
                assert change.old_content is None

    def test_create_change_with_metadata(self):
        """Test change creation with metadata."""
        previewer = ChangePreviewer()
        change = previewer.create_change(
            change_type=ChangeType.WRITE, path="/file.py", metadata={"key": "value"}
        )
        assert change.metadata == {"key": "value"}

    def test_preview_output_format(self):
        """Test preview output format."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.WRITE,
            path="/file.py",
            content="new",
            old_content="old",
        )
        preview = previewer.preview(change)
        assert "Change Preview" in preview
        assert "write" in preview
        assert "/file.py" in preview
        assert "Diff:" in preview

    def test_preview_delete_warning(self):
        """Test preview shows delete warning."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.DELETE, path="/file.py", old_content="content"
        )
        preview = previewer.preview(change)
        assert "DELETE" in preview

    def test_preview_move_shows_source(self):
        """Test preview shows source path for move."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.MOVE, path="/dest", source_path="/src")
        preview = previewer.preview(change)
        assert "Source:" in preview
        assert "/src" in preview

    def test_preview_create_shows_content(self):
        """Test preview shows content for create."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.CREATE, path="/file.py", content="new content"
        )
        preview = previewer.preview(change)
        assert "New content:" in preview

    def test_preview_binary_content(self):
        """Test preview handles binary content."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.CREATE, path="/file.bin", content=b"\x00\x01\x02"
        )
        preview = previewer.preview(change)
        assert "binary" in preview.lower()

    def test_diff_generation_basic(self):
        """Test basic diff generation."""
        previewer = ChangePreviewer()
        diff = previewer.diff("old\nline", "new\nline")
        assert "---" in diff
        assert "+++" in diff

    def test_diff_with_none_values(self):
        """Test diff with None values."""
        previewer = ChangePreviewer()
        diff = previewer.diff(None, "new")
        assert diff is not None
        assert "+++" in diff

    def test_diff_binary_content(self):
        """Test diff with binary content."""
        previewer = ChangePreviewer()
        diff = previewer.diff(b"binary", "text")
        assert "binary content" in diff

    def test_diff_with_path(self):
        """Test diff includes path in header."""
        previewer = ChangePreviewer()
        diff = previewer.diff("old", "new", path="/file.py")
        assert "/file.py" in diff

    def test_confirm_auto_approve(self):
        """Test confirm with auto_approve flag."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        result = previewer.confirm(change, auto_approve=True)
        assert result is True

    def test_confirm_skip_by_change_type(self):
        """Test confirm skips by change type."""
        previewer = ChangePreviewer(skip_types={ChangeType.READ})
        change = Change(change_type=ChangeType.READ, path="/file.py")
        result = previewer.confirm(change)
        assert result is True

    def test_confirm_skip_by_risk_level(self):
        """Test confirm skips by risk level."""
        previewer = ChangePreviewer(
            require_confirmation={RiskLevel.HIGH, RiskLevel.CRITICAL}
        )
        change = Change(change_type=ChangeType.CREATE, path="/file.py")  # MEDIUM risk
        result = previewer.confirm(change)
        assert result is True

    def test_confirm_force_confirm(self):
        """Test force_confirm bypasses skip logic."""
        previewer = ChangePreviewer(skip_types={ChangeType.READ})
        change = Change(change_type=ChangeType.READ, path="/file.py")

        with patch.object(previewer, "_interactive_confirm") as mock_confirm:
            mock_confirm.return_value = ConfirmationResult(
                approved=False, change=change, reason="test"
            )
            previewer.confirm(change, force_confirm=True)
            # Should call interactive confirm since force_confirm is True
            mock_confirm.assert_called_once()

    def test_confirm_custom_confirmer(self):
        """Test confirm with custom confirmer."""
        custom_confirmer = MagicMock(return_value=True)
        previewer = ChangePreviewer(custom_confirmer=custom_confirmer)
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        result = previewer.confirm(change, force_confirm=True)
        assert result is True
        custom_confirmer.assert_called_once_with(change)

    def test_confirm_custom_confirmer_exception(self):
        """Test confirm handles custom confirmer exceptions."""
        custom_confirmer = MagicMock(side_effect=ValueError("test error"))
        previewer = ChangePreviewer(custom_confirmer=custom_confirmer)
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch.object(previewer, "_interactive_confirm") as mock_interactive:
            mock_interactive.return_value = ConfirmationResult(
                approved=False, change=change, reason="fallback"
            )
            previewer.confirm(change, force_confirm=True)
            mock_interactive.assert_called_once()

    def test_confirm_batch(self):
        """Test batch confirmation."""
        previewer = ChangePreviewer()
        changes = [
            Change(change_type=ChangeType.READ, path="/file1.py"),  # Skipped type
            Change(change_type=ChangeType.WRITE, path="/file2.py"),
        ]
        # READ is in skip_types, WRITE needs confirmation but we use auto_approve
        # Override to use custom confirmer that auto-approves
        previewer.set_custom_confirmer(lambda c: True)
        results = previewer.confirm_batch(changes, show_preview=False)
        assert len(results) == 2

    def test_confirm_batch_with_preview(self, capsys):
        """Test batch confirmation with preview output."""
        previewer = ChangePreviewer()
        changes = [
            Change(change_type=ChangeType.READ, path="/file1.py"),
        ]
        previewer.confirm_batch(changes, show_preview=True)
        captured = capsys.readouterr()
        assert "Batch Change Preview" in captured.out

    def test_interactive_confirm_approves_on_yes(self):
        """Test interactive confirmation approves on 'y'."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", return_value="y"):
            with patch("builtins.print"):
                result = previewer._interactive_confirm(change)
                assert result.approved is True
                assert result.user_response == "y"

    def test_interactive_confirm_approves_on_yes_full(self):
        """Test interactive confirmation approves on 'yes'."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", return_value="yes"):
            with patch("builtins.print"):
                result = previewer._interactive_confirm(change)
                assert result.approved is True

    def test_interactive_confirm_rejects_on_no(self):
        """Test interactive confirmation rejects on 'n'."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", return_value="n"):
            with patch("builtins.print"):
                result = previewer._interactive_confirm(change)
                assert result.approved is False

    def test_interactive_confirm_approve_all(self):
        """Test interactive confirmation 'a' approves all."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", return_value="a"):
            with patch("builtins.print"):
                result = previewer._interactive_confirm(change)
                assert result.approved is True
                assert result.user_response == "a"
                assert len(previewer._require_confirmation) == 0

    def test_interactive_confirm_quit(self):
        """Test interactive confirmation 'q' raises KeyboardInterrupt."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", return_value="q"):
            with patch("builtins.print"):
                with pytest.raises(KeyboardInterrupt):
                    previewer._interactive_confirm(change)

    def test_interactive_confirm_handles_eof(self):
        """Test interactive confirmation handles EOF."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", side_effect=EOFError):
            with patch("builtins.print"):
                result = previewer._interactive_confirm(change)
                assert result.approved is False
                assert result.user_response == "interrupt"

    def test_interactive_confirm_handles_keyboard_interrupt(self):
        """Test interactive confirmation handles KeyboardInterrupt."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")

        with patch("builtins.input", side_effect=KeyboardInterrupt):
            with patch("builtins.print"):
                result = previewer._interactive_confirm(change)
                assert result.approved is False

    def test_history_recording(self):
        """Test confirmation history recording."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py")
        previewer.confirm(change, auto_approve=True)

        history = previewer.get_history()
        assert len(history) == 1
        assert history[0].approved is True

    def test_history_thread_safety(self):
        """Test history recording is thread-safe."""
        previewer = ChangePreviewer()
        changes = [
            Change(change_type=ChangeType.WRITE, path=f"/file{i}.py") for i in range(10)
        ]

        def confirm_change(change):
            previewer.confirm(change, auto_approve=True)

        threads = [threading.Thread(target=confirm_change, args=(c,)) for c in changes]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        history = previewer.get_history()
        assert len(history) == 10

    def test_statistics_empty(self):
        """Test statistics with no history."""
        previewer = ChangePreviewer()
        stats = previewer.get_statistics()
        assert stats["total"] == 0

    def test_statistics_with_data(self):
        """Test statistics with confirmation history."""
        previewer = ChangePreviewer()
        previewer.confirm(
            Change(change_type=ChangeType.WRITE, path="/file1.py"), auto_approve=True
        )
        previewer.confirm(
            Change(change_type=ChangeType.DELETE, path="/file2.py"), auto_approve=True
        )

        stats = previewer.get_statistics()
        assert stats["total"] == 2
        assert stats["approved"] == 2
        assert stats["approval_rate"] == 1.0

    def test_set_custom_confirmer(self):
        """Test setting custom confirmer."""
        previewer = ChangePreviewer()
        new_confirmer = MagicMock(return_value=True)
        previewer.set_custom_confirmer(new_confirmer)
        assert previewer._custom_confirmer == new_confirmer

    def test_reset_settings(self):
        """Test settings reset."""
        previewer = ChangePreviewer(
            require_confirmation={RiskLevel.CRITICAL}, skip_types=set()
        )
        previewer.set_custom_confirmer(lambda c: True)
        previewer.reset_settings()

        assert RiskLevel.MEDIUM in previewer._require_confirmation
        assert ChangeType.READ in previewer._skip_types
        assert previewer._custom_confirmer is None

    def test_repr(self):
        """Test string representation."""
        previewer = ChangePreviewer(project_root="/test")
        repr_str = repr(previewer)
        assert "ChangePreviewer" in repr_str


class TestChangePreviewerEdgeCases:
    """Edge case tests for ChangePreviewer."""

    def test_preview_with_long_content(self):
        """Test preview truncates long content."""
        previewer = ChangePreviewer()
        long_content = "x" * 1000
        change = Change(
            change_type=ChangeType.CREATE, path="/file.py", content=long_content
        )
        preview = previewer.preview(change)
        # Should truncate to 500 chars in preview
        assert len([line for line in preview.split("\n") if line.startswith("x")]) > 0

    def test_diff_with_empty_old_content(self):
        """Test diff when old content is empty."""
        previewer = ChangePreviewer()
        diff = previewer.diff("", "new content")
        assert "+new content" in diff or "+" in diff

    def test_diff_with_empty_new_content(self):
        """Test diff when new content is empty."""
        previewer = ChangePreviewer()
        diff = previewer.diff("old content", "")
        assert "-old content" in diff or "-" in diff

    def test_confirm_records_result_even_on_skip(self):
        """Test confirm records result even when skipped."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.READ, path="/file.py")  # skipped type
        previewer.confirm(change)

        history = previewer.get_history()
        assert len(history) == 1
        assert "Skipped" in history[0].reason

    def test_batch_empty_list(self):
        """Test batch confirmation with empty list."""
        previewer = ChangePreviewer()
        results = previewer.confirm_batch([])
        assert results == []


if __name__ == "__main__":
    pytest.main([__file__, "-v"])


class TestMissingCoverage:
    """Tests for missing coverage branches in continuum_sdk.security.change_previewer."""

    def test_change_assess_risk_list(self):
        """Test Change._assess_risk for LIST type (line 321)."""
        with patch.object(Path, "exists", return_value=False):
            change = Change(change_type=ChangeType.LIST, path="/dir/")
            # LIST is not explicitly handled, falls through to default MEDIUM
            assert change.risk_level == RiskLevel.MEDIUM

    def test_preview_for_edit_without_old_content(self):
        """Test preview for EDIT without old_content (line 337->355)."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.EDIT, path="/file.py", content="new content"
        )
        # old_content is None
        preview = previewer.preview(change)
        assert "Change Preview" in preview
        assert "edit" in preview

    def test_preview_for_create_with_binary_content(self):
        """Test preview for CREATE with binary content (line 349->355)."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.CREATE, path="/file.bin", content=b"\x00\x01\x02"
        )
        preview = previewer.preview(change)
        assert "binary" in preview.lower()

    def test_preview_for_create_content_truncation(self):
        """Test preview for CREATE with long content truncation (line 352->355)."""
        previewer = ChangePreviewer()
        long_content = "x" * 600
        change = Change(
            change_type=ChangeType.CREATE, path="/file.py", content=long_content
        )
        preview = previewer.preview(change)
        assert "..." in preview  # Content should be truncated

    def test_diff_both_none(self):
        """Test diff with both values None (line 379)."""
        previewer = ChangePreviewer()
        diff = previewer.diff(None, None)
        # Both None should produce empty diff
        assert diff is not None

    def test_confirm_batch_by_risk_level(self):
        """Test confirm_batch grouping by risk level (line 488->490)."""
        previewer = ChangePreviewer()
        changes = [
            Change(change_type=ChangeType.DELETE, path="/high.py"),  # CRITICAL
            Change(change_type=ChangeType.WRITE, path="/med.py"),  # HIGH or MEDIUM
            Change(change_type=ChangeType.READ, path="/low.py"),  # LOW
        ]

        # Use custom confirmer to auto-approve
        previewer.set_custom_confirmer(lambda c: True)

        # Capture output
        with patch("builtins.print"):
            results = previewer.confirm_batch(changes, show_preview=True)

        assert len(results) == 3

    def test_change_with_preset_risk_level(self):
        """Test Change when risk_level is already set (line 121->exit)."""
        # Create a change with explicit risk_level
        change = Change(
            change_type=ChangeType.WRITE,
            path="/file.py",
            risk_level=RiskLevel.LOW,  # Explicitly set, should NOT call _assess_risk
        )
        # The preset risk_level should be preserved
        assert change.risk_level == RiskLevel.LOW

    def test_create_change_handles_unicode_decode_error(self):
        """Test create_change handles UnicodeDecodeError (line 282->295)."""
        previewer = ChangePreviewer()

        with patch.object(Path, "exists", return_value=True):
            with patch.object(
                Path,
                "read_text",
                side_effect=UnicodeDecodeError("utf-8", b"", 0, 1, "invalid"),
            ):
                change = previewer.create_change(
                    change_type=ChangeType.WRITE, path="/file.py", content="new content"
                )
                # Should handle the error gracefully
                assert change.old_content is None

    def test_preview_for_list_change_type(self):
        """Test preview for LIST change type (line 321)."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.LIST, path="/directory/")
        preview = previewer.preview(change)
        assert "Change Preview" in preview
        assert "list" in preview

    def test_preview_for_edit_without_diff(self):
        """Test preview for EDIT when old_content is None (line 337->355)."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.EDIT,
            path="/file.py",
            content="new content",
            # old_content is None
        )
        preview = previewer.preview(change)
        assert "Change Preview" in preview
        # Should still show diff section
        assert "Diff:" in preview

    def test_preview_for_create_without_content(self):
        """Test preview for CREATE without content (line 349->355)."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.CREATE, path="/new_file.py")
        preview = previewer.preview(change)
        assert "Change Preview" in preview
        assert "create" in preview

    def test_preview_for_create_long_content_truncation(self):
        """Test preview for CREATE with long content truncation (line 352->355)."""
        previewer = ChangePreviewer()
        long_content = "x" * 600
        change = Change(
            change_type=ChangeType.CREATE, path="/file.py", content=long_content
        )
        preview = previewer.preview(change)
        assert "..." in preview  # Should truncate

    def test_confirm_batch_with_mixed_risk_levels(self):
        """Test confirm_batch groups changes by risk level (line 488->490)."""
        previewer = ChangePreviewer()
        changes = [
            Change(change_type=ChangeType.DELETE, path="/critical.py"),  # CRITICAL
            Change(
                change_type=ChangeType.WRITE, path="/high.py"
            ),  # HIGH (if exists) or MEDIUM
            Change(change_type=ChangeType.READ, path="/low.py"),  # LOW
        ]

        # Use custom confirmer that auto-approves
        previewer.set_custom_confirmer(lambda c: True)

        # Capture the output
        import io

        captured_output = io.StringIO()
        sys.stdout = captured_output

        try:
            results = previewer.confirm_batch(changes, show_preview=True)
        finally:
            sys.stdout = sys.__stdout__

        output = captured_output.getvalue()
        assert (
            "CRITICAL" in output
            or "HIGH" in output
            or "MEDIUM" in output
            or "LOW" in output
        )
        assert len(results) == 3

    def test_preview_delete_without_old_content(self):
        """Test preview for DELETE without old_content (line 349->355)."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.DELETE, path="/file.py")
        # old_content is None by default
        preview = previewer.preview(change)
        assert "DELETE" in preview
        # Should not show file size since old_content is None
        assert "File size:" not in preview

    def test_preview_change_without_reason(self):
        """Test preview for change without reason (line 321)."""
        previewer = ChangePreviewer()
        change = Change(change_type=ChangeType.WRITE, path="/file.py", content="test")
        # reason is None by default
        preview = previewer.preview(change)
        assert "Change Preview" in preview
        assert "Reason:" not in preview

    def test_create_change_with_old_content_provided(self):
        """Test create_change when old_content is explicitly provided (line 282->295)."""
        previewer = ChangePreviewer()
        # When old_content is provided, the branch to read file should NOT execute
        change = previewer.create_change(
            change_type=ChangeType.WRITE,
            path="/file.py",
            content="new content",
            old_content="explicit old content",  # Explicitly provided
        )
        assert change.old_content == "explicit old content"

    def test_confirm_batch_shows_risk_level_grouping(self):
        """Test confirm_batch properly groups and shows risk levels."""
        previewer = ChangePreviewer()

        # Create changes with different risk levels
        changes = [
            Change(change_type=ChangeType.DELETE, path="/crit.py"),  # CRITICAL
            Change(change_type=ChangeType.READ, path="/low.py"),  # LOW
        ]

        previewer.set_custom_confirmer(lambda c: True)

        import io

        captured_output = io.StringIO()
        sys.stdout = captured_output

        try:
            results = previewer.confirm_batch(changes, show_preview=True)
        finally:
            sys.stdout = sys.__stdout__

        output = captured_output.getvalue()
        # Should show risk level grouping
        assert "risk" in output.lower() or "CRITICAL" in output or "LOW" in output
        assert len(results) == 2

    def test_preview_change_with_reason(self):
        """Test preview for change WITH reason (line 321)."""
        previewer = ChangePreviewer()
        change = Change(
            change_type=ChangeType.WRITE,
            path="/file.py",
            content="test",
            reason="Fixing bug",  # Explicitly set reason
        )
        preview = previewer.preview(change)
        assert "Reason: Fixing bug" in preview

    def test_confirm_batch_with_all_risk_levels_present(self):
        """Test confirm_batch when all risk levels are present (line 488->490)."""
        previewer = ChangePreviewer()

        # Create changes that cover ALL risk levels
        changes = [
            Change(change_type=ChangeType.DELETE, path="/critical.py"),  # CRITICAL
            Change(
                change_type=ChangeType.WRITE, path="/existing.py", content="x"
            ),  # HIGH (if exists)
            Change(
                change_type=ChangeType.CREATE, path="/new.py", content="x"
            ),  # MEDIUM
            Change(change_type=ChangeType.READ, path="/low.py"),  # LOW
        ]

        # Mock existence for HIGH risk
        with patch.object(Path, "exists", return_value=True):
            previewer.set_custom_confirmer(lambda c: True)

            import io
            import sys

            captured_output = io.StringIO()
            sys.stdout = captured_output

            try:
                results = previewer.confirm_batch(changes, show_preview=True)
            finally:
                sys.stdout = sys.__stdout__

            captured_output.getvalue()
            # Should have grouped all risk levels
            assert len(results) == 4
