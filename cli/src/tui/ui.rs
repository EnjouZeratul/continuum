//! TUI UI 渲染

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use super::app::App;
use super::components::{
    ChatComponent, InputComponent, KeyHintsComponent, StatusComponent, ToolDisplayComponent,
};

/// UI 渲染上下文，封装所有需要渲染的组件
pub struct RenderContext<'a> {
    pub chat: &'a mut ChatComponent,
    pub input: &'a mut InputComponent,
    pub status: &'a StatusComponent,
    pub tools: &'a ToolDisplayComponent,
    pub key_hints: &'a KeyHintsComponent,
}

/// 渲染 TUI 界面
pub fn render(f: &mut Frame, _app: &App, ctx: RenderContext<'_>, show_tools: bool) {
    // 工具显示的高度（折叠模式为1行统计）
    let tools_height = if show_tools { 4 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),            // 状态栏
            Constraint::Min(1),               // 聊天区域
            Constraint::Length(tools_height), // 工具显示（动态）
            Constraint::Length(3),            // 输入区域
            Constraint::Length(1),            // 快捷键提示栏
        ])
        .split(f.area());

    // 渲染状态栏
    render_status(f, chunks[0], ctx.status);

    // 渲染聊天区域
    render_chat(f, chunks[1], ctx.chat);

    // 工具显示区域
    if show_tools {
        render_tools_collapsed(f, chunks[2], ctx.tools);
    }

    // 渲染输入区域
    render_input(f, chunks[3], ctx.input);

    // 渲染快捷键提示栏
    render_key_hints(f, chunks[4], ctx.key_hints);
}

/// 渲染状态栏
fn render_status(f: &mut Frame, area: Rect, status: &StatusComponent) {
    status.render(f, area);
}

/// 渲染聊天区域
fn render_chat(f: &mut Frame, area: Rect, chat: &mut ChatComponent) {
    chat.render(f, area);
}

/// 渲染输入区域
fn render_input(f: &mut Frame, area: Rect, input: &mut InputComponent) {
    input.render(f, area);
}

/// 渲染工具显示（折叠模式）
fn render_tools_collapsed(f: &mut Frame, area: Rect, tools: &ToolDisplayComponent) {
    tools.render(f, area, true);
}

/// 渲染工具显示（展开模式）
pub fn render_tools_expanded(f: &mut Frame, area: Rect, tools: &ToolDisplayComponent) {
    tools.render(f, area, false);
}

/// 渲染快捷键提示栏
fn render_key_hints(f: &mut Frame, area: Rect, key_hints: &KeyHintsComponent) {
    key_hints.render(f, area);
}
