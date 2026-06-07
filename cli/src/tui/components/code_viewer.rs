//! 代码查看器组件
//!
//! 支持代码显示、语法高亮、行号、搜索等功能。

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::color_theme::ColorTheme;

/// 代码查看器组件
pub struct CodeViewerComponent {
    /// 文件路径
    file_path: Option<String>,
    /// 文件内容（按行）
    lines: Vec<String>,
    /// 滚动偏移
    scroll_offset: usize,
    /// 可见行数
    visible_lines: usize,
    /// 光标行
    cursor_line: usize,
    /// 光标列
    cursor_col: usize,
    /// 搜索模式
    #[allow(dead_code)]
    search_mode: bool,
    /// 搜索词
    search_term: String,
    /// 搜索结果
    search_results: Vec<(usize, usize)>, // (line, col)
    /// 当前搜索结果索引
    current_search_idx: usize,
    /// 是否已修改
    modified: bool,
    /// 文件语言
    language: CodeLanguage,
    /// 折叠状态
    folds: HashMap<usize, bool>,
}

/// 支持的代码语言
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodeLanguage {
    Python,
    Rust,
    JavaScript,
    TypeScript,
    Json,
    Toml,
    Markdown,
    Plain,
}

impl CodeLanguage {
    /// 从文件扩展名推断语言
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "py" => CodeLanguage::Python,
            "rs" => CodeLanguage::Rust,
            "js" => CodeLanguage::JavaScript,
            "ts" => CodeLanguage::TypeScript,
            "json" => CodeLanguage::Json,
            "toml" => CodeLanguage::Toml,
            "md" => CodeLanguage::Markdown,
            _ => CodeLanguage::Plain,
        }
    }

    /// 获取语言名称
    pub fn name(&self) -> &str {
        match self {
            CodeLanguage::Python => "Python",
            CodeLanguage::Rust => "Rust",
            CodeLanguage::JavaScript => "JavaScript",
            CodeLanguage::TypeScript => "TypeScript",
            CodeLanguage::Json => "JSON",
            CodeLanguage::Toml => "TOML",
            CodeLanguage::Markdown => "Markdown",
            CodeLanguage::Plain => "Plain Text",
        }
    }

    /// 获取注释前缀
    pub fn comment_prefix(&self) -> &str {
        match self {
            CodeLanguage::Python => "#",
            CodeLanguage::Rust => "//",
            CodeLanguage::JavaScript => "//",
            CodeLanguage::TypeScript => "//",
            CodeLanguage::Json => "",
            CodeLanguage::Toml => "#",
            CodeLanguage::Markdown => "",
            CodeLanguage::Plain => "",
        }
    }
}

/// 语法高亮器
#[derive(Default)]
pub struct SyntaxHighlighter {
    /// 颜色主题
    theme: ColorTheme,
}

impl SyntaxHighlighter {
    /// 使用指定主题创建高亮器
    pub fn with_theme(theme: ColorTheme) -> Self {
        Self { theme }
    }

    /// 获取当前主题
    pub fn theme(&self) -> &ColorTheme {
        &self.theme
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.theme = theme;
    }

    /// 高亮一行代码
    pub fn highlight_line(&self, line: &str, language: CodeLanguage) -> Vec<Span<'_>> {
        let mut spans = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = line.chars().collect();

        while pos < chars.len() {
            // 检查注释
            if self.is_comment_start(&chars, pos, language) {
                let comment: String = chars[pos..].iter().collect();
                spans.push(Span::styled(
                    comment,
                    Style::default().fg(self.theme.comment),
                ));
                break;
            }

            // 检查字符串
            if let Some(end) = self.find_string_end(&chars, pos, language) {
                let s: String = chars[pos..=end].iter().collect();
                spans.push(Span::styled(s, Style::default().fg(self.theme.string)));
                pos = end + 1;
                continue;
            }

            // 检查数字
            if chars[pos].is_ascii_digit()
                || (chars[pos] == '-' && pos + 1 < chars.len() && chars[pos + 1].is_ascii_digit())
            {
                let start = if chars[pos] == '-' { pos + 1 } else { pos };
                let mut end = start;
                while end < chars.len()
                    && (chars[end].is_ascii_digit()
                        || chars[end] == '.'
                        || chars[end] == 'x'
                        || chars[end] == 'b'
                        || chars[end] == 'o'
                        || (end > start && chars[end].is_ascii_hexdigit()))
                {
                    end += 1;
                }
                // Include negative sign if present
                let actual_start = if chars[pos] == '-' { pos } else { start };
                let num: String = chars[actual_start..end].iter().collect();
                spans.push(Span::styled(num, Style::default().fg(self.theme.number)));
                pos = end;
                continue;
            }

            // 检查关键字/标识符
            if chars[pos].is_alphabetic() || chars[pos] == '_' {
                let mut end = pos;
                while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                    end += 1;
                }
                let word: String = chars[pos..end].iter().collect();

                let style = if self.is_keyword(&word, language) {
                    Style::default()
                        .fg(self.theme.keyword)
                        .add_modifier(Modifier::BOLD)
                } else if self.is_type(&word, language) {
                    Style::default().fg(self.theme.type_name)
                } else if self.is_builtin(&word, language)
                    || (end < chars.len() && chars[end] == '(')
                {
                    Style::default().fg(self.theme.function)
                } else if SyntaxHighlighter::is_constant(&word) {
                    Style::default()
                        .fg(self.theme.number)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.theme.variable)
                };

                spans.push(Span::styled(word, style));
                pos = end;
                continue;
            }

            // 检查操作符
            if SyntaxHighlighter::is_operator(chars[pos]) {
                let mut end = pos;
                while end < chars.len() && SyntaxHighlighter::is_operator(chars[end]) {
                    end += 1;
                }
                let op: String = chars[pos..end].iter().collect();
                spans.push(Span::styled(op, Style::default().fg(self.theme.operator)));
                pos = end;
                continue;
            }

            // 检查括号
            if SyntaxHighlighter::is_bracket(chars[pos]) {
                spans.push(Span::styled(
                    chars[pos].to_string(),
                    Style::default().fg(self.theme.bracket),
                ));
                pos += 1;
                continue;
            }

            // 检查标点
            if SyntaxHighlighter::is_punctuation(chars[pos]) {
                spans.push(Span::styled(
                    chars[pos].to_string(),
                    Style::default().fg(self.theme.punctuation),
                ));
                pos += 1;
                continue;
            }

            // 普通字符
            spans.push(Span::raw(chars[pos].to_string()));
            pos += 1;
        }

        spans
    }

    fn is_operator(c: char) -> bool {
        matches!(
            c,
            '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '&' | '|' | '^' | '~' | '!' | '?' | ':'
        )
    }

    fn is_bracket(c: char) -> bool {
        matches!(c, '(' | ')' | '[' | ']' | '{' | '}')
    }

    fn is_punctuation(c: char) -> bool {
        matches!(c, ',' | '.' | ';' | '@' | '#' | '$')
    }

    fn is_constant(word: &str) -> bool {
        // 全大写或已知常量
        word.chars().all(|c| c.is_uppercase() || c == '_')
            || matches!(
                word,
                "None" | "null" | "nil" | "undefined" | "NaN" | "Infinity"
            )
    }

    fn is_builtin(&self, word: &str, language: CodeLanguage) -> bool {
        let builtins = match language {
            CodeLanguage::Python => &[
                "print",
                "len",
                "range",
                "enumerate",
                "zip",
                "map",
                "filter",
                "sorted",
                "reversed",
                "min",
                "max",
                "sum",
                "abs",
                "round",
                "int",
                "float",
                "str",
                "bool",
                "list",
                "dict",
                "set",
                "tuple",
                "open",
                "input",
                "type",
                "isinstance",
                "hasattr",
                "getattr",
                "setattr",
                "delattr",
                "property",
                "staticmethod",
                "classmethod",
                "super",
                "isinstance",
                "issubclass",
                "callable",
                "iter",
                "next",
                "repr",
                "hash",
                "id",
                "dir",
                "vars",
                "locals",
                "globals",
                "exec",
                "eval",
                "compile",
                "__import__",
            ] as &[&str],
            CodeLanguage::Rust => &[
                "println",
                "print",
                "format",
                "vec",
                "String",
                "Box",
                "Rc",
                "Arc",
                "Some",
                "None",
                "Ok",
                "Err",
                "panic",
                "assert",
                "assert_eq",
                "assert_ne",
                "debug_assert",
                "debug_assert_eq",
                "debug_assert_ne",
                "todo",
                "unimplemented",
                "unreachable",
                "cfg",
                "include",
                "include_str",
                "concat",
                "env",
                "option_env",
                "panic",
            ],
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => &[
                "console",
                "log",
                "alert",
                "confirm",
                "prompt",
                "parseInt",
                "parseFloat",
                "Number",
                "String",
                "Boolean",
                "Array",
                "Object",
                "Map",
                "Set",
                "WeakMap",
                "WeakSet",
                "Date",
                "RegExp",
                "Error",
                "TypeError",
                "ReferenceError",
                "JSON",
                "Math",
                "Promise",
                "Symbol",
                "Proxy",
                "Reflect",
                "setTimeout",
                "setInterval",
                "clearTimeout",
                "clearInterval",
                "fetch",
                "require",
                "exports",
                "module",
            ],
            _ => &[],
        };
        builtins.contains(&word)
    }

    fn is_comment_start(&self, chars: &[char], pos: usize, language: CodeLanguage) -> bool {
        match language {
            CodeLanguage::Python | CodeLanguage::Toml => chars[pos] == '#',
            CodeLanguage::Rust | CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '/'
            }
            _ => false,
        }
    }

    fn find_string_end(
        &self,
        chars: &[char],
        pos: usize,
        _language: CodeLanguage,
    ) -> Option<usize> {
        if chars[pos] != '"' && chars[pos] != '\'' {
            return None;
        }
        let quote = chars[pos];
        let mut end = pos + 1;
        while end < chars.len() {
            if chars[end] == '\\' && end + 1 < chars.len() {
                end += 2; // 跳过转义字符
                continue;
            }
            if chars[end] == quote {
                return Some(end);
            }
            end += 1;
        }
        None
    }

    fn is_keyword(&self, word: &str, language: CodeLanguage) -> bool {
        let keywords = match language {
            CodeLanguage::Python => &[
                "def", "class", "if", "else", "elif", "for", "while", "return", "import", "from",
                "as", "try", "except", "finally", "with", "yield", "lambda", "pass", "break",
                "continue", "raise", "True", "False", "None", "and", "or", "not", "in", "is",
                "async", "await", "global", "nonlocal", "assert",
            ] as &[&str],
            CodeLanguage::Rust => &[
                "fn", "let", "mut", "const", "static", "pub", "mod", "use", "struct", "enum",
                "impl", "trait", "type", "where", "for", "loop", "while", "if", "else", "match",
                "return", "break", "continue", "async", "await", "move", "ref", "self", "Self",
                "true", "false", "Some", "None", "Ok", "Err",
            ],
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => &[
                "function",
                "const",
                "let",
                "var",
                "class",
                "extends",
                "import",
                "export",
                "from",
                "if",
                "else",
                "for",
                "while",
                "return",
                "try",
                "catch",
                "finally",
                "throw",
                "async",
                "await",
                "true",
                "false",
                "null",
                "undefined",
                "this",
                "new",
            ],
            _ => &[],
        };
        keywords.contains(&word)
    }

    fn is_type(&self, word: &str, language: CodeLanguage) -> bool {
        let types = match language {
            CodeLanguage::Python => &[
                "int", "float", "str", "bool", "list", "dict", "set", "tuple", "None", "Any",
                "Union", "Optional", "Callable", "Type",
            ] as &[&str],
            CodeLanguage::Rust => &[
                "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128",
                "usize", "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result",
                "Box", "Rc", "Arc",
            ],
            CodeLanguage::TypeScript => &[
                "string",
                "number",
                "boolean",
                "object",
                "any",
                "void",
                "null",
                "undefined",
                "never",
                "unknown",
                "Promise",
            ],
            _ => &[],
        };
        types.contains(&word)
    }
}

impl CodeViewerComponent {
    /// 创建新的代码查看器
    pub fn new() -> Self {
        Self {
            file_path: None,
            lines: Vec::new(),
            scroll_offset: 0,
            visible_lines: 20,
            cursor_line: 0,
            cursor_col: 0,
            search_mode: false,
            search_term: String::new(),
            search_results: Vec::new(),
            current_search_idx: 0,
            modified: false,
            language: CodeLanguage::Plain,
            folds: HashMap::new(),
        }
    }

    /// 加载文件
    pub fn load_file(&mut self, path: &str) -> std::io::Result<()> {
        let content = fs::read_to_string(path)?;

        // 检测二进制文件
        if content.bytes().any(|b| b == 0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Binary file not supported",
            ));
        }

        self.file_path = Some(path.to_string());
        self.lines = content.lines().map(|s| s.to_string()).collect();
        self.scroll_offset = 0;
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.modified = false;

        // 推断语言
        if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
            self.language = CodeLanguage::from_extension(ext);
        }

        Ok(())
    }

    /// 获取文件路径
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    /// 获取总行数
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 滚动
    pub fn scroll(&mut self, delta: i32) {
        if delta > 0 {
            self.scroll_offset = (self.scroll_offset + delta as usize)
                .min(self.lines.len().saturating_sub(self.visible_lines));
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
        }
    }

    /// 向上翻页
    pub fn page_up(&mut self) {
        self.scroll(-(self.visible_lines as i32));
    }

    /// 向下翻页
    pub fn page_down(&mut self) {
        self.scroll(self.visible_lines as i32);
    }

    /// 移动到指定行
    pub fn goto_line(&mut self, line: usize) {
        if line < self.lines.len() {
            self.cursor_line = line;
            // 确保可见
            if line < self.scroll_offset {
                self.scroll_offset = line;
            } else if line >= self.scroll_offset + self.visible_lines {
                self.scroll_offset = line.saturating_sub(self.visible_lines / 2);
            }
        }
    }

    /// 搜索
    pub fn search(&mut self, term: &str) {
        self.search_term = term.to_string();
        self.search_results.clear();
        self.current_search_idx = 0;

        if term.is_empty() {
            return;
        }

        for (line_idx, line) in self.lines.iter().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(term) {
                self.search_results.push((line_idx, start + pos));
                start += pos + term.len();
            }
        }
    }

    /// 下一个搜索结果
    pub fn next_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        self.current_search_idx = (self.current_search_idx + 1) % self.search_results.len();
        let (line, _) = self.search_results[self.current_search_idx];
        self.goto_line(line);
    }

    /// 上一个搜索结果
    pub fn prev_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        self.current_search_idx = if self.current_search_idx == 0 {
            self.search_results.len() - 1
        } else {
            self.current_search_idx - 1
        };
        let (line, _) = self.search_results[self.current_search_idx];
        self.goto_line(line);
    }

    /// 获取搜索结果数量
    pub fn search_result_count(&self) -> usize {
        self.search_results.len()
    }

    /// 获取当前搜索结果索引
    pub fn current_search_index(&self) -> usize {
        self.current_search_idx
    }

    /// 折叠/展开当前行
    pub fn toggle_fold(&mut self) {
        if self.cursor_line < self.lines.len() {
            let folded = self.folds.get(&self.cursor_line).copied().unwrap_or(false);
            self.folds.insert(self.cursor_line, !folded);
        }
    }

    /// 渲染组件
    pub fn render(&self, f: &mut Frame, area: Rect, highlighter: &SyntaxHighlighter) {
        let block = Block::default()
            .title(format!(
                " {} {} {}",
                self.file_path.as_deref().unwrap_or("No file"),
                if self.modified { "[*]" } else { "" },
                if self.search_results.is_empty() {
                    String::new()
                } else {
                    format!(
                        " [{}/{}]",
                        self.current_search_idx + 1,
                        self.search_results.len()
                    )
                }
            ))
            .borders(Borders::ALL);

        let inner = block.inner(area);
        f.render_widget(block, area);

        // 更新可见行数
        let visible_lines = inner.height as usize;
        let visible_lines = visible_lines.max(1);

        // 计算行号宽度
        let line_num_width = (self.lines.len().to_string().len() + 1).max(3);

        // 渲染行
        let mut lines_to_render = Vec::new();
        let end_line = (self.scroll_offset + visible_lines).min(self.lines.len());

        for line_num in self.scroll_offset..end_line {
            if let Some(line_content) = self.lines.get(line_num) {
                let is_current_line = line_num == self.cursor_line;
                let folded = self.folds.get(&line_num).copied().unwrap_or(false);

                // 行号
                let line_num_str = format!("{:width$}", line_num + 1, width = line_num_width);
                let line_num_span = Span::styled(
                    line_num_str,
                    Style::default().fg(Color::DarkGray).bg(if is_current_line {
                        Color::DarkGray
                    } else {
                        Color::Reset
                    }),
                );

                // 代码内容
                let mut code_spans = if folded {
                    vec![Span::styled("...", Style::default().fg(Color::DarkGray))]
                } else {
                    highlighter.highlight_line(line_content, self.language)
                };

                // 搜索高亮
                if !self.search_term.is_empty() {
                    code_spans = self.highlight_search_results(line_num, code_spans);
                }

                let mut spans = vec![line_num_span, Span::raw(" ")];
                spans.extend(code_spans);

                let line_style = if is_current_line {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                lines_to_render.push(Line::from(spans).style(line_style));
            }
        }

        let paragraph = Paragraph::new(lines_to_render);
        f.render_widget(paragraph, inner);

        // 渲染滚动条
        if self.lines.len() > visible_lines {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(self.lines.len())
                .position(self.scroll_offset)
                .viewport_content_length(visible_lines);

            f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    /// 高亮搜索结果
    fn highlight_search_results(&self, _line_num: usize, spans: Vec<Span>) -> Vec<Span<'_>> {
        let mut result = Vec::new();
        let highlight_style = Style::default().bg(Color::Yellow).fg(Color::Black);

        for span in spans {
            let content = span.content.as_ref();
            let style = span.style;

            // 查找所有匹配位置
            let mut last_end = 0;
            let mut start = 0;

            while let Some(pos) = content[start..].find(&self.search_term) {
                let abs_pos = start + pos;

                // 添加匹配前的文本
                if abs_pos > last_end {
                    result.push(Span::styled(content[last_end..abs_pos].to_string(), style));
                }

                // 添加高亮的匹配文本
                result.push(Span::styled(self.search_term.clone(), highlight_style));

                last_end = abs_pos + self.search_term.len();
                start = last_end;
            }

            // 添加剩余文本
            if last_end < content.len() {
                result.push(Span::styled(content[last_end..].to_string(), style));
            }
        }

        result
    }

    /// 获取当前行内容
    pub fn current_line(&self) -> Option<&str> {
        self.lines.get(self.cursor_line).map(|s| s.as_str())
    }

    /// 获取选中的文本范围
    pub fn get_selection(
        &self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> String {
        if start_line == end_line {
            if let Some(line) = self.lines.get(start_line) {
                let start = start_col.min(line.len());
                let end = end_col.min(line.len());
                return line[start..end].to_string();
            }
        } else {
            let mut result = String::new();
            for line_num in start_line..=end_line {
                if let Some(line) = self.lines.get(line_num) {
                    if line_num == start_line {
                        result.push_str(&line[start_col.min(line.len())..]);
                    } else if line_num == end_line {
                        result.push_str(&line[..end_col.min(line.len())]);
                    } else {
                        result.push_str(line);
                    }
                    if line_num < end_line {
                        result.push('\n');
                    }
                }
            }
            return result;
        }
        String::new()
    }

    /// 更新可见行数
    pub fn set_visible_lines(&mut self, count: usize) {
        self.visible_lines = count;
    }

    /// 获取语言
    pub fn language(&self) -> CodeLanguage {
        self.language
    }
}

impl Default for CodeViewerComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_viewer_creation() {
        let viewer = CodeViewerComponent::new();
        assert!(viewer.file_path.is_none());
        assert!(viewer.lines.is_empty());
        assert_eq!(viewer.line_count(), 0);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(CodeLanguage::from_extension("py"), CodeLanguage::Python);
        assert_eq!(CodeLanguage::from_extension("rs"), CodeLanguage::Rust);
        assert_eq!(CodeLanguage::from_extension("js"), CodeLanguage::JavaScript);
        assert_eq!(CodeLanguage::from_extension("unknown"), CodeLanguage::Plain);
    }

    #[test]
    fn test_goto_line() {
        let mut viewer = CodeViewerComponent::new();
        viewer.lines = vec![
            "line1".to_string(),
            "line2".to_string(),
            "line3".to_string(),
        ];
        viewer.goto_line(2);
        assert_eq!(viewer.cursor_line, 2);
    }

    #[test]
    fn test_search() {
        let mut viewer = CodeViewerComponent::new();
        viewer.lines = vec![
            "hello world".to_string(),
            "foo bar".to_string(),
            "hello again".to_string(),
        ];
        viewer.search("hello");
        assert_eq!(viewer.search_result_count(), 2);
    }

    #[test]
    fn test_highlighter() {
        let highlighter = SyntaxHighlighter::default();
        let spans = highlighter.highlight_line("def hello():", CodeLanguage::Python);
        assert!(!spans.is_empty());
    }
}
