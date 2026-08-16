//! # Layer 2 Tool Adapter
//!
//! 将 Layer 3 builtin_tools 适配为 Layer 2 Tool trait。

use crate::builtin_tools::BuiltinTool;
use async_trait::async_trait;
use sh_layer2::{Layer2Result, Tool as Layer2Tool, ToolRegistryTrait, ToolResult};
use std::sync::Arc;

/// 适配器：将 Layer3 BuiltinTool 适配为 Layer2 Tool
pub struct ToolAdapter {
    inner: Box<dyn BuiltinTool>,
}

impl ToolAdapter {
    pub fn new(tool: Box<dyn BuiltinTool>) -> Self {
        Self { inner: tool }
    }
}

#[async_trait]
impl Layer2Tool for ToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters(&self) -> serde_json::Value {
        self.inner.parameters_schema()
    }

    async fn execute(&self, args: &str) -> Layer2Result<ToolResult> {
        // Legacy path: no call_id available — delegate with empty.
        self.execute_with_call_id(args, "").await
    }

    async fn execute_with_call_id(&self, args: &str, call_id: &str) -> Layer2Result<ToolResult> {
        // 解析参数
        let args_value: serde_json::Value = if args.is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            serde_json::from_str(args).map_err(|e| {
                sh_layer2::Layer2Error::AgentError(format!("Parse args error: {}", e))
            })?
        };

        // Build ExecutionContext and set as current for the duration of this call
        let ctx = crate::builtin_tools::exec_context::current_context().with_call_id(call_id);
        crate::builtin_tools::exec_context::set_current_context(ctx.clone());

        // 执行工具 (always via context-aware path so file tools get staleness check)
        let result = self
            .inner
            .execute_with_context(args_value, &ctx)
            .await
            .map_err(|e| sh_layer2::Layer2Error::AgentError(e.to_string()));

        // Clear context (avoid leaking across calls in same process)
        crate::builtin_tools::exec_context::clear_current_context();

        let result = result?;

        // 返回 ToolResult — tool_call_id now propagated (was String::new())
        Ok(ToolResult {
            tool_call_id: call_id.to_string(),
            name: self.inner.name().to_string(),
            content: result,
            is_error: false,
        })
    }
}

/// 注册所有内置工具到 Layer 2 ToolRegistry
///
/// 记忆工具共享同一个（临时、非持久）UnifiedMemorySystem —— 单测与
/// 不落盘场景。生产装配请用 [`register_builtin_tools_with_memory`]
/// 注入带 ProjectMemory 的持久系统。
pub fn register_builtin_tools(registry: &sh_layer2::ToolRegistry) -> anyhow::Result<()> {
    let memory = Arc::new(crate::memory_system::UnifiedMemorySystem::new("default"));
    register_builtin_tools_with_memory(registry, memory)
}

/// 同 [`register_builtin_tools`]，但记忆工具使用调用方提供的统一记忆
/// 系统（可含 ProjectMemory / LongTermMemory 持久后端）。
pub fn register_builtin_tools_with_memory(
    registry: &sh_layer2::ToolRegistry,
    memory: Arc<crate::memory_system::UnifiedMemorySystem>,
) -> anyhow::Result<()> {
    use super::code::*;
    use super::file_ops::*;
    use super::memory_tools::*;
    use super::search::*;
    use super::shell::*;
    use super::workflow_tools::*;

    // 文件操作工具
    registry.register(Box::new(ToolAdapter::new(Box::new(ReadFileTool::new()))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(WriteFileTool::new()))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(EditFileTool::new()))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(
        ListDirectoryTool::new(),
    ))))?;

    // 搜索工具
    registry.register(Box::new(ToolAdapter::new(Box::new(GrepTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(GlobTool))))?;

    // Shell 工具
    registry.register(Box::new(ToolAdapter::new(Box::new(BashTool::new()))))?;

    // 代码分析工具
    registry.register(Box::new(ToolAdapter::new(Box::new(GoToDefinitionTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(FindReferencesTool))))?;

    // 记忆工具（共享同一系统 —— save 的条目 query 立即可见）
    registry.register(Box::new(ToolAdapter::new(Box::new(
        SaveMemoryTool::with_system(memory.clone()),
    ))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(
        QueryMemoryTool::with_system(memory.clone()),
    ))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(
        ClearMemoryTool::with_system(memory),
    ))))?;

    // 工作流工具
    registry.register(Box::new(ToolAdapter::new(Box::new(
        CreateCheckpointTool::new(),
    ))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(
        RestoreCheckpointTool::new(),
    ))))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::data_processing::UuidGenerateTool;
    use super::super::file_ops::ReadFileTool;
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let tool = ToolAdapter::new(Box::new(ReadFileTool::new()));
        assert_eq!(tool.name(), "read_file");
    }

    #[test]
    fn test_adapter_methods_forward() {
        let tool = ToolAdapter::new(Box::new(UuidGenerateTool));
        assert_eq!(tool.name(), "uuid_generate");
        assert!(!tool.description().is_empty());
        assert!(tool.parameters().is_object());
    }

    #[tokio::test]
    async fn test_execute_empty_args() {
        let tool = ToolAdapter::new(Box::new(UuidGenerateTool));
        let result = tool.execute("").await.unwrap();
        assert!(!result.content.is_empty());
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_execute_with_call_id_propagates() {
        let tool = ToolAdapter::new(Box::new(UuidGenerateTool));
        let result = tool.execute_with_call_id("{}", "call-123").await.unwrap();
        assert_eq!(result.tool_call_id, "call-123");
        assert_eq!(result.name, "uuid_generate");
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_execute_invalid_json_errors() {
        let tool = ToolAdapter::new(Box::new(UuidGenerateTool));
        let result = tool.execute("not valid json {{{").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_tool_error_wrapped() {
        // ReadFileTool on nonexistent path → tool error wrapped as Layer2Error
        let tool = ToolAdapter::new(Box::new(ReadFileTool::new()));
        let result = tool
            .execute(r#"{"path":"/nonexistent/adapter_test_xyz_123.txt"}"#)
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_register_builtin_tools() {
        let registry = sh_layer2::ToolRegistry::new();
        register_builtin_tools(&registry).unwrap();
        assert!(
            registry.count() >= 10,
            "expected >= 10 tools, got {}",
            registry.count()
        );
    }
}
