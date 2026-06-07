//! MCP 桥接器
//!
//! MCP (Model Context Protocol) 协议的主要实现。
//!
//! # 功能
//!
//! - MCP 协议消息处理与路由
//! - 工具注册与发现
//! - 多种传输层支持 (stdio, tcp, unix socket)
//! - 请求/响应生命周期管理
//!
//! # 用法示例
//!
//! ```rust,ignore
//! use sh_layer4::mcp_bridge::{McpBridge, McpBridgeConfig, ToolDefinition, ToolResult, ContentBlock};
//!
//! // 创建桥接器
//! let config = McpBridgeConfig {
//!     server_name: "my-server".to_string(),
//!     server_version: "1.0.0".to_string(),
//!     request_timeout_ms: 30000,
//!     max_concurrent_requests: 100,
//! };
//! let bridge = McpBridge::new(config);
//!
//! // 注册工具
//! bridge.register_simple_tool("echo", "Echo input text", |_name, args| {
//!     Ok(ToolResult {
//!         is_error: false,
//!         content: vec![ContentBlock::Text { text: args.to_string() }],
//!     })
//! });
//!
//! // 启动服务
//! bridge.start().await?;
//! ```

use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::handler::{DefaultHandler, McpHandler, ToolExecutor};
use super::protocol::{
    McpMessage, McpNotification, McpRequest, McpResponse, RequestId, ToolDefinition, ToolResult,
};
use super::transport::McpTransport;
use anyhow::{anyhow, Result};

/// MCP 桥接器配置
///
/// 配置 MCP 服务端的基本参数。
#[derive(Debug, Clone)]
pub struct McpBridgeConfig {
    /// 服务端名称，用于客户端识别
    pub server_name: String,
    /// 服务端版本号
    pub server_version: String,
    /// 请求超时时间 (毫秒)
    pub request_timeout_ms: u64,
    /// 最大并发请求数
    pub max_concurrent_requests: usize,
}

impl Default for McpBridgeConfig {
    fn default() -> Self {
        Self {
            server_name: "Continuum".to_string(),
            server_version: "0.1.0".to_string(),
            request_timeout_ms: 30000,
            max_concurrent_requests: 100,
        }
    }
}

/// MCP 桥接器
///
/// MCP 协议的核心实现，负责消息处理、工具注册和传输层管理。
///
/// # 线程安全
///
/// 所有内部状态都通过 `RwLock` 或原子类型保护，支持多线程并发访问。
pub struct McpBridge {
    /// 传输层实例
    transport: RwLock<Option<Arc<dyn McpTransport>>>,
    /// 消息处理器
    handler: Arc<DefaultHandler>,
    /// 配置
    config: McpBridgeConfig,
    /// 请求 ID 计数器 (原子递增)
    request_id_counter: AtomicU64,
    /// 待处理响应映射表
    pending_responses: Arc<RwLock<HashMap<RequestId, mpsc::Sender<McpResponse>>>>,
    /// 运行状态
    running: Arc<AtomicBool>,
}

impl McpBridge {
    /// 创建新的 MCP 桥接器
    ///
    /// # 参数
    ///
    /// - `config`: 桥接器配置
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let config = McpBridgeConfig::default();
    /// let bridge = McpBridge::new(config);
    /// ```
    pub fn new(config: McpBridgeConfig) -> Self {
        let handler = DefaultHandler::new(&config.server_name, &config.server_version);
        Self {
            transport: RwLock::new(None),
            handler: Arc::new(handler),
            config,
            request_id_counter: AtomicU64::new(0),
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 设置传输层 (Builder 模式)
    ///
    /// # 参数
    ///
    /// - `transport`: 传输层实现 (StdioTransport, TcpTransport 等)
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let bridge = McpBridge::new(config)
    ///     .with_transport(Box::new(StdioTransport::new()));
    /// ```
    pub fn with_transport(self, transport: Box<dyn McpTransport>) -> Self {
        *self.transport.write() = Some(Arc::from(transport));
        self
    }

    /// 注册工具及其执行器
    ///
    /// # 参数
    ///
    /// - `tool`: 工具定义
    /// - `executor`: 工具执行器实现
    pub fn register_tool(&self, tool: ToolDefinition, executor: Arc<dyn ToolExecutor>) {
        self.handler.register_tool(tool, executor);
    }

    /// 注册简单工具 (便捷方法)
    ///
    /// 适用于不需要复杂执行器逻辑的工具。
    ///
    /// # 参数
    ///
    /// - `name`: 工具名称
    /// - `description`: 工具描述
    /// - `executor`: 执行函数，接收工具名和参数，返回执行结果
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// bridge.register_simple_tool("echo", "Echo input", |_name, args| {
    ///     Ok(ToolResult {
    ///         is_error: false,
    ///         content: vec![ContentBlock::Text { text: args.to_string() }],
    ///     })
    /// });
    /// ```
    pub fn register_simple_tool<F>(&self, name: &str, description: &str, executor: F)
    where
        F: Fn(&str, Value) -> Result<ToolResult> + Send + Sync + 'static,
    {
        let tool = ToolDefinition {
            name: name.to_string(),
            description: Some(description.to_string()),
            input_schema: None,
        };
        self.register_tool(tool, Arc::new(super::handler::SimpleToolExecutor(executor)));
    }

    /// 生成下一个请求 ID (内部方法)
    fn next_request_id(&self) -> RequestId {
        RequestId::Number(self.request_id_counter.fetch_add(1, Ordering::SeqCst) as i64)
    }

    /// 启动桥接器
    ///
    /// 初始化消息处理循环，开始接收和处理 MCP 消息。
    ///
    /// # 错误
    ///
    /// 如果传输层未初始化或启动失败，返回错误。
    pub async fn start(&self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        // Clone transport Arc while holding lock
        let transport_opt = {
            let transport_guard = self.transport.read();
            transport_guard.clone()
        };

        // 启动消息处理循环
        let handler = self.handler.clone();
        let pending = self.pending_responses.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            info!("MCP message loop started");

            if transport_opt.is_none() {
                info!("No transport configured, message loop will idle");
            }

            loop {
                // Check if we should stop
                if !running.load(Ordering::SeqCst) {
                    info!("MCP message loop stopping");
                    break;
                }

                // Read message from transport
                if let Some(ref t) = transport_opt {
                    match t.receive().await {
                        Ok(Some(message)) => {
                            // Process received message
                            match message {
                                McpMessage::Request(request) => {
                                    // Handle incoming request
                                    match handler.handle(&request).await {
                                        Ok(response) => {
                                            // Send response back
                                            if let Err(e) =
                                                t.send(&McpMessage::Response(response)).await
                                            {
                                                warn!("Failed to send response: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Handler error for request {:?}: {}",
                                                request.id, e
                                            );
                                        }
                                    }
                                }
                                McpMessage::Notification(notification) => {
                                    // Handle notification (no response needed)
                                    if let Err(e) = handler.handle_notification(&notification).await
                                    {
                                        warn!("Notification handler error: {}", e);
                                    }
                                }
                                McpMessage::Response(response) => {
                                    // Response to our request - find matching pending request
                                    // Release lock before await to satisfy Send requirement
                                    let sender_opt = pending.write().remove(&response.id);
                                    if let Some(sender) = sender_opt {
                                        if let Err(e) = sender.send(response).await {
                                            warn!(
                                                "Failed to forward response to pending request: {}",
                                                e
                                            );
                                        }
                                    } else {
                                        debug!(
                                            "Received response for unknown request {:?}",
                                            response.id
                                        );
                                    }
                                }
                                McpMessage::Error(error) => {
                                    warn!("Received error: {:?}", error);
                                }
                            }
                        }
                        Ok(None) => {
                            // No message available, brief sleep
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                        Err(e) => {
                            warn!("Transport receive error: {}", e);
                            // Brief pause before retry to avoid tight loop on error
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                } else {
                    // No transport configured
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            // Clean up pending responses when stopping
            pending.write().clear();
            info!("MCP message loop stopped");
        });

        Ok(())
    }

    /// 停止桥接器
    ///
    /// 关闭消息处理循环并释放传输层资源。
    ///
    /// # 错误
    ///
    /// 如果传输层关闭失败，返回错误。
    pub async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        let transport = self.transport.write().take();
        if let Some(transport) = transport {
            transport.close().await?;
        }

        Ok(())
    }

    /// 发送请求并等待响应
    ///
    /// 向 MCP 服务端发送请求消息，并等待对应的响应。
    ///
    /// # 参数
    ///
    /// - `method`: MCP 方法名 (如 "tools/list", "tools/call")
    /// - `params`: 可选的请求参数
    ///
    /// # 返回
    ///
    /// 返回对应的响应消息。
    ///
    /// # 错误
    ///
    /// - 如果传输层未初始化，返回 `Transport not initialized` 错误
    /// - 如果请求超时，返回超时错误
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<McpResponse> {
        let id = self.next_request_id();
        let timeout_duration = std::time::Duration::from_millis(self.config.request_timeout_ms);

        // 创建响应接收通道
        let (tx, mut rx) = mpsc::channel::<McpResponse>(1);

        // 注册待处理请求
        {
            self.pending_responses.write().insert(id.clone(), tx);
        }

        let request = McpRequest {
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let message = McpMessage::Request(request);

        // 发送请求
        {
            let transport_guard = self.transport.read();
            let transport = transport_guard
                .as_ref()
                .ok_or_else(|| anyhow!("Transport not initialized"))?;
            transport.send(&message).await?;
        }

        // 等待响应，带超时
        let result = tokio::time::timeout(timeout_duration, rx.recv()).await;

        // 清理待处理请求（无论成功还是失败）
        self.pending_responses.write().remove(&id);

        match result {
            Ok(Some(response)) => Ok(response),
            Ok(None) => Err(anyhow!("Response channel closed")),
            Err(_) => Err(anyhow!(
                "Request timeout after {}ms",
                self.config.request_timeout_ms
            )),
        }
    }

    /// 发送通知消息
    ///
    /// 向 MCP 服务端发送通知消息，不等待响应。
    ///
    /// # 参数
    ///
    /// - `method`: 通知方法名 (如 "notifications/initialized")
    /// - `params`: 可选的通知参数
    ///
    /// # 错误
    ///
    /// 如果传输层未初始化，返回 `Transport not initialized` 错误。
    #[allow(clippy::await_holding_lock)]
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = McpNotification {
            method: method.to_string(),
            params,
        };

        let message = McpMessage::Notification(notification);

        let transport_guard = self.transport.read();
        let transport = transport_guard
            .as_ref()
            .ok_or_else(|| anyhow!("Transport not initialized"))?;
        transport.send(&message).await?;

        Ok(())
    }

    /// 列出可用工具
    ///
    /// 请求 MCP 服务端列出所有可用工具。
    ///
    /// # 返回
    ///
    /// 返回工具定义列表。
    ///
    /// # 错误
    ///
    /// 如果请求失败或响应解析失败，返回错误。
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>> {
        let response = self.request("tools/list", None).await?;

        if let Some(result) = response.result {
            let tools: Vec<ToolDefinition> = serde_json::from_value(
                result.get("tools").cloned().unwrap_or(Value::Array(vec![])),
            )?;
            Ok(tools)
        } else {
            Ok(vec![])
        }
    }

    /// 调用工具
    ///
    /// 调用 MCP 服务端上的指定工具。
    ///
    /// # 参数
    ///
    /// - `name`: 工具名称
    /// - `arguments`: 工具参数 (JSON 格式)
    ///
    /// # 返回
    ///
    /// 返回工具执行结果。
    ///
    /// # 错误
    ///
    /// - 如果工具不存在或执行失败，返回错误
    /// - 如果响应解析失败，返回错误
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolResult> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });

        let response = self.request("tools/call", Some(params)).await?;

        if let Some(result) = response.result {
            let tool_result: ToolResult = serde_json::from_value(result)?;
            Ok(tool_result)
        } else if let Some(error) = response.error {
            Err(anyhow!("Tool call error: {}", error.message))
        } else {
            Err(anyhow!("Unknown error"))
        }
    }

    /// 初始化 MCP 连接
    ///
    /// 与 MCP 服务端进行握手，交换协议版本和能力信息。
    ///
    /// # 参数
    ///
    /// - `client_info`: 客户端名称
    /// - `version`: 客户端版本号
    ///
    /// # 流程
    ///
    /// 1. 发送 `initialize` 请求
    /// 2. 接收服务端响应
    /// 3. 发送 `notifications/initialized` 通知
    ///
    /// # 错误
    ///
    /// 如果初始化请求失败，返回 `Initialize failed` 错误。
    pub async fn initialize(&self, client_info: &str, version: &str) -> Result<()> {
        let params = serde_json::json!({
            "protocol_version": "2024-11-05",
            "capabilities": {},
            "client_info": {
                "name": client_info,
                "version": version
            }
        });

        let response = self.request("initialize", Some(params)).await?;

        if response.error.is_some() {
            return Err(anyhow!("Initialize failed"));
        }

        // 发送 initialized 通知
        self.notify("notifications/initialized", None).await?;

        Ok(())
    }

    /// 检查桥接器是否正在运行
    ///
    /// # 返回
    ///
    /// 如果桥接器已启动且正在处理消息，返回 `true`；否则返回 `false`。
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_bridge::protocol::ContentBlock;

    #[tokio::test]
    async fn test_bridge_creation() {
        let config = McpBridgeConfig::default();
        let bridge = McpBridge::new(config);

        assert!(!bridge.is_running());
    }

    #[tokio::test]
    async fn test_register_tool() {
        let bridge = McpBridge::new(McpBridgeConfig::default());

        bridge.register_simple_tool("test_tool", "A test tool", |_name, _args| {
            Ok(ToolResult {
                is_error: false,
                content: vec![ContentBlock::Text {
                    text: "OK".to_string(),
                }],
            })
        });

        // 工具已注册
    }

    #[tokio::test]
    async fn test_next_request_id() {
        let bridge = McpBridge::new(McpBridgeConfig::default());

        let id1 = bridge.next_request_id();
        let id2 = bridge.next_request_id();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_config_default() {
        let config = McpBridgeConfig::default();
        assert_eq!(config.server_name, "Continuum");
        assert_eq!(config.request_timeout_ms, 30000);
    }
}
