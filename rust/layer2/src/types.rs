//! # Layer 2 Core Types
//!
//! 定义 Layer 2 使用的核心类型，供所有模块共享。

use serde::{Deserialize, Serialize};
use sh_layer1::generate_short_id;
use std::fmt;

/// 会话 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(generate_short_id())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Agent ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new() -> Self {
        Self("default".to_string())
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 任务 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(generate_short_id())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 检查点 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    pub fn new() -> Self {
        Self(generate_short_id())
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Agent 执行状态机
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    #[default]
    Idle,
    Running,
    ToolCalling,
    WaitingTool,
    Stopped,
    Error,
    Completed,
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Running => write!(f, "running"),
            Self::ToolCalling => write!(f, "tool_calling"),
            Self::WaitingTool => write!(f, "waiting_tool"),
            Self::Stopped => write!(f, "stopped"),
            Self::Error => write!(f, "error"),
            Self::Completed => write!(f, "completed"),
        }
    }
}

impl std::str::FromStr for AgentState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "idle" => Ok(Self::Idle),
            "running" => Ok(Self::Running),
            "tool_calling" => Ok(Self::ToolCalling),
            "waiting_tool" => Ok(Self::WaitingTool),
            "stopped" => Ok(Self::Stopped),
            "error" => Ok(Self::Error),
            "completed" => Ok(Self::Completed),
            _ => Err(format!("Unknown agent state: {}", s)),
        }
    }
}

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// OpenAI 格式消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            name: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// 工具结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
    pub is_error: bool,
}

/// Hook 事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    BeforeAgentStart,
    AfterAgentStop,
    BeforeToolCall,
    AfterToolCall,
    BeforeCheckpoint,
    AfterCheckpoint,
    OnError,
    OnStateChange,
}

/// 工作流节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// 检查点元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMeta {
    pub checkpoint_id: CheckpointId,
    pub session_id: SessionId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub trigger: String,
    pub iteration: i32,
    #[serde(rename = "_checksum")]
    pub checksum: String,
}

/// 会话元数据（用于列表显示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub state: AgentState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
    pub checkpoint_count: i32,
}

/// 统一的 Layer 2 Result 类型
pub type Layer2Result<T> = anyhow::Result<T>;

/// 统一的 Layer 2 Error 类型
#[derive(Debug, thiserror::Error)]
pub enum Layer2Error {
    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(CheckpointId),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("Lock acquisition timeout")]
    LockTimeout,

    #[error("Invalid state transition: from {from} to {to}")]
    InvalidStateTransition { from: AgentState, to: AgentState },

    #[error("Checkpoint corrupted: {0}")]
    CheckpointCorrupted(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Max iterations reached: {0}")]
    MaxIterations(i32),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("LLM client not configured")]
    LlmNotConfigured,

    #[error("Max sessions reached: {0}")]
    MaxSessionsReached(usize),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

// 注意：不需要手动实现 From<Layer2Error> for anyhow::Error，
// 因为 Layer2Error 实现了 std::error::Error + Send + Sync + 'static，
// anyhow 会自动实现这个转换。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_creation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
        assert_eq!(id1.0.len(), 8);
    }

    #[test]
    fn test_agent_state_display() {
        assert_eq!(format!("{}", AgentState::Running), "running");
        assert_eq!(format!("{}", AgentState::ToolCalling), "tool_calling");
    }

    #[test]
    fn test_agent_state_from_str() {
        let state: AgentState = "running".parse().unwrap();
        assert_eq!(state, AgentState::Running);
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");
    }
}
