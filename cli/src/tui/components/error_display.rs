//! TUI 错误消息显示组件

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorType {
    /// 配置错误（API key 等）
    Config,
    /// 网络错误
    Network,
    /// API 错误（限流、认证等）
    Api,
    /// 文件系统错误
    FileSystem,
    /// 权限错误
    Permission,
    /// 用户输入错误
    UserInput,
    /// 内部错误
    Internal,
    /// 警告（非致命）
    Warning,
}

/// 错误消息
#[derive(Debug, Clone)]
pub struct ErrorMessage {
    /// 错误类型
    pub error_type: ErrorType,
    /// 简短标题
    pub title: String,
    /// 详细描述
    pub description: String,
    /// 建议操作
    pub suggestions: Vec<String>,
    /// 是否可恢复
    pub recoverable: bool,
    /// 错误代码（可选）
    pub code: Option<String>,
}

impl ErrorMessage {
    /// 创建新的错误消息
    pub fn new(error_type: ErrorType, title: impl Into<String>) -> Self {
        Self {
            error_type,
            title: title.into(),
            description: String::new(),
            suggestions: Vec::new(),
            recoverable: true,
            code: None,
        }
    }

    /// 添加描述
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 添加建议
    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// 设置是否可恢复
    pub fn recoverable(mut self, recoverable: bool) -> Self {
        self.recoverable = recoverable;
        self
    }

    /// 设置错误代码
    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// 从 AgentError 转换
    pub fn from_agent_error(error: &crate::agent::AgentError) -> Self {
        use crate::agent::AgentError;

        match error {
            AgentError::Config(msg) => Self::new(ErrorType::Config, "Configuration Error")
                .description(msg.clone())
                .suggestion("Check your API key configuration")
                .suggestion("Run: continuum config add-provider anthropic --key YOUR_KEY")
                .suggestion("Or set environment variable: CONTINUUM_API_KEY"),

            AgentError::Network(msg) => Self::new(ErrorType::Network, "Network Error")
                .description(msg.clone())
                .suggestion("Check your internet connection")
                .suggestion("Verify the API endpoint is accessible")
                .suggestion("Try again in a few moments"),

            AgentError::Api(msg) => {
                let mut err = Self::new(ErrorType::Api, "API Error").description(msg.clone());

                // 根据消息内容添加针对性建议
                if msg.contains("rate limit") || msg.contains("429") {
                    err = err
                        .suggestion("Rate limit reached - wait before retrying")
                        .suggestion("Consider using a different model/provider");
                } else if msg.contains("unauthorized") || msg.contains("401") {
                    err = err
                        .suggestion("API key may be invalid or expired")
                        .suggestion("Verify your API key is correct");
                } else if msg.contains("timeout") {
                    err = err.suggestion("Request timed out - try a shorter prompt");
                }

                err.suggestion("Check API status: status.anthropic.com")
            }
        }
    }

    /// 创建通用错误
    pub fn generic(msg: impl Into<String>) -> Self {
        Self::new(ErrorType::Internal, "Error").description(msg.into())
    }

    /// 创建警告
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(ErrorType::Warning, title).recoverable(true)
    }
}

/// 错误显示组件
pub struct ErrorDisplay {
    /// 当前错误消息
    error: Option<ErrorMessage>,
    /// 是否显示详细视图
    expanded: bool,
    /// 错误历史（最近10条）
    history: Vec<ErrorMessage>,
    /// 当前历史索引
    history_index: usize,
}

impl ErrorDisplay {
    /// 创建新的错误显示组件
    pub fn new() -> Self {
        Self {
            error: None,
            expanded: false,
            history: Vec::with_capacity(10),
            history_index: 0,
        }
    }

    /// 显示错误
    pub fn show(&mut self, error: ErrorMessage) {
        // 添加到历史
        if self.history.len() >= 10 {
            self.history.remove(0);
        }
        self.history.push(error.clone());
        self.history_index = self.history.len();

        self.error = Some(error);
        self.expanded = false;
    }

    /// 显示 Agent 错误
    pub fn show_agent_error(&mut self, error: &crate::agent::AgentError) {
        self.show(ErrorMessage::from_agent_error(error));
    }

    /// 显示简单错误消息
    pub fn show_error(&mut self, msg: impl Into<String>) {
        self.show(ErrorMessage::generic(msg));
    }

    /// 显示警告
    pub fn show_warning(&mut self, msg: impl Into<String>) {
        self.show(ErrorMessage::warning(msg));
    }

    /// 隐藏错误
    pub fn hide(&mut self) {
        self.error = None;
        self.expanded = false;
    }

    /// 切换展开状态
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// 设置展开状态
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// 是否可见
    pub fn is_visible(&self) -> bool {
        self.error.is_some()
    }

    /// 获取当前错误
    pub fn current_error(&self) -> Option<&ErrorMessage> {
        self.error.as_ref()
    }

    /// 导航历史（上一条）
    pub fn history_prev(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.error = Some(self.history[self.history_index].clone());
        }
    }

    /// 导航历史（下一条）
    pub fn history_next(&mut self) {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            self.error = Some(self.history[self.history_index].clone());
        } else if self.history_index == self.history.len() - 1 {
            // 到达最新，隐藏错误
            self.error = None;
            self.history_index = self.history.len();
        }
    }

    /// 清空历史
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_index = 0;
        self.error = None;
    }

    /// 渲染组件（内联模式）
    pub fn render_inline(&self, f: &mut Frame, area: Rect) {
        let Some(error) = &self.error else { return };

        let (fg_color, bg_color) = Self::get_colors(&error.error_type);

        let icon = match error.error_type {
            ErrorType::Warning => "⚠",
            _ => "✗",
        };

        let mut spans = vec![
            Span::styled(
                format!(" {} ", icon),
                Style::default()
                    .fg(Color::White)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(&error.title, Style::default().fg(fg_color).bold()),
        ];

        if !error.description.is_empty() {
            spans.push(Span::raw(": "));
            spans.push(Span::styled(
                &error.description,
                Style::default().fg(Color::Gray),
            ));
        }

        if error.recoverable {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "[Press Esc to dismiss]",
                Style::default().fg(Color::DarkGray),
            ));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }

    /// 渲染组件（弹窗模式）
    pub fn render_popup(&self, f: &mut Frame, area: Rect) {
        let Some(error) = &self.error else { return };

        // 清除背景
        f.render_widget(Clear, area);

        let (fg_color, border_color) = Self::get_colors(&error.error_type);

        let icon = match error.error_type {
            ErrorType::Warning => "⚠",
            _ => "✗",
        };

        // 计算弹窗大小
        let width = area.width.clamp(30, 60);
        let height = if self.expanded {
            (4 + error.suggestions.len() + 2).min(15) as u16
        } else {
            5
        };

        let popup_area = Rect::new(
            (area.width.saturating_sub(width)) / 2,
            (area.height.saturating_sub(height)) / 2,
            width,
            height,
        );

        // 标题行
        let title_line = Line::from(vec![
            Span::styled(
                format!(" {} ", icon),
                Style::default().fg(Color::White).bg(border_color),
            ),
            Span::raw(" "),
            Span::styled(&error.title, Style::default().fg(fg_color).bold()),
            if let Some(code) = &error.code {
                Span::styled(format!(" [{}]", code), Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            },
        ]);

        let mut lines = vec![title_line];

        // 描述
        if !error.description.is_empty() && self.expanded {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                &error.description,
                Style::default().fg(Color::Gray),
            ));
        }

        // 建议
        if self.expanded && !error.suggestions.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Suggestions:",
                Style::default().fg(Color::Cyan),
            ));

            for suggestion in &error.suggestions {
                lines.push(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::DarkGray)),
                    Span::styled(suggestion, Style::default().fg(Color::Gray)),
                ]));
            }
        }

        // 底部提示
        let help_text = if self.expanded {
            "Esc: Dismiss | Enter: Collapse | ←→: History"
        } else {
            "Esc: Dismiss | Enter: Expand | ←→: History"
        };

        lines.push(Line::raw(""));
        lines.push(Line::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        ));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let paragraph = Paragraph::new(lines).block(block);

        f.render_widget(paragraph, popup_area);
    }

    /// 获取颜色
    fn get_colors(error_type: &ErrorType) -> (Color, Color) {
        match error_type {
            ErrorType::Config => (Color::Red, Color::LightRed),
            ErrorType::Network => (Color::Yellow, Color::LightYellow),
            ErrorType::Api => (Color::Magenta, Color::LightMagenta),
            ErrorType::FileSystem => (Color::Blue, Color::LightBlue),
            ErrorType::Permission => (Color::Red, Color::LightRed),
            ErrorType::UserInput => (Color::Cyan, Color::LightCyan),
            ErrorType::Internal => (Color::Red, Color::LightRed),
            ErrorType::Warning => (Color::Yellow, Color::LightYellow),
        }
    }
}

impl Default for ErrorDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_message_creation() {
        let error = ErrorMessage::new(ErrorType::Config, "Test Error")
            .description("Test description")
            .suggestion("Test suggestion");

        assert_eq!(error.title, "Test Error");
        assert_eq!(error.description, "Test description");
        assert_eq!(error.suggestions.len(), 1);
        assert!(error.recoverable);
    }

    #[test]
    fn test_error_message_code() {
        let error = ErrorMessage::new(ErrorType::Api, "API Error").code("E001");

        assert_eq!(error.code, Some("E001".to_string()));
    }

    #[test]
    fn test_error_display_creation() {
        let display = ErrorDisplay::new();
        assert!(!display.is_visible());
        assert!(display.history.is_empty());
    }

    #[test]
    fn test_show_error() {
        let mut display = ErrorDisplay::new();
        display.show(ErrorMessage::new(ErrorType::Config, "Test"));

        assert!(display.is_visible());
        assert_eq!(display.history.len(), 1);
    }

    #[test]
    fn test_hide_error() {
        let mut display = ErrorDisplay::new();
        display.show(ErrorMessage::new(ErrorType::Config, "Test"));
        display.hide();

        assert!(!display.is_visible());
    }

    #[test]
    fn test_toggle_expanded() {
        let mut display = ErrorDisplay::new();
        display.show(ErrorMessage::new(ErrorType::Config, "Test"));

        assert!(!display.expanded);
        display.toggle_expanded();
        assert!(display.expanded);
    }

    #[test]
    fn test_history_navigation() {
        let mut display = ErrorDisplay::new();

        display.show(ErrorMessage::new(ErrorType::Config, "Error 1"));
        display.show(ErrorMessage::new(ErrorType::Network, "Error 2"));

        assert_eq!(display.history.len(), 2);
        assert_eq!(display.history_index, 2);

        display.history_prev();
        assert_eq!(display.history_index, 1);

        display.history_next();
        assert_eq!(display.history_index, 2);
    }

    #[test]
    fn test_history_limit() {
        let mut display = ErrorDisplay::new();

        for i in 0..12 {
            display.show(ErrorMessage::new(ErrorType::Config, format!("Error {}", i)));
        }

        assert_eq!(display.history.len(), 10);
    }

    #[test]
    fn test_show_error_convenience() {
        let mut display = ErrorDisplay::new();
        display.show_error("Something went wrong");

        assert!(display.is_visible());
        assert_eq!(display.current_error().unwrap().title, "Error");
    }

    #[test]
    fn test_show_warning() {
        let mut display = ErrorDisplay::new();
        display.show_warning("This is a warning");

        assert!(display.is_visible());
        assert_eq!(
            display.current_error().unwrap().error_type,
            ErrorType::Warning
        );
    }

    #[test]
    fn test_clear_history() {
        let mut display = ErrorDisplay::new();
        display.show(ErrorMessage::new(ErrorType::Config, "Test"));
        display.clear_history();

        assert!(display.history.is_empty());
        assert!(!display.is_visible());
    }

    #[test]
    fn test_recoverable_setting() {
        let error = ErrorMessage::new(ErrorType::Config, "Test").recoverable(false);

        assert!(!error.recoverable);
    }

    #[test]
    fn test_generic_error() {
        let error = ErrorMessage::generic("Generic error");
        assert_eq!(error.title, "Error");
        assert_eq!(error.description, "Generic error");
    }

    #[test]
    fn test_warning_creation() {
        let error = ErrorMessage::warning("Warning title");
        assert_eq!(error.title, "Warning title");
        assert_eq!(error.error_type, ErrorType::Warning);
        assert!(error.recoverable);
    }
}
