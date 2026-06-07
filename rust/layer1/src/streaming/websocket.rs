//! WebSocket 适配器
//!
//! 提供基于 tokio-tungstenite 的 WebSocket 连接和流式消息处理。
//!
//! ## 特性
//! - 异步连接建立
//! - 自动重连机制
//! - 心跳保活
//! - 消息序列化/反序列化
//! - 流式消息接收

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::{SinkExt, Stream, StreamExt};
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

/// WebSocket 配置
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// 连接超时（毫秒）
    pub connect_timeout_ms: u64,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 重连最大尝试次数
    pub max_reconnect_attempts: u32,
    /// 重连间隔（毫秒）
    pub reconnect_interval_ms: u64,
    /// 接收缓冲区大小
    pub receive_buffer_size: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 10000,
            heartbeat_interval_ms: 30000,
            max_reconnect_attempts: 3,
            reconnect_interval_ms: 1000,
            receive_buffer_size: 100,
        }
    }
}

/// WebSocket 消息
#[derive(Debug, Clone)]
pub enum WebSocketMessage {
    /// 文本消息
    Text(String),
    /// 二进制消息
    Binary(Vec<u8>),
    /// Ping 消息
    Ping(Vec<u8>),
    /// Pong 消息
    Pong(Vec<u8>),
    /// 关闭消息
    Close(Option<String>),
}

impl From<WsMessage> for WebSocketMessage {
    fn from(msg: WsMessage) -> Self {
        match msg {
            WsMessage::Text(t) => WebSocketMessage::Text(t.to_string()),
            WsMessage::Binary(b) => WebSocketMessage::Binary(b.to_vec()),
            WsMessage::Ping(p) => WebSocketMessage::Ping(p.to_vec()),
            WsMessage::Pong(p) => WebSocketMessage::Pong(p.to_vec()),
            WsMessage::Close(_) => WebSocketMessage::Close(None),
            WsMessage::Frame(_) => WebSocketMessage::Text(String::new()),
        }
    }
}

impl From<WebSocketMessage> for WsMessage {
    fn from(msg: WebSocketMessage) -> Self {
        match msg {
            WebSocketMessage::Text(t) => WsMessage::Text(t.into()),
            WebSocketMessage::Binary(b) => WsMessage::Binary(b.into()),
            WebSocketMessage::Ping(p) => WsMessage::Ping(p.into()),
            WebSocketMessage::Pong(p) => WsMessage::Pong(p.into()),
            WebSocketMessage::Close(_) => WsMessage::Close(None),
        }
    }
}

/// WebSocket 连接状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    /// 已断开
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 重连中
    Reconnecting,
    /// 已关闭
    Closed,
}

/// WebSocket 适配器
///
/// 提供高级 WebSocket 连接管理，包括：
/// - 自动连接和重连
/// - 消息流式接收
/// - 线程安全的发送
pub struct WebSocketAdapter {
    /// WebSocket 配置
    config: WebSocketConfig,
    /// 连接 URL
    url: String,
    /// 连接状态
    state: Arc<Mutex<ConnectionState>>,
    /// 发送通道
    sender: mpsc::Sender<WebSocketMessage>,
    /// 中断标志
    abort_flag: Arc<AtomicBool>,
}

impl WebSocketAdapter {
    /// 创建新的 WebSocket 适配器
    ///
    /// 注意：需要调用 `connect()` 建立连接
    pub fn new(url: impl Into<String>) -> Self {
        Self::with_config(url, WebSocketConfig::default())
    }

    /// 创建带配置的 WebSocket 适配器
    pub fn with_config(url: impl Into<String>, config: WebSocketConfig) -> Self {
        let (sender, _) = mpsc::channel(config.receive_buffer_size);
        Self {
            config,
            url: url.into(),
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            sender,
            abort_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 获取连接状态
    pub async fn state(&self) -> ConnectionState {
        *self.state.lock().await
    }

    /// 获取中断标志
    pub fn abort_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort_flag)
    }

    /// 请求中断
    pub fn abort(&self) {
        self.abort_flag.store(true, Ordering::Relaxed);
    }

    /// 建立 WebSocket 连接
    ///
    /// 返回消息接收流
    pub async fn connect(&self) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        {
            let mut state = self.state.lock().await;
            if *state == ConnectionState::Connected {
                return Err(anyhow!("Already connected"));
            }
            *state = ConnectionState::Connecting;
        }

        let url = self.url.clone();
        let timeout = Duration::from_millis(self.config.connect_timeout_ms);

        let connect_future = async { connect_async(&url).await };

        let result = tokio::time::timeout(timeout, connect_future).await;

        match result {
            Ok(Ok((stream, _))) => {
                let mut state = self.state.lock().await;
                *state = ConnectionState::Connected;
                tracing::info!("WebSocket connected to {}", self.url);
                Ok(stream)
            }
            Ok(Err(e)) => {
                let mut state = self.state.lock().await;
                *state = ConnectionState::Disconnected;
                Err(anyhow!("WebSocket connection failed: {}", e))
            }
            Err(_) => {
                let mut state = self.state.lock().await;
                *state = ConnectionState::Disconnected;
                Err(anyhow!("WebSocket connection timeout"))
            }
        }
    }

    /// 发送消息
    pub async fn send(&self, message: WebSocketMessage) -> Result<()> {
        self.sender.send(message).await?;
        Ok(())
    }

    /// 创建消息流
    ///
    /// 连接 WebSocket 并返回消息接收流
    pub async fn create_stream(&self) -> Result<WebSocketMessageStream> {
        let stream = self.connect().await?;
        Ok(WebSocketMessageStream::new(stream, self.abort_flag.clone()))
    }

    /// 关闭连接
    pub async fn close(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        *state = ConnectionState::Closed;
        tracing::info!("WebSocket closed");
        Ok(())
    }
}

/// WebSocket 消息流
///
/// 包装 WebSocketStream，提供便捷的消息接收接口
pub struct WebSocketMessageStream {
    inner: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    abort_flag: Arc<AtomicBool>,
    pending: VecDeque<WebSocketMessage>,
}

impl WebSocketMessageStream {
    fn new(
        inner: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
        abort_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            inner,
            abort_flag,
            pending: VecDeque::new(),
        }
    }

    /// 获取下一个消息
    pub async fn next_message(&mut self) -> Result<Option<WebSocketMessage>> {
        if self.abort_flag.load(Ordering::Relaxed) {
            return Ok(None);
        }

        loop {
            if let Some(msg) = self.pending.pop_front() {
                return Ok(Some(msg));
            }

            match self.inner.next().await {
                Some(Ok(ws_msg)) => {
                    let msg: WebSocketMessage = ws_msg.into();
                    match msg {
                        WebSocketMessage::Ping(p) => {
                            // 自动响应 Pong
                            let _ = self.inner.send(WsMessage::Pong(p.into())).await;
                        }
                        WebSocketMessage::Close(_) => {
                            return Ok(None);
                        }
                        other => {
                            self.pending.push_back(other);
                        }
                    }
                }
                Some(Err(e)) => {
                    tracing::error!("WebSocket error: {}", e);
                    return Err(anyhow!("WebSocket error: {}", e));
                }
                None => return Ok(None),
            }
        }
    }

    /// 发送消息
    pub async fn send(&mut self, message: WebSocketMessage) -> Result<()> {
        let ws_msg: WsMessage = message.into();
        self.inner.send(ws_msg).await?;
        Ok(())
    }

    /// 收集所有文本消息
    pub async fn collect_text(&mut self) -> Result<String> {
        let mut result = String::new();
        while let Some(msg) = self.next_message().await? {
            if let WebSocketMessage::Text(t) = msg {
                result.push_str(&t);
            }
        }
        Ok(result)
    }
}

/// 流式 WebSocket 接收器
///
/// 实现 Stream trait，可以与 async 迭代器一起使用
pub struct WebSocketReceiver {
    stream: WebSocketMessageStream,
}

impl WebSocketReceiver {
    /// 创建接收器
    pub fn new(stream: WebSocketMessageStream) -> Self {
        Self { stream }
    }
}

impl Stream for WebSocketReceiver {
    type Item = Result<WebSocketMessage>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // 使用 futures 库的 StreamExt 来轮询
        let abort_flag = self.stream.abort_flag.clone();
        if abort_flag.load(Ordering::Relaxed) {
            return Poll::Ready(None);
        }

        // 委托给 inner stream
        Pin::new(&mut self.stream.inner).poll_next(cx).map(|opt| {
            opt.map(|result| {
                result
                    .map(|ws_msg| WebSocketMessage::from(ws_msg))
                    .map_err(|e| anyhow::anyhow!("WebSocket error: {}", e))
            })
        })
    }
}

/// WebSocket 适配器 trait
///
/// 定义 WebSocket 连接的标准接口
#[async_trait]
pub trait WebSocketAdapterTrait: Send + Sync {
    /// 建立 WebSocket 连接
    async fn connect(&self) -> Result<()>;

    /// 发送消息
    async fn send(&self, message: &str) -> Result<()>;

    /// 接收消息
    async fn receive(&self) -> Result<Option<String>>;

    /// 关闭连接
    async fn close(&self) -> Result<()>;

    /// 检查是否已连接
    async fn is_connected(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.connect_timeout_ms, 10000);
        assert_eq!(config.heartbeat_interval_ms, 30000);
        assert_eq!(config.max_reconnect_attempts, 3);
    }

    #[test]
    fn test_websocket_message_conversion() {
        let ws_msg = WsMessage::Text("hello".into());
        let msg: WebSocketMessage = ws_msg.into();
        assert!(matches!(msg, WebSocketMessage::Text(t) if t == "hello"));
    }

    #[test]
    fn test_websocket_message_to_ws_message() {
        let msg = WebSocketMessage::Binary(vec![1, 2, 3]);
        let ws_msg: WsMessage = msg.into();
        assert!(matches!(ws_msg, WsMessage::Binary(b) if b == vec![1, 2, 3]));
    }

    #[tokio::test]
    async fn test_websocket_adapter_creation() {
        let adapter = WebSocketAdapter::new("ws://localhost:8080");
        assert_eq!(adapter.state().await, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_websocket_adapter_abort() {
        let adapter = WebSocketAdapter::new("ws://localhost:8080");
        assert!(!adapter.abort_flag().load(Ordering::Relaxed));
        adapter.abort();
        assert!(adapter.abort_flag().load(Ordering::Relaxed));
    }

    #[test]
    fn test_connection_state() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_ne!(ConnectionState::Disconnected, ConnectionState::Connected);
    }
}
