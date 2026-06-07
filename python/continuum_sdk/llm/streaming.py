"""SSE Parser

Server-Sent Events parser with cross-chunk frame boundary handling.

Reference Rust implementation: layer1/src/streaming.rs

Features:
    - SSE frame parsing (cross-chunk)
    - Anthropic/OpenAI streaming format support
    - on_chunk callback mechanism
    - abort interruption support
"""

from __future__ import annotations

import json
from collections import deque
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable


@dataclass
class SseEvent:
    """SSE event"""

    event: str | None = None
    data: str = ""


class SseParser:
    """SSE Parser

    Parses Server-Sent Events format streaming data.
    Supports cross-chunk frame boundary handling.
    """

    def __init__(self, provider: str | None = None, model: str | None = None):
        """Initialize SSE parser

        Args:
            provider: Provider name (for error reporting)
            model: Model name (for error reporting)
        """
        self._buffer = ""
        self._provider = provider
        self._model = model

    def push(self, chunk: str) -> list[SseEvent]:
        """Push data chunk and parse complete events

        Args:
            chunk: Data chunk string

        Returns:
            List of parsed SSE events
        """
        self._buffer += chunk
        events: list[SseEvent] = []

        while True:
            frame = self._next_frame()
            if frame is None:
                break

            event = self._parse_frame(frame)
            if event is not None:
                events.append(event)

        return events

    def finish(self) -> list[SseEvent]:
        """Finish parsing, process remaining data in buffer

        Returns:
            List of remaining SSE events
        """
        if not self._buffer:
            return []

        trailing = self._buffer
        self._buffer = ""

        event = self._parse_frame(trailing)
        if event is not None:
            return [event]
        return []

    def _next_frame(self) -> str | None:
        """Extract the next frame"""
        # Find \n\n or \r\n\r\n separator
        separator_pos = None
        separator_len = 0

        # First find \n\n
        if "\n\n" in self._buffer:
            pos = self._buffer.index("\n\n")
            separator_pos = pos
            separator_len = 2
        # Then find \r\n\r\n
        elif "\r\n\r\n" in self._buffer:
            pos = self._buffer.index("\r\n\r\n")
            separator_pos = pos
            separator_len = 4

        if separator_pos is None:
            return None

        frame = self._buffer[:separator_pos]
        self._buffer = self._buffer[separator_pos + separator_len :]
        return frame

    def _parse_frame(self, frame: str) -> SseEvent | None:
        """Parse frame"""
        trimmed = frame.strip()
        if not trimmed:
            return None

        data_lines: list[str] = []
        event_name: str | None = None

        for line in trimmed.split("\n"):
            # Skip comment lines
            if line.startswith(":"):
                continue

            # Parse event field
            if line.startswith("event:"):
                event_name = line[6:].strip()
                continue

            # Parse data field
            if line.startswith("data:"):  # pragma: no branch
                data_lines.append(line[5:].lstrip())

        # Skip ping events
        if event_name == "ping":
            return None

        if not data_lines:
            return None

        payload = "\n".join(data_lines)

        # Handle [DONE] marker (OpenAI format)
        if payload == "[DONE]":
            return None

        return SseEvent(event=event_name, data=payload)


class ContentBlockType(Enum):
    """Content block type"""

    TEXT = "text"
    THINKING = "thinking"
    TOOL_USE = "tool_use"


@dataclass
class ContentBlockStart:
    """Content block start"""

    index: int
    block_type: ContentBlockType
    tool_id: str | None = None
    tool_name: str | None = None


@dataclass
class ContentBlockDelta:
    """Content block delta"""

    index: int
    delta_type: str  # "text", "thinking", "tool_input"
    content: str


@dataclass
class ContentBlockStop:
    """Content block stop"""

    index: int


@dataclass
class StreamUsage:
    """Stream usage"""

    input_tokens: int = 0
    output_tokens: int = 0


@dataclass
class StreamEvent:
    """Unified stream event"""

    event_type: str  # message_start, content_block_start, content_block_delta, content_block_stop, message_delta, message_stop
    data: dict[str, Any] = field(default_factory=dict)


class StreamState:
    """Stream response state

    Processes Anthropic and OpenAI stream events, converts to unified format.
    """

    def __init__(self, model: str):
        """Initialize stream state

        Args:
            model: Model name
        """
        self.model = model
        self.message_started = False
        self.text_started = False
        self.text_finished = False
        self.thinking_started = False
        self.thinking_finished = False
        self.finished = False
        self.stop_reason: str | None = None
        self.usage: StreamUsage | None = None
        self.tool_index_offset = 0

    def ingest_anthropic(self, event_data: dict[str, Any]) -> list[StreamEvent]:
        """Process Anthropic event"""
        events: list[StreamEvent] = []
        event_type = event_data.get("type", "")

        if event_type == "message_start":
            message = event_data.get("message", {})
            if not self.message_started:
                self.message_started = True
                events.append(StreamEvent(
                    event_type="message_start",
                    data={"id": message.get("id", ""), "model": message.get("model", self.model)}
                ))

        elif event_type == "content_block_start":
            index = event_data.get("index", 0)
            content_block = event_data.get("content_block", {})
            block_type_str = content_block.get("type", "text")

            block_type = ContentBlockType.TEXT
            tool_id = None
            tool_name = None

            if block_type_str == "text":
                block_type = ContentBlockType.TEXT
            elif block_type_str == "thinking":
                block_type = ContentBlockType.THINKING
            elif block_type_str == "tool_use":
                block_type = ContentBlockType.TOOL_USE
                tool_id = content_block.get("id")
                tool_name = content_block.get("name")

            events.append(StreamEvent(
                event_type="content_block_start",
                data={"index": index, "block_type": block_type.value, "tool_id": tool_id, "tool_name": tool_name}
            ))

        elif event_type == "content_block_delta":
            index = event_data.get("index", 0)
            delta = event_data.get("delta", {})
            delta_type = delta.get("type", "text_delta")

            content = ""
            if delta_type == "text_delta":
                content = delta.get("text", "")
            elif delta_type == "thinking_delta":
                content = delta.get("thinking", "")
            elif delta_type == "input_json_delta":
                content = delta.get("partial_json", "")

            events.append(StreamEvent(
                event_type="content_block_delta",
                data={"index": index, "delta_type": delta_type, "content": content}
            ))

        elif event_type == "content_block_stop":
            index = event_data.get("index", 0)
            events.append(StreamEvent(
                event_type="content_block_stop",
                data={"index": index}
            ))

        elif event_type == "message_delta":
            delta = event_data.get("delta", {})
            usage_data = event_data.get("usage", {})

            self.stop_reason = delta.get("stop_reason")
            self.usage = StreamUsage(
                input_tokens=usage_data.get("input_tokens", 0),
                output_tokens=usage_data.get("output_tokens", 0)
            )

            events.append(StreamEvent(
                event_type="message_delta",
                data={"stop_reason": self.stop_reason, "usage": {"input_tokens": self.usage.input_tokens, "output_tokens": self.usage.output_tokens}}
            ))

        elif event_type == "message_stop":
            events.append(StreamEvent(event_type="message_stop"))

        return events

    def ingest_openai(self, chunk_data: dict[str, Any]) -> list[StreamEvent]:
        """Process OpenAI event"""
        events: list[StreamEvent] = []

        if not self.message_started:
            self.message_started = True
            events.append(StreamEvent(
                event_type="message_start",
                data={"id": chunk_data.get("id", ""), "model": chunk_data.get("model", self.model)}
            ))

        # Handle usage
        usage_data = chunk_data.get("usage")
        if usage_data:
            self.usage = StreamUsage(
                input_tokens=usage_data.get("prompt_tokens", 0),
                output_tokens=usage_data.get("completion_tokens", 0)
            )

        choices = chunk_data.get("choices", [])
        for choice in choices:
            delta = choice.get("delta", {})

            # Handle reasoning_content (thinking content)
            reasoning = delta.get("reasoning_content")
            if reasoning:
                if not self.thinking_started:
                    self.thinking_started = True
                    events.append(StreamEvent(
                        event_type="content_block_start",
                        data={"index": 0, "block_type": "thinking"}
                    ))
                events.append(StreamEvent(
                    event_type="content_block_delta",
                    data={"index": 0, "delta_type": "thinking_delta", "content": reasoning}
                ))

            # Handle regular content
            content = delta.get("content")
            if content:
                # If there was a previous thinking block, close it first
                if self.thinking_started and not self.thinking_finished:
                    self.thinking_finished = True
                    events.append(StreamEvent(
                        event_type="content_block_stop",
                        data={"index": 0}
                    ))

                text_index = 1 if self.thinking_started else 0
                if not self.text_started:
                    self.text_started = True
                    events.append(StreamEvent(
                        event_type="content_block_start",
                        data={"index": text_index, "block_type": "text"}
                    ))
                events.append(StreamEvent(
                    event_type="content_block_delta",
                    data={"index": text_index, "delta_type": "text_delta", "content": content}
                ))

            # Handle finish reason
            finish_reason = choice.get("finish_reason")
            if finish_reason:
                self.stop_reason = self._normalize_finish_reason(finish_reason)

        return events

    def _normalize_finish_reason(self, reason: str) -> str:
        """Normalize OpenAI finish reason"""
        if reason == "stop":
            return "end_turn"
        elif reason == "tool_calls":
            return "tool_use"
        return reason

    def finish(self) -> list[StreamEvent]:
        """Finish stream processing"""
        if self.finished:
            return []
        self.finished = True

        events: list[StreamEvent] = []

        # Close thinking block
        if self.thinking_started and not self.thinking_finished:
            self.thinking_finished = True
            events.append(StreamEvent(event_type="content_block_stop", data={"index": 0}))

        # Close text block
        if self.text_started and not self.text_finished:
            self.text_finished = True
            text_index = 1 if self.thinking_started else 0
            events.append(StreamEvent(event_type="content_block_stop", data={"index": text_index}))

        # Send message delta
        if self.message_started:
            events.append(StreamEvent(
                event_type="message_delta",
                data={
                    "stop_reason": self.stop_reason or "end_turn",
                    "usage": {"input_tokens": self.usage.input_tokens if self.usage else 0, "output_tokens": self.usage.output_tokens if self.usage else 0}
                }
            ))
            events.append(StreamEvent(event_type="message_stop"))

        return events


class CallbackStream:
    """Callback-based stream response

    Supports on_chunk callback mechanism and abort interruption.
    """

    def __init__(
        self,
        on_chunk: Callable[[str], None] | None = None,
        on_event: Callable[[StreamEvent], None] | None = None,
    ):
        """Initialize callback stream

        Args:
            on_chunk: Text delta callback function
            on_event: Event callback function
        """
        self._on_chunk = on_chunk
        self._on_event = on_event
        self._abort_flag = False
        self._pending: deque[StreamEvent] = deque()

    def abort(self) -> None:
        """Request abort"""
        self._abort_flag = True

    def is_aborted(self) -> bool:
        """Check if aborted"""
        return self._abort_flag

    def push_sse_event(self, sse_event: SseEvent, state: StreamState, provider: str) -> list[StreamEvent]:
        """Push SSE event and process

        Args:
            sse_event: SSE event
            state: Stream state
            provider: Provider type

        Returns:
            List of processed stream events
        """
        if self._abort_flag:
            return []

        try:
            data = json.loads(sse_event.data)
        except json.JSONDecodeError:
            return []

        events: list[StreamEvent] = []
        if provider.lower() == "anthropic":
            events = state.ingest_anthropic(data)
        else:
            events = state.ingest_openai(data)

        # Trigger callbacks
        for event in events:
            if self._on_event:
                self._on_event(event)

            if event.event_type == "content_block_delta":
                content = event.data.get("content", "")
                if content and self._on_chunk:
                    self._on_chunk(content)

        return events

    def finish(self, state: StreamState) -> list[StreamEvent]:
        """Finish stream processing

        Args:
            state: Stream state

        Returns:
            List of finish events
        """
        if self._abort_flag:
            return []

        events = state.finish()

        for event in events:
            if self._on_event:
                self._on_event(event)

            if event.event_type == "content_block_delta":
                content = event.data.get("content", "")
                if content and self._on_chunk:
                    self._on_chunk(content)

        return events