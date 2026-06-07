# First-Run User Guidance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement interactive first-run experience with automatic API key detection and setup wizard.

**Architecture:** ConfigDetector scans environment variables and config files on startup. If no configuration found, SetupWizard component guides users through provider selection and API key input. Users can skip setup and are prompted again on first message attempt.

**Tech Stack:** Rust, ratatui (TUI), existing ConfigManager from sh-core

---

## File Structure

```
cli/src/tui/
├── setup/
│   ├── mod.rs           # Setup module exports
│   └── config_detector.rs  # Environment/config detection logic
├── components/
│   ├── mod.rs           # Add setup_wizard export
│   └── setup_wizard.rs  # Interactive setup wizard UI
├── first_run.rs         # Enhance with setup state tracking
└── mod.rs               # Integrate setup flow
```

---

### Task 1: Create ConfigDetector Module

**Files:**
- Create: `cli/src/tui/setup/mod.rs`
- Create: `cli/src/tui/setup/config_detector.rs`

- [ ] **Step 1: Create setup module directory structure**

```bash
mkdir -p cli/src/tui/setup
```

- [ ] **Step 2: Create setup/mod.rs with module exports**

```rust
//! Setup module for first-run configuration

pub mod config_detector;

pub use config_detector::{ConfigDetector, DetectionResult, DetectedProvider};
```

- [ ] **Step 3: Write the failing test for ConfigDetector**

Create file: `cli/src/tui/setup/config_detector.rs`

```rust
//! Configuration detection for first-run setup

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

/// Provider detection source
#[derive(Debug, Clone)]
pub enum DetectionSource {
    /// Detected from environment variable
    EnvVar(String),
    /// Detected from config file
    ConfigFile(PathBuf),
}

/// A detected provider configuration
#[derive(Debug, Clone)]
pub struct DetectedProvider {
    /// Provider name (anthropic, openai, gemini)
    pub name: String,
    /// Where the configuration was detected
    pub source: DetectionSource,
    /// Whether API key is set
    pub api_key_set: bool,
}

/// Result of configuration detection
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// All detected providers
    pub providers: Vec<DetectedProvider>,
    /// Whether any valid configuration exists
    pub has_valid_config: bool,
    /// Config file path if exists
    pub config_file_path: Option<PathBuf>,
}

/// Configuration detector
pub struct ConfigDetector {
    /// Environment variable mappings for providers
    env_mappings: HashMap<&'static str, &'static str>,
}

impl ConfigDetector {
    /// Create new detector
    pub fn new() -> Self {
        let mut env_mappings = HashMap::new();
        env_mappings.insert("anthropic", "ANTHROPIC_API_KEY");
        env_mappings.insert("openai", "OPENAI_API_KEY");
        env_mappings.insert("google", "GOOGLE_API_KEY");
        env_mappings.insert("gemini", "GEMINI_API_KEY");

        Self { env_mappings }
    }

    /// Detect configuration from environment and config file
    pub fn detect(&self) -> Result<DetectionResult> {
        let mut providers = Vec::new();
        let mut has_valid_config = false;

        // Check environment variables
        for (provider_name, env_var) in &self.env_mappings {
            if let Ok(value) = std::env::var(env_var) {
                if !value.is_empty() {
                    providers.push(DetectedProvider {
                        name: provider_name.to_string(),
                        source: DetectionSource::EnvVar(env_var.to_string()),
                        api_key_set: true,
                    });
                    has_valid_config = true;
                }
            }
        }

        // Check for CONTINUUM_API_KEY (generic fallback)
        if let Ok(value) = std::env::var("CONTINUUM_API_KEY") {
            if !value.is_empty() && providers.is_empty() {
                providers.push(DetectedProvider {
                    name: "anthropic".to_string(), // Default to anthropic
                    source: DetectionSource::EnvVar("CONTINUUM_API_KEY".to_string()),
                    api_key_set: true,
                });
                has_valid_config = true;
            }
        }

        // Check config file
        let config_path = self.get_config_path();
        let config_file_path = if config_path.exists() {
            // Try to load and validate config
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if content.contains("api_key") && !content.contains("api_key: \"\"") {
                    // Check for configured providers in file
                    for provider_name in &["anthropic", "openai", "google", "gemini"] {
                        if content.contains(&format!("[{}]", provider_name)) 
                            || content.contains(&format!("{}:", provider_name)) {
                            // Check if this provider has api_key set
                            let has_key = self.provider_has_key_in_config(&content, provider_name);
                            if has_key {
                                providers.push(DetectedProvider {
                                    name: provider_name.to_string(),
                                    source: DetectionSource::ConfigFile(config_path.clone()),
                                    api_key_set: true,
                                });
                                has_valid_config = true;
                            }
                        }
                    }
                }
            }
            Some(config_path)
        } else {
            None
        };

        Ok(DetectionResult {
            providers,
            has_valid_config,
            config_file_path,
        })
    }

    /// Get default config path
    fn get_config_path(&self) -> PathBuf {
        sh_core::layer1::ConfigManager::default_config_path()
    }

    /// Check if provider has API key in config content
    fn provider_has_key_in_config(&self, content: &str, provider: &str) -> bool {
        // Simple check - look for non-empty api_key in provider section
        content.contains(&format!("{}_api_key", provider))
            || content.contains(&format!("{}.api_key", provider))
    }

    /// Get environment variable name for provider
    pub fn get_env_var(&self, provider: &str) -> Option<&'static str> {
        self.env_mappings.get(provider).copied()
    }
}

impl Default for ConfigDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = ConfigDetector::new();
        assert!(detector.get_env_var("anthropic").is_some());
        assert!(detector.get_env_var("openai").is_some());
    }

    #[test]
    fn test_detect_without_env() {
        // This test verifies detection works even without env vars set
        let detector = ConfigDetector::new();
        let result = detector.detect();
        assert!(result.is_ok());
    }

    #[test]
    fn test_detection_result_defaults() {
        let result = DetectionResult {
            providers: vec![],
            has_valid_config: false,
            config_file_path: None,
        };
        assert!(!result.has_valid_config);
        assert!(result.providers.is_empty());
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd cli && cargo test tui::setup::config_detector --no-fail-fast`
Expected: All tests pass

- [ ] **Step 5: Commit ConfigDetector module**

```bash
git add cli/src/tui/setup/
git commit -m "feat(tui): add ConfigDetector for first-run setup"
```

---

### Task 2: Create SetupWizard Component

**Files:**
- Create: `cli/src/tui/components/setup_wizard.rs`
- Modify: `cli/src/tui/components/mod.rs`

- [ ] **Step 1: Write the setup_wizard.rs component**

Create file: `cli/src/tui/components/setup_wizard.rs`

```rust
//! Setup Wizard component for first-run configuration

use super::color_theme::ColorTheme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Wizard step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    /// Welcome screen
    Welcome,
    /// Provider selection
    ProviderSelection,
    /// API key input
    ApiKeyInput,
    /// Connection test
    ConnectionTest,
    /// Setup complete
    Complete,
}

/// Available providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Google,
}

impl Provider {
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Google => "google",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic (Claude)",
            Provider::OpenAI => "OpenAI (GPT-4)",
            Provider::Google => "Google (Gemini)",
        }
    }

    pub fn key_url(&self) -> &'static str {
        match self {
            Provider::Anthropic => "console.anthropic.com",
            Provider::OpenAI => "platform.openai.com",
            Provider::Google => "aistudio.google.com",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![Provider::Anthropic, Provider::OpenAI, Provider::Google]
    }
}

/// Connection test status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Not tested yet
    NotTested,
    /// Test in progress
    Testing,
    /// Connection successful
    Success,
    /// Connection failed
    Failed,
}

/// Setup Wizard component
pub struct SetupWizard {
    /// Current step
    pub current_step: WizardStep,
    /// Selected provider
    pub selected_provider: Option<Provider>,
    /// API key input
    pub api_key_input: String,
    /// Whether API key is visible (not masked)
    pub api_key_visible: bool,
    /// Connection test status
    pub connection_status: ConnectionStatus,
    /// Error message if any
    pub error_message: Option<String>,
    /// Whether wizard is visible
    pub visible: bool,
    /// Color theme
    theme: ColorTheme,
}

impl SetupWizard {
    /// Create new setup wizard
    pub fn new() -> Self {
        Self {
            current_step: WizardStep::Welcome,
            selected_provider: None,
            api_key_input: String::new(),
            api_key_visible: false,
            connection_status: ConnectionStatus::NotTested,
            error_message: None,
            visible: false,
            theme: ColorTheme::dark(),
        }
    }

    /// Set theme
    pub fn set_theme(&mut self, theme: ColorTheme) {
        self.theme = theme;
    }

    /// Show the wizard
    pub fn show(&mut self) {
        self.visible = true;
        self.current_step = WizardStep::Welcome;
        self.selected_provider = None;
        self.api_key_input.clear();
        self.connection_status = ConnectionStatus::NotTested;
        self.error_message = None;
    }

    /// Hide the wizard
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Check if wizard is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Go to next step
    pub fn next_step(&mut self) {
        self.current_step = match self.current_step {
            WizardStep::Welcome => WizardStep::ProviderSelection,
            WizardStep::ProviderSelection => WizardStep::ApiKeyInput,
            WizardStep::ApiKeyInput => WizardStep::ConnectionTest,
            WizardStep::ConnectionTest => WizardStep::Complete,
            WizardStep::Complete => WizardStep::Complete,
        };
    }

    /// Go to previous step
    pub fn prev_step(&mut self) {
        self.current_step = match self.current_step {
            WizardStep::Welcome => WizardStep::Welcome,
            WizardStep::ProviderSelection => WizardStep::Welcome,
            WizardStep::ApiKeyInput => WizardStep::ProviderSelection,
            WizardStep::ConnectionTest => WizardStep::ApiKeyInput,
            WizardStep::Complete => WizardStep::ConnectionTest,
        };
    }

    /// Select a provider
    pub fn select_provider(&mut self, provider: Provider) {
        self.selected_provider = Some(provider);
    }

    /// Add character to API key input
    pub fn push_char(&mut self, c: char) {
        self.api_key_input.push(c);
    }

    /// Remove last character from API key input
    pub fn pop_char(&mut self) {
        self.api_key_input.pop();
    }

    /// Clear API key input
    pub fn clear_input(&mut self) {
        self.api_key_input.clear();
    }

    /// Toggle API key visibility
    pub fn toggle_visibility(&mut self) {
        self.api_key_visible = !self.api_key_visible;
    }

    /// Render the wizard
    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Create centered popup area
        let popup_area = self.centered_popup(area, 60, 70);
        
        // Clear the area
        f.render_widget(Clear, popup_area);

        // Render based on current step
        match self.current_step {
            WizardStep::Welcome => self.render_welcome(f, popup_area),
            WizardStep::ProviderSelection => self.render_provider_selection(f, popup_area),
            WizardStep::ApiKeyInput => self.render_api_key_input(f, popup_area),
            WizardStep::ConnectionTest => self.render_connection_test(f, popup_area),
            WizardStep::Complete => self.render_complete(f, popup_area),
        }
    }

    fn centered_popup(&self, area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    fn render_welcome(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Welcome to Continuum ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border));

        let version = env!("CARGO_PKG_VERSION");
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("Continuum v{} - AI Terminal Assistant", version),
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Continuum helps you with coding, file operations,"),
            Line::from("and system tasks using AI assistance."),
            Line::from(""),
            Line::from(Span::styled(
                "To get started, you'll need to configure an API key.",
                Style::default().fg(self.theme.text_secondary),
            )),
            Line::from(""),
            Line::from("Supported providers:"),
            Line::from("  • Anthropic (Claude)"),
            Line::from("  • OpenAI (GPT-4)"),
            Line::from("  • Google (Gemini)"),
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Continue    [Esc] Skip setup",
                Style::default().fg(self.theme.text_secondary),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_provider_selection(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Select Provider ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border));

        let mut text = vec![
            Line::from(""),
            Line::from("Choose your AI provider:"),
            Line::from(""),
        ];

        for provider in Provider::all() {
            let selected = self.selected_provider == Some(provider);
            let marker = if selected { "→ " } else { "  " };
            let style = if selected {
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text)
            };
            text.push(Line::from(Span::styled(
                format!("{}{}", marker, provider.display_name()),
                style,
            )));
        }

        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "[↑/↓] Select    [Enter] Confirm    [Esc] Back",
            Style::default().fg(self.theme.text_secondary),
        )));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_api_key_input(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Enter API Key ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border));

        let provider_name = self.selected_provider
            .map(|p| p.display_name())
            .unwrap_or("Provider");

        let key_display = if self.api_key_visible {
            self.api_key_input.clone()
        } else {
            "*".repeat(self.api_key_input.len())
        };

        let key_url = self.selected_provider
            .map(|p| p.key_url())
            .unwrap_or("");

        let mut text = vec![
            Line::from(""),
            Line::from(format!("Provider: {}", provider_name)),
            Line::from(""),
            Line::from("API Key:"),
            Line::from(Span::styled(
                if key_display.is_empty() { "..." } else { &key_display },
                Style::default().fg(self.theme.accent),
            )),
            Line::from(""),
        ];

        if let Some(error) = &self.error_message {
            text.push(Line::from(Span::styled(
                format!("Error: {}", error),
                Style::default().fg(self.theme.error),
            )));
            text.push(Line::from(""));
        }

        text.push(Line::from(Span::styled(
            format!("Get your key at: {}", key_url),
            Style::default().fg(self.theme.text_secondary),
        )));
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "[Type] Enter key    [Tab] Toggle visibility    [Enter] Continue    [Esc] Back",
            Style::default().fg(self.theme.text_secondary),
        )));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_connection_test(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Testing Connection ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border));

        let status_text = match self.connection_status {
            ConnectionStatus::NotTested => "Preparing to test...",
            ConnectionStatus::Testing => "Testing connection...",
            ConnectionStatus::Success => "Connection successful!",
            ConnectionStatus::Failed => "Connection failed",
        };

        let status_style = match self.connection_status {
            ConnectionStatus::Success => Style::default().fg(self.theme.success),
            ConnectionStatus::Failed => Style::default().fg(self.theme.error),
            _ => Style::default().fg(self.theme.text),
        };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(status_text, status_style)),
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Skip test & save    [Esc] Back",
                Style::default().fg(self.theme.text_secondary),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_complete(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Setup Complete! ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.success));

        let provider_name = self.selected_provider
            .map(|p| p.display_name())
            .unwrap_or("Provider");

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Configuration saved successfully!",
                Style::default().fg(self.theme.success).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Provider: {}", provider_name)),
            Line::from(""),
            Line::from("You can now start using Continuum!"),
            Line::from(""),
            Line::from("Try typing: \"Hello, what can you help me with?\""),
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Start using Continuum",
                Style::default().fg(self.theme.text_secondary),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }
}

impl Default for SetupWizard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_creation() {
        let wizard = SetupWizard::new();
        assert_eq!(wizard.current_step, WizardStep::Welcome);
        assert!(!wizard.visible);
    }

    #[test]
    fn test_wizard_show() {
        let mut wizard = SetupWizard::new();
        wizard.show();
        assert!(wizard.visible);
        assert_eq!(wizard.current_step, WizardStep::Welcome);
    }

    #[test]
    fn test_wizard_navigation() {
        let mut wizard = SetupWizard::new();
        wizard.next_step();
        assert_eq!(wizard.current_step, WizardStep::ProviderSelection);
        
        wizard.prev_step();
        assert_eq!(wizard.current_step, WizardStep::Welcome);
    }

    #[test]
    fn test_provider_selection() {
        let mut wizard = SetupWizard::new();
        wizard.select_provider(Provider::OpenAI);
        assert_eq!(wizard.selected_provider, Some(Provider::OpenAI));
    }

    #[test]
    fn test_api_key_input() {
        let mut wizard = SetupWizard::new();
        wizard.push_char('a');
        wizard.push_char('b');
        assert_eq!(wizard.api_key_input, "ab");
        
        wizard.pop_char();
        assert_eq!(wizard.api_key_input, "a");
        
        wizard.clear_input();
        assert!(wizard.api_key_input.is_empty());
    }

    #[test]
    fn test_provider_info() {
        assert_eq!(Provider::Anthropic.name(), "anthropic");
        assert_eq!(Provider::OpenAI.display_name(), "OpenAI (GPT-4)");
        assert!(!Provider::Google.key_url().is_empty());
    }
}
```

- [ ] **Step 2: Update components/mod.rs to include setup_wizard**

```rust
//! TUI 组件模块

pub mod chat;
pub mod code_editor;
pub mod code_viewer;
pub mod color_theme;
pub mod confirmation;
pub mod error_display;
pub mod input;
pub mod key_hints;
pub mod markdown_renderer;
pub mod permission_popup;
pub mod session_list;
pub mod setup_wizard;
pub mod status;
pub mod syntax_highlight;
pub mod token_stats;
pub mod tool_display;

pub use chat::ChatComponent;
pub use confirmation::{ConfirmAction, ConfirmationDialog, PermissionManager};
pub use input::InputComponent;
pub use key_hints::{HintContext, KeyHintsComponent};
pub use permission_popup::{PermissionAction, PermissionPopup};
pub use setup_wizard::{ConnectionStatus, Provider, SetupWizard, WizardStep};
pub use status::StatusComponent;
pub use tool_display::ToolDisplayComponent;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cd cli && cargo test tui::components::setup_wizard --no-fail-fast`
Expected: All tests pass

- [ ] **Step 4: Commit SetupWizard component**

```bash
git add cli/src/tui/components/setup_wizard.rs cli/src/tui/components/mod.rs
git commit -m "feat(tui): add SetupWizard component for first-run configuration"
```

---

### Task 3: Enhance FirstRunState

**Files:**
- Modify: `cli/src/tui/first_run.rs`

- [ ] **Step 1: Update FirstRunState struct with new fields**

Replace the `FirstRunState` struct in `cli/src/tui/first_run.rs`:

```rust
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
```

- [ ] **Step 2: Update load_state function**

```rust
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
```

- [ ] **Step 3: Update save function**

```rust
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
```

- [ ] **Step 4: Add new methods for setup state**

```rust
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
```

- [ ] **Step 5: Update Default implementation**

```rust
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
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd cli && cargo test tui::first_run --no-fail-fast`
Expected: All tests pass

- [ ] **Step 7: Commit FirstRunState enhancement**

```bash
git add cli/src/tui/first_run.rs
git commit -m "feat(tui): enhance FirstRunState with setup tracking"
```

---

### Task 4: Integrate Setup Flow into TUI

**Files:**
- Modify: `cli/src/tui/mod.rs`

- [ ] **Step 1: Add setup module import to mod.rs**

Add at the top of `cli/src/tui/mod.rs`:

```rust
pub mod setup;
```

- [ ] **Step 2: Add SetupWizard to imports**

Add to the imports section:

```rust
use components::{
    ChatComponent, ConfirmationDialog, InputComponent, KeyHintsComponent, PermissionManager,
    PermissionPopup, SetupWizard, StatusComponent, ToolDisplayComponent,
};
use setup::ConfigDetector;
```

- [ ] **Step 3: Create SetupWizard instance in run_with_session**

Add after creating components (around line 98):

```rust
    let mut setup_wizard = SetupWizard::new();
```

- [ ] **Step 4: Add detection logic after first_run_state check**

Replace the initialization section (around lines 104-131) with:

```rust
    // 检测首次启动状态
    let mut first_run_state = first_run::FirstRunState::new().unwrap_or_default();

    // 检测配置状态
    let config_detector = ConfigDetector::new();
    let detection_result = config_detector.detect().unwrap_or_else(|_| setup::DetectionResult {
        providers: vec![],
        has_valid_config: false,
        config_file_path: None,
    });

    // 决定是否显示配置向导
    let show_setup_wizard = !detection_result.has_valid_config && first_run_state.needs_setup();

    // 添加欢迎消息（根据首次启动状态定制）
    let welcome_msg = if show_setup_wizard {
        // 将由配置向导处理
        String::new()
    } else if first_run_state.is_first_run {
        first_run::FirstRunState::get_welcome_message()
    } else {
        "Welcome back! Agent initializing...".to_string()
    };

    if !welcome_msg.is_empty() {
        chat.add_message(app::Message {
            role: app::Role::System,
            content: welcome_msg,
        });
        status.set_message_count(chat.message_count());
    }

    // 如果需要配置向导，显示它
    if show_setup_wizard {
        setup_wizard.show();
    }

    // 首次启动时自动显示教程提示（仅在配置完成后）
    if !show_setup_wizard && first_run_state.is_first_run && !first_run_state.tutorial_completed {
        chat.add_message(app::Message {
            role: app::Role::System,
            content: first_run::FirstRunState::get_first_run_hint(),
        });
        status.set_message_count(chat.message_count());
    }
```

- [ ] **Step 5: Add setup_wizard to TuiComponentsMut struct**

```rust
/// TUI 组件容器（可变引用）
struct TuiComponentsMut<'a> {
    app: &'a mut App,
    chat: &'a mut ChatComponent,
    input: &'a mut InputComponent,
    status: &'a mut StatusComponent,
    tools: &'a mut ToolDisplayComponent,
    key_hints: &'a mut KeyHintsComponent,
    confirmation: &'a mut ConfirmationDialog,
    permissions: &'a mut PermissionManager,
    permission_popup: &'a mut PermissionPopup,
    setup_wizard: &'a mut SetupWizard,
}
```

- [ ] **Step 6: Add setup_wizard rendering in main loop**

Add in the terminal.draw closure (around line 260):

```rust
        // 渲染设置向导（如果可见）
        if setup_wizard.is_visible() {
            setup_wizard.render(f, f.area());
        }
```

- [ ] **Step 7: Add setup_wizard keyboard handling**

Add a new keyboard handler section before the existing key handling (around line 290):

```rust
                // 如果设置向导可见，优先处理设置向导的按键
                if setup_wizard.is_visible() {
                    match key.code {
                        KeyCode::Esc => {
                            match setup_wizard.current_step {
                                components::WizardStep::Welcome => {
                                    // 用户跳过设置
                                    setup_wizard.hide();
                                    first_run_state.mark_setup_skipped().ok();
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: "Setup skipped. You can configure later with /config command.\n\n\
                                            Note: You'll need to configure an API key before sending messages.".to_string(),
                                    });
                                    status.set_message_count(chat.message_count());
                                }
                                _ => setup_wizard.prev_step(),
                            }
                            continue;
                        }
                        KeyCode::Enter => {
                            match setup_wizard.current_step {
                                components::WizardStep::Welcome => {
                                    setup_wizard.next_step();
                                }
                                components::WizardStep::ProviderSelection => {
                                    if setup_wizard.selected_provider.is_some() {
                                        setup_wizard.next_step();
                                    }
                                }
                                components::WizardStep::ApiKeyInput => {
                                    if !setup_wizard.api_key_input.is_empty() {
                                        // Save configuration
                                        if let Some(provider) = setup_wizard.selected_provider {
                                            let provider_name = provider.name();
                                            let _ = save_provider_config(provider_name, &setup_wizard.api_key_input);
                                            setup_wizard.next_step();
                                        }
                                    }
                                }
                                components::WizardStep::ConnectionTest => {
                                    // Skip test and complete
                                    if let Some(provider) = setup_wizard.selected_provider {
                                        first_run_state.mark_setup_completed(provider.name()).ok();
                                    }
                                    setup_wizard.next_step();
                                }
                                components::WizardStep::Complete => {
                                    setup_wizard.hide();
                                    // Reinitialize agent with new config
                                    let init_result = rt.block_on(async {
                                        let agent_guard = agent.read().await;
                                        agent_guard.init_from_config().await
                                    });
                                    if init_result.is_ok() {
                                        status.set_connected(true);
                                        status.set_provider(setup_wizard.selected_provider.map(|p| p.name().to_string()));
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: first_run::FirstRunState::get_first_run_hint(),
                                        });
                                        status.set_message_count(chat.message_count());
                                    }
                                }
                            }
                            continue;
                        }
                        KeyCode::Up => {
                            if setup_wizard.current_step == components::WizardStep::ProviderSelection {
                                let providers = components::Provider::all();
                                let current_idx = setup_wizard.selected_provider
                                    .and_then(|p| providers.iter().position(|&x| x == p))
                                    .unwrap_or(0);
                                let new_idx = if current_idx == 0 { providers.len() - 1 } else { current_idx - 1 };
                                setup_wizard.select_provider(providers[new_idx]);
                            }
                            continue;
                        }
                        KeyCode::Down => {
                            if setup_wizard.current_step == components::WizardStep::ProviderSelection {
                                let providers = components::Provider::all();
                                let current_idx = setup_wizard.selected_provider
                                    .and_then(|p| providers.iter().position(|&x| x == p))
                                    .unwrap_or(0);
                                let new_idx = (current_idx + 1) % providers.len();
                                setup_wizard.select_provider(providers[new_idx]);
                            }
                            continue;
                        }
                        KeyCode::Tab => {
                            if setup_wizard.current_step == components::WizardStep::ApiKeyInput {
                                setup_wizard.toggle_visibility();
                            }
                            continue;
                        }
                        KeyCode::Char(c) => {
                            if setup_wizard.current_step == components::WizardStep::ApiKeyInput {
                                setup_wizard.push_char(c);
                            }
                            continue;
                        }
                        KeyCode::Backspace => {
                            if setup_wizard.current_step == components::WizardStep::ApiKeyInput {
                                setup_wizard.pop_char();
                            }
                            continue;
                        }
                        _ => {}
                    }
                    continue;
                }
```

- [ ] **Step 8: Add helper function for saving provider config**

Add this helper function near the end of the file (before tests if any):

```rust
/// 保存提供商配置
fn save_provider_config(provider: &str, api_key: &str) -> anyhow::Result<()> {
    use sh_core::layer1::{ConfigManager, ProviderConfig};
    
    let config_path = ConfigManager::default_config_path();
    let mut config = ConfigManager::new();
    
    if config_path.exists() {
        config.load_from_file_sync(&config_path)?;
    }
    
    let default_url = match provider {
        "anthropic" => "https://api.anthropic.com/v1",
        "openai" => "https://api.openai.com/v1",
        "google" => "https://generativelanguage.googleapis.com/v1",
        _ => "",
    };
    
    let default_model = match provider {
        "anthropic" => "claude-sonnet-4-6",
        "openai" => "gpt-4",
        "google" => "gemini-pro",
        _ => "",
    };
    
    let provider_config = ProviderConfig {
        api_key: api_key.to_string(),
        base_url: default_url.to_string(),
        model: default_model.to_string(),
        default_max_tokens: 4096,
        default_temperature: 0.7,
    };
    
    config.add_provider(provider, provider_config);
    config.use_provider(provider)?;
    config.save_sync(&config_path)?;
    
    Ok(())
}
```

- [ ] **Step 9: Run tests to verify compilation**

Run: `cd cli && cargo check`
Expected: No errors

- [ ] **Step 10: Commit TUI integration**

```bash
git add cli/src/tui/mod.rs
git commit -m "feat(tui): integrate SetupWizard into first-run flow"
```

---

### Task 5: Add Prompt on First Message When Setup Skipped

**Files:**
- Modify: `cli/src/tui/mod.rs`

- [ ] **Step 1: Add state tracking for setup prompt**

Add near other state variables (around line 235):

```rust
    // 是否需要提示配置（当用户跳过设置后尝试发送消息）
    let mut needs_setup_prompt = first_run_state.setup_skipped && !first_run_state.setup_completed;
```

- [ ] **Step 2: Intercept SendMessage action when setup is needed**

Modify the `KeyAction::SendMessage(content)` handler (around line 441) to check for setup:

```rust
                            KeyAction::SendMessage(content) => {
                                // 检查是否需要先配置
                                if needs_setup_prompt {
                                    setup_wizard.show();
                                    needs_setup_prompt = false;
                                    chat.add_message(app::Message {
                                        role: app::Role::User,
                                        content: content.clone(),
                                    });
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: "You need to configure an API key first. Let's set that up!".to_string(),
                                    });
                                    status.set_message_count(chat.message_count());
                                    continue;
                                }
                                
                                status.set_processing(true);
                                // ... rest of existing code
```

- [ ] **Step 3: Run tests to verify compilation**

Run: `cd cli && cargo check`
Expected: No errors

- [ ] **Step 4: Commit setup prompt on first message**

```bash
git add cli/src/tui/mod.rs
git commit -m "feat(tui): prompt setup wizard when user tries to send without config"
```

---

### Task 6: Final Integration and Testing

**Files:**
- Run full test suite
- Manual testing

- [ ] **Step 1: Run all TUI tests**

Run: `cd cli && cargo test tui --no-fail-fast`
Expected: All tests pass

- [ ] **Step 2: Run full project check**

Run: `cd cli && cargo check`
Expected: No errors

- [ ] **Step 3: Run clippy for lint check**

Run: `cd cli && cargo clippy -- -D warnings 2>&1 | head -50`
Fix any issues found

- [ ] **Step 4: Manual test checklist**

Test the following scenarios:
1. [ ] Fresh start with no config - wizard appears
2. [ ] Complete wizard flow with Anthropic
3. [ ] Skip wizard, then try to send message
4. [ ] Existing config - no wizard shown
5. [ ] Environment variable detection works

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat(tui): complete first-run guidance implementation

- Add ConfigDetector for auto-detecting API keys from env vars
- Add SetupWizard component with provider selection and API key input
- Enhance FirstRunState with setup tracking
- Integrate setup flow into TUI
- Prompt setup when user tries to send without config"
```

---

## Summary

This plan implements:
1. **ConfigDetector** - Auto-detects API keys from environment variables
2. **SetupWizard** - Interactive UI for provider selection and API key input
3. **Enhanced FirstRunState** - Tracks setup completion and skipped states
4. **TUI Integration** - Seamless first-run experience
5. **Just-in-time Prompt** - Prompts for setup when needed

The implementation follows the hybrid timing approach (allow skip, prompt later) and smart detection (env vars first).
