//! # Channel Gateway
//!
//! 多渠道消息网关，支持 CLI、HTTP、WebSocket 等多种渠道接入。

pub mod adapter;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sh_layer3::generate_short_id;
use std::collections::HashMap;
use std::sync::Arc;

use crate::types::Layer4Result;

/// 渠道接口
#[async_trait]
pub trait Channel: Send + Sync {
    /// 获取渠道 ID
    fn id(&self) -> &str;

    /// 获取渠道类型
    fn channel_type(&self) -> ChannelType;

    /// 发送消息
    async fn send(&self, message: &OutboundMessage) -> Layer4Result<()>;

    /// 接收消息（非阻塞）
    async fn try_receive(&self) -> Layer4Result<Option<InboundMessage>>;

    /// 检查是否连接
    fn is_connected(&self) -> bool;

    /// 关闭渠道
    async fn close(&self) -> Layer4Result<()>;
}

/// 渠道类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelType {
    Cli,
    Http,
    WebSocket,
    Mqtt,
    Custom,
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cli => write!(f, "cli"),
            Self::Http => write!(f, "http"),
            Self::WebSocket => write!(f, "websocket"),
            Self::Mqtt => write!(f, "mqtt"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// 入站消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// 消息 ID
    pub message_id: String,
    /// 渠道 ID
    pub channel_id: String,
    /// 用户 ID
    pub user_id: String,
    /// 会话 ID（可选）
    pub session_id: Option<String>,
    /// 消息内容
    pub content: String,
    /// 消息类型
    pub message_type: MessageType,
    /// 元数据
    pub metadata: serde_json::Value,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl InboundMessage {
    pub fn new(
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            message_id: generate_short_id(),
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            session_id: None,
            content: content.into(),
            message_type: MessageType::Text,
            metadata: serde_json::Value::Null,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// 出站消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// 消息 ID
    pub message_id: String,
    /// 消息内容
    pub content: String,
    /// 消息类型
    pub message_type: MessageType,
    /// 目标
    pub target: MessageTarget,
    /// 元数据
    pub metadata: serde_json::Value,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl OutboundMessage {
    pub fn new(content: impl Into<String>, target: MessageTarget) -> Self {
        Self {
            message_id: generate_short_id(),
            content: content.into(),
            message_type: MessageType::Text,
            target,
            metadata: serde_json::Value::Null,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn broadcast(content: impl Into<String>) -> Self {
        Self::new(content, MessageTarget::All)
    }

    pub fn to_channel(channel_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(content, MessageTarget::Channel(channel_id.into()))
    }

    pub fn to_user(user_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(content, MessageTarget::User(user_id.into()))
    }
}

/// 消息目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageTarget {
    /// 广播到所有渠道
    All,
    /// 发送到指定渠道
    Channel(String),
    /// 发送到指定用户
    User(String),
    /// 发送到指定会话
    Session(String),
}

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    Json,
    Binary,
    Command,
    Event,
    Error,
}

/// 渠道网关
pub struct ChannelGateway {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
    router: MessageRouter,
    message_queue: RwLock<Vec<InboundMessage>>,
}

impl ChannelGateway {
    /// 创建新的渠道网关
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            router: MessageRouter::new(),
            message_queue: RwLock::new(Vec::new()),
        }
    }

    /// 注册渠道
    pub async fn register_channel(&self, channel: Box<dyn Channel>) -> Layer4Result<()> {
        let id = channel.id().to_string();
        let channel_type = channel.channel_type();

        self.channels.write().insert(id.clone(), channel);
        self.router.register_channel(&id, channel_type);

        tracing::info!("Registered channel: {} ({})", id, channel_type);
        Ok(())
    }

    /// 注销渠道
    pub async fn unregister_channel(&self, channel_id: &str) -> Layer4Result<bool> {
        let channel = self.channels.write().remove(channel_id);
        if let Some(channel) = channel {
            channel.close().await?;
            self.router.unregister_channel(channel_id);
            tracing::info!("Unregistered channel: {}", channel_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取渠道
    pub fn get_channel(&self, _channel_id: &str) -> Option<Arc<dyn Channel>> {
        // 由于 Box<dyn Channel> 不能直接克隆，我们返回 Option
        // 实际使用时需要重新设计
        None
    }

    /// 列出所有渠道
    pub fn list_channels(&self) -> Vec<(String, ChannelType)> {
        self.channels
            .read()
            .iter()
            .map(|(id, ch)| (id.clone(), ch.channel_type()))
            .collect()
    }

    /// 广播消息到所有渠道
    #[allow(clippy::await_holding_lock)]
    pub async fn broadcast(&self, message: &OutboundMessage) -> Layer4Result<()> {
        let channels = self.channels.read();
        for (id, channel) in channels.iter() {
            if let Err(e) = channel.send(message).await {
                tracing::error!("Failed to send to channel {}: {}", id, e);
            }
        }
        Ok(())
    }

    /// 发送消息到指定目标
    #[allow(clippy::await_holding_lock)]
    pub async fn send_to(
        &self,
        target: &MessageTarget,
        message: &OutboundMessage,
    ) -> Layer4Result<()> {
        match target {
            MessageTarget::All => self.broadcast(message).await,
            MessageTarget::Channel(channel_id) => {
                let channels = self.channels.read();
                if let Some(channel) = channels.get(channel_id) {
                    channel.send(message).await?;
                }
                Ok(())
            }
            MessageTarget::User(user_id) => {
                // 路由到用户所在的渠道
                let channel_id = self.router.find_user_channel(user_id);
                if let Some(cid) = channel_id {
                    let channels = self.channels.read();
                    if let Some(channel) = channels.get(&cid) {
                        channel.send(message).await?;
                    }
                }
                Ok(())
            }
            MessageTarget::Session(session_id) => {
                let channel_id = self.router.find_session_channel(session_id);
                if let Some(cid) = channel_id {
                    let channels = self.channels.read();
                    if let Some(channel) = channels.get(&cid) {
                        channel.send(message).await?;
                    }
                }
                Ok(())
            }
        }
    }

    /// 接收消息（轮询所有渠道）
    #[allow(clippy::await_holding_lock)]
    pub async fn receive(&self) -> Layer4Result<Option<InboundMessage>> {
        // 先检查队列
        if let Some(msg) = self.message_queue.write().pop() {
            return Ok(Some(msg));
        }

        // 轮询所有渠道
        let channels = self.channels.read();
        for (_, channel) in channels.iter() {
            if let Some(msg) = channel.try_receive().await? {
                // 更新路由信息
                self.router
                    .update_user_channel(&msg.user_id, &msg.channel_id);
                if let Some(ref session_id) = msg.session_id {
                    self.router
                        .update_session_channel(session_id, &msg.channel_id);
                }
                return Ok(Some(msg));
            }
        }

        Ok(None)
    }

    /// 接收所有待处理消息
    #[allow(clippy::await_holding_lock)]
    pub async fn receive_all(&self) -> Layer4Result<Vec<InboundMessage>> {
        let mut messages = Vec::new();

        // 先处理队列
        messages.append(&mut self.message_queue.write());

        // 轮询所有渠道
        let channels = self.channels.read();
        for (_, channel) in channels.iter() {
            while let Some(msg) = channel.try_receive().await? {
                messages.push(msg);
            }
        }

        Ok(messages)
    }

    /// 渠道数量
    pub fn channel_count(&self) -> usize {
        self.channels.read().len()
    }

    /// 关闭所有渠道
    #[allow(clippy::await_holding_lock)]
    pub async fn close_all(&self) -> Layer4Result<()> {
        let mut channels = self.channels.write();
        for (id, channel) in channels.drain() {
            if let Err(e) = channel.close().await {
                tracing::error!("Failed to close channel {}: {}", id, e);
            }
        }
        Ok(())
    }
}

impl Default for ChannelGateway {
    fn default() -> Self {
        Self::new()
    }
}

/// 消息路由器
pub struct MessageRouter {
    user_channels: RwLock<HashMap<String, String>>,
    session_channels: RwLock<HashMap<String, String>>,
    channel_registry: RwLock<HashMap<String, ChannelType>>,
}

impl MessageRouter {
    pub fn new() -> Self {
        Self {
            user_channels: RwLock::new(HashMap::new()),
            session_channels: RwLock::new(HashMap::new()),
            channel_registry: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_channel(&self, channel_id: &str, channel_type: ChannelType) {
        self.channel_registry
            .write()
            .insert(channel_id.to_string(), channel_type);
    }

    pub fn unregister_channel(&self, channel_id: &str) {
        self.channel_registry.write().remove(channel_id);

        // 清理用户和会话映射
        self.user_channels.write().retain(|_, v| v != channel_id);
        self.session_channels.write().retain(|_, v| v != channel_id);
    }

    pub fn update_user_channel(&self, user_id: &str, channel_id: &str) {
        self.user_channels
            .write()
            .insert(user_id.to_string(), channel_id.to_string());
    }

    pub fn update_session_channel(&self, session_id: &str, channel_id: &str) {
        self.session_channels
            .write()
            .insert(session_id.to_string(), channel_id.to_string());
    }

    pub fn find_user_channel(&self, user_id: &str) -> Option<String> {
        self.user_channels.read().get(user_id).cloned()
    }

    pub fn find_session_channel(&self, session_id: &str) -> Option<String> {
        self.session_channels.read().get(session_id).cloned()
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message_creation() {
        let msg = InboundMessage::new("cli-1", "user-1", "Hello");
        assert_eq!(msg.channel_id, "cli-1");
        assert_eq!(msg.user_id, "user-1");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_outbound_message_broadcast() {
        let msg = OutboundMessage::broadcast("Hello all");
        assert!(matches!(msg.target, MessageTarget::All));
    }

    #[test]
    fn test_channel_gateway_creation() {
        let gateway = ChannelGateway::new();
        assert_eq!(gateway.channel_count(), 0);
    }

    #[test]
    fn test_message_router() {
        let router = MessageRouter::new();
        router.update_user_channel("user-1", "cli-1");

        let channel = router.find_user_channel("user-1");
        assert_eq!(channel, Some("cli-1".to_string()));
    }
}
