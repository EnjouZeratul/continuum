//! TUI 快捷键提示组件

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// 快捷键定义
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// 按键描述
    pub key: String,
    /// 功能描述
    pub action: String,
    /// 分组
    pub group: KeyGroup,
}

/// 快捷键分组
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyGroup {
    /// 基础操作
    Basic,
    /// 导航
    Navigation,
    /// 编辑
    Editing,
    /// 会话
    Session,
    /// 工具
    Tools,
}

/// 快捷键提示组件
pub struct KeyHintsComponent {
    /// 当前显示的分组
    active_groups: Vec<KeyGroup>,
    /// 是否显示完整列表
    expanded: bool,
    /// 上下文模式（根据当前状态自动调整显示的快捷键）
    context: HintContext,
}

/// 提示上下文
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintContext {
    /// 普通模式
    Normal,
    /// 输入模式
    Input,
    /// 处理中
    Processing,
    /// 工具面板打开
    ToolsVisible,
}

impl KeyHintsComponent {
    /// 创建新的快捷键提示组件
    pub fn new() -> Self {
        Self {
            active_groups: vec![KeyGroup::Basic, KeyGroup::Session],
            expanded: false,
            context: HintContext::Normal,
        }
    }

    /// 设置上下文
    pub fn set_context(&mut self, context: HintContext) {
        self.context = context;
        self.update_groups();
    }

    /// 更新显示分组
    fn update_groups(&mut self) {
        match self.context {
            HintContext::Normal => {
                self.active_groups = vec![KeyGroup::Basic, KeyGroup::Session];
            }
            HintContext::Input => {
                self.active_groups = vec![KeyGroup::Basic, KeyGroup::Editing];
            }
            HintContext::Processing => {
                self.active_groups = vec![KeyGroup::Basic];
            }
            HintContext::ToolsVisible => {
                self.active_groups = vec![KeyGroup::Basic, KeyGroup::Tools];
            }
        }
    }

    /// 设置展开状态
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// 切换展开状态
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// 获取所有快捷键
    fn all_bindings() -> Vec<KeyBinding> {
        vec![
            // 基础操作
            KeyBinding {
                key: "Ctrl+C".to_string(),
                action: "Quit".to_string(),
                group: KeyGroup::Basic,
            },
            KeyBinding {
                key: "Ctrl+D".to_string(),
                action: "Quit".to_string(),
                group: KeyGroup::Basic,
            },
            KeyBinding {
                key: "Enter".to_string(),
                action: "Send".to_string(),
                group: KeyGroup::Basic,
            },
            KeyBinding {
                key: "Esc".to_string(),
                action: "Cancel/Clear".to_string(),
                group: KeyGroup::Basic,
            },
            KeyBinding {
                key: "Ctrl+H".to_string(),
                action: "Help".to_string(),
                group: KeyGroup::Basic,
            },
            KeyBinding {
                key: "F1".to_string(),
                action: "Key Hints".to_string(),
                group: KeyGroup::Basic,
            },
            // 导航
            KeyBinding {
                key: "↑/↓".to_string(),
                action: "Scroll/History".to_string(),
                group: KeyGroup::Navigation,
            },
            KeyBinding {
                key: "PgUp/PgDn".to_string(),
                action: "Page Scroll".to_string(),
                group: KeyGroup::Navigation,
            },
            KeyBinding {
                key: "Home/End".to_string(),
                action: "Cursor Start/End".to_string(),
                group: KeyGroup::Navigation,
            },
            // 编辑
            KeyBinding {
                key: "Ctrl+W".to_string(),
                action: "Delete Word".to_string(),
                group: KeyGroup::Editing,
            },
            KeyBinding {
                key: "Ctrl+A".to_string(),
                action: "Cursor Start".to_string(),
                group: KeyGroup::Editing,
            },
            KeyBinding {
                key: "Ctrl+E".to_string(),
                action: "Cursor End".to_string(),
                group: KeyGroup::Editing,
            },
            KeyBinding {
                key: "Alt+B".to_string(),
                action: "Word Left".to_string(),
                group: KeyGroup::Editing,
            },
            KeyBinding {
                key: "Alt+F".to_string(),
                action: "Word Right".to_string(),
                group: KeyGroup::Editing,
            },
            KeyBinding {
                key: "Alt+Enter".to_string(),
                action: "New Line".to_string(),
                group: KeyGroup::Editing,
            },
            KeyBinding {
                key: "Tab".to_string(),
                action: "Complete".to_string(),
                group: KeyGroup::Editing,
            },
            // 会话
            KeyBinding {
                key: "Ctrl+N".to_string(),
                action: "New Session".to_string(),
                group: KeyGroup::Session,
            },
            KeyBinding {
                key: "Ctrl+S".to_string(),
                action: "Save Session".to_string(),
                group: KeyGroup::Session,
            },
            KeyBinding {
                key: "Ctrl+L".to_string(),
                action: "Clear Screen".to_string(),
                group: KeyGroup::Session,
            },
            // 工具
            KeyBinding {
                key: "Ctrl+T".to_string(),
                action: "Toggle Tools".to_string(),
                group: KeyGroup::Tools,
            },
        ]
    }

    /// 获取当前显示的快捷键
    fn visible_bindings(&self) -> Vec<KeyBinding> {
        let all = Self::all_bindings();
        if self.expanded {
            all
        } else {
            all.into_iter()
                .filter(|b| self.active_groups.contains(&b.group))
                .collect()
        }
    }

    /// 渲染组件
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let bindings = self.visible_bindings();

        // 根据宽度决定显示方式
        let available_width = area.width as usize;

        // 构建快捷键文本
        let spans: Vec<Span> = if available_width > 80 {
            // 宽屏：分组显示
            self.render_compact(&bindings)
        } else {
            // 窄屏：紧凑显示
            self.render_minimal(&bindings, available_width)
        };

        let hint_line = Line::from(spans);

        let paragraph = Paragraph::new(hint_line)
            .block(Block::default().borders(Borders::NONE))
            .style(Style::default().fg(Color::DarkGray));

        f.render_widget(paragraph, area);
    }

    /// 紧凑渲染（宽屏）
    fn render_compact(&self, bindings: &[KeyBinding]) -> Vec<Span<'_>> {
        let mut spans = vec![Span::styled(" ", Style::default())];

        for (i, binding) in bindings.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
            }

            // 按键高亮
            spans.push(Span::styled(
                binding.key.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

            spans.push(Span::styled(": ", Style::default().fg(Color::DarkGray)));

            // 功能描述
            spans.push(Span::styled(
                binding.action.clone(),
                Style::default().fg(Color::Gray),
            ));
        }

        // 如果未展开，显示展���提示
        if !self.expanded {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                "[F1/Ctrl+? More]",
                Style::default().fg(Color::Cyan),
            ));
        }

        spans
    }

    /// 最小渲染（窄屏）
    fn render_minimal(&self, bindings: &[KeyBinding], width: usize) -> Vec<Span<'_>> {
        let mut spans = vec![Span::styled(" ", Style::default())];
        let mut current_width = 1;

        // 只显示最重要的快捷键
        let priority_keys = ["Ctrl+C", "Enter", "Esc", "Ctrl+N"];
        let filtered: Vec<_> = bindings
            .iter()
            .filter(|b| priority_keys.contains(&b.key.as_str()))
            .collect();

        for binding in filtered {
            let item_width = binding.key.len() + binding.action.len() + 3;
            if current_width + item_width > width {
                break;
            }

            if current_width > 1 {
                spans.push(Span::styled(" ", Style::default()));
            }

            spans.push(Span::styled(
                binding.key.clone(),
                Style::default().fg(Color::Yellow),
            ));
            spans.push(Span::styled(
                format!(" {}", binding.action),
                Style::default().fg(Color::Gray),
            ));

            current_width += item_width;
        }

        spans
    }
}

impl Default for KeyHintsComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_hints_creation() {
        let hints = KeyHintsComponent::new();
        assert!(!hints.expanded);
        assert_eq!(hints.context, HintContext::Normal);
    }

    #[test]
    fn test_set_context() {
        let mut hints = KeyHintsComponent::new();
        hints.set_context(HintContext::Input);
        assert_eq!(hints.context, HintContext::Input);
        assert!(hints.active_groups.contains(&KeyGroup::Editing));
    }

    #[test]
    fn test_set_expanded() {
        let mut hints = KeyHintsComponent::new();
        hints.set_expanded(true);
        assert!(hints.expanded);
    }

    #[test]
    fn test_toggle_expanded() {
        let mut hints = KeyHintsComponent::new();
        assert!(!hints.expanded);
        hints.toggle_expanded();
        assert!(hints.expanded);
        hints.toggle_expanded();
        assert!(!hints.expanded);
    }

    #[test]
    fn test_all_bindings_count() {
        let bindings = KeyHintsComponent::all_bindings();
        assert!(bindings.len() >= 15);
    }

    #[test]
    fn test_visible_bindings_filters_by_group() {
        let hints = KeyHintsComponent::new();
        let visible = hints.visible_bindings();
        // 默认只显示 Basic 和 Session 分组
        for binding in &visible {
            assert!(binding.group == KeyGroup::Basic || binding.group == KeyGroup::Session);
        }
    }

    #[test]
    fn test_expanded_shows_all() {
        let mut hints = KeyHintsComponent::new();
        hints.set_expanded(true);
        let visible = hints.visible_bindings();
        let all = KeyHintsComponent::all_bindings();
        assert_eq!(visible.len(), all.len());
    }

    #[test]
    fn test_context_processing() {
        let mut hints = KeyHintsComponent::new();
        hints.set_context(HintContext::Processing);
        let visible = hints.visible_bindings();
        // 处理中只显示 Basic（主要是 Quit）
        for binding in &visible {
            assert_eq!(binding.group, KeyGroup::Basic);
        }
    }

    #[test]
    fn test_default() {
        let hints = KeyHintsComponent::default();
        assert_eq!(hints.context, HintContext::Normal);
    }
}
