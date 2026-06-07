//! # Session Definition
//!
//! 会话结构定义。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{AgentId, AgentState, Message, MessageRole, SessionId, ToolCall, ToolResult};

/// 会话配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub temperature: f32,
    pub max_iterations: i32,
    pub system_prompt: Option<String>,
    /// 最大消息数量（防止内存无限增长）
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
    /// 最大工具注册数量
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,
}

fn default_max_messages() -> usize {
    1000
}
fn default_max_tools() -> usize {
    100
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-6".to_string(),
            temperature: 0.7,
            max_iterations: 100,
            system_prompt: None,
            max_messages: 1000,
            max_tools: 100,
        }
    }
}

/// 会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// 会话 ID
    pub session_id: SessionId,
    /// Agent ID
    pub agent_id: AgentId,
    /// 当前状态
    pub state: AgentState,
    /// 当前迭代次数
    pub iteration: i32,
    /// 最大迭代次数
    pub max_iterations: i32,
    /// 消息历史
    pub messages: Vec<Message>,
    /// 已注册工具
    pub tools_registered: Vec<String>,
    /// 待处理的工具调用
    pub tool_calls_pending: Vec<ToolCall>,
    /// 工具结果缓存
    pub tool_results_cache: Vec<ToolResult>,
    /// 模型名称
    pub model: String,
    /// 温度参数
    pub temperature: f32,
    /// 系统提示词
    pub system_prompt: String,
    /// Token 使用统计
    pub tokens_total: i64,
    pub tokens_prompt: i64,
    pub tokens_completion: i64,
    /// 成本估算
    pub cost_estimate: f64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
    /// 检查点计数
    pub checkpoint_count: i32,
    /// 最大消息数量
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
    /// 最大工具数量
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,
}

impl Session {
    /// 创建新会话
    pub fn new(config: &SessionConfig) -> Self {
        let now = Utc::now();
        Self {
            session_id: SessionId::new(),
            agent_id: AgentId::new(),
            state: AgentState::Idle,
            iteration: 0,
            max_iterations: config.max_iterations,
            messages: Vec::new(),
            tools_registered: Vec::new(),
            tool_calls_pending: Vec::new(),
            tool_results_cache: Vec::new(),
            model: config.model.clone(),
            temperature: config.temperature,
            system_prompt: config.system_prompt.clone().unwrap_or_default(),
            tokens_total: 0,
            tokens_prompt: 0,
            tokens_completion: 0,
            cost_estimate: 0.0,
            created_at: now,
            last_updated: now,
            checkpoint_count: 0,
            max_messages: config.max_messages,
            max_tools: config.max_tools,
        }
    }

    /// 添加用户消息
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message::user(content));
        self.trim_messages();
        self.iteration += 1;
        self.touch();
    }

    /// 添加助手消息
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message::assistant(content));
        self.trim_messages();
        self.touch();
    }

    /// 添加系统消息
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message::system(content));
        self.trim_messages();
        self.touch();
    }

    /// 当消息超过上限时，删除最旧的消息（保留第一条系统消息）
    fn trim_messages(&mut self) {
        if self.messages.len() > self.max_messages {
            let excess = self.messages.len() - self.max_messages;
            // 保留第一条消息（通常是系统提示）
            let first_is_system = self
                .messages
                .first()
                .map(|m| m.role == MessageRole::System)
                .unwrap_or(false);

            if first_is_system && excess > 0 {
                // 删除第二条到第excess+1条
                self.messages.drain(1..=excess.min(self.messages.len() - 1));
            } else {
                // 删除最旧的excess条
                self.messages.drain(0..excess);
            }
        }
    }

    /// 注册工具，如果超过上限则移除最旧的
    pub fn register_tool(&mut self, tool_name: &str) {
        if !self.tools_registered.contains(&tool_name.to_string()) {
            if self.tools_registered.len() >= self.max_tools {
                // 移除最旧的工具
                self.tools_registered.remove(0);
            }
            self.tools_registered.push(tool_name.to_string());
            self.touch();
        }
    }

    /// 更新最后修改时间
    pub fn touch(&mut self) {
        self.last_updated = Utc::now();
    }

    /// 检查是否可以继续执行
    pub fn can_continue(&self) -> bool {
        self.iteration < self.max_iterations
            && matches!(self.state, AgentState::Running | AgentState::Idle)
    }

    /// 序列化为 JSON
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// 从 JSON 反序列化
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// 转换为字典（兼容 Python 版本）
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new(&SessionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = Session::new(&config);

        assert!(session.messages.is_empty());
        assert_eq!(session.state, AgentState::Idle);
        assert_eq!(session.iteration, 0);
    }

    #[test]
    fn test_session_messages() {
        let config = SessionConfig::default();
        let mut session = Session::new(&config);

        session.add_user_message("Hello");
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.iteration, 1);

        session.add_assistant_message("Hi there!");
        assert_eq!(session.messages.len(), 2);
    }

    #[test]
    fn test_session_can_continue() {
        let config = SessionConfig {
            max_iterations: 5,
            ..Default::default()
        };

        let mut session = Session::new(&config);
        assert!(session.can_continue());

        session.state = AgentState::Running;
        assert!(session.can_continue());

        session.state = AgentState::Stopped;
        assert!(!session.can_continue());
    }

    #[test]
    fn test_session_serialization() {
        let config = SessionConfig::default();
        let session = Session::new(&config);

        let json = session.to_json().unwrap();
        let restored = Session::from_json(&json).unwrap();

        assert_eq!(session.session_id, restored.session_id);
        assert_eq!(session.state, restored.state);
    }

    #[test]
    fn test_session_max_messages_limit() {
        let config = SessionConfig {
            max_messages: 5,
            ..Default::default()
        };
        let mut session = Session::new(&config);

        // 添加超过上限的消息
        for i in 0..10 {
            session.add_user_message(&format!("Message {}", i));
        }

        // 应该被限制在 max_messages
        assert_eq!(session.messages.len(), 5);
    }

    #[test]
    fn test_session_preserves_system_message() {
        let config = SessionConfig {
            max_messages: 3,
            system_prompt: Some("System prompt".to_string()),
            ..Default::default()
        };
        let mut session = Session::new(&config);

        session.add_system_message("System prompt");
        for i in 0..5 {
            session.add_user_message(&format!("User {}", i));
        }

        // 第一条系统消息应该保留
        assert_eq!(session.messages.len(), 3);
        assert!(session
            .messages
            .first()
            .map(|m| m.role == MessageRole::System)
            .unwrap_or(false));
    }

    #[test]
    fn test_session_max_tools_limit() {
        let config = SessionConfig {
            max_tools: 3,
            ..Default::default()
        };
        let mut session = Session::new(&config);

        for i in 0..5 {
            session.register_tool(&format!("tool_{}", i));
        }

        // 应该被限制在 max_tools
        assert_eq!(session.tools_registered.len(), 3);
        // 最旧的工具应该被移除
        assert!(!session.tools_registered.contains(&"tool_0".to_string()));
        assert!(!session.tools_registered.contains(&"tool_1".to_string()));
    }

    #[test]
    fn test_session_no_duplicate_tools() {
        let config = SessionConfig::default();
        let mut session = Session::new(&config);

        session.register_tool("tool_a");
        session.register_tool("tool_a");
        session.register_tool("tool_a");

        assert_eq!(session.tools_registered.len(), 1);
    }
}
