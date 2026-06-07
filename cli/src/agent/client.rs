//! Agent 客户端实现
//!
//! 连接真实 LLM API 进行对话。

use anyhow::Result;
use sh_layer4::sh_layer3::sh_layer2::sh_layer1::{
    config_manager::ConfigManager,
    llm_client::{LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, Message, MessageRole},
    streaming::{ContentDelta, StreamEvent as LlmStreamEvent},
};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Agent 状态
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    /// 空闲状态
    Idle,
    /// 正在处理
    Processing,
    /// 错误状态
    Error,
}

/// 流式事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 开始
    Start,
    /// 响应块
    Chunk(String),
    /// 完成
    Done,
    /// 错误
    Error(String),
}

/// 聊天消息
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// 角色
    pub role: String,
    /// 内容
    pub content: String,
}

/// Agent 错误类型
#[derive(Debug, Clone)]
pub enum AgentError {
    /// 配置错误（无API密钥等）
    Config(String),
    /// API 调用错误
    Api(String),
    /// 网络错误
    Network(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AgentError::Api(msg) => write!(f, "API error: {}", msg),
            AgentError::Network(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for AgentError {}

/// Token 使用统计
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// 输入 token 数
    pub input_tokens: u64,
    /// 输出 token 数
    pub output_tokens: u64,
    /// 总 token 数
    pub total_tokens: u64,
}

impl TokenUsage {
    /// 创建新的 token 使用统计
    pub fn new(input: u64, output: u64) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        }
    }

    /// 累加使用量
    pub fn add(&mut self, input: u64, output: u64) {
        self.input_tokens += input;
        self.output_tokens += output;
        self.total_tokens += input + output;
    }

    /// 格式化为显示字符串
    pub fn format_report(&self) -> String {
        format!(
            "Token Usage Statistics\n\
            ======================\n\
            Input Tokens:  {:>10}\n\
            Output Tokens: {:>10}\n\
            Total Tokens:  {:>10}\n",
            self.input_tokens, self.output_tokens, self.total_tokens
        )
    }
}

/// Agent 客户端
///
/// 提供真实的 LLM Agent 功能，连接真实的 API。
pub struct AgentClient {
    /// 配置管理器
    config: Arc<RwLock<ConfigManager>>,
    /// LLM 客户端
    llm_client: Arc<RwLock<Option<LlmClient>>>,
    /// 当前状态
    state: Arc<RwLock<AgentState>>,
    /// 消息历史
    message_history: Arc<RwLock<Vec<ChatMessage>>>,
    /// 当前提供商名称
    current_provider: Arc<RwLock<String>>,
    /// Token 使用统计
    token_usage: Arc<RwLock<TokenUsage>>,
}

impl AgentClient {
    /// 创建新的 Agent 客户端
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(ConfigManager::new())),
            llm_client: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(AgentState::Idle)),
            message_history: Arc::new(RwLock::new(Vec::new())),
            current_provider: Arc::new(RwLock::new(String::new())),
            token_usage: Arc::new(RwLock::new(TokenUsage::default())),
        }
    }

    /// 从配置文件初始化
    pub async fn init_from_config(&self) -> Result<(), AgentError> {
        // 加载完整配置
        let config = ConfigManager::load_full()
            .await
            .map_err(|e| AgentError::Config(format!("Failed to load config: {}", e)))?;

        // 解析环境变量引用
        let mut config = config;
        config.resolve_env_refs();

        // 检查是否有配置的提供商
        if config.providers.is_empty() {
            return Err(AgentError::Config(
                "No providers configured. Use 'continuum config add-provider' or set environment variables.".to_string()
            ));
        }

        // 获取当前提供商
        let provider_name = config.active_provider.clone();
        let provider_config = config.providers.get(&provider_name).cloned();

        if provider_config.is_none() {
            return Err(AgentError::Config(format!(
                "Active provider '{}' not found in configuration",
                provider_name
            )));
        }

        let provider_config = provider_config.unwrap();

        // 检查 API 密钥
        if provider_config.api_key.is_empty() {
            return Err(AgentError::Config(
                format!("API key not set for provider '{}'. Use 'continuum config set provider.{}.api_key YOUR_KEY' or set environment variable.",
                    provider_name, provider_name)
            ));
        }

        // 创建 LLM 客户端
        let llm_provider = Self::map_provider(&provider_name, &provider_config.base_url);
        let llm_client = LlmClient::new(llm_provider, provider_config.api_key.clone());

        // 更新状态
        {
            let mut cfg = self.config.write().await;
            *cfg = config;
        }
        {
            let mut client = self.llm_client.write().await;
            *client = Some(llm_client);
        }
        {
            let mut p = self.current_provider.write().await;
            *p = provider_name.clone();
        }

        tracing::info!("AgentClient initialized with provider: {}", provider_name);
        Ok(())
    }

    /// 映射提供商名称到 LlmProvider
    fn map_provider(name: &str, base_url: &str) -> LlmProvider {
        match name {
            "anthropic" => LlmProvider::Anthropic,
            "openai" => LlmProvider::OpenAI,
            "gemini" => LlmProvider::Gemini,
            // OpenAI-compatible providers: reuse OpenAI chat/completions protocol
            "deepseek" => LlmProvider::OpenAICompatible {
                base_url: if base_url.is_empty() {
                    "https://api.deepseek.com/v1".to_string()
                } else {
                    base_url.to_string()
                },
            },
            "glm" => LlmProvider::OpenAICompatible {
                base_url: if base_url.is_empty() {
                    "https://open.bigmodel.cn/api/paas/v4".to_string()
                } else {
                    base_url.to_string()
                },
            },
            "qwen" => LlmProvider::OpenAICompatible {
                base_url: if base_url.is_empty() {
                    "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()
                } else {
                    base_url.to_string()
                },
            },
            "kimi" | "moonshot" => LlmProvider::OpenAICompatible {
                base_url: if base_url.is_empty() {
                    "https://api.moonshot.cn/v1".to_string()
                } else {
                    base_url.to_string()
                },
            },
            "grok" => LlmProvider::OpenAICompatible {
                base_url: if base_url.is_empty() {
                    "https://api.x.ai/v1".to_string()
                } else {
                    base_url.to_string()
                },
            },
            _ => LlmProvider::Custom(base_url.to_string()),
        }
    }

    /// 发送消息并获取响应
    pub async fn send_message(&self, user_message: &str) -> Result<String, AgentError> {
        // 检查客户端是否初始化
        let client_guard = self.llm_client.read().await;
        if client_guard.is_none() {
            return Err(AgentError::Config(
                "Agent not initialized. Call init_from_config() first.".to_string(),
            ));
        }

        // 设置状态为处理中
        {
            let mut state = self.state.write().await;
            *state = AgentState::Processing;
        }

        // 添加用户消息到历史
        {
            let mut history = self.message_history.write().await;
            history.push(ChatMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            });
        }

        // 构建请求
        let config = self.config.read().await;
        let provider_config = config
            .current()
            .map_err(|e| AgentError::Config(e.to_string()))?;

        let request_config = LlmRequestConfig {
            model: provider_config.model.clone(),
            max_tokens: provider_config.default_max_tokens,
            temperature: provider_config.default_temperature,
            system_prompt: Some("You are a helpful AI assistant running in Continuum terminal. Be concise and helpful.".to_string()),
            stop_sequences: vec![],
        };

        // 构建消息历史
        let history = self.message_history.read().await;
        let messages: Vec<Message> = history
            .iter()
            .map(|m| Message {
                role: if m.role == "user" {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: m.content.clone(),
            })
            .collect();

        // 发送请求
        let client = client_guard.as_ref().unwrap();
        let result = client.send(messages, &request_config).await;

        // 处理响应
        match result {
            Ok(response) => {
                // 添加助手响应到历史
                {
                    let mut history = self.message_history.write().await;
                    history.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: response.content.clone(),
                    });
                }

                // 设置状态为空闲
                {
                    let mut state = self.state.write().await;
                    *state = AgentState::Idle;
                }

                // 更新 token 使用统计
                {
                    let mut usage = self.token_usage.write().await;
                    usage.add(
                        response.usage.input_tokens as u64,
                        response.usage.output_tokens as u64,
                    );
                }

                tracing::debug!(
                    "Agent response: {} tokens used",
                    response.usage.input_tokens + response.usage.output_tokens
                );
                Ok(response.content)
            }
            Err(e) => {
                // 设置状态为错误
                {
                    let mut state = self.state.write().await;
                    *state = AgentState::Error;
                }

                // 分类错误类型
                let error_msg = e.to_string();
                if error_msg.contains("connection") || error_msg.contains("network") {
                    Err(AgentError::Network(error_msg))
                } else {
                    Err(AgentError::Api(error_msg))
                }
            }
        }
    }

    /// 获取当前状态
    pub async fn state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    /// 获取当前提供商名称
    pub async fn current_provider(&self) -> String {
        self.current_provider.read().await.clone()
    }

    /// 获取消息历史
    pub async fn message_history(&self) -> Vec<ChatMessage> {
        self.message_history.read().await.clone()
    }

    /// 清空消息历史
    pub async fn clear_history(&self) {
        let mut history = self.message_history.write().await;
        history.clear();
    }

    /// 获取配置信息
    pub async fn config_info(&self) -> String {
        let config = self.config.read().await;
        let provider = self.current_provider.read().await.clone();

        if let Some(provider_config) = config.providers.get(&provider) {
            format!(
                "Provider: {} | Model: {} | MaxTokens: {}",
                provider, provider_config.model, provider_config.default_max_tokens
            )
        } else {
            format!("Provider: {} (not configured)", provider)
        }
    }

    /// 检查是否已初始化
    pub async fn is_initialized(&self) -> bool {
        self.llm_client.read().await.is_some()
    }

    /// 切换模型
    ///
    /// 更新当前提供商的模型配置
    pub async fn set_model(&self, model_name: &str) -> Result<(), AgentError> {
        let provider_name = self.current_provider.read().await.clone();

        // 更新配置中的模型
        {
            let mut config = self.config.write().await;
            if let Some(provider_config) = config.providers.get_mut(&provider_name) {
                provider_config.model = model_name.to_string();
                tracing::info!(
                    "Model switched to: {} for provider: {}",
                    model_name,
                    provider_name
                );
            } else {
                return Err(AgentError::Config(format!(
                    "Provider '{}' not found in configuration",
                    provider_name
                )));
            }
        }

        Ok(())
    }

    /// 获取当前模型名称
    pub async fn current_model(&self) -> Option<String> {
        let config = self.config.read().await;
        let provider_name = self.current_provider.read().await.clone();

        config
            .providers
            .get(&provider_name)
            .map(|p| p.model.clone())
    }

    /// 获取 token 使用统计
    pub async fn get_token_usage(&self) -> TokenUsage {
        self.token_usage.read().await.clone()
    }

    /// 重置 token 使用统计
    pub async fn reset_token_usage(&self) {
        let mut usage = self.token_usage.write().await;
        *usage = TokenUsage::default();
    }

    /// 列出可用模型
    pub async fn list_available_models(&self) -> Vec<String> {
        let provider_name = self.current_provider.read().await.clone();

        // 根据提供商返回可用模型列表
        match provider_name.as_str() {
            "anthropic" => vec![
                "claude-opus-4-8".to_string(),
                "claude-opus-4-7".to_string(),
                "claude-opus-4-6".to_string(),
                "claude-opus-4-5".to_string(),
                "claude-sonnet-4-6".to_string(),
                "claude-sonnet-4-5".to_string(),
                "claude-haiku-4-5".to_string(),
            ],
            "openai" => vec![
                "gpt-5.5".to_string(),
                "gpt-5.4".to_string(),
                "gpt-5.2".to_string(),
                "gpt-5.1".to_string(),
                "gpt-5".to_string(),
                "o3-mini".to_string(),
                "o1".to_string(),
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
            ],
            "google" | "gemini" => vec![
                "gemini-3.1-pro-preview".to_string(),
                "gemini-3.5-flash".to_string(),
                "gemini-3.0-pro".to_string(),
                "gemini-3.0-flash".to_string(),
                "gemini-2.5-pro".to_string(),
                "gemini-2.5-flash".to_string(),
            ],
            "deepseek" => vec![
                "deepseek-v4-pro".to_string(),
                "deepseek-v4-flash".to_string(),
                "deepseek-v3.2".to_string(),
                "deepseek-v3.1-terminus".to_string(),
                "deepseek-v3".to_string(),
                "deepseek-chat".to_string(),
                "deepseek-reasoner".to_string(),
            ],
            "glm" => vec![
                "glm-5.1".to_string(),
                "glm-5".to_string(),
                "glm-4.7".to_string(),
                "glm-4.6".to_string(),
            ],
            "kimi" | "moonshot" => vec![
                "kimi-k2.6".to_string(),
                "kimi-k2-thinking".to_string(),
                "kimi-k2.5".to_string(),
            ],
            "qwen" => vec![
                "qwen3.7-max".to_string(),
                "qwen3.6-plus".to_string(),
                "qwen3.5-27B".to_string(),
            ],
            "grok" => vec!["grok-4-heavy".to_string(), "grok-4".to_string()],
            _ => vec![],
        }
    }

    /// 切换提供商
    ///
    /// 切换到指定的提供商，需要提供商已配置
    pub async fn set_provider(&self, provider_name: &str) -> Result<(), AgentError> {
        // 检查提供商是否存在
        {
            let config = self.config.read().await;
            if !config.providers.contains_key(provider_name) {
                return Err(AgentError::Config(format!(
                    "Provider '{}' not found. Use 'continuum config add-provider {}' first.",
                    provider_name, provider_name
                )));
            }
        }

        // 更新当前提供商
        {
            let mut p = self.current_provider.write().await;
            *p = provider_name.to_string();
        }

        // 更新配置的 active_provider
        {
            let mut config = self.config.write().await;
            config.active_provider = provider_name.to_string();
        }

        // 重新创建 LLM 客户端
        {
            let config = self.config.read().await;
            let provider_config = config.providers.get(provider_name).cloned();

            if let Some(pc) = provider_config {
                if pc.api_key.is_empty() {
                    return Err(AgentError::Config(format!(
                        "API key not set for provider '{}'. Use 'continuum config set provider.{}.api_key YOUR_KEY'",
                        provider_name, provider_name
                    )));
                }

                let llm_provider = Self::map_provider(provider_name, &pc.base_url);
                let llm_client = LlmClient::new(llm_provider, pc.api_key.clone());

                let mut client = self.llm_client.write().await;
                *client = Some(llm_client);

                tracing::info!("Provider switched to: {}", provider_name);
            }
        }

        Ok(())
    }

    /// 获取当前提供商名称
    pub async fn current_provider_name(&self) -> String {
        self.current_provider.read().await.clone()
    }

    /// 列出所有已配置的提供商
    pub async fn list_providers(&self) -> Vec<String> {
        let config = self.config.read().await;
        config.providers.keys().cloned().collect()
    }

    /// 列出可用工具
    ///
    /// 返回所有内置工具的名称和描述
    pub fn list_tools(&self) -> Vec<(String, String, String)> {
        use sh_layer4::sh_layer3::tool_executor::DefaultToolExecutor;
        use sh_layer4::sh_layer3::ToolExecutor;

        let executor = DefaultToolExecutor::new();
        let tools = executor.list_tools();

        tools
            .iter()
            .map(|meta| {
                let category = format!("{:?}", meta.category).to_lowercase();
                (meta.name.clone(), meta.description.clone(), category)
            })
            .collect()
    }

    /// 发送消息并流式获取响应
    ///
    /// 使用真实的 SSE 流式响应，通过 MessageStream 处理。
    /// 返回一个接收器，通过该接收器可以接收流式响应块
    pub async fn send_message_stream(
        &self,
        user_message: &str,
    ) -> Result<mpsc::Receiver<StreamEvent>, AgentError> {
        // 检查客户端是否初始化
        let client_guard = self.llm_client.read().await;
        if client_guard.is_none() {
            return Err(AgentError::Config(
                "Agent not initialized. Call init_from_config() first.".to_string(),
            ));
        }

        // 添加用户消息到历史
        {
            let mut history = self.message_history.write().await;
            history.push(ChatMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            });
        }

        // 构建请求配置
        let config = self.config.read().await;
        let provider_config = config
            .current()
            .map_err(|e| AgentError::Config(e.to_string()))?;

        let request_config = LlmRequestConfig {
            model: provider_config.model.clone(),
            max_tokens: provider_config.default_max_tokens,
            temperature: provider_config.default_temperature,
            system_prompt: Some(
                "You are a helpful AI assistant running in Continuum terminal. Be concise and helpful."
                    .to_string(),
            ),
            stop_sequences: vec![],
        };

        // 构建消息历史
        let history = self.message_history.read().await;
        let messages: Vec<Message> = history
            .iter()
            .map(|m| Message {
                role: if m.role == "user" {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: m.content.clone(),
            })
            .collect();

        // 创建通道
        let (tx, rx) = mpsc::channel(64);

        // 克隆必要的数据
        let llm_client = self.llm_client.clone();
        let message_history = self.message_history.clone();
        let state = self.state.clone();
        let messages_clone = messages;
        let request_config_clone = request_config;

        // 设置状态为处理中
        {
            let mut s = state.write().await;
            *s = AgentState::Processing;
        }

        // 发送开始事件
        let _ = tx.send(StreamEvent::Start).await;

        // 在后台任务中处理流式 API 调用
        tokio::spawn(async move {
            // 读取客户端
            let client_guard = llm_client.read().await;
            let client = match client_guard.as_ref() {
                Some(c) => c,
                None => {
                    let _ = tx
                        .send(StreamEvent::Error("Agent not initialized".to_string()))
                        .await;
                    let _ = tx.send(StreamEvent::Done).await;
                    let mut s = state.write().await;
                    *s = AgentState::Idle;
                    return;
                }
            };

            // 发送流式请求
            let stream_result = client
                .send_stream(messages_clone, &request_config_clone)
                .await;

            match stream_result {
                Ok(mut message_stream) => {
                    let mut full_content = String::new();
                    let mut message_id = String::new();
                    let mut model_name = request_config_clone.model.clone();

                    // 处理流式事件
                    loop {
                        match message_stream.next_event().await {
                            Ok(Some(event)) => {
                                match event {
                                    LlmStreamEvent::MessageStart { id, model } => {
                                        message_id = id;
                                        model_name = model;
                                    }
                                    LlmStreamEvent::ContentBlockDelta { delta, .. } => {
                                        match delta {
                                            ContentDelta::Text(text) => {
                                                full_content.push_str(&text);
                                                if tx.send(StreamEvent::Chunk(text)).await.is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            ContentDelta::Thinking(thinking) => {
                                                // 可选：发送思考内容作为特殊事件
                                                if tx
                                                    .send(StreamEvent::Chunk(format!(
                                                        "[思考] {} ",
                                                        thinking
                                                    )))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            ContentDelta::ToolInput(input) => {
                                                // 工具调用输入
                                                if tx
                                                    .send(StreamEvent::Chunk(format!(
                                                        "[工具] {} ",
                                                        input
                                                    )))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    LlmStreamEvent::MessageDelta { stop_reason, .. } => {
                                        // 消息增量，记录结束原因
                                        tracing::debug!("Stream finished: {:?}", stop_reason);
                                    }
                                    LlmStreamEvent::MessageStop => {
                                        // 消息结束
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            Ok(None) => {
                                // 流结束
                                break;
                            }
                            Err(e) => {
                                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
                                break;
                            }
                        }
                    }

                    // 添加助手响应到历史
                    if !full_content.is_empty() {
                        let mut history = message_history.write().await;
                        history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: full_content,
                        });
                    }

                    tracing::debug!(
                        "Stream completed: message_id={}, model={}",
                        message_id,
                        model_name
                    );
                }
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(e.to_string())).await;
                }
            }

            let _ = tx.send(StreamEvent::Done).await;

            // 设置状态为空闲
            let mut s = state.write().await;
            *s = AgentState::Idle;
        });

        Ok(rx)
    }

    /// 发送消息并使用回调处理流式响应
    ///
    /// 提供 on_chunk 回调函数，实时处理每个响应块
    pub async fn send_message_with_callback<F>(
        &self,
        user_message: &str,
        mut on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send,
    {
        let mut receiver = self.send_message_stream(user_message).await?;

        let mut full_response = String::new();

        while let Some(event) = receiver.recv().await {
            match event {
                StreamEvent::Start => {
                    // 流开始
                }
                StreamEvent::Chunk(text) => {
                    on_chunk(&text);
                    full_response.push_str(&text);
                }
                StreamEvent::Done => {
                    break;
                }
                StreamEvent::Error(msg) => {
                    return Err(AgentError::Api(msg));
                }
            }
        }

        Ok(full_response)
    }
}

impl Default for AgentClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_client_creation() {
        let _client = AgentClient::new();
    }

    #[test]
    fn test_agent_error_display() {
        let error = AgentError::Config("test error".to_string());
        assert!(error.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_chat_message_creation() {
        let message = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "Hello");
    }

    #[tokio::test]
    async fn test_agent_state_initial() {
        let client = AgentClient::new();
        let state = client.state().await;
        assert_eq!(state, AgentState::Idle);
    }

    #[tokio::test]
    async fn test_message_history_empty() {
        let client = AgentClient::new();
        let history = client.message_history().await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_clear_history() {
        let client = AgentClient::new();
        client.clear_history().await;
        let history = client.message_history().await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_list_available_models_anthropic() {
        let client = AgentClient::new();
        // 设置提供商为 anthropic
        {
            let mut provider = client.current_provider.write().await;
            *provider = "anthropic".to_string();
        }
        let models = client.list_available_models().await;
        assert!(!models.is_empty());
        assert!(models.contains(&"claude-sonnet-4-6".to_string()));
    }

    #[tokio::test]
    async fn test_list_available_models_openai() {
        let client = AgentClient::new();
        // 设置提供商为 openai
        {
            let mut provider = client.current_provider.write().await;
            *provider = "openai".to_string();
        }
        let models = client.list_available_models().await;
        assert!(!models.is_empty());
        assert!(models.contains(&"gpt-5.5".to_string()));
    }

    #[tokio::test]
    async fn test_list_available_models_unknown() {
        let client = AgentClient::new();
        // 设置提供商为未知
        {
            let mut provider = client.current_provider.write().await;
            *provider = "unknown".to_string();
        }
        let models = client.list_available_models().await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_current_model_none_when_not_initialized() {
        let client = AgentClient::new();
        let model = client.current_model().await;
        assert!(model.is_none());
    }
}
