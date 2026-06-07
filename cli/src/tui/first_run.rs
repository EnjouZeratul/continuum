//! 首次启动检测和状态管理

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// 首次启动状态
#[derive(Debug, Clone)]
pub struct FirstRunState {
    /// 是否首次运行
    pub is_first_run: bool,
    /// 是否已完成教程
    pub tutorial_completed: bool,
    /// 是否已显示欢迎信息
    pub welcome_shown: bool,
    /// 是否已完成配置向导
    pub setup_completed: bool,
    /// 是否跳过了配置向导
    pub setup_skipped: bool,
    /// 检测到的提供商
    pub detected_provider: Option<String>,
    /// 用户配置目录
    config_dir: PathBuf,
}

impl FirstRunState {
    /// 创建新的首次启动状态检测器
    pub fn new() -> Result<Self> {
        let config_dir = Self::get_config_dir()?;
        let state_file = config_dir.join(".first_run");

        let exists = state_file.exists();
        let state = if exists {
            Self::load_state(&state_file)?
        } else {
            Self {
                is_first_run: true,
                tutorial_completed: false,
                welcome_shown: false,
                setup_completed: false,
                setup_skipped: false,
                detected_provider: None,
                config_dir,
            }
        };

        Ok(state)
    }

    /// 获取配置目录
    fn get_config_dir() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "windows") {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        };
        Ok(config_dir.join("continuum"))
    }

    /// 加载状态文件
    fn load_state(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();

        let config_dir = Self::get_config_dir()?;

        Ok(Self {
            is_first_run: false,
            tutorial_completed: lines
                .iter()
                .any(|l| l.starts_with("tutorial_completed=true")),
            welcome_shown: lines.iter().any(|l| l.starts_with("welcome_shown=true")),
            setup_completed: lines.iter().any(|l| l.starts_with("setup_completed=true")),
            setup_skipped: lines.iter().any(|l| l.starts_with("setup_skipped=true")),
            detected_provider: lines
                .iter()
                .find(|l| l.starts_with("detected_provider="))
                .and_then(|l| l.strip_prefix("detected_provider=").map(|s| s.to_string())),
            config_dir,
        })
    }

    /// 保存状态
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)?;

        let state_file = self.config_dir.join(".first_run");
        let content = format!(
            "tutorial_completed={}\nwelcome_shown={}\nsetup_completed={}\nsetup_skipped={}\ndetected_provider={}\n",
            self.tutorial_completed,
            self.welcome_shown,
            self.setup_completed,
            self.setup_skipped,
            self.detected_provider.as_deref().unwrap_or("")
        );

        fs::write(state_file, content)?;
        Ok(())
    }

    /// 标记教程已完成
    pub fn mark_tutorial_completed(&mut self) -> Result<()> {
        self.tutorial_completed = true;
        self.is_first_run = false;
        self.save()
    }

    /// 标记欢迎信息已显示
    pub fn mark_welcome_shown(&mut self) -> Result<()> {
        self.welcome_shown = true;
        self.save()
    }

    /// 标记首次启动完成
    pub fn mark_first_run_done(&mut self) -> Result<()> {
        self.is_first_run = false;
        self.welcome_shown = true;
        self.save()
    }

    /// 标记配置向导已完成
    pub fn mark_setup_completed(&mut self, provider: &str) -> Result<()> {
        self.setup_completed = true;
        self.setup_skipped = false;
        self.detected_provider = Some(provider.to_string());
        self.is_first_run = false;
        self.save()
    }

    /// 标记配置向导已跳过
    pub fn mark_setup_skipped(&mut self) -> Result<()> {
        self.setup_skipped = true;
        self.save()
    }

    /// 检查是否需要配置
    pub fn needs_setup(&self) -> bool {
        !self.setup_completed && !self.setup_skipped
    }

    /// 获取欢迎消息
    pub fn get_welcome_message() -> String {
        let version = env!("CARGO_PKG_VERSION");
        format!(
            "Welcome to Continuum v{}!\n\n\
            Continuum is your AI-powered terminal assistant.\n\
            It can help you with coding, file operations, and system tasks.\n\n\
            Quick Start:\n\
            - Type your question and press Enter\n\
            - Press F1 or Ctrl+? for keyboard shortcuts\n\
            - Type /help for all commands\n\
            - Type /tutorial for interactive guide\n\n\
            Press Enter to continue...",
            version
        )
    }

    /// 获取首次启动提示
    pub fn get_first_run_hint() -> String {
        "It looks like this is your first time using Continuum!\n\
         Would you like to take a quick tutorial? (Y/n)"
            .to_string()
    }
}

impl Default for FirstRunState {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            is_first_run: true,
            tutorial_completed: false,
            welcome_shown: false,
            setup_completed: false,
            setup_skipped: false,
            detected_provider: None,
            config_dir: PathBuf::from("."),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_message() {
        let msg = FirstRunState::get_welcome_message();
        assert!(msg.contains("Welcome to Continuum"));
        assert!(msg.contains("/help"));
        assert!(msg.contains("/tutorial"));
    }

    #[test]
    fn test_first_run_hint() {
        let hint = FirstRunState::get_first_run_hint();
        assert!(hint.contains("first time"));
        assert!(hint.contains("tutorial"));
    }

    #[test]
    fn test_default_state() {
        let state = FirstRunState::default();
        // May or may not be first run depending on existing config
        assert!(!state.config_dir.to_string_lossy().is_empty());
    }

    #[test]
    fn test_needs_setup() {
        let mut state = FirstRunState {
            is_first_run: true,
            tutorial_completed: false,
            welcome_shown: false,
            setup_completed: false,
            setup_skipped: false,
            detected_provider: None,
            config_dir: PathBuf::from("."),
        };
        assert!(state.needs_setup());

        state.setup_completed = true;
        assert!(!state.needs_setup());

        state.setup_completed = false;
        state.setup_skipped = true;
        assert!(!state.needs_setup());
    }

    #[test]
    fn test_mark_setup_completed() {
        let mut state = FirstRunState {
            is_first_run: true,
            tutorial_completed: false,
            welcome_shown: false,
            setup_completed: false,
            setup_skipped: false,
            detected_provider: None,
            config_dir: PathBuf::from("."),
        };
        // Note: This will fail to save but should still update state
        let _ = state.mark_setup_completed("anthropic");
        assert!(state.setup_completed);
        assert!(!state.setup_skipped);
        assert_eq!(state.detected_provider, Some("anthropic".to_string()));
    }
}
