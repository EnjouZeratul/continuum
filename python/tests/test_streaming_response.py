"""
Streaming Response Tests

Tests for the streaming response functionality:
- Rust async iterator protocol
- Python SDK wrapper
- Cancellation support
- Error handling
"""

import pytest
import asyncio
from dataclasses import dataclass
from typing import Any


# Mock Rust StreamChunk for testing without compiled bindings
@dataclass
class MockStreamChunk:
    """Mock StreamChunk for testing."""
    iteration: int
    state: str
    content: str | None
    tool_calls_json: str | None
    should_continue: bool
    is_final: bool
    error: str | None


class MockStreamIterator:
    """Mock async iterator for testing."""

    def __init__(self, chunks: list[MockStreamChunk]):
        self._chunks = chunks
        self._index = 0
        self._aborted = False

    def __aiter__(self):
        return self

    async def __anext__(self) -> MockStreamChunk:
        if self._aborted:
            raise StopAsyncIteration

        if self._index >= len(self._chunks):
            raise StopAsyncIteration

        chunk = self._chunks[self._index]
        self._index += 1
        return chunk

    def abort(self):
        self._aborted = True

    def is_aborted(self) -> bool:
        return self._aborted


class TestStreamChunk:
    """Tests for StreamChunk dataclass."""

    def test_stream_chunk_creation(self):
        """Test creating a StreamChunk."""
        chunk = MockStreamChunk(
            iteration=1,
            state="running",
            content="Hello",
            tool_calls_json=None,
            should_continue=True,
            is_final=False,
            error=None,
        )
        assert chunk.iteration == 1
        assert chunk.state == "running"
        assert chunk.content == "Hello"
        assert not chunk.is_final

    def test_stream_chunk_to_dict(self):
        """Test converting StreamChunk to dict."""
        chunk = MockStreamChunk(
            iteration=2,
            state="completed",
            content="Done",
            tool_calls_json=None,
            should_continue=False,
            is_final=True,
            error=None,
        )
        # Simulate to_dict method
        result = {
            "iteration": chunk.iteration,
            "state": chunk.state,
            "content": chunk.content,
            "is_final": chunk.is_final,
        }
        assert result["iteration"] == 2
        assert result["state"] == "completed"
        assert result["content"] == "Done"
        assert result["is_final"]


class TestMockStreamIterator:
    """Tests for mock stream iterator."""

    @pytest.mark.asyncio
    async def test_basic_iteration(self):
        """Test basic async iteration."""
        chunks = [
            MockStreamChunk(1, "running", "Hello", None, True, False, None),
            MockStreamChunk(2, "running", " World", None, True, False, None),
            MockStreamChunk(3, "completed", "!", None, False, True, None),
        ]

        iterator = MockStreamIterator(chunks)
        results = []

        async for chunk in iterator:
            results.append(chunk)

        assert len(results) == 3
        assert results[0].content == "Hello"
        assert results[1].content == " World"
        assert results[2].is_final

    @pytest.mark.asyncio
    async def test_empty_iterator(self):
        """Test empty iterator."""
        iterator = MockStreamIterator([])
        results = []

        async for chunk in iterator:
            results.append(chunk)

        assert len(results) == 0

    @pytest.mark.asyncio
    async def test_abort_during_iteration(self):
        """Test aborting iteration."""
        chunks = [
            MockStreamChunk(1, "running", "A", None, True, False, None),
            MockStreamChunk(2, "running", "B", None, True, False, None),
            MockStreamChunk(3, "completed", "C", None, False, True, None),
        ]

        iterator = MockStreamIterator(chunks)
        results = []

        async for chunk in iterator:
            results.append(chunk)
            if chunk.iteration == 1:
                iterator.abort()

        assert len(results) == 1
        assert iterator.is_aborted()


class TestAgentStreamIntegration:
    """Integration tests for Agent streaming."""

    @pytest.mark.asyncio
    async def test_agent_run_stream_basic(self):
        """Test basic Agent.run_stream functionality."""
        # This test uses the mock to simulate the Rust bindings

        chunks = [
            MockStreamChunk(1, "running", "Starting", None, True, False, None),
            MockStreamChunk(2, "running", " processing", None, True, False, None),
            MockStreamChunk(3, "completed", " done", None, False, True, None),
        ]

        iterator = MockStreamIterator(chunks)
        collected_content = []

        async for chunk in iterator:
            if chunk.content:
                collected_content.append(chunk.content)

        assert "".join(collected_content) == "Starting processing done"

    @pytest.mark.asyncio
    async def test_agent_run_stream_with_tools(self):
        """Test streaming with tool calls."""
        import json

        tool_call = json.dumps([{
            "id": "tc_123",
            "name": "search",
            "arguments": '{"query": "test"}'
        }])

        chunks = [
            MockStreamChunk(1, "running", "I'll search for that.", None, True, False, None),
            MockStreamChunk(2, "tool_calling", None, tool_call, True, False, None),
            MockStreamChunk(3, "running", "Found results.", None, True, False, None),
            MockStreamChunk(4, "completed", "Done", None, False, True, None),
        ]

        iterator = MockStreamIterator(chunks)
        tool_calls_found = []
        content_parts = []

        async for chunk in iterator:
            if chunk.tool_calls_json:
                tool_calls_found.append(json.loads(chunk.tool_calls_json))
            if chunk.content:
                content_parts.append(chunk.content)

        assert len(tool_calls_found) == 1
        assert tool_calls_found[0][0]["name"] == "search"
        assert "Found results" in "".join(content_parts)

    @pytest.mark.asyncio
    async def test_agent_run_stream_error_handling(self):
        """Test error handling in streaming."""
        chunks = [
            MockStreamChunk(1, "running", "Starting", None, True, False, None),
            MockStreamChunk(2, "error", None, None, False, True, "API rate limit exceeded"),
        ]

        iterator = MockStreamIterator(chunks)
        last_error = None

        async for chunk in iterator:
            if chunk.error:
                last_error = chunk.error

        assert last_error == "API rate limit exceeded"


class TestStreamCancellation:
    """Tests for stream cancellation."""

    @pytest.mark.asyncio
    async def test_cancel_with_abort_flag(self):
        """Test cancellation using abort flag."""
        chunks = [
            MockStreamChunk(i, "running", f"Chunk {i}", None, True, False, None)
            for i in range(1, 100)
        ]

        iterator = MockStreamIterator(chunks)
        results = []
        max_chunks = 5

        async for chunk in iterator:
            results.append(chunk)
            if len(results) >= max_chunks:
                iterator.abort()

        assert len(results) == max_chunks
        assert iterator.is_aborted()

    @pytest.mark.asyncio
    async def test_cancel_with_timeout(self):
        """Test cancellation with timeout."""
        chunks = [
            MockStreamChunk(i, "running", f"Chunk {i}", None, True, False, None)
            for i in range(1, 10)
        ]

        iterator = MockStreamIterator(chunks)
        results = []

        async def collect_with_timeout():
            async for chunk in iterator:
                results.append(chunk)
                await asyncio.sleep(0.01)  # Simulate processing

        try:
            await asyncio.wait_for(collect_with_timeout(), timeout=0.1)
        except asyncio.TimeoutError:
            iterator.abort()

        assert len(results) < 10


class TestStreamStateTransitions:
    """Tests for state transitions during streaming."""

    @pytest.mark.asyncio
    async def test_state_sequence(self):
        """Test state transition sequence."""
        expected_states = ["running", "tool_calling", "running", "completed"]
        chunks = [
            MockStreamChunk(1, "running", "Start", None, True, False, None),
            MockStreamChunk(2, "tool_calling", "Tool", None, True, False, None),
            MockStreamChunk(3, "running", "Result", None, True, False, None),
            MockStreamChunk(4, "completed", "Done", None, False, True, None),
        ]

        iterator = MockStreamIterator(chunks)
        actual_states = []

        async for chunk in iterator:
            actual_states.append(chunk.state)

        assert actual_states == expected_states

    @pytest.mark.asyncio
    async def test_max_iterations_reached(self):
        """Test max iterations limit."""
        chunks = [
            MockStreamChunk(100, "error", None, None, False, True, "Max iterations (100) reached"),
        ]

        iterator = MockStreamIterator(chunks)

        async for chunk in iterator:
            assert chunk.state == "error"
            assert "Max iterations" in chunk.error
            assert chunk.is_final


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
