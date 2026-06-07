//! TUI 状态栏组件

use super::color_theme::ColorTheme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// 状态栏组件
pub struct StatusComponent {
    /// 当前模式
    mode: String,
    /// 会话 ID
    session_id: Option<String>,
    /// 消息数量
    message_count: usize,
    /// 当前状态
    status: String,
    /// 是否正在处理
    is_processing: bool,
    /// 当前提供商
    provider: Option<String>,
    /// 当前模型
    model: Option<String>,
    /// Agent 状态
    agent_state: String,
    /// 是否已连接
    connected: bool,
    /// 调试模式
    debug_mode: bool,
    /// 颜色主题
    theme: ColorTheme,
}

impl StatusComponent {
    /// 创建新的状态栏组件
    pub fn new() -> Self {
        Self {
            mode: "Normal".to_string(),
            session_id: None,
            message_count: 0,
            status: "Ready".to_string(),
            is_processing: false,
            provider: None,
            model: None,
            agent_state: "Idle".to_string(),
            connected: false,
            debug_mode: false,
            theme: ColorTheme::dark(),
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.theme = theme;
    }

    /// 设置模式
    pub fn set_mode(&mut self, mode: &str) {
        self.mode = mode.to_string();
    }

    /// 设置会话 ID
    pub fn set_session_id(&mut self, session_id: Option<String>) {
        self.session_id = session_id;
    }

    /// 设置消息数量
    pub fn set_message_count(&mut self, count: usize) {
        self.message_count = count;
    }

    /// 设置状态
    pub fn set_status(&mut self, status: &str) {
        self.status = status.to_string();
    }

    /// 设置处理状态
    pub fn set_processing(&mut self, processing: bool) {
        self.is_processing = processing;
        if processing {
            self.status = "Processing...".to_string();
            self.agent_state = "Running".to_string();
        } else {
            self.agent_state = "Idle".to_string();
        }
    }

    /// 设置提供商
    pub fn set_provider(&mut self, provider: Option<String>) {
        self.provider = provider;
    }

    /// 设置模型
    pub fn set_model(&mut self, model: Option<String>) {
        self.model = model;
    }

    /// 设置连接状态
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
        if connected {
            self.status = "Connected".to_string();
        } else {
            self.status = "Disconnected".to_string();
        }
    }

    /// 设置 Agent 状态
    pub fn set_agent_state(&mut self, state: &str) {
        self.agent_state = state.to_string();
    }

    /// 设置调试模式
    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    /// 是否处于调试模式
    pub fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// 渲染组件
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let mode_color = match self.mode.as_str() {
            "Normal" => self.theme.border,
            "Insert" => self.theme.success_message,
            "Command" => self.theme.warning_message,
            _ => self.theme.punctuation,
        };

        let conn_icon = if self.connected { "🟢" } else { "🔴" };

        let session_info = self
            .session_id
            .as_ref()
            .map(|s| format!("Session: {} | ", s))
            .unwrap_or_default();

        let provider_info = match (&self.provider, &self.model) {
            (Some(p), Some(m)) => format!("{}: {} | ", p, m),
            (Some(p), None) => format!("{} | ", p),
            (None, Some(m)) => format!("Model: {} | ", m),
            (None, None) => String::new(),
        };

        let agent_color = match self.agent_state.as_str() {
            "Running" => self.theme.tool_running,
            "Idle" => self.theme.tool_success,
            "Error" => self.theme.tool_failed,
            _ => self.theme.punctuation,
        };

        // Debug mode indicator
        let debug_indicator = if self.debug_mode {
            vec![Span::styled(
                " [DEBUG] ",
                Style::default()
                    .fg(self.theme.background)
                    .bg(self.theme.warning_message),
            )]
        } else {
            vec![]
        };

        let mut spans = vec![
            Span::styled(
                format!(" {} ", self.mode),
                Style::default().fg(self.theme.background).bg(mode_color),
            ),
            Span::raw(" "),
        ];
        spans.extend(debug_indicator);
        spans.extend(vec![
            Span::styled(
                conn_icon.to_string(),
                Style::default().fg(if self.connected {
                    self.theme.success_message
                } else {
                    self.theme.error_message
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}] ", self.agent_state),
                Style::default().fg(agent_color),
            ),
            Span::styled(
                format!(
                    "{}{}Msgs: {} | ",
                    provider_info, session_info, self.message_count
                ),
                Style::default().fg(self.theme.foreground),
            ),
            Span::styled(
                format!("{} ", self.status),
                Style::default().fg(self.theme.title),
            ),
            Span::raw(" "),
            Span::styled(
                "Ctrl+C: Quit | Enter: Send | Esc: Cancel",
                Style::default().fg(self.theme.comment),
            ),
        ]);

        let status_line = Line::from(spans);

        let paragraph = Paragraph::new(status_line).block(Block::default().borders(Borders::NONE));

        f.render_widget(paragraph, area);
    }

    /// 获取模式
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// 获取消息数量
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// 是否正在处理
    pub fn is_processing(&self) -> bool {
        self.is_processing
    }

    /// 获取提供商
    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    /// 获取模型
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// 是否已连接
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Default for StatusComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_component_creation() {
        let status = StatusComponent::new();
        assert_eq!(status.mode(), "Normal");
        assert!(!status.is_processing());
        assert!(!status.is_connected());
    }

    #[test]
    fn test_set_mode() {
        let mut status = StatusComponent::new();
        status.set_mode("Insert");
        assert_eq!(status.mode(), "Insert");
    }

    #[test]
    fn test_set_processing() {
        let mut status = StatusComponent::new();
        status.set_processing(true);
        assert!(status.is_processing());
        assert_eq!(status.status, "Processing...");
        assert_eq!(status.agent_state, "Running");
    }

    #[test]
    fn test_set_processing_false() {
        let mut status = StatusComponent::new();
        status.set_processing(true);
        status.set_processing(false);
        assert!(!status.is_processing());
        assert_eq!(status.agent_state, "Idle");
    }

    #[test]
    fn test_set_session_id() {
        let mut status = StatusComponent::new();
        status.set_session_id(Some("test-session".to_string()));
        assert!(status.session_id.is_some());
    }

    #[test]
    fn test_message_count() {
        let mut status = StatusComponent::new();
        status.set_message_count(10);
        assert_eq!(status.message_count(), 10);
    }

    #[test]
    fn test_set_provider() {
        let mut status = StatusComponent::new();
        status.set_provider(Some("anthropic".to_string()));
        assert_eq!(status.provider(), Some("anthropic"));
    }

    #[test]
    fn test_set_model() {
        let mut status = StatusComponent::new();
        status.set_model(Some("claude-sonnet-4-6".to_string()));
        assert_eq!(status.model(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn test_set_connected() {
        let mut status = StatusComponent::new();
        status.set_connected(true);
        assert!(status.is_connected());
        assert_eq!(status.status, "Connected");
    }

    #[test]
    fn test_set_connected_false() {
        let mut status = StatusComponent::new();
        status.set_connected(true);
        status.set_connected(false);
        assert!(!status.is_connected());
        assert_eq!(status.status, "Disconnected");
    }

    #[test]
    fn test_set_agent_state() {
        let mut status = StatusComponent::new();
        status.set_agent_state("Running");
        assert_eq!(status.agent_state, "Running");
    }

    #[test]
    fn test_default() {
        let status = StatusComponent::default();
        assert_eq!(status.mode(), "Normal");
    }

    #[test]
    fn test_set_debug_mode() {
        let mut status = StatusComponent::new();
        assert!(!status.is_debug_mode());
        status.set_debug_mode(true);
        assert!(status.is_debug_mode());
        status.set_debug_mode(false);
        assert!(!status.is_debug_mode());
    }
}
