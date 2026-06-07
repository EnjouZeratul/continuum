//! # Execution Context
//!
//! Agent 执行上下文，支持序列化和恢复。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{AgentId, AgentState, Message, SessionId, ToolCall, ToolResult};

/// 执行上下文
///
/// Agent 执行的完整上下文，包含所有必要的状态信息。
/// 这是 Python 版本 ExecutionContext 的 Rust 移植。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// 会话 ID
    pub session_id: SessionId,
    /// Agent ID
    pub agent_id: AgentId,

    // 状态
    pub state: AgentState,
    pub iteration: i32,
    pub max_iterations: i32,

    /// 消息历史（OpenAI 格式）
    pub messages: Vec<Message>,

    /// Tool 管理
    pub tools_registered: Vec<String>,
    pub tool_calls_pending: Vec<ToolCall>,
    pub tool_results_cache: Vec<ToolResult>,

    /// 配置快照
    pub model: String,
    pub temperature: f32,
    pub system_prompt: String,

    /// 追踪数据
    pub tokens_total: i64,
    pub tokens_prompt: i64,
    pub tokens_completion: i64,
    pub cost_estimate: f64,

    /// 元数据
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub checkpoint_count: i32,

    /// 扩展数据（用于存储额外信息）
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            session_id: SessionId::new(),
            agent_id: AgentId::new(),
            state: AgentState::Idle,
            iteration: 0,
            max_iterations: 100,
            messages: Vec::new(),
            tools_registered: Vec::new(),
            tool_calls_pending: Vec::new(),
            tool_results_cache: Vec::new(),
            model: "claude-sonnet-4-6".to_string(),
            temperature: 0.7,
            system_prompt: String::new(),
            tokens_total: 0,
            tokens_prompt: 0,
            tokens_completion: 0,
            cost_estimate: 0.0,
            created_at: now,
            last_updated: now,
            checkpoint_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// 使用配置创建执行上下文
    pub fn with_config(
        model: impl Into<String>,
        temperature: f32,
        system_prompt: impl Into<String>,
    ) -> Self {
        let mut ctx = Self::new();
        ctx.model = model.into();
        ctx.temperature = temperature;
        ctx.system_prompt = system_prompt.into();
        ctx
    }

    /// 添加消息
    pub fn add_message(&mut self, role: &str, content: &str) {
        use crate::types::MessageRole;
        let role = match role {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };
        self.messages.push(Message::new(role, content));
        self.iteration += 1;
        self.touch();
    }

    /// 更新最后修改时间
    pub fn touch(&mut self) {
        self.last_updated = Utc::now();
    }

    /// 增加检查点计数
    pub fn increment_checkpoint(&mut self) {
        self.checkpoint_count += 1;
        self.touch();
    }

    /// 添加 token 使用
    pub fn add_tokens(&mut self, prompt: i64, completion: i64) {
        self.tokens_prompt += prompt;
        self.tokens_completion += completion;
        self.tokens_total += prompt + completion;
        self.touch();
    }

    /// 序列化为字典
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// 从字典恢复
    pub fn from_dict(data: &serde_json::Value) -> serde_json::Result<Self> {
        serde_json::from_value(data.clone())
    }

    /// 转换为 JSON 字符串
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// 从 JSON 字符串恢复
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// 设置状态
    pub fn set_state(&mut self, state: AgentState) {
        self.state = state;
        self.touch();
    }

    /// 检查是否可以继续
    pub fn can_continue(&self) -> bool {
        self.iteration < self.max_iterations
            && matches!(
                self.state,
                AgentState::Running | AgentState::Idle | AgentState::WaitingTool
            )
    }

    /// 获取消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_creation() {
        let ctx = ExecutionContext::new();
        assert_eq!(ctx.state, AgentState::Idle);
        assert_eq!(ctx.iteration, 0);
        assert!(ctx.messages.is_empty());
    }

    #[test]
    fn test_add_message() {
        let mut ctx = ExecutionContext::new();
        ctx.add_message("user", "Hello");

        assert_eq!(ctx.messages.len(), 1);
        assert_eq!(ctx.iteration, 1);
    }

    #[test]
    fn test_add_tokens() {
        let mut ctx = ExecutionContext::new();
        ctx.add_tokens(100, 50);

        assert_eq!(ctx.tokens_prompt, 100);
        assert_eq!(ctx.tokens_completion, 50);
        assert_eq!(ctx.tokens_total, 150);
    }

    #[test]
    fn test_serialization() {
        let ctx = ExecutionContext::new();
        let json = ctx.to_json().unwrap();
        let restored = ExecutionContext::from_json(&json).unwrap();

        assert_eq!(ctx.session_id, restored.session_id);
    }
}
