//! MCP 消息处理器
//!
//! 处理各类 MCP 消息。

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use super::protocol::{
    error_codes, Implementation, InitializeParams, InitializeResult, McpErrorData, McpNotification,
    McpRequest, McpResponse, RequestId, ServerCapabilities, ToolDefinition, ToolResult,
    MCP_VERSION,
};
use anyhow::{anyhow, Result};

/// MCP 消息处理器 trait
#[async_trait]
pub trait McpHandler: Send + Sync {
    /// 处理请求
    async fn handle(&self, request: &McpRequest) -> Result<McpResponse>;

    /// 处理通知
    async fn handle_notification(&self, notification: &McpNotification) -> Result<()>;
}

/// 默认处理器实现
pub struct DefaultHandler {
    /// 服务端信息
    server_info: Implementation,
    /// 已注册的工具
    tools: Arc<parking_lot::RwLock<HashMap<String, ToolDefinition>>>,
    /// 工具执行器
    tool_executors: Arc<parking_lot::RwLock<HashMap<String, Arc<dyn ToolExecutor>>>>,
}

/// 工具执行器 trait
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// 执行工具
    async fn execute(&self, name: &str, arguments: Value) -> Result<ToolResult>;
}

impl DefaultHandler {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            server_info: Implementation {
                name: name.to_string(),
                version: version.to_string(),
            },
            tools: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            tool_executors: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// 注册工具
    pub fn register_tool(&self, tool: ToolDefinition, executor: Arc<dyn ToolExecutor>) {
        let name = tool.name.clone();
        self.tools.write().insert(name.clone(), tool);
        self.tool_executors.write().insert(name.clone(), executor);
    }

    /// 处理初始化请求
    fn handle_initialize(&self, _params: &InitializeParams) -> Result<McpResponse> {
        let result = InitializeResult {
            protocol_version: MCP_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: Some(Default::default()),
                resources: Some(Default::default()),
                prompts: Some(Default::default()),
                ..Default::default()
            },
            server_info: self.server_info.clone(),
            instructions: Some("Continuum MCP Server".to_string()),
        };

        Ok(McpResponse {
            id: RequestId::Number(0),
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    /// 处理列出工具请求
    fn handle_list_tools(&self, id: &RequestId) -> Result<McpResponse> {
        let tools: Vec<ToolDefinition> = self.tools.read().values().cloned().collect();
        Ok(McpResponse {
            id: id.clone(),
            result: Some(serde_json::json!({ "tools": tools })),
            error: None,
        })
    }

    /// 处理调用工具请求
    async fn handle_call_tool(
        &self,
        id: &RequestId,
        params: Option<&Value>,
    ) -> Result<McpResponse> {
        let params = params.ok_or_else(|| anyhow!("Missing params"))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing tool name"))?;

        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

        let executor = {
            let executors = self.tool_executors.read();
            executors
                .get(name)
                .ok_or_else(|| anyhow!("Tool not found: {}", name))?
                .clone()
        };

        match executor.execute(name, arguments).await {
            Ok(result) => Ok(McpResponse {
                id: id.clone(),
                result: Some(serde_json::to_value(result)?),
                error: None,
            }),
            Err(e) => Ok(McpResponse {
                id: id.clone(),
                result: None,
                error: Some(McpErrorData {
                    code: error_codes::INTERNAL_ERROR,
                    message: e.to_string(),
                    data: None,
                }),
            }),
        }
    }
}

#[async_trait]
impl McpHandler for DefaultHandler {
    async fn handle(&self, request: &McpRequest) -> Result<McpResponse> {
        match request.method.as_str() {
            "initialize" => {
                let params = request
                    .params
                    .as_ref()
                    .map(|p| serde_json::from_value(p.clone()))
                    .transpose()?;

                if let Some(params) = params {
                    self.handle_initialize(&params)
                } else {
                    Ok(McpResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpErrorData {
                            code: error_codes::INVALID_PARAMS,
                            message: "Missing initialize params".to_string(),
                            data: None,
                        }),
                    })
                }
            }
            "tools/list" => self.handle_list_tools(&request.id),
            "tools/call" => {
                self.handle_call_tool(&request.id, request.params.as_ref())
                    .await
            }
            "shutdown" => Ok(McpResponse {
                id: request.id.clone(),
                result: Some(Value::Null),
                error: None,
            }),
            _ => Ok(McpResponse {
                id: request.id.clone(),
                result: None,
                error: Some(McpErrorData {
                    code: error_codes::METHOD_NOT_FOUND,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            }),
        }
    }

    async fn handle_notification(&self, _notification: &McpNotification) -> Result<()> {
        // Notifications are handled asynchronously and don't require responses
        Ok(())
    }
}

/// 简单工具执行器
///
/// 将闭包包装为 `ToolExecutor` trait 实现。
/// 适用于不需要异步执行的简单工具。
pub struct SimpleToolExecutor<F>(pub F)
where
    F: Fn(&str, Value) -> Result<ToolResult> + Send + Sync;

#[async_trait]
impl<F> ToolExecutor for SimpleToolExecutor<F>
where
    F: Fn(&str, Value) -> Result<ToolResult> + Send + Sync,
{
    async fn execute(&self, name: &str, arguments: Value) -> Result<ToolResult> {
        (self.0)(name, arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_bridge::protocol::ContentBlock;

    #[tokio::test]
    async fn test_handle_initialize() {
        let handler = DefaultHandler::new("test-server", "1.0.0");
        let request = McpRequest {
            id: RequestId::Number(1),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocol_version": "2024-11-05",
                "capabilities": {},
                "client_info": { "name": "test-client", "version": "1.0.0" }
            })),
        };

        let response = handler.handle(&request).await.unwrap();
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_list_tools() {
        let handler = DefaultHandler::new("test-server", "1.0.0");
        handler.register_tool(
            ToolDefinition {
                name: "test_tool".to_string(),
                description: Some("A test tool".to_string()),
                input_schema: None,
            },
            Arc::new(SimpleToolExecutor(|_name, _args| {
                Ok(ToolResult {
                    is_error: false,
                    content: vec![ContentBlock::Text {
                        text: "OK".to_string(),
                    }],
                })
            })),
        );

        let request = McpRequest {
            id: RequestId::Number(2),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(&request).await.unwrap();
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let handler = DefaultHandler::new("test-server", "1.0.0");
        let request = McpRequest {
            id: RequestId::Number(3),
            method: "unknown_method".to_string(),
            params: None,
        };

        let response = handler.handle(&request).await.unwrap();
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }
}
