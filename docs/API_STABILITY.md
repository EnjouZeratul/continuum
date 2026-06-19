# API Stability Policy

Continuum is pre-1.0 (v1.x). This document declares which APIs are stable vs experimental.

## Crate-Level Stability

| Crate | Status | SemVer Commitment |
|-------|--------|-------------------|
| `sh-layer0` | **Stable** | Security primitives, rarely changes |
| `sh-layer1` | **Stable** | LLM client / config / cache — core plumbing |
| `sh-layer2` | **Stable** | Session / Tool trait / checkpoint |
| `sh-layer3` | **Evolving** | Builtin tools actively hardened; tool API may adjust |
| `sh-layer4` | **Evolving** | Integration layer, CLI/TUI may change |
| `sh-core` | **Stable facade** | Re-exports, follows underlying layers |
| `continuum` (CLI) | **Evolving** | CLI flags / TUI may change |
| `continuum-agent-sdk` (Python) | **Evolving** | PyO3 bindings, may adjust |

## Public API Classification

### Stable (SemVer protected)
- `BuiltinTool` trait definition (`name`/`description`/`parameters_schema`/`category`/`execute`)
- `Tool` trait (Layer 2)
- Core types: `Session`, `Message`, `ToolRequest`, `ToolResponse`, `ToolResult`
- `FileOpsLimits` struct + builder methods
- `BuiltinToolRegistry` API

### Experimental (may change in minor versions)
- `execute_with_context` / `execute_with_call_id` (v1.1 additive, signature may evolve)
- `ExecutionContext` (fields may expand)
- `ReadStateStore` / `StaleReadError` (stale-read internals)
- `UrlValidator` / `DefaultUrlValidator` (SSRF, may add methods)
- `SecretScrubber` (patterns may expand)

### Internal (not SemVer protected, `#[doc(hidden)]` candidates)
- `ToolAdapter` bridge implementation
- `exec_context` module (process-wide context plumbing)
- `metrics` counters (internal observability)
- All `builtin_tools::*` tool struct internals (fields, private methods)

## Migration Guarantees

- **Patch (1.x.Y)**: Bug fixes, security fixes, no API changes. CVE fixes (e.g., PyO3 0.29 upgrade) are patch.
- **Minor (1.X.0)**: New features, additive API changes. Existing stable API unchanged. Experimental API may adjust.
- **Major (2.0.0)**: Breaking changes to stable API. Not planned until post-1.0 maturation.

## Deprecated Items

- `BuiltinTool::execute` (legacy, no context) — superseded by `execute_with_context`. Will not be removed (47 tools depend on it).
- `std::env::set_var` in `SetEnvTool` — wrapped in `#[allow(deprecated)]` for Rust 2024 forward-compat.

## Versioning History

| Version | Change |
|---------|--------|
| 1.0.3 | FileOps hardening (size limits, binary detection, overwrite protection) |
| 1.0.4 | P0 critical (BashTool denylist, SSRF, DeleteFileTool path safety) |
| 1.0.5 | P1 (env scrubbing, MoveFile path safety, git hardening) |
| 1.0.6 | P2 (memory UTF-8 fix, checkpoint path, WebSearch) |
| 1.0.7 | P3 (RegexMatch ReDoS, TextDiff cap) |
| 1.1.0 | Stale-read prevention + PyO3 0.29 + ExecutionContext + CWE security suite |
