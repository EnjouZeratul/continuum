# Continuum Python SDK

A production-grade agent framework with crash safety guarantees.

English | [简体中文](README_CN.md)

## Quick Start (3 steps)

```python
# Illustrative - requires CONTINUUM_API_KEY in environment
from continuum import Agent

agent = Agent()  # Auto-loads config from environment
result = agent.run("your task")
```

## Why Continuum?

| Feature | Continuum | Others |
|---------|-----------|--------|
| **Session Persistence** | Built-in checkpoint & recovery | Manual implementation |
| **Multi-Provider** | 13 providers, one API | Separate integrations |
| **Security** | PathValidator + AuditLogger | No built-in security |
| **Chinese LLM** | GLM/KIMI/DeepSeek native | Manual configuration |
| **Development Speed** | 3-line setup | Complex configuration |

### Key Advantages

1. **Crash Safety**: Sessions automatically saved and recoverable
2. **Provider Switching**: Change providers with one config line
3. **Built-in Tools**: 16+ tools ready to use (file, search, shell, LSP)
4. **Production Ready**: Security, audit logging, error recovery

## Security (Production-Ready)

Continuum provides built-in security for production deployment. Security checks
are **opt-in via the `workspace` argument** — once a workspace is configured,
path validation, permission checks, and audit logging are enforced for every
file operation. Without a workspace the tools log a one-time warning and run
without enforcement (transition-period default).

```python
from continuum_sdk.security import (
    PathValidator, AuditLogger, PermissionChecker, Permission,
)
from continuum_sdk.tools import read_file, write_file, edit_file, list_directory

# 1. Direct use of the security components
validator = PathValidator("/workspace")
result = validator.validate("./file.txt")
if result.is_valid:                       # use .is_valid (NOT truthy on the object)
    content = read_file(result.resolved_path, workspace="/workspace")

# 2. Recommended: pass `workspace=` to the tool and let it enforce the pipeline
content = read_file("./src/main.py", workspace="/workspace")
write_file("./out.txt", "data", workspace="/workspace")
edit_file("./config.py", "old", "new", workspace="/workspace")
list_directory("./src", workspace="/workspace")

# 3. Explicit components (custom audit log, strict checker)
audit = AuditLogger("audit.json")
security_config = {
    "validator": PathValidator("/workspace"),
    "checker": PermissionChecker(strict_mode=True),
    "auditor": audit,
}
write_file("./out.txt", "data", security_config=security_config)
```

**Pipeline (executed in order)**: `PathValidator.validate(path)` → if
`result.is_valid` is False the operation is denied and audited; otherwise
`PermissionChecker.check(path, ...)` runs; on success the operation executes
and `AuditLogger.log(..., AuditResult.SUCCESS)` records it.

### Shell Command Security

The `bash` tool / `BashTool` apply a **token-level command policy** — the
command is parsed with `shlex` and every sub-command (including those after
`|`, `&&`, `||`, `;`) is checked. This is not bypassable by leading whitespace,
absolute paths (`/usr/bin/sudo`), or pipelines (`ls | sudo cat`).

```python
from continuum_sdk.tools import BashTool

bash = BashTool(workspace="/workspace")   # cwd validated against workspace
bash.run("echo hello")                    # OK
bash.run("rm -rf build/", confirm=True)   # DANGEROUS — requires confirm=True
bash.run("sudo ls")                       # BLOCKED unconditionally
```

By default the subprocess environment is **minimised** to a safe whitelist
(`PATH`, `HOME`, `USER`, `TEMP`/`TMPDIR`, locale, plus Windows essentials).
Pass `inherit_env=True` on `BashTool` to inherit the full parent environment
(not recommended for untrusted commands).

### Security Features

| Feature | Description |
|---------|-------------|
| **PathValidator** | Path boundary + symlink escape detection (`result.is_valid` returns the verdict) |
| **AuditLogger** | JSON/CSV export, retention policy, success/failure/denied records |
| **PermissionChecker** | READ/WRITE/EXECUTE/DELETE/CREATE |
| **Environment Whitelist** | Minimised subprocess env by default |
| **Token-Level Shell Policy** | Pipeline-aware blocklist; unbypassable by prefix tricks |
| **Confirmation for Dangerous Commands** | `rm`, `git push`, `chmod` etc. require `confirm=True` |

---

## Supported Providers

Continuum supports 17 LLM providers with unified API:

### International Providers

| Provider | Models | Best For |
|----------|--------|----------|
| **Anthropic** | claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5 | Code generation |
| **OpenAI** | gpt-5.5, gpt-5.4, gpt-4.1, o3-mini, o1 | General purpose |
| **Google/Gemini** | gemini-3.1-pro-preview, gemini-3.0-pro | Multimodal |
| **Cohere** | command, command-light | Enterprise |
| **HuggingFace** | (any model) | Custom models |
| **Together AI** | Llama-3, Mixtral, Mistral | Open models |
| **Groq** | llama-3.3-70b, mixtral | Low latency |
| **Grok (xAI)** | grok-4-heavy, grok-4 | Real-time info |
| **Azure OpenAI** | gpt-4o, gpt-4o-mini | Enterprise cloud |
| **AWS Bedrock** | claude-sonnet-4-6, llama3 | Cloud LLM |
| **Ollama** | llama3, mistral, codellama | Local models |

### Chinese Providers

| Provider | Models | Best For |
|----------|--------|----------|
| **GLM (智谱AI)** | glm-5.1, glm-5 | 中文对话 |
| **KIMI (月之暗面)** | kimi-k2.6, kimi-k2.5 | 长文本处理 |
| **DeepSeek** | deepseek-v4-pro, deepseek-v3 | 成本优化 |
| **Moonshot** | kimi-k2.6, moonshot-v1-128k | 长上下文 |
| **Qwen (阿里巴巴)** | qwen3.7-max, qwen3.6-plus | 企业应用 |

### Provider Switching

```python
# Illustrative - requires API key
from continuum_sdk import Config

config = Config.from_env()
config.use("anthropic")  # Claude
config.use("deepseek")   # Switch to DeepSeek
config.use("glm")        # Switch to GLM
```

---

## Installation

```bash
pip install continuum-agent-sdk
```

## Configuration

### Environment Variables

Priority: `CONTINUUM_*` > `ANTHROPIC_*` > `OPENAI_*`

```bash
export CONTINUUM_API_KEY=your_api_key
export CONTINUUM_PROVIDER=anthropic  # or openai, google
export CONTINUUM_MODEL=claude-sonnet-4-6
```

### Config File

Create `~/.continuum/config.toml`:

```toml
[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
base_url = "https://api.anthropic.com/v1"
model = "claude-sonnet-4-6"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
model = "gpt-4"

[settings]
session_auto_save = true
checkpoint_enabled = true
audit_enabled = true
```

## Features

- **Agent**: One-line agent creation with automatic configuration
- **Session**: Conversation history management with checkpoint support
- **Tools**: Built-in and custom tool registration
- **Memory**: Multi-layer memory system (episodic, semantic, procedural)
- **VectorStore**: In-memory and persistent vector storage with FTS5 search
- **Config**: Multi-provider configuration with environment variable support

### VectorStore Quick Example

```python
# Illustrative - requires embedding vectors
from continuum_sdk.rag import InMemoryVectorStore

# Create in-memory store
store = InMemoryVectorStore()

# Upsert documents with pre-computed embeddings
store.upsert("doc1", [0.1, 0.2, ...], {"text": "Hello world"})
store.upsert("doc2", [0.3, 0.4, ...], {"text": "Python is great"})

# Search (returns matching document IDs and scores)
results = store.search([0.15, 0.25, ...], top_k=2)
```

---

## API Reference

### Agent

The main entry point for running AI agents.

```python
# Illustrative - requires API key
from continuum import Agent

# Create agent with defaults (auto-config from env)
agent = Agent()

# Create agent with explicit settings
agent = Agent(
    name="my-agent",
    model="claude-sonnet-4-6",
    provider="anthropic"
)

# One-shot task execution
result = agent.run("Analyze this codebase structure")

# Session creation
session = agent.create_session("analysis-session")
```

#### Agent Methods

| Method | Description | Returns |
|--------|-------------|---------|
| `run(task)` | Execute a single task | `str` - Task result |
| `arun(task)` | Execute a single task asynchronously | `str` - Task result |
| `create_session(id=None)` | Create new session | `Session` |
| `register_tool(name, func, description='', parameters=None)` | Register a custom tool | `None` |

### Session

Manages conversation history and state.

```python
from pathlib import Path
from continuum import Session

# Create session
session = Session()
session.add_user_message("Hello")
session.add_assistant_message("Hi! How can I help?")

# Inspect messages
messages = session.get_messages()

# Save and load
path = Path("session.json")
session.save(path)
session = Session.load(path)
```

#### Session Methods

| Method | Description | Returns |
|--------|-------------|---------|
| `add_user_message(msg)` | Add user message | `Message` |
| `add_assistant_message(msg)` | Add assistant message | `Message` |
| `add_system_message(msg)` | Add system message | `Message` |
| `get_messages(limit=None)` | Return conversation messages | `list[Message]` |
| `get_last_message()` | Return the latest message | `Message \| None` |
| `clear_messages()` | Clear conversation messages | `None` |
| `save(path)` | Persist to a JSON file | `Path` |
| `load(path)` | Load from a JSON file | `Session` (classmethod) |
| `to_dict()` | Serialize session state to a dictionary | `dict` |
| `from_dict(data)` | Restore session state from a dictionary | `Session` (classmethod) |

### Config

Multi-provider configuration management.

```python
# Illustrative - requires API key
from continuum_sdk import Config

# Load from environment
config = Config.from_env()

# Load from file
config = Config.from_file("~/.continuum/config.toml")

# Switch provider
config.use("openai")
config.use("anthropic")

# Get current settings
print(config.model)      # "claude-sonnet-4-6"
print(config.provider)  # "anthropic"
```

---

## Built-in Tools

Continuum provides 16+ built-in tools organized by category:

### File Operations

| Tool | Description |
|------|-------------|
| `read_file` | Read file content with pagination |
| `write_file` | Write content to file |
| `edit_file` | Find and replace in file |
| `list_directory` | List directory contents |

### Search

| Tool | Description |
|------|-------------|
| `grep` | Regex search in files |
| `glob` | Pattern-based file search |

### Shell

| Tool | Description |
|------|-------------|
| `bash` | Execute shell commands |

### Code Analysis (LSP)

| Tool | Description |
|------|-------------|
| `go_to_definition` | Jump to symbol definition |
| `find_references` | Find all references |
| `hover` | Get type information |

---

## Custom Tools

### Method 1: Decorator (Recommended)

```python
from continuum_sdk.tools import tool

@tool(name="weather", description="Get weather info")
async def get_weather(city: str, unit: str = "celsius") -> str:
    """Get weather for a city"""
    # Implementation
    return f"Weather in {city}: 22°{unit[0].upper()}"

# Auto-registered, use immediately
```

### Method 2: Class-based

```python
from continuum_sdk.tools import CustomTool

class CalculatorTool(CustomTool):
    @property
    def name(self) -> str:
        return "calculator"

    @property
    def description(self) -> str:
        return "Perform math operations"

    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "operation": {"type": "string"},
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["operation", "a", "b"]
        }

    async def execute(self, **kwargs) -> str:
        # Implementation
        return result
```

### Tool Configuration

```python
@tool(
    name="dangerous_op",
    description="A dangerous operation",
    is_dangerous=True,
    requires_confirmation=True
)
async def dangerous_operation(path: str) -> str:
    # Will prompt user before execution
    return f"Deleted {path}"
```

---

## MCP Integration

Connect to MCP (Model Context Protocol) servers for extended capabilities.

```python
from continuum_sdk.tools import MCPToolRegistry, create_mcp_registry

# Quick setup with predefined servers
registry = create_mcp_registry(
    ["filesystem", "github"],
    root_path="/path/to/project"
)

# Manual configuration
registry = MCPToolRegistry()
registry.connect_stdio(
    "filesystem",
    command="uvx",
    args=["mcp-server-filesystem", "--root", "/project"]
)
registry.connect_sse(
    "remote",
    url="http://localhost:8000/sse"
)

# Use MCP tools
for tool in registry.get_tools():
    print(f"{tool.name}: {tool.description}")

# Execute
result = await registry.execute("filesystem/read_file", path="README.md")

# Cleanup
registry.close()
```

### Predefined MCP Servers

| Server | Package | Description |
|--------|---------|-------------|
| `filesystem` | `mcp-server-filesystem` | File operations |
| `github` | `mcp-server-github` | GitHub API |
| `puppeteer` | `mcp-server-puppeteer` | Browser automation |
| `slack` | `mcp-server-slack` | Slack messaging |
| `postgres` | `mcp-server-postgres` | PostgreSQL |
| `memory` | `mcp-server-memory` | Persistent memory |

---

## Advanced Usage

### Intelligent Agent with Planning

```python
# Illustrative - requires API key
from continuum_sdk.agent import IntelligentAgent, AgentMode

agent = IntelligentAgent(
    model="claude-sonnet-4-6",
    mode=AgentMode.AUTONOMOUS  # Auto-execute without confirmation
)

# Plan task
plan = await agent.plan("Refactor auth.py to use OAuth2")
print(plan.to_dict())

# Execute plan
result = await agent.execute(plan)
print(f"Completed {result.completed_steps}/{result.total_steps}")

# Track progress
print(agent.get_progress_text())  # "[3/5] 60% in 10s ETA: 6s"
```

### Multi-Layer Memory

```python
from continuum_sdk.api import MemorySystem

memory = MemorySystem()

# Working memory: Current context
memory.store("working", "User prefers dark mode")

# Session memory: Session-level facts
memory.store("session", "Project uses Python 3.11")

# Project memory: Project knowledge
memory.store("project", "To run tests: pytest tests/")

# Query
results = memory.query("How do I run tests?")
```

### Workflow with DAG

```python
from continuum_sdk.workflow import DAG, Node

dag = DAG("refactor_workflow")

# Define nodes with dependencies
dag.add(Node(
    id="analyze",
    func=lambda: analyze_codebase()
))
dag.add(Node(
    id="refactor",
    func=lambda: refactor_code()
).depends_on("analyze"))
dag.add(Node(
    id="test",
    func=lambda: run_tests()
).depends_on("refactor"))

# Execute
result = await dag.execute()
```

---

## Examples

See the `examples/` directory for complete examples:

| Example | Description |
|---------|-------------|
| `basic/hello_world.py` | Minimal agent example |
| `basic/hello_agent.py` | Agent with session |
| `basic/session_example.py` | Session management |
| `advanced/custom_tools.py` | Custom tool creation |
| `advanced/workflow.py` | Workflow orchestration |

Run examples:

```bash
cd python
python -m continuum_sdk.examples.basic.hello_agent
```

---

## Error Handling

```python
from continuum_sdk.errors import (
    ContinuumError,
    ConfigError,
    ToolExecutionError,
    LLMError
)

try:
    result = agent.run("complex task")
except ConfigError as e:
    print(f"Configuration error: {e}")
except ToolExecutionError as e:
    print(f"Tool failed: {e.tool_name} - {e.message}")
except LLMError as e:
    print(f"LLM API error: {e}")
except ContinuumError as e:
    print(f"General error: {e}")
```

---

## Logging

```python
import logging

# Enable debug logging
logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger("continuum_sdk")
logger.setLevel(logging.DEBUG)
```

---

## License

MIT License
