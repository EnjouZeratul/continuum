//! 终端颜色输出模块
//!
//! 提供非 TUI 模式下的 ANSI 颜色输出支持，包括：
//! - Markdown 渲染
//! - 语法高亮
//! - 主题系统

use std::fmt;
use std::io::{self, Write};

// ============================================================================
// ANSI 颜色和样式
// ============================================================================

/// ANSI 颜色代码
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnsiColor {
    // 基础颜色
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    // 亮色
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    // 256色
    Rgb(u8, u8, u8),
    // 重置
    Reset,
}

impl AnsiColor {
    /// 获取 ANSI 转义码
    pub fn to_ansi(self) -> String {
        match self {
            AnsiColor::Black => "\x1b[30m".to_string(),
            AnsiColor::Red => "\x1b[31m".to_string(),
            AnsiColor::Green => "\x1b[32m".to_string(),
            AnsiColor::Yellow => "\x1b[33m".to_string(),
            AnsiColor::Blue => "\x1b[34m".to_string(),
            AnsiColor::Magenta => "\x1b[35m".to_string(),
            AnsiColor::Cyan => "\x1b[36m".to_string(),
            AnsiColor::White => "\x1b[37m".to_string(),
            AnsiColor::BrightBlack => "\x1b[90m".to_string(),
            AnsiColor::BrightRed => "\x1b[91m".to_string(),
            AnsiColor::BrightGreen => "\x1b[92m".to_string(),
            AnsiColor::BrightYellow => "\x1b[93m".to_string(),
            AnsiColor::BrightBlue => "\x1b[94m".to_string(),
            AnsiColor::BrightMagenta => "\x1b[95m".to_string(),
            AnsiColor::BrightCyan => "\x1b[96m".to_string(),
            AnsiColor::BrightWhite => "\x1b[97m".to_string(),
            AnsiColor::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
            AnsiColor::Reset => "\x1b[0m".to_string(),
        }
    }

    /// 获取背景色 ANSI 转义码
    pub fn to_bg_ansi(self) -> String {
        match self {
            AnsiColor::Black => "\x1b[40m".to_string(),
            AnsiColor::Red => "\x1b[41m".to_string(),
            AnsiColor::Green => "\x1b[42m".to_string(),
            AnsiColor::Yellow => "\x1b[43m".to_string(),
            AnsiColor::Blue => "\x1b[44m".to_string(),
            AnsiColor::Magenta => "\x1b[45m".to_string(),
            AnsiColor::Cyan => "\x1b[46m".to_string(),
            AnsiColor::White => "\x1b[47m".to_string(),
            AnsiColor::BrightBlack => "\x1b[100m".to_string(),
            AnsiColor::BrightRed => "\x1b[101m".to_string(),
            AnsiColor::BrightGreen => "\x1b[102m".to_string(),
            AnsiColor::BrightYellow => "\x1b[103m".to_string(),
            AnsiColor::BrightBlue => "\x1b[104m".to_string(),
            AnsiColor::BrightMagenta => "\x1b[105m".to_string(),
            AnsiColor::BrightCyan => "\x1b[106m".to_string(),
            AnsiColor::BrightWhite => "\x1b[107m".to_string(),
            AnsiColor::Rgb(r, g, b) => format!("\x1b[48;2;{};{};{}m", r, g, b),
            AnsiColor::Reset => "\x1b[0m".to_string(),
        }
    }
}

/// 文本样式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextStyle {
    Bold,
    Dim,
    Italic,
    Underline,
    Blink,
    Reverse,
    Hidden,
    Strikethrough,
}

impl TextStyle {
    /// 获取 ANSI 转义码
    pub fn to_ansi(self) -> &'static str {
        match self {
            TextStyle::Bold => "\x1b[1m",
            TextStyle::Dim => "\x1b[2m",
            TextStyle::Italic => "\x1b[3m",
            TextStyle::Underline => "\x1b[4m",
            TextStyle::Blink => "\x1b[5m",
            TextStyle::Reverse => "\x1b[7m",
            TextStyle::Hidden => "\x1b[8m",
            TextStyle::Strikethrough => "\x1b[9m",
        }
    }

    /// 获取重置码
    pub fn reset_ansi(self) -> &'static str {
        match self {
            TextStyle::Bold => "\x1b[22m",
            TextStyle::Dim => "\x1b[22m",
            TextStyle::Italic => "\x1b[23m",
            TextStyle::Underline => "\x1b[24m",
            TextStyle::Blink => "\x1b[25m",
            TextStyle::Reverse => "\x1b[27m",
            TextStyle::Hidden => "\x1b[28m",
            TextStyle::Strikethrough => "\x1b[29m",
        }
    }
}

// ============================================================================
// 彩色文本
// ============================================================================

/// 彩色文本
#[derive(Debug, Clone)]
pub struct ColoredText {
    text: String,
    fg_color: Option<AnsiColor>,
    bg_color: Option<AnsiColor>,
    styles: Vec<TextStyle>,
}

impl ColoredText {
    /// 创建新的彩色文本
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg_color: None,
            bg_color: None,
            styles: Vec::new(),
        }
    }

    /// 设置前景色
    pub fn fg(mut self, color: AnsiColor) -> Self {
        self.fg_color = Some(color);
        self
    }

    /// 设置背景色
    pub fn bg(mut self, color: AnsiColor) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// 添加样式
    pub fn style(mut self, style: TextStyle) -> Self {
        self.styles.push(style);
        self
    }

    /// 添加粗体样式
    pub fn bold(self) -> Self {
        self.style(TextStyle::Bold)
    }

    /// 添加斜体样式
    pub fn italic(self) -> Self {
        self.style(TextStyle::Italic)
    }

    /// 添加下划线样式
    pub fn underline(self) -> Self {
        self.style(TextStyle::Underline)
    }

    /// 添加暗淡样式
    pub fn dim(self) -> Self {
        self.style(TextStyle::Dim)
    }

    /// 渲染为带 ANSI 代码的字符串
    pub fn render(&self) -> String {
        let mut result = String::new();

        // 添加背景色
        if let Some(bg) = self.bg_color {
            result.push_str(&bg.to_bg_ansi());
        }

        // 添加前景色
        if let Some(fg) = self.fg_color {
            result.push_str(&fg.to_ansi());
        }

        // 添加样式
        for &style in &self.styles {
            result.push_str(style.to_ansi());
        }

        // 添加文本
        result.push_str(&self.text);

        // 重置
        result.push_str("\x1b[0m");

        result
    }

    /// 获取原始文本（不带 ANSI 代码）
    pub fn plain(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for ColoredText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ============================================================================
// 终端输出主题
// ============================================================================

/// 终端输出主题
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    /// 主题名称
    pub name: String,

    // Markdown 颜色
    /// 标题颜色
    pub heading: AnsiColor,
    /// 粗体文本颜色
    pub bold: AnsiColor,
    /// 斜体文本颜色
    pub italic: AnsiColor,
    /// 代码块颜色
    pub code_block: AnsiColor,
    /// 行内代码颜色
    pub inline_code: AnsiColor,
    /// 链接颜色
    pub link: AnsiColor,
    /// 引用颜色
    pub quote: AnsiColor,
    /// 列表标记颜色
    pub list_marker: AnsiColor,
    /// 水平线颜色
    pub hr: AnsiColor,

    // 消息颜色
    /// 错误消息颜色
    pub error: AnsiColor,
    /// 警告消息颜色
    pub warning: AnsiColor,
    /// 成功消息颜色
    pub success: AnsiColor,
    /// 信息消息颜色
    pub info: AnsiColor,

    // 语法高亮颜色
    /// 关键字颜色
    pub keyword: AnsiColor,
    /// 字符串颜色
    pub string: AnsiColor,
    /// 注释颜色
    pub comment: AnsiColor,
    /// 数字颜色
    pub number: AnsiColor,
    /// 函数颜色
    pub function: AnsiColor,
    /// 类型颜色
    pub type_name: AnsiColor,
    /// 操作符颜色
    pub operator: AnsiColor,
    /// 括号颜色
    pub bracket: AnsiColor,
    /// 变量颜色
    pub variable: AnsiColor,

    // UI 颜色
    /// 边框颜色
    pub border: AnsiColor,
    /// 标题颜色
    pub title: AnsiColor,
    /// 高亮颜色
    pub highlight: AnsiColor,
}

impl TerminalTheme {
    /// 深色主题
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),

            // Markdown
            heading: AnsiColor::BrightCyan,
            bold: AnsiColor::BrightWhite,
            italic: AnsiColor::Cyan,
            code_block: AnsiColor::Green,
            inline_code: AnsiColor::Yellow,
            link: AnsiColor::BrightBlue,
            quote: AnsiColor::BrightBlack,
            list_marker: AnsiColor::Yellow,
            hr: AnsiColor::BrightBlack,

            // Messages
            error: AnsiColor::BrightRed,
            warning: AnsiColor::BrightYellow,
            success: AnsiColor::BrightGreen,
            info: AnsiColor::BrightCyan,

            // Syntax
            keyword: AnsiColor::BrightMagenta,
            string: AnsiColor::Green,
            comment: AnsiColor::BrightBlack,
            number: AnsiColor::Yellow,
            function: AnsiColor::BrightCyan,
            type_name: AnsiColor::BrightBlue,
            operator: AnsiColor::BrightRed,
            bracket: AnsiColor::Yellow,
            variable: AnsiColor::White,

            // UI
            border: AnsiColor::Blue,
            title: AnsiColor::BrightCyan,
            highlight: AnsiColor::BrightYellow,
        }
    }

    /// 浅色主题
    pub fn light() -> Self {
        Self {
            name: "light".to_string(),

            // Markdown
            heading: AnsiColor::Cyan,
            bold: AnsiColor::Black,
            italic: AnsiColor::Blue,
            code_block: AnsiColor::Green,
            inline_code: AnsiColor::Yellow,
            link: AnsiColor::Blue,
            quote: AnsiColor::BrightBlack,
            list_marker: AnsiColor::Yellow,
            hr: AnsiColor::BrightBlack,

            // Messages
            error: AnsiColor::Red,
            warning: AnsiColor::Yellow,
            success: AnsiColor::Green,
            info: AnsiColor::Cyan,

            // Syntax
            keyword: AnsiColor::Magenta,
            string: AnsiColor::Green,
            comment: AnsiColor::BrightBlack,
            number: AnsiColor::Yellow,
            function: AnsiColor::Cyan,
            type_name: AnsiColor::Blue,
            operator: AnsiColor::Red,
            bracket: AnsiColor::Yellow,
            variable: AnsiColor::Black,

            // UI
            border: AnsiColor::Blue,
            title: AnsiColor::Cyan,
            highlight: AnsiColor::Yellow,
        }
    }

    /// Monokai 主题
    pub fn monokai() -> Self {
        Self {
            name: "monokai".to_string(),

            // Markdown
            heading: AnsiColor::Rgb(102, 217, 239),     // Cyan
            bold: AnsiColor::Rgb(248, 248, 242),        // White
            italic: AnsiColor::Rgb(102, 217, 239),      // Cyan
            code_block: AnsiColor::Rgb(166, 226, 46),   // Green
            inline_code: AnsiColor::Rgb(230, 219, 116), // Yellow
            link: AnsiColor::Rgb(102, 217, 239),        // Cyan
            quote: AnsiColor::Rgb(117, 113, 94),        // Comment gray
            list_marker: AnsiColor::Rgb(253, 151, 31),  // Orange
            hr: AnsiColor::Rgb(117, 113, 94),

            // Messages
            error: AnsiColor::Rgb(249, 38, 114),   // Pink/Red
            warning: AnsiColor::Rgb(253, 151, 31), // Orange
            success: AnsiColor::Rgb(166, 226, 46), // Green
            info: AnsiColor::Rgb(102, 217, 239),   // Cyan

            // Syntax
            keyword: AnsiColor::Rgb(249, 38, 114),    // Pink
            string: AnsiColor::Rgb(230, 219, 116),    // Yellow
            comment: AnsiColor::Rgb(117, 113, 94),    // Gray
            number: AnsiColor::Rgb(174, 129, 255),    // Purple
            function: AnsiColor::Rgb(166, 226, 46),   // Green
            type_name: AnsiColor::Rgb(102, 217, 239), // Cyan
            operator: AnsiColor::Rgb(249, 38, 114),   // Pink
            bracket: AnsiColor::Rgb(253, 151, 31),    // Orange
            variable: AnsiColor::Rgb(248, 248, 242),  // White

            // UI
            border: AnsiColor::Rgb(117, 113, 94),
            title: AnsiColor::Rgb(253, 151, 31),
            highlight: AnsiColor::Rgb(230, 219, 116),
        }
    }

    /// 根据名称获取主题
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "light" => Self::light(),
            "monokai" => Self::monokai(),
            _ => Self::dark(),
        }
    }
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::dark()
    }
}

// ============================================================================
// 终端输出器
// ============================================================================

/// 终端颜色输出器
pub struct TerminalOutput {
    /// 是否支持颜色
    color_enabled: bool,
    /// 是否支持 256 色
    true_color: bool,
    /// 当前主题
    theme: TerminalTheme,
}

impl TerminalOutput {
    /// 创建新的输出器（自动检测终端能力）
    pub fn new() -> Self {
        let color_enabled = Self::detect_color_support();
        let true_color = Self::detect_true_color_support();
        Self {
            color_enabled,
            true_color,
            theme: TerminalTheme::dark(),
        }
    }

    /// 创建禁用颜色的输出器
    pub fn no_color() -> Self {
        Self {
            color_enabled: false,
            true_color: false,
            theme: TerminalTheme::dark(),
        }
    }

    /// 强制启用颜色
    pub fn force_color() -> Self {
        Self {
            color_enabled: true,
            true_color: true,
            theme: TerminalTheme::dark(),
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.theme = theme;
    }

    /// 获取当前主题
    pub fn theme(&self) -> &TerminalTheme {
        &self.theme
    }

    /// 检测是否支持颜色
    fn detect_color_support() -> bool {
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        if let Ok(term) = std::env::var("TERM") {
            if term == "dumb" {
                return false;
            }
        }

        #[cfg(not(windows))]
        {
            // Unix: 检查是否在 TTY 中
            use std::io::IsTerminal;
            std::io::stdout().is_terminal()
        }

        #[cfg(windows)]
        {
            // Windows: 默认支持颜色
            true
        }
    }

    /// 检测是否支持真彩色
    fn detect_true_color_support() -> bool {
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            return colorterm == "truecolor" || colorterm == "24bit";
        }

        if let Ok(term) = std::env::var("TERM") {
            return term.contains("256color") || term.contains("truecolor");
        }

        false
    }

    /// 打印彩色文本
    pub fn print(&self, text: &ColoredText) -> io::Result<()> {
        let output = if self.color_enabled {
            text.render()
        } else {
            text.plain().to_string()
        };
        print!("{}", output);
        io::stdout().flush()
    }

    /// 打印一行彩色文本
    pub fn println(&self, text: &ColoredText) -> io::Result<()> {
        self.print(text)?;
        println!();
        Ok(())
    }

    /// 打印成功消息
    pub fn success(&self, msg: &str) -> io::Result<()> {
        self.println(
            &ColoredText::new(format!("✓ {}", msg))
                .fg(self.theme.success)
                .bold(),
        )
    }

    /// 打印错误消息
    pub fn error(&self, msg: &str) -> io::Result<()> {
        self.println(
            &ColoredText::new(format!("✗ {}", msg))
                .fg(self.theme.error)
                .bold(),
        )
    }

    /// 打印警告消息
    pub fn warning(&self, msg: &str) -> io::Result<()> {
        self.println(&ColoredText::new(format!("⚠ {}", msg)).fg(self.theme.warning))
    }

    /// 打印信息消息
    pub fn info(&self, msg: &str) -> io::Result<()> {
        self.println(&ColoredText::new(format!("ℹ {}", msg)).fg(self.theme.info))
    }

    /// 打印标题
    pub fn title(&self, msg: &str) -> io::Result<()> {
        self.println(&ColoredText::new(msg).fg(self.theme.title).bold())
    }

    /// 打印分隔线
    pub fn separator(&self) -> io::Result<()> {
        println!("{}", "━".repeat(60));
        Ok(())
    }

    /// 打印进度指示器
    pub fn progress(&self, current: usize, total: usize, msg: &str) -> io::Result<()> {
        let percentage = (current * 100).checked_div(total).unwrap_or(0);
        let bar_len = 30;
        let filled = (percentage * bar_len) / 100;

        let bar: String = "█".repeat(filled) + &"░".repeat(bar_len - filled);
        let progress_text = format!("[{}] {}% - {}", bar, percentage, msg);

        self.print(&ColoredText::new(progress_text).fg(self.theme.success))
    }

    /// 清除当前行
    pub fn clear_line(&self) -> io::Result<()> {
        print!("\r\x1b[2K");
        io::stdout().flush()
    }

    /// 是否支持颜色
    pub fn is_color_enabled(&self) -> bool {
        self.color_enabled
    }

    /// 是否支持真彩色
    pub fn is_true_color(&self) -> bool {
        self.true_color
    }
}

impl Default for TerminalOutput {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Markdown 渲染器
// ============================================================================

/// Markdown 渲染器
pub struct MarkdownRenderer {
    output: TerminalOutput,
}

impl MarkdownRenderer {
    /// 创建新的渲染器
    pub fn new() -> Self {
        Self {
            output: TerminalOutput::new(),
        }
    }

    /// 使用指定输出器创建渲染器
    pub fn with_output(output: TerminalOutput) -> Self {
        Self { output }
    }

    /// 渲染 Markdown 文本
    pub fn render(&self, markdown: &str) -> io::Result<()> {
        let lines: Vec<&str> = markdown.lines().collect();
        let mut in_code_block = false;
        let mut code_language = String::new();
        let mut code_buffer = String::new();

        for line in lines {
            // 代码块开始/结束
            if line.starts_with("```") {
                if in_code_block {
                    // 结束代码块
                    self.render_code_block(&code_buffer, &code_language)?;
                    code_buffer.clear();
                    code_language.clear();
                    in_code_block = false;
                } else {
                    // 开始代码块
                    in_code_block = true;
                    code_language = line.trim_start_matches('`').trim().to_string();
                }
                continue;
            }

            if in_code_block {
                code_buffer.push_str(line);
                code_buffer.push('\n');
            } else {
                self.render_line(line)?;
            }
        }

        // 处理未关闭的代码块
        if !code_buffer.is_empty() {
            self.render_code_block(&code_buffer, &code_language)?;
        }

        Ok(())
    }

    /// 渲染单行 Markdown
    fn render_line(&self, line: &str) -> io::Result<()> {
        // 标题
        if line.starts_with("### ") {
            let text = line.trim_start_matches("### ");
            self.output.println(
                &ColoredText::new(text)
                    .fg(self.output.theme.heading)
                    .bold()
                    .bold(),
            )?;
            return Ok(());
        }
        if line.starts_with("## ") {
            let text = line.trim_start_matches("## ");
            self.output
                .println(&ColoredText::new(text).fg(self.output.theme.heading).bold())?;
            return Ok(());
        }
        if line.starts_with("# ") {
            let text = line.trim_start_matches("# ");
            self.output.println(
                &ColoredText::new(text)
                    .fg(self.output.theme.heading)
                    .bold()
                    .underline(),
            )?;
            return Ok(());
        }

        // 水平线
        if line == "---" || line == "***" || line == "___" {
            self.output
                .println(&ColoredText::new("─".repeat(60)).fg(self.output.theme.hr))?;
            return Ok(());
        }

        // 引用
        if line.starts_with("> ") {
            let text = line.trim_start_matches("> ");
            self.output
                .println(&ColoredText::new(format!("│ {}", text)).fg(self.output.theme.quote))?;
            return Ok(());
        }

        // 列表
        if line.starts_with("- ") || line.starts_with("* ") {
            let text = line.trim_start_matches(['-', '*', ' ']);
            let rendered = self.render_inline(text);
            self.output.println(
                &ColoredText::new(format!("• {}", rendered)).fg(self.output.theme.list_marker),
            )?;
            return Ok(());
        }

        // 有序列表
        if let Some(pos) = line.find(". ") {
            if pos > 0 && line[..pos].chars().all(|c| c.is_ascii_digit()) {
                let text = &line[pos + 2..];
                let rendered = self.render_inline(text);
                self.output.println(&ColoredText::new(format!(
                    "{}. {}",
                    &line[..pos + 1],
                    rendered
                )))?;
                return Ok(());
            }
        }

        // 普通文本（处理内联样式）
        let rendered = self.render_inline(line);
        println!("{}", rendered);
        Ok(())
    }

    /// 渲染内联 Markdown 样式
    fn render_inline(&self, text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                // 粗体 **text**
                '*' if chars.peek() == Some(&'*') => {
                    chars.next(); // 消费第二个 *
                    let mut bold_text = String::new();
                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'*') {
                            chars.next();
                            break;
                        }
                        bold_text.push(c);
                    }
                    result.push_str(
                        &ColoredText::new(bold_text)
                            .fg(self.output.theme.bold)
                            .bold()
                            .render(),
                    );
                }
                // 斜体 *text* 或 _text_
                '*' | '_' => {
                    let marker = ch;
                    let mut italic_text = String::new();
                    let mut found_end = false;
                    for c in chars.by_ref() {
                        if c == marker {
                            found_end = true;
                            break;
                        }
                        italic_text.push(c);
                    }
                    if found_end && !italic_text.is_empty() {
                        result.push_str(
                            &ColoredText::new(italic_text)
                                .fg(self.output.theme.italic)
                                .italic()
                                .render(),
                        );
                    } else {
                        result.push(marker);
                        result.push_str(&italic_text);
                        if found_end {
                            result.push(marker);
                        }
                    }
                }
                // 行内代码 `code`
                '`' => {
                    let mut code_text = String::new();
                    for c in chars.by_ref() {
                        if c == '`' {
                            break;
                        }
                        code_text.push(c);
                    }
                    result.push_str(
                        &ColoredText::new(code_text)
                            .fg(self.output.theme.inline_code)
                            .render(),
                    );
                }
                // 链接 [text](url)
                '[' => {
                    let mut link_text = String::new();
                    for c in chars.by_ref() {
                        if c == ']' {
                            break;
                        }
                        link_text.push(c);
                    }
                    if chars.peek() == Some(&'(') {
                        chars.next();
                        let mut url = String::new();
                        for c in chars.by_ref() {
                            if c == ')' {
                                break;
                            }
                            url.push(c);
                        }
                        result.push_str(
                            &ColoredText::new(&link_text)
                                .fg(self.output.theme.link)
                                .underline()
                                .render(),
                        );
                    } else {
                        result.push('[');
                        result.push_str(&link_text);
                        result.push(']');
                    }
                }
                _ => {
                    result.push(ch);
                }
            }
        }

        result
    }

    /// 渲染代码块
    fn render_code_block(&self, code: &str, language: &str) -> io::Result<()> {
        // 代码块边框
        self.output.println(
            &ColoredText::new("┌".to_string() + &"─".repeat(58) + "┐").fg(self.output.theme.border),
        )?;

        for line in code.lines() {
            // 带行号的代码行
            let highlighted = self.highlight_line(line, language);
            self.output
                .println(&ColoredText::new(format!("│ {}", highlighted)))?;
        }

        self.output.println(
            &ColoredText::new("└".to_string() + &"─".repeat(58) + "┘").fg(self.output.theme.border),
        )?;

        Ok(())
    }

    /// 高亮单行代码
    fn highlight_line(&self, line: &str, _language: &str) -> String {
        let mut result = String::new();
        let mut in_string = false;
        let mut in_comment = false;
        let mut current_word = String::new();

        for ch in line.chars() {
            if in_comment {
                result.push_str(
                    &ColoredText::new(ch.to_string())
                        .fg(self.output.theme.comment)
                        .render(),
                );
                continue;
            }

            match ch {
                '"' | '\'' if !in_string => {
                    if !current_word.is_empty() {
                        result.push_str(&self.highlight_word(&current_word));
                        current_word.clear();
                    }
                    in_string = true;
                    result.push_str(
                        &ColoredText::new(ch.to_string())
                            .fg(self.output.theme.string)
                            .render(),
                    );
                }
                '"' | '\'' if in_string => {
                    result.push_str(
                        &ColoredText::new(ch.to_string())
                            .fg(self.output.theme.string)
                            .render(),
                    );
                    in_string = false;
                }
                '#' | '/' if !in_string => {
                    if !current_word.is_empty() {
                        result.push_str(&self.highlight_word(&current_word));
                        current_word.clear();
                    }
                    in_comment = true;
                    result.push_str(
                        &ColoredText::new(ch.to_string())
                            .fg(self.output.theme.comment)
                            .render(),
                    );
                }
                ' ' | '\t' => {
                    if !current_word.is_empty() {
                        result.push_str(&self.highlight_word(&current_word));
                        current_word.clear();
                    }
                    result.push(ch);
                }
                _ if in_string => {
                    result.push_str(
                        &ColoredText::new(ch.to_string())
                            .fg(self.output.theme.string)
                            .render(),
                    );
                }
                _ => {
                    current_word.push(ch);
                }
            }
        }

        if !current_word.is_empty() {
            result.push_str(&self.highlight_word(&current_word));
        }

        result
    }

    /// 高亮单词
    fn highlight_word(&self, word: &str) -> String {
        let keywords = [
            "fn",
            "let",
            "mut",
            "const",
            "pub",
            "mod",
            "use",
            "struct",
            "enum",
            "impl",
            "trait",
            "type",
            "where",
            "for",
            "loop",
            "while",
            "if",
            "else",
            "match",
            "return",
            "break",
            "continue",
            "async",
            "await",
            "move",
            "ref",
            "self",
            "def",
            "class",
            "import",
            "from",
            "as",
            "try",
            "except",
            "finally",
            "with",
            "yield",
            "lambda",
            "pass",
            "raise",
            "True",
            "False",
            "None",
            "and",
            "or",
            "not",
            "in",
            "is",
            "function",
            "var",
            "const",
            "let",
            "class",
            "extends",
            "export",
            "static",
            "interface",
            "return",
            "if",
            "else",
            "for",
            "while",
        ];

        if keywords.contains(&word) {
            ColoredText::new(word)
                .fg(self.output.theme.keyword)
                .bold()
                .render()
        } else if word.parse::<f64>().is_ok() {
            ColoredText::new(word).fg(self.output.theme.number).render()
        } else if word.starts_with(|c: char| c.is_uppercase()) {
            ColoredText::new(word)
                .fg(self.output.theme.type_name)
                .render()
        } else if word.contains('(') {
            ColoredText::new(word)
                .fg(self.output.theme.function)
                .render()
        } else {
            word.to_string()
        }
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_color() {
        assert_eq!(AnsiColor::Red.to_ansi(), "\x1b[31m");
        assert_eq!(AnsiColor::Reset.to_ansi(), "\x1b[0m");
    }

    #[test]
    fn test_colored_text() {
        let text = ColoredText::new("Hello").fg(AnsiColor::Red).bold();
        let rendered = text.render();
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("\x1b[31m")); // Red
        assert!(rendered.contains("\x1b[1m")); // Bold
    }

    #[test]
    fn test_colored_text_plain() {
        let text = ColoredText::new("Hello");
        assert_eq!(text.plain(), "Hello");
    }

    #[test]
    fn test_rgb_color() {
        let color = AnsiColor::Rgb(255, 165, 0);
        let ansi = color.to_ansi();
        assert!(ansi.contains("38;2;255;165;0"));
    }

    #[test]
    fn test_bg_color() {
        let color = AnsiColor::Blue;
        let bg_ansi = color.to_bg_ansi();
        assert_eq!(bg_ansi, "\x1b[44m");
    }

    #[test]
    fn test_chained_styles() {
        let text = ColoredText::new("Test")
            .fg(AnsiColor::Green)
            .bg(AnsiColor::Black)
            .bold()
            .underline();
        let rendered = text.render();
        assert!(rendered.contains("\x1b[32m")); // Green fg
        assert!(rendered.contains("\x1b[40m")); // Black bg
        assert!(rendered.contains("\x1b[1m")); // Bold
        assert!(rendered.contains("\x1b[4m")); // Underline
    }

    #[test]
    fn test_theme() {
        let theme = TerminalTheme::dark();
        assert_eq!(theme.name, "dark");
        assert_eq!(theme.error, AnsiColor::BrightRed);
    }

    #[test]
    fn test_markdown_renderer_inline() {
        let renderer = MarkdownRenderer::new();
        let result = renderer.render_inline("Hello **world**!");
        assert!(result.contains("world"));
    }
}
