//! 流式处理模块
//!
//! SSE、WebSocket、HTTP 等流式响应处理。
//!
//! ## 模块结构
//! - `sse`: SSE 解析器和事件处理
//! - `websocket`: WebSocket 适配器
//! - `http`: HTTP 流式适配器
//! - `providers`: LLM 提供商特定的流式格式

pub mod http;
pub mod providers;
pub mod sse;
pub mod websocket;

// Re-export from submodules
pub use http::{HttpAdapter, HttpConfig, HttpRequest, HttpResponseStream, SseStream};
pub use providers::{
    ContentBlockType, ContentDelta, StreamEvent, StreamProvider, StreamState, StreamUsage,
};
pub use sse::{SseEvent, SseParser};
pub use websocket::{WebSocketAdapter, WebSocketConfig, WebSocketMessage, WebSocketMessageStream};

use anyhow::Result;
use futures::Stream;
use reqwest::Response;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

// Re-export provider types used in parse_sse_event
pub use providers::{AnthropicStreamEvent, OllamaStreamChunk, OpenAiStreamChunk};

/// 流处理器（兼容旧 API）
pub struct StreamHandler;

impl StreamHandler {
    /// 创建 SSE 流
    pub fn create_sse_stream(
        source: impl Stream<Item = Result<String>> + Send + 'static,
    ) -> impl Stream<Item = Result<String>> {
        use futures::StreamExt;

        source.map(|item| match item {
            Ok(data) => Ok(format!("data: {}\n\n", data)),
            Err(e) => Err(e),
        })
    }
}

/// 可中断的流式响应
pub struct AbortableStream<S> {
    inner: S,
    abort_flag: Arc<AtomicBool>,
}

impl<S> AbortableStream<S> {
    /// 创建可中断的流
    pub fn new(inner: S, abort_flag: Arc<AtomicBool>) -> Self {
        Self { inner, abort_flag }
    }

    /// 检查是否已中断
    pub fn is_aborted(&self) -> bool {
        self.abort_flag.load(Ordering::Relaxed)
    }
}

impl<S, T> Stream for AbortableStream<S>
where
    S: Stream<Item = Result<T>> + Unpin,
{
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.abort_flag.load(Ordering::Relaxed) {
            return Poll::Ready(None);
        }
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

/// 流式消息（兼容旧 API）
pub struct MessageStream {
    response: Response,
    parser: SseParser,
    pending: VecDeque<StreamEvent>,
    done: bool,
    state: StreamState,
    provider: StreamProvider,
}

impl MessageStream {
    /// 创建新的流式消息
    pub fn new(response: Response, provider: StreamProvider, model: String) -> Self {
        let parser = SseParser::new().with_context(
            match provider {
                StreamProvider::Anthropic => "Anthropic",
                StreamProvider::OpenAI => "OpenAI",
                StreamProvider::Gemini => "Gemini",
                StreamProvider::AzureOpenAI => "AzureOpenAI",
                StreamProvider::Bedrock => "Bedrock",
                StreamProvider::Ollama => "Ollama",
            },
            &model,
        );
        Self {
            response,
            parser,
            pending: VecDeque::new(),
            done: false,
            state: StreamState::new(model),
            provider,
        }
    }

    /// 获取下一个事件
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }

            if self.done {
                let _remaining = self.parser.finish()?;
                for event in self.state.finish() {
                    self.pending.push_back(event);
                }
                if let Some(event) = self.pending.pop_front() {
                    return Ok(Some(event));
                }
                return Ok(None);
            }

            match self.response.chunk().await? {
                Some(chunk) => {
                    let sse_events = self.parser.push(&chunk)?;
                    for sse_event in sse_events {
                        let events = self.parse_sse_event(&sse_event)?;
                        self.pending.extend(events);
                    }
                }
                None => {
                    self.done = true;
                }
            }
        }
    }

    fn parse_sse_event(
        &mut self,
        event: &crate::streaming::sse::SseEvent,
    ) -> Result<Vec<StreamEvent>> {
        use crate::streaming::providers::*;

        match self.provider {
            StreamProvider::Anthropic => {
                let anthropic_event: AnthropicStreamEvent = serde_json::from_str(&event.data)?;
                Ok(self.state.ingest_anthropic(anthropic_event))
            }
            StreamProvider::OpenAI => {
                let openai_chunk: OpenAiStreamChunk = serde_json::from_str(&event.data)?;
                Ok(self.state.ingest_openai(openai_chunk))
            }
            StreamProvider::Gemini => {
                let openai_chunk: OpenAiStreamChunk = serde_json::from_str(&event.data)?;
                Ok(self.state.ingest_openai(openai_chunk))
            }
            StreamProvider::AzureOpenAI => {
                let openai_chunk: OpenAiStreamChunk = serde_json::from_str(&event.data)?;
                Ok(self.state.ingest_openai(openai_chunk))
            }
            StreamProvider::Bedrock => {
                let anthropic_event: AnthropicStreamEvent = serde_json::from_str(&event.data)?;
                Ok(self.state.ingest_anthropic(anthropic_event))
            }
            StreamProvider::Ollama => {
                let ollama_chunk: OllamaStreamChunk = serde_json::from_str(&event.data)?;
                Ok(self.state.ingest_ollama(ollama_chunk))
            }
        }
    }

    /// 收集所有文本内容
    pub async fn collect_text(&mut self) -> Result<String> {
        let mut text = String::new();
        while let Some(event) = self.next_event().await? {
            if let StreamEvent::ContentBlockDelta {
                delta: ContentDelta::Text(t),
                ..
            } = event
            {
                text.push_str(&t);
            }
        }
        Ok(text)
    }
}

/// 回调类型
pub type OnChunkCallback = Box<dyn Fn(&str) + Send + Sync>;

/// 带回调的流式响应
pub struct CallbackStream {
    inner: MessageStream,
    on_chunk: Option<OnChunkCallback>,
    abort_flag: Arc<AtomicBool>,
}

impl CallbackStream {
    /// 创建带回调的流
    pub fn new(inner: MessageStream, on_chunk: Option<OnChunkCallback>) -> Self {
        Self {
            inner,
            on_chunk,
            abort_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 获取中断标志
    pub fn abort_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort_flag)
    }

    /// 请求中断
    pub fn abort(&self) {
        self.abort_flag.store(true, Ordering::Relaxed);
    }

    /// 获取下一个事件
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>> {
        if self.abort_flag.load(Ordering::Relaxed) {
            return Ok(None);
        }

        let event = self.inner.next_event().await?;

        // 触发回调
        if let Some(ref callback) = self.on_chunk {
            if let Some(StreamEvent::ContentBlockDelta {
                delta: ContentDelta::Text(t),
                ..
            }) = event.as_ref()
            {
                callback(t);
            }
        }

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abortable_stream_respects_abort_flag() {
        use futures::stream;

        let abort_flag = Arc::new(AtomicBool::new(true));
        let inner = stream::iter(vec![Ok("test".to_string())]);
        let mut stream = AbortableStream::new(inner, abort_flag);

        let result = futures::executor::block_on_stream(&mut stream).next();
        assert!(
            result.is_none(),
            "aborted stream should return None immediately"
        );
    }
}
