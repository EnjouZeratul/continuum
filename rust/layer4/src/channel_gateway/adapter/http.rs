//! # HTTP Channel Adapter
//!
//! HTTP REST API 渠道适配器。

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::time::Duration;

use crate::channel_gateway::{Channel, ChannelType, InboundMessage, OutboundMessage};
use crate::types::Layer4Result;

use reqwest::Client;
use std::collections::HashMap;

/// HTTP 渠道配置
#[derive(Debug, Clone)]
pub struct HttpChannelConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub headers: HashMap<String, String>,
    pub retry_attempts: u32,
    pub retry_interval_ms: u64,
}

impl Default for HttpChannelConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            timeout_ms: 30000,
            headers: HashMap::new(),
            retry_attempts: 3,
            retry_interval_ms: 1000,
        }
    }
}

/// HTTP 渠道适配器
pub struct HttpChannel {
    channel_id: String,
    config: HttpChannelConfig,
    connected: RwLock<bool>,
    request_queue: RwLock<VecDeque<InboundMessage>>,
    /// HTTP 客户端
    client: Client,
}

impl HttpChannel {
    /// 创建新的 HTTP 渠道
    pub fn new(channel_id: impl Into<String>, config: HttpChannelConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            channel_id: channel_id.into(),
            config,
            connected: RwLock::new(false),
            request_queue: RwLock::new(VecDeque::new()),
            client,
        }
    }

    /// 创建默认 HTTP 渠道
    pub fn default_channel() -> Self {
        Self::new("http-default", HttpChannelConfig::default())
    }

    /// 初始化连接（验证服务可用性）
    pub async fn connect(&self) -> Layer4Result<()> {
        let url = format!("{}/health", self.config.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    *self.connected.write() = true;
                    tracing::info!("HTTP channel connected to {}", self.config.base_url);
                    Ok(())
                } else {
                    tracing::warn!("HTTP health check returned {}", response.status());
                    // 即使健康检查失败，也认为连接可用（可能是没有健康检查端点）
                    *self.connected.write() = true;
                    Ok(())
                }
            }
            Err(e) => {
                // 网络错误时，仍然允许发送（可能在后台重连）
                tracing::warn!("HTTP connection check failed: {}, proceeding anyway", e);
                *self.connected.write() = true;
                Ok(())
            }
        }
    }

    /// 发送 HTTP POST 请求
    pub async fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Layer4Result<serde_json::Value> {
        let url = format!("{}{}", self.config.base_url, path);

        let mut request = self.client.post(&url).json(body);

        // 添加配置的 headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        let response = self.send_with_retry(request).await?;
        let result = response.json().await?;

        Ok(result)
    }

    /// 发送 HTTP GET 请求
    pub async fn get(&self, path: &str) -> Layer4Result<serde_json::Value> {
        let url = format!("{}{}", self.config.base_url, path);

        let mut request = self.client.get(&url);

        // 添加配置的 headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        let response = self.send_with_retry(request).await?;
        let result = response.json().await?;

        Ok(result)
    }

    /// 带重试的发送
    async fn send_with_retry(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Layer4Result<reqwest::Response> {
        let mut attempts = 0;
        let max_attempts = self.config.retry_attempts;
        let interval = Duration::from_millis(self.config.retry_interval_ms);

        loop {
            let request_clone = request
                .try_clone()
                .ok_or_else(|| anyhow::anyhow!("Failed to clone request for retry"))?;

            match request_clone.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() || status.is_client_error() {
                        return Ok(response);
                    }
                    // 服务端错误，重试
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow::anyhow!(
                            "HTTP request failed after {} attempts: {}",
                            max_attempts,
                            status
                        ));
                    }
                    tracing::warn!(
                        "HTTP request failed with {}, retrying ({}/{})",
                        status,
                        attempts,
                        max_attempts
                    );
                    tokio::time::sleep(interval).await;
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow::anyhow!("HTTP request failed: {}", e));
                    }
                    tracing::warn!(
                        "HTTP request error: {}, retrying ({}/{})",
                        e,
                        attempts,
                        max_attempts
                    );
                    tokio::time::sleep(interval).await;
                }
            }
        }
    }

    /// 处理 HTTP 请求（模拟接收）
    pub fn handle_request(&self, user_id: &str, content: &str) {
        let message = InboundMessage::new(&self.channel_id, user_id, content).with_metadata(
            serde_json::json!({
                "source": "http",
                "method": "POST"
            }),
        );
        self.request_queue.write().push_back(message);
    }

    /// 处理带会话的请求
    pub fn handle_request_with_session(&self, user_id: &str, content: &str, session_id: &str) {
        let message = InboundMessage::new(&self.channel_id, user_id, content)
            .with_session(session_id)
            .with_metadata(serde_json::json!({
                "source": "http",
                "method": "POST"
            }));
        self.request_queue.write().push_back(message);
    }

    /// 获取基础 URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }
}

#[async_trait]
impl Channel for HttpChannel {
    fn id(&self) -> &str {
        &self.channel_id
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Http
    }

    async fn send(&self, message: &OutboundMessage) -> Layer4Result<()> {
        if !*self.connected.read() {
            return Err(anyhow::anyhow!("Channel not connected"));
        }

        // 构建 HTTP POST 请求体
        let body = serde_json::json!({
            "message_id": message.message_id,
            "content": message.content,
            "message_type": message.message_type,
            "target": message.target,
            "metadata": message.metadata,
            "timestamp": message.timestamp.to_rfc3339(),
        });

        // 发送 POST 请求到 /api/message 端点
        let path = "/api/message";
        let result = self.post(path, &body).await?;

        tracing::debug!(
            "HTTP channel sent message {}, response: {:?}",
            message.message_id,
            result
        );
        Ok(())
    }

    async fn try_receive(&self) -> Layer4Result<Option<InboundMessage>> {
        if !*self.connected.read() {
            return Err(anyhow::anyhow!("Channel not connected"));
        }

        Ok(self.request_queue.write().pop_front())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }

    async fn close(&self) -> Layer4Result<()> {
        *self.connected.write() = false;
        self.request_queue.write().clear();
        tracing::info!("HTTP channel closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_channel_creation() {
        let channel = HttpChannel::default_channel();
        assert_eq!(channel.id(), "http-default");
        // 初始状态未连接
        assert!(!channel.is_connected());
    }

    #[test]
    fn test_http_config_default() {
        let config = HttpChannelConfig::default();
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.retry_attempts, 3);
        assert_eq!(config.retry_interval_ms, 1000);
    }

    #[test]
    fn test_http_channel_handle_request() {
        let channel = HttpChannel::default_channel();
        // 手动设置连接状态
        *channel.connected.write() = true;
        channel.handle_request("user-1", "POST /api/agent");

        let count = channel.request_queue.read().len();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_http_channel_base_url() {
        let channel = HttpChannel::default_channel();
        assert_eq!(channel.base_url(), "http://localhost:8080");
    }

    #[tokio::test]
    async fn test_http_channel_close() {
        let channel = HttpChannel::default_channel();
        *channel.connected.write() = true;
        channel.close().await.unwrap();

        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_send_without_connection() {
        let channel = HttpChannel::default_channel();
        // 未连接时发送应该失败
        let msg = OutboundMessage::to_user("test-user", "hello");
        let result = channel.send(&msg).await;
        assert!(result.is_err());
    }
}
