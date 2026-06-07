# First-Run User Guidance Design

**Date:** 2026-05-30
**Task:** #177 - P0: 首次使用引导
**Goal:** Improve first-use experience from 2/10 to 7/10

## Overview

Implement an interactive first-run experience that automatically detects existing API configurations, provides a setup wizard for new users, and allows graceful exploration without configuration.

## Design Decisions

### API Key Configuration Timing: Hybrid Mode
- Check configuration on startup
- Show friendly setup wizard if configuration missing
- Allow users to skip and explore
- Prompt again on first message send attempt

### Multi-Provider Support: Smart Detection
- Auto-detect environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`)
- Priority: Environment Variables > Config File > Interactive Input
- Support: Anthropic, OpenAI, Google Gemini, Local models

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    First Run Flow                           │
├─────────────────────────────────────────────────────────────┤
│  1. Check Environment Variables                            │
│     └─> ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_API_KEY  │
│                                                             │
│  2. Check Config File                                       │
│     └─> ~/.config/continuum/config.yaml                    │
│                                                             │
│  3. If Config Found:                                         │
│     └─> Validate and initialize agent                      │
│                                                             │
│  4. If No Config Found:                                      │
│     └─> Show Setup Wizard                                   │
│         ├─> Welcome Screen                                  │
│         ├─> Provider Selection (Anthropic/OpenAI/Gemini)   │
│         ├─> API Key Input (masked)                          │
│         ├─> Connection Test (optional)                      │
│         └─> Save Configuration                              │
│                                                             │
│  5. Allow Skip (explore mode)                               │
│     └─> Show limited functionality notice                   │
│     └─> Prompt again on first message send                 │
│                                                             │
│  6. Show Tutorial Prompt                                    │
│     └─> Interactive Y/N with quick start guide             │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. ConfigDetector (New)
**File:** `cli/src/tui/setup/config_detector.rs`

Responsibilities:
- Scan environment variables for API keys
- Validate config file existence and format
- Auto-configure from detected keys
- Return detection result with provider info

```rust
pub struct ConfigDetector {
    detected_providers: Vec<DetectedProvider>,
    config_file_exists: bool,
}

pub struct DetectedProvider {
    name: String,
    source: DetectionSource, // EnvVar | ConfigFile
    api_key_set: bool,
}

pub enum DetectionSource {
    EnvVar(String),
    ConfigFile(PathBuf),
}
```

### 2. SetupWizard (New)
**File:** `cli/src/tui/components/setup_wizard.rs`

Responsibilities:
- Display multi-step configuration form
- Handle provider selection
- Collect API key input (masked)
- Test connection (optional)
- Save configuration

```rust
pub struct SetupWizard {
    current_step: WizardStep,
    selected_provider: Option<String>,
    api_key_input: String,
    connection_status: Option<ConnectionStatus>,
}

pub enum WizardStep {
    Welcome,
    ProviderSelection,
    ApiKeyInput,
    ConnectionTest,
    Complete,
}
```

### 3. Enhanced FirstRunState (Modify)
**File:** `cli/src/tui/first_run.rs`

Add fields:
```rust
pub struct FirstRunState {
    pub is_first_run: bool,
    pub tutorial_completed: bool,
    pub welcome_shown: bool,
    pub setup_completed: bool,      // NEW
    pub setup_skipped: bool,        // NEW
    pub detected_provider: Option<String>, // NEW
    config_dir: PathBuf,
}
```

### 4. Setup Wizard UI Component
**File:** `cli/src/tui/components/setup_wizard.rs`

Render functions for:
- Welcome message with product intro
- Provider selection cards (Anthropic/OpenAI/Gemini)
- API key input field (masked)
- Connection test spinner/result
- Success confirmation

## User Flow Details

### Flow 1: Environment Variable Detected
```
Start TUI
  │
  ▼
ConfigDetector::detect()
  │
  ├─> Found ANTHROPIC_API_KEY
  │     └─> Auto-configure Anthropic provider
  │
  ▼
Show Welcome (configured mode)
  │
  ▼
Show Tutorial Prompt
```

### Flow 2: No Configuration - User Completes Setup
```
Start TUI
  │
  ▼
ConfigDetector::detect() -> None
  │
  ▼
Show SetupWizard (Step 1: Welcome)
  │
  ▼
Provider Selection (Step 2)
  │
  ├─> Anthropic
  ├─> OpenAI
  └─> Google Gemini
  │
  ▼
API Key Input (Step 3)
  │
  ▼
Connection Test (Step 4) [Optional]
  │
  ▼
Save Config (Step 5)
  │
  ▼
Show Welcome
  │
  ▼
Show Tutorial Prompt
```

### Flow 3: No Configuration - User Skips
```
Start TUI
  │
  ▼
ConfigDetector::detect() -> None
  │
  ▼
Show SetupWizard (Welcome)
  │
  ▼
User presses "Skip" / Esc
  │
  ▼
Mark setup_skipped = true
  │
  ▼
Show Limited Welcome
  "You can explore the interface, but need to configure API key to chat."
  │
  ▼
User tries to send message
  │
  ▼
Show Setup Prompt again
```

## UI Design

### Welcome Screen
```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│     Welcome to Continuum v1.0.0                             │
│     Your AI-powered terminal assistant                     │
│                                                             │
│     To get started, you'll need to configure an API key.    │
│                                                             │
│     [Configure Now]  [Skip for now]                         │
│                                                             │
│     Supported providers:                                    │
│       • Anthropic (Claude)                                  │
│       • OpenAI (GPT-4)                                      │
│       • Google (Gemini)                                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Provider Selection
```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│     Select your AI provider                                 │
│                                                             │
│     ┌─────────────────┐  ┌─────────────────┐               │
│     │   Anthropic     │  │     OpenAI      │               │
│     │   Claude        │  │   GPT-4         │               │
│     │   [Selected]    │  │                 │               │
│     └─────────────────┘  └─────────────────┘               │
│                                                             │
│     ┌─────────────────┐                                    │
│     │   Google        │                                    │
│     │   Gemini        │                                    │
│     └─────────────────┘                                    │
│                                                             │
│     [Enter to continue]                                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### API Key Input
```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│     Enter your Anthropic API key                           │
│                                                             │
│     API Key: sk-ant-****-****-****-****                   │
│                                                             │
│     [Toggle visibility] [Test connection]                  │
│                                                             │
│     Get your API key at: console.anthropic.com             │
│                                                             │
│     [Back]  [Save & Continue]                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Error Handling

### Connection Test Failure
- Show friendly error message
- Offer retry or skip test
- Allow saving anyway (key might be valid, test might fail for other reasons)

### Invalid API Key Format
- Validate format before saving
- Show format requirements
- Offer example format

### Network Error
- Detect network issues separately from auth errors
- Suggest checking internet connection
- Allow saving configuration anyway

## Testing Strategy

### Unit Tests
- ConfigDetector: Environment variable detection
- ConfigDetector: Config file parsing
- SetupWizard: Step transitions
- FirstRunState: State persistence

### Integration Tests
- Full setup flow from empty state
- Skip and retry flow
- Environment variable auto-configuration

### Manual Testing Checklist
- [ ] Fresh install experience
- [ ] Skip setup, then configure later
- [ ] Environment variable detection
- [ ] Each provider configuration
- [ ] Connection test success/failure
- [ ] Tutorial prompt after setup

## Implementation Priority

1. **ConfigDetector** - Core detection logic
2. **SetupWizard Component** - UI for configuration
3. **FirstRunState Enhancement** - State management
4. **TUI Integration** - Wire into mod.rs flow
5. **Error Handling** - Friendly error messages
6. **Testing** - Unit and integration tests

## Success Metrics

- First-time setup completion rate > 80%
- Time to first successful message < 2 minutes
- User satisfaction (first use) improved from 2/10 to 7/10
- Support tickets related to setup reduced by 50%
