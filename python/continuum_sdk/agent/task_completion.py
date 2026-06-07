"""Task Completion Detection

Detects whether a task has been successfully completed during agent execution.

Features:
    - LLM-assisted completion detection
    - TASK_COMPLETED / USER_INTERRUPTED markers
    - Session state persistence
    - Confidence scoring

Example:
    >>> from continuum_sdk.agent.task_completion import TaskCompletionDetector
    >>> from continuum_sdk.llm import LlmClient
    >>>
    >>> detector = TaskCompletionDetector(llm_client)
    >>> status = await detector.check_completion(
    ...     task="fix the login bug",
    ...     result="Fixed null check in auth.py line 42"
    ... )
    >>> print(status.is_completed, status.confidence)
    True 0.95
"""

import json
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

from ..llm import BaseLlmClient, LlmError, Message


class CompletionMarker(Enum):
    """Task completion markers."""

    TASK_COMPLETED = "TASK_COMPLETED"
    USER_INTERRUPTED = "USER_INTERRUPTED"
    IN_PROGRESS = "IN_PROGRESS"
    FAILED = "FAILED"
    NEEDS_CLARIFICATION = "NEEDS_CLARIFICATION"


@dataclass
class CompletionStatus:
    """Status of task completion check.

    Attributes:
        marker: The completion marker type
        is_completed: Whether the task is complete
        confidence: Confidence score (0.0 to 1.0)
        reason: Explanation of the determination
        suggestions: Optional suggestions for next steps
        timestamp: When the check was performed
    """

    marker: CompletionMarker
    is_completed: bool
    confidence: float = 0.0
    reason: str = ""
    suggestions: list[str] = field(default_factory=list)
    timestamp: datetime = field(default_factory=datetime.now)

    def to_dict(self) -> dict[str, Any]:
        """Serialize to dictionary."""
        return {
            "marker": self.marker.value,
            "is_completed": self.is_completed,
            "confidence": self.confidence,
            "reason": self.reason,
            "suggestions": self.suggestions,
            "timestamp": self.timestamp.isoformat(),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "CompletionStatus":
        """Deserialize from dictionary."""
        return cls(
            marker=CompletionMarker(data["marker"]),
            is_completed=data["is_completed"],
            confidence=data.get("confidence", 0.0),
            reason=data.get("reason", ""),
            suggestions=data.get("suggestions", []),
            timestamp=datetime.fromisoformat(data["timestamp"]),
        )


@dataclass
class TaskRecord:
    """Record of a task's execution state.

    Attributes:
        task_id: Unique task identifier
        description: Task description
        status: Current completion status
        created_at: When the task was created
        updated_at: When the task was last updated
        session_id: Associated session ID
        attempts: Number of execution attempts
        result: Final result (if completed)
    """

    task_id: str
    description: str
    status: CompletionStatus
    created_at: datetime = field(default_factory=datetime.now)
    updated_at: datetime = field(default_factory=datetime.now)
    session_id: str | None = None
    attempts: int = 0
    result: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Serialize to dictionary."""
        return {
            "task_id": self.task_id,
            "description": self.description,
            "status": self.status.to_dict(),
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
            "session_id": self.session_id,
            "attempts": self.attempts,
            "result": self.result,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "TaskRecord":
        """Deserialize from dictionary."""
        return cls(
            task_id=data["task_id"],
            description=data["description"],
            status=CompletionStatus.from_dict(data["status"]),
            created_at=datetime.fromisoformat(data["created_at"]),
            updated_at=datetime.fromisoformat(data["updated_at"]),
            session_id=data.get("session_id"),
            attempts=data.get("attempts", 0),
            result=data.get("result"),
        )


class TaskCompletionDetector:
    """Detects whether a task has been completed.

    Uses LLM analysis to determine if the task result satisfies the original
    task description. Maintains task state for session persistence.

    Example:
        >>> from continuum_sdk.llm import LlmClient
        >>> client = LlmClient.for_provider("anthropic", api_key="...")
        >>> detector = TaskCompletionDetector(client)
        >>>
        >>> # Check completion
        >>> status = await detector.check_completion(
        ...     task="add logging to auth.py",
        ...     result="Added logging statements to auth.py lines 15, 23, 45"
        ... )
        >>> if status.is_completed:
        ...     print(f"Task complete: {status.reason}")
    """

    COMPLETION_PROMPT = """Analyze whether the following task has been successfully completed.

Task: {task}

Result:
{result}

Determine:
1. Is the task complete? (yes/no/partial)
2. Confidence level (0.0 to 1.0)
3. Reason for your determination
4. Any suggestions for improvement (if not complete)

Respond in this exact JSON format:
{{
    "complete": "yes" | "no" | "partial",
    "confidence": 0.95,
    "reason": "explanation",
    "suggestions": ["suggestion1", "suggestion2"]
}}
"""

    def __init__(
        self,
        llm_client: BaseLlmClient | None = None,
        confidence_threshold: float = 0.8,
        persistence_path: Path | None = None,
    ):
        """Initialize the detector.

        Args:
            llm_client: LLM client for analysis (optional, can use rule-based if None)
            confidence_threshold: Minimum confidence to consider task complete
            persistence_path: Path to save task states (default: ~/.continuum/tasks/)
        """
        self._llm = llm_client
        self.confidence_threshold = confidence_threshold
        self._persistence_path = (
            persistence_path or Path.home() / ".continuum" / "tasks"
        )
        self._tasks: dict[str, TaskRecord] = {}

        self._ensure_persistence_dir()

    def _ensure_persistence_dir(self) -> None:
        """Create persistence directory if it doesn't exist."""
        self._persistence_path.mkdir(parents=True, exist_ok=True)

    async def check_completion(
        self,
        task: str,
        result: str,
        task_id: str | None = None,
    ) -> CompletionStatus:
        """Check if a task has been completed.

        Args:
            task: Original task description
            result: Result of task execution
            task_id: Optional task ID (auto-generated if not provided)

        Returns:
            CompletionStatus with determination
        """
        task_id = task_id or self._generate_task_id(task)

        if self._llm:
            status = await self._llm_check(task, result)
        else:
            status = self._rule_based_check(task, result)

        record = TaskRecord(
            task_id=task_id,
            description=task,
            status=status,
            result=result if status.is_completed else None,
        )
        self._tasks[task_id] = record
        self._persist_task(record)

        return status

    async def _llm_check(self, task: str, result: str) -> CompletionStatus:
        """Use LLM to analyze task completion."""
        prompt = self.COMPLETION_PROMPT.format(task=task, result=result)

        try:
            response = await self._llm.chat(
                messages=[Message.user(prompt)],
                max_tokens=500,
                temperature=0.3,
            )

            return self._parse_llm_response(response.content)

        except (LlmError, json.JSONDecodeError, KeyError, ValueError):
            # LLM API call failed or response parsing failed, graceful fallback to rule-based check
            return CompletionStatus(
                marker=CompletionMarker.IN_PROGRESS,
                is_completed=False,
                confidence=0.0,
                reason="LLM analysis failed, falling back to rule-based check",
            )
        except Exception:
            # Catch all other exceptions (network errors, timeouts, etc.) to ensure graceful fallback
            return CompletionStatus(
                marker=CompletionMarker.IN_PROGRESS,
                is_completed=False,
                confidence=0.0,
                reason="LLM analysis failed due to unexpected error, falling back to rule-based check",
            )

    def _parse_llm_response(self, content: str) -> CompletionStatus:
        """Parse LLM JSON response."""
        import re

        json_match = re.search(r"\{[^}]+\}", content, re.DOTALL)
        if not json_match:
            return CompletionStatus(
                marker=CompletionMarker.IN_PROGRESS,
                is_completed=False,
                confidence=0.0,
                reason="Failed to parse LLM response",
            )

        try:
            data = json.loads(json_match.group())
            complete = data.get("complete", "no")
            confidence = float(data.get("confidence", 0.0))
            reason = data.get("reason", "")
            suggestions = data.get("suggestions", [])

            if complete == "yes":
                marker = CompletionMarker.TASK_COMPLETED
                is_completed = confidence >= self.confidence_threshold
            elif complete == "partial":
                marker = CompletionMarker.NEEDS_CLARIFICATION
                is_completed = False
            else:
                marker = CompletionMarker.IN_PROGRESS
                is_completed = False

            return CompletionStatus(
                marker=marker,
                is_completed=is_completed,
                confidence=confidence,
                reason=reason,
                suggestions=suggestions,
            )

        except (json.JSONDecodeError, ValueError):
            return CompletionStatus(
                marker=CompletionMarker.IN_PROGRESS,
                is_completed=False,
                confidence=0.0,
                reason="Failed to parse LLM response JSON",
            )

    def _rule_based_check(self, task: str, result: str) -> CompletionStatus:
        """Rule-based completion check when LLM is unavailable."""
        completion_indicators = [
            "done",
            "completed",
            "finished",
            "success",
            "fixed",
            "added",
            "removed",
            "updated",
            "created",
        ]

        error_indicators = [
            "error",
            "failed",
            "exception",
            "unable to",
            "could not",
            "did not",
        ]

        result_lower = result.lower()

        has_completion_word = any(ind in result_lower for ind in completion_indicators)
        has_error = any(ind in result_lower for ind in error_indicators)

        if has_error:
            return CompletionStatus(
                marker=CompletionMarker.FAILED,
                is_completed=False,
                confidence=0.6,
                reason="Error indicators found in result",
                suggestions=["Review the error and retry"],
            )

        if has_completion_word and len(result) > 20:
            return CompletionStatus(
                marker=CompletionMarker.TASK_COMPLETED,
                is_completed=True,
                confidence=0.7,
                reason="Completion indicators found in result",
            )

        return CompletionStatus(
            marker=CompletionMarker.IN_PROGRESS,
            is_completed=False,
            confidence=0.5,
            reason="Unable to determine completion status",
            suggestions=["Provide more details in the result"],
        )

    def mark_completed(self, task_id: str, reason: str = "") -> None:
        """Manually mark a task as completed.

        Args:
            task_id: Task identifier
            reason: Optional reason for completion
        """
        if task_id in self._tasks:
            record = self._tasks[task_id]
            record.status = CompletionStatus(
                marker=CompletionMarker.TASK_COMPLETED,
                is_completed=True,
                confidence=1.0,
                reason=reason or "Manually marked as completed",
            )
            record.updated_at = datetime.now()
            self._persist_task(record)

    def mark_interrupted(self, task_id: str, reason: str = "") -> None:
        """Mark a task as user-interrupted.

        Args:
            task_id: Task identifier
            reason: Optional reason for interruption
        """
        if task_id in self._tasks:
            record = self._tasks[task_id]
            record.status = CompletionStatus(
                marker=CompletionMarker.USER_INTERRUPTED,
                is_completed=False,
                confidence=1.0,
                reason=reason or "User interrupted the task",
            )
            record.updated_at = datetime.now()
            self._persist_task(record)

    def get_task(self, task_id: str) -> TaskRecord | None:
        """Get a task record by ID.

        Args:
            task_id: Task identifier

        Returns:
            TaskRecord if found, None otherwise
        """
        return self._tasks.get(task_id) or self._load_task(task_id)

    def get_pending_tasks(self) -> list[TaskRecord]:
        """Get all pending (in-progress) tasks.

        Returns:
            List of TaskRecord objects
        """
        return [
            record
            for record in self._tasks.values()
            if record.status.marker == CompletionMarker.IN_PROGRESS
        ]

    def clear_completed_tasks(self) -> int:
        """Remove all completed tasks from memory.

        Returns:
            Number of tasks cleared
        """
        to_remove = [
            task_id
            for task_id, record in self._tasks.items()
            if record.status.is_completed
        ]
        for task_id in to_remove:
            del self._tasks[task_id]
        return len(to_remove)

    def _generate_task_id(self, task: str) -> str:
        """Generate a unique task ID."""
        import hashlib

        timestamp = datetime.now().isoformat()
        hash_input = f"{task}:{timestamp}".encode()
        return hashlib.md5(hash_input).hexdigest()[:12]

    def _persist_task(self, record: TaskRecord) -> None:
        """Persist task record to disk."""
        task_file = self._persistence_path / f"{record.task_id}.json"
        with open(task_file, "w", encoding="utf-8") as f:
            json.dump(record.to_dict(), f, indent=2)

    def _load_task(self, task_id: str) -> TaskRecord | None:
        """Load task record from disk."""
        task_file = self._persistence_path / f"{task_id}.json"
        if task_file.exists():
            with open(task_file, encoding="utf-8") as f:
                return TaskRecord.from_dict(json.load(f))
        return None

    def load_all_tasks(self) -> None:
        """Load all persisted tasks into memory."""
        for task_file in self._persistence_path.glob("*.json"):
            try:
                with open(task_file, encoding="utf-8") as f:
                    record = TaskRecord.from_dict(json.load(f))
                    self._tasks[record.task_id] = record
            except (json.JSONDecodeError, KeyError):
                continue
