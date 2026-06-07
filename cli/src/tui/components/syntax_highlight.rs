//! 语法高亮模块
//!
//! 使用 syntect 提供代码语法高亮支持

use ratatui::style::Color;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use super::color_theme::ColorTheme;

/// 语法高亮器
pub struct SyntaxHighlighter {
    /// 语法集合
    syntax_set: SyntaxSet,
    /// 主题集合
    theme_set: ThemeSet,
    /// 当前主题名
    current_theme: String,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// 创建新的语法高亮器
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        Self {
            syntax_set,
            theme_set,
            current_theme: "base16-ocean.dark".to_string(),
        }
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme_name: &str) {
        // 映射我们的主题名到 syntect 主题
        let syntect_theme = match theme_name {
            "dark" | "monokai" => "base16-ocean.dark",
            "light" => "base16-ocean.light",
            "dracula" => "base16-eighties.dark",
            "nord" => "base16-nord",
            _ => "base16-ocean.dark",
        };
        self.current_theme = syntect_theme.to_string();
    }

    /// 获取语言对应的语法名
    fn get_syntax_name(language: &str) -> &str {
        match language.to_lowercase().as_str() {
            "rust" | "rs" => "Rust",
            "python" | "py" => "Python",
            "javascript" | "js" | "node" => "JavaScript",
            "typescript" | "ts" => "TypeScript",
            "java" => "Java",
            "go" => "Go",
            "c" => "C",
            "cpp" | "c++" => "C++",
            "csharp" | "c#" | "cs" => "C#",
            "ruby" | "rb" => "Ruby",
            "php" => "PHP",
            "swift" => "Swift",
            "kotlin" | "kt" => "Kotlin",
            "scala" => "Scala",
            "html" => "HTML",
            "css" => "CSS",
            "scss" | "sass" => "SCSS",
            "json" => "JSON",
            "yaml" | "yml" => "YAML",
            "toml" => "TOML",
            "markdown" | "md" => "Markdown",
            "sql" => "SQL",
            "shell" | "bash" | "sh" | "zsh" => "Bash",
            "powershell" | "ps1" => "PowerShell",
            "docker" | "dockerfile" => "Dockerfile",
            "xml" => "XML",
            "lua" => "Lua",
            "perl" => "Perl",
            "r" => "R",
            "haskell" => "Haskell",
            "elixir" => "Elixir",
            "erlang" => "Erlang",
            "clojure" => "Clojure",
            "ocaml" => "OCaml",
            "f#" => "F#",
            "dart" => "Dart",
            "vue" => "Vue Component",
            "jsx" => "JSX",
            "tsx" => "TSX",
            "graphql" | "gql" => "GraphQL",
            "protobuf" => "Protocol Buffer",
            "regex" => "Regular Expression",
            _ => "Plain Text",
        }
    }

    /// 高亮代码
    pub fn highlight(&self, code: &str, language: &str) -> Vec<HighlightedLine> {
        let syntax_name = Self::get_syntax_name(language);

        let syntax = match self.syntax_set.find_syntax_by_name(syntax_name) {
            Some(s) => s,
            None => match self.syntax_set.find_syntax_by_first_line(code) {
                Some(s) => s,
                None => self.syntax_set.find_syntax_plain_text(),
            },
        };

        let theme = match self.theme_set.themes.get(&self.current_theme) {
            Some(t) => t,
            None => &self.theme_set.themes["base16-ocean.dark"],
        };

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();

        for line in LinesWithEndings::from(code) {
            let ranges: Vec<(Style, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let spans: Vec<HighlightedSpan> = ranges
                .into_iter()
                .map(|(style, text)| HighlightedSpan {
                    text: text.to_string(),
                    color: syntect_color_to_ratatui(style.foreground),
                    is_bold: style.font_style.contains(FontStyle::BOLD),
                    is_italic: style.font_style.contains(FontStyle::ITALIC),
                    is_underline: style.font_style.contains(FontStyle::UNDERLINE),
                })
                .collect();
            lines.push(HighlightedLine { spans });
        }

        lines
    }

    /// 使用 ColorTheme 进行手动高亮（备用方案）
    pub fn highlight_with_theme(
        &self,
        code: &str,
        _language: &str,
        theme: &ColorTheme,
    ) -> Vec<HighlightedLine> {
        let mut lines = Vec::new();

        for line in code.lines() {
            let spans = self.tokenize_line(line, theme);
            lines.push(HighlightedLine { spans });
        }

        lines
    }

    /// 简单的行级词法分析
    fn tokenize_line(&self, line: &str, theme: &ColorTheme) -> Vec<HighlightedSpan> {
        let mut spans = Vec::new();
        let mut current_word = String::new();
        let mut in_string = false;
        let mut string_char = ' ';
        let mut in_comment = false;
        let mut in_number = false;

        for ch in line.chars() {
            if in_comment {
                current_word.push(ch);
                continue;
            }

            match ch {
                '"' | '\'' | '`' if !in_string => {
                    if !current_word.is_empty() {
                        spans.push(self.classify_token(&current_word, theme));
                        current_word.clear();
                    }
                    in_string = true;
                    string_char = ch;
                    current_word.push(ch);
                }
                '"' | '\'' | '`' if in_string && ch == string_char => {
                    current_word.push(ch);
                    spans.push(HighlightedSpan {
                        text: current_word.clone(),
                        color: theme.string,
                        is_bold: false,
                        is_italic: false,
                        is_underline: false,
                    });
                    current_word.clear();
                    in_string = false;
                }
                '#' | '/' if !in_string => {
                    if !current_word.is_empty() {
                        spans.push(self.classify_token(&current_word, theme));
                        current_word.clear();
                    }
                    in_comment = true;
                    current_word.push(ch);
                }
                ' ' | '\t' => {
                    if in_number && current_word.parse::<f64>().is_err() {
                        in_number = false;
                    }
                    if !current_word.is_empty() {
                        spans.push(self.classify_token(&current_word, theme));
                        current_word.clear();
                    }
                    spans.push(HighlightedSpan {
                        text: ch.to_string(),
                        color: theme.foreground,
                        is_bold: false,
                        is_italic: false,
                        is_underline: false,
                    });
                }
                '0'..='9' | '.' if !in_string => {
                    if current_word.is_empty() || in_number {
                        in_number = true;
                    }
                    current_word.push(ch);
                }
                _ if in_string => {
                    current_word.push(ch);
                }
                _ => {
                    if in_number && !ch.is_ascii_digit() && ch != '.' {
                        in_number = false;
                    }
                    current_word.push(ch);
                }
            }
        }

        if !current_word.is_empty() {
            if in_comment {
                spans.push(HighlightedSpan {
                    text: current_word,
                    color: theme.comment,
                    is_bold: false,
                    is_italic: true,
                    is_underline: false,
                });
            } else if in_string {
                spans.push(HighlightedSpan {
                    text: current_word,
                    color: theme.string,
                    is_bold: false,
                    is_italic: false,
                    is_underline: false,
                });
            } else {
                spans.push(self.classify_token(&current_word, theme));
            }
        }

        spans
    }

    /// 分类 token 类型
    fn classify_token(&self, token: &str, theme: &ColorTheme) -> HighlightedSpan {
        let keywords = [
            // Rust
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
            "Self",
            "static",
            "dyn",
            "unsafe",
            "extern",
            "crate",
            "super",
            // Python
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
            "global",
            "nonlocal",
            "assert",
            "del",
            "print",
            // JavaScript/TypeScript
            "function",
            "var",
            "const",
            "let",
            "class",
            "extends",
            "export",
            "static",
            "interface",
            "implements",
            "abstract",
            "private",
            "protected",
            "public",
            "readonly",
            "namespace",
            "declare",
            "type",
            "keyof",
            "infer",
            // Go
            "package",
            "go",
            "select",
            "defer",
            "chan",
            "fallthrough",
            "range",
            // Java
            "new",
            "throws",
            "throw",
            "synchronized",
            "volatile",
            "transient",
            "native",
            "strictfp",
            "instanceof",
            "default",
            // C/C++
            "sizeof",
            "typedef",
            "union",
            "unsigned",
            "signed",
            "void",
            "auto",
            "register",
            // General
            "true",
            "false",
            "null",
            "nil",
            "undefined",
            "NaN",
            "Infinity",
        ];

        let types = [
            "int",
            "float",
            "double",
            "char",
            "bool",
            "boolean",
            "string",
            "str",
            "i8",
            "i16",
            "i32",
            "i64",
            "i128",
            "u8",
            "u16",
            "u32",
            "u64",
            "u128",
            "f32",
            "f64",
            "isize",
            "usize",
            "Option",
            "Result",
            "Vec",
            "String",
            "HashMap",
            "HashSet",
            "BTreeMap",
            "BTreeSet",
            "Box",
            "Rc",
            "Arc",
            "RefCell",
            "Cell",
            "Cow",
            "PhantomData",
            "Duration",
            "Instant",
            "list",
            "dict",
            "set",
            "tuple",
            "object",
            "Array",
            "Map",
            "Set",
            "Promise",
            "Observable",
            "Function",
            "Symbol",
        ];

        let operators = [
            "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "->", "=>", "::", "+", "-", "*", "/",
            "%", "&", "|", "^", "!", "<", ">", "=",
        ];

        let is_keyword = keywords.contains(&token);
        let is_type = types.contains(&token);
        let is_number = token.parse::<f64>().is_ok();
        let is_operator = operators.contains(&token);

        let (color, is_bold) = if is_keyword {
            (theme.keyword, true)
        } else if is_type {
            (theme.type_name, false)
        } else if is_number {
            (theme.number, false)
        } else if is_operator {
            (theme.operator, false)
        } else if token.starts_with(|c: char| c.is_uppercase()) {
            (theme.type_name, false)
        } else if token.starts_with(['$', '@', ':']) {
            (theme.variable, false)
        } else {
            (theme.foreground, false)
        };

        HighlightedSpan {
            text: token.to_string(),
            color,
            is_bold,
            is_italic: false,
            is_underline: false,
        }
    }
}

/// 高亮行
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    /// 该行的高亮片段
    pub spans: Vec<HighlightedSpan>,
}

/// 高亮片段
#[derive(Debug, Clone)]
pub struct HighlightedSpan {
    /// 文本内容
    pub text: String,
    /// 前景色
    pub color: Color,
    /// 是否粗体
    pub is_bold: bool,
    /// 是否斜体
    pub is_italic: bool,
    /// 是否下划线
    pub is_underline: bool,
}

/// 将 syntect 颜色转换为 ratatui 颜色
fn syntect_color_to_ratatui(color: syntect::highlighting::Color) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

/// 全局语法高亮器实例
static GLOBAL_HIGHLIGHTER: OnceLock<SyntaxHighlighter> = OnceLock::new();

/// 获取全局语法高亮器
pub fn get_highlighter() -> &'static SyntaxHighlighter {
    GLOBAL_HIGHLIGHTER.get_or_init(SyntaxHighlighter::new)
}

/// 快捷高亮函数
pub fn highlight_code(code: &str, language: &str) -> Vec<HighlightedLine> {
    get_highlighter().highlight(code, language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlighter_creation() {
        let highlighter = SyntaxHighlighter::new();
        let lines = highlighter.highlight("fn main() {}", "rust");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_syntax_name_mapping() {
        assert_eq!(SyntaxHighlighter::get_syntax_name("rust"), "Rust");
        assert_eq!(SyntaxHighlighter::get_syntax_name("python"), "Python");
        assert_eq!(SyntaxHighlighter::get_syntax_name("js"), "JavaScript");
    }

    #[test]
    fn test_get_highlighter() {
        let highlighter = get_highlighter();
        let lines = highlighter.highlight("print('hello')", "python");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_highlight_with_theme() {
        let highlighter = SyntaxHighlighter::new();
        let theme = ColorTheme::dark();
        let lines = highlighter.highlight_with_theme("let x = 5;", "rust", &theme);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_classify_keyword() {
        let highlighter = SyntaxHighlighter::new();
        let theme = ColorTheme::dark();
        let span = highlighter.classify_token("fn", &theme);
        assert!(span.is_bold);
    }

    #[test]
    fn test_classify_number() {
        let highlighter = SyntaxHighlighter::new();
        let theme = ColorTheme::dark();
        let span = highlighter.classify_token("42", &theme);
        assert_eq!(span.color, theme.number);
    }
}
