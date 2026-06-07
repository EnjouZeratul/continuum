//! Tutorial 新手教程内容

/// 教程步骤
#[derive(Debug, Clone)]
pub struct TutorialStep {
    /// 步骤编号
    pub number: usize,
    /// 步骤标题
    pub title: String,
    /// 步骤描述
    pub description: String,
    /// 详细说明
    pub details: Vec<String>,
    /// 快捷键提示
    pub keybindings: Vec<String>,
    /// 提示
    pub tips: Vec<String>,
}

macro_rules! tutorial_step {
    ($num:expr, $title:expr, $desc:expr, [$($detail:expr),*], [$($kb:expr),*], [$($tip:expr),*]) => {
        TutorialStep {
            number: $num,
            title: $title.to_string(),
            description: $desc.to_string(),
            details: vec![$($detail.to_string()),*],
            keybindings: vec![$($kb.to_string()),*],
            tips: vec![$($tip.to_string()),*],
        }
    };
}

/// 教程内容
pub struct Tutorial {
    /// 当前步骤
    current_step: usize,
    /// 总步骤数
    total_steps: usize,
    /// 步骤列表
    steps: Vec<TutorialStep>,
}

impl Tutorial {
    /// 创建新教程
    pub fn new() -> Self {
        Self {
            current_step: 0,
            total_steps: 5,
            steps: Self::create_steps(),
        }
    }

    /// 创建教程步骤
    fn create_steps() -> Vec<TutorialStep> {
        vec![
            tutorial_step!(
                1,
                "Welcome to Continuum",
                "Let's learn how to use the terminal interface.",
                [
                    "Continuum is a powerful AI assistant that runs in your terminal.",
                    "It can help you with coding, file operations, and system tasks.",
                    "This tutorial will guide you through the basic features."
                ],
                [
                    "Ctrl+C / Ctrl+D - Quit the application",
                    "Enter - Send your message",
                    "Esc - Cancel current input"
                ],
                [
                    "You can skip this tutorial at any time by pressing Esc.",
                    "Use /help to see all available commands."
                ]
            ),
            tutorial_step!(
                2,
                "Sending Messages",
                "How to communicate with the AI assistant.",
                [
                    "Type your question or request in the input area at the bottom.",
                    "Press Enter to send your message to the AI.",
                    "The AI will process your request and respond in the chat area.",
                    "You can use multi-line input by pressing Alt+Enter."
                ],
                [
                    "Enter - Send message",
                    "Alt+Enter / Shift+Enter - Insert new line",
                    "Ctrl+W - Delete previous word",
                    "Up/Down - Navigate input history"
                ],
                [
                    "Try typing: 'Hello, what can you help me with?'",
                    "Press Tab to autocomplete slash commands."
                ]
            ),
            tutorial_step!(
                3,
                "Slash Commands",
                "Special commands to control the application.",
                [
                    "Slash commands start with / and provide special functionality.",
                    "Common commands: /help, /clear, /new, /tokens",
                    "File commands: /read, /write, /edit",
                    "Git commands: /git status, /git diff"
                ],
                [
                    "/help - Show all commands",
                    "/clear - Clear chat history",
                    "/new - Start a new session",
                    "Tab - Autocomplete commands"
                ],
                [
                    "Some commands require confirmation before execution.",
                    "Use /help <command> for detailed help on a specific command."
                ]
            ),
            tutorial_step!(
                4,
                "Tools Panel",
                "Viewing tool execution in real-time.",
                [
                    "When the AI uses tools, they appear in the tools panel.",
                    "You can see which tools are being called and their results.",
                    "Toggle the tools panel to see more details.",
                    "Tools include: file operations, shell commands, and more."
                ],
                [
                    "Ctrl+T - Toggle tools panel",
                    "The panel shows tool name, input, and output."
                ],
                [
                    "Some tools require your permission before execution.",
                    "You can grant permanent permission for trusted tools."
                ]
            ),
            tutorial_step!(
                5,
                "Session Management",
                "Managing your conversation sessions.",
                [
                    "Each conversation is a session with its own history.",
                    "You can start new sessions, save current ones.",
                    "Session information appears in the status bar.",
                    "The AI remembers context within the same session."
                ],
                [
                    "Ctrl+N - New session",
                    "Ctrl+S - Save session",
                    "Ctrl+L - Clear screen",
                    "Ctrl+H - Show help"
                ],
                [
                    "Use /new to start fresh when switching tasks.",
                    "Check /tokens to see token usage statistics.",
                    "Press Ctrl+? to see all available shortcuts."
                ]
            ),
        ]
    }

    /// 获取当前步骤
    pub fn current_step(&self) -> Option<&TutorialStep> {
        if self.current_step > 0 && self.current_step <= self.total_steps {
            self.steps.get(self.current_step - 1)
        } else {
            None
        }
    }

    /// 获取指定步骤
    pub fn get_step(&self, number: usize) -> Option<&TutorialStep> {
        if number > 0 && number <= self.total_steps {
            self.steps.get(number - 1)
        } else {
            None
        }
    }

    /// 开始教程
    pub fn start(&mut self) -> Option<&TutorialStep> {
        self.current_step = 1;
        self.current_step()
    }

    /// 下一步
    pub fn next_step(&mut self) -> Option<&TutorialStep> {
        if self.current_step < self.total_steps {
            self.current_step += 1;
            self.current_step()
        } else {
            None
        }
    }

    /// 上一步
    pub fn prev_step(&mut self) -> Option<&TutorialStep> {
        if self.current_step > 1 {
            self.current_step -= 1;
            self.current_step()
        } else {
            None
        }
    }

    /// 跳转到指定步骤
    pub fn jump_to(&mut self, step: usize) -> Option<&TutorialStep> {
        if step > 0 && step <= self.total_steps {
            self.current_step = step;
            self.current_step()
        } else {
            None
        }
    }

    /// 重置教程
    pub fn reset(&mut self) {
        self.current_step = 0;
    }

    /// 是否已完成
    pub fn is_complete(&self) -> bool {
        self.current_step >= self.total_steps
    }

    /// 获取总步骤数
    pub fn total_steps(&self) -> usize {
        self.total_steps
    }

    /// 获取当前步骤编号
    pub fn current_step_number(&self) -> usize {
        self.current_step
    }

    /// 获取教程概述
    pub fn overview() -> String {
        "Welcome to Continuum Tutorial!\n\n\
        This interactive guide will help you learn the basics of using Continuum.\n\n\
        Topics covered:\n\
        1. Introduction and basic navigation\n\
        2. Sending messages to the AI\n\
        3. Using slash commands\n\
        4. Viewing tool execution\n\
        5. Session management\n\n\
        Use /tutorial <step> to jump to a specific step, or /tutorial 1 to start from the beginning.\n\n\
        Press Esc to dismiss this message."
            .to_string()
    }

    /// 格式化步骤为显示文本
    pub fn format_step(step: &TutorialStep) -> String {
        let mut text = format!(
            "Step {} of 5: {}\n\n{}\n\n",
            step.number, step.title, step.description
        );

        text.push_str("Details:\n");
        for detail in &step.details {
            text.push_str(&format!("  • {}\n", detail));
        }

        text.push_str("\nKeybindings:\n");
        for kb in &step.keybindings {
            text.push_str(&format!(
                "  {} {}\n",
                kb.split('-').next().unwrap_or(""),
                kb
            ));
        }

        text.push_str("\nTips:\n");
        for tip in &step.tips {
            text.push_str(&format!("  → {}\n", tip));
        }

        if step.number < 5 {
            text.push_str("\n[Press Enter for next step, Esc to dismiss]");
        } else {
            text.push_str("\n[Tutorial complete! Press Esc to dismiss]");
        }

        text
    }
}

impl Default for Tutorial {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tutorial_creation() {
        let tutorial = Tutorial::new();
        assert_eq!(tutorial.total_steps(), 5);
        assert_eq!(tutorial.current_step_number(), 0);
    }

    #[test]
    fn test_start_tutorial() {
        let mut tutorial = Tutorial::new();
        tutorial.start();

        assert_eq!(tutorial.current_step_number(), 1);
        let step = tutorial.current_step();
        assert!(step.is_some());
        assert_eq!(step.unwrap().number, 1);
    }

    #[test]
    fn test_next_step() {
        let mut tutorial = Tutorial::new();
        tutorial.start();

        tutorial.next_step();
        assert_eq!(tutorial.current_step_number(), 2);
        let step = tutorial.current_step();
        assert!(step.is_some());
    }

    #[test]
    fn test_prev_step() {
        let mut tutorial = Tutorial::new();
        tutorial.start();
        tutorial.next_step();

        tutorial.prev_step();
        assert_eq!(tutorial.current_step_number(), 1);
        let step = tutorial.current_step();
        assert!(step.is_some());
    }

    #[test]
    fn test_jump_to() {
        let mut tutorial = Tutorial::new();
        tutorial.jump_to(3);

        assert_eq!(tutorial.current_step_number(), 3);
        let step = tutorial.current_step();
        assert!(step.is_some());
    }

    #[test]
    fn test_jump_invalid() {
        let mut tutorial = Tutorial::new();
        let step = tutorial.jump_to(10);

        assert!(step.is_none());
    }

    #[test]
    fn test_is_complete() {
        let mut tutorial = Tutorial::new();
        assert!(!tutorial.is_complete());

        tutorial.jump_to(5);
        assert!(tutorial.is_complete());
    }

    #[test]
    fn test_reset() {
        let mut tutorial = Tutorial::new();
        tutorial.start();
        tutorial.reset();

        assert_eq!(tutorial.current_step_number(), 0);
    }

    #[test]
    fn test_get_step() {
        let tutorial = Tutorial::new();
        let step = tutorial.get_step(2);

        assert!(step.is_some());
        assert_eq!(step.unwrap().title, "Sending Messages");
    }

    #[test]
    fn test_format_step() {
        let tutorial = Tutorial::new();
        let step = tutorial.get_step(1).unwrap();
        let text = Tutorial::format_step(step);

        assert!(text.contains("Step 1 of 5"));
        assert!(text.contains("Welcome to Continuum"));
    }

    #[test]
    fn test_overview() {
        let overview = Tutorial::overview();
        assert!(overview.contains("Welcome to Continuum Tutorial"));
        assert!(overview.contains("5"));
    }

    #[test]
    fn test_default() {
        let tutorial = Tutorial::default();
        assert_eq!(tutorial.total_steps(), 5);
    }
}
