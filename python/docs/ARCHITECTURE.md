# Continuum SDK Architecture

This document describes the architecture and design decisions of Continuum SDK.

## Overview

Continuum SDK is designed as a modular, extensible framework for building LLM-powered agents. The architecture follows a layered approach with clear separation of concerns.

```
┌─────────────────────────────────────────────────────────────┐
│                      Application Layer                        │
│  (User Code, CLI, Web UI)                                    │
├─────────────────────────────────────────────────────────────┤
│                      API Layer                                │
│  Agent, Session, Config (api.py)                             │
├─────────────────────────────────────────────────────────────┤
│                 Agent Intelligence Layer                      │
│  Checkpoint, History, Planner, Progress, SelfCorrection      │
├─────────────────────────────────────────────────────────────┤
│                   Capability Layer                            │
│  Memory, RAG, Security, Render                               │
├─────────────────────────────────────────────────────────────┤
│                     LLM Layer                                 │
│  Clients (Anthropic, OpenAI, Gemini, ...), Types             │
├─────────────────────────────────────────────────────────────┤
│                    Foundation Layer                           │
│  Config Loader, Providers, Error Handling                    │
└─────────────────────────────────────────────────────────────┘
```

## Module Structure

```
continuum_sdk/
├── __init__.py          # Public API exports
├── api.py               # Unified API entry point
├│
├── agent/               # Agent Intelligence
│   ├── __init__.py
│   ├── checkpoint.py    # State snapshots
│   ├── history.py       # Conversation history
│   ├── intelligent.py   # Intelligent behavior
│   ├── planner.py       # Task planning
│   ├── progress.py      # Progress tracking
│   ├── runtime.py       # Agent runtime
│   ├── self_correction.py # Error correction
│   ├── session.py       # Session management
│   └── task_completion.py # Task finalization
│
├── config/              # Configuration
│   ├── __init__.py
│   ├── loader.py        # Config loading (TOML)
│   ├── providers.py     # Provider definitions
│   └── theme.py         # Theme system
│
├── llm/                 # LLM Integration
│   ├── __init__.py
│   ├── client.py        # Base client
│   ├── errors.py        # Error types
│   ├── fallback.py      # Provider fallback
│   ├── streaming.py     # Stream handling
│   └── types.py         # Message types
│
├── memory/              # Memory System
│   ├── __init__.py
│   ├── layers.py        # Memory tiers
│   └── storage.py       # SQLite storage
│
├── rag/                 # RAG Capabilities
│   ├── __init__.py
│   ├── loader.py        # Document loading
│   ├── splitter.py      # Text splitting
│   └── store.py         # Vector store
│
├── security/            # Security
│   ├── __init__.py
│   ├── path_validator.py    # Path validation
│   ├── permission_checker.py # Permission checks
│   ├── audit_logger.py      # Audit logging
│   └── change_previewer.py  # Change preview
│
├── render/              # Rendering
│   ├── __init__.py
│   └ markdown_renderer.py   # Markdown rendering
│
├── tools/               # Built-in Tools
│   └── __init__.py
│
└── workflow/            # Workflow Support
    └ __init__.py
```

## Design Principles

### 1. Separation of Concerns

Each module handles a specific domain:

- **LLM**: Communication with language models
- **Agent**: Agent behavior and intelligence
- **Security**: Safety and validation
- **Memory**: State persistence

### 2. Dependency Injection

Components receive dependencies, not create them:

```python
# Good - dependency injection
agent = Agent(config=config, memory=memory)

# Bad - internal creation
agent = Agent()  # Creates its own config
```

### 3. Async-First

All LLM operations are async:

```python
async def run(self, prompt: str) -> str:
    response = await self._client.chat(messages)
    return response.content
```

### 4. Protocol-Based Design

Use protocols for flexibility:

```python
from typing import Protocol

class LlmClient(Protocol):
    async def chat(self, messages: list[Message]) -> ChatResponse:
        ...
    
    async def chat_stream(self, messages: list[Message]) -> AsyncIterator[StreamChunk]:
        ...
```

### 5. Error Transparency

Errors are propagated with context:

```python
class LlmError(Exception):
    def __init__(self, message: str, provider: str, model: str):
        self.provider = provider
        self.model = model
        super().__init__(message)
```

## Key Components

### Agent Intelligence

The agent intelligence layer provides sophisticated behavior:

```
┌──────────────────┐
│      Planner     │ Decomposes tasks into steps
├──────────────────┤
│    Progress      │ Tracks execution progress
├──────────────────┤
│ Self-Correction  │ Detects and fixes errors
├──────────────────┤
│    Checkpoint    │ Saves and restores state
├──────────────────┤
│     History      │ Maintains conversation log
└──────────────────┘
```

### Memory System

Four-tier memory architecture:

```
┌───────────────────────────────────────────────────────┐
│                    LONGTERM                           │
│  Permanent storage (project knowledge, learned facts) │
├───────────────────────────────────────────────────────┤
│                    PROJECT                            │
│  Project-scoped data (config, preferences)            │
├───────────────────────────────────────────────────────┤
│                    SESSION                            │
│  Current session data (conversation context)          │
├───────────────────────────────────────────────────────┤
│                    WORKING                            │
│  Immediate context (current task, recent interactions)│
└───────────────────────────────────────────────────────┘
```

### Security Module

Multi-layered security approach:

```
┌─────────────────────────────────────────────────────┐
│                ChangePreviewer                      │
│  Risk assessment before operations                  │
├─────────────────────────────────────────────────────┤
│                AuditLogger                          │
│  Operation logging and traceability                 │
├─────────────────────────────────────────────────────┤
│               PermissionChecker                     │
│  Permission verification                            │
├─────────────────────────────────────────────────────┤
│                PathValidator                        │
│  Path boundary and symlink validation               │
└─────────────────────────────────────────────────────┘
```

### Provider Architecture

Provider abstraction layer:

```python
class ProviderInfo:
    name: str
    display_name: str
    default_model: str
    default_base_url: str
    env_key_name: str
    models: list[str]
    api_format: ApiFormat  # ANTHROPIC, OPENAI, GOOGLE
```

## Data Flow

### Request Flow

```
User Request
    │
    ▼
┌───────────┐
│   Agent   │
└───┬───────┘
    │
    ▼
┌───────────┐
│  Planner  │ Create execution plan
└───┬───────┘
    │
    ▼
┌───────────┐
│ Progress  │ Track steps
└───┬───────┘
    │
    ▼
┌───────────┐
│LLM Client │ Execute prompts
└───┬───────┘
    │
    ▼
┌───────────┐
│Self-Corr. │ Handle errors
└───┬───────┘
    │
    ▼
┌───────────┐
│ Response  │ Return result
└───────────┘
```

### Memory Flow

```
Agent Interaction
    │
    ▼
┌────────────────┐
│ Working Memory │ Store immediate context
└────┬───────────┘
     │ Decay policy
     ▼
┌────────────────┐
│Session Memory  │ Promote important items
└────┬───────────┘
     │ Session end
     ▼
┌────────────────┐
│Project Memory  │ Persist project-level
└────┬───────────┘
     │ Manual promotion
     ▼
┌────────────────┐
│Longterm Memory │ Permanent storage
└────────────────┘
```

## Extension Points

### Custom LLM Client

```python
from continuum_sdk.llm import LlmClient, Message, ChatResponse

class CustomClient(LlmClient):
    async def chat(self, messages: list[Message]) -> ChatResponse:
        # Custom implementation
        ...
    
    async def chat_stream(self, messages: list[Message]):
        # Custom streaming
        ...
```

### Custom Tool

```python
from continuum_sdk import Agent

agent = Agent()
agent.register_tool(
    "my_tool",
    my_function,
    description="Custom tool description",
    parameters={
        "type": "object",
        "properties": {
            "input": {"type": "string"}
        }
    }
)
```

### Custom Memory Tier

```python
from continuum_sdk.memory import MemoryTier, MemoryStore

class CustomMemoryStore(MemoryStore):
    def store(self, tier: MemoryTier, content: str, metadata: dict) -> str:
        # Custom storage logic
        ...
    
    def query(self, query: str, tier: MemoryTier) -> list:
        # Custom query logic
        ...
```

## Performance Considerations

### Connection Pooling

LLM clients maintain connection pools for efficiency:

```python
# Connection reused across requests
client = AnthropicClient()
response1 = await client.chat(messages1)
response2 = await client.chat(messages2)  # Same connection
```

### Async Concurrency

Use `asyncio.gather` for parallel operations:

```python
results = await asyncio.gather(
    client.chat(messages1),
    client.chat(messages2),
    client.chat(messages3)
)
```

### Memory Decay

Automatic cleanup prevents memory bloat:

```python
class DecayPolicy:
    working_ttl: int = 300  # 5 minutes
    session_ttl: int = 3600  # 1 hour
    project_ttl: int = 86400  # 24 hours
```

## Testing Architecture

### Layer Testing

Each layer tested independently:

```python
# LLM layer tests
def test_anthropic_client():
    client = AnthropicClient(api_key="test")
    # Mock API calls

# Agent layer tests
def test_agent_planning():
    agent = Agent(config=test_config)
    plan = agent._planner.create_plan("test task")

# Integration tests
def test_full_flow():
    agent = Agent()
    result = agent.run("hello")
```

### Mocking Strategy

```python
from unittest.mock import AsyncMock

@pytest.mark.asyncio
async def test_with_mock():
    client = AnthropicClient()
    client.chat = AsyncMock(return_value=test_response)
    
    agent = Agent(client=client)
    result = await agent.arun("test")
```

## Future Architecture

### Planned Extensions

1. **Plugin System**: Dynamic plugin loading
2. **Distributed Memory**: Multi-node memory sync
3. **Telemetry**: OpenTelemetry integration
4. **Web Dashboard**: Real-time monitoring

### Versioning Strategy

- Major: Breaking API changes
- Minor: New features, backward compatible
- Patch: Bug fixes

## Summary

Continuum SDK architecture follows:

1. **Layered Design**: Clear separation of concerns
2. **Async-First**: Efficient I/O handling
3. **Protocol-Based**: Flexible implementations
4. **Security-Focused**: Multi-layer protection
5. **Extensible**: Clear extension points

This architecture enables building robust, maintainable, and performant LLM-powered applications.