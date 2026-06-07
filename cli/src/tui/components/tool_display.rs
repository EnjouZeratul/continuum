//! TUI 工具执行显示组件

use super::color_theme::ColorTheme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::time::{Duration, Instant};

/// 工具调用状态
#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 执行成功
    Success,
    /// 执行失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 工具调用记录
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// 工具名称
    pub name: String,
    /// 调用参数
    pub arguments: String,
    /// 执行状态
    pub status: ToolStatus,
    /// 执行结果
    pub result: Option<String>,
    /// 开始时间
    pub started_at: Instant,
    /// 执行耗时
    pub duration: Option<Duration>,
    /// 进度百分比 (0.0 - 100.0)
    pub progress: f32,
}

impl ToolCall {
    /// 创建新的工具调用记录
    pub fn new(name: String, arguments: String) -> Self {
        Self {
            name,
            arguments,
            status: ToolStatus::Pending,
            result: None,
            started_at: Instant::now(),
            duration: None,
            progress: 0.0,
        }
    }

    /// 标记为运行中
    pub fn mark_running(&mut self) {
        self.status = ToolStatus::Running;
    }

    /// 标记为完成
    pub fn mark_completed(&mut self, result: String, is_error: bool) {
        self.status = if is_error {
            ToolStatus::Failed
        } else {
            ToolStatus::Success
        };
        self.result = Some(result);
        self.duration = Some(self.started_at.elapsed());
    }
}

/// 工具执行显示组件
pub struct ToolDisplayComponent {
    /// 工具调用列表
    pub tool_calls: Vec<ToolCall>,
    /// 是否显示详细参数
    verbose: bool,
    /// 最大显示数量
    max_display: usize,
    /// 颜色主题
    theme: ColorTheme,
}

impl ToolDisplayComponent {
    /// 创建新的组件
    pub fn new() -> Self {
        Self {
            tool_calls: Vec::new(),
            verbose: false,
            max_display: 10,
            theme: ColorTheme::dark(),
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.theme = theme;
    }

    /// 添加工具调用
    pub fn add_tool_call(&mut self, name: String, arguments: String) -> usize {
        let call = ToolCall::new(name, arguments);
        self.tool_calls.push(call);

        // 保持最大显示数量
        if self.tool_calls.len() > self.max_display * 2 {
            self.tool_calls.drain(0..self.max_display);
        }

        self.tool_calls.len() - 1
    }

    /// 更新工具状态为运行中
    pub fn set_running(&mut self, index: usize) {
        if let Some(call) = self.tool_calls.get_mut(index) {
            call.mark_running();
        }
    }

    /// 完成工具调用
    pub fn complete_tool_call(&mut self, index: usize, result: String, is_error: bool) {
        if let Some(call) = self.tool_calls.get_mut(index) {
            call.mark_completed(result, is_error);
        }
    }

    /// Get elapsed time for a running tool
    pub fn get_elapsed_time(&self, index: usize) -> Option<String> {
        self.tool_calls
            .get(index)
            .map(|call| format_duration_short(call.started_at.elapsed()))
    }

    /// Get progress percentage for a tool
    pub fn get_progress(&self, index: usize) -> Option<f32> {
        self.tool_calls.get(index).map(|call| call.progress)
    }

    /// Set progress percentage for a tool
    pub fn set_progress(&mut self, index: usize, progress: f32) {
        if let Some(call) = self.tool_calls.get_mut(index) {
            call.progress = progress.clamp(0.0, 100.0);
        }
    }

    /// Render progress bar string
    pub fn render_progress_bar(&self, index: usize, width: usize) -> String {
        if let Some(call) = self.tool_calls.get(index) {
            let filled = (call.progress / 100.0 * width as f32).round() as usize;
            let empty = width.saturating_sub(filled);
            format!(
                "[{}{}] {:.0}%",
                "█".repeat(filled),
                "░".repeat(empty),
                call.progress
            )
        } else {
            format!("[{}] 0%", "░".repeat(width))
        }
    }

    /// Cancel a running tool
    pub fn cancel_tool(&mut self, index: usize) -> bool {
        if let Some(call) = self.tool_calls.get_mut(index) {
            if call.status == ToolStatus::Running {
                call.status = ToolStatus::Cancelled;
                call.duration = Some(call.started_at.elapsed());
                call.result = Some("Cancelled by user".to_string());
                return true;
            }
        }
        false
    }

    /// Get spinner animation frame
    pub fn get_spinner_frame(&self, _index: usize, frame: usize) -> &'static str {
        const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        SPINNER_FRAMES[frame % SPINNER_FRAMES.len()]
    }

    /// 设置详细模式
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// 清空调用记录
    pub fn clear(&mut self) {
        self.tool_calls.clear();
    }

    /// 获取调用数量
    pub fn count(&self) -> usize {
        self.tool_calls.len()
    }

    /// 获取成功数量
    pub fn success_count(&self) -> usize {
        self.tool_calls
            .iter()
            .filter(|c| c.status == ToolStatus::Success)
            .count()
    }

    /// 获取失败数量
    pub fn failed_count(&self) -> usize {
        self.tool_calls
            .iter()
            .filter(|c| c.status == ToolStatus::Failed)
            .count()
    }

    /// 渲染组件
    pub fn render(&self, f: &mut Frame, area: Rect, collapsed: bool) {
        if collapsed {
            // 折叠模式：只显示统计
            let running = self
                .tool_calls
                .iter()
                .filter(|c| c.status == ToolStatus::Running)
                .count();
            let success = self.success_count();
            let failed = self.failed_count();

            let status_line = if running > 0 {
                format!(
                    "🔧 Tools: {} running, {} success, {} failed",
                    running, success, failed
                )
            } else {
                format!(
                    "🔧 Tools: {} calls ({} success, {} failed)",
                    self.count(),
                    success,
                    failed
                )
            };

            let paragraph =
                Paragraph::new(status_line).style(Style::default().fg(self.theme.tool_running));
            f.render_widget(paragraph, area);
            return;
        }

        // 展开模式：显示工具列表
        let items: Vec<ListItem> = self
            .tool_calls
            .iter()
            .rev()
            .take(self.max_display)
            .map(|call| {
                let (status_icon, status_color) = match &call.status {
                    ToolStatus::Pending => ("⏳", self.theme.tool_pending),
                    ToolStatus::Running => ("🔄", self.theme.tool_running),
                    ToolStatus::Success => ("✅", self.theme.tool_success),
                    ToolStatus::Failed => ("❌", self.theme.tool_failed),
                    ToolStatus::Cancelled => ("⏹", self.theme.tool_pending),
                };

                // For running tools, show elapsed time and progress
                let elapsed_str = if call.status == ToolStatus::Running {
                    format_duration_short(call.started_at.elapsed())
                } else {
                    call.duration
                        .map(|d| format_duration_short(d))
                        .unwrap_or_default()
                };

                let mut spans = vec![
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(&call.name, Style::default().fg(self.theme.function)),
                ];

                // Show progress bar for running tools
                if call.status == ToolStatus::Running && call.progress > 0.0 {
                    spans.push(Span::raw(" "));
                    let progress_bar = format!(
                        "[{}{}] {:.0}%",
                        "█".repeat((call.progress / 10.0) as usize),
                        "░".repeat(10 - (call.progress / 10.0) as usize),
                        call.progress
                    );
                    spans.push(Span::styled(
                        progress_bar,
                        Style::default().fg(self.theme.highlight),
                    ));
                }

                if self.verbose && !call.arguments.is_empty() {
                    spans.push(Span::raw(" "));
                    let args_preview = if call.arguments.len() > 50 {
                        format!("{}...", &call.arguments[..47])
                    } else {
                        call.arguments.clone()
                    };
                    spans.push(Span::styled(
                        args_preview,
                        Style::default().fg(self.theme.comment),
                    ));
                }

                if !elapsed_str.is_empty() {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("[{}]", elapsed_str),
                        Style::default().fg(self.theme.token_count),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let block = Block::default()
            .title(format!(" Tools ({}) ", self.count()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border));

        let list = List::new(items).block(block);
        f.render_widget(list, area);
    }

    /// 获取最后一个工具调用
    pub fn last_call(&self) -> Option<&ToolCall> {
        self.tool_calls.last()
    }

    /// 检查是否有正在运行的工具
    pub fn has_running(&self) -> bool {
        self.tool_calls
            .iter()
            .any(|c| c.status == ToolStatus::Running)
    }
}

impl Default for ToolDisplayComponent {
    fn default() -> Self {
        Self::new()
    }
}

/// Format duration in a short human-readable format
fn format_duration_short(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let millis = duration.as_millis();

    if total_secs >= 60 {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{}m {}s", mins, secs)
    } else if millis >= 500 {
        // Show as seconds with decimal for >= 500ms
        let secs = millis as f64 / 1000.0;
        format!("{}s", secs)
    } else {
        format!("{}ms", millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_display_creation() {
        let display = ToolDisplayComponent::new();
        assert_eq!(display.count(), 0);
    }

    #[test]
    fn test_add_tool_call() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test_tool".to_string(), "{}".to_string());
        assert_eq!(idx, 0);
        assert_eq!(display.count(), 1);
    }

    #[test]
    fn test_set_running() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);
        let call = display.last_call().unwrap();
        assert_eq!(call.status, ToolStatus::Running);
    }

    #[test]
    fn test_complete_tool_call() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.complete_tool_call(idx, "result".to_string(), false);
        let call = display.last_call().unwrap();
        assert_eq!(call.status, ToolStatus::Success);
        assert_eq!(call.result, Some("result".to_string()));
    }

    #[test]
    fn test_complete_with_error() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.complete_tool_call(idx, "error msg".to_string(), true);
        let call = display.last_call().unwrap();
        assert_eq!(call.status, ToolStatus::Failed);
    }

    #[test]
    fn test_counts() {
        let mut display = ToolDisplayComponent::new();
        display.add_tool_call("a".to_string(), "{}".to_string());
        display.add_tool_call("b".to_string(), "{}".to_string());
        display.complete_tool_call(0, "ok".to_string(), false);
        display.complete_tool_call(1, "err".to_string(), true);
        assert_eq!(display.success_count(), 1);
        assert_eq!(display.failed_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut display = ToolDisplayComponent::new();
        display.add_tool_call("test".to_string(), "{}".to_string());
        display.clear();
        assert_eq!(display.count(), 0);
    }

    #[test]
    fn test_has_running() {
        let mut display = ToolDisplayComponent::new();
        display.add_tool_call("test".to_string(), "{}".to_string());
        assert!(!display.has_running());
        display.set_running(0);
        assert!(display.has_running());
        display.complete_tool_call(0, "ok".to_string(), false);
        assert!(!display.has_running());
    }

    #[test]
    fn test_default() {
        let display = ToolDisplayComponent::default();
        assert_eq!(display.count(), 0);
    }

    // ========== Progress Bar Tests ==========

    #[test]
    fn test_elapsed_time_while_running() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);

        // Should return elapsed time string
        let elapsed = display.get_elapsed_time(idx);
        assert!(elapsed.is_some());
        let elapsed_str = elapsed.unwrap();
        assert!(elapsed_str.contains('s') || elapsed_str.contains("ms"));
    }

    #[test]
    fn test_progress_percentage_default() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);

        // Default progress is 0 when not set
        let progress = display.get_progress(idx);
        assert_eq!(progress, Some(0.0f32));
    }

    #[test]
    fn test_set_progress_percentage() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);

        display.set_progress(idx, 50.0);
        assert_eq!(display.get_progress(idx), Some(50.0));

        display.set_progress(idx, 100.0);
        assert_eq!(display.get_progress(idx), Some(100.0));
    }

    #[test]
    fn test_progress_bar_string() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);

        display.set_progress(idx, 50.0);
        let bar = display.render_progress_bar(idx, 10);
        assert_eq!(bar, "[█████░░░░░] 50%");
    }

    #[test]
    fn test_cancel_tool() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);

        // Cancel the tool
        let cancelled = display.cancel_tool(idx);
        assert!(cancelled);

        let call = display.last_call().unwrap();
        assert_eq!(call.status, ToolStatus::Cancelled);
    }

    #[test]
    fn test_spinner_animation() {
        let mut display = ToolDisplayComponent::new();
        let idx = display.add_tool_call("test".to_string(), "{}".to_string());
        display.set_running(idx);

        // Spinner should cycle through frames
        let frame1 = display.get_spinner_frame(idx, 0);
        let frame2 = display.get_spinner_frame(idx, 1);
        assert_ne!(frame1, frame2);
    }

    #[test]
    fn test_format_duration() {
        // Test duration formatting
        assert_eq!(format_duration_short(Duration::from_millis(500)), "0.5s");
        assert_eq!(format_duration_short(Duration::from_millis(1500)), "1.5s");
        assert_eq!(format_duration_short(Duration::from_millis(100)), "100ms");
        assert_eq!(format_duration_short(Duration::from_secs(65)), "1m 5s");
    }
}
