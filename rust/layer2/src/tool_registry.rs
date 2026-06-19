//! # Tool Registry
//!
//! 工具注册和发现机制。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::types::{Layer2Error, Layer2Result, ToolResult};

/// 工具元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub required: Vec<String>,
}

/// 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// 工具定义（OpenAI 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: FunctionDefinition,
}

/// 函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// 工具接口
///
/// 所有工具必须实现此接口。
#[async_trait]
pub trait Tool: Send + Sync {
    /// 获取工具名称
    fn name(&self) -> &str;

    /// 获取工具描述
    fn description(&self) -> &str;

    /// 获取参数 schema
    fn parameters(&self) -> serde_json::Value;

    /// 执行工具
    async fn execute(&self, args: &str) -> Layer2Result<ToolResult>;

    /// Context-aware execute: receives the LLM-issued `call_id` so it can be
    /// propagated to the resulting `ToolResult.tool_call_id` (instead of being
    /// lost at the adapter boundary).
    ///
    /// Default impl: ignore call_id, delegate to `execute(args)`. The result
    /// will then have `tool_call_id: String::new()` (legacy behavior).
    /// Adapters that want to preserve call_id override this method.
    async fn execute_with_call_id(&self, args: &str, _call_id: &str) -> Layer2Result<ToolResult> {
        self.execute(args).await
    }

    /// 验证参数
    fn validate_args(&self, _args: &serde_json::Value) -> Layer2Result<bool> {
        // 默认实现：总是返回 true
        Ok(true)
    }
}

/// 工具注册接口
#[async_trait]
pub trait ToolRegistryTrait: Send + Sync {
    /// 注册工具
    fn register(&self, tool: Box<dyn Tool>) -> Layer2Result<()>;

    /// 注销工具
    fn unregister(&self, name: &str) -> Layer2Result<bool>;

    /// 获取工具
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;

    /// 检查工具是否存在
    fn exists(&self, name: &str) -> bool;

    /// 列出所有工具名称
    fn list(&self) -> Vec<String>;

    /// 获取所有工具定义（OpenAI 格式）
    fn definitions(&self) -> Vec<ToolDefinition>;

    /// 执行工具
    async fn execute(&self, name: &str, args: &str) -> Layer2Result<ToolResult>;

    /// 执行工具 (with call_id propagation). Default delegates to execute().
    async fn execute_with_call_id(
        &self,
        name: &str,
        args: &str,
        call_id: &str,
    ) -> Layer2Result<ToolResult> {
        let _ = call_id;
        self.execute(name, args).await
    }

    /// 获取工具数量
    fn count(&self) -> usize;
}

/// 工具注册表实现
pub struct ToolRegistry {
    tools: parking_lot::RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// 创建带内置工具的注册表
    pub fn with_builtin_tools() -> Self {
        Self::new()
        // 内置工具将在 Layer 3 实现
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistryTrait for ToolRegistry {
    fn register(&self, tool: Box<dyn Tool>) -> Layer2Result<()> {
        let mut tools = self.tools.write();
        let name = tool.name().to_string();
        tools.insert(name, Arc::from(tool));
        Ok(())
    }

    fn unregister(&self, name: &str) -> Layer2Result<bool> {
        let mut tools = self.tools.write();
        Ok(tools.remove(name).is_some())
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read();
        tools.get(name).cloned()
    }

    fn exists(&self, name: &str) -> bool {
        let tools = self.tools.read();
        tools.contains_key(name)
    }

    fn list(&self) -> Vec<String> {
        let tools = self.tools.read();
        tools.keys().cloned().collect()
    }

    fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read();
        tools
            .values()
            .map(|tool| ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters(),
                },
            })
            .collect()
    }

    async fn execute(&self, name: &str, args: &str) -> Layer2Result<ToolResult> {
        let tool = self
            .get(name)
            .ok_or_else(|| Layer2Error::ToolNotFound(name.to_string()))?;

        tool.execute(args).await
    }

    async fn execute_with_call_id(
        &self,
        name: &str,
        args: &str,
        call_id: &str,
    ) -> Layer2Result<ToolResult> {
        let tool = self
            .get(name)
            .ok_or_else(|| Layer2Error::ToolNotFound(name.to_string()))?;

        tool.execute_with_call_id(args, call_id).await
    }

    fn count(&self) -> usize {
        let tools = self.tools.read();
        tools.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry_creation() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_tool_registry_list() {
        let registry = ToolRegistry::new();
        let list = registry.list();
        assert!(list.is_empty());
    }
}
