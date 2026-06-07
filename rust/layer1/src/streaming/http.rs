//! HTTP 流式适配器
//!
//! 提供基于 reqwest 的 HTTP 流式响应处理。
//!
//! ## 特性
//! - 流式请求/响应
//! - SSE (Server-Sent Events) 支持
//! - 请求超时和重试
//! - 请求/响应拦截器

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::Stream;
use reqwest::{Client, Response, StatusCode};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::streaming::providers::StreamEvent;
use crate::streaming::sse::SseEvent;
use crate::streaming::sse::SseParser;

/// HTTP 配置
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// 请求超时（毫秒）
    pub timeout_ms: u64,
    /// 连接超时（毫秒）
    pub connect_timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_interval_ms: u64,
    /// 流式读取超时（毫秒）
    pub stream_timeout_ms: u64,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30000,
            connect_timeout_ms: 10000,
            max_retries: 3,
            retry_interval_ms: 1000,
            stream_timeout_ms: 60000,
        }
    }
}

/// HTTP 请求方法
#[derive(Debug, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

/// HTTP 请求配置
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// URL
    pub url: String,
    /// 方法
    pub method: HttpMethod,
    /// 头部
    pub headers: Vec<(String, String)>,
    /// 请求体
    pub body: Option<serde_json::Value>,
}

impl HttpRequest {
    /// 创建 GET 请求
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Get,
            headers: Vec::new(),
            body: None,
        }
    }

    /// 创建 POST 请求
    pub fn post(url: impl Into<String>, body: serde_json::Value) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Post,
            headers: Vec::new(),
            body: Some(body),
        }
    }

    /// 添加头部
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    /// 添加 Authorization 头部
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.headers.push((
            "Authorization".to_string(),
            format!("Bearer {}", token.into()),
        ));
        self
    }

    /// 添加 API Key 头部
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.headers.push(("x-api-key".to_string(), key.into()));
        self
    }
}

/// HTTP 响应流
///
/// 包装 reqwest Response，提供流式数据读取
pub struct HttpResponseStream {
    response: Response,
    parser: SseParser,
    pending: VecDeque<StreamEvent>,
    done: bool,
    abort_flag: Arc<AtomicBool>,
}

impl HttpResponseStream {
    /// 创建新的响应流
    pub fn new(response: Response, abort_flag: Arc<AtomicBool>) -> Self {
        Self {
            response,
            parser: SseParser::new(),
            pending: VecDeque::new(),
            done: false,
            abort_flag,
        }
    }

    /// 获取响应状态码
    pub fn status(&self) -> StatusCode {
        self.response.status()
    }

    /// 获取响应头部
    pub fn headers(&self) -> &reqwest::header::HeaderMap {
        self.response.headers()
    }

    /// 读取下一个事件
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>> {
        // 使用简化的 SSE 事件返回
        loop {
            if self.abort_flag.load(Ordering::Relaxed) {
                return Ok(None);
            }

            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }

            if self.done {
                let _remaining = self.parser.finish()?;
                if let Some(event) = self.pending.pop_front() {
                    return Ok(Some(event));
                }
                return Ok(None);
            }

            match self.response.chunk().await? {
                Some(chunk) => {
                    let sse_events = self.parser.push(&chunk)?;
                    for _sse_event in sse_events {
                        // 将 SSE 事件转换为流事件（简化版）
                        self.pending.push_back(StreamEvent::MessageStart {
                            id: String::new(),
                            model: String::new(),
                        });
                    }
                }
                None => {
                    self.done = true;
                }
            }
        }
    }

    /// 收集所有文本内容
    pub async fn collect_text(&mut self) -> Result<String> {
        let mut result = String::new();
        while let Some(event) = self.next_event().await? {
            if let StreamEvent::ContentBlockDelta {
                delta: crate::streaming::providers::ContentDelta::Text(t),
                ..
            } = event
            {
                result.push_str(&t);
            }
        }
        Ok(result)
    }

    /// 创建 SSE 事件流
    pub fn into_sse_stream(mut self) -> impl Stream<Item = Result<SseEvent>> {
        async_stream::stream! {
            loop {
                if self.abort_flag.load(Ordering::Relaxed) {
                    break;
                }

                match self.response.chunk().await {
                    Ok(Some(chunk)) => {
                        let events = self.parser.push(&chunk)?;
                        for event in events {
                            yield Ok(event);
                        }
                    }
                    Ok(None) => {
                        let remaining = self.parser.finish()?;
                        for event in remaining {
                            yield Ok(event);
                        }
                        break;
                    }
                    Err(e) => {
                        yield Err(anyhow!("Stream error: {}", e));
                        break;
                    }
                }
            }
        }
    }
}

/// HTTP 适配器
///
/// 提供高级 HTTP 请求功能，包括流式响应处理
pub struct HttpAdapter {
    /// HTTP 客户端
    client: Client,
    /// 配置
    config: HttpConfig,
    /// 中断标志
    abort_flag: Arc<AtomicBool>,
}

impl HttpAdapter {
    /// 创建新的 HTTP 适配器
    pub fn new() -> Self {
        Self::with_config(HttpConfig::default())
    }

    /// 创建带配置的 HTTP 适配器
    pub fn with_config(config: HttpConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
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

    /// 重置中断标志
    pub fn reset(&self) {
        self.abort_flag.store(false, Ordering::Relaxed);
    }

    /// 检查是否已中断
    pub fn is_aborted(&self) -> bool {
        self.abort_flag.load(Ordering::Relaxed)
    }

    /// 执行 HTTP 请求
    pub async fn request(&self, request: HttpRequest) -> Result<Response> {
        self.request_with_retry(request, self.config.max_retries)
            .await
    }

    /// 执行带重试的 HTTP 请求
    async fn request_with_retry(&self, request: HttpRequest, max_retries: u32) -> Result<Response> {
        let mut attempts = 0;

        loop {
            if self.is_aborted() {
                return Err(anyhow!("Request aborted"));
            }

            attempts += 1;

            let result = self.execute_request(&request).await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return Ok(response);
                    }

                    // 如果是可重试的错误状态码，继续重试
                    if Self::is_retryable_status(status) && attempts <= max_retries {
                        tracing::warn!(
                            "HTTP request failed with status {}, attempt {}/{}",
                            status,
                            attempts,
                            max_retries
                        );
                        let delay = Duration::from_millis(
                            self.config.retry_interval_ms * (1 << (attempts - 1)),
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    // 其他错误，返回响应体内容
                    let body = response.text().await.unwrap_or_default();
                    return Err(anyhow!("HTTP {}: {}", status, body));
                }
                Err(e) => {
                    // 网络错误等可重试
                    if Self::is_retryable_error(&e) && attempts <= max_retries {
                        tracing::warn!(
                            "HTTP request error: {}, attempt {}/{}",
                            e,
                            attempts,
                            max_retries
                        );
                        let delay = Duration::from_millis(
                            self.config.retry_interval_ms * (1 << (attempts - 1)),
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }

    /// 执行单次 HTTP 请求
    async fn execute_request(&self, request: &HttpRequest) -> Result<Response> {
        let builder = match request.method {
            HttpMethod::Get => self.client.get(&request.url),
            HttpMethod::Post => self.client.post(&request.url),
            HttpMethod::Put => self.client.put(&request.url),
            HttpMethod::Delete => self.client.delete(&request.url),
            HttpMethod::Patch => self.client.patch(&request.url),
        };

        // 添加头部
        let builder = request
            .headers
            .iter()
            .fold(builder, |b, (k, v)| b.header(k, v));

        // 添加请求体
        let builder = if let Some(body) = &request.body {
            builder.json(body)
        } else {
            builder
        };

        let response = builder.send().await?;
        Ok(response)
    }

    /// 执行流式 HTTP 请求
    pub async fn request_stream(&self, request: HttpRequest) -> Result<HttpResponseStream> {
        let response = self.request(request).await?;
        Ok(HttpResponseStream::new(response, self.abort_flag.clone()))
    }

    /// 执行 SSE 流式请求
    pub async fn request_sse(&self, request: HttpRequest) -> Result<SseStream> {
        let builder = self.client.post(&request.url);

        let builder = request
            .headers
            .iter()
            .fold(builder, |b, (k, v)| b.header(k, v));

        let builder = if let Some(body) = &request.body {
            builder.json(body)
        } else {
            builder
        };

        let builder = builder.header("Accept", "text/event-stream");

        let response = builder.send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "SSE request failed with status {}: {}",
                status,
                body
            ));
        }

        Ok(SseStream::new(response, self.abort_flag.clone()))
    }

    /// 检查状态码是否可重试
    fn is_retryable_status(status: StatusCode) -> bool {
        matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
    }

    /// 检查错误是否可重试
    fn is_retryable_error(error: &anyhow::Error) -> bool {
        let msg = error.to_string().to_lowercase();
        msg.contains("timeout")
            || msg.contains("connection")
            || msg.contains("network")
            || msg.contains("429")
            || msg.contains("overloaded")
    }
}

impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// SSE 流
///
/// 专门处理 Server-Sent Events 的流式响应
pub struct SseStream {
    response: Response,
    parser: SseParser,
    abort_flag: Arc<AtomicBool>,
    done: bool,
}

impl SseStream {
    /// 创建新的 SSE 流
    pub fn new(response: Response, abort_flag: Arc<AtomicBool>) -> Self {
        Self {
            response,
            parser: SseParser::new(),
            abort_flag,
            done: false,
        }
    }

    /// 获取下一个 SSE 事件
    pub async fn next_event(&mut self) -> Result<Option<SseEvent>> {
        loop {
            if self.abort_flag.load(Ordering::Relaxed) {
                return Ok(None);
            }

            if self.done {
                let remaining = self.parser.finish()?;
                if remaining.is_empty() {
                    return Ok(None);
                }
                // 返回第一个剩余事件
                return Ok(remaining.into_iter().next());
            }

            match self.response.chunk().await? {
                Some(chunk) => {
                    let events = self.parser.push(&chunk)?;
                    if !events.is_empty() {
                        return Ok(Some(events.into_iter().next().unwrap()));
                    }
                }
                None => {
                    self.done = true;
                }
            }
        }
    }

    /// 收集所有事件
    pub async fn collect_events(&mut self) -> Result<Vec<SseEvent>> {
        let mut events = Vec::new();
        while let Some(event) = self.next_event().await? {
            events.push(event);
        }
        Ok(events)
    }
}

/// HTTP 适配器 trait
///
/// 定义 HTTP 请求的标准接口
#[async_trait]
pub trait HttpAdapterTrait: Send + Sync {
    /// 执行 HTTP GET 请求
    async fn get(&self, url: &str) -> Result<String>;

    /// 执行 HTTP POST 请求
    async fn post(&self, url: &str, body: serde_json::Value) -> Result<String>;

    /// 执行流式 HTTP POST 请求
    async fn post_stream(&self, url: &str, body: serde_json::Value) -> Result<HttpResponseStream>;

    /// 执行 SSE 流式请求
    async fn post_sse(&self, url: &str, body: serde_json::Value) -> Result<SseStream>;
}

#[async_trait]
impl HttpAdapterTrait for HttpAdapter {
    async fn get(&self, url: &str) -> Result<String> {
        let request = HttpRequest::get(url);
        let response = self.request(request).await?;
        let text = response.text().await?;
        Ok(text)
    }

    async fn post(&self, url: &str, body: serde_json::Value) -> Result<String> {
        let request = HttpRequest::post(url, body);
        let response = self.request(request).await?;
        let text = response.text().await?;
        Ok(text)
    }

    async fn post_stream(&self, url: &str, body: serde_json::Value) -> Result<HttpResponseStream> {
        let request = HttpRequest::post(url, body);
        self.request_stream(request).await
    }

    async fn post_sse(&self, url: &str, body: serde_json::Value) -> Result<SseStream> {
        let request = HttpRequest::post(url, body).header("Accept", "text/event-stream");
        self.request_sse(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_config_default() {
        let config = HttpConfig::default();
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.connect_timeout_ms, 10000);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_http_request_builder() {
        let request = HttpRequest::get("https://api.example.com")
            .bearer_auth("token123")
            .header("X-Custom", "value");

        assert_eq!(request.url, "https://api.example.com");
        assert_eq!(request.headers.len(), 2);
    }

    #[test]
    fn test_http_request_post() {
        let body = serde_json::json!({"key": "value"});
        let request = HttpRequest::post("https://api.example.com", body.clone());

        assert_eq!(request.url, "https://api.example.com");
        assert!(matches!(request.method, HttpMethod::Post));
        assert_eq!(request.body, Some(body));
    }

    #[test]
    fn test_is_retryable_status() {
        assert!(HttpAdapter::is_retryable_status(
            StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(HttpAdapter::is_retryable_status(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(HttpAdapter::is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(HttpAdapter::is_retryable_status(
            StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(HttpAdapter::is_retryable_status(
            StatusCode::GATEWAY_TIMEOUT
        ));

        assert!(!HttpAdapter::is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!HttpAdapter::is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!HttpAdapter::is_retryable_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn test_http_adapter_creation() {
        let adapter = HttpAdapter::new();
        assert!(!adapter.is_aborted());
    }

    #[test]
    fn test_http_adapter_abort() {
        let adapter = HttpAdapter::new();
        adapter.abort();
        assert!(adapter.is_aborted());
        adapter.reset();
        assert!(!adapter.is_aborted());
    }
}
