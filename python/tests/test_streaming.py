"""Streaming Tests

测试 SSE 解析器和流式状态处理。
"""

import pytest

from continuum_sdk.llm.streaming import (
    CallbackStream,
    ContentBlockType,
    SseEvent,
    SseParser,
    StreamEvent,
    StreamState,
)


class TestSseParser:
    """测试 SSE 解析器"""

    def test_parse_single_frame(self):
        """解析单个帧"""
        frame = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n"

        parser = SseParser()
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].event == "content_block_start"
        assert "content_block_start" in events[0].data

    def test_parse_chunked_stream(self):
        """解析跨 chunk 的流"""
        parser = SseParser()

        # 第一块（不完整）
        first = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel"
        events = parser.push(first)
        assert len(events) == 0  # 还没有完整帧

        # 第二块（完成帧）
        second = "lo\"}}\n\n"
        events = parser.push(second)
        assert len(events) == 1
        assert "Hello" in events[0].data

    def test_ignore_ping_and_done(self):
        """忽略 ping 事件和 [DONE] 标记"""
        parser = SseParser()

        payload = ": keepalive\nevent: ping\ndata: {\"type\":\"ping\"}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\ndata: [DONE]\n\n"
        events = parser.push(payload)

        assert len(events) == 1  # 只有 message_delta，ping 和 [DONE] 被忽略
        assert events[0].event == "message_delta"

    def test_multiple_frames(self):
        """解析多个帧"""
        parser = SseParser()

        payload = "data: frame1\n\ndata: frame2\n\ndata: frame3\n\n"
        events = parser.push(payload)

        assert len(events) == 3
        assert events[0].data == "frame1"
        assert events[1].data == "frame2"
        assert events[2].data == "frame3"

    def test_finish_remaining_buffer(self):
        """完成时处理剩余缓冲区"""
        parser = SseParser()

        # 没有分隔符的不完整帧
        parser.push("data: incomplete")
        events = parser.finish()

        # finish 会尝试解析剩余缓冲区（即使没有分隔符）
        assert len(events) == 1
        assert events[0].data == "incomplete"

        # 完整帧应该在 push 时就被解析
        parser2 = SseParser()
        events = parser2.push("data: final\n\n")
        assert len(events) == 1
        events = parser2.finish()
        assert len(events) == 0

    def test_crlf_separator(self):
        """测试 CRLF 分隔符处理"""
        parser = SseParser()

        # 使用 \r\n\r\n 分隔符
        frame = "event: message\ndata: test\r\n\r\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].event == "message"
        assert events[0].data == "test"

    def test_parse_frame_empty_trimmed(self):
        """测试空帧处理"""
        parser = SseParser()

        # 只有空白字符的帧
        frame = "   \n\n"
        events = parser.push(frame)

        assert len(events) == 0

    def test_parse_frame_comment_line(self):
        """测试注释行处理"""
        parser = SseParser()

        # 包含注释行的帧
        frame = ": this is a comment\ndata: payload\n\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].data == "payload"

    def test_parse_frame_no_data_lines(self):
        """测试没有数据行的帧"""
        parser = SseParser()

        # 只有事件类型没有数据
        frame = "event: ping\n\n"
        events = parser.push(frame)

        assert len(events) == 0

        # 直接调用 _parse_frame 测试返回 None
        result = parser._parse_frame("event: ping\n\n")
        assert result is None

    def test_push_multiple_data_lines(self):
        """测试多个 data 行合并"""
        parser = SseParser()

        frame = "data: line1\ndata: line2\ndata: line3\n\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].data == "line1\nline2\nline3"

    def test_push_data_line_then_event_line_loop_back(self):
        """测试 data 行后跟着 event 行触发循环回跳 (branch 133->122)"""
        parser = SseParser()

        # 这个帧有 event 行和多个 data 行
        # 循环: line 133 -> append -> 122 (循环继续)
        frame = "event: test\ndata: line1\ndata: line2\n\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].event == "test"
        assert events[0].data == "line1\nline2"

    def test_push_comment_line_skips_to_loop(self):
        """测试注释行跳过后继续循环 (branch 125->122 via continue)"""
        parser = SseParser()

        # 包含注释行的帧
        frame = ": this is a comment\ndata: real data\n\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].data == "real data"

    def test_push_event_line_skips_to_loop(self):
        """测试 event 行处理后继续循环 (branch 130->122 via continue)"""
        parser = SseParser()

        # event 行后还有 data 行
        frame = "event: custom\ndata: data1\ndata: data2\n\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].event == "custom"

    def test_finish_returns_event(self):
        """测试 finish 返回事件"""
        parser = SseParser()

        # 不完整的帧（没有分隔符）
        parser._buffer = "data: incomplete"
        events = parser.finish()

        # _parse_frame 会被调用并返回事件
        assert len(events) == 1
        assert events[0].data == "incomplete"

    def test_finish_returns_empty_when_parse_frame_returns_none(self):
        """测试 finish 返回空列表"""
        parser = SseParser()

        # 空缓冲区应该返回空列表
        events = parser.finish()
        assert len(events) == 0

        # 只有空白字符的缓冲区也应该返回空列表
        parser2 = SseParser()
        parser2._buffer = "   \n  "
        events = parser2.finish()
        assert len(events) == 0


class TestStreamState:
    """测试流状态"""

    def test_anthropic_message_start(self):
        """处理 Anthropic message_start 事件"""
        state = StreamState("claude-sonnet-4-6")

        event_data = {
            "type": "message_start",
            "message": {
                "id": "msg_123",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-6",
            }
        }

        events = state.ingest_anthropic(event_data)
        assert len(events) == 1
        assert events[0].event_type == "message_start"
        assert events[0].data["id"] == "msg_123"

    def test_anthropic_message_start_already_started(self):
        """测试 message_start 已经开始的情况"""
        state = StreamState("claude-sonnet-4-6")

        # 第一个 message_start
        event_data = {
            "type": "message_start",
            "message": {"id": "msg_1", "model": "claude-sonnet-4-6"}
        }
        events = state.ingest_anthropic(event_data)
        assert len(events) == 1

        # 第二个 message_start（已开始，不应该产生事件）
        events = state.ingest_anthropic(event_data)
        assert len(events) == 0

    def test_anthropic_content_block_delta(self):
        """处理 Anthropic content_block_delta 事件"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": "Hello"
            }
        }

        events = state.ingest_anthropic(event_data)
        assert len(events) == 1
        assert events[0].event_type == "content_block_delta"
        assert events[0].data["content"] == "Hello"

    def test_anthropic_tool_use_block(self):
        """测试 Anthropic tool_use 内容块"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "tool_use",
                "id": "tool_123",
                "name": "search"
            }
        }
        events = state.ingest_anthropic(event_data)

        assert len(events) == 1
        assert events[0].data["block_type"] == "tool_use"
        assert events[0].data["tool_id"] == "tool_123"
        assert events[0].data["tool_name"] == "search"

    def test_anthropic_thinking_block(self):
        """测试 Anthropic thinking 内容块"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "thinking",
                "thinking": ""
            }
        }
        events = state.ingest_anthropic(event_data)

        assert len(events) == 1
        assert events[0].data["block_type"] == "thinking"

    def test_anthropic_thinking_delta(self):
        """测试 Anthropic thinking_delta"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "thinking_delta",
                "thinking": "Let me think..."
            }
        }
        events = state.ingest_anthropic(event_data)

        assert len(events) == 1
        assert events[0].data["delta_type"] == "thinking_delta"
        assert events[0].data["content"] == "Let me think..."

    def test_anthropic_input_json_delta(self):
        """测试 Anthropic input_json_delta"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "input_json_delta",
                "partial_json": '{"query": "test'
            }
        }
        events = state.ingest_anthropic(event_data)

        assert len(events) == 1
        assert events[0].data["delta_type"] == "input_json_delta"
        assert events[0].data["content"] == '{"query": "test'

    def test_anthropic_message_stop(self):
        """测试 Anthropic message_stop 事件"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {"type": "message_stop"}
        events = state.ingest_anthropic(event_data)

        assert len(events) == 1
        assert events[0].event_type == "message_stop"

    def test_openai_chunk(self):
        """处理 OpenAI 流式块"""
        state = StreamState("gpt-4o")

        chunk_data = {
            "id": "chatcmpl_123",
            "model": "gpt-4o",
            "choices": [{
                "delta": {"content": "Hello"},
                "finish_reason": None
            }]
        }

        events = state.ingest_openai(chunk_data)
        assert len(events) >= 2  # message_start + content_block_start + delta
        assert events[0].event_type == "message_start"

    def test_openai_with_reasoning(self):
        """处理 OpenAI reasoning_content"""
        state = StreamState("gpt-4o")

        # 先发送 reasoning
        chunk1 = {
            "id": "chatcmpl_123",
            "model": "gpt-4o",
            "choices": [{
                "delta": {"reasoning_content": "Thinking..."},
                "finish_reason": None
            }]
        }
        events = state.ingest_openai(chunk1)

        # 应该有 thinking block
        assert any(e.data.get("block_type") == "thinking" for e in events)

        # 然后发送内容
        chunk2 = {
            "id": "chatcmpl_123",
            "model": "gpt-4o",
            "choices": [{
                "delta": {"content": "Answer"},
                "finish_reason": None
            }]
        }
        events = state.ingest_openai(chunk2)

        # 应该关闭 thinking block 并开始 text block
        assert any(e.event_type == "content_block_stop" for e in events)

    def test_openai_reasoning_already_started(self):
        """测试 OpenAI reasoning 已开始的情况"""
        state = StreamState("gpt-4o")
        state.message_started = True
        state.thinking_started = True

        # 第二次发送 reasoning，thinking_started 已经是 True
        chunk = {
            "id": "chatcmpl_1",
            "model": "gpt-4o",
            "choices": [{
                "delta": {"reasoning_content": "More thoughts..."},
                "finish_reason": None
            }]
        }
        events = state.ingest_openai(chunk)

        # 应该只有 delta 事件，没有 start 事件（因为已经开始了）
        start_events = [e for e in events if e.event_type == "content_block_start"]
        assert len(start_events) == 0

        delta_events = [e for e in events if e.event_type == "content_block_delta"]
        assert len(delta_events) == 1
        assert delta_events[0].data["content"] == "More thoughts..."

    def test_openai_usage(self):
        """测试 OpenAI usage 数据"""
        state = StreamState("gpt-4o")

        chunk_data = {
            "id": "chatcmpl_1",
            "model": "gpt-4o",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20
            },
            "choices": []
        }
        state.ingest_openai(chunk_data)

        assert state.usage is not None
        assert state.usage.input_tokens == 10
        assert state.usage.output_tokens == 20

    def test_openai_reasoning_content(self):
        """测试 OpenAI reasoning_content 处理"""
        state = StreamState("gpt-4o")

        # 第一个 chunk 包含 reasoning
        chunk = {
            "id": "chatcmpl_1",
            "model": "gpt-4o",
            "choices": [{
                "delta": {"reasoning_content": "Thinking..."},
                "finish_reason": None
            }]
        }
        events = state.ingest_openai(chunk)

        # 应该产生 thinking block start 和 delta
        assert any(e.event_type == "content_block_start" and e.data.get("block_type") == "thinking" for e in events)
        assert any(e.event_type == "content_block_delta" and e.data.get("delta_type") == "thinking_delta" for e in events)

    def test_openai_normalize_finish_reason(self):
        """测试 OpenAI finish reason 标准化"""
        state = StreamState("gpt-4o")

        # stop -> end_turn
        chunk = {
            "id": "chatcmpl_1",
            "model": "gpt-4o",
            "choices": [{"delta": {}, "finish_reason": "stop"}]
        }
        state.ingest_openai(chunk)
        assert state.stop_reason == "end_turn"

        # tool_calls -> tool_use
        state2 = StreamState("gpt-4o")
        chunk2 = {
            "id": "chatcmpl_2",
            "model": "gpt-4o",
            "choices": [{"delta": {}, "finish_reason": "tool_calls"}]
        }
        state2.ingest_openai(chunk2)
        assert state2.stop_reason == "tool_use"

        # 其他原因保持不变
        state3 = StreamState("gpt-4o")
        chunk3 = {
            "id": "chatcmpl_3",
            "model": "gpt-4o",
            "choices": [{"delta": {}, "finish_reason": "length"}]
        }
        state3.ingest_openai(chunk3)
        assert state3.stop_reason == "length"

    def test_finish(self):
        """完成流处理"""
        state = StreamState("gpt-4o")
        state.message_started = True
        state.text_started = True

        events = state.finish()
        assert len(events) >= 2  # message_delta + message_stop
        assert events[-1].event_type == "message_stop"

    def test_finish_already_finished(self):
        """测试 finish 已经完成的情况"""
        state = StreamState("gpt-4o")
        state.message_started = True
        state.finished = True

        events = state.finish()
        assert len(events) == 0

    def test_finish_close_thinking_block(self):
        """测试 finish 关闭 thinking 块"""
        state = StreamState("gpt-4o")
        state.message_started = True
        state.thinking_started = True
        state.thinking_finished = False

        events = state.finish()
        assert any(e.event_type == "content_block_stop" and e.data.get("index") == 0 for e in events)

    def test_finish_close_text_block_with_thinking(self):
        """测试 finish 在有 thinking 的情况下关闭 text 块"""
        state = StreamState("gpt-4o")
        state.message_started = True
        state.thinking_started = True
        state.thinking_finished = True
        state.text_started = True
        state.text_finished = False

        events = state.finish()
        # text block 应该在 index 1
        assert any(e.event_type == "content_block_stop" and e.data.get("index") == 1 for e in events)

    def test_finish_close_text_block_without_thinking(self):
        """测试 finish 在没有 thinking 的情况下关闭 text 块"""
        state = StreamState("gpt-4o")
        state.message_started = True
        state.text_started = True
        state.text_finished = False

        events = state.finish()
        # text block 应该在 index 0
        assert any(e.event_type == "content_block_stop" and e.data.get("index") == 0 for e in events)

    def test_finish_with_message_started(self):
        """测试 finish 当 message_started 为 True"""
        state = StreamState("gpt-4o")
        state.message_started = True

        events = state.finish()
        # 应该产生 message_delta 和 message_stop
        assert len(events) == 2
        assert events[0].event_type == "message_delta"
        assert events[1].event_type == "message_stop"

    def test_finish_without_message_started(self):
        """测试 finish 当 message_started 为 False"""
        state = StreamState("gpt-4o")
        state.message_started = False

        events = state.finish()
        # 不应该产生任何事件
        assert len(events) == 0


class TestCallbackStream:
    """测试回调流"""

    def test_on_chunk_callback(self):
        """测试 on_chunk 回调"""
        chunks_received: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks_received.append(c))
        state = StreamState("claude-sonnet-4-6")

        sse_event = SseEvent(
            event="content_block_delta",
            data='{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}'
        )

        stream.push_sse_event(sse_event, state, "anthropic")

        assert len(chunks_received) == 1
        assert chunks_received[0] == "Hello"

    def test_on_event_callback(self):
        """测试 on_event 回调"""
        events_received: list[StreamEvent] = []

        stream = CallbackStream(on_event=lambda e: events_received.append(e))
        state = StreamState("claude-sonnet-4-6")

        sse_event = SseEvent(
            event="message_start",
            data='{"type":"message_start","message":{"id":"msg_123","model":"claude-sonnet-4-6"}}'
        )

        stream.push_sse_event(sse_event, state, "anthropic")

        assert len(events_received) == 1
        assert events_received[0].event_type == "message_start"

    def test_abort(self):
        """测试中断"""
        chunks_received: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks_received.append(c))
        state = StreamState("claude-sonnet-4-6")

        # 中断
        stream.abort()

        sse_event = SseEvent(
            event="content_block_delta",
            data='{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}'
        )

        events = stream.push_sse_event(sse_event, state, "anthropic")

        # 中断后不应该有事件
        assert len(events) == 0
        assert len(chunks_received) == 0

    def test_is_aborted(self):
        """测试中断检查"""
        stream = CallbackStream()
        assert not stream.is_aborted()

        stream.abort()
        assert stream.is_aborted()

    def test_push_sse_event_json_decode_error(self):
        """测试 JSON 解析错误"""
        stream = CallbackStream()
        state = StreamState("claude-sonnet-4-6")

        sse_event = SseEvent(
            event="content_block_delta",
            data="invalid json {{{"
        )

        events = stream.push_sse_event(sse_event, state, "anthropic")
        assert len(events) == 0

    def test_push_sse_event_empty_content(self):
        """测试空内容不触发回调"""
        chunks_received: list[str] = []
        stream = CallbackStream(on_chunk=lambda c: chunks_received.append(c))
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        sse_event = SseEvent(
            event="content_block_delta",
            data='{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":""}}'
        )

        stream.push_sse_event(sse_event, state, "anthropic")
        assert len(chunks_received) == 0

    def test_finish_with_abort(self):
        """测试中断后的 finish"""
        stream = CallbackStream()
        state = StreamState("gpt-4o")
        state.message_started = True

        stream.abort()
        events = stream.finish(state)
        assert len(events) == 0

    def test_finish_with_callbacks(self):
        """测试 finish 触发回调"""
        chunks_received: list[str] = []
        events_received: list[StreamEvent] = []

        stream = CallbackStream(
            on_chunk=lambda c: chunks_received.append(c),
            on_event=lambda e: events_received.append(e)
        )
        state = StreamState("gpt-4o")
        state.message_started = True
        state.text_started = True

        events = stream.finish(state)
        assert len(events_received) == len(events)

    def test_finish_with_content_callback(self):
        """测试 finish 中的内容触发 on_chunk"""
        chunks_received: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks_received.append(c))
        state = StreamState("gpt-4o")
        state.message_started = True
        state.text_started = True
        state.text_finished = False

        stream.finish(state)
        # finish 不产生新的 content，只有 stop 事件
        # 所以 chunks_received 应该为空
        assert len(chunks_received) == 0

    def test_push_sse_event_non_anthropic_provider(self):
        """测试非 Anthropic provider 路由到 OpenAI 处理"""
        stream = CallbackStream()
        state = StreamState("gpt-4o")

        sse_event = SseEvent(
            event="message_start",
            data='{"id": "chatcmpl_1", "model": "gpt-4o", "choices": []}'
        )

        stream.push_sse_event(sse_event, state, "openai")

        # 应该调用 ingest_openai
        assert state.message_started is True

    def test_push_sse_event_provider_case_insensitive(self):
        """测试 provider 大小写不敏感"""
        stream = CallbackStream()
        state = StreamState("claude-sonnet-4-6")

        sse_event = SseEvent(
            event="message_start",
            data='{"type": "message_start", "message": {"id": "msg_1"}}'
        )

        # ANTHropic 大写也应该路由到 ingest_anthropic
        events = stream.push_sse_event(sse_event, state, "ANTHROPIC")
        assert len(events) == 1
        assert events[0].event_type == "message_start"

    def test_finish_with_content_block_delta_event(self):
        """测试 finish 中 content_block_delta 事件触发 on_chunk (lines 501-503)"""

        chunks_received: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks_received.append(c))
        state = StreamState("gpt-4o")
        state.message_started = True

        # Mock state.finish() to return a content_block_delta event
        # This tests the edge case where finish returns content_block_delta
        mock_event = StreamEvent(
            event_type="content_block_delta",
            data={"content": "final chunk"}
        )
        state.finish = lambda: [mock_event]

        stream.finish(state)

        # on_chunk should have been called with "final chunk"
        assert len(chunks_received) == 1
        assert chunks_received[0] == "final chunk"

    def test_finish_with_empty_content_in_delta(self):
        """测试 finish 中 content_block_delta 空内容不触发 on_chunk (line 502)"""

        chunks_received: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks_received.append(c))
        state = StreamState("gpt-4o")
        state.message_started = True

        # Mock state.finish() to return a content_block_delta with empty content
        mock_event = StreamEvent(
            event_type="content_block_delta",
            data={"content": ""}
        )
        state.finish = lambda: [mock_event]

        stream.finish(state)

        # on_chunk should NOT have been called (content is empty)
        assert len(chunks_received) == 0


class TestIntegration:
    """集成测试"""

    def test_full_anthropic_stream(self):
        """完整的 Anthropic 流处理"""
        parser = SseParser()
        state = StreamState("claude-sonnet-4-6")
        chunks: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks.append(c))

        # 模拟 Anthropic 流式响应
        frames = [
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-6\"}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ]

        for frame in frames:
            sse_events = parser.push(frame)
            for sse_event in sse_events:
                stream.push_sse_event(sse_event, state, "anthropic")

        # 验证收集的文本
        assert chunks == ["Hello", " world"]

    def test_full_openai_stream(self):
        """完整的 OpenAI 流处理"""
        parser = SseParser()
        state = StreamState("gpt-4o")
        chunks: list[str] = []

        stream = CallbackStream(on_chunk=lambda c: chunks.append(c))

        # 模拟 OpenAI 流式响应
        frames = [
            "data: {\"id\":\"chatcmpl_1\",\"model\":\"gpt-4o\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl_1\",\"model\":\"gpt-4o\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl_1\",\"model\":\"gpt-4o\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        ]

        for frame in frames:
            sse_events = parser.push(frame)
            for sse_event in sse_events:
                stream.push_sse_event(sse_event, state, "openai")

        # 验证收集的文本
        assert chunks == ["Hello", " world"]


class TestContentBlockType:
    """测试 ContentBlockType 枚举"""

    def test_content_block_type_values(self):
        """测试枚举值"""
        assert ContentBlockType.TEXT.value == "text"
        assert ContentBlockType.THINKING.value == "thinking"
        assert ContentBlockType.TOOL_USE.value == "tool_use"


class TestSseParserEdgeCases:
    """测试 SSE 解析器边界情况"""

    def test_next_frame_loop_continuation(self):
        """测试 _next_frame 循环继续分支"""
        parser = SseParser()

        # 先放入一个完整帧
        parser.push("data: first\n\n")
        assert parser._buffer == ""

        # 再放入两个帧
        events = parser.push("data: second\n\ndata: third\n\n")
        assert len(events) == 2

    def test_parse_frame_returns_none_no_data(self):
        """测试 _parse_frame 没有 data 行返回 None"""
        parser = SseParser()

        # 直接测试 _parse_frame
        result = parser._parse_frame("event: something\n\n")
        assert result is None

        # 另一个测试：只有注释
        result = parser._parse_frame(": comment\n\n")
        assert result is None

    def test_finish_with_non_empty_buffer_returns_event(self):
        """测试 finish 有非空缓冲区返回事件"""
        parser = SseParser()

        # 放入不完整数据
        parser._buffer = "data: partial"
        events = parser.finish()

        assert len(events) == 1
        assert events[0].data == "partial"

    def test_parse_frame_with_data_after_event(self):
        """测试 event 行后有 data 行的循环继续"""
        parser = SseParser()

        # event: 后面有 data: 的帧
        frame = "event: test\ndata: payload\n\n"
        events = parser.push(frame)

        assert len(events) == 1
        assert events[0].event == "test"
        assert events[0].data == "payload"


class TestStreamStateEdgeCases:
    """测试流状态边界情况"""

    def test_anthropic_unknown_block_type(self):
        """测试 Anthropic 未知块类型 - 走 else 分支"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        # 使用未知的 block_type
        event_data = {
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "unknown_type"
            }
        }
        events = state.ingest_anthropic(event_data)

        # 应该产生事件，block_type 为默认的 TEXT
        assert len(events) == 1
        assert events[0].data["block_type"] == "text"

    def test_anthropic_unknown_delta_type(self):
        """测试 Anthropic 未知 delta 类型 - 走 else 分支"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "unknown_delta_type",
                "value": "test"
            }
        }
        events = state.ingest_anthropic(event_data)

        # 应该产生事件，content 为空字符串
        assert len(events) == 1
        assert events[0].data["content"] == ""

    def test_anthropic_unknown_event_type(self):
        """测试 Anthropic 未知事件类型"""
        state = StreamState("claude-sonnet-4-6")
        state.message_started = True

        event_data = {
            "type": "unknown_event_type"
        }
        events = state.ingest_anthropic(event_data)

        # 未知事件类型应该返回空列表
        assert len(events) == 0


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
