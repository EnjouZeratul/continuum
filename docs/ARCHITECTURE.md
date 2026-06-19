# Continuum Architecture

5-layer Rust agent SDK. Each layer has a single responsibility and depends only on lower layers.

```
Layer 4: Integration    CLI · TUI · HTTP · WebSocket · MCP · Plugin Loader
    ↓
Layer 3: Capabilities   Tools (50+) · Memory · RAG · LSP · Workflow
    ↓
Layer 2: Core Engine    Session · Context · Orchestration · Checkpoint
    ↓
Layer 1: Foundation     LLM Client · Streaming · Config · Cache · Storage
    ↓
Layer 0: Security       Sandbox · Permissions · PII Scrubbing · Validation
```

## Dependency Direction (verified via Cargo.toml)

```
cli ──→ sh-layer4 ──→ sh-layer3 ──→ sh-layer2 ──→ sh-layer1
                                                    ↓
                                               sh-layer0
```

**Rule**: Higher layers depend on lower layers. Lower layers never reference higher ones. This is enforced by Cargo dependency declarations (no reverse deps possible).

| Crate | Depends on |
|-------|-----------|
| `sh-layer0` | (standalone) |
| `sh-layer1` | (standalone) |
| `sh-layer2` | `sh-layer1` |
| `sh-layer3` | `sh-layer1`, `sh-layer2` |
| `sh-layer4` | `sh-layer3` |
| `sh-core` | All layers (unified re-exports) |
| `continuum` (CLI) | `sh-layer4`, `sh-core` |

## Layer Responsibilities

### Layer 0: Security Gateway (`sh-layer0`)
Sandbox execution, permission checking, input validation, PII scrubbing, rate limiting. No dependency on other layers. Provides the security primitives that upper layers use.

### Layer 1: Foundation (`sh-layer1`)
LLM client (multi-provider: Anthropic/OpenAI/Gemini/DeepSeek/GLM/Qwen/Kimi), streaming SSE parser, config manager, cache (moka), storage (sled). The "plumbing" layer.

### Layer 2: Core Engine (`sh-layer2`)
Session management, context handling, agent orchestration, checkpoint system (crash recovery), tool registry (Layer 2 `Tool` trait), workflow engine (DAG). The "brain" layer.

### Layer 3: Capabilities (`sh-layer3`)
50+ built-in tools (file ops, shell, search, network, git, system, memory, code analysis, data processing, text processing), tool executor (`BuiltinTool` trait), RAG retriever, LSP client, vector store. The "hands" layer.

**Key subsystem**: `builtin_tools/` — all tools implement `BuiltinTool` trait, hardened with `FileOpsLimits` config + security modules (`path_safety`, `network_safety`, `secret_scrub`, `read_state`, `safe_truncate`).

### Layer 4: Integration (`sh-layer4`)
CLI entry point, TUI (ratatui), HTTP server, WebSocket, MCP bridge, plugin loader (dylib + wasmtime). The "interface" layer.

### `sh-core` (Unified Re-exports)
Convenience crate that re-exports all layers. For users who want `use sh_core::*` without managing 5 dependencies.

## Key Design Decisions

### D1: BuiltinTool trait (Layer 3) vs Tool trait (Layer 2)
Layer 3 has its own `BuiltinTool` trait (`execute(args) → String`). Layer 2 has `Tool` trait (`execute(args: &str) → ToolResult`). They are bridged by `ToolAdapter` (`builtin_tools/adapter.rs`). This separation allows Layer 3 tools to be rich (typed args, JSON) while Layer 2 remains protocol-agnostic.

### D2: Stale-read prevention via ExecutionContext
`BuiltinTool::execute_with_context` (additive method, default delegates to `execute`) passes `ExecutionContext` to tools. File tools (`Read`/`Write`/`Edit`) use it for SHA-256-based stale-read detection. The context is set/cleared per-call by `ToolAdapter` (process-wide via `LazyLock`).

### D3: Security at tool layer (not just sandbox)
Unlike Claude Code (which relies on sandbox + permission prompts), Continuum enforces security **inside each tool** (size limits, SSRF, path checks, secret scrubbing). This enables safe operation without external sandbox — the core differentiator for server-side/embedded deployment. See `SECURITY_INVARIANTS.md`.

### D4: Additive trait methods (no breaking changes)
`execute_with_context` and `execute_with_call_id` are additive (default impl delegates to legacy). This allows v1.0.x→1.1.0 evolution without breaking 47 existing tool implementations.

### D5: anyhow (internal) + thiserror (boundary) error model
`Layer3Result = anyhow::Result<T>` for tool-internal errors (dynamic, message-driven). `Layer3Error` (thiserror enum) at executor boundary for structured categorization. See `METRICS.md` for rationale (avoids over-engineering).

## Python SDK (`continuum-agent-sdk`)

PyO3 bindings (`rust/sh-python/`) expose Layer 3+4 to Python. The `continuum` CLI binary embeds both Rust and Python (via maturin build). Python users get the same hardened tools via Rust backend.
