//! # Built-in Tools
//!
//! 内置工具集：提供 40+ 常用工具的实现。

pub mod adapter;
pub mod code;
pub mod data_processing;
pub mod exec_context;
pub mod file_ops;
pub mod git_tools;
pub mod limits;
pub mod memory_tools;
pub mod metrics;
pub mod network;
pub mod network_safety;
pub mod network_tools;
pub mod read_state;
pub mod search;
pub mod shell;
pub mod skill_tools;
pub mod system_tools;
pub mod text_tools;
pub mod web_search;
pub mod workflow_tools;

// Re-export safety modules from sh-safety crate (extracted for fast fuzz compilation)
pub use sh_safety::{path_safety, safe_truncate, secret_scrub};

// Re-export adapter for Layer 2 integration
pub use adapter::{register_builtin_tools, ToolAdapter};

use crate::types::{Layer3Result, ToolCategory, ToolMeta};
use async_trait::async_trait;
use std::collections::HashMap;

/// 内置工具 trait
///
/// 所有内置工具必须实现此 trait。
///
/// # v1.1.0 演进说明
///
/// v1.1.0 引入 `LegacyBuiltinTool` 作为本 trait 的别名，标记 sessionless
/// 工具。新工具（需要 session 上下文）应实现未来的 `ContextualBuiltinTool`
/// trait（v1.1.1+ 引入），通过 `ExecutionContext` 参数接收会话状态。
///
/// 详见 `docs/superpowers/specs/2026-06-14-stale-read-prevention-design.md`。
#[async_trait]
pub trait BuiltinTool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    fn parameters_schema(&self) -> serde_json::Value;

    /// 工具分类
    fn category(&self) -> ToolCategory;

    /// 是否需要用户确认
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// 是否为危险操作
    fn is_dangerous(&self) -> bool {
        false
    }

    /// 执行工具
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String>;

    /// Context-aware execute. Default implementation ignores the context and
    /// delegates to `execute(args)`. Tools that need session-scoped state
    /// (e.g., `ReadFileTool`, `EditFileTool`, `WriteFileTool` for stale-read
    /// prevention) override this method.
    ///
    /// The executor should always call `execute_with_context`; the default
    /// impl makes it safe for sessionless tools.
    async fn execute_with_context(
        &self,
        args: serde_json::Value,
        _ctx: &crate::builtin_tools::exec_context::ExecutionContext,
    ) -> Layer3Result<String> {
        self.execute(args).await
    }

    /// 获取元数据
    fn meta(&self) -> ToolMeta {
        ToolMeta {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
            requires_confirmation: self.requires_confirmation(),
            is_dangerous: self.is_dangerous(),
            category: self.category(),
        }
    }
}

// v1.1.0 引入：标记 sessionless 工具的概念性别名。
//
// 当前 `BuiltinTool` 就是 sessionless trait（无 context 参数）。v1.1.1+ 将
// 引入 `ContextualBuiltinTool` 接收 `ExecutionContext`，届时本 trait 会
// 重命名为 `LegacyBuiltinTool`，新 `BuiltinTool` 名字会绑定到 context-aware 版本。
//
// 工具实现者目前实现的 `BuiltinTool` 即等价于未来的 `LegacyBuiltinTool`。
//
// 注：Rust 不支持 trait alias，因此这里没有真正的 `type` 别名 — 通过文档约定。

/// 内置工具注册表
///
/// 管理所有内置工具的注册和查找。
pub struct BuiltinToolRegistry {
    tools: HashMap<String, Box<dyn BuiltinTool>>,
}

impl BuiltinToolRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 创建并注册所有默认工具
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Shell 工具 (1)
        registry.register(Box::new(shell::BashTool::new()));

        // 搜索工具 (2)
        registry.register(Box::new(search::GrepTool));
        registry.register(Box::new(search::GlobTool));

        // Web 搜索工具 (1)
        registry.register(Box::new(web_search::WebSearchTool::new()));

        // 文件操作工具 (8)
        registry.register(Box::new(file_ops::ReadFileTool::new()));
        registry.register(Box::new(file_ops::WriteFileTool::new()));
        registry.register(Box::new(file_ops::EditFileTool::new()));
        registry.register(Box::new(file_ops::ListDirectoryTool::new()));
        registry.register(Box::new(file_ops::CreateDirectoryTool));
        registry.register(Box::new(file_ops::MoveFileTool::new()));
        registry.register(Box::new(file_ops::CopyFileTool));
        registry.register(Box::new(file_ops::DeleteFileTool::new()));

        // Workflow 工具 (3)
        registry.register(Box::new(workflow_tools::CreateCheckpointTool::new()));
        registry.register(Box::new(workflow_tools::RestoreCheckpointTool::new()));
        registry.register(Box::new(workflow_tools::ListCheckpointsTool::new()));

        // Code Analysis 工具 (4)
        registry.register(Box::new(code::GoToDefinitionTool));
        registry.register(Box::new(code::FindReferencesTool));
        registry.register(Box::new(code::GetHoverTool));
        registry.register(Box::new(code::RenameSymbolTool));

        // Network 工具 (2)
        registry.register(Box::new(network::HttpRequestTool::new()));
        registry.register(Box::new(network::WebFetchTool::new()));

        // System Tools (8)
        registry.register(Box::new(system_tools::GetEnvTool::new()));
        registry.register(Box::new(system_tools::ListEnvTool::new()));
        registry.register(Box::new(system_tools::SetEnvTool));

        // Memory Tools (3)
        registry.register(Box::new(memory_tools::SaveMemoryTool::new()));
        registry.register(Box::new(memory_tools::QueryMemoryTool::new()));
        registry.register(Box::new(memory_tools::ClearMemoryTool::new()));

        // Data Processing 工具 (14)
        registry.register(Box::new(data_processing::JsonParseTool));
        registry.register(Box::new(data_processing::JsonStringifyTool));
        registry.register(Box::new(data_processing::YamlParseTool));
        registry.register(Box::new(data_processing::YamlStringifyTool));
        registry.register(Box::new(data_processing::TomlParseTool));
        registry.register(Box::new(data_processing::CsvParseTool));
        registry.register(Box::new(data_processing::Base64EncodeTool));
        registry.register(Box::new(data_processing::Base64DecodeTool));
        registry.register(Box::new(data_processing::UrlEncodeTool));
        registry.register(Box::new(data_processing::UrlDecodeTool));
        registry.register(Box::new(data_processing::HashTool));
        registry.register(Box::new(data_processing::UuidGenerateTool));

        // Network Tools (6)
        registry.register(Box::new(network_tools::HttpGetTool));
        registry.register(Box::new(network_tools::HttpPostTool));
        registry.register(Box::new(network_tools::DownloadFileTool));
        registry.register(Box::new(network_tools::PingTool));
        registry.register(Box::new(network_tools::DnsLookupTool));

        // Git Tools (8)
        registry.register(Box::new(git_tools::GitStatusTool));
        registry.register(Box::new(git_tools::GitLogTool));
        registry.register(Box::new(git_tools::GitDiffTool));
        registry.register(Box::new(git_tools::GitBranchTool));
        registry.register(Box::new(git_tools::GitAddTool));
        registry.register(Box::new(git_tools::GitCommitTool));
        registry.register(Box::new(git_tools::GitShowTool));
        registry.register(Box::new(git_tools::GitStashTool));

        // System Tools (remaining - GetEnv/ListEnv/SetEnv registered above)
        registry.register(Box::new(system_tools::GetCwdTool));
        registry.register(Box::new(system_tools::ChangeDirTool));
        registry.register(Box::new(system_tools::SystemInfoTool));
        registry.register(Box::new(system_tools::ProcessListTool::new()));
        registry.register(Box::new(system_tools::DiskUsageTool));
        registry.register(Box::new(system_tools::MemoryUsageTool));

        // Text Processing Tools (7)
        registry.register(Box::new(text_tools::CountLinesTool));
        registry.register(Box::new(text_tools::WordFrequencyTool));
        registry.register(Box::new(text_tools::TextTransformTool));
        registry.register(Box::new(text_tools::TextSplitTool));
        registry.register(Box::new(text_tools::RegexMatchTool));
        registry.register(Box::new(text_tools::TextDiffTool));
        registry.register(Box::new(text_tools::SortLinesTool));

        registry
    }

    /// 注册工具
    pub fn register(&mut self, tool: Box<dyn BuiltinTool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// 获取工具
    pub fn get(&self, name: &str) -> Option<&dyn BuiltinTool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// 列出所有工具
    pub fn list(&self) -> Vec<&dyn BuiltinTool> {
        self.tools.values().map(|b| b.as_ref()).collect()
    }

    /// 列出所有工具元数据
    pub fn list_meta(&self) -> Vec<ToolMeta> {
        self.tools.values().map(|t| t.meta()).collect()
    }

    /// 按分类列出工具
    pub fn list_by_category(&self, category: ToolCategory) -> Vec<&dyn BuiltinTool> {
        self.tools
            .values()
            .filter(|t| t.category() == category)
            .map(|b| b.as_ref())
            .collect()
    }
}

impl Default for BuiltinToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 工具分类快捷常量
pub const FILE_OPS_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "create_file",
    "delete_file",
    "list_directory",
    "copy_file",
    "move_file",
];

pub const SEARCH_TOOLS: &[&str] = &["grep", "glob", "find_in_files", "search_content"];

pub const SHELL_TOOLS: &[&str] = &["bash", "run_command", "shell_exec"];

pub const CODE_TOOLS: &[&str] = &[
    "go_to_definition",
    "find_references",
    "get_hover",
    "list_symbols",
];

pub const MEMORY_TOOLS: &[&str] = &["save_memory", "load_memory", "query_memory", "clear_memory"];

pub const WORKFLOW_TOOLS: &[&str] = &[
    "create_checkpoint",
    "restore_checkpoint",
    "list_checkpoints",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = BuiltinToolRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_tool_categories() {
        assert!(!FILE_OPS_TOOLS.is_empty());
        assert!(!SEARCH_TOOLS.is_empty());
    }
}
