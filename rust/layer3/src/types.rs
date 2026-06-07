//! # Layer 3 Core Types
//!
//! Layer 3 使用的核心类型定义。

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

// ============================================================================
// Tool 相关类型
// ============================================================================

/// 工具 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ToolId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// 工具调用 ID（由 LLM 生成）
    pub call_id: String,
    /// 工具名称
    pub name: String,
    /// JSON 格式的参数
    pub arguments: serde_json::Value,
}

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// 对应的调用 ID
    pub call_id: String,
    /// 工具名称
    pub name: String,
    /// 执行结果内容
    pub content: String,
    /// 是否为错误
    pub is_error: bool,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

impl ToolResponse {
    pub fn success(
        call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            name: name.into(),
            content: content.into(),
            is_error: false,
            duration_ms: 0,
        }
    }

    pub fn error(
        call_id: impl Into<String>,
        name: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            name: name.into(),
            content: error.into(),
            is_error: true,
            duration_ms: 0,
        }
    }
}

/// 工具元数据（用于注册和发现）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    /// 工具名称（唯一标识）
    pub name: String,
    /// 工具描述（供 LLM 理解用途）
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: serde_json::Value,
    /// 是否需要用户确认
    #[serde(default)]
    pub requires_confirmation: bool,
    /// 是否为危险操作
    #[serde(default)]
    pub is_dangerous: bool,
    /// 工具分类
    pub category: ToolCategory,
}

/// 工具分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// 文件操作（读、写、编辑）
    FileOps,
    /// 搜索（grep、文件搜索）
    Search,
    /// Shell 命令执行
    Shell,
    /// 网络请求
    Network,
    /// 代码分析（LSP、AST）
    CodeAnalysis,
    /// 记忆操作
    Memory,
    /// 工作流控制
    Workflow,
    /// 系统/进程管理
    System,
    /// 数据处理（JSON/YAML/CSV等）
    DataProcessing,
    /// 文本处理（统计、转换等）
    TextProcessing,
    /// 版本控制（Git等）
    VersionControl,
    /// 其他
    Other,
}

// ============================================================================
// Memory 相关类型
// ============================================================================

/// 记忆层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// 工作记忆：当前对话上下文
    #[default]
    Working,
    /// 会话记忆：单次会话内的持久化
    Session,
    /// 项目记忆：项目级别的知识库
    Project,
    /// 长期记忆：跨项目的通用知识
    LongTerm,
}

/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 条目 ID
    pub id: String,
    /// 记忆层级
    pub tier: MemoryTier,
    /// 内容
    pub content: String,
    /// 元数据（可选的额外信息）
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后访问时间
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    /// 访问次数
    #[serde(default)]
    pub access_count: u32,
    /// 重要性分数 (0.0-1.0)
    #[serde(default)]
    pub importance: f32,
}

/// 记忆查询
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    /// 查询文本
    pub query: String,
    /// 限制层级（可选）
    pub tier: Option<MemoryTier>,
    /// 限制数量
    pub limit: Option<usize>,
    /// 时间范围（可选）
    pub time_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
}

// ============================================================================
// Query Engine 相关类型
// ============================================================================

/// 代码查询类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryType {
    /// 符号定义
    Definition,
    /// 符号引用
    References,
    /// 符号实现
    Implementations,
    /// 类型定义
    TypeDefinition,
    /// 文档符号
    DocumentSymbols,
    /// 工作区符号
    WorkspaceSymbols,
    /// 悬停信息
    Hover,
}

/// 代码位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    /// 文件路径
    pub file: PathBuf,
    /// 行号（1-based）
    pub line: u32,
    /// 列号（1-based）
    pub column: u32,
}

impl fmt::Display for CodeLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file.display(), self.line, self.column)
    }
}

/// 代码范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRange {
    pub start: CodeLocation,
    pub end: CodeLocation,
}

/// 查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// 查询类型
    pub query_type: QueryType,
    /// 结果位置
    pub location: CodeLocation,
    /// 结果范围（可选）
    pub range: Option<CodeRange>,
    /// 显示文本
    pub display_text: String,
    /// 所在文件内容片段（可选）
    pub snippet: Option<String>,
}

// ============================================================================
// Process 相关类型
// ============================================================================

/// 进程状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    Running,
    Stopped,
    Exited,
    Killed,
}

/// 进程信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// 进程 ID
    pub pid: u32,
    /// 进程名称
    pub name: String,
    /// 状态
    pub state: ProcessState,
    /// 启动时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// 命令行
    pub command: String,
    /// 工作目录
    pub working_dir: PathBuf,
}

// ============================================================================
// Error 类型
// ============================================================================

/// Layer 3 统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum Layer3Error {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("Tool validation failed: {0}")]
    ToolValidationFailed(String),

    #[error("Memory not found: {0}")]
    MemoryNotFound(String),

    #[error("Memory query failed: {0}")]
    MemoryQueryFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("LSP error: {0}")]
    LspError(String),

    #[error("Sandbox error: {0}")]
    SandboxError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Vector store error: {0}")]
    VectorStoreError(String),

    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    VectorDimensionMismatch { expected: usize, actual: usize },

    #[error("Vector not found: {0}")]
    VectorNotFound(String),

    #[error("Vector operation failed: {operation} - {reason}")]
    VectorOperationFailed { operation: String, reason: String },

    #[error("Persistence error: {0}")]
    PersistenceError(String),

    #[error("Invalid vector: {0}")]
    InvalidVector(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Layer 3 Result 类型
pub type Layer3Result<T> = anyhow::Result<T>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_response_creation() {
        let resp = ToolResponse::success("call_1", "test_tool", "result");
        assert!(!resp.is_error);
        assert_eq!(resp.name, "test_tool");

        let err_resp = ToolResponse::error("call_2", "test_tool", "error");
        assert!(err_resp.is_error);
    }

    #[test]
    fn test_memory_tier_default() {
        let tier = MemoryTier::default();
        assert_eq!(tier, MemoryTier::Working);
    }

    #[test]
    fn test_code_location_display() {
        let loc = CodeLocation {
            file: PathBuf::from("src/main.rs"),
            line: 10,
            column: 5,
        };
        assert!(loc.to_string().contains("main.rs"));
    }
}
