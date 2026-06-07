"""Tests for task completion detection module."""


import tempfile
from datetime import datetime
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

from continuum_sdk.agent.task_completion import (
    CompletionMarker,
    CompletionStatus,
    TaskCompletionDetector,
    TaskRecord,
)


class TestCompletionMarker:
    """CompletionMarker enum tests."""

    def test_marker_values(self):
        """Test marker enum values."""
        assert CompletionMarker.TASK_COMPLETED.value == "TASK_COMPLETED"
        assert CompletionMarker.USER_INTERRUPTED.value == "USER_INTERRUPTED"
        assert CompletionMarker.IN_PROGRESS.value == "IN_PROGRESS"
        assert CompletionMarker.FAILED.value == "FAILED"
        assert CompletionMarker.NEEDS_CLARIFICATION.value == "NEEDS_CLARIFICATION"


class TestCompletionStatus:
    """CompletionStatus dataclass tests."""

    def test_creation(self):
        """Test creating a CompletionStatus."""
        status = CompletionStatus(
            marker=CompletionMarker.TASK_COMPLETED,
            is_completed=True,
            confidence=0.95,
            reason="Task finished successfully",
        )
        assert status.marker == CompletionMarker.TASK_COMPLETED
        assert status.is_completed is True
        assert status.confidence == 0.95
        assert status.reason == "Task finished successfully"
        assert status.suggestions == []

    def test_with_suggestions(self):
        """Test CompletionStatus with suggestions."""
        status = CompletionStatus(
            marker=CompletionMarker.NEEDS_CLARIFICATION,
            is_completed=False,
            confidence=0.6,
            reason="Partial completion",
            suggestions=["Add more tests", "Fix edge case"],
        )
        assert len(status.suggestions) == 2

    def test_to_dict(self):
        """Test serialization to dict."""
        status = CompletionStatus(
            marker=CompletionMarker.TASK_COMPLETED,
            is_completed=True,
            confidence=0.9,
            reason="Done",
            suggestions=["verify"],
        )
        data = status.to_dict()
        assert data["marker"] == "TASK_COMPLETED"
        assert data["is_completed"] is True
        assert data["confidence"] == 0.9
        assert data["reason"] == "Done"
        assert data["suggestions"] == ["verify"]
        assert "timestamp" in data

    def test_from_dict(self):
        """Test deserialization from dict."""
        data = {
            "marker": "IN_PROGRESS",
            "is_completed": False,
            "confidence": 0.5,
            "reason": "Working on it",
            "suggestions": [],
            "timestamp": datetime.now().isoformat(),
        }
        status = CompletionStatus.from_dict(data)
        assert status.marker == CompletionMarker.IN_PROGRESS
        assert status.is_completed is False
        assert status.confidence == 0.5


class TestTaskRecord:
    """TaskRecord dataclass tests."""

    def test_creation(self):
        """Test creating a TaskRecord."""
        status = CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.0,
        )
        record = TaskRecord(
            task_id="abc123",
            description="Fix the bug",
            status=status,
        )
        assert record.task_id == "abc123"
        assert record.description == "Fix the bug"
        assert record.attempts == 0
        assert record.result is None

    def test_to_dict_and_from_dict(self):
        """Test TaskRecord serialization."""
        status = CompletionStatus(
            marker=CompletionMarker.TASK_COMPLETED,
            is_completed=True,
            confidence=1.0,
            reason="Done",
        )
        record = TaskRecord(
            task_id="test-001",
            description="Test task",
            status=status,
            session_id="session-001",
            attempts=2,
            result="All tests passed",
        )

        data = record.to_dict()
        loaded = TaskRecord.from_dict(data)

        assert loaded.task_id == "test-001"
        assert loaded.description == "Test task"
        assert loaded.status.marker == CompletionMarker.TASK_COMPLETED
        assert loaded.session_id == "session-001"
        assert loaded.attempts == 2
        assert loaded.result == "All tests passed"


class TestTaskCompletionDetector:
    """TaskCompletionDetector tests."""

    @pytest.fixture
    def temp_persistence_path(self):
        """Create temp directory for persistence."""
        with tempfile.TemporaryDirectory() as tmpdir:
            yield Path(tmpdir)

    def test_initialization(self, temp_persistence_path):
        """Test detector initialization."""
        detector = TaskCompletionDetector(
            confidence_threshold=0.8,
            persistence_path=temp_persistence_path,
        )
        assert detector.confidence_threshold == 0.8
        assert detector._persistence_path == temp_persistence_path

    @pytest.mark.asyncio
    async def test_rule_based_check_completed(self, temp_persistence_path):
        """Test rule-based check detects completion."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )
        status = await detector.check_completion(
            task="Add logging to auth.py",
            result="Added logging statements to auth.py lines 15, 23, and 45",
        )
        assert status.marker == CompletionMarker.TASK_COMPLETED
        assert status.is_completed is True
        assert status.confidence >= 0.7

    @pytest.mark.asyncio
    async def test_rule_based_check_failed(self, temp_persistence_path):
        """Test rule-based check detects failure."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )
        status = await detector.check_completion(
            task="Fix the bug",
            result="Error: could not find the file. Failed to complete.",
        )
        assert status.marker == CompletionMarker.FAILED
        assert status.is_completed is False

    @pytest.mark.asyncio
    async def test_rule_based_check_in_progress(self, temp_persistence_path):
        """Test rule-based check returns in_progress for unclear cases."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )
        status = await detector.check_completion(
            task="Implement feature X",
            result="Started working on it",
        )
        assert status.marker == CompletionMarker.IN_PROGRESS
        assert status.is_completed is False

    @pytest.mark.asyncio
    async def test_task_persistence(self, temp_persistence_path):
        """Test that tasks are persisted to disk."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )
        await detector.check_completion(
            task="Test task",
            result="Completed successfully",
            task_id="test-001",
        )

        # Check file exists
        task_file = temp_persistence_path / "test-001.json"
        assert task_file.exists()

        # Verify can load
        loaded = detector._load_task("test-001")
        assert loaded is not None
        assert loaded.task_id == "test-001"

    def test_mark_completed(self, temp_persistence_path):
        """Test manually marking task as completed."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Create a task record first
        status = CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.0,
        )
        record = TaskRecord(
            task_id="manual-001",
            description="Manual test",
            status=status,
        )
        detector._tasks["manual-001"] = record

        detector.mark_completed("manual-001", "Manually approved")

        updated = detector.get_task("manual-001")
        assert updated.status.marker == CompletionMarker.TASK_COMPLETED
        assert updated.status.is_completed is True

    def test_mark_interrupted(self, temp_persistence_path):
        """Test marking task as user-interrupted."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        status = CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.0,
        )
        record = TaskRecord(
            task_id="interrupt-001",
            description="Test interruption",
            status=status,
        )
        detector._tasks["interrupt-001"] = record

        detector.mark_interrupted("interrupt-001", "User cancelled")

        updated = detector.get_task("interrupt-001")
        assert updated.status.marker == CompletionMarker.USER_INTERRUPTED
        assert updated.status.is_completed is False

    def test_get_pending_tasks(self, temp_persistence_path):
        """Test getting pending tasks."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Add some tasks
        for i in range(3):
            status = CompletionStatus(
                marker=(
                    CompletionMarker.IN_PROGRESS
                    if i < 2
                    else CompletionMarker.TASK_COMPLETED
                ),
                is_completed=i >= 2,
                confidence=0.0,
            )
            record = TaskRecord(
                task_id=f"task-{i}",
                description=f"Task {i}",
                status=status,
            )
            detector._tasks[f"task-{i}"] = record

        pending = detector.get_pending_tasks()
        assert len(pending) == 2

    def test_clear_completed_tasks(self, temp_persistence_path):
        """Test clearing completed tasks."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Add completed and pending tasks
        for marker, is_completed in [
            (CompletionMarker.TASK_COMPLETED, True),
            (CompletionMarker.IN_PROGRESS, False),
            (CompletionMarker.TASK_COMPLETED, True),
        ]:
            status = CompletionStatus(
                marker=marker,
                is_completed=is_completed,
                confidence=0.0,
            )
            task_id = f"task-{len(detector._tasks)}"
            record = TaskRecord(
                task_id=task_id,
                description=f"Task {task_id}",
                status=status,
            )
            detector._tasks[task_id] = record

        cleared = detector.clear_completed_tasks()
        assert cleared == 2
        assert len(detector._tasks) == 1

    def test_load_all_tasks(self, temp_persistence_path):
        """Test loading all tasks from disk."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Create some task files
        for i in range(3):
            status = CompletionStatus(
                marker=CompletionMarker.IN_PROGRESS,
                is_completed=False,
                confidence=0.0,
            )
            record = TaskRecord(
                task_id=f"loaded-{i}",
                description=f"Loaded task {i}",
                status=status,
            )
            detector._persist_task(record)

        # Clear memory and reload
        detector._tasks.clear()
        assert len(detector._tasks) == 0

        detector.load_all_tasks()
        assert len(detector._tasks) == 3

    @pytest.mark.asyncio
    async def test_llm_check_completed(self, temp_persistence_path):
        """Test LLM-based check returns completed."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.content = '{"complete": "yes", "confidence": 0.95, "reason": "Task completed", "suggestions": []}'
        mock_client.chat = AsyncMock(return_value=mock_response)

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Fix the bug",
            result="Fixed the null pointer in line 42",
        )
        assert status.marker == CompletionMarker.TASK_COMPLETED
        assert status.is_completed is True

    @pytest.mark.asyncio
    async def test_llm_check_partial(self, temp_persistence_path):
        """Test LLM check returns partial completion."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.content = '{"complete": "partial", "confidence": 0.6, "reason": "Only some parts done", "suggestions": ["Add tests"]}'
        mock_client.chat = AsyncMock(return_value=mock_response)

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Implement feature X",
            result="Implemented basic functionality",
        )
        assert status.marker == CompletionMarker.NEEDS_CLARIFICATION
        assert status.is_completed is False
        assert "Add tests" in status.suggestions

    @pytest.mark.asyncio
    async def test_llm_check_fails_gracefully(self, temp_persistence_path):
        """Test that LLM failure falls back gracefully."""
        from continuum_sdk.llm.errors import LlmError

        mock_client = MagicMock()
        # 使用具体的 LLM 异常类型，而不是通用 Exception
        mock_client.chat = AsyncMock(side_effect=LlmError("API error", provider="test"))

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Do something",
            result="Did it",
        )
        # Should fall back to rule-based
        assert status.marker in [CompletionMarker.IN_PROGRESS, CompletionMarker.FAILED]

    @pytest.mark.asyncio
    async def test_llm_invalid_json_fallback(self, temp_persistence_path):
        """Test handling of invalid LLM JSON response."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.content = "This is not valid JSON"
        mock_client.chat = AsyncMock(return_value=mock_response)

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Task",
            result="Result",
        )
        assert status.marker == CompletionMarker.IN_PROGRESS
        assert "Failed to parse" in status.reason

    @pytest.mark.asyncio
    async def test_confidence_threshold(self, temp_persistence_path):
        """Test that low confidence doesn't mark as complete."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.content = '{"complete": "yes", "confidence": 0.5, "reason": "Maybe done", "suggestions": []}'
        mock_client.chat = AsyncMock(return_value=mock_response)

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            confidence_threshold=0.8,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Task",
            result="Result",
        )
        # Below threshold, so not marked complete even if LLM says yes
        assert status.is_completed is False

    @pytest.mark.asyncio
    async def test_llm_check_no_completion(self, temp_persistence_path):
        """Test LLM check returns no completion."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.content = '{"complete": "no", "confidence": 0.3, "reason": "Not done yet", "suggestions": ["Keep working"]}'
        mock_client.chat = AsyncMock(return_value=mock_response)

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Do something",
            result="Started working",
        )
        assert status.marker == CompletionMarker.IN_PROGRESS
        assert status.is_completed is False
        assert status.confidence == 0.3
        assert "Keep working" in status.suggestions

    @pytest.mark.asyncio
    async def test_llm_unexpected_exception_fallback(self, temp_persistence_path):
        """Test that unexpected exceptions fall back gracefully."""
        mock_client = MagicMock()
        # Use a generic exception to test the broad except clause
        mock_client.chat = AsyncMock(side_effect=RuntimeError("Unexpected error"))

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Do something",
            result="Did it",
        )
        assert status.marker == CompletionMarker.IN_PROGRESS
        assert status.is_completed is False
        assert "unexpected error" in status.reason.lower()

    @pytest.mark.asyncio
    async def test_llm_json_decode_error_in_parse(self, temp_persistence_path):
        """Test handling of malformed JSON in LLM response."""
        mock_client = MagicMock()
        mock_response = MagicMock()
        # JSON with invalid value type for confidence
        mock_response.content = '{"complete": "yes", "confidence": "not_a_number", "reason": "Bad JSON"}'
        mock_client.chat = AsyncMock(return_value=mock_response)

        detector = TaskCompletionDetector(
            llm_client=mock_client,
            persistence_path=temp_persistence_path,
        )

        status = await detector.check_completion(
            task="Task",
            result="Result",
        )
        assert status.marker == CompletionMarker.IN_PROGRESS
        assert "Failed to parse" in status.reason

    def test_mark_completed_nonexistent_task(self, temp_persistence_path):
        """Test marking a non-existent task as completed does nothing."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Should not raise, just do nothing
        detector.mark_completed("nonexistent-task", "Should be ignored")
        assert detector.get_task("nonexistent-task") is None

    def test_mark_interrupted_nonexistent_task(self, temp_persistence_path):
        """Test marking a non-existent task as interrupted does nothing."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Should not raise, just do nothing
        detector.mark_interrupted("nonexistent-task", "Should be ignored")
        assert detector.get_task("nonexistent-task") is None

    def test_load_task_nonexistent(self, temp_persistence_path):
        """Test loading a non-existent task returns None."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        result = detector._load_task("nonexistent-task")
        assert result is None

    def test_load_all_tasks_handles_corrupted_files(self, temp_persistence_path):
        """Test that corrupted task files are skipped during load."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        # Create a valid task file
        status = CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.0,
        )
        valid_record = TaskRecord(
            task_id="valid-task",
            description="Valid task",
            status=status,
        )
        detector._persist_task(valid_record)

        # Create a corrupted JSON file
        corrupted_file = temp_persistence_path / "corrupted.json"
        with open(corrupted_file, "w", encoding="utf-8") as f:
            f.write("{ this is not valid json }")

        # Create a file with missing required keys
        missing_keys_file = temp_persistence_path / "missing_keys.json"
        with open(missing_keys_file, "w", encoding="utf-8") as f:
            f.write('{"some_key": "some_value"}')

        # Clear memory and reload
        detector._tasks.clear()
        detector.load_all_tasks()

        # Only the valid task should be loaded
        assert len(detector._tasks) == 1
        assert "valid-task" in detector._tasks

    def test_mark_completed_without_reason(self, temp_persistence_path):
        """Test marking task completed without providing a reason."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        status = CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.0,
        )
        record = TaskRecord(
            task_id="no-reason-001",
            description="Test without reason",
            status=status,
        )
        detector._tasks["no-reason-001"] = record

        detector.mark_completed("no-reason-001")

        updated = detector.get_task("no-reason-001")
        assert updated.status.marker == CompletionMarker.TASK_COMPLETED
        assert updated.status.is_completed is True
        assert "Manually marked" in updated.status.reason

    def test_mark_interrupted_without_reason(self, temp_persistence_path):
        """Test marking task interrupted without providing a reason."""
        detector = TaskCompletionDetector(
            persistence_path=temp_persistence_path,
        )

        status = CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.0,
        )
        record = TaskRecord(
            task_id="no-reason-int-001",
            description="Test without reason",
            status=status,
        )
        detector._tasks["no-reason-int-001"] = record

        detector.mark_interrupted("no-reason-int-001")

        updated = detector.get_task("no-reason-int-001")
        assert updated.status.marker == CompletionMarker.USER_INTERRUPTED
        assert updated.status.is_completed is False
        assert "User interrupted" in updated.status.reason
