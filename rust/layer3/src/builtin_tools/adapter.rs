//! # Layer 2 Tool Adapter
//!
//! 将 Layer 3 builtin_tools 适配为 Layer 2 Tool trait。

use crate::builtin_tools::BuiltinTool;
use async_trait::async_trait;
use sh_layer2::{Layer2Result, Tool as Layer2Tool, ToolRegistryTrait, ToolResult};

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
        // 解析参数
        let args_value: serde_json::Value = if args.is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            serde_json::from_str(args).map_err(|e| {
                sh_layer2::Layer2Error::AgentError(format!("Parse args error: {}", e))
            })?
        };

        // 执行工具
        let result = self
            .inner
            .execute(args_value)
            .await
            .map_err(|e| sh_layer2::Layer2Error::AgentError(e.to_string()))?;

        // 返回 ToolResult
        Ok(ToolResult {
            tool_call_id: String::new(),
            name: self.inner.name().to_string(),
            content: result,
            is_error: false,
        })
    }
}

/// 注册所有内置工具到 Layer 2 ToolRegistry
pub fn register_builtin_tools(registry: &sh_layer2::ToolRegistry) -> anyhow::Result<()> {
    use super::code::*;
    use super::file_ops::*;
    use super::memory_tools::*;
    use super::search::*;
    use super::shell::*;
    use super::workflow_tools::*;

    // 文件操作工具
    registry.register(Box::new(ToolAdapter::new(Box::new(ReadFileTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(WriteFileTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(EditFileTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(ListDirectoryTool))))?;

    // 搜索工具
    registry.register(Box::new(ToolAdapter::new(Box::new(GrepTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(GlobTool))))?;

    // Shell 工具
    registry.register(Box::new(ToolAdapter::new(Box::new(BashTool))))?;

    // 代码分析工具
    registry.register(Box::new(ToolAdapter::new(Box::new(GoToDefinitionTool))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(FindReferencesTool))))?;

    // 记忆工具
    registry.register(Box::new(ToolAdapter::new(Box::new(SaveMemoryTool::new()))))?;
    registry.register(Box::new(ToolAdapter::new(Box::new(QueryMemoryTool::new()))))?;

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
    use super::super::file_ops::ReadFileTool;
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let tool = ToolAdapter::new(Box::new(ReadFileTool));
        assert_eq!(tool.name(), "read_file");
    }
}
