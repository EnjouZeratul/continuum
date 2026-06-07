"""
Agent Module

Provides Agent classes for interacting with Continuum.

Classes:
    - Agent: Basic agent with LLM integration
    - IntelligentAgent: Agent with planning and self-correction
    - Session: Conversation session management

Components:
    - Planner: Task decomposition and planning
    - SelfCorrection: Error analysis and recovery
    - ProgressTracker: Real-time progress tracking
"""

from .checkpoint import (
    CheckpointClient,
    CheckpointMeta,
)
from .history import (
    HistoryBrowser,
    HistoryFilter,
    HistoryStatistics,
    SearchResult,
    SortOrder,
    browse_session,
    browse_session_file,
)
from .intelligent import AgentMode, ExecutionResult, IntelligentAgent
from .planner import (
    Plan,
    Planner,
    Step,
    StepStatus,
    StepType,
)
from .progress import (
    ProgressEvent,
    ProgressState,
    ProgressTracker,
    StepLogger,
)
from .runtime import Agent, AgentConfig, AgentState
from .self_correction import (
    Correction,
    ErrorContext,
    ErrorType,
    RecoveryStrategy,
    SelfCorrection,
)
from .session import Message, MessageRole, Session
from .task_completion import (
    CompletionMarker,
    CompletionStatus,
    TaskCompletionDetector,
    TaskRecord,
)

__all__ = [
    # Core Agent
    "Agent",
    "AgentConfig",
    "AgentState",
    "Session",
    "Message",
    "MessageRole",
    # History Browser
    "HistoryBrowser",
    "HistoryFilter",
    "HistoryStatistics",
    "SearchResult",
    "SortOrder",
    "browse_session",
    "browse_session_file",
    # Intelligent Agent
    "IntelligentAgent",
    "AgentMode",
    "ExecutionResult",
    # Task Completion
    "TaskCompletionDetector",
    "CompletionStatus",
    "CompletionMarker",
    "TaskRecord",
    # Checkpoint
    "CheckpointClient",
    "CheckpointMeta",
    # Planner
    "Planner",
    "Plan",
    "Step",
    "StepType",
    "StepStatus",
    # Self-Correction
    "SelfCorrection",
    "ErrorContext",
    "ErrorType",
    "Correction",
    "RecoveryStrategy",
    # Progress
    "ProgressTracker",
    "ProgressEvent",
    "ProgressState",
    "StepLogger",
]
