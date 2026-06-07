//! 权限确认对话框组件

use super::color_theme::ColorTheme;
use crate::tui::slash_commands::{ParsedCommand, RiskLevel};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 确认对话框
pub struct ConfirmationDialog {
    /// 是否可见
    visible: bool,
    /// 标题
    title: String,
    /// 消息内容
    message: String,
    /// 详细信息
    details: Vec<String>,
    /// 风险等级
    risk_level: RiskLevel,
    /// 待确认的命令
    pending_command: Option<ParsedCommand>,
    /// 当前选中的按钮 (0: 确认, 1: 取消)
    selected_button: usize,
    /// 操作类型描述
    operation_type: String,
    /// 颜色主题
    theme: ColorTheme,
}

impl ConfirmationDialog {
    /// 创建新的确认对话框
    pub fn new() -> Self {
        Self {
            visible: false,
            title: "确认操作".to_string(),
            message: String::new(),
            details: Vec::new(),
            risk_level: RiskLevel::Low,
            pending_command: None,
            selected_button: 0,
            operation_type: "操作".to_string(),
            theme: ColorTheme::dark(),
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.theme = theme;
    }

    /// 显示确认对话框
    pub fn show(&mut self, command: ParsedCommand, operation_type: &str, details: Vec<String>) {
        self.visible = true;
        self.pending_command = Some(command.clone());
        self.operation_type = operation_type.to_string();
        self.details = details;
        self.risk_level = command.command.risk_level;
        self.selected_button = 0;

        self.title = match self.risk_level {
            RiskLevel::Low => "确认操作".to_string(),
            RiskLevel::Medium => "确认操作 (中风险)".to_string(),
            RiskLevel::High => "确认操作 (高风险)".to_string(),
        };

        self.message = format!("是否执行: /{}?", command.command.name);
    }

    /// 显示权限请求对话框
    pub fn show_permission_request(
        &mut self,
        tool_name: &str,
        action: &str,
        risk_level: RiskLevel,
        details: Vec<String>,
    ) {
        self.visible = true;
        self.pending_command = None;
        self.operation_type = tool_name.to_string();
        self.details = details;
        self.risk_level = risk_level;
        self.selected_button = 0;

        self.title = match risk_level {
            RiskLevel::Low => "权限请求".to_string(),
            RiskLevel::Medium => "权限请求 (中风险)".to_string(),
            RiskLevel::High => "权限请求 (高风险)".to_string(),
        };

        self.message = format!("{}: {}", tool_name, action);
    }

    /// 隐藏对话框
    pub fn hide(&mut self) {
        self.visible = false;
        self.pending_command = None;
        self.message.clear();
        self.details.clear();
    }

    /// 是否可见
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 获取待确认的命令
    pub fn get_pending_command(&self) -> Option<&ParsedCommand> {
        self.pending_command.as_ref()
    }

    /// 选择下一个按钮
    pub fn select_next(&mut self) {
        self.selected_button = (self.selected_button + 1) % 2;
    }

    /// 选择上一个按钮
    pub fn select_prev(&mut self) {
        self.selected_button = if self.selected_button == 0 { 1 } else { 0 };
    }

    /// 选择左侧按钮（确认）
    pub fn select_confirm(&mut self) {
        self.selected_button = 0;
    }

    /// 选择右侧按钮（取消）
    pub fn select_cancel(&mut self) {
        self.selected_button = 1;
    }

    /// 是否选择了确认
    pub fn is_confirm_selected(&self) -> bool {
        self.selected_button == 0
    }

    /// 处理按键
    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) -> ConfirmAction {
        use crossterm::event::KeyCode;

        match key {
            KeyCode::Left | KeyCode::Char('h') => {
                self.select_confirm();
                ConfirmAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.select_cancel();
                ConfirmAction::None
            }
            KeyCode::Tab => {
                self.select_next();
                ConfirmAction::None
            }
            KeyCode::Enter => {
                if self.is_confirm_selected() {
                    ConfirmAction::Confirmed
                } else {
                    ConfirmAction::Cancelled
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.select_confirm();
                ConfirmAction::Confirmed
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => ConfirmAction::Cancelled,
            _ => ConfirmAction::None,
        }
    }

    /// 渲染对话框
    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // 计算对话框大小
        let dialog_width = std::cmp::min(60, area.width - 4);
        let dialog_height = self.calculate_height(dialog_width as usize);

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
        let border_color = match self.risk_level {
            RiskLevel::Low => self.theme.success_message,
            RiskLevel::Medium => self.theme.warning_message,
            RiskLevel::High => self.theme.error_message,
        };

        // 创建对话框边框
        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner_area = block.inner(dialog_area);
        f.render_widget(block, dialog_area);

        // 内部布局
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // 消息
                Constraint::Min(3),    // 详情
                Constraint::Length(3), // 按钮
                Constraint::Length(1), // 快捷键提示
            ])
            .split(inner_area);

        // 渲染消息
        let message_style = match self.risk_level {
            RiskLevel::Low => Style::default().fg(self.theme.foreground),
            RiskLevel::Medium => Style::default().fg(self.theme.warning_message),
            RiskLevel::High => Style::default()
                .fg(self.theme.error_message)
                .add_modifier(Modifier::BOLD),
        };

        let message = Paragraph::new(self.message.clone())
            .style(message_style)
            .alignment(Alignment::Center);
        f.render_widget(message, chunks[0]);

        // 渲染详情
        if !self.details.is_empty() {
            let details_text: Vec<Line> = self
                .details
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        line,
                        Style::default().fg(self.theme.punctuation),
                    ))
                })
                .collect();

            let details = Paragraph::new(details_text).wrap(Wrap { trim: true });
            f.render_widget(details, chunks[1]);
        }

        // 渲染按钮
        self.render_buttons(f, chunks[2]);

        // 渲染快捷键提示
        let hints = Paragraph::new(Line::from(vec![
            Span::styled(
                "[Y]",
                Style::default()
                    .fg(self.theme.success_message)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" 确认  "),
            Span::styled(
                "[N/Esc]",
                Style::default()
                    .fg(self.theme.error_message)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" 取消  "),
            Span::styled("[←→]", Style::default().fg(self.theme.punctuation)),
            Span::raw(" 切换"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hints, chunks[3]);
    }

    /// 渲染按钮
    fn render_buttons(&self, f: &mut Frame, area: Rect) {
        let buttons_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // 确认按钮
        let confirm_style = if self.selected_button == 0 {
            Style::default()
                .fg(self.theme.success_message)
                .bg(self.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.punctuation)
        };

        let confirm_button = Paragraph::new(" [ 确认 ] ")
            .style(confirm_style)
            .alignment(Alignment::Center);
        f.render_widget(confirm_button, buttons_layout[0]);

        // 取消按钮
        let cancel_style = if self.selected_button == 1 {
            Style::default()
                .fg(self.theme.error_message)
                .bg(self.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.punctuation)
        };

        let cancel_button = Paragraph::new(" [ 取消 ] ")
            .style(cancel_style)
            .alignment(Alignment::Center);
        f.render_widget(cancel_button, buttons_layout[1]);
    }

    /// 计算对话框高度
    fn calculate_height(&self, width: usize) -> u16 {
        // 基础高度: 边框(2) + 消息(2) + 按钮(3) + 提示(1) = 8
        let base_height = 8u16;

        // 详情高度
        let details_height: u16 = if self.details.is_empty() {
            0
        } else {
            // 简单估算行数
            let total_chars: usize = self.details.iter().map(|s| s.len()).sum();
            (total_chars / width.saturating_sub(2) + self.details.len()) as u16
        };

        base_height + details_height.min(10)
    }
}

impl Default for ConfirmationDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// 确认动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// 无动作
    None,
    /// 已确认
    Confirmed,
    /// 已取消
    Cancelled,
}

/// 权限管理器
pub struct PermissionManager {
    /// 已授予的权限
    granted_permissions: std::collections::HashSet<String>,
    /// 拒绝的权限
    denied_permissions: std::collections::HashSet<String>,
    /// 当前会话的临时权限
    session_permissions: std::collections::HashSet<String>,
}

impl PermissionManager {
    /// 创建新的权限管理器
    pub fn new() -> Self {
        Self {
            granted_permissions: std::collections::HashSet::new(),
            denied_permissions: std::collections::HashSet::new(),
            session_permissions: std::collections::HashSet::new(),
        }
    }

    /// 检查是否有权限
    pub fn has_permission(&self, permission: &str) -> bool {
        self.granted_permissions.contains(permission)
            || self.session_permissions.contains(permission)
    }

    /// 授予权限
    pub fn grant_permission(&mut self, permission: &str, permanent: bool) {
        if permanent {
            self.granted_permissions.insert(permission.to_string());
        } else {
            self.session_permissions.insert(permission.to_string());
        }
        self.denied_permissions.remove(permission);
    }

    /// 拒绝权限
    pub fn deny_permission(&mut self, permission: &str) {
        self.denied_permissions.insert(permission.to_string());
        self.granted_permissions.remove(permission);
        self.session_permissions.remove(permission);
    }

    /// 撤销权限
    pub fn revoke_permission(&mut self, permission: &str) {
        self.granted_permissions.remove(permission);
        self.session_permissions.remove(permission);
    }

    /// 清除会话权限
    pub fn clear_session_permissions(&mut self) {
        self.session_permissions.clear();
    }

    /// 获取权限标识
    pub fn get_permission_key(tool: &str, action: &str) -> String {
        format!("{}:{}", tool, action)
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::slash_commands::CommandParser;

    #[test]
    fn test_dialog_creation() {
        let dialog = ConfirmationDialog::new();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn test_dialog_show_hide() {
        let mut dialog = ConfirmationDialog::new();
        let parser = CommandParser::new();
        let cmd = parser.parse("/bash ls").unwrap();

        dialog.show(cmd, "执行命令", vec!["ls".to_string()]);
        assert!(dialog.is_visible());

        dialog.hide();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn test_button_selection() {
        let mut dialog = ConfirmationDialog::new();

        assert!(dialog.is_confirm_selected());

        dialog.select_next();
        assert!(!dialog.is_confirm_selected());

        dialog.select_prev();
        assert!(dialog.is_confirm_selected());
    }

    #[test]
    fn test_key_handling() {
        let mut dialog = ConfirmationDialog::new();

        let action = dialog.handle_key(crossterm::event::KeyCode::Left);
        assert_eq!(action, ConfirmAction::None);
        assert!(dialog.is_confirm_selected());

        let action = dialog.handle_key(crossterm::event::KeyCode::Right);
        assert_eq!(action, ConfirmAction::None);
        assert!(!dialog.is_confirm_selected());

        let action = dialog.handle_key(crossterm::event::KeyCode::Char('y'));
        assert_eq!(action, ConfirmAction::Confirmed);

        let action = dialog.handle_key(crossterm::event::KeyCode::Char('n'));
        assert_eq!(action, ConfirmAction::Cancelled);

        let action = dialog.handle_key(crossterm::event::KeyCode::Esc);
        assert_eq!(action, ConfirmAction::Cancelled);
    }

    #[test]
    fn test_permission_manager() {
        let mut manager = PermissionManager::new();

        assert!(!manager.has_permission("bash:execute"));

        manager.grant_permission("bash:execute", false);
        assert!(manager.has_permission("bash:execute"));

        manager.deny_permission("bash:execute");
        assert!(!manager.has_permission("bash:execute"));

        manager.grant_permission("bash:execute", true);
        assert!(manager.has_permission("bash:execute"));

        manager.revoke_permission("bash:execute");
        assert!(!manager.has_permission("bash:execute"));
    }

    #[test]
    fn test_permission_key() {
        let key = PermissionManager::get_permission_key("bash", "execute");
        assert_eq!(key, "bash:execute");
    }

    #[test]
    fn test_clear_session_permissions() {
        let mut manager = PermissionManager::new();

        manager.grant_permission("temp:action", false);
        manager.grant_permission("perm:action", true);

        manager.clear_session_permissions();

        assert!(!manager.has_permission("temp:action"));
        assert!(manager.has_permission("perm:action"));
    }
}
