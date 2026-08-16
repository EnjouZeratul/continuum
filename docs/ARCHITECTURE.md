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
CLI entry point, TUI (ratatui), HTTP server, WebSocket, MCP bridge, plugin loader (dylib + wasmtime), capability installation (`capability_tools.rs` — the agent-authored tool lifecycle). The "interface" layer.

### `sh-safety` (Pure Validation)
Dependency-minimal crate for path safety, UTF-8 truncation, secret scrubbing, and the self-modification policy. No async, no I/O — extracted so fuzz targets compile in seconds. Consumed by layers 3–4.

### `sh-core` (Unified Re-Exports)
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

### D6: Self-evolution pipeline (policy-gated, sandbox-verified)
The agent can grow its own capability set at runtime through five cooperating pieces:

1. **`SelfModificationPolicy`** (`sh-safety`, decides before anything runs): three-valued decisions (Allow / Deny / RequiresApproval), hostile tool-name validation (path traversal, metacharacters, reserved names, non-ASCII), dynamic-tool count cap, builtin protection list, and an agent-core path guardrail (`.continuum/{core,registry,policy}` matched as component sequences — writes there are the sandbox-escape vector). `SelfModificationPolicy::locked()` is the kill switch.
2. **`install_capability`** (`layer4`): agent authors a tool as WAT text (or base64 WASM); wasmtime compiles it in-process (no external toolchain, no shell); the module must run under the sandboxed `CapabilitySet` (no fs/net/process, 16MB/5s) and pass a mandatory smoke test before registration; failures unload cleanly. Actionable failures surface as `is_error` results so the agent can self-correct. Compare DeepSeek Harness: same "agent writes code" capability, but DSH has no real sandbox — Continuum's differentiator.
3. **`SkillStore`** (`layer3`): skills are *data* — parameterized tool-call scripts (`{{param}}` templating: full-value → typed JSON substitution, partial → string interpolation) persisted one JSON file per skill with usage/success statistics. `run_skill` executes steps through the live Layer 2 registry (so skills compose with dynamic WASM tools); `improve_skill` updates in place preserving id + stats.
4. **Persistent memory** (`layer3`): Save/Query/Clear share one `Arc<UnifiedMemorySystem>` (fixes the former split-brain where each tool had a private memory); `query_all` spans Working→Session→Project→LongTerm; `ProjectMemory` lazy-loads disk state (survives restart); `promote_session_end` copies high-importance session entries into project tier (`HeuristicImportanceScorer`: base + log access bonus + 24h recency).
5. **`LlmTaskDecomposer`** (`layer2`): LLM-generated subtask DAGs replace keyword heuristics when a client is available; when the task needs a capability missing from the registry, the plan itself can contain an `install_capability` subtask (the capability-gap → tool-creation loop). Any failure — no client, API error, unparseable output, cyclic DAG — degrades to the heuristic decomposer; `PlanResult::source` marks which produced the plan.

### D7: Capability tools live in Layer 4, not Layer 3
`install_capability` needs the wasmtime loader (Layer 4) and the live registry (Layer 2). Since the dependency direction is L2→L3→L4, placing it in Layer 3 would create a cycle. Skill tools *do* live in Layer 3 (they only need `Arc<ToolRegistry>`, which Layer 3 already depends on).

## Python SDK (`continuum-agent-sdk`)

PyO3 bindings (`rust/sh-python/`) expose Layer 3+4 to Python. The `continuum` CLI binary embeds both Rust and Python (via maturin build). Python users get the same hardened tools via Rust backend.
