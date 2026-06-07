"""
Task Planner

Analyzes tasks and generates execution plans with decomposed steps.

Features:
    - Task decomposition: Break complex tasks into executable steps
    - Dependency analysis: Identify step dependencies
    - Plan generation: Create structured execution plan
    - Step estimation: Estimate complexity and time
    - LLM-powered planning: Use AI to generate intelligent plans

Step Types:
    - ANALYZE: Analyze problem/code structure
    - PLAN: Create detailed plan
    - SEARCH: Search codebase for patterns
    - READ: Read and understand files
    - EDIT: Modify existing files
    - WRITE: Create new files
    - TEST: Run tests to verify changes
    - VERIFY: Verify the results
    - CONFIRM: Ask user for confirmation

Quick Start:
    >>> from continuum_sdk.agent.planner import Planner
    >>> from continuum_sdk.llm import LlmClient
    >>>
    >>> planner = Planner()
    >>> planner.llm_client = LlmClient.for_provider("anthropic", api_key="key")
    >>>
    >>> plan = await planner.plan("Fix the null pointer bug in auth.py")
    >>> print(f"Generated {len(plan.steps)} steps")

Plan Structure:
    >>> plan.to_dict()  # Export plan as dictionary
    >>> plan.get_progress()  # Get execution progress
    >>> plan.get_pending_steps()  # Get next steps to execute

Step Properties:
    >>> for step in plan.steps:
    ...     print(f"[{step.id}] {step.type.value}: {step.description}")
    ...     print(f"  Dependencies: {step.dependencies}")
    ...     print(f"  Status: {step.status.value}")

Manual Plan Creation:
    >>> from continuum_sdk.agent.planner import Plan, Step, StepType
    >>> plan = Plan(
    ...     id="manual-plan",
    ...     task="Custom task",
    ...     steps=[
    ...         Step(id="s1", type=StepType.READ, description="Read file"),
    ...         Step(id="s2", type=StepType.EDIT, description="Edit file", dependencies=["s1"]),
    ...     ]
    ... )

See Also:
    IntelligentAgent: Higher-level agent with planner integration
    SelfCorrection: Error recovery during plan execution
    ProgressTracker: Track execution progress
"""

import json
import logging
import re
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import TYPE_CHECKING, Any, Optional

logger = logging.getLogger(__name__)

if TYPE_CHECKING:
    from ..llm import BaseLlmClient


class StepStatus(Enum):
    """Step execution status."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    SKIPPED = "skipped"
    RETRYING = "retrying"


class StepType(Enum):
    """Step type classification."""

    ANALYZE = "analyze"  # Analyze problem/code
    PLAN = "plan"  # Create plan
    SEARCH = "search"  # Search codebase
    READ = "read"  # Read files
    EDIT = "edit"  # Modify files
    WRITE = "write"  # Create files
    TEST = "test"  # Run tests
    VERIFY = "verify"  # Verify changes
    CONFIRM = "confirm"  # Ask user confirmation


@dataclass
class Step:
    """
    Single execution step.

    Attributes:
        id: Unique step identifier
        type: Step type
        description: Human-readable description
        action: Action to perform (e.g., "read file X")
        target: Target file/element (if applicable)
        dependencies: IDs of dependent steps
        status: Current status
        result: Execution result
        error: Error message if failed
        retry_count: Number of retry attempts
        max_retries: Maximum retries allowed
        estimated_time: Estimated duration in seconds
        metadata: Additional metadata
    """

    id: str
    type: StepType
    description: str
    action: str
    target: str | None = None
    dependencies: list[str] = field(default_factory=list)
    status: StepStatus = StepStatus.PENDING
    result: str | None = None
    error: str | None = None
    retry_count: int = 0
    max_retries: int = 3
    estimated_time: float = 5.0
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "type": self.type.value,
            "description": self.description,
            "action": self.action,
            "target": self.target,
            "dependencies": self.dependencies,
            "status": self.status.value,
            "result": self.result,
            "error": self.error,
            "retry_count": self.retry_count,
            "estimated_time": self.estimated_time,
        }


@dataclass
class Plan:
    """
    Execution plan with multiple steps.

    Attributes:
        id: Unique plan identifier
        task: Original task description
        steps: List of execution steps
        created_at: Plan creation time
        current_step: Current executing step ID
        status: Overall plan status
        context: Accumulated context from execution
    """

    id: str
    task: str
    steps: list[Step] = field(default_factory=list)
    created_at: datetime = field(default_factory=datetime.now)
    current_step: str | None = None
    status: str = "pending"
    context: dict[str, Any] = field(default_factory=dict)

    def get_step(self, step_id: str) -> Step | None:
        """Get step by ID."""
        for step in self.steps:
            if step.id == step_id:
                return step
        return None

    def get_pending_steps(self) -> list[Step]:
        """Get all pending steps with satisfied dependencies."""
        completed_ids = {
            s.id
            for s in self.steps
            if s.status in (StepStatus.COMPLETED, StepStatus.SKIPPED)
        }
        pending = []
        for step in self.steps:
            if step.status != StepStatus.PENDING:
                continue
            # Check all dependencies are satisfied
            if all(dep_id in completed_ids for dep_id in step.dependencies):
                pending.append(step)
        return pending

    def get_progress(self) -> dict[str, Any]:
        """Calculate execution progress."""
        total = len(self.steps)
        completed = sum(1 for s in self.steps if s.status == StepStatus.COMPLETED)
        failed = sum(1 for s in self.steps if s.status == StepStatus.FAILED)
        skipped = sum(1 for s in self.steps if s.status == StepStatus.SKIPPED)
        pending = sum(1 for s in self.steps if s.status == StepStatus.PENDING)
        running = sum(1 for s in self.steps if s.status == StepStatus.RUNNING)

        return {
            "total": total,
            "completed": completed,
            "failed": failed,
            "skipped": skipped,
            "pending": pending,
            "running": running,
            "progress_percent": (completed + skipped) / total * 100 if total > 0 else 0,
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "task": self.task,
            "steps": [s.to_dict() for s in self.steps],
            "created_at": self.created_at.isoformat(),
            "current_step": self.current_step,
            "status": self.status,
            "progress": self.get_progress(),
        }


class Planner:
    """
    Task planner that decomposes tasks into execution steps.

    Uses LLM for complex task analysis, with fallback to pattern matching.
    """

    # Pattern-based task templates
    TASK_PATTERNS = {
        "fix_bug": [
            r"fix\s+(?:bug\s+)?(?:in\s+)?(.+)",
            r"修复(?:bug)?(?:在)?(.+)",
            r"解决(?:问题)?(?:在)?(.+)",
        ],
        "add_feature": [
            r"add\s+(?:feature\s+)?(?:to\s+)?(.+)",
            r"添加(?:功能)?(?:到)?(.+)",
            r"实现(.+)",
        ],
        "refactor": [
            r"refactor\s+(.+)",
            r"重构(.+)",
        ],
        "update_doc": [
            r"(?:update|add)\s+(?:doc|documentation)(?:\s+for)?\s*(.+)",
            r"更新(?:文档)?(?:为)?(.+)",
        ],
        "run_test": [
            r"(?:run|execute)\s+(?:test|tests)(?:\s+for)?\s*(.+)",
            r"运行(?:测试)?(?:为)?(.+)",
        ],
    }

    # Default step templates by task type
    STEP_TEMPLATES = {
        "fix_bug": [
            StepType.SEARCH,  # Search for relevant code
            StepType.READ,  # Read and understand code
            StepType.ANALYZE,  # Analyze the bug
            StepType.EDIT,  # Apply fix
            StepType.TEST,  # Run tests
            StepType.VERIFY,  # Verify fix
        ],
        "add_feature": [
            StepType.SEARCH,  # Search existing patterns
            StepType.READ,  # Read relevant code
            StepType.PLAN,  # Design implementation
            StepType.EDIT,  # Implement changes
            StepType.TEST,  # Test changes
            StepType.VERIFY,  # Verify functionality
        ],
        "refactor": [
            StepType.READ,  # Read code to refactor
            StepType.ANALYZE,  # Analyze structure
            StepType.PLAN,  # Plan refactoring
            StepType.EDIT,  # Apply changes
            StepType.TEST,  # Run tests
            StepType.VERIFY,  # Verify behavior
        ],
    }

    def __init__(self, llm_client: Optional["BaseLlmClient"] = None):
        self.llm_client = llm_client

    async def plan(self, task: str, context: dict[str, Any] | None = None) -> Plan:
        """
        Create execution plan for task.

        Args:
            task: Task description
            context: Additional context (codebase info, previous steps, etc.)

        Returns:
            Plan with decomposed steps
        """
        from ..utils import generate_short_id

        plan_id = generate_short_id()
        plan = Plan(id=plan_id, task=task)

        # Try LLM-based planning first
        if self.llm_client:
            try:
                steps = await self._plan_with_llm(task, context)
                if steps:
                    plan.steps = steps
                    return plan
            except Exception as e:
                logger.debug("LLM planning failed, falling back to patterns: %s", e)

        # Fallback to pattern-based planning
        steps = self._plan_with_patterns(task, context)
        plan.steps = steps

        return plan

    async def _plan_with_llm(
        self, task: str, context: dict[str, Any] | None = None
    ) -> list[Step]:
        """Use LLM to analyze task and generate steps."""
        if not self.llm_client:
            return []

        from ..llm import Message

        system_prompt = """You are a task planner. Analyze the given task and break it down into executable steps.

Output a JSON array of steps. Each step should have:
- id: step number (s1, s2, etc.)
- type: one of analyze, search, read, edit, write, test, verify, confirm
- description: brief description of what this step does
- action: specific action to take
- target: file or element this step targets (if applicable)
- dependencies: array of step IDs this step depends on

Example output:
[
  {"id": "s1", "type": "search", "description": "Find bug location", "action": "search for error pattern", "dependencies": []},
  {"id": "s2", "type": "read", "description": "Read buggy code", "action": "read found files", "target": null, "dependencies": ["s1"]}
]"""

        user_message = f"Task: {task}\n"
        if context:
            user_message += f"\nContext: {json.dumps(context, indent=2)}"

        messages = [Message.user(user_message)]

        try:
            response = await self.llm_client.chat(
                messages=messages,
                system_prompt=system_prompt,
                temperature=0.3,
                max_tokens=2000,
            )

            # Parse JSON from response
            content = response.content
            # Extract JSON array
            json_match = re.search(r"\[[\s\S]*\]", content)
            if json_match:
                steps_data = json.loads(json_match.group())
                return self._parse_steps(steps_data)

        except Exception as e:
            logger.debug("Failed to parse LLM planning response: %s", e)

        return []

    def _plan_with_patterns(
        self, task: str, context: dict[str, Any] | None = None
    ) -> list[Step]:
        """Pattern-based task decomposition."""

        # Detect task type
        task_type = self._detect_task_type(task)

        # Get step template
        template = self.STEP_TEMPLATES.get(task_type, self.STEP_TEMPLATES["fix_bug"])

        # Generate steps from template
        steps = []
        prev_id = None

        for i, step_type in enumerate(template):
            step_id = f"s{i+1}"
            step = self._create_step_from_type(
                step_id=step_id,
                step_type=step_type,
                task=task,
                prev_step_id=prev_id,
            )
            steps.append(step)
            prev_id = step_id

        return steps

    def _detect_task_type(self, task: str) -> str:
        """Detect task type from description."""
        task_lower = task.lower()

        for task_type, patterns in self.TASK_PATTERNS.items():
            for pattern in patterns:
                if re.search(pattern, task_lower):
                    return task_type

        return "fix_bug"  # Default

    def _create_step_from_type(
        self,
        step_id: str,
        step_type: StepType,
        task: str,
        prev_step_id: str | None,
    ) -> Step:
        """Create step from type template."""
        step_descriptions = {
            StepType.SEARCH: f"Search codebase for: {task}",
            StepType.READ: "Read and analyze relevant files",
            StepType.ANALYZE: "Analyze the problem and identify root cause",
            StepType.PLAN: "Create implementation plan",
            StepType.EDIT: "Apply changes to fix the issue",
            StepType.WRITE: "Create new files if needed",
            StepType.TEST: "Run tests to verify changes",
            StepType.VERIFY: "Verify the solution works correctly",
        }

        step_actions = {
            StepType.SEARCH: "search",
            StepType.READ: "read",
            StepType.ANALYZE: "analyze",
            StepType.PLAN: "plan",
            StepType.EDIT: "edit",
            StepType.WRITE: "write",
            StepType.TEST: "test",
            StepType.VERIFY: "verify",
        }

        dependencies = [prev_step_id] if prev_step_id else []

        return Step(
            id=step_id,
            type=step_type,
            description=step_descriptions.get(step_type, f"Execute {step_type.value}"),
            action=step_actions.get(step_type, step_type.value),
            dependencies=dependencies,
        )

    def _parse_steps(self, steps_data: list[dict]) -> list[Step]:
        """Parse steps from LLM response."""
        steps = []
        type_map = {t.value: t for t in StepType}

        for data in steps_data:
            step_type = type_map.get(data.get("type", "analyze"), StepType.ANALYZE)
            step = Step(
                id=data.get("id", f"s{len(steps)+1}"),
                type=step_type,
                description=data.get("description", ""),
                action=data.get("action", ""),
                target=data.get("target"),
                dependencies=data.get("dependencies", []),
            )
            steps.append(step)

        return steps

    def add_step(
        self,
        plan: Plan,
        step_type: StepType,
        description: str,
        action: str,
        target: str | None = None,
        dependencies: list[str] | None = None,
    ) -> Step:
        """Add new step to plan."""
        step_id = f"s{len(plan.steps)+1}"
        step = Step(
            id=step_id,
            type=step_type,
            description=description,
            action=action,
            target=target,
            dependencies=dependencies or [],
        )
        plan.steps.append(step)
        return step
