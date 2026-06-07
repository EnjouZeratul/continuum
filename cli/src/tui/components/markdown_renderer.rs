//! Markdown 渲染模块
//!
//! 解析 Markdown 并转换为 TUI 可显示的格式

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::collections::HashMap;

use super::color_theme::ColorTheme;
use super::syntax_highlight::{HighlightedLine, HighlightedSpan, SyntaxHighlighter};

/// Markdown 渲染器
pub struct MarkdownRenderer {
    /// 当前主题
    theme: ColorTheme,
    /// 语法高亮器
    highlighter: SyntaxHighlighter,
    /// 代码块当前语言
    current_code_lang: Option<String>,
    /// 代码块内容累积
    code_buffer: String,
    /// 列表层级计数器
    list_depth: usize,
    /// 列表项序号
    list_counters: HashMap<usize, usize>,
    /// 是否在代码块中
    in_code_block: bool,
}

impl MarkdownRenderer {
    /// 创建新的渲染器
    pub fn new(theme: ColorTheme) -> Self {
        Self {
            theme,
            highlighter: SyntaxHighlighter::new(),
            current_code_lang: None,
            code_buffer: String::new(),
            list_depth: 0,
            list_counters: HashMap::new(),
            in_code_block: false,
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.highlighter.set_theme(&theme.name);
        self.theme = theme;
    }

    /// 渲染 Markdown 文本为 TUI Lines
    pub fn render(&mut self, markdown: &str) -> Vec<Line<'static>> {
        let parser = Parser::new(markdown);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut current_line_spans: Vec<Span<'static>> = Vec::new();

        for event in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Heading { level, .. } => {
                            if !current_line_spans.is_empty() {
                                lines.push(Line::from(current_line_spans.clone()));
                                current_line_spans.clear();
                            }
                            self.render_heading_start(&mut current_line_spans, level);
                        }
                        Tag::Paragraph if !lines.is_empty() && !self.in_code_block => {
                            lines.push(Line::from(Span::raw("")));
                        }
                        Tag::CodeBlock(kind) => {
                            self.in_code_block = true;
                            self.code_buffer.clear();
                            self.current_code_lang = match kind {
                                CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                                CodeBlockKind::Indented => None,
                            };
                            if !current_line_spans.is_empty() {
                                lines.push(Line::from(current_line_spans.clone()));
                                current_line_spans.clear();
                            }
                        }
                        Tag::List(_) => {
                            self.list_depth += 1;
                            self.list_counters.insert(self.list_depth, 1);
                        }
                        Tag::Item => {
                            if !current_line_spans.is_empty() {
                                lines.push(Line::from(current_line_spans.clone()));
                                current_line_spans.clear();
                            }
                            self.render_list_item_prefix(&mut current_line_spans);
                        }
                        Tag::Emphasis => {
                            current_line_spans.push(Span::styled(
                                "",
                                Style::default().add_modifier(Modifier::ITALIC),
                            ));
                        }
                        Tag::Strong => {
                            current_line_spans.push(Span::styled(
                                "",
                                Style::default().add_modifier(Modifier::BOLD),
                            ));
                        }
                        Tag::Strikethrough => {
                            current_line_spans.push(Span::styled(
                                "",
                                Style::default().add_modifier(Modifier::DIM),
                            ));
                        }
                        Tag::Link { dest_url, .. } => {
                            let link_text = format!("[{}] ", dest_url);
                            current_line_spans.push(Span::styled(
                                link_text,
                                Style::default().fg(self.theme.type_name),
                            ));
                        }
                        Tag::BlockQuote(_) => {
                            if !current_line_spans.is_empty() {
                                lines.push(Line::from(current_line_spans.clone()));
                                current_line_spans.clear();
                            }
                            current_line_spans
                                .push(Span::styled("│ ", Style::default().fg(self.theme.border)));
                        }
                        Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => {
                            // Table support - simplified
                        }
                        Tag::FootnoteDefinition(_) | Tag::MetadataBlock(_) => {}
                        _ => {}
                    }
                }
                Event::End(tag_end) => match tag_end {
                    TagEnd::Heading(_) => {
                        lines.push(Line::from(current_line_spans.clone()));
                        current_line_spans.clear();
                    }
                    TagEnd::Paragraph if !current_line_spans.is_empty() => {
                        lines.push(Line::from(current_line_spans.clone()));
                        current_line_spans.clear();
                    }
                    TagEnd::CodeBlock => {
                        self.in_code_block = false;
                        let code_lines = self.render_code_block();
                        for line in code_lines {
                            lines.push(line);
                        }
                        lines.push(Line::from(Span::raw("")));
                        self.code_buffer.clear();
                        self.current_code_lang = None;
                    }
                    TagEnd::List(_) => {
                        self.list_counters.remove(&self.list_depth);
                        self.list_depth = self.list_depth.saturating_sub(1);
                    }
                    TagEnd::Item if !current_line_spans.is_empty() => {
                        lines.push(Line::from(current_line_spans.clone()));
                        current_line_spans.clear();
                    }
                    TagEnd::BlockQuote if !current_line_spans.is_empty() => {
                        lines.push(Line::from(current_line_spans.clone()));
                        current_line_spans.clear();
                    }
                    TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {}
                    _ => {}
                },
                Event::Text(text) => {
                    if self.in_code_block {
                        self.code_buffer.push_str(&text);
                    } else {
                        let style = self.get_current_text_style(&current_line_spans);
                        current_line_spans.push(Span::styled(text.to_string(), style));
                    }
                }
                Event::Code(text) => {
                    let style = Style::default()
                        .fg(self.theme.function)
                        .bg(self.theme.selection_bg);
                    current_line_spans.push(Span::styled(text.to_string(), style));
                }
                Event::SoftBreak => {
                    current_line_spans.push(Span::raw(" "));
                }
                Event::HardBreak => {
                    lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                }
                Event::Html(html) => {
                    current_line_spans.push(Span::styled(
                        html.to_string(),
                        Style::default().fg(self.theme.comment),
                    ));
                }
                Event::FootnoteReference(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {}
                _ => {}
            }
        }

        if !current_line_spans.is_empty() {
            lines.push(Line::from(current_line_spans));
        }

        lines
    }

    /// 渲染标题前缀
    fn render_heading_start(&self, spans: &mut Vec<Span<'static>>, level: HeadingLevel) {
        let (prefix, style) = match level {
            HeadingLevel::H1 => (
                "# ",
                Style::default()
                    .fg(self.theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            HeadingLevel::H2 => (
                "## ",
                Style::default()
                    .fg(self.theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            HeadingLevel::H3 => ("### ", Style::default().fg(self.theme.highlight)),
            HeadingLevel::H4 => ("#### ", Style::default().fg(self.theme.highlight)),
            HeadingLevel::H5 => ("##### ", Style::default().fg(self.theme.assistant_message)),
            HeadingLevel::H6 => ("###### ", Style::default().fg(self.theme.assistant_message)),
        };
        spans.push(Span::styled(prefix, style));
    }

    /// 渲染列表项前缀
    fn render_list_item_prefix(&self, spans: &mut Vec<Span<'static>>) {
        let indent = "  ".repeat(self.list_depth.saturating_sub(1));
        let prefix = if let Some(counter) = self.list_counters.get(&self.list_depth) {
            format!("{}. ", counter)
        } else {
            "• ".to_string()
        };
        spans.push(Span::styled(
            format!("{}{}", indent, prefix),
            Style::default().fg(self.theme.punctuation),
        ));
    }

    /// 渲染代码块
    fn render_code_block(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let language = self.current_code_lang.as_deref().unwrap_or("text");

        // 添加语言标签头
        if let Some(lang) = &self.current_code_lang {
            lines.push(Line::from(vec![
                Span::styled("┌─ ", Style::default().fg(self.theme.border)),
                Span::styled(lang.clone(), Style::default().fg(self.theme.type_name)),
                Span::styled(" ─", Style::default().fg(self.theme.border)),
            ]));
        }

        // 语法高亮
        let highlighted = self.highlighter.highlight(&self.code_buffer, language);

        // 行号宽度计算
        let line_count = highlighted.len();
        let line_num_width = line_count.to_string().len().max(1);

        for (idx, line) in highlighted.iter().enumerate() {
            let mut spans: Vec<Span<'static>> = Vec::new();

            // 左边界
            spans.push(Span::styled("│ ", Style::default().fg(self.theme.border)));

            // 行号
            let line_num = format!("{:>width$} ", idx + 1, width = line_num_width);
            spans.push(Span::styled(
                line_num,
                Style::default().fg(self.theme.comment),
            ));

            // 代码内容
            for span in &line.spans {
                let mut style = Style::default().fg(span.color);
                if span.is_bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if span.is_italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if span.is_underline {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                spans.push(Span::styled(span.text.clone(), style));
            }

            lines.push(Line::from(spans));
        }

        // 底边界
        lines.push(Line::from(Span::styled(
            "└───",
            Style::default().fg(self.theme.border),
        )));

        lines
    }

    /// 获取当前文本样式（考虑斜体、粗体等）
    fn get_current_text_style(&self, spans: &[Span<'static>]) -> Style {
        let mut style = Style::default().fg(self.theme.foreground);

        for span in spans {
            let span_modifiers = span.style.add_modifier;
            if span_modifiers.contains(Modifier::BOLD) {
                style = style.add_modifier(Modifier::BOLD);
            }
            if span_modifiers.contains(Modifier::ITALIC) {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if span_modifiers.contains(Modifier::UNDERLINED) {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
        }

        style
    }

    /// 渲染纯文本（无样式）
    pub fn render_plain(markdown: &str) -> String {
        let parser = Parser::new(markdown);
        let mut result = String::new();

        for event in parser {
            if let Event::Text(text) = event {
                result.push_str(&text);
            }
        }

        result
    }

    /// 快捷渲染方法（创建临时渲染器）
    pub fn render_quick(markdown: &str, theme: &ColorTheme) -> Vec<Line<'static>> {
        let mut renderer = MarkdownRenderer::new(theme.clone());
        renderer.render(markdown)
    }
}

/// 将 HighlightedSpan 转换为 ratatui Span
pub fn highlighted_span_to_span(span: &HighlightedSpan) -> Span<'static> {
    let mut style = Style::default().fg(span.color);
    if span.is_bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if span.is_italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if span.is_underline {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    Span::styled(span.text.clone(), style)
}

/// 将 HighlightedLine 转换为 ratatui Line
pub fn highlighted_line_to_line(line: &HighlightedLine) -> Line<'static> {
    Line::from(
        line.spans
            .iter()
            .map(highlighted_span_to_span)
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_rendering() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("# Hello World");
        assert!(!lines.is_empty());
        assert!(lines[0].spans.iter().any(|s| s.content.contains("#")));
    }

    #[test]
    fn test_paragraph_rendering() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("This is a paragraph.");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_code_block_rendering() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let markdown = "```rust\nfn main() {}\n```";
        let lines = renderer.render(markdown);
        assert!(lines.len() > 3); // Header + code + footer
    }

    #[test]
    fn test_inline_code() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("Use `println!` for output.");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_list_rendering() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("- Item 1\n- Item 2");
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_ordered_list() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("1. First\n2. Second");
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_nested_list() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("- Item 1\n  - Nested");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_bold_text() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("**bold text**");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_italic_text() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("*italic text*");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_link_rendering() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("[Link](https://example.com)");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_blockquote() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let lines = renderer.render("> quoted text");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_plain() {
        let plain = MarkdownRenderer::render_plain("# Hello **World**");
        assert!(plain.contains("Hello"));
        assert!(plain.contains("World"));
    }

    #[test]
    fn test_render_quick() {
        let theme = ColorTheme::dark();
        let lines = MarkdownRenderer::render_quick("Quick test", &theme);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_set_theme() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        renderer.set_theme(ColorTheme::light());
        // Theme should be changed
    }

    #[test]
    fn test_multi_language_code() {
        let theme = ColorTheme::dark();
        let mut renderer = MarkdownRenderer::new(theme);
        let markdown = "```python\nprint('hello')\n```";
        let lines = renderer.render(markdown);
        assert!(!lines.is_empty());
    }
}
