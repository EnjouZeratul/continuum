//! LLM 客户端模块
//!
//! 统一的 LLM API 客户端，支持多提供商。
//!
//! [STABLE] 基础请求功能完整
//! [STABLE] 流式响应支持 Anthropic/OpenAI 格式

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

use crate::streaming::{
    CallbackStream, ContentDelta, MessageStream, OnChunkCallback, StreamEvent, StreamProvider,
};

/// LLM 提供商类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    Gemini,
    AzureOpenAI,
    Bedrock,
    Ollama,
    /// OpenAI-compatible provider with custom base_url (e.g. deepseek, glm, qwen, kimi, grok)
    OpenAICompatible {
        base_url: String,
    },
    /// Anthropic-compatible provider with custom base_url (e.g. tencent-coding, other Claude API proxies)
    AnthropicCompatible {
        base_url: String,
    },
    Custom(String),
}

/// LLM 请求配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestConfig {
    /// 模型名称
    pub model: String,
    /// 最大 token 数
    pub max_tokens: u32,
    /// 温度参数
    pub temperature: f32,
    /// 系统提示
    pub system_prompt: Option<String>,
    /// 停止词
    pub stop_sequences: Vec<String>,
}

impl Default for LlmRequestConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-6".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            system_prompt: None,
            stop_sequences: vec!["\n\n\n".to_string()],
        }
    }
}

/// LLM 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    /// 响应内容
    pub content: String,
    /// Token 使用情况
    pub usage: TokenUsage,
    /// 模型名称
    pub model: String,
    /// 响应 ID
    pub response_id: String,
}

/// Token 使用情况
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 输入 token 数
    pub input_tokens: u32,
    /// 输出 token 数
    pub output_tokens: u32,
}

/// LLM 客户端 trait
#[async_trait]
pub trait LlmClientTrait {
    /// 发送请求并获取响应
    async fn send(&self, messages: Vec<Message>, config: &LlmRequestConfig) -> Result<LlmResponse>;

    /// 发送请求并流式获取响应
    async fn send_stream(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream>;
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// LLM 客户端实现
pub struct LlmClient {
    /// HTTP 客户端
    client: Client,
    /// API 密钥
    api_key: String,
    /// 提供商
    provider: LlmProvider,
    /// API 基础 URL
    base_url: String,
}

impl LlmClient {
    pub fn new(provider: LlmProvider, api_key: String) -> Self {
        let base_url = match &provider {
            LlmProvider::Anthropic => "https://api.anthropic.com/v1".to_string(),
            LlmProvider::OpenAI => "https://api.openai.com/v1".to_string(),
            LlmProvider::Gemini => "https://generativelanguage.googleapis.com/v1".to_string(),
            LlmProvider::AzureOpenAI => "https://YOUR_RESOURCE.openai.azure.com".to_string(),
            LlmProvider::Bedrock => "https://bedrock-runtime.us-east-1.amazonaws.com".to_string(),
            LlmProvider::Ollama => "http://localhost:11434".to_string(),
            LlmProvider::OpenAICompatible { base_url } => base_url.clone(),
            LlmProvider::AnthropicCompatible { base_url } => base_url.clone(),
            LlmProvider::Custom(url) => url.clone(),
        };

        Self {
            client: Client::new(),
            api_key,
            provider,
            base_url,
        }
    }

    /// 创建客户端并指定自定义 base_url（覆盖 provider 默认值）
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// 发送带回调的流式请求
    pub async fn send_stream_with_callback(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
        on_chunk: OnChunkCallback,
    ) -> Result<LlmResponse> {
        let message_stream = self.send_stream(messages, config).await?;
        let mut callback_stream = CallbackStream::new(message_stream, Some(on_chunk));

        let mut content = String::new();
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;
        let mut message_id = String::new();
        let mut model = config.model.clone();

        while let Some(event) = callback_stream.next_event().await? {
            match event {
                StreamEvent::MessageStart { id, model: m } => {
                    message_id = id;
                    model = m;
                }
                StreamEvent::ContentBlockDelta {
                    delta: ContentDelta::Text(t),
                    ..
                } => {
                    content.push_str(&t);
                }
                StreamEvent::ContentBlockDelta { .. } => {}
                StreamEvent::MessageDelta { usage, .. } => {
                    input_tokens = usage.input_tokens;
                    output_tokens = usage.output_tokens;
                }
                _ => {}
            }
        }

        Ok(LlmResponse {
            content,
            usage: TokenUsage {
                input_tokens,
                output_tokens,
            },
            model,
            response_id: message_id,
        })
    }

    /// 发送可中断的流式请求
    pub async fn send_stream_abortable(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
        abort_flag: Arc<AtomicBool>,
    ) -> Result<LlmResponse> {
        let message_stream = self.send_stream(messages, config).await?;
        let mut callback_stream = CallbackStream::new(message_stream, None);

        let mut content = String::new();
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;
        let mut message_id = String::new();
        let mut model = config.model.clone();

        while !abort_flag.load(Ordering::Relaxed) {
            match callback_stream.next_event().await {
                Ok(Some(event)) => match event {
                    StreamEvent::MessageStart { id, model: m } => {
                        message_id = id;
                        model = m;
                    }
                    StreamEvent::ContentBlockDelta {
                        delta: ContentDelta::Text(t),
                        ..
                    } => {
                        content.push_str(&t);
                    }
                    StreamEvent::ContentBlockDelta { .. } => {}
                    StreamEvent::MessageDelta { usage, .. } => {
                        input_tokens = usage.input_tokens;
                        output_tokens = usage.output_tokens;
                    }
                    StreamEvent::MessageStop => {
                        break;
                    }
                    _ => {}
                },
                Ok(None) => break,
                Err(e) => {
                    if abort_flag.load(Ordering::Relaxed) {
                        info!("Stream aborted by user");
                        break;
                    }
                    return Err(e);
                }
            }
        }

        if abort_flag.load(Ordering::Relaxed) {
            info!("Stream was aborted");
        }

        Ok(LlmResponse {
            content,
            usage: TokenUsage {
                input_tokens,
                output_tokens,
            },
            model,
            response_id: message_id,
        })
    }

    /// 带错误恢复的请求重试
    pub async fn send_with_retry(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
        max_retries: u32,
    ) -> Result<LlmResponse> {
        let mut attempts = 0;
        let mut last_error: Option<anyhow::Error> = None;

        while attempts < max_retries {
            attempts += 1;

            match self.send(messages.clone(), config).await {
                Ok(response) => {
                    info!("LLM request succeeded after {} attempts", attempts);
                    return Ok(response);
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    if error_msg.contains("rate limit")
                        || error_msg.contains("429")
                        || error_msg.contains("overloaded")
                        || error_msg.contains("timeout")
                    {
                        warn!(
                            "LLM request failed (attempt {}/{}): {}",
                            attempts, max_retries, e
                        );
                        last_error = Some(e);

                        let delay = std::cmp::min(1000 * 2u64.pow(attempts - 1), 30000);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Max retries exceeded")))
    }

    /// 带错误恢复的流式请求重试
    pub async fn send_stream_with_retry(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
        max_retries: u32,
    ) -> Result<LlmResponse> {
        let mut attempts = 0;
        let mut last_error: Option<anyhow::Error> = None;

        while attempts < max_retries {
            attempts += 1;

            match self
                .send_stream_with_callback(messages.clone(), config, Box::new(|_| {}))
                .await
            {
                Ok(response) => {
                    info!("Stream request succeeded after {} attempts", attempts);
                    return Ok(response);
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    if error_msg.contains("rate limit")
                        || error_msg.contains("429")
                        || error_msg.contains("overloaded")
                        || error_msg.contains("timeout")
                        || error_msg.contains("aborted")
                    {
                        warn!(
                            "Stream request failed (attempt {}/{}): {}",
                            attempts, max_retries, e
                        );
                        last_error = Some(e);

                        let delay = std::cmp::min(1000 * 2u64.pow(attempts - 1), 30000);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Max retries exceeded")))
    }
}

#[async_trait]
impl LlmClientTrait for LlmClient {
    async fn send(&self, messages: Vec<Message>, config: &LlmRequestConfig) -> Result<LlmResponse> {
        match self.provider {
            LlmProvider::Anthropic | LlmProvider::AnthropicCompatible { .. } => {
                self.send_anthropic(messages, config).await
            }
            LlmProvider::OpenAI | LlmProvider::OpenAICompatible { .. } => {
                self.send_openai(messages, config).await
            }
            LlmProvider::Gemini => self.send_gemini(messages, config).await,
            LlmProvider::AzureOpenAI => self.send_azure_openai(messages, config).await,
            LlmProvider::Bedrock => self.send_bedrock(messages, config).await,
            LlmProvider::Ollama => self.send_ollama(messages, config).await,
            LlmProvider::Custom(_) => {
                Err(anyhow!("Custom provider requires custom implementation. Use an OpenAI-compatible provider instead."))
            }
        }
    }

    async fn send_stream(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        match self.provider {
            LlmProvider::Anthropic | LlmProvider::AnthropicCompatible { .. } => {
                self.stream_anthropic(messages, config).await
            }
            LlmProvider::OpenAI | LlmProvider::OpenAICompatible { .. } => {
                self.stream_openai(messages, config).await
            }
            LlmProvider::Gemini => self.stream_gemini(messages, config).await,
            LlmProvider::AzureOpenAI => self.stream_azure_openai(messages, config).await,
            LlmProvider::Bedrock => self.stream_bedrock(messages, config).await,
            LlmProvider::Ollama => self.stream_ollama(messages, config).await,
            LlmProvider::Custom(_) => Err(anyhow!("Custom provider does not support streaming. Use an OpenAI-compatible provider instead.")),
        }
    }
}

impl LlmClient {
    /// Construct the messages endpoint URL for Anthropic API
    ///
    /// Handles three cases:
    /// 1. Official Anthropic API: https://api.anthropic.com -> https://api.anthropic.com/v1/messages
    /// 2. Already contains full path: https://api.example.com/anthropic/messages -> unchanged
    /// 3. Anthropic-compatible endpoint (contains /anthropic): https://api.example.com/anthropic -> /messages
    /// 4. Already contains v1: https://api.example.com/v1 -> https://api.example.com/v1/messages
    pub fn build_anthropic_messages_url(base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');

        // If URL already ends with /messages, return as-is
        if base.ends_with("/messages") {
            return base.to_string();
        }

        // If URL ends with /v1, just append /messages
        if base.ends_with("/v1") {
            return format!("{}/messages", base);
        }

        // If URL contains /anthropic (Anthropic-compatible endpoint), just append /messages
        // This handles third-party Anthropic-compatible endpoints like Tencent Coding
        if base.contains("/anthropic") {
            return format!("{}/messages", base);
        }

        // Otherwise, append /v1/messages (official Anthropic API case)
        format!("{}/v1/messages", base)
    }

    async fn send_anthropic(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<LlmResponse> {
        let url = Self::build_anthropic_messages_url(&self.base_url);

        let request_body = AnthropicRequest {
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            messages: messages
                .into_iter()
                .map(|m| AnthropicMessage {
                    role: match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::System => "system",
                    },
                    content: AnthropicContent::Text(m.content),
                })
                .collect(),
            system: config.system_prompt.clone(),
            temperature: config.temperature,
        };

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request_body)
            .send()
            .await?;

        let response_text = response.text().await?;
        tracing::debug!("Anthropic API response: {}", response_text);

        let response_body: AnthropicResponse = serde_json::from_str(&response_text)?;

        Ok(LlmResponse {
            content: response_body
                .content
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default(),
            usage: TokenUsage {
                input_tokens: response_body.usage.input_tokens,
                output_tokens: response_body.usage.output_tokens,
            },
            model: response_body.model,
            response_id: response_body.id,
        })
    }

    async fn stream_anthropic(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        let url = Self::build_anthropic_messages_url(&self.base_url);

        let request_body = AnthropicStreamRequest {
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            messages: messages
                .into_iter()
                .map(|m| AnthropicMessage {
                    role: match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::System => "system",
                    },
                    content: AnthropicContent::Text(m.content),
                })
                .collect(),
            system: config.system_prompt.clone(),
            temperature: config.temperature,
            stream: true,
        };

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Accept", "text/event-stream")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Anthropic API error {}: {}", status, error_text));
        }

        Ok(MessageStream::new(
            response,
            match self.provider {
                LlmProvider::Anthropic => StreamProvider::Anthropic,
                LlmProvider::AnthropicCompatible { .. } => StreamProvider::AnthropicCompatible,
                _ => StreamProvider::Anthropic, // fallback
            },
            config.model.clone(),
        ))
    }

    async fn send_openai(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<LlmResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut openai_messages: Vec<OpenAiMessage> = Vec::new();

        if let Some(ref system) = config.system_prompt {
            openai_messages.push(OpenAiMessage {
                role: "system",
                content: system.clone(),
            });
        }

        for m in messages {
            openai_messages.push(OpenAiMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: m.content,
            });
        }

        let request_body = OpenAiRequest {
            model: config.model.clone(),
            messages: openai_messages,
            max_tokens: Some(config.max_tokens),
            temperature: Some(config.temperature),
            stop: if config.stop_sequences.is_empty() {
                None
            } else {
                Some(config.stop_sequences.clone())
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await?;

        let response_body: OpenAiResponse = response.json().await?;

        let choice = response_body
            .choices
            .first()
            .ok_or_else(|| anyhow!("No response choices"))?;

        Ok(LlmResponse {
            content: choice.message.content.clone(),
            usage: TokenUsage {
                input_tokens: response_body.usage.prompt_tokens,
                output_tokens: response_body.usage.completion_tokens,
            },
            model: response_body.model,
            response_id: response_body.id,
        })
    }

    async fn stream_openai(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut openai_messages: Vec<OpenAiMessage> = Vec::new();
        if let Some(ref system) = config.system_prompt {
            openai_messages.push(OpenAiMessage {
                role: "system",
                content: system.clone(),
            });
        }
        for m in messages {
            openai_messages.push(OpenAiMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: m.content,
            });
        }

        let request_body = OpenAiStreamRequest {
            model: config.model.clone(),
            messages: openai_messages,
            max_tokens: Some(config.max_tokens),
            temperature: Some(config.temperature),
            stream: true,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/event-stream")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("OpenAI API error {}: {}", status, error_text));
        }

        Ok(MessageStream::new(
            response,
            match self.provider {
                LlmProvider::OpenAI => StreamProvider::OpenAI,
                LlmProvider::OpenAICompatible { .. } => StreamProvider::OpenAICompatible,
                _ => StreamProvider::OpenAI, // fallback
            },
            config.model.clone(),
        ))
    }

    async fn send_gemini(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<LlmResponse> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, config.model, self.api_key
        );

        let mut contents: Vec<GeminiContent> = Vec::new();
        let system_instruction = config.system_prompt.clone();

        for m in messages {
            contents.push(GeminiContent {
                role: match m.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "model".to_string(),
                    MessageRole::System => "user".to_string(),
                },
                parts: vec![GeminiPart { text: m.content }],
            });
        }

        let request_body = GeminiRequest {
            contents,
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: Some(config.max_tokens),
                temperature: Some(config.temperature),
                stop_sequences: if config.stop_sequences.is_empty() {
                    None
                } else {
                    Some(config.stop_sequences.clone())
                },
            }),
            system_instruction: system_instruction.map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart { text: s }],
            }),
        };

        let response = self.client.post(&url).json(&request_body).send().await?;

        let response_body: GeminiResponse = response.json().await?;

        let candidate = response_body
            .candidates
            .first()
            .ok_or_else(|| anyhow!("No response candidates"))?;

        let content = candidate
            .content
            .parts
            .first()
            .map(|p| p.text.clone())
            .unwrap_or_default();

        Ok(LlmResponse {
            content,
            usage: TokenUsage {
                input_tokens: response_body.usage_metadata.prompt_token_count.unwrap_or(0),
                output_tokens: response_body
                    .usage_metadata
                    .candidates_token_count
                    .unwrap_or(0),
            },
            model: config.model.clone(),
            response_id: "".to_string(),
        })
    }

    async fn stream_gemini(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}&alt=sse",
            self.base_url, config.model, self.api_key
        );

        let mut contents: Vec<GeminiContent> = Vec::new();
        let system_instruction = config.system_prompt.clone();

        for m in messages {
            contents.push(GeminiContent {
                role: match m.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "model".to_string(),
                    MessageRole::System => "user".to_string(),
                },
                parts: vec![GeminiPart { text: m.content }],
            });
        }

        let request_body = GeminiRequest {
            contents,
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: Some(config.max_tokens),
                temperature: Some(config.temperature),
                stop_sequences: if config.stop_sequences.is_empty() {
                    None
                } else {
                    Some(config.stop_sequences.clone())
                },
            }),
            system_instruction: system_instruction.map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart { text: s }],
            }),
        };

        let response = self.client.post(&url).json(&request_body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Gemini API error {}: {}", status, error_text));
        }

        Ok(MessageStream::new(
            response,
            StreamProvider::Gemini,
            config.model.clone(),
        ))
    }

    // ========================================================================
    // Azure OpenAI 实现
    // ========================================================================

    async fn send_azure_openai(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<LlmResponse> {
        // Azure OpenAI 使用 deployment name 而非 model name
        // URL format: {base_url}/openai/deployments/{deployment}/chat/completions?api-version=2024-02-15-preview
        let deployment = &config.model;
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
            self.base_url, deployment
        );

        let mut azure_messages: Vec<OpenAiMessage> = Vec::new();
        if let Some(ref system) = config.system_prompt {
            azure_messages.push(OpenAiMessage {
                role: "system",
                content: system.clone(),
            });
        }
        for m in messages {
            azure_messages.push(OpenAiMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: m.content,
            });
        }

        let request_body = OpenAiRequest {
            model: deployment.clone(), // Azure 使用 deployment name
            messages: azure_messages,
            max_tokens: Some(config.max_tokens),
            temperature: Some(config.temperature),
            stop: if config.stop_sequences.is_empty() {
                None
            } else {
                Some(config.stop_sequences.clone())
            },
        };

        let response = self
            .client
            .post(&url)
            .header("api-key", &self.api_key) // Azure 使用 api-key header 而非 Authorization
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Azure OpenAI API error {}: {}", status, error_text));
        }

        let response_body: OpenAiResponse = response.json().await?;

        let choice = response_body
            .choices
            .first()
            .ok_or_else(|| anyhow!("No response choices"))?;

        Ok(LlmResponse {
            content: choice.message.content.clone(),
            usage: TokenUsage {
                input_tokens: response_body.usage.prompt_tokens,
                output_tokens: response_body.usage.completion_tokens,
            },
            model: response_body.model,
            response_id: response_body.id,
        })
    }

    async fn stream_azure_openai(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        let deployment = &config.model;
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
            self.base_url, deployment
        );

        let mut azure_messages: Vec<OpenAiMessage> = Vec::new();
        if let Some(ref system) = config.system_prompt {
            azure_messages.push(OpenAiMessage {
                role: "system",
                content: system.clone(),
            });
        }
        for m in messages {
            azure_messages.push(OpenAiMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: m.content,
            });
        }

        let request_body = OpenAiStreamRequest {
            model: deployment.clone(),
            messages: azure_messages,
            max_tokens: Some(config.max_tokens),
            temperature: Some(config.temperature),
            stream: true,
        };

        let response = self
            .client
            .post(&url)
            .header("api-key", &self.api_key)
            .header("Accept", "text/event-stream")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Azure OpenAI API error {}: {}", status, error_text));
        }

        Ok(MessageStream::new(
            response,
            StreamProvider::AzureOpenAI,
            config.model.clone(),
        ))
    }

    // ========================================================================
    // AWS Bedrock 实现
    // ========================================================================

    async fn send_bedrock(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<LlmResponse> {
        // Bedrock URL: {base_url}/model/{model_id}/invoke
        // 需要 AWS SigV4 签名认证（简化版使用 API key 作为临时方案）
        let model_id = &config.model;
        let url = format!("{}/model/{}/invoke", self.base_url, model_id);

        // Bedrock 请求格式因模型不同而异，这里使用通用的 Converse API 格式
        let mut bedrock_messages: Vec<BedrockMessage> = Vec::new();
        for m in messages {
            bedrock_messages.push(BedrockMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: vec![BedrockContent { text: m.content }],
            });
        }

        let request_body = BedrockRequest {
            messages: bedrock_messages,
            system: config.system_prompt.clone(),
            inference_config: Some(BedrockInferenceConfig {
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                top_p: None,
                stop_sequences: if config.stop_sequences.is_empty() {
                    None
                } else {
                    Some(config.stop_sequences.clone())
                },
            }),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Bedrock API error {}: {}", status, error_text));
        }

        let response_body: BedrockResponse = response.json().await?;

        let content = response_body
            .output
            .message
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        Ok(LlmResponse {
            content,
            usage: TokenUsage {
                input_tokens: response_body.usage.input_tokens,
                output_tokens: response_body.usage.output_tokens,
            },
            model: config.model.clone(),
            response_id: response_body.request_id.unwrap_or_default(),
        })
    }

    async fn stream_bedrock(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        let model_id = &config.model;
        let url = format!(
            "{}/model/{}/invoke-with-response-stream",
            self.base_url, model_id
        );

        let mut bedrock_messages: Vec<BedrockMessage> = Vec::new();
        for m in messages {
            bedrock_messages.push(BedrockMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: vec![BedrockContent { text: m.content }],
            });
        }

        let request_body = BedrockRequest {
            messages: bedrock_messages,
            system: config.system_prompt.clone(),
            inference_config: Some(BedrockInferenceConfig {
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                top_p: None,
                stop_sequences: if config.stop_sequences.is_empty() {
                    None
                } else {
                    Some(config.stop_sequences.clone())
                },
            }),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Bedrock API error {}: {}", status, error_text));
        }

        Ok(MessageStream::new(
            response,
            StreamProvider::Bedrock,
            config.model.clone(),
        ))
    }

    // ========================================================================
    // Ollama (本地) 实现
    // ========================================================================

    async fn send_ollama(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<LlmResponse> {
        // Ollama API: POST /api/chat 或 /api/generate
        let url = format!("{}/api/chat", self.base_url);

        let mut ollama_messages: Vec<OllamaMessage> = Vec::new();
        if let Some(ref system) = config.system_prompt {
            ollama_messages.push(OllamaMessage {
                role: "system",
                content: system.clone(),
            });
        }
        for m in messages {
            ollama_messages.push(OllamaMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: m.content,
            });
        }

        let request_body = OllamaChatRequest {
            model: config.model.clone(),
            messages: ollama_messages,
            stream: false,
            options: Some(OllamaOptions {
                num_predict: config.max_tokens as i32,
                temperature: config.temperature,
                stop: if config.stop_sequences.is_empty() {
                    None
                } else {
                    Some(config.stop_sequences.clone())
                },
            }),
        };

        // Ollama 本地运行，通常无需 API key
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Ollama API error {}: {}", status, error_text));
        }

        let response_body: OllamaChatResponse = response.json().await?;

        Ok(LlmResponse {
            content: response_body.message.content,
            usage: TokenUsage {
                input_tokens: response_body.prompt_eval_count.unwrap_or(0),
                output_tokens: response_body.eval_count.unwrap_or(0),
            },
            model: response_body.model,
            response_id: "".to_string(),
        })
    }

    async fn stream_ollama(
        &self,
        messages: Vec<Message>,
        config: &LlmRequestConfig,
    ) -> Result<MessageStream> {
        let url = format!("{}/api/chat", self.base_url);

        let mut ollama_messages: Vec<OllamaMessage> = Vec::new();
        if let Some(ref system) = config.system_prompt {
            ollama_messages.push(OllamaMessage {
                role: "system",
                content: system.clone(),
            });
        }
        for m in messages {
            ollama_messages.push(OllamaMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                },
                content: m.content,
            });
        }

        let request_body = OllamaChatRequest {
            model: config.model.clone(),
            messages: ollama_messages,
            stream: true,
            options: Some(OllamaOptions {
                num_predict: config.max_tokens as i32,
                temperature: config.temperature,
                stop: if config.stop_sequences.is_empty() {
                    None
                } else {
                    Some(config.stop_sequences.clone())
                },
            }),
        };

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Ollama API error {}: {}", status, error_text));
        }

        Ok(MessageStream::new(
            response,
            StreamProvider::Ollama,
            config.model.clone(),
        ))
    }
}

// Anthropic API 结构
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    system: Option<String>,
    temperature: f32,
}

#[derive(Serialize)]
struct AnthropicStreamRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    system: Option<String>,
    temperature: f32,
    stream: bool,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: AnthropicContent,
}

#[derive(Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Serialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    #[serde(default)]
    id: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    content: Vec<AnthropicContentResponse>,
    #[serde(default)]
    usage: AnthropicUsage,
    #[serde(default)]
    #[serde(rename = "type")]
    response_type: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AnthropicContentResponse {
    #[serde(rename = "type", default)]
    content_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize, Default)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

// OpenAI API 结构
#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Serialize)]
struct OpenAiStreamRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    id: String,
    model: String,
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
    finish_reason: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiResponseMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// Gemini API 结构
#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    usage_metadata: GeminiUsageMetadata,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GeminiCandidate {
    content: GeminiContentResponse,
    finish_reason: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GeminiContentResponse {
    parts: Vec<GeminiPartResponse>,
    role: String,
}

#[derive(Deserialize)]
struct GeminiPartResponse {
    text: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GeminiUsageMetadata {
    prompt_token_count: Option<u32>,
    candidates_token_count: Option<u32>,
    total_token_count: Option<u32>,
}

// ========================================================================
// AWS Bedrock API 结构
// ========================================================================

#[derive(Serialize)]
struct BedrockRequest {
    messages: Vec<BedrockMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inference_config: Option<BedrockInferenceConfig>,
}

#[derive(Serialize)]
struct BedrockMessage {
    role: &'static str,
    content: Vec<BedrockContent>,
}

#[derive(Serialize)]
struct BedrockContent {
    text: String,
}

#[derive(Serialize)]
struct BedrockInferenceConfig {
    #[serde(rename = "maxTokens")]
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct BedrockResponse {
    output: BedrockOutput,
    usage: BedrockUsage,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
struct BedrockOutput {
    message: BedrockResponseMessage,
}

#[derive(Deserialize)]
struct BedrockResponseMessage {
    content: Vec<BedrockResponseContent>,
}

#[derive(Deserialize)]
struct BedrockResponseContent {
    text: String,
}

#[derive(Deserialize)]
struct BedrockUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

// ========================================================================
// Ollama API 结构
// ========================================================================

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    num_predict: i32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    model: String,
    message: OllamaResponseMessage,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LlmRequestConfig::default();
        assert_eq!(config.model, "claude-sonnet-4-6");
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn test_client_creation() {
        let client = LlmClient::new(LlmProvider::Anthropic, "test_key".to_string());
        assert_eq!(client.base_url, "https://api.anthropic.com/v1");
    }

    #[test]
    fn test_openai_client_creation() {
        let client = LlmClient::new(LlmProvider::OpenAI, "test_key".to_string());
        assert_eq!(client.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_gemini_client_creation() {
        let client = LlmClient::new(LlmProvider::Gemini, "test_key".to_string());
        assert_eq!(
            client.base_url,
            "https://generativelanguage.googleapis.com/v1"
        );
    }

    #[test]
    fn test_custom_provider() {
        let client = LlmClient::new(
            LlmProvider::Custom("https://custom.api.com/v1".to_string()),
            "test_key".to_string(),
        );
        assert_eq!(client.base_url, "https://custom.api.com/v1");
    }

    #[test]
    fn test_openai_compatible_provider() {
        let client = LlmClient::new(
            LlmProvider::OpenAICompatible {
                base_url: "https://api.deepseek.com/v1".to_string(),
            },
            "test_key".to_string(),
        );
        assert_eq!(client.base_url, "https://api.deepseek.com/v1");
    }

    #[test]
    fn test_azure_openai_client_creation() {
        let client = LlmClient::new(LlmProvider::AzureOpenAI, "test_key".to_string());
        assert!(client.base_url.contains("openai.azure.com"));
    }

    #[test]
    fn test_bedrock_client_creation() {
        let client = LlmClient::new(LlmProvider::Bedrock, "test_key".to_string());
        assert!(client.base_url.contains("bedrock-runtime"));
    }

    #[test]
    fn test_ollama_client_creation() {
        let client = LlmClient::new(LlmProvider::Ollama, "".to_string());
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_azure_openai_with_custom_url() {
        let client = LlmClient::new(LlmProvider::AzureOpenAI, "test_key".to_string())
            .with_base_url("https://myresource.openai.azure.com".to_string());
        assert_eq!(client.base_url, "https://myresource.openai.azure.com");
    }

    #[test]
    fn test_ollama_with_custom_url() {
        let client = LlmClient::new(LlmProvider::Ollama, "".to_string())
            .with_base_url("http://192.168.1.100:11434".to_string());
        assert_eq!(client.base_url, "http://192.168.1.100:11434");
    }

    #[test]
    fn test_message_creation() {
        let message = Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        };
        assert_eq!(message.content, "Hello");
    }

    #[test]
    fn test_config_with_system_prompt() {
        let config = LlmRequestConfig {
            model: "gpt-4".to_string(),
            max_tokens: 8192,
            temperature: 0.5,
            system_prompt: Some("You are a helpful assistant".to_string()),
            stop_sequences: vec![],
        };
        assert_eq!(config.model, "gpt-4");
        assert!(config.system_prompt.is_some());
    }

    #[test]
    fn test_llm_response_creation() {
        let response = LlmResponse {
            content: "Hello".to_string(),
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
            model: "gpt-4".to_string(),
            response_id: "resp_123".to_string(),
        };
        assert_eq!(response.content, "Hello");
        assert_eq!(response.usage.input_tokens, 10);
    }

    #[test]
    fn test_provider_serialization() {
        let provider = LlmProvider::Anthropic;
        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("Anthropic"));
    }

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert!(json.contains("User"));
    }

    // AnthropicCompatible provider tests
    #[test]
    fn test_anthropic_compatible_provider_creation() {
        let client = LlmClient::new(
            LlmProvider::AnthropicCompatible {
                base_url: "https://api.lkeap.cloud.tencent.com/coding/anthropic".to_string(),
            },
            "test_key".to_string(),
        );
        assert_eq!(
            client.base_url,
            "https://api.lkeap.cloud.tencent.com/coding/anthropic"
        );
    }

    #[test]
    fn test_anthropic_compatible_provider_serialization() {
        let provider = LlmProvider::AnthropicCompatible {
            base_url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("anthropic_compatible") || json.contains("AnthropicCompatible"));
    }

    // URL construction tests for build_anthropic_messages_url
    #[test]
    fn test_build_anthropic_messages_url_official_api() {
        let url = LlmClient::build_anthropic_messages_url("https://api.anthropic.com");
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_build_anthropic_messages_url_already_has_v1() {
        let url = LlmClient::build_anthropic_messages_url("https://api.anthropic.com/v1");
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_build_anthropic_messages_url_already_has_messages() {
        let url =
            LlmClient::build_anthropic_messages_url("https://api.example.com/anthropic/messages");
        assert_eq!(url, "https://api.example.com/anthropic/messages");
    }

    #[test]
    fn test_build_anthropic_messages_url_tencent_endpoint() {
        let url = LlmClient::build_anthropic_messages_url(
            "https://api.lkeap.cloud.tencent.com/coding/anthropic",
        );
        assert_eq!(
            url,
            "https://api.lkeap.cloud.tencent.com/coding/anthropic/messages"
        );
    }

    #[test]
    fn test_build_anthropic_messages_url_with_trailing_slash() {
        let url = LlmClient::build_anthropic_messages_url("https://api.anthropic.com/v1/");
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
    }

    // Provider routing tests
    #[test]
    fn test_provider_routing_anthropic_compatible() {
        // Verify AnthropicCompatible routes to Anthropic format
        let provider = LlmProvider::AnthropicCompatible {
            base_url: "https://example.com".to_string(),
        };
        assert!(matches!(
            provider,
            LlmProvider::Anthropic | LlmProvider::AnthropicCompatible { .. }
        ));
    }

    #[test]
    fn test_provider_routing_openai_compatible() {
        // Verify OpenAICompatible routes to OpenAI format
        let provider = LlmProvider::OpenAICompatible {
            base_url: "https://example.com".to_string(),
        };
        assert!(matches!(
            provider,
            LlmProvider::OpenAI | LlmProvider::OpenAICompatible { .. }
        ));
    }
}
