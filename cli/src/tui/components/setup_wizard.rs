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
                Style::default()
                    .fg(self.theme.highlight)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Continuum helps you with coding, file operations,"),
            Line::from("and system tasks using AI assistance."),
            Line::from(""),
            Line::from(Span::styled(
                "To get started, you'll need to configure an API key.",
                Style::default().fg(self.theme.comment),
            )),
            Line::from(""),
            Line::from("Supported providers:"),
            Line::from("  • Anthropic (Claude)"),
            Line::from("  • OpenAI (GPT-4)"),
            Line::from("  • Google (Gemini)"),
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Continue    [Esc] Skip setup",
                Style::default().fg(self.theme.comment),
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
            let marker = if selected { "-> " } else { "   " };
            let style = if selected {
                Style::default()
                    .fg(self.theme.highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.foreground)
            };
            text.push(Line::from(Span::styled(
                format!("{}{}", marker, provider.display_name()),
                style,
            )));
        }

        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "[Up/Down] Select    [Enter] Confirm    [Esc] Back",
            Style::default().fg(self.theme.comment),
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

        let provider_name = self
            .selected_provider
            .map(|p| p.display_name())
            .unwrap_or("Provider");

        let key_display = if self.api_key_visible {
            self.api_key_input.clone()
        } else {
            "*".repeat(self.api_key_input.len())
        };

        let key_url = self.selected_provider.map(|p| p.key_url()).unwrap_or("");

        let mut text = vec![
            Line::from(""),
            Line::from(format!("Provider: {}", provider_name)),
            Line::from(""),
            Line::from("API Key:"),
            Line::from(Span::styled(
                if key_display.is_empty() {
                    "..."
                } else {
                    &key_display
                },
                Style::default().fg(self.theme.highlight),
            )),
            Line::from(""),
        ];

        if let Some(error) = &self.error_message {
            text.push(Line::from(Span::styled(
                format!("Error: {}", error),
                Style::default().fg(self.theme.error_message),
            )));
            text.push(Line::from(""));
        }

        text.push(Line::from(Span::styled(
            format!("Get your key at: {}", key_url),
            Style::default().fg(self.theme.comment),
        )));
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "[Type] Enter key    [Tab] Toggle visibility    [Enter] Continue    [Esc] Back",
            Style::default().fg(self.theme.comment),
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
            ConnectionStatus::Success => Style::default().fg(self.theme.success_message),
            ConnectionStatus::Failed => Style::default().fg(self.theme.error_message),
            _ => Style::default().fg(self.theme.foreground),
        };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(status_text, status_style)),
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Skip test & save    [Esc] Back",
                Style::default().fg(self.theme.comment),
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
            .border_style(Style::default().fg(self.theme.success_message));

        let provider_name = self
            .selected_provider
            .map(|p| p.display_name())
            .unwrap_or("Provider");

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Configuration saved successfully!",
                Style::default()
                    .fg(self.theme.success_message)
                    .add_modifier(Modifier::BOLD),
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
                Style::default().fg(self.theme.comment),
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
