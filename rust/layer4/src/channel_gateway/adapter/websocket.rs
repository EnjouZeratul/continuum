//! # WebSocket Channel Adapter
//!
//! WebSocket 实时通信渠道适配器。

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

use crate::channel_gateway::{Channel, ChannelType, InboundMessage, OutboundMessage};
use crate::types::Layer4Result;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

/// WebSocket 渠道配置
#[derive(Debug, Clone)]
pub struct WebSocketChannelConfig {
    pub url: String,
    pub reconnect_attempts: u32,
    pub reconnect_interval_ms: u64,
    pub ping_interval_ms: u64,
    pub connect_timeout_ms: u64,
}

impl Default for WebSocketChannelConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8080/ws".to_string(),
            reconnect_attempts: 3,
            reconnect_interval_ms: 1000,
            ping_interval_ms: 30000,
            connect_timeout_ms: 10000,
        }
    }
}

/// WebSocket 连接类型
type WsConnection = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// WebSocket 渠道适配器
pub struct WebSocketChannel {
    channel_id: String,
    config: WebSocketChannelConfig,
    connected: RwLock<bool>,
    message_queue: RwLock<VecDeque<InboundMessage>>,
    sessions: RwLock<HashMap<String, String>>, // session_id -> user_id
    /// WebSocket 连接（用于发送）
    ws_sender: Arc<AsyncMutex<Option<futures::stream::SplitSink<WsConnection, WsMessage>>>>,
}

impl WebSocketChannel {
    /// 创建新的 WebSocket 渠道
    pub fn new(channel_id: impl Into<String>, config: WebSocketChannelConfig) -> Self {
        Self {
            channel_id: channel_id.into(),
            config,
            connected: RwLock::new(false),
            message_queue: RwLock::new(VecDeque::new()),
            sessions: RwLock::new(HashMap::new()),
            ws_sender: Arc::new(AsyncMutex::new(None)),
        }
    }

    /// 创建默认 WebSocket 渠道
    pub fn default_channel() -> Self {
        Self::new("ws-default", WebSocketChannelConfig::default())
    }

    /// 建立 WebSocket 连接
    pub async fn connect(&self) -> Layer4Result<()> {
        let url = self.config.url.clone();
        let timeout = Duration::from_millis(self.config.connect_timeout_ms);

        let connect_future = async { connect_async(&url).await };

        let result = tokio::time::timeout(timeout, connect_future).await;

        match result {
            Ok(Ok((stream, _))) => {
                // 分离读写
                let (sink, _stream) = stream.split();
                *self.ws_sender.lock().await = Some(sink);
                *self.connected.write() = true;
                tracing::info!("WebSocket connected to {}", url);
                Ok(())
            }
            Ok(Err(e)) => {
                tracing::error!("WebSocket connection failed: {}", e);
                Err(anyhow::anyhow!("WebSocket connection failed: {}", e))
            }
            Err(_) => {
                tracing::error!("WebSocket connection timeout");
                Err(anyhow::anyhow!("WebSocket connection timeout"))
            }
        }
    }

    /// 带重连的连接
    pub async fn connect_with_retry(&self) -> Layer4Result<()> {
        let mut attempts = 0;
        let max_attempts = self.config.reconnect_attempts;
        let interval = Duration::from_millis(self.config.reconnect_interval_ms);

        loop {
            match self.connect().await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                    tracing::warn!(
                        "WebSocket connection attempt {}/{} failed, retrying...",
                        attempts,
                        max_attempts
                    );
                    tokio::time::sleep(interval).await;
                }
            }
        }
    }

    /// 发送原始 WebSocket 消息
    pub async fn send_raw(&self, message: WsMessage) -> Layer4Result<()> {
        let mut sender = self.ws_sender.lock().await;
        if let Some(ref mut sink) = *sender {
            sink.send(message).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("WebSocket not connected"))
        }
    }

    /// 发送文本消息
    pub async fn send_text(&self, text: &str) -> Layer4Result<()> {
        self.send_raw(WsMessage::Text(text.into())).await
    }

    /// 发送二进制消息
    pub async fn send_binary(&self, data: Vec<u8>) -> Layer4Result<()> {
        self.send_raw(WsMessage::Binary(data.into())).await
    }

    /// 注册会话
    pub fn register_session(&self, session_id: &str, user_id: &str) {
        self.sessions
            .write()
            .insert(session_id.to_string(), user_id.to_string());
    }

    /// 注销会话
    pub fn unregister_session(&self, session_id: &str) {
        self.sessions.write().remove(session_id);
    }

    /// 接收 WebSocket 消息（模拟）
    pub fn receive_message(&self, session_id: &str, content: &str) {
        let user_id = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let message = InboundMessage::new(&self.channel_id, &user_id, content)
            .with_session(session_id)
            .with_metadata(serde_json::json!({
                "source": "websocket",
                "session_id": session_id
            }));
        self.message_queue.write().push_back(message);
    }

    /// 获取活跃会话数量
    pub fn active_sessions(&self) -> usize {
        self.sessions.read().len()
    }
}

#[async_trait]
impl Channel for WebSocketChannel {
    fn id(&self) -> &str {
        &self.channel_id
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::WebSocket
    }

    async fn send(&self, message: &OutboundMessage) -> Layer4Result<()> {
        if !*self.connected.read() {
            return Err(anyhow::anyhow!("Channel not connected"));
        }

        // 序列化消息为 JSON
        let payload = serde_json::json!({
            "message_id": message.message_id,
            "content": message.content,
            "message_type": message.message_type,
            "target": message.target,
            "metadata": message.metadata,
            "timestamp": message.timestamp.to_rfc3339(),
        });

        // 发送 WebSocket 文本消息
        self.send_text(&payload.to_string()).await?;

        tracing::debug!("WebSocket channel sent message {}", message.message_id);
        Ok(())
    }

    async fn try_receive(&self) -> Layer4Result<Option<InboundMessage>> {
        if !*self.connected.read() {
            return Err(anyhow::anyhow!("Channel not connected"));
        }

        Ok(self.message_queue.write().pop_front())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }

    async fn close(&self) -> Layer4Result<()> {
        // 发送 Close 帧
        let mut sender = self.ws_sender.lock().await;
        if let Some(ref mut sink) = *sender {
            sink.close().await?;
        }
        *sender = None;

        *self.connected.write() = false;
        self.message_queue.write().clear();
        self.sessions.write().clear();
        tracing::info!("WebSocket channel closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_channel_creation() {
        let channel = WebSocketChannel::default_channel();
        assert_eq!(channel.id(), "ws-default");
        // 初始状态未连接
        assert!(!channel.is_connected());
    }

    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketChannelConfig::default();
        assert_eq!(config.reconnect_attempts, 3);
        assert_eq!(config.ping_interval_ms, 30000);
        assert_eq!(config.connect_timeout_ms, 10000);
    }

    #[test]
    fn test_websocket_session_management() {
        let channel = WebSocketChannel::default_channel();
        channel.register_session("session-1", "user-1");

        assert_eq!(channel.active_sessions(), 1);

        channel.unregister_session("session-1");
        assert_eq!(channel.active_sessions(), 0);
    }

    #[test]
    fn test_websocket_receive_message() {
        let channel = WebSocketChannel::default_channel();
        // 手动设置连接状态以便测试消息接收
        *channel.connected.write() = true;
        channel.register_session("session-1", "user-1");
        channel.receive_message("session-1", "Hello");

        let count = channel.message_queue.read().len();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_websocket_channel_close() {
        let channel = WebSocketChannel::default_channel();
        // 手动设置连接状态
        *channel.connected.write() = true;
        channel.register_session("session-1", "user-1");
        channel.close().await.unwrap();

        assert!(!channel.is_connected());
        assert_eq!(channel.active_sessions(), 0);
    }

    #[tokio::test]
    async fn test_send_without_connection() {
        let channel = WebSocketChannel::default_channel();
        // 未连接时发送应该失败
        let msg = OutboundMessage::to_user("test-user", "hello");
        let result = channel.send(&msg).await;
        assert!(result.is_err());
    }
}
