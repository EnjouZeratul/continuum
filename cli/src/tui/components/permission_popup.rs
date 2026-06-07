//! 权限确认弹窗组件
//!
//! 用于工具调用前的权限确认，支持 Allow / Deny / Always Allow 三个选项

use super::color_theme::ColorTheme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 权限动作结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    /// 无动作
    None,
    /// 允许（本次）
    Allow,
    /// 拒绝
    Deny,
    /// 始终允许（永久授权）
    AlwaysAllow,
}

/// 权限请求信息
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// 工具名称
    pub tool_name: String,
    /// 动作描述
    pub action: String,
    /// 参数详情（JSON 格式）
    pub parameters: String,
    /// 风险等级
    pub risk_level: PermissionRisk,
    /// 请求来源
    pub source: String,
}

/// 权限风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionRisk {
    /// 低风险 - 只读操作
    Low,
    /// 中风险 - 可逆修改
    Medium,
    /// 高风险 - 不可逆操作或外部系统交互
    High,
}

/// 权限确认弹窗
pub struct PermissionPopup {
    /// 是否可见
    visible: bool,
    /// 当前请求
    request: Option<PermissionRequest>,
    /// 选中的按钮索引 (0: Allow, 1: Deny, 2: Always Allow)
    selected: usize,
    /// 错误消息
    error_message: Option<String>,
    /// 颜色主题
    theme: ColorTheme,
}

impl PermissionPopup {
    /// 创建新的权限确认弹窗
    pub fn new() -> Self {
        Self {
            visible: false,
            request: None,
            selected: 0,
            error_message: None,
            theme: ColorTheme::dark(),
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.theme = theme;
    }

    /// 显示权限请求
    pub fn show(&mut self, request: PermissionRequest) {
        self.visible = true;
        self.request = Some(request);
        self.selected = 0;
        self.error_message = None;
    }

    /// 隐藏弹窗
    pub fn hide(&mut self) {
        self.visible = false;
        self.request = None;
        self.error_message = None;
    }

    /// 是否可见
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 获取当前请求
    pub fn get_request(&self) -> Option<&PermissionRequest> {
        self.request.as_ref()
    }

    /// 选择下一个按钮
    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % 3;
    }

    /// 选择上一个按钮
    pub fn select_prev(&mut self) {
        self.selected = if self.selected == 0 {
            2
        } else {
            self.selected - 1
        };
    }

    /// 选择 Allow
    pub fn select_allow(&mut self) {
        self.selected = 0;
    }

    /// 选择 Deny
    pub fn select_deny(&mut self) {
        self.selected = 1;
    }

    /// 选择 Always Allow
    pub fn select_always_allow(&mut self) {
        self.selected = 2;
    }

    /// 当前是否选中 Allow
    pub fn is_allow_selected(&self) -> bool {
        self.selected == 0
    }

    /// 当前是否选中 Deny
    pub fn is_deny_selected(&self) -> bool {
        self.selected == 1
    }

    /// 当前是否选中 Always Allow
    pub fn is_always_allow_selected(&self) -> bool {
        self.selected == 2
    }

    /// 设置错误消息
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    /// 清除错误消息
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// 处理键盘事件
    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) -> PermissionAction {
        use crossterm::event::KeyCode;

        match key {
            // 左右箭头切换按钮
            KeyCode::Left | KeyCode::Char('h') => {
                self.select_prev();
                PermissionAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.select_next();
                PermissionAction::None
            }
            // Tab 切换按钮
            KeyCode::Tab | KeyCode::BackTab => {
                self.select_next();
                PermissionAction::None
            }
            // Enter 确认当前选择
            KeyCode::Enter => match self.selected {
                0 => PermissionAction::Allow,
                1 => PermissionAction::Deny,
                2 => PermissionAction::AlwaysAllow,
                _ => PermissionAction::None,
            },
            // 快捷键 Y - Allow
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.select_allow();
                PermissionAction::Allow
            }
            // 快捷键 N - Deny
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.select_deny();
                PermissionAction::Deny
            }
            // 快捷键 A - Always Allow
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.select_always_allow();
                PermissionAction::AlwaysAllow
            }
            _ => PermissionAction::None,
        }
    }

    /// 渲染弹窗
    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let request = match &self.request {
            Some(r) => r,
            None => return,
        };

        // 计算弹窗大小
        let dialog_width = std::cmp::min(70, area.width.saturating_sub(4));
        let dialog_height = self.calculate_height(dialog_width);

        // 居中
        let dialog_area = Rect::new(
            (area.width.saturating_sub(dialog_width)) / 2,
            (area.height.saturating_sub(dialog_height)) / 2,
            dialog_width,
            dialog_height,
        );

        // 清除背景
        f.render_widget(Clear, dialog_area);

        // 获取边框颜色基于风险等级
        let border_color = match request.risk_level {
            PermissionRisk::Low => self.theme.success_message,
            PermissionRisk::Medium => self.theme.warning_message,
            PermissionRisk::High => self.theme.error_message,
        };

        // 获取标题
        let title = match request.risk_level {
            PermissionRisk::Low => " 权限请求 ",
            PermissionRisk::Medium => " 权限请求 (中风险) ",
            PermissionRisk::High => " 权限请求 (高风险) ",
        };

        // 创建边框
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner_area = block.inner(dialog_area);
        f.render_widget(block, dialog_area);

        // 内部布局
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // 工具名称和动作
                Constraint::Min(4),    // 参数详情
                Constraint::Length(1), // 来源
                Constraint::Length(3), // 按钮
                Constraint::Length(1), // 快捷键提示
            ])
            .split(inner_area);

        // 渲染工具名称和动作
        let header_style = match request.risk_level {
            PermissionRisk::Low => Style::default().fg(self.theme.foreground),
            PermissionRisk::Medium => Style::default().fg(self.theme.warning_message),
            PermissionRisk::High => Style::default()
                .fg(self.theme.error_message)
                .add_modifier(Modifier::BOLD),
        };

        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                &request.tool_name,
                Style::default()
                    .fg(self.theme.function)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": "),
            Span::styled(&request.action, header_style),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(header, chunks[0]);

        // 渲染参数详情
        let params_lines = self.format_parameters(&request.parameters, chunks[1].width as usize);
        let params: Vec<Line> = params_lines
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    line,
                    Style::default().fg(self.theme.punctuation),
                ))
            })
            .collect();

        let params_widget = Paragraph::new(params)
            .block(Block::default().borders(Borders::TOP).title(" 参数 "))
            .wrap(Wrap { trim: true });
        f.render_widget(params_widget, chunks[1]);

        // 渲染来源
        let source = Paragraph::new(Line::from(vec![
            Span::styled("来源: ", Style::default().fg(self.theme.comment)),
            Span::styled(&request.source, Style::default().fg(self.theme.punctuation)),
        ]))
        .alignment(Alignment::Left);
        f.render_widget(source, chunks[2]);

        // 渲染按钮
        self.render_buttons(f, chunks[3]);

        // 渲染快捷键提示
        let hints = Paragraph::new(Line::from(vec![
            Span::styled(
                "[Y]",
                Style::default()
                    .fg(self.theme.success_message)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" 允许  "),
            Span::styled(
                "[N/Esc]",
                Style::default()
                    .fg(self.theme.error_message)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" 拒绝  "),
            Span::styled(
                "[A]",
                Style::default()
                    .fg(self.theme.warning_message)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" 始终允许  "),
            Span::styled("[←→]", Style::default().fg(self.theme.comment)),
            Span::raw(" 切换"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hints, chunks[4]);

        // 渲染错误消息（如果有）
        if let Some(error) = &self.error_message {
            let error_area = Rect::new(
                dialog_area.x + 1,
                dialog_area.y + dialog_height.saturating_sub(1),
                dialog_area.width.saturating_sub(2),
                1,
            );
            let error = Paragraph::new(error.as_str())
                .style(
                    Style::default()
                        .fg(self.theme.error_message)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center);
            f.render_widget(error, error_area);
        }
    }

    /// 渲染按钮
    fn render_buttons(&self, f: &mut Frame, area: Rect) {
        let buttons_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area);

        // Allow 按钮
        let allow_style = if self.selected == 0 {
            Style::default()
                .fg(self.theme.success_message)
                .bg(self.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.punctuation)
        };
        let allow_button = Paragraph::new(" [ Allow ] ")
            .style(allow_style)
            .alignment(Alignment::Center);
        f.render_widget(allow_button, buttons_layout[0]);

        // Deny 按钮
        let deny_style = if self.selected == 1 {
            Style::default()
                .fg(self.theme.error_message)
                .bg(self.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.punctuation)
        };
        let deny_button = Paragraph::new(" [ Deny ] ")
            .style(deny_style)
            .alignment(Alignment::Center);
        f.render_widget(deny_button, buttons_layout[1]);

        // Always Allow 按钮
        let always_style = if self.selected == 2 {
            Style::default()
                .fg(self.theme.warning_message)
                .bg(self.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.punctuation)
        };
        let always_button = Paragraph::new(" [ Always Allow ] ")
            .style(always_style)
            .alignment(Alignment::Center);
        f.render_widget(always_button, buttons_layout[2]);
    }

    /// 格式化参数显示
    fn format_parameters(&self, params: &str, max_width: usize) -> Vec<String> {
        if params.is_empty() {
            return vec!["(无参数)".to_string()];
        }

        // 尝试解析 JSON 并格式化
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(params) {
            let formatted =
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| params.to_string());
            formatted
                .lines()
                .flat_map(|line| self.wrap_line(line, max_width))
                .collect()
        } else {
            // 不是 JSON，直接分行
            params
                .lines()
                .flat_map(|line| self.wrap_line(line, max_width))
                .collect()
        }
    }

    /// 自动换行
    fn wrap_line(&self, line: &str, max_width: usize) -> Vec<String> {
        if line.len() <= max_width {
            return vec![line.to_string()];
        }

        let mut result = Vec::new();
        let mut remaining = line;

        while remaining.len() > max_width {
            // 尝试在合适的位置断开（逗号、空格等）
            let break_pos = remaining[..max_width]
                .rfind([',', ' '])
                .unwrap_or(max_width - 1);

            result.push(remaining[..=break_pos].to_string());
            remaining = &remaining[break_pos + 1..];
        }

        if !remaining.is_empty() {
            result.push(remaining.to_string());
        }

        result
    }

    /// 计算弹窗高度
    fn calculate_height(&self, width: u16) -> u16 {
        // 基础高度: 边框(2) + 标题(2) + 来源(1) + 按钮(3) + 提示(1) = 9
        let base_height: u16 = 9;

        // 参数高度（估算）
        let params_height: u16 = if let Some(req) = &self.request {
            let lines = self.format_parameters(&req.parameters, width as usize);
            std::cmp::min(lines.len() as u16 + 2, 10) // 最多 10 行
        } else {
            4
        };

        // 错误消息高度
        let error_height: u16 = if self.error_message.is_some() { 1 } else { 0 };

        base_height + params_height + error_height
    }
}

impl Default for PermissionPopup {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_creation() {
        let popup = PermissionPopup::new();
        assert!(!popup.is_visible());
        assert!(popup.get_request().is_none());
    }

    #[test]
    fn test_show_hide() {
        let mut popup = PermissionPopup::new();
        let request = PermissionRequest {
            tool_name: "bash".to_string(),
            action: "execute command".to_string(),
            parameters: r#"{"command": "ls"}"#.to_string(),
            risk_level: PermissionRisk::High,
            source: "agent".to_string(),
        };

        popup.show(request);
        assert!(popup.is_visible());
        assert!(popup.get_request().is_some());

        popup.hide();
        assert!(!popup.is_visible());
        assert!(popup.get_request().is_none());
    }

    #[test]
    fn test_button_selection() {
        let mut popup = PermissionPopup::new();

        assert!(popup.is_allow_selected());
        assert!(!popup.is_deny_selected());
        assert!(!popup.is_always_allow_selected());

        popup.select_next();
        assert!(!popup.is_allow_selected());
        assert!(popup.is_deny_selected());

        popup.select_next();
        assert!(!popup.is_deny_selected());
        assert!(popup.is_always_allow_selected());

        popup.select_next();
        assert!(popup.is_allow_selected());
        assert!(!popup.is_deny_selected());
    }

    #[test]
    fn test_key_handling() {
        let mut popup = PermissionPopup::new();

        // 左右切换
        let action = popup.handle_key(crossterm::event::KeyCode::Right);
        assert_eq!(action, PermissionAction::None);
        assert!(popup.is_deny_selected());

        // Y 键
        let action = popup.handle_key(crossterm::event::KeyCode::Char('y'));
        assert_eq!(action, PermissionAction::Allow);
        assert!(popup.is_allow_selected());

        // N 键
        let action = popup.handle_key(crossterm::event::KeyCode::Char('n'));
        assert_eq!(action, PermissionAction::Deny);
        assert!(popup.is_deny_selected());

        // A 键
        let action = popup.handle_key(crossterm::event::KeyCode::Char('a'));
        assert_eq!(action, PermissionAction::AlwaysAllow);
        assert!(popup.is_always_allow_selected());

        // Esc 键
        let action = popup.handle_key(crossterm::event::KeyCode::Esc);
        assert_eq!(action, PermissionAction::Deny);
    }

    #[test]
    fn test_enter_key() {
        let mut popup = PermissionPopup::new();

        popup.select_allow();
        let action = popup.handle_key(crossterm::event::KeyCode::Enter);
        assert_eq!(action, PermissionAction::Allow);

        popup.select_deny();
        let action = popup.handle_key(crossterm::event::KeyCode::Enter);
        assert_eq!(action, PermissionAction::Deny);

        popup.select_always_allow();
        let action = popup.handle_key(crossterm::event::KeyCode::Enter);
        assert_eq!(action, PermissionAction::AlwaysAllow);
    }

    #[test]
    fn test_error_message() {
        let mut popup = PermissionPopup::new();

        popup.set_error("Test error".to_string());
        assert!(popup.error_message.is_some());

        popup.clear_error();
        assert!(popup.error_message.is_none());
    }

    #[test]
    fn test_risk_level_colors() {
        let mut popup = PermissionPopup::new();

        let low_risk = PermissionRequest {
            tool_name: "read".to_string(),
            action: "read file".to_string(),
            parameters: "{}".to_string(),
            risk_level: PermissionRisk::Low,
            source: "agent".to_string(),
        };

        popup.show(low_risk);
        assert!(popup.is_visible());

        let high_risk = PermissionRequest {
            tool_name: "bash".to_string(),
            action: "rm -rf /".to_string(),
            parameters: r#"{"command": "rm -rf /"}"#.to_string(),
            risk_level: PermissionRisk::High,
            source: "agent".to_string(),
        };

        popup.show(high_risk);
        assert!(popup.is_visible());
    }

    #[test]
    fn test_format_parameters() {
        let popup = PermissionPopup::new();

        // 空 JSON
        let lines = popup.format_parameters("{}", 50);
        assert!(!lines.is_empty());

        // 有效 JSON
        let json = r#"{"file": "/path/to/file", "mode": "read"}"#;
        let lines = popup.format_parameters(json, 50);
        assert!(!lines.is_empty());

        // 非 JSON
        let text = "plain text parameter";
        let lines = popup.format_parameters(text, 50);
        assert!(!lines.is_empty());
    }
}
