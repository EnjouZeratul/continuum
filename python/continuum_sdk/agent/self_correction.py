"""
Self-Correction Module

Analyzes errors and generates corrective actions for intelligent recovery.

Features:
    - Error classification: Categorize errors into 12 types
    - Recovery strategies: 6 intelligent recovery approaches
    - LLM-powered analysis: Use AI for error understanding
    - Pattern learning: Remember successful corrections
    - Context-aware: Consider execution context

Error Types:
    - SYNTAX: Code syntax errors
    - RUNTIME: Runtime exceptions
    - IMPORT: Module import failures
    - TYPE: Type mismatch errors
    - VALUE: Invalid value errors
    - PERMISSION: Access/permission denied
    - NOT_FOUND: File or command not found
    - TIMEOUT: Operation timeout
    - NETWORK: Network connectivity issues
    - TEST_FAILURE: Test assertion failures
    - LINT: Linting/style violations
    - UNKNOWN: Unclassified errors

Recovery Strategies:
    - RETRY: Simple retry the same action
    - RETRY_MODIFIED: Retry with adjusted parameters
    - SKIP: Skip the step and continue
    - ALTERNATIVE: Use a different approach
    - ASK_USER: Request human intervention
    - ABORT: Stop execution entirely

Quick Start:
    >>> from continuum_sdk.agent.self_correction import SelfCorrection
    >>>
    >>> correction = SelfCorrection()
    >>>
    >>> # Analyze an error
    >>> error_ctx = correction.analyze_error(
    ...     error=ValueError("Invalid input"),
    ...     step_id="step-1",
    ...     action="process_data"
    ... )
    >>> print(f"Error type: {error_ctx.error_type.value}")
    >>> print(f"Suggested strategy: {error_ctx.suggested_strategy}")

Recovery Proposal:
    >>> proposal = correction.propose_correction(error_ctx, context={})
    >>> print(f"Strategy: {proposal.strategy.value}")
    >>> print(f"Description: {proposal.description}")
    >>> if proposal.modified_action:
    ...     print(f"Modified action: {proposal.modified_action}")

LLM-Enhanced Analysis:
    >>> from continuum_sdk.llm import LlmClient
    >>>
    >>> correction.llm_client = LlmClient.for_provider("anthropic", api_key="key")
    >>> error_ctx = await correction.analyze_with_llm(
    ...     error=ValueError("Invalid input"),
    ...     context="Processing user data"
    ... )

Learning from Corrections:
    >>> # Successful corrections are stored
    >>> correction.learn(error_ctx, proposal, success=True)
    >>>
    >>> # Similar errors will use learned patterns
    >>> similar_ctx = correction.analyze_error(
    ...     error=ValueError("Another invalid input"),
    ...     step_id="step-2"
    ... )

See Also:
    IntelligentAgent: Uses SelfCorrection for automatic recovery
    ErrorContext: Error classification container
    CorrectionProposal: Recovery action container
"""

import asyncio
import json
import logging
import re
import traceback
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import TYPE_CHECKING, Any, Optional

logger = logging.getLogger(__name__)

if TYPE_CHECKING:
    from ..llm import BaseLlmClient


class ErrorType(Enum):
    """Error classification."""

    SYNTAX = "syntax"  # Syntax errors in code
    RUNTIME = "runtime"  # Runtime errors
    IMPORT = "import"  # Import/module errors
    TYPE = "type"  # Type errors
    VALUE = "value"  # Value errors
    PERMISSION = "permission"  # Permission errors
    NOT_FOUND = "not_found"  # File/command not found
    TIMEOUT = "timeout"  # Timeout errors
    NETWORK = "network"  # Network errors
    TEST_FAILURE = "test"  # Test failures
    LINT = "lint"  # Linting errors
    UNKNOWN = "unknown"  # Unknown errors


class RecoveryStrategy(Enum):
    """Recovery strategy types."""

    RETRY = "retry"  # Simple retry
    RETRY_MODIFIED = "retry_modified"  # Retry with modifications
    SKIP = "skip"  # Skip and continue
    ALTERNATIVE = "alternative"  # Use alternative approach
    ASK_USER = "ask_user"  # Ask user for help
    ABORT = "abort"  # Abort execution


@dataclass
class ErrorContext:
    """
    Context information for an error.

    Attributes:
        error_type: Classified error type
        message: Error message
        traceback: Full traceback
        step_id: Step that caused the error
        action: Action that failed
        target: Target file/element
        attempt: Attempt number
        timestamp: When error occurred
        context: Additional context
    """

    error_type: ErrorType
    message: str
    traceback: str | None = None
    step_id: str | None = None
    action: str | None = None
    target: str | None = None
    attempt: int = 1
    timestamp: datetime = field(default_factory=datetime.now)
    context: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "error_type": self.error_type.value,
            "message": self.message,
            "traceback": self.traceback,
            "step_id": self.step_id,
            "action": self.action,
            "target": self.target,
            "attempt": self.attempt,
            "timestamp": self.timestamp.isoformat(),
        }


@dataclass
class Correction:
    """
    Proposed correction for an error.

    Attributes:
        strategy: Recovery strategy
        description: Human-readable description
        modified_action: Modified action to try
        parameters: Additional parameters
        confidence: Confidence level (0-1)
    """

    strategy: RecoveryStrategy
    description: str
    modified_action: str | None = None
    parameters: dict[str, Any] = field(default_factory=dict)
    confidence: float = 0.5


class SelfCorrection:
    """
    Self-correction system that analyzes errors and proposes fixes.
    """

    # Error pattern matching rules
    ERROR_PATTERNS = {
        ErrorType.SYNTAX: [
            r"SyntaxError:",
            r"IndentationError:",
            r"TabError:",
            r"unexpected token",
            r"expected.*but found",
            r"invalid syntax",
        ],
        ErrorType.IMPORT: [
            r"ImportError:",
            r"ModuleNotFoundError:",
            r"cannot import",
            r"No module named",
        ],
        ErrorType.TYPE: [
            r"TypeError:",
            r"type.*not.*supported",
            r"'NoneType'",
            r"unsupported operand",
        ],
        ErrorType.VALUE: [
            r"ValueError:",
            r"invalid value",
            r"invalid literal",
        ],
        ErrorType.NOT_FOUND: [
            r"FileNotFoundError:",
            r"not found",
            r"No such file",
            r"does not exist",
        ],
        ErrorType.PERMISSION: [
            r"PermissionError:",
            r"Permission denied",
            r"Access denied",
        ],
        ErrorType.TIMEOUT: [
            r"TimeoutError:",
            r"timed out",
            r"timeout",
        ],
        ErrorType.NETWORK: [
            r"ConnectionError:",
            r"NetworkError:",
            r"Connection refused",
            r"Name or service not known",
        ],
        ErrorType.TEST_FAILURE: [
            r"AssertionError:",
            r"FAILED",
            r"test failed",
        ],
    }

    # Common error fixes
    COMMON_FIXES = {
        ErrorType.IMPORT: {
            "pattern": r"No module named '(\w+)'",
            "fix": "Install missing module or check import path",
            "action": "pip install {module}",
        },
        ErrorType.NOT_FOUND: {
            "pattern": r"FileNotFoundError:.*'([^']+)'",
            "fix": "Check file path or create missing file",
            "action": "verify path: {path}",
        },
        ErrorType.SYNTAX: {
            "pattern": r"line (\d+)",
            "fix": "Check syntax at indicated line",
            "action": "read file at line {line}",
        },
    }

    def __init__(self, llm_client: Optional["BaseLlmClient"] = None):
        self.llm_client = llm_client
        self.error_history: list[ErrorContext] = []
        self.successful_corrections: dict[str, Correction] = {}

    def analyze_error(
        self,
        error: Exception,
        step_id: str | None = None,
        action: str | None = None,
        target: str | None = None,
        context: dict[str, Any] | None = None,
    ) -> ErrorContext:
        """
        Analyze an error and classify it.

        Args:
            error: The exception that occurred
            step_id: ID of the step that failed
            action: Action that was being performed
            target: Target file/element
            context: Additional context

        Returns:
            ErrorContext with classification
        """
        message = str(error)
        error_type = self._classify_error(message)
        tb = traceback.format_exc() if error else None

        error_ctx = ErrorContext(
            error_type=error_type,
            message=message,
            traceback=tb,
            step_id=step_id,
            action=action,
            target=target,
            context=context or {},
        )

        self.error_history.append(error_ctx)
        return error_ctx

    def propose_correction(
        self,
        error_ctx: ErrorContext,
        context: dict[str, Any] | None = None,
    ) -> Correction:
        """
        Propose a correction for the error.

        Args:
            error_ctx: Error context
            context: Additional context for decision

        Returns:
            Proposed correction
        """
        # Check if we've seen this error before and have a successful fix
        error_key = self._make_error_key(error_ctx)
        if error_key in self.successful_corrections:
            return self.successful_corrections[error_key]

        # Try pattern-based correction first
        correction = self._pattern_based_correction(error_ctx)
        if correction and correction.confidence > 0.7:
            return correction

        # Try LLM-based analysis
        if self.llm_client:
            try:
                correction = asyncio.run(self._llm_based_correction(error_ctx, context))
                if correction:
                    return correction
            except Exception as e:
                logger.debug("LLM-based correction failed: %s", e)

        # Fallback to retry with modifications
        return self._default_correction(error_ctx)

    async def _llm_based_correction(
        self,
        error_ctx: ErrorContext,
        context: dict[str, Any] | None = None,
    ) -> Correction | None:
        """Use LLM to analyze error and propose fix."""
        if not self.llm_client:
            return None

        from ..llm import Message

        system_prompt = """You are an error analyzer. Analyze the error and propose a correction.

Output JSON with:
{
  "strategy": "retry|retry_modified|skip|alternative|ask_user|abort",
  "description": "Brief description of the fix",
  "modified_action": "Modified action to take (if retry_modified)",
  "parameters": {},
  "confidence": 0.0-1.0
}

Strategies:
- retry: Same action might work (transient error)
- retry_modified: Need to modify the action
- skip: Skip this step, not critical
- alternative: Use different approach
- ask_user: Cannot determine fix, need user input
- abort: Fatal error, cannot recover"""

        user_message = f"""Error: {error_ctx.message}
Type: {error_ctx.error_type.value}
Action: {error_ctx.action}
Target: {error_ctx.target}
Attempt: {error_ctx.attempt}

Context: {json.dumps(context, indent=2) if context else 'None'}"""

        messages = [Message.user(user_message)]

        try:
            response = await self.llm_client.chat(
                messages=messages,
                system_prompt=system_prompt,
                temperature=0.3,
                max_tokens=500,
            )

            # Parse JSON response
            content = response.content
            json_match = re.search(r"\{[\s\S]*\}", content)
            if json_match:
                data = json.loads(json_match.group())
                strategy_map = {s.value: s for s in RecoveryStrategy}

                return Correction(
                    strategy=strategy_map.get(
                        data.get("strategy", "retry"), RecoveryStrategy.RETRY
                    ),
                    description=data.get("description", ""),
                    modified_action=data.get("modified_action"),
                    parameters=data.get("parameters", {}),
                    confidence=data.get("confidence", 0.5),
                )

        except Exception as e:
            logger.debug("Failed to parse LLM correction response: %s", e)

        return None

    def _classify_error(self, message: str) -> ErrorType:
        """Classify error type from message."""
        message_lower = message.lower()

        for error_type, patterns in self.ERROR_PATTERNS.items():
            for pattern in patterns:
                if re.search(pattern, message_lower, re.IGNORECASE):
                    return error_type

        return ErrorType.UNKNOWN

    def _pattern_based_correction(self, error_ctx: ErrorContext) -> Correction | None:
        """Apply pattern-based correction rules."""
        fix_rules = self.COMMON_FIXES.get(error_ctx.error_type)
        if not fix_rules:
            return None

        pattern = fix_rules.get("pattern", "")
        match = re.search(pattern, error_ctx.message, re.IGNORECASE)

        if match:
            groups = match.groups() if match.groups() else []
            description = fix_rules.get("fix", "Apply fix")
            action_template = fix_rules.get("action", "")

            # Substitute captured groups into action
            modified_action = action_template
            if groups:
                modified_action = action_template.format(
                    module=groups[0] if len(groups) > 0 else "",
                    path=groups[0] if len(groups) > 0 else "",
                    line=groups[0] if len(groups) > 0 else "",
                )

            return Correction(
                strategy=RecoveryStrategy.RETRY_MODIFIED,
                description=description,
                modified_action=modified_action,
                confidence=0.8,
            )

        return None

    def _default_correction(self, error_ctx: ErrorContext) -> Correction:
        """Generate default correction based on error type and attempt count."""
        # Too many attempts
        if error_ctx.attempt >= 3:
            return Correction(
                strategy=RecoveryStrategy.ASK_USER,
                description=f"Failed after {error_ctx.attempt} attempts. Need user guidance.",
                confidence=0.9,
            )

        # Error-type specific defaults
        strategies = {
            ErrorType.NETWORK: RecoveryStrategy.RETRY,
            ErrorType.TIMEOUT: RecoveryStrategy.RETRY,
            ErrorType.IMPORT: RecoveryStrategy.RETRY_MODIFIED,
            ErrorType.NOT_FOUND: RecoveryStrategy.ASK_USER,
            ErrorType.PERMISSION: RecoveryStrategy.ASK_USER,
            ErrorType.TEST_FAILURE: RecoveryStrategy.RETRY_MODIFIED,
        }

        strategy = strategies.get(error_ctx.error_type, RecoveryStrategy.RETRY_MODIFIED)

        return Correction(
            strategy=strategy,
            description=f"Retry with adjustments for {error_ctx.error_type.value} error",
            modified_action=error_ctx.action,
            confidence=0.6,
        )

    def _make_error_key(self, error_ctx: ErrorContext) -> str:
        """Create a key for identifying similar errors."""
        # Normalize error message
        message = re.sub(r"\d+", "N", error_ctx.message[:100])
        return f"{error_ctx.error_type.value}:{message}"

    def mark_successful(self, error_ctx: ErrorContext, correction: Correction):
        """Mark a correction as successful for future reference."""
        error_key = self._make_error_key(error_ctx)
        self.successful_corrections[error_key] = correction

    def get_error_history(self) -> list[ErrorContext]:
        """Get error history."""
        return self.error_history.copy()
