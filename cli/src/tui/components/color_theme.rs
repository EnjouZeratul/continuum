//! 颜色主题系统
//!
//! 提供统一的颜色配置，支持多种主题切换

use ratatui::style::Color;
use std::collections::HashMap;

/// 颜色主题定义
#[derive(Debug, Clone)]
pub struct ColorTheme {
    /// 主题名称
    pub name: String,

    // === 基础颜色 ===
    /// 前景色（文字颜色）
    pub foreground: Color,
    /// 背景色
    pub background: Color,

    // === 消息角色颜色 ===
    /// 用户消息颜色
    pub user_message: Color,
    /// 助手消息颜色
    pub assistant_message: Color,
    /// 系统消息颜色
    pub system_message: Color,
    /// 错误消息颜色
    pub error_message: Color,
    /// 警告消息颜色
    pub warning_message: Color,
    /// 成功消息颜色
    pub success_message: Color,

    // === 语法高亮颜色 ===
    /// 关键字颜色
    pub keyword: Color,
    /// 字符串颜色
    pub string: Color,
    /// 注释颜色
    pub comment: Color,
    /// 数字颜色
    pub number: Color,
    /// 函数名颜色
    pub function: Color,
    /// 类型名颜色
    pub type_name: Color,
    /// 变量颜色
    pub variable: Color,
    /// 操作符颜色
    pub operator: Color,
    /// 括号颜色
    pub bracket: Color,
    /// 标点符号颜色
    pub punctuation: Color,

    // === UI 组件颜色 ===
    /// 边框颜色
    pub border: Color,
    /// 标题颜色
    pub title: Color,
    /// 高亮颜色（搜索等）
    pub highlight: Color,
    /// 选中行背景颜色
    pub selection_bg: Color,
    /// 光标颜色
    pub cursor: Color,

    // === 工具状态颜色 ===
    /// 待执行状态颜色
    pub tool_pending: Color,
    /// 执行中状态颜色
    pub tool_running: Color,
    /// 成功状态颜色
    pub tool_success: Color,
    /// 失败状态颜色
    pub tool_failed: Color,

    // === 状态栏颜色 ===
    /// 状态栏背景
    pub status_bar_bg: Color,
    /// 状态栏文字
    pub status_bar_fg: Color,

    // === Token/成本颜色 ===
    /// Token 计数颜色
    pub token_count: Color,
    /// 成本颜色
    pub cost: Color,
    /// 预算警告颜色
    pub budget_warning: Color,
}

impl ColorTheme {
    /// 创建默认深色主题
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),

            // 基础颜色
            foreground: Color::White,
            background: Color::Reset,

            // 消息角色颜色
            user_message: Color::Green,
            assistant_message: Color::Cyan,
            system_message: Color::Yellow,
            error_message: Color::Red,
            warning_message: Color::Rgb(255, 165, 0), // Orange
            success_message: Color::Green,

            // 语法高亮颜色
            keyword: Color::Magenta,
            string: Color::Green,
            comment: Color::DarkGray,
            number: Color::Yellow,
            function: Color::Cyan,
            type_name: Color::Blue,
            variable: Color::White,
            operator: Color::Rgb(255, 165, 0), // Orange
            bracket: Color::Rgb(255, 215, 0),  // Gold
            punctuation: Color::Gray,

            // UI 组件颜色
            border: Color::Blue,
            title: Color::Cyan,
            highlight: Color::Yellow,
            selection_bg: Color::DarkGray,
            cursor: Color::White,

            // 工具状态颜色
            tool_pending: Color::Gray,
            tool_running: Color::Yellow,
            tool_success: Color::Green,
            tool_failed: Color::Red,

            // 状态栏颜色
            status_bar_bg: Color::DarkGray,
            status_bar_fg: Color::White,

            // Token/成本颜色
            token_count: Color::Magenta,
            cost: Color::Green,
            budget_warning: Color::Rgb(255, 165, 0),
        }
    }

    /// 创建浅色主题
    pub fn light() -> Self {
        Self {
            name: "light".to_string(),

            // 基础颜色
            foreground: Color::Black,
            background: Color::Reset,

            // 消息角色颜色
            user_message: Color::Rgb(0, 100, 0), // Dark Green
            assistant_message: Color::Rgb(0, 128, 128), // Teal
            system_message: Color::Rgb(139, 69, 19), // Saddle Brown
            error_message: Color::Rgb(178, 34, 34), // Fire Brick
            warning_message: Color::Rgb(255, 140, 0), // Dark Orange
            success_message: Color::Rgb(0, 128, 0), // Green

            // 语法高亮颜色
            keyword: Color::Rgb(128, 0, 128),    // Purple
            string: Color::Rgb(0, 128, 0),       // Green
            comment: Color::Rgb(105, 105, 105),  // Dim Gray
            number: Color::Rgb(184, 134, 11),    // Dark Goldenrod
            function: Color::Rgb(0, 0, 139),     // Dark Blue
            type_name: Color::Rgb(70, 130, 180), // Steel Blue
            variable: Color::Black,
            operator: Color::Rgb(255, 140, 0), // Dark Orange
            bracket: Color::Rgb(184, 134, 11), // Dark Goldenrod
            punctuation: Color::Gray,

            // UI 组件颜色
            border: Color::Rgb(70, 130, 180),        // Steel Blue
            title: Color::Rgb(0, 128, 128),          // Teal
            highlight: Color::Rgb(255, 215, 0),      // Gold
            selection_bg: Color::Rgb(211, 211, 211), // Light Gray
            cursor: Color::Black,

            // 工具状态颜色
            tool_pending: Color::Gray,
            tool_running: Color::Rgb(255, 140, 0),
            tool_success: Color::Rgb(0, 128, 0),
            tool_failed: Color::Rgb(178, 34, 34),

            // 状态栏颜色
            status_bar_bg: Color::Rgb(211, 211, 211),
            status_bar_fg: Color::Black,

            // Token/成本颜色
            token_count: Color::Rgb(128, 0, 128),
            cost: Color::Rgb(0, 128, 0),
            budget_warning: Color::Rgb(255, 140, 0),
        }
    }

    /// 创建 Monokai 主题
    pub fn monokai() -> Self {
        Self {
            name: "monokai".to_string(),

            // 基础颜色
            foreground: Color::Rgb(248, 248, 242),
            background: Color::Rgb(39, 40, 34),

            // 消息角色颜色
            user_message: Color::Rgb(166, 226, 46), // Green
            assistant_message: Color::Rgb(102, 217, 239), // Cyan
            system_message: Color::Rgb(253, 151, 31), // Orange
            error_message: Color::Rgb(249, 38, 114), // Pink/Red
            warning_message: Color::Rgb(253, 151, 31),
            success_message: Color::Rgb(166, 226, 46),

            // 语法高亮颜色
            keyword: Color::Rgb(249, 38, 114),    // Pink
            string: Color::Rgb(230, 219, 116),    // Yellow
            comment: Color::Rgb(117, 113, 94),    // Gray
            number: Color::Rgb(174, 129, 255),    // Purple
            function: Color::Rgb(166, 226, 46),   // Green
            type_name: Color::Rgb(102, 217, 239), // Cyan
            variable: Color::Rgb(248, 248, 242),
            operator: Color::Rgb(249, 38, 114),
            bracket: Color::Rgb(253, 151, 31),
            punctuation: Color::Rgb(117, 113, 94),

            // UI 组件颜色
            border: Color::Rgb(117, 113, 94),
            title: Color::Rgb(253, 151, 31),
            highlight: Color::Rgb(230, 219, 116),
            selection_bg: Color::Rgb(73, 72, 62),
            cursor: Color::Rgb(248, 248, 242),

            // 工具状态颜色
            tool_pending: Color::Rgb(117, 113, 94),
            tool_running: Color::Rgb(253, 151, 31),
            tool_success: Color::Rgb(166, 226, 46),
            tool_failed: Color::Rgb(249, 38, 114),

            // 状态栏颜色
            status_bar_bg: Color::Rgb(73, 72, 62),
            status_bar_fg: Color::Rgb(248, 248, 242),

            // Token/成本颜色
            token_count: Color::Rgb(174, 129, 255),
            cost: Color::Rgb(166, 226, 46),
            budget_warning: Color::Rgb(253, 151, 31),
        }
    }

    /// 创建 Dracula 主题
    pub fn dracula() -> Self {
        Self {
            name: "dracula".to_string(),

            // 基础颜色
            foreground: Color::Rgb(248, 250, 252),
            background: Color::Rgb(40, 42, 54),

            // 消息角色颜色
            user_message: Color::Rgb(80, 250, 123), // Green
            assistant_message: Color::Rgb(139, 233, 253), // Cyan
            system_message: Color::Rgb(255, 184, 108), // Orange
            error_message: Color::Rgb(255, 85, 85), // Red
            warning_message: Color::Rgb(255, 184, 108),
            success_message: Color::Rgb(80, 250, 123),

            // 语法高亮颜色
            keyword: Color::Rgb(255, 121, 198),   // Pink
            string: Color::Rgb(241, 250, 140),    // Yellow
            comment: Color::Rgb(98, 114, 164),    // Comment
            number: Color::Rgb(189, 147, 249),    // Purple
            function: Color::Rgb(80, 250, 123),   // Green
            type_name: Color::Rgb(139, 233, 253), // Cyan
            variable: Color::Rgb(248, 250, 252),
            operator: Color::Rgb(255, 121, 198),
            bracket: Color::Rgb(255, 184, 108),
            punctuation: Color::Rgb(98, 114, 164),

            // UI 组件颜色
            border: Color::Rgb(98, 114, 164),
            title: Color::Rgb(255, 184, 108),
            highlight: Color::Rgb(241, 250, 140),
            selection_bg: Color::Rgb(68, 71, 90),
            cursor: Color::Rgb(248, 250, 252),

            // 工具状态颜色
            tool_pending: Color::Rgb(98, 114, 164),
            tool_running: Color::Rgb(255, 184, 108),
            tool_success: Color::Rgb(80, 250, 123),
            tool_failed: Color::Rgb(255, 85, 85),

            // 状态栏颜色
            status_bar_bg: Color::Rgb(68, 71, 90),
            status_bar_fg: Color::Rgb(248, 250, 252),

            // Token/成本颜色
            token_count: Color::Rgb(189, 147, 249),
            cost: Color::Rgb(80, 250, 123),
            budget_warning: Color::Rgb(255, 184, 108),
        }
    }

    /// 创建 Nord 主题
    pub fn nord() -> Self {
        Self {
            name: "nord".to_string(),

            // 基础颜色
            foreground: Color::Rgb(216, 222, 233), // Snow Storm
            background: Color::Rgb(46, 52, 64),    // Polar Night

            // 消息角色颜色
            user_message: Color::Rgb(163, 190, 140), // Aurora Green
            assistant_message: Color::Rgb(136, 192, 208), // Frost
            system_message: Color::Rgb(235, 203, 139), // Aurora Yellow
            error_message: Color::Rgb(191, 97, 106), // Aurora Red
            warning_message: Color::Rgb(235, 203, 139),
            success_message: Color::Rgb(163, 190, 140),

            // 语法高亮颜色
            keyword: Color::Rgb(180, 142, 173),   // Aurora Purple
            string: Color::Rgb(163, 190, 140),    // Aurora Green
            comment: Color::Rgb(143, 188, 187),   // Frost
            number: Color::Rgb(208, 135, 112),    // Aurora Orange
            function: Color::Rgb(136, 192, 208),  // Frost
            type_name: Color::Rgb(129, 161, 193), // Frost
            variable: Color::Rgb(216, 222, 233),
            operator: Color::Rgb(180, 142, 173),
            bracket: Color::Rgb(235, 203, 139),
            punctuation: Color::Rgb(143, 188, 187),

            // UI 组件颜色
            border: Color::Rgb(94, 129, 172), // Frost
            title: Color::Rgb(136, 192, 208),
            highlight: Color::Rgb(235, 203, 139),
            selection_bg: Color::Rgb(59, 66, 82), // Polar Night
            cursor: Color::Rgb(216, 222, 233),

            // 工具状态颜色
            tool_pending: Color::Rgb(143, 188, 187),
            tool_running: Color::Rgb(235, 203, 139),
            tool_success: Color::Rgb(163, 190, 140),
            tool_failed: Color::Rgb(191, 97, 106),

            // 状态栏颜色
            status_bar_bg: Color::Rgb(59, 66, 82),
            status_bar_fg: Color::Rgb(216, 222, 233),

            // Token/成本颜色
            token_count: Color::Rgb(180, 142, 173),
            cost: Color::Rgb(163, 190, 140),
            budget_warning: Color::Rgb(235, 203, 139),
        }
    }

    /// 创建高对比度深色主题 (WCAG AA compliant)
    /// 背景纯黑，前景纯白，对比度 21:1 (远超 AA 4.5:1)
    pub fn high_contrast_dark() -> Self {
        Self {
            name: "high_contrast_dark".to_string(),

            // 基础颜色 - 最高对比度
            foreground: Color::Rgb(255, 255, 255), // 纯白
            background: Color::Rgb(0, 0, 0),       // 纯黑

            // 消息角色颜色 - 高对比度
            user_message: Color::Rgb(0, 255, 0),        // 纯绿
            assistant_message: Color::Rgb(0, 255, 255), // 纯青
            system_message: Color::Rgb(255, 255, 0),    // 纯黄
            error_message: Color::Rgb(255, 0, 0),       // 纯红
            warning_message: Color::Rgb(255, 255, 0),   // 纯黄
            success_message: Color::Rgb(0, 255, 0),     // 纯绿

            // 语法高亮颜色 - 高对比度
            keyword: Color::Rgb(255, 0, 255),       // 纯紫
            string: Color::Rgb(0, 255, 0),          // 纯绿
            comment: Color::Rgb(192, 192, 192),     // 亮灰
            number: Color::Rgb(255, 255, 0),        // 纯黄
            function: Color::Rgb(0, 255, 255),      // 纯青
            type_name: Color::Rgb(255, 0, 255),     // 纯紫
            variable: Color::Rgb(255, 255, 255),    // 纯白
            operator: Color::Rgb(255, 255, 0),      // 纯黄
            bracket: Color::Rgb(255, 255, 0),       // 纯黄
            punctuation: Color::Rgb(192, 192, 192), // 亮灰

            // UI 组件颜色 - 高对比度
            border: Color::Rgb(255, 255, 255),    // 纯白
            title: Color::Rgb(255, 255, 0),       // 纯黄
            highlight: Color::Rgb(255, 255, 0),   // 纯黄
            selection_bg: Color::Rgb(48, 48, 48), // 深灰背景
            cursor: Color::Rgb(255, 255, 255),    // 纯白

            // 工具状态颜色 - 高对比度
            tool_pending: Color::Rgb(192, 192, 192), // 亮灰
            tool_running: Color::Rgb(255, 255, 0),   // 纯黄
            tool_success: Color::Rgb(0, 255, 0),     // 纯绿
            tool_failed: Color::Rgb(255, 0, 0),      // 纯红

            // 状态栏颜色 - 高对比度
            status_bar_bg: Color::Rgb(0, 0, 0),       // 纯黑
            status_bar_fg: Color::Rgb(255, 255, 255), // 纯白

            // Token/成本颜色 - 高对比度
            token_count: Color::Rgb(255, 0, 255),    // 纯紫
            cost: Color::Rgb(0, 255, 0),             // 纯绿
            budget_warning: Color::Rgb(255, 255, 0), // 纯黄
        }
    }

    /// 创建高对比度浅色主题 (WCAG AA compliant)
    /// 背景纯白，前景纯黑，对比度 21:1 (远超 AA 4.5:1)
    pub fn high_contrast_light() -> Self {
        Self {
            name: "high_contrast_light".to_string(),

            // 基础颜色 - 最高对比度
            foreground: Color::Rgb(0, 0, 0),       // 纯黑
            background: Color::Rgb(255, 255, 255), // 纯白

            // 消息角色颜色 - 高对比度深色
            user_message: Color::Rgb(0, 128, 0),      // 深绿
            assistant_message: Color::Rgb(0, 0, 128), // 深蓝
            system_message: Color::Rgb(128, 128, 0),  // 深黄绿
            error_message: Color::Rgb(128, 0, 0),     // 深红
            warning_message: Color::Rgb(128, 128, 0), // 深黄绿
            success_message: Color::Rgb(0, 128, 0),   // 深绿

            // 语法高亮颜色 - 高对比度深色
            keyword: Color::Rgb(128, 0, 128),    // 深紫
            string: Color::Rgb(0, 128, 0),       // 深绿
            comment: Color::Rgb(64, 64, 64),     // 深灰
            number: Color::Rgb(0, 0, 128),       // 深蓝
            function: Color::Rgb(0, 0, 128),     // 深蓝
            type_name: Color::Rgb(128, 0, 128),  // 深紫
            variable: Color::Rgb(0, 0, 0),       // 纯黑
            operator: Color::Rgb(128, 128, 0),   // 深黄绿
            bracket: Color::Rgb(0, 0, 128),      // 深蓝
            punctuation: Color::Rgb(64, 64, 64), // 深灰

            // UI 组件颜色 - 高对比度
            border: Color::Rgb(0, 0, 0),             // 纯黑
            title: Color::Rgb(0, 0, 128),            // 深蓝
            highlight: Color::Rgb(0, 0, 128),        // 深蓝
            selection_bg: Color::Rgb(192, 192, 192), // 浅灰背景
            cursor: Color::Rgb(0, 0, 0),             // 纯黑

            // 工具状态颜色 - 高对比度
            tool_pending: Color::Rgb(64, 64, 64),  // 深灰
            tool_running: Color::Rgb(128, 128, 0), // 深黄绿
            tool_success: Color::Rgb(0, 128, 0),   // 深绿
            tool_failed: Color::Rgb(128, 0, 0),    // 深红

            // 状态栏颜色 - 高对比度
            status_bar_bg: Color::Rgb(192, 192, 192), // 浅灰
            status_bar_fg: Color::Rgb(0, 0, 0),       // 纯黑

            // Token/成本颜色 - 高对比度
            token_count: Color::Rgb(128, 0, 128),    // 深紫
            cost: Color::Rgb(0, 128, 0),             // 深绿
            budget_warning: Color::Rgb(128, 128, 0), // 深黄绿
        }
    }

    /// 根据名称获取主题
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "dark" => Self::dark(),
            "light" => Self::light(),
            "monokai" => Self::monokai(),
            "dracula" => Self::dracula(),
            "nord" => Self::nord(),
            "high_contrast_dark" => Self::high_contrast_dark(),
            "high_contrast_light" => Self::high_contrast_light(),
            _ => Self::dark(),
        }
    }

    /// 获取所有可用主题名称
    pub fn available_themes() -> Vec<&'static str> {
        vec![
            "dark",
            "light",
            "monokai",
            "dracula",
            "nord",
            "high_contrast_dark",
            "high_contrast_light",
        ]
    }
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self::dark()
    }
}

/// 主题管理器
pub struct ThemeManager {
    /// 当前主题
    current_theme: ColorTheme,
    /// 可用主题列表
    themes: HashMap<String, ColorTheme>,
}

impl ThemeManager {
    /// 创建新的主题管理器
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        // 注册内置主题
        let dark = ColorTheme::dark();
        let light = ColorTheme::light();
        let monokai = ColorTheme::monokai();
        let dracula = ColorTheme::dracula();
        let nord = ColorTheme::nord();
        let high_contrast_dark = ColorTheme::high_contrast_dark();
        let high_contrast_light = ColorTheme::high_contrast_light();

        themes.insert(dark.name.clone(), dark);
        themes.insert(light.name.clone(), light);
        themes.insert(monokai.name.clone(), monokai);
        themes.insert(dracula.name.clone(), dracula);
        themes.insert(nord.name.clone(), nord);
        themes.insert(high_contrast_dark.name.clone(), high_contrast_dark);
        themes.insert(high_contrast_light.name.clone(), high_contrast_light);

        Self {
            current_theme: ColorTheme::dark(),
            themes,
        }
    }

    /// 获取当前主题
    pub fn current(&self) -> &ColorTheme {
        &self.current_theme
    }

    /// 切换主题
    pub fn set_theme(&mut self, name: &str) -> bool {
        if let Some(theme) = self.themes.get(name) {
            self.current_theme = theme.clone();
            true
        } else {
            false
        }
    }

    /// 切换到下一个主题
    pub fn next_theme(&mut self) {
        let themes = ColorTheme::available_themes();
        let current_idx = themes
            .iter()
            .position(|t| *t == self.current_theme.name.as_str())
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % themes.len();
        self.set_theme(themes[next_idx]);
    }

    /// 切换到上一个主题
    pub fn prev_theme(&mut self) {
        let themes = ColorTheme::available_themes();
        let current_idx = themes
            .iter()
            .position(|t| *t == self.current_theme.name.as_str())
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            themes.len() - 1
        } else {
            current_idx - 1
        };
        self.set_theme(themes[prev_idx]);
    }

    /// 注册自定义主题
    pub fn register_theme(&mut self, theme: ColorTheme) {
        self.themes.insert(theme.name.clone(), theme);
    }

    /// 列出所有主题名称
    pub fn list_themes(&self) -> Vec<&String> {
        self.themes.keys().collect()
    }

    /// 从配置文件加载主题
    pub fn load_from_config(config_path: &str) -> Option<ColorTheme> {
        // 尝试读取 JSON 配置文件
        if let Ok(content) = std::fs::read_to_string(config_path) {
            if let Ok(theme) = serde_json::from_str::<ColorThemeConfig>(&content) {
                return Some(theme.into_color_theme());
            }
        }
        None
    }

    /// 保存当前主题到配置文件
    pub fn save_to_config(&self, config_path: &str) -> std::io::Result<()> {
        let config = ColorThemeConfig::from_color_theme(&self.current_theme);
        let content = serde_json::to_string_pretty(&config).unwrap_or_default();
        std::fs::write(config_path, content)
    }
}

/// 可序列化的主题配置（用于配置文件）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColorThemeConfig {
    /// 主题名称
    pub name: String,

    // === 基础颜色（使用 RGB 字符串格式）===
    pub foreground: String,
    pub background: String,

    // === Markdown 颜色 ===
    pub md_heading: String,
    pub md_paragraph: String,
    pub md_link: String,
    pub md_blockquote: String,
    pub md_inline_code: String,

    // === 语法高亮颜色 ===
    pub syntax_keyword: String,
    pub syntax_string: String,
    pub syntax_comment: String,
    pub syntax_number: String,
    pub syntax_function: String,
    pub syntax_type: String,
    pub syntax_variable: String,
    pub syntax_operator: String,

    // === UI 颜色 ===
    pub ui_border: String,
    pub ui_title: String,
    pub ui_highlight: String,
    pub ui_selection_bg: String,

    // === 消息颜色 ===
    pub msg_user: String,
    pub msg_assistant: String,
    pub msg_system: String,
    pub msg_error: String,
    pub msg_warning: String,
    pub msg_success: String,
}

impl ColorThemeConfig {
    /// 从 ColorTheme 转换
    pub fn from_color_theme(theme: &ColorTheme) -> Self {
        Self {
            name: theme.name.clone(),
            foreground: color_to_hex(theme.foreground),
            background: color_to_hex(theme.background),
            md_heading: color_to_hex(theme.title),
            md_paragraph: color_to_hex(theme.foreground),
            md_link: color_to_hex(theme.type_name),
            md_blockquote: color_to_hex(theme.border),
            md_inline_code: color_to_hex(theme.function),
            syntax_keyword: color_to_hex(theme.keyword),
            syntax_string: color_to_hex(theme.string),
            syntax_comment: color_to_hex(theme.comment),
            syntax_number: color_to_hex(theme.number),
            syntax_function: color_to_hex(theme.function),
            syntax_type: color_to_hex(theme.type_name),
            syntax_variable: color_to_hex(theme.variable),
            syntax_operator: color_to_hex(theme.operator),
            ui_border: color_to_hex(theme.border),
            ui_title: color_to_hex(theme.title),
            ui_highlight: color_to_hex(theme.highlight),
            ui_selection_bg: color_to_hex(theme.selection_bg),
            msg_user: color_to_hex(theme.user_message),
            msg_assistant: color_to_hex(theme.assistant_message),
            msg_system: color_to_hex(theme.system_message),
            msg_error: color_to_hex(theme.error_message),
            msg_warning: color_to_hex(theme.warning_message),
            msg_success: color_to_hex(theme.success_message),
        }
    }

    /// 转换为 ColorTheme
    pub fn into_color_theme(self) -> ColorTheme {
        ColorTheme {
            name: self.name,
            foreground: hex_to_color(&self.foreground),
            background: hex_to_color(&self.background),
            title: hex_to_color(&self.md_heading),
            highlight: hex_to_color(&self.md_link),
            border: hex_to_color(&self.md_blockquote),
            function: hex_to_color(&self.md_inline_code),
            keyword: hex_to_color(&self.syntax_keyword),
            string: hex_to_color(&self.syntax_string),
            comment: hex_to_color(&self.syntax_comment),
            number: hex_to_color(&self.syntax_number),
            type_name: hex_to_color(&self.syntax_type),
            variable: hex_to_color(&self.syntax_variable),
            operator: hex_to_color(&self.syntax_operator),
            selection_bg: hex_to_color(&self.ui_selection_bg),
            user_message: hex_to_color(&self.msg_user),
            assistant_message: hex_to_color(&self.msg_assistant),
            system_message: hex_to_color(&self.msg_system),
            error_message: hex_to_color(&self.msg_error),
            warning_message: hex_to_color(&self.msg_warning),
            success_message: hex_to_color(&self.msg_success),
            cursor: Color::White,
            token_count: Color::Magenta,
            cost: Color::Green,
            budget_warning: Color::Rgb(255, 165, 0),
            tool_pending: Color::Gray,
            tool_running: Color::Yellow,
            tool_success: Color::Green,
            tool_failed: Color::Red,
            status_bar_bg: Color::DarkGray,
            status_bar_fg: Color::White,
            punctuation: Color::Gray,
            bracket: Color::Rgb(255, 215, 0),
        }
    }
}

/// 将 Color 转换为十六进制字符串
fn color_to_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::Red => "#ff0000".to_string(),
        Color::Green => "#00ff00".to_string(),
        Color::Yellow => "#ffff00".to_string(),
        Color::Blue => "#0000ff".to_string(),
        Color::Magenta => "#ff00ff".to_string(),
        Color::Cyan => "#00ffff".to_string(),
        Color::White => "#ffffff".to_string(),
        Color::Black => "#000000".to_string(),
        Color::Gray => "#808080".to_string().to_string(),
        Color::DarkGray => "#404040".to_string(),
        Color::LightRed => "#ff8080".to_string(),
        Color::LightGreen => "#80ff80".to_string(),
        Color::LightYellow => "#ffff80".to_string(),
        Color::LightBlue => "#8080ff".to_string(),
        Color::LightMagenta => "#ff80ff".to_string(),
        Color::LightCyan => "#80ffff".to_string(),
        Color::Reset => "#ffffff".to_string(),
        _ => "#ffffff".to_string(),
    }
    .to_string()
}

/// 将十六进制字符串转换为 Color
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        Color::Rgb(r, g, b)
    } else {
        Color::White
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme() {
        let theme = ColorTheme::dark();
        assert_eq!(theme.name, "dark");
        assert_eq!(theme.foreground, Color::White);
    }

    #[test]
    fn test_light_theme() {
        let theme = ColorTheme::light();
        assert_eq!(theme.name, "light");
        assert_eq!(theme.foreground, Color::Black);
    }

    #[test]
    fn test_monokai_theme() {
        let theme = ColorTheme::monokai();
        assert_eq!(theme.name, "monokai");
    }

    #[test]
    fn test_theme_from_name() {
        let theme = ColorTheme::from_name("dracula");
        assert_eq!(theme.name, "dracula");

        let unknown = ColorTheme::from_name("unknown");
        assert_eq!(unknown.name, "dark");
    }

    #[test]
    fn test_theme_manager() {
        let mut manager = ThemeManager::new();
        assert_eq!(manager.current().name, "dark");

        manager.set_theme("light");
        assert_eq!(manager.current().name, "light");

        manager.next_theme();
        assert_eq!(manager.current().name, "monokai");
    }

    #[test]
    fn test_available_themes() {
        let themes = ColorTheme::available_themes();
        assert_eq!(themes.len(), 7);
        assert!(themes.contains(&"dark"));
        assert!(themes.contains(&"light"));
        assert!(themes.contains(&"high_contrast_dark"));
        assert!(themes.contains(&"high_contrast_light"));
    }

    #[test]
    fn test_high_contrast_dark_theme() {
        let theme = ColorTheme::high_contrast_dark();
        assert_eq!(theme.name, "high_contrast_dark");
        // WCAG AA: 黑底白字对比度 21:1，远超 4.5:1
        assert_eq!(theme.foreground, Color::Rgb(255, 255, 255));
        assert_eq!(theme.background, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn test_high_contrast_light_theme() {
        let theme = ColorTheme::high_contrast_light();
        assert_eq!(theme.name, "high_contrast_light");
        // WCAG AA: 白底黑字对比度 21:1，远超 4.5:1
        assert_eq!(theme.foreground, Color::Rgb(0, 0, 0));
        assert_eq!(theme.background, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn test_high_contrast_from_name() {
        let dark = ColorTheme::from_name("high_contrast_dark");
        assert_eq!(dark.name, "high_contrast_dark");

        let light = ColorTheme::from_name("high_contrast_light");
        assert_eq!(light.name, "high_contrast_light");
    }

    #[test]
    fn test_custom_theme() {
        let mut manager = ThemeManager::new();
        let custom = ColorTheme {
            name: "custom".to_string(),
            ..ColorTheme::dark()
        };
        manager.register_theme(custom);
        assert!(manager.set_theme("custom"));
    }
}
