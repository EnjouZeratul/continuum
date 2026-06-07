//! 流式输出处理模块
//!
//! 提供 CLI 层的流式响应输出能力，集成 Layer 1 的流式处理。

use anyhow::Result;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::debug;

/// 流式输出配置
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// 是否显示思考过程
    pub show_thinking: bool,
    /// 是否显示工具调用
    pub show_tool_calls: bool,
    /// 是否使用颜色输出
    pub colorize: bool,
    /// 刷新间隔（毫秒）
    pub flush_interval_ms: u64,
    /// 是否显示 token 使用统计
    pub show_usage: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            show_thinking: false,
            show_tool_calls: false,
            colorize: true,
            flush_interval_ms: 50,
            show_usage: true,
        }
    }
}

/// 流式输出事件
#[derive(Debug, Clone)]
pub enum OutputEvent {
    /// 流开始
    Start { model: String },
    /// 文本块
    TextChunk(String),
    /// 思考块
    ThinkingChunk(String),
    /// 工具调用块
    ToolCallChunk { name: String, input: String },
    /// 流结束
    Done { usage: Option<UsageStats> },
    /// 错误
    Error(String),
}

/// Token 使用统计
#[derive(Debug, Clone, Default)]
pub struct UsageStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// 流式输出处理器
pub struct StreamingOutput {
    config: StreamConfig,
    abort_flag: Arc<AtomicBool>,
}

impl StreamingOutput {
    /// 创建新的流式输出处理器
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            abort_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 创建默认配置的处理器
    pub fn default_config() -> Self {
        Self::new(StreamConfig::default())
    }

    /// 获取中断标志的克隆
    pub fn abort_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort_flag)
    }

    /// 请求中断流式输出
    pub fn abort(&self) {
        self.abort_flag.store(true, Ordering::Relaxed);
    }

    /// 重置中断标志
    pub fn reset(&self) {
        self.abort_flag.store(false, Ordering::Relaxed);
    }

    /// 检查是否已中断
    pub fn is_aborted(&self) -> bool {
        self.abort_flag.load(Ordering::Relaxed)
    }

    /// 输出文本块（同步方式）
    pub fn output(&self, text: &str) -> Result<()> {
        let mut stdout = stdout();
        write!(stdout, "{}", text)?;
        stdout.flush()?;
        Ok(())
    }

    /// 输出带颜色的文本
    pub fn output_colored(&self, text: &str, color: OutputColor) -> Result<()> {
        if !self.config.colorize {
            return self.output(text);
        }

        let color_code = match color {
            OutputColor::Default => "\x1b[0m",
            OutputColor::Gray => "\x1b[90m",
            OutputColor::Green => "\x1b[32m",
            OutputColor::Yellow => "\x1b[33m",
            OutputColor::Blue => "\x1b[34m",
            OutputColor::Cyan => "\x1b[36m",
            OutputColor::Magenta => "\x1b[35m",
            OutputColor::Red => "\x1b[31m",
        };

        let mut stdout = stdout();
        write!(stdout, "{}{}\x1b[0m", color_code, text)?;
        stdout.flush()?;
        Ok(())
    }

    /// 处理流式事件（异步方式）
    ///
    /// 从接收器读取事件并实时输出
    pub async fn process_stream(
        &self,
        mut receiver: mpsc::Receiver<OutputEvent>,
    ) -> Result<String> {
        let mut full_content = String::new();
        let mut stdout = stdout();

        while let Some(event) = receiver.recv().await {
            if self.is_aborted() {
                debug!("Stream output aborted");
                break;
            }

            match event {
                OutputEvent::Start { model } => {
                    if self.config.colorize {
                        write!(stdout, "\x1b[90m[{}]\x1b[0m ", model)?;
                    }
                    stdout.flush()?;
                }
                OutputEvent::TextChunk(text) => {
                    full_content.push_str(&text);
                    write!(stdout, "{}", text)?;
                    stdout.flush()?;
                }
                OutputEvent::ThinkingChunk(thinking) => {
                    if self.config.show_thinking {
                        if self.config.colorize {
                            write!(stdout, "\x1b[90m[思考] {}\x1b[0m", thinking)?;
                        } else {
                            write!(stdout, "[思考] {}", thinking)?;
                        }
                        stdout.flush()?;
                    }
                }
                OutputEvent::ToolCallChunk { name, input } => {
                    if self.config.show_tool_calls {
                        if self.config.colorize {
                            write!(stdout, "\n\x1b[36m[工具: {}]\x1b[0m {}\n", name, input)?;
                        } else {
                            write!(stdout, "\n[工具: {}] {}\n", name, input)?;
                        }
                        stdout.flush()?;
                    }
                }
                OutputEvent::Done { usage } => {
                    if self.config.show_usage {
                        if let Some(stats) = usage {
                            writeln!(
                                stdout,
                                "\n\x1b[90m--- 输入: {} | 输出: {} ---\x1b[0m",
                                stats.input_tokens, stats.output_tokens
                            )?;
                        }
                    }
                    stdout.flush()?;
                    break;
                }
                OutputEvent::Error(msg) => {
                    if self.config.colorize {
                        write!(stdout, "\n\x1b[31m错误: {}\x1b[0m\n", msg)?;
                    } else {
                        write!(stdout, "\n错误: {}\n", msg)?;
                    }
                    stdout.flush()?;
                    return Err(anyhow::anyhow!("Stream error: {}", msg));
                }
            }
        }

        Ok(full_content)
    }

    /// 创建一个带实时输出的回调函数
    ///
    /// 适用于 AgentClient::send_message_with_callback
    pub fn create_callback(&self) -> impl FnMut(&str) + Send {
        let mut stdout = stdout();
        move |text: &str| {
            let _ = write!(stdout, "{}", text);
            let _ = stdout.flush();
        }
    }

    /// 输出分隔线
    pub fn output_separator(&self) -> Result<()> {
        let mut stdout = stdout();
        if self.config.colorize {
            writeln!(
                stdout,
                "\x1b[90m────────────────────────────────────\x1b[0m"
            )?;
        } else {
            writeln!(stdout, "────────────────────────────────────")?;
        }
        stdout.flush()?;
        Ok(())
    }

    /// 输出状态信息
    pub fn output_status(&self, status: &str) -> Result<()> {
        let mut stdout = stdout();
        if self.config.colorize {
            writeln!(stdout, "\x1b[90m[{}]\x1b[0m", status)?;
        } else {
            writeln!(stdout, "[{}]", status)?;
        }
        stdout.flush()?;
        Ok(())
    }
}

impl Default for StreamingOutput {
    fn default() -> Self {
        Self::default_config()
    }
}

/// 输出颜色
#[derive(Debug, Clone, Copy)]
pub enum OutputColor {
    Default,
    Gray,
    Green,
    Yellow,
    Blue,
    Cyan,
    Magenta,
    Red,
}

/// 进度指示器
///
/// 用于显示长时间操作的状态
pub struct ProgressIndicator {
    message: String,
    spinner_chars: &'static [char],
    current_idx: usize,
    colorize: bool,
}

impl ProgressIndicator {
    /// 创建新的进度指示器
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            spinner_chars: &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
            current_idx: 0,
            colorize: true,
        }
    }

    /// 设置是否使用颜色
    pub fn with_color(mut self, colorize: bool) -> Self {
        self.colorize = colorize;
        self
    }

    /// 更新并显示进度
    pub fn tick(&mut self) -> Result<()> {
        let mut stdout = stdout();

        // 清除当前行
        write!(stdout, "\r\x1b[2K")?;

        // 显示新的进度
        let spinner = self.spinner_chars[self.current_idx];
        self.current_idx = (self.current_idx + 1) % self.spinner_chars.len();

        if self.colorize {
            write!(stdout, "\x1b[36m{}\x1b[0m {}", spinner, self.message)?;
        } else {
            write!(stdout, "{} {}", spinner, self.message)?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// 完成并清除进度指示器
    pub fn finish(&mut self, final_message: Option<&str>) -> Result<()> {
        let mut stdout = stdout();

        // 清除当前行
        write!(stdout, "\r\x1b[2K")?;

        // 显示完成消息
        if let Some(msg) = final_message {
            if self.colorize {
                writeln!(stdout, "\x1b[32m✓\x1b[0m {}", msg)?;
            } else {
                writeln!(stdout, "✓ {}", msg)?;
            }
        } else {
            writeln!(stdout)?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// 显示失败状态
    pub fn fail(&mut self, error_message: &str) -> Result<()> {
        let mut stdout = stdout();

        // 清除当前行
        write!(stdout, "\r\x1b[2K")?;

        if self.colorize {
            writeln!(stdout, "\x1b[31m✗\x1b[0m {}", error_message)?;
        } else {
            writeln!(stdout, "✗ {}", error_message)?;
        }

        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert!(!config.show_thinking);
        assert!(!config.show_tool_calls);
        assert!(config.colorize);
    }

    #[test]
    fn test_streaming_output_creation() {
        let output = StreamingOutput::default_config();
        assert!(!output.is_aborted());
    }

    #[test]
    fn test_streaming_output_abort() {
        let output = StreamingOutput::default_config();
        output.abort();
        assert!(output.is_aborted());
        output.reset();
        assert!(!output.is_aborted());
    }

    #[test]
    fn test_output_event_variants() {
        let event = OutputEvent::Start {
            model: "claude-sonnet-4-6".to_string(),
        };
        assert!(matches!(event, OutputEvent::Start { .. }));

        let event = OutputEvent::TextChunk("Hello".to_string());
        assert!(matches!(event, OutputEvent::TextChunk(_)));

        let event = OutputEvent::Done { usage: None };
        assert!(matches!(event, OutputEvent::Done { .. }));
    }

    #[test]
    fn test_usage_stats() {
        let stats = UsageStats {
            input_tokens: 100,
            output_tokens: 50,
        };
        assert_eq!(stats.input_tokens, 100);
        assert_eq!(stats.output_tokens, 50);
    }

    #[test]
    fn test_progress_indicator_creation() {
        let progress = ProgressIndicator::new("Loading...");
        assert!(!progress.message.is_empty());
    }

    #[tokio::test]
    async fn test_process_stream_empty() {
        let output = StreamingOutput::default_config();
        let (_, rx) = mpsc::channel::<OutputEvent>(1);

        // 空通道应该返回空字符串
        let result = output.process_stream(rx).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
