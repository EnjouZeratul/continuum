//! LLM 提供商流式格式
//!
//! 定义各 LLM 提供商的流式响应格式和统一的事件抽象。

use serde::{Deserialize, Serialize};

/// 统一的流式事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 消息开始
    MessageStart { id: String, model: String },
    /// 内容块开始
    ContentBlockStart {
        index: u32,
        block_type: ContentBlockType,
    },
    /// 内容块增量
    ContentBlockDelta { index: u32, delta: ContentDelta },
    /// 内容块结束
    ContentBlockStop { index: u32 },
    /// 消息增量
    MessageDelta {
        stop_reason: Option<String>,
        usage: StreamUsage,
    },
    /// 消息结束
    MessageStop,
}

/// 内容块类型
#[derive(Debug, Clone)]
pub enum ContentBlockType {
    Text,
    Thinking,
    ToolUse { id: String, name: String },
}

/// 内容增量
#[derive(Debug, Clone)]
pub enum ContentDelta {
    Text(String),
    Thinking(String),
    ToolInput(String),
}

/// 流式用量
#[derive(Debug, Clone, Default)]
pub struct StreamUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// 流式提供商类型
#[derive(Debug, Clone, Copy)]
pub enum StreamProvider {
    Anthropic,
    OpenAI,
    Gemini,
    AzureOpenAI,
    Bedrock,
    Ollama,
}

/// 流式响应状态
#[derive(Debug)]
pub struct StreamState {
    model: String,
    message_started: bool,
    text_started: bool,
    text_finished: bool,
    thinking_started: bool,
    thinking_finished: bool,
    finished: bool,
    stop_reason: Option<String>,
    usage: Option<StreamUsage>,
    #[allow(dead_code)]
    tool_index_offset: u32,
    #[allow(dead_code)]
    tool_calls_count: u32,
}

impl StreamState {
    /// 创建新的流状态
    pub fn new(model: String) -> Self {
        Self {
            model,
            message_started: false,
            text_started: false,
            text_finished: false,
            thinking_started: false,
            thinking_finished: false,
            finished: false,
            stop_reason: None,
            usage: None,
            tool_index_offset: 0,
            tool_calls_count: 0,
        }
    }

    /// 处理 Anthropic 事件
    pub fn ingest_anthropic(&mut self, event: AnthropicStreamEvent) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                if !self.message_started {
                    self.message_started = true;
                    events.push(StreamEvent::MessageStart {
                        id: message.id,
                        model: message.model,
                    });
                }
            }
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let block_type = match content_block {
                    AnthropicContentBlock::Text { .. } => ContentBlockType::Text,
                    AnthropicContentBlock::Thinking { .. } => ContentBlockType::Thinking,
                    AnthropicContentBlock::ToolUse { id, name, .. } => {
                        ContentBlockType::ToolUse { id, name }
                    }
                };
                events.push(StreamEvent::ContentBlockStart { index, block_type });
            }
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                let content_delta = match delta {
                    AnthropicContentDelta::Text { text } => ContentDelta::Text(text),
                    AnthropicContentDelta::Thinking { thinking } => {
                        ContentDelta::Thinking(thinking)
                    }
                    AnthropicContentDelta::InputJson { partial_json } => {
                        ContentDelta::ToolInput(partial_json)
                    }
                };
                events.push(StreamEvent::ContentBlockDelta {
                    index,
                    delta: content_delta,
                });
            }
            AnthropicStreamEvent::ContentBlockStop { index } => {
                events.push(StreamEvent::ContentBlockStop { index });
            }
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                self.stop_reason = delta.stop_reason;
                self.usage = Some(StreamUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                });
                events.push(StreamEvent::MessageDelta {
                    stop_reason: self.stop_reason.clone(),
                    usage: self.usage.clone().unwrap_or_default(),
                });
            }
            AnthropicStreamEvent::MessageStop { .. } => {
                events.push(StreamEvent::MessageStop);
            }
        }

        events
    }

    /// 处理 OpenAI 事件
    pub fn ingest_openai(&mut self, chunk: OpenAiStreamChunk) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        if !self.message_started {
            self.message_started = true;
            events.push(StreamEvent::MessageStart {
                id: chunk.id.clone(),
                model: chunk.model.clone().unwrap_or_else(|| self.model.clone()),
            });
        }

        if let Some(usage) = chunk.usage {
            self.usage = Some(StreamUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            });
        }

        for choice in chunk.choices {
            // 处理 reasoning_content（思考内容）
            if let Some(reasoning) = choice.delta.reasoning_content.filter(|v| !v.is_empty()) {
                if !self.thinking_started {
                    self.thinking_started = true;
                    events.push(StreamEvent::ContentBlockStart {
                        index: 0,
                        block_type: ContentBlockType::Thinking,
                    });
                }
                events.push(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::Thinking(reasoning),
                });
            }

            // 处理常规内容
            if let Some(content) = choice.delta.content.filter(|v| !v.is_empty()) {
                // 如果之前有思考块，先关闭它
                if self.thinking_started && !self.thinking_finished {
                    self.thinking_finished = true;
                    events.push(StreamEvent::ContentBlockStop { index: 0 });
                }

                let text_index = if self.thinking_started { 1 } else { 0 };
                if !self.text_started {
                    self.text_started = true;
                    events.push(StreamEvent::ContentBlockStart {
                        index: text_index,
                        block_type: ContentBlockType::Text,
                    });
                }
                events.push(StreamEvent::ContentBlockDelta {
                    index: text_index,
                    delta: ContentDelta::Text(content),
                });
            }

            // 处理工具调用
            for (i, tool_call) in choice.delta.tool_calls.into_iter().enumerate() {
                let tool_index = (if self.thinking_started { 2 } else { 1 }) + i as u32;
                if let Some(name) = tool_call.function.name {
                    events.push(StreamEvent::ContentBlockStart {
                        index: tool_index,
                        block_type: ContentBlockType::ToolUse {
                            id: tool_call.id.unwrap_or_default(),
                            name,
                        },
                    });
                }
                if let Some(args) = tool_call.function.arguments {
                    events.push(StreamEvent::ContentBlockDelta {
                        index: tool_index,
                        delta: ContentDelta::ToolInput(args),
                    });
                }
            }

            // 处理结束原因
            if let Some(finish_reason) = choice.finish_reason {
                self.stop_reason = Some(normalize_openai_finish_reason(&finish_reason));
            }
        }

        events
    }

    /// 处理 Ollama 流式事件
    pub fn ingest_ollama(&mut self, chunk: OllamaStreamChunk) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        // 消息开始
        if !self.message_started {
            self.message_started = true;
            events.push(StreamEvent::MessageStart {
                id: "".to_string(),
                model: chunk.model.clone().unwrap_or_else(|| self.model.clone()),
            });
        }

        // 内容增量
        if let Some(message) = &chunk.message {
            if let Some(content) = &message.content {
                if !content.is_empty() {
                    if !self.text_started {
                        self.text_started = true;
                        events.push(StreamEvent::ContentBlockStart {
                            index: 0,
                            block_type: ContentBlockType::Text,
                        });
                    }
                    events.push(StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: ContentDelta::Text(content.clone()),
                    });
                }
            }
        }

        // 消息结束
        if chunk.done {
            // 更新用量
            if chunk.prompt_eval_count.is_some() || chunk.eval_count.is_some() {
                self.usage = Some(StreamUsage {
                    input_tokens: chunk.prompt_eval_count.unwrap_or(0),
                    output_tokens: chunk.eval_count.unwrap_or(0),
                });
            }

            // 关闭文本块
            if self.text_started && !self.text_finished {
                self.text_finished = true;
                events.push(StreamEvent::ContentBlockStop { index: 0 });
            }

            events.push(StreamEvent::MessageDelta {
                stop_reason: Some("stop".to_string()),
                usage: self.usage.clone().unwrap_or_default(),
            });
            events.push(StreamEvent::MessageStop);
        }

        events
    }

    /// 完成流处理
    pub fn finish(&mut self) -> Vec<StreamEvent> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;

        let mut events = Vec::new();

        // 关闭思考块
        if self.thinking_started && !self.thinking_finished {
            self.thinking_finished = true;
            events.push(StreamEvent::ContentBlockStop { index: 0 });
        }

        // 关闭文本块
        if self.text_started && !self.text_finished {
            self.text_finished = true;
            let text_index = if self.thinking_started { 1 } else { 0 };
            events.push(StreamEvent::ContentBlockStop { index: text_index });
        }

        // 发送消息增量
        if self.message_started {
            events.push(StreamEvent::MessageDelta {
                stop_reason: self
                    .stop_reason
                    .clone()
                    .or_else(|| Some("end_turn".to_string())),
                usage: self.usage.clone().unwrap_or_default(),
            });
            events.push(StreamEvent::MessageStop);
        }

        events
    }
}

fn normalize_openai_finish_reason(reason: &str) -> String {
    match reason {
        "stop" => "end_turn".to_string(),
        "tool_calls" => "tool_use".to_string(),
        other => other.to_string(),
    }
}

// ============================================================================
// Anthropic 流式格式
// ============================================================================

/// Anthropic 流式事件
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    /// 消息开始
    MessageStart { message: AnthropicMessageStart },
    /// 内容块开始
    ContentBlockStart {
        index: u32,
        content_block: AnthropicContentBlock,
    },
    /// 内容块增量
    ContentBlockDelta {
        index: u32,
        delta: AnthropicContentDelta,
    },
    /// 内容块结束
    ContentBlockStop { index: u32 },
    /// 消息增量
    MessageDelta {
        delta: AnthropicMessageDelta,
        #[serde(default)]
        usage: AnthropicStreamUsage,
    },
    /// 消息结束
    MessageStop {},
}

/// Anthropic 消息开始事件
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicMessageStart {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    pub model: String,
    #[serde(default)]
    pub content: Vec<AnthropicContentBlock>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    #[serde(default)]
    pub usage: AnthropicStreamUsage,
}

/// Anthropic 内容块
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Anthropic 内容增量
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentDelta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
}

/// Anthropic 消息增量
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicMessageDelta {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

/// Anthropic 流式用量
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AnthropicStreamUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
}

// ============================================================================
// OpenAI 流式格式
// ============================================================================

/// OpenAI 流式事件
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiStreamChunk {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    pub usage: Option<OpenAiStreamUsage>,
}

/// OpenAI 流式选择
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiStreamChoice {
    pub delta: OpenAiStreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// OpenAI 流式增量
#[derive(Debug, Default, Clone, Deserialize)]
pub struct OpenAiStreamDelta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<OpenAiStreamToolCall>,
}

/// OpenAI 流式工具调用
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiStreamToolCall {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: OpenAiStreamFunction,
}

/// OpenAI 流式函数
#[derive(Debug, Default, Clone, Deserialize)]
pub struct OpenAiStreamFunction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

/// OpenAI 流式用量
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiStreamUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
}

// ============================================================================
// Ollama 流式格式
// ============================================================================

/// Ollama 流式响应块
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaStreamChunk {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub message: Option<OllamaStreamMessage>,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
    #[serde(default)]
    pub eval_count: Option<u32>,
}

/// Ollama 流式消息
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaStreamMessage {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_state_handles_anthropic_events() {
        let mut state = StreamState::new("claude-sonnet-4-6".to_string());

        let start_event = AnthropicStreamEvent::MessageStart {
            message: AnthropicMessageStart {
                id: "msg_123".to_string(),
                kind: "message".to_string(),
                role: "assistant".to_string(),
                model: "claude-sonnet-4-6".to_string(),
                content: vec![],
                stop_reason: None,
                stop_sequence: None,
                usage: AnthropicStreamUsage::default(),
            },
        };

        let events = state.ingest_anthropic(start_event);
        assert!(matches!(events[0], StreamEvent::MessageStart { .. }));
    }

    #[test]
    fn stream_state_handles_openai_events() {
        let mut state = StreamState::new("gpt-4o".to_string());

        let chunk = OpenAiStreamChunk {
            id: "chatcmpl_123".to_string(),
            model: Some("gpt-4o".to_string()),
            choices: vec![OpenAiStreamChoice {
                delta: OpenAiStreamDelta {
                    content: Some("Hello".to_string()),
                    ..Default::default()
                },
                finish_reason: None,
            }],
            usage: None,
        };

        let events = state.ingest_openai(chunk);
        assert!(matches!(events[0], StreamEvent::MessageStart { .. }));
        assert!(matches!(events[1], StreamEvent::ContentBlockStart { .. }));
    }
}
