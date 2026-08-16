//! # Default Tool Executor Implementation
//!
//! 工具执行器的默认实现。

use crate::builtin_tools::{BuiltinTool, BuiltinToolRegistry};
use crate::tool_executor::{ToolExecutor, ToolValidator};
use crate::types::{Layer3Error, Layer3Result, ToolMeta, ToolRequest, ToolResponse};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

/// 默认工具执行器
pub struct DefaultToolExecutor {
    /// 内置工具注册表
    builtin: BuiltinToolRegistry,
    /// 执行历史（用于调试）
    history: Arc<RwLock<VecDeque<ExecutionRecord>>>,
    /// 最大历史记录数
    max_history: usize,
}

/// 执行记录
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub request: ToolRequest,
    pub response: ToolResponse,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
}

impl DefaultToolExecutor {
    pub fn new() -> Self {
        Self {
            builtin: BuiltinToolRegistry::with_defaults(),
            history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            max_history: 1000,
        }
    }

    /// 注册内置工具
    pub fn register_tool(&mut self, tool: Box<dyn BuiltinTool>) {
        self.builtin.register(tool);
    }

    /// 获取执行历史
    pub fn history(&self) -> Vec<ExecutionRecord> {
        self.history.read().iter().cloned().collect()
    }

    /// 清空历史
    pub fn clear_history(&self) {
        self.history.write().clear();
    }

    /// 记录执行
    fn record(&self, request: &ToolRequest, response: &ToolResponse, duration_ms: u64) {
        let mut history = self.history.write();
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(ExecutionRecord {
            request: request.clone(),
            response: response.clone(),
            timestamp: chrono::Utc::now(),
            duration_ms,
        });
    }
}

impl Default for DefaultToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolExecutor for DefaultToolExecutor {
    async fn execute(&self, request: ToolRequest) -> Layer3Result<ToolResponse> {
        let start = Instant::now();

        // 查找工具
        let tool = self
            .builtin
            .get(&request.name)
            .ok_or_else(|| Layer3Error::ToolNotFound(request.name.clone()))?;

        // 执行工具 (always use context-aware path so file tools get staleness check)
        let ctx = crate::builtin_tools::exec_context::current_context()
            .with_call_id(request.call_id.clone());
        let result = tool
            .execute_with_context(request.arguments.clone(), &ctx)
            .await
            .map_err(|e| Layer3Error::ToolExecutionFailed(e.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let response = ToolResponse {
            call_id: request.call_id.clone(),
            name: request.name.clone(),
            content: result,
            is_error: false,
            duration_ms,
        };

        // 记录
        self.record(&request, &response, duration_ms);

        Ok(response)
    }

    async fn execute_batch(&self, requests: Vec<ToolRequest>) -> Layer3Result<Vec<ToolResponse>> {
        // Parallel execution: all tools run concurrently via join_all.
        // Order is preserved (results[i] corresponds to requests[i]).
        // Fail-fast: any error aborts the batch (consistent with previous sequential behavior).
        let futures: Vec<_> = requests.into_iter().map(|req| self.execute(req)).collect();
        let results = futures::future::try_join_all(futures).await?;
        Ok(results)
    }

    fn is_available(&self, name: &str) -> bool {
        self.builtin.get(name).is_some()
    }

    fn get_meta(&self, name: &str) -> Option<ToolMeta> {
        // 需要在 BuiltinToolRegistry 中添加 get_meta 方法
        self.builtin.get(name).map(|t| t.meta())
    }

    fn list_tools(&self) -> Vec<ToolMeta> {
        self.builtin.list_meta()
    }
}

/// 参数验证器
pub struct JsonSchemaValidator {
    schemas: HashMap<String, serde_json::Value>,
}

impl JsonSchemaValidator {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    pub fn register_schema(&mut self, tool_name: String, schema: serde_json::Value) {
        self.schemas.insert(tool_name, schema);
    }
}

impl Default for JsonSchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolValidator for JsonSchemaValidator {
    fn validate(&self, request: &ToolRequest) -> bool {
        // 简化验证：检查必需字段是否存在
        if let Some(schema) = self.schemas.get(&request.name) {
            if let Some(required) = schema.get("required") {
                if let Some(required_arr) = required.as_array() {
                    for field in required_arr {
                        if let Some(field_name) = field.as_str() {
                            if request.arguments.get(field_name).is_none() {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    fn validate_with_reason(&self, request: &ToolRequest) -> Result<(), String> {
        if self.validate(request) {
            Ok(())
        } else {
            Err(format!("Validation failed for tool: {}", request.name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = DefaultToolExecutor::new();
        // 整改后默认注册了内置工具
        let tools = executor.list_tools();
        assert!(!tools.is_empty(), "Expected tools to be registered");
        // 至少包含基础工具：read_file, write_file, bash, grep, glob
        assert!(
            tools.len() >= 5,
            "Expected at least 5 basic tools, got {}",
            tools.len()
        );
    }

    #[test]
    fn test_validator() {
        let validator = JsonSchemaValidator::new();
        let request = ToolRequest {
            call_id: "1".to_string(),
            name: "test".to_string(),
            arguments: serde_json::json!({"path": "test"}),
        };
        assert!(validator.validate(&request));
    }
}
