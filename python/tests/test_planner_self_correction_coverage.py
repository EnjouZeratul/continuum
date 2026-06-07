"""
Tests for planner.py and self_correction.py edge cases to achieve 100% coverage.
"""

import asyncio
from unittest.mock import AsyncMock, Mock

from continuum_sdk.agent.planner import Planner, StepType
from continuum_sdk.agent.self_correction import (
    ErrorContext,
    ErrorType,
    RecoveryStrategy,
    SelfCorrection,
)

# ==================== Planner Edge Cases ====================


class TestPlannerExceptionHandling:
    """Test exception handling in planner.plan() method."""

    def test_plan_catches_exception_from_llm_planning(self):
        """Test that plan() catches exception from _plan_with_llm and falls back."""
        # We need to mock _plan_with_llm itself to raise an exception
        # because _plan_with_llm has its own exception handling that returns []
        planner = Planner(llm_client=AsyncMock())

        # Patch _plan_with_llm to raise an exception directly
        async def raise_error(task, context):
            raise RuntimeError("LLM service completely unavailable")

        planner._plan_with_llm = raise_error

        # This should catch the exception and fall back to pattern-based planning
        plan = asyncio.run(planner.plan("fix critical bug in auth.py"))

        # Should still produce steps via pattern fallback
        assert len(plan.steps) > 0
        # Should have used pattern-based planning (fix_bug template)
        assert any(step.type == StepType.SEARCH for step in plan.steps)


class TestPlannerLLMClientNone:
    """Test _plan_with_llm when llm_client is None."""

    def test_plan_with_llm_returns_empty_when_no_client(self):
        """Test that _plan_with_llm returns empty list when llm_client is None."""
        planner = Planner(llm_client=None)

        result = asyncio.run(planner._plan_with_llm("fix bug", None))

        assert result == []


class TestPlannerWithContext:
    """Test LLM planning with context parameter."""

    def test_plan_with_llm_includes_context_in_message(self):
        """Test that context is included in the LLM message when provided."""
        mock_client = AsyncMock()
        mock_response = Mock()
        mock_response.content = '[{"id": "s1", "type": "analyze", "description": "Analyze", "action": "analyze", "dependencies": []}]'
        mock_client.chat = AsyncMock(return_value=mock_response)

        planner = Planner(llm_client=mock_client)

        context = {"files": ["auth.py"], "language": "python"}
        plan = asyncio.run(planner.plan("fix bug in auth", context=context))

        # Verify the chat was called
        assert mock_client.chat.called
        # Verify we got steps from LLM
        assert len(plan.steps) == 1
        assert plan.steps[0].type == StepType.ANALYZE


# ==================== Self-Correction Edge Cases ====================


class TestSelfCorrectionExceptionHandling:
    """Test exception handling in propose_correction method."""

    def test_propose_correction_catches_exception_from_llm(self):
        """Test that propose_correction catches exception from _llm_based_correction."""
        # We need to mock _llm_based_correction to raise directly
        # because it has internal exception handling
        correction = SelfCorrection(llm_client=AsyncMock())

        # Patch the method to raise
        async def raise_error(error_ctx, context):
            raise RuntimeError("API timeout")

        correction._llm_based_correction = raise_error

        error_ctx = ErrorContext(
            error_type=ErrorType.RUNTIME,
            message="Division by zero",
            attempt=1,
        )

        # This should catch the exception and fall back to default correction
        proposal = correction.propose_correction(error_ctx)

        # Should get a default correction
        assert proposal is not None
        assert proposal.strategy in list(RecoveryStrategy)


class TestSelfCorrectionLLMClientNone:
    """Test _llm_based_correction when llm_client is None."""

    def test_llm_based_correction_returns_none_when_no_client(self):
        """Test that _llm_based_correction returns None when llm_client is None."""
        correction = SelfCorrection(llm_client=None)

        error_ctx = ErrorContext(
            error_type=ErrorType.VALUE,
            message="Invalid value",
            attempt=1,
        )

        result = asyncio.run(correction._llm_based_correction(error_ctx, None))

        assert result is None


class TestSelfCorrectionPatternBasedWithGroups:
    """Test pattern-based correction with captured groups."""

    def test_pattern_based_correction_formats_groups(self):
        """Test that pattern-based correction substitutes captured groups into action."""
        correction = SelfCorrection()

        # Test ImportError with module name
        error_ctx = ErrorContext(
            error_type=ErrorType.IMPORT,
            message="No module named 'nonexistent_module'",
            attempt=1,
        )

        proposal = correction.propose_correction(error_ctx)

        # Should have formatted the action with the captured module name
        assert proposal is not None
        assert proposal.modified_action is not None
        assert (
            "nonexistent_module" in proposal.modified_action
            or "pip install" in proposal.modified_action
        )

    def test_pattern_based_correction_formats_path(self):
        """Test pattern-based correction with file path."""
        correction = SelfCorrection()

        error_ctx = ErrorContext(
            error_type=ErrorType.NOT_FOUND,
            message="FileNotFoundError: [Errno 2] No such file or directory: '/path/to/missing.py'",
            attempt=1,
        )

        proposal = correction.propose_correction(error_ctx)

        assert proposal is not None
        assert proposal.modified_action is not None

    def test_pattern_based_correction_formats_line_number(self):
        """Test pattern-based correction with line number."""
        correction = SelfCorrection()

        error_ctx = ErrorContext(
            error_type=ErrorType.SYNTAX,
            message="SyntaxError: invalid syntax (line 42)",
            attempt=1,
        )

        proposal = correction.propose_correction(error_ctx)

        assert proposal is not None
        assert proposal.description is not None

    def test_pattern_based_correction_no_groups(self):
        """Test pattern-based correction when pattern has no groups."""
        correction = SelfCorrection()

        # Temporarily patch COMMON_FIXES to have a pattern without groups
        import continuum_sdk.agent.self_correction as sc_module

        original_fixes = sc_module.SelfCorrection.COMMON_FIXES.copy()

        # Add a pattern that matches but has no groups
        sc_module.SelfCorrection.COMMON_FIXES[ErrorType.SYNTAX] = {
            "pattern": r"SyntaxError",
            "fix": "Check syntax",
            "action": "read file",  # No placeholders
        }

        try:
            error_ctx = ErrorContext(
                error_type=ErrorType.SYNTAX,
                message="SyntaxError: invalid syntax",
                attempt=1,
            )

            proposal = correction._pattern_based_correction(error_ctx)

            # Should return a correction even without groups
            assert proposal is not None
            assert proposal.strategy == RecoveryStrategy.RETRY_MODIFIED
            # action_template should be used as-is without formatting
            assert proposal.modified_action == "read file"
        finally:
            # Restore original
            sc_module.SelfCorrection.COMMON_FIXES = original_fixes


class TestSelfCorrectionLLMJSONParsing:
    """Test LLM-based correction JSON parsing paths."""

    def test_llm_correction_parses_valid_json_object(self):
        """Test that _llm_based_correction correctly parses JSON object."""
        mock_client = AsyncMock()
        mock_response = Mock()
        # Return valid JSON object
        mock_response.content = """{"strategy": "retry", "description": "Transient error", "confidence": 0.8}"""
        mock_client.chat = AsyncMock(return_value=mock_response)

        correction = SelfCorrection(llm_client=mock_client)
        error_ctx = ErrorContext(
            error_type=ErrorType.NETWORK,
            message="Connection refused",
            attempt=1,
        )

        proposal = asyncio.run(correction._llm_based_correction(error_ctx, None))

        assert proposal is not None
        assert proposal.strategy == RecoveryStrategy.RETRY
        assert proposal.confidence == 0.8

    def test_llm_correction_with_context(self):
        """Test LLM-based correction with context parameter."""
        mock_client = AsyncMock()
        mock_response = Mock()
        mock_response.content = (
            '{"strategy": "skip", "description": "Non-critical", "confidence": 0.7}'
        )
        mock_client.chat = AsyncMock(return_value=mock_response)

        correction = SelfCorrection(llm_client=mock_client)
        error_ctx = ErrorContext(
            error_type=ErrorType.VALUE,
            message="Invalid parameter",
            action="process_data",
            attempt=1,
        )

        context = {"step": "data_processing", "critical": False}
        proposal = asyncio.run(correction._llm_based_correction(error_ctx, context))

        assert proposal is not None
        assert proposal.strategy == RecoveryStrategy.SKIP


class TestSelfCorrectionJSONParseFailure:
    """Test LLM-based correction when JSON parsing fails."""

    def test_llm_correction_returns_none_on_json_parse_error(self):
        """Test that _llm_based_correction returns None when JSON parsing fails."""
        mock_client = AsyncMock()
        mock_response = Mock()
        # Return invalid JSON
        mock_response.content = "This is not JSON at all"
        mock_client.chat = AsyncMock(return_value=mock_response)

        correction = SelfCorrection(llm_client=mock_client)
        error_ctx = ErrorContext(
            error_type=ErrorType.RUNTIME,
            message="Some error",
            attempt=1,
        )

        result = asyncio.run(correction._llm_based_correction(error_ctx, None))

        # Should return None when JSON parsing fails
        assert result is None


class TestSelfCorrectionDefaultFallback:
    """Test default correction fallback."""

    def test_default_correction_for_network_error(self):
        """Test default correction for network errors is RETRY."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.NETWORK,
            message="Connection timeout",
            attempt=1,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.RETRY

    def test_default_correction_for_timeout_error(self):
        """Test default correction for timeout errors is RETRY."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.TIMEOUT,
            message="Operation timed out",
            attempt=1,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.RETRY

    def test_default_correction_for_import_error(self):
        """Test default correction for import errors is RETRY_MODIFIED."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.IMPORT,
            message="No module named 'requests'",
            attempt=1,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.RETRY_MODIFIED

    def test_default_correction_for_permission_error(self):
        """Test default correction for permission errors is ASK_USER."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.PERMISSION,
            message="Permission denied",
            attempt=1,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.ASK_USER

    def test_default_correction_for_test_failure(self):
        """Test default correction for test failures is RETRY_MODIFIED."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.TEST_FAILURE,
            message="AssertionError: expected True",
            attempt=1,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.RETRY_MODIFIED


class TestSelfCorrectionHighAttemptCount:
    """Test behavior when attempt count is high."""

    def test_default_correction_asks_user_after_three_attempts(self):
        """Test that after 3 attempts, default correction asks user."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.RUNTIME,
            message="Persistent error",
            attempt=3,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.ASK_USER
        assert "3 attempts" in proposal.description

    def test_default_correction_asks_user_after_many_attempts(self):
        """Test that after many attempts, default correction asks user."""
        correction = SelfCorrection()
        error_ctx = ErrorContext(
            error_type=ErrorType.RUNTIME,
            message="Persistent error",
            attempt=5,
        )

        proposal = correction._default_correction(error_ctx)

        assert proposal.strategy == RecoveryStrategy.ASK_USER
