# Continuum CLI

A terminal agent product - command-line interface and TUI for the Continuum agent runtime.

## Installation

```bash
# Install from crates.io
cargo install continuum

# Or build from source
git clone https://github.com/EnjouZeratul/continuum
cd continuum/cli
cargo install --path .
```

### Prerequisites

- Rust 1.70+ (for building from source)
- API key for at least one provider

## Quick Start

```bash
# Launch interactive TUI (default behavior)
continuum

# Run a one-shot task
continuum run "Explain this codebase"

# Initialize configuration
continuum config init
```

## Commands Reference

### Core Commands

#### `continuum run`

Execute an agent task.

```bash
continuum run "your task description"
continuum run "fix the bug" --session abc123
continuum run "analyze logs" --budget 5.00 --non-interactive
```

| Flag | Description |
|------|-------------|
| `--session <id>` | Resume existing session |
| `--budget <amount>` | Set cost budget limit |
| `--non-interactive` | Disable confirmations |

#### `continuum tui`

Launch the terminal UI (default when no command specified).

```bash
continuum tui
continuum tui --session abc123  # Resume session
```

#### `continuum config`

Manage configuration.

```bash
continuum config init                    # Create default config
continuum config show                    # Display all settings
continuum config show provider.anthropic # Show specific provider
continuum config set settings.checkpoint_enabled true
continuum config get active_provider
continuum config keys                    # List all config keys
continuum config list                    # List configured providers
```

**Provider management:**

```bash
continuum config add-provider anthropic --key sk-ant-xxx
continuum config add-provider openai --key sk-xxx --url https://api.openai.com/v1
continuum config add-provider ollama --url http://localhost:11434
continuum config use anthropic           # Switch active provider
```

| Flag | Description |
|------|-------------|
| `--key <api_key>` | API key for provider |
| `--url <base_url>` | Custom base URL |
| `--model <model>` | Default model |

#### `continuum session`

Manage sessions.

```bash
continuum session list          # List active sessions
continuum session list --all    # Include completed sessions
continuum session show abc123   # View session details
continuum session resume abc123 # Resume a session
continuum session delete abc123 # Delete (prompts for confirmation)
continuum session delete abc123 --force  # Delete without prompt
```

#### `continuum checkpoint`

Manage checkpoints.

```bash
continuum checkpoint list                    # List checkpoints
continuum checkpoint list --session abc123   # For specific session
continuum checkpoint restore <checkpoint_id> # Restore state
continuum checkpoint delete <checkpoint_id>  # Delete checkpoint
```

#### `continuum tools`

List available tools.

```bash
continuum tools
continuum tools --filter file
continuum tools --verbose
```

#### `continuum dashboard`

Observability web UI.

```bash
continuum dashboard start                    # Start on localhost:8080
continuum dashboard start --port 3000 --host 0.0.0.0
continuum dashboard status                   # Check if running
```

### Toolchain Commands

Direct file and shell operations.

#### `continuum bash`

Execute shell commands.

```bash
continuum bash "ls -la"
continuum bash "npm test" --cwd ./project --timeout 300
continuum bash "make build" --capture-stderr
```

| Flag | Default | Description |
|------|---------|-------------|
| `--cwd <dir>` | Current dir | Working directory |
| `--timeout <sec>` | 120 | Timeout in seconds |
| `--capture-stderr` | false | Include stderr in output |

#### `continuum read`

Read file contents.

```bash
continuum read src/main.rs
continuum read README.md --offset 10 --limit 50
continuum read Cargo.toml --line-numbers
```

| Flag | Description |
|------|-------------|
| `--offset <n>` | Start from line n |
| `--limit <n>` | Read n lines |
| `--line-numbers` | Show line numbers |

#### `continuum write`

Write to files.

```bash
continuum write output.txt "Hello world"
continuum write log.txt "New entry" --append
continuum write config.json '{"key": "value"}' --backup
echo "content" | continuum write file.txt -
```

| Flag | Description |
|------|-------------|
| `--append` | Append to file |
| `--backup` | Create backup before writing |

#### `continuum edit`

Precise string replacement in files.

```bash
continuum edit src/main.rs --old "fn main()" --new "fn main() -> Result<()>"
continuum edit config.json --old "debug: false" --new "debug: true" --replace-all
```

| Flag | Description |
|------|-------------|
| `--old <text>` | Text to replace (required) |
| `--new <text>` | Replacement text (required) |
| `--replace-all` | Replace all occurrences |

#### `continuum grep`

Search file contents with regex.

```bash
continuum grep "fn main"
continuum grep "TODO" --path src/ --glob "*.rs"
continuum grep "error" --ignore-case --context 3
```

| Flag | Default | Description |
|------|---------|-------------|
| `--path <dir>` | . | Search directory |
| `--glob <pattern>` | * | File filter |
| `--ignore-case` | false | Case insensitive |
| `--context <n>` | 0 | Show n lines around match |

#### `continuum glob`

Find files by pattern.

```bash
continuum glob "**/*.rs"
continuum glob "*.toml" --path ./config
```

#### `continuum lsp`

LSP code intelligence.

```bash
continuum lsp definition src/main.rs 10 5
continuum lsp references src/lib.rs 25 1
continuum lsp hover src/utils.rs 100 15
continuum lsp symbols src/module.rs
```

### Git Integration

```bash
# Status
continuum git status
continuum git status --short

# Diff
continuum git diff
continuum git diff --staged
continuum git diff src/main.rs

# Commit
continuum git commit --message "fix: bug in parser"
continuum git commit --amend
continuum git commit --add-all

# Add
continuum git add src/main.rs src/lib.rs

# Branch
continuum git branch list
continuum git branch list --all
continuum git branch create feature-x --switch
continuum git branch switch main
continuum git branch delete old-feature --force

# Pull Request
continuum git pr create --title "Add feature" --body "Description"
continuum git pr create --draft --base develop
continuum git pr list --state open
```

## TUI Mode

When launched without arguments, Continuum starts in TUI mode.

### UI Components

- **Chat Panel** - Message history with syntax highlighting
- **Input Box** - Multiline input with history navigation
- **Status Bar** - Provider, model, session ID, token count
- **Tools Panel** - Real-time tool execution display (toggle with `Ctrl+T`)
- **Key Hints Bar** - Context-sensitive shortcuts (expand with `Ctrl+?`)

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+C` / `Ctrl+D` | Exit |
| `Ctrl+L` | Clear screen |
| `Ctrl+S` | Save session |
| `Ctrl+N` | New session |
| `Ctrl+T` | Toggle tools panel |
| `Ctrl+H` | Show help |
| `Ctrl+X` | Cancel running tool |
| `Ctrl+W` | Delete previous word |
| `Ctrl+A` | Move to line start |
| `Ctrl+E` | Move to line end |
| `Alt+B` | Move word backward |
| `Alt+F` | Move word forward |
| `Alt+Enter` / `Shift+Enter` | Insert newline |
| `Tab` | Command completion |
| `Up` / `Down` | Scroll messages or input history |
| `Page Up` / `Page Down` | Scroll by page |
| `F1` / `Ctrl+?` | Expand key hints |

### Slash Commands

Commands available in TUI input:

| Command | Description | Risk |
|---------|-------------|------|
| `/help [command]` | Show help | Low |
| `/clear` | Clear chat history | Medium |
| `/save [name]` | Save current session | Low |
| `/new` | Start new session | Medium |
| `/exit` | Exit application | Medium |
| `/config [key] [value]` | View/modify config | Low |
| `/model [name]` | Switch or show model | Low |
| `/provider [name]` | Switch or show provider | Low |
| `/tools` | List available tools | Low |
| `/bash <cmd>` | Execute shell command | High |
| `/read <file>` | Read file | Low |
| `/write <file> <content>` | Write file | High |
| `/edit <file> --old --new` | Edit file | High |
| `/grep <pattern>` | Search content | Low |
| `/glob <pattern>` | Find files | Low |
| `/git <subcommand>` | Git operations | High |
| `/debug` | Toggle debug mode | Low |
| `/tokens` | Show token usage | Low |
| `/history [count]` | Show command history | Low |
| `/undo` | Undo last operation | Medium |
| `/checkpoint [msg]` | Create checkpoint | Low |
| `/tutorial [step]` | Interactive tutorial | Low |

High-risk commands require confirmation before execution.

### Setup Wizard

On first run without configuration, Continuum launches a setup wizard:

1. **Welcome** - Introduction screen
2. **Provider Selection** - Choose from Anthropic, OpenAI, Gemini
3. **API Key Input** - Secure entry with visibility toggle (Tab)
4. **Connection Test** - Optional verification
5. **Complete** - Ready to use

Skip with `Esc` and configure manually via environment variables or `config init`.

### Interactive Tutorial

```bash
/tutorial          # Start or show overview
/tutorial 1        # Jump to step 1-5
```

## Supported Providers

| Provider | CLI Support | Default Model | API Format |
|----------|-------------|---------------|------------|
| **Anthropic** | Yes | claude-sonnet-4-6 | Anthropic |
| **OpenAI** | Yes | gpt-5.5 | OpenAI |
| **Gemini** | Yes | gemini-3.0-pro | Google |
| **Azure OpenAI** | Yes | gpt-4o | OpenAI |
| **AWS Bedrock** | Yes | claude-sonnet-4-6 | OpenAI |
| **Ollama** | Yes | llama3 | OpenAI |

**SDK-only providers** (Python SDK supported, CLI routing not yet available):
- Cohere, HuggingFace, Together, Groq, DeepSeek, Moonshot, GLM, Kimi, Qwen, Grok

### Environment Variables

```bash
# CLI-supported providers
ANTHROPIC_API_KEY=sk-ant-xxx
OPENAI_API_KEY=sk-xxx
GOOGLE_API_KEY=xxx          # Also used for Gemini
GEMINI_API_KEY=xxx          # Alias for GOOGLE_API_KEY
AZURE_OPENAI_API_KEY=xxx
AWS_ACCESS_KEY_ID=xxx       # For Bedrock

# Ollama uses no API key (local)
```

## Configuration

### Config File Location

```
~/.config/continuum/config.toml
```

### Config File Structure

```toml
active_provider = "anthropic"

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
base_url = "https://api.anthropic.com"
model = "claude-sonnet-4-6"
default_max_tokens = 4096
default_temperature = 1.0

[providers.openai]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
model = "gpt-5.5"
default_max_tokens = 4096
default_temperature = 1.0

[settings]
session_auto_save = true
session_max_history = 100
checkpoint_enabled = true
checkpoint_interval_sec = 300
audit_enabled = false
mcp_enabled = false
```

### Settings Reference

| Setting | Default | Description |
|---------|---------|-------------|
| `session_auto_save` | true | Auto-save sessions |
| `session_max_history` | 100 | Max sessions to keep |
| `checkpoint_enabled` | true | Enable auto-checkpoints |
| `checkpoint_interval_sec` | 300 | Checkpoint interval |
| `audit_enabled` | false | Enable audit logging |
| `mcp_enabled` | false | Enable MCP integration |

## Session and Checkpoint

### Sessions

Sessions persist conversation history and agent state.

**Auto-save**: When `session_auto_save` is enabled, sessions save automatically.

**Manual save**:
- TUI: `Ctrl+S`
- CLI: `continuum session save` (via TUI `/save`)

**Storage**: `~/.local/share/continuum/sessions/`

**Resume**:
```bash
continuum session resume <session_id>
continuum tui --session <session_id>
continuum run "continue task" --session <session_id>
```

### Checkpoints

Checkpoints capture agent state for rollback.

```bash
# List checkpoints
continuum checkpoint list

# Create checkpoint (in TUI)
/checkpoint "before risky operation"

# Restore
continuum checkpoint restore <checkpoint_id>
```

## Experimental Features

### Dashboard

Web-based observability interface for monitoring agent sessions.

```bash
continuum dashboard start --port 8080
```

### MCP Integration

Model Context Protocol for external tool integration.

Enable in config:
```toml
[settings]
mcp_enabled = true
```

## Troubleshooting

### No providers configured

```
Error: No providers configured. Run 'continuum config init' or set ANTHROPIC_API_KEY.
```

Solution: Run `continuum config init` or set an environment variable.

### Provider not supported in CLI

```
Error: Provider 'deepseek' is not supported in CLI. SDK only.
```

Solution: Use a CLI-supported provider, or use the Python SDK for deepseek.

### Session not found

```
Error: Session 'abc123' not found.
```

Solution: Run `continuum session list` to see available sessions.

## Related

- [Python SDK Documentation](../python/README.md) - For programmatic access
- [Architecture](../docs/ARCHITECTURE_V4.md) - Internal design

## License

MIT License - see [LICENSE](../LICENSE) for details.

## Contact

Email: 1281676337@qq.com

## Related
