//! Server-Sent Events 解析器
//!
//! 解析 SSE 格式的流式数据，支持跨 chunk 的帧边界处理。

use anyhow::Result;

/// SSE 解析器
///
/// 解析 Server-Sent Events 格式的流式数据。
/// 支持跨 chunk 的帧边界处理。
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    provider: Option<String>,
    model: Option<String>,
}

impl SseParser {
    /// 创建新的 SSE 解析器
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加上下文信息（用于错误报告）
    #[must_use]
    pub fn with_context(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self.model = Some(model.into());
        self
    }

    /// 推送数据块并解析出完整的事件
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        while let Some(frame) = self.next_frame() {
            if let Some(event) = self.parse_frame(&frame)? {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// 完成解析，处理缓冲区中剩余的数据
    pub fn finish(&mut self) -> Result<Vec<SseEvent>> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }

        let trailing = std::mem::take(&mut self.buffer);
        match self.parse_frame(&String::from_utf8_lossy(&trailing))? {
            Some(event) => Ok(vec![event]),
            None => Ok(Vec::new()),
        }
    }

    fn next_frame(&mut self) -> Option<String> {
        // 查找 \n\n 或 \r\n\r\n 分隔符
        let separator = self
            .buffer
            .windows(2)
            .position(|window| window == b"\n\n")
            .map(|position| (position, 2))
            .or_else(|| {
                self.buffer
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|position| (position, 4))
            })?;

        let (position, separator_len) = separator;
        let frame = self
            .buffer
            .drain(..position + separator_len)
            .collect::<Vec<_>>();
        let frame_len = frame.len().saturating_sub(separator_len);
        Some(String::from_utf8_lossy(&frame[..frame_len]).into_owned())
    }

    fn parse_frame(&self, frame: &str) -> Result<Option<SseEvent>> {
        let trimmed = frame.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let mut data_lines = Vec::new();
        let mut event_name: Option<String> = None;

        for line in trimmed.lines() {
            // 跳过注释行
            if line.starts_with(':') {
                continue;
            }
            // 解析 event 字段
            if let Some(name) = line.strip_prefix("event:") {
                event_name = Some(name.trim().to_string());
                continue;
            }
            // 解析 data 字段
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start().to_string());
            }
        }

        // 跳过 ping 事件
        if matches!(event_name.as_deref(), Some("ping")) {
            return Ok(None);
        }

        if data_lines.is_empty() {
            return Ok(None);
        }

        let payload = data_lines.join("\n");

        // 处理 [DONE] 标记（OpenAI 格式）
        if payload == "[DONE]" {
            return Ok(None);
        }

        Ok(Some(SseEvent {
            event: event_name,
            data: payload,
        }))
    }
}

/// SSE 事件
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// 事件类型（可选）
    pub event: Option<String>,
    /// 事件数据
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_parses_single_frame() {
        let frame = concat!(
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n"
        );

        let mut parser = SseParser::new();
        let events = parser.push(frame.as_bytes()).expect("frame should parse");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("content_block_start".to_string()));
        assert!(events[0].data.contains("content_block_start"));
    }

    #[test]
    fn sse_parser_handles_chunked_stream() {
        let mut parser = SseParser::new();
        let first = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel";
        let second = b"lo\"}}\n\n";

        assert!(parser
            .push(first)
            .expect("first chunk should buffer")
            .is_empty());
        let events = parser.push(second).expect("second chunk should parse");

        assert_eq!(events.len(), 1);
        assert!(events[0].data.contains("Hello"));
    }

    #[test]
    fn sse_parser_ignores_ping_and_done() {
        let mut parser = SseParser::new();
        let payload = concat!(
            ": keepalive\n",
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
            "data: [DONE]\n\n"
        );

        let events = parser
            .push(payload.as_bytes())
            .expect("parser should succeed");
        assert_eq!(events.len(), 1); // Only message_delta, ping and [DONE] are ignored
    }
}
