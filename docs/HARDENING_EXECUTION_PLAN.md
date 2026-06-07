# Continuum Hardening Execution Plan

## Document Status

This document provides detailed, evidence-based execution plans for Hardening Phases 3–7. Each task includes current state, target state, specific files, acceptance tests, risks, and dependencies.

Phases 1–4 are complete. See `SDK_TUI_HARDENING_PLAN.md` for principles, contracts, and governance.

---

## Phase 3: Provider and Config Unification

### Current State (Evidence)

| Component | Providers | Source |
|-----------|-----------|--------|
| Python `BUILTIN_PROVIDERS` | 14 (anthropic, openai, google, gemini, cohere, huggingface, together, groq, deepseek, moonshot, glm, kimi, qwen, grok) | `python/continuum_sdk/config/providers.py:47-270` |
| Python `ProviderType` enum | 5 (ANTHROPIC, OPENAI, GOOGLE, GEMINI, CUSTOM) | `python/continuum_sdk/config/providers.py:14-21` |
| Python `Provider` enum | 5 (ANTHROPIC, OPENAI, GOOGLE, GEMINI, CUSTOM) | `python/continuum_sdk/config/loader.py:107-114` |
| Python `ALLOWED_ENV_VARS` | 33 entries | `python/continuum_sdk/config/loader.py:24-76` |
| CLI `add_provider` defaults | 3 (anthropic, openai, gemini) | `cli/src/commands/config.rs:341-353` |
| CLI `map_provider` routing | 4 (anthropic→Anthropic, openai→OpenAI, gemini→Gemini, other→Custom) | `cli/src/agent/client.rs:204-211` |
| Rust `LlmProvider` enum | 7 (Anthropic, OpenAI, Gemini, AzureOpenAI, Bedrock, Ollama, Custom) | `rust/layer1/src/llm_client.rs:22-30` |
| Rust `LlmProvider::Custom` | Returns error on send/stream | `rust/layer1/src/llm_client.rs:358-359` |
| CLI `list_available_models` | 8 provider model lists | `cli/src/agent/client.rs:416-476` |
| CLI config_detector | 4 env keys (ANTHROPIC, OPENAI, GOOGLE, GEMINI) | `cli/src/tui/setup/config_detector.rs:47-53` |
| Python env whitelist gaps | TOGETHER_API_KEY, GROQ_API_KEY missing | `python/continuum_sdk/config/loader.py:24-76` |

### Key Problem

The CLI/TUI routes deepseek/glm/qwen/kimi/grok/moonshot providers to `LlmProvider::Custom`, which returns an error. Users who add these providers through `continuum config add-provider` will get runtime failures.

Meanwhile, `list_available_models` in the CLI lists models for 8 providers, implying they work.

### Task 3.1: Create Single Provider Registry Source

**Input**: Current scattered provider definitions across Python, Rust, CLI.
**Output**: A single `providers.toml` (or `providers.json`) file in the repo root.

File: `providers.toml`

Each provider entry:
```toml
[providers.anthropic]
name = "anthropic"
display_name = "Anthropic (Claude)"
api_format = "anthropic"
default_base_url = "https://api.anthropic.com"
env_key = "ANTHROPIC_API_KEY"
default_model = "claude-sonnet-4-6"
models = ["claude-opus-4-8", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-6", "claude-sonnet-4-5", "claude-haiku-4-5", "claude-mythos-preview"]
sdk_support = true
cli_support = true   # fully routed in Rust LlmProvider

[providers.deepseek]
name = "deepseek"
display_name = "DeepSeek"
api_format = "openai"
default_base_url = "https://api.deepseek.com/v1"
env_key = "DEEPSEEK_API_KEY"
default_model = "deepseek-v4-pro"
models = ["deepseek-v4-pro", "deepseek-v4-flash", "deepseek-v3.2", "deepseek-v3.1-terminus", "deepseek-v3", "deepseek-chat", "deepseek-reasoner"]
sdk_support = true
cli_support = false  # routed to Custom which errors; needs OpenAI-compatible routing
```

`cli_support` must be `true` only if Rust `LlmProvider` has a working route. All OpenAI-compatible providers that don't have a dedicated Rust variant should eventually route through `OpenAI` with a custom base_url, not `Custom`.

**Acceptance**:
- File exists at repo root.
- Every provider in Python `BUILTIN_PROVIDERS` has an entry.
- `cli_support` accurately reflects current Rust routing.
- Python script or CI check can validate Python registry against this file.

### Task 3.2: Make OpenAI-Compatible Providers Work in CLI

**Input**: providers.toml, Rust `LlmProvider` enum.
**Output**: CLI/TUI can use any OpenAI-compatible provider via `LlmProvider::OpenAI` with custom base_url.

Files to modify:
- `rust/layer1/src/llm_client.rs`: Add `base_url` field to `LlmProvider::OpenAI` variant, or add a new `OpenAICompatible { base_url: String, api_key: String, model: String }` variant with working send/stream.
- `cli/src/agent/client.rs:204-211`: Route OpenAI-compatible providers to the new variant instead of `Custom`.
- `cli/src/commands/config.rs:341-353`: Add default URL/model for all providers from registry.
- `cli/src/tui/setup/config_detector.rs:47-53`: Add all provider env keys from registry.

**Risk**: Changing `LlmProvider` enum is a Rust breaking change within the crate. All match arms must be updated.

**Acceptance**:
- `continuum config add-provider deepseek --key YOUR_KEY` + `continuum run "hello"` succeeds.
- All providers marked `cli_support = true` in registry are routeable.
- Providers marked `cli_support = false` fail with a clear error message, not a silent Custom error.

### Task 3.3: Align Python Provider Registry and Env Whitelist

**Input**: providers.toml.
**Output**: Python `BUILTIN_PROVIDERS` and `ALLOWED_ENV_VARS` match registry.

Files to modify:
- `python/continuum_sdk/config/providers.py`: Regenerate or validate `BUILTIN_PROVIDERS` from providers.toml.
- `python/continuum_sdk/config/loader.py`: Add missing env keys (TOGETHER_API_KEY, GROQ_API_KEY); ensure all provider env keys from registry are present.
- `python/continuum_sdk/config/providers.py`: Reconcile `ProviderType` enum with actual providers.

**Acceptance**:
- Every provider in registry has its env key in `ALLOWED_ENV_VARS`.
- `ProviderType` enum covers all API formats actually used.
- CI test checks Python registry against providers.toml.

### Task 3.4: Unify Default Model/Base URL Across Python/CLI/Rust

**Input**: providers.toml as single source of truth.
**Output**: Python, CLI, and Rust all use the same defaults.

Files to modify:
- `python/continuum_sdk/config/providers.py`: Default models and URLs match registry.
- `python/continuum_sdk/config/loader.py`: `_get_default_model()` matches registry.
- `cli/src/commands/config.rs`: Default URLs and models match registry.
- `rust/layer1/src/llm_client.rs`: Default URLs match registry.

**Acceptance**:
- Same provider has same default model and base_url across all three layers.
- CI test validates consistency.

### Task 3.5: Fix Version Number Inconsistency

**Input**: Current state: `continuum/__init__.py` = `0.1.0`, `continuum_sdk/__init__.py` = `1.0.0`.
**Output**: Single version source.

Files to modify:
- `python/continuum/__init__.py:13`: Change `0.1.0` to `1.0.0`.
- Consider: read version from `pyproject.toml` at build time to prevent future drift.

**Acceptance**:
- `continuum.__version__ == continuum_sdk.__version__`.
- Both match `pyproject.toml` version.

### Phase 3 Dependencies

- Task 3.1 must complete before 3.2, 3.3, 3.4 (registry is the source of truth).
- Task 3.2 can proceed in parallel with 3.3 (different codebases: Rust vs Python).
- Task 3.4 depends on 3.1 and 3.2/3.3.
- Task 3.5 is independent.

### Phase 3 Recommended Team

| Member | Tasks |
|--------|-------|
| **provider-engineer** | 3.1 (registry), 3.3 (Python align), 3.5 (version) |
| **rust-provider-engineer** | 3.2 (CLI/Rust OpenAI-compatible routing), 3.4 (Rust defaults) |

---

## Phase 4: Security Hardening

### Current State (Evidence)

| Component | Status | Source |
|-----------|--------|--------|
| `PathValidator` | Implemented, not integrated into tools | `python/continuum_sdk/security/path_validator.py` |
| `PermissionChecker` | Implemented, not integrated into tools | `python/continuum_sdk/security/permission_checker.py` |
| `AuditLogger` | Implemented, not integrated into tools | `python/continuum_sdk/security/audit_logger.py` |
| `ChangePreviewer` | Implemented, not integrated into tools | `python/continuum_sdk/security/change_previewer.py` |
| `read_file` | No PathValidator, no PermissionChecker, no audit | `python/continuum_sdk/tools/file_ops.py:58-161` |
| `write_file` | No PathValidator, no PermissionChecker, no audit; auto-creates parent dirs | `python/continuum_sdk/tools/file_ops.py:205-286` |
| `edit_file` | No PathValidator, no PermissionChecker, no audit | `python/continuum_sdk/tools/file_ops.py:333-445` |
| `bash` | Blocklist uses `startswith` only; no pipe/redirect parsing; copies full `os.environ`; no audit | `python/continuum_sdk/tools/bash.py` |
| Rust layer0 security modules | Exist (access_controller, input_validator, secrets_manager, pii_scrubber, rate_limiter, encryption_engine, threat_detector) but integration unclear | `rust/layer0/` |

### Task 4.1: Define Security Contract

Before integrating security into tools, define what "secure by default" means:

**File tool contract**:
- All file operations must validate path against configured workspace boundary.
- All file operations must check permission (READ/WRITE/EXECUTE).
- All file operations must log to AuditLogger when audit is enabled.
- PathValidator must be configured with a workspace root (default: CWD or explicit config).
- Validation failures raise clear errors, not silent skips.

**Shell tool contract**:
- Default environment is minimal (PATH, HOME, USER only), not full `os.environ`.
- Command policy: deny list is parsed after shell expansion considerations.
- Dangerous commands require explicit confirmation (policy-driven, not hardcoded).
- All executions are audit-logged.
- Working directory is bounded to workspace.

**Output**: Security contract written as part of this document or as an ADR.

### Task 4.2: Integrate PathValidator into File Tools

**Files to modify**:
- `python/continuum_sdk/tools/file_ops.py`: Add PathValidator and PermissionChecker checks to `read_file`, `write_file`, `edit_file`, `list_directory`.
- `python/continuum_sdk/tools/file_ops.py`: Add AuditLogger calls.

**Design**:
- Tool functions accept an optional `security_context` or read from a global/session config.
- If no workspace boundary is configured, file tools should warn but still operate (to avoid breaking existing users during migration).
- Future: workspace boundary becomes required, warning becomes error.

**Acceptance**:
- `read_file("../../etc/passwd", workspace="/project")` → blocked by PathValidator.
- `write_file("/tmp/outside.txt", workspace="/project")` → blocked.
- `read_file("src/main.py", workspace="/project")` → allowed, audit logged.
- Tests cover: boundary enforcement, symlink escape detection, permission check, audit log entries.

### Task 4.3: Integrate Security Policy into Shell Tool

**Files to modify**:
- `python/continuum_sdk/tools/bash.py`: Replace `startswith` blocklist with proper command parsing. Add environment filtering. Add AuditLogger. Add confirmation policy.

**Design**:
- Parse command into tokens; check each token against policy, not just prefix.
- Default env: `PATH`, `HOME`, `USER`, `TEMP`/`TMPDIR`, plus any explicitly allowed keys.
- Confirmation policy: dangerous commands require `confirm=True` or `--yes` flag.
- Working directory enforced to workspace boundary.

**Acceptance**:
- `bash("echo ok; sudo rm -rf /")` → blocked (sudo in pipeline).
- `bash("ls", env_passthrough=["MY_VAR"])` → allowed, MY_VAR present, other vars absent.
- `bash("rm -rf /tmp/test")` → requires confirmation.
- Audit log records command, result, and policy decision.

### Task 4.4: Fix Security Documentation

**Files to modify**:
- `python/README.md`: Update security examples to use `ValidationResult.is_valid` not truthy check.
- `python/README.md`: Document that security is enforced by default when workspace is configured.
- `python/README.md`: Document shell policy behavior.

**Acceptance**:
- Examples use correct API.
- Docs describe enforced behavior, not just available helpers.
- No "sandbox" or "production-ready security" claims without evidence of enforcement.

### Task 4.5: Evaluate Rust Layer0 Security Integration

**Scope**: Read-only evaluation.

**Questions to answer**:
- Which Rust layer0 modules map to Python security components?
- Can Rust layer0 provide enforcement for CLI/TUI tool execution?
- What is the integration gap between Rust security and Python security?

**Output**: Evaluation report with findings and recommendations.

### Phase 4 Dependencies

- Task 4.1 must complete before 4.2 and 4.3 (contract first).
- Task 4.4 depends on 4.2 and 4.3 (docs reflect implementation).
- Task 4.5 is independent (read-only).

### Phase 4 Recommended Team

| Member | Tasks |
|--------|-------|
| **security-engineer** | 4.1 (contract), 4.2 (file tools), 4.3 (shell tool) |
| **security-reviewer** | 4.4 (docs), 4.5 (Rust eval), final review |

### Phase 4 Execution Status

| Task | Status | Implementation | Key Changes |
|------|--------|----------------|-------------|
| 4.1 Security Contract | ✅ Complete | Defined file + shell tool contracts | PathValidator → PermissionChecker → AuditLogger pipeline; opt-in via workspace parameter |
| 4.2 File Tools Integration | ✅ Complete | `_security.py` + `file_ops.py` | New `SecurityContext`/`resolve_security`/`enforce_path`/`record_audit` helpers; read/write/edit/list all routed through pipeline; new `list_directory` + `ListDirectoryTool` |
| 4.3 Shell Tool Integration | ✅ Complete | `bash.py` rewritten | Token-level policy via `shlex.split` + `_split_glued_separators`; `_SUBSTITUTION_RE` blocks `$()`/backtick/`<(`; `BLOCKED_COMMANDS` includes shell interpreters; `SAFE_ENV_KEYS` whitelist; `confirm` gate for `DANGEROUS_COMMANDS`; 120s default timeout; cwd boundary via enforce_path; AuditLogger for SUCCESS/FAILURE/DENIED |
| 4.4 Security Documentation | ✅ Complete | `python/README.md` + root `README.md` | `result.is_valid` usage; shell policy docs; Rust Layer 0 honestly disclosed as not wired into Python SDK |
| 4.5 Rust Layer0 Evaluation | ✅ Complete | Read-only analysis | Rust security is dead code (not called by CLI/TUI); Python security components also not integrated (now fixed); Rust secrets_manager uses XOR fake encryption (P2 roadmap) |

**Review iterations**:
- Round 1: security-reviewer found 0 P1, 5 P2, 5 P3
- P2-2/3/4 (bash bypasses): semicolon joining, shell-of-shell, command substitution — all fixed by team-lead
- P2-1 (README overclaiming): root + Chinese READMEs corrected
- P2-5 (write_file path order): fixed, then regression P1-NEW found (Permission.WRITE blocks new file creation), fixed with Permission.CREATE
- P2-NEW-1: `_pre_split_separators` corrupted quoted content — replaced with edge-only `_split_glued_separators` (post-shlex)
- P2-NEW-2: subshell parentheses bypass — `_basename` now strips leading/trailing `()'"`
- P3-2: `_security.py` workspace-only auditor was always None — fixed to construct default `AuditLogger()`
- P3-3: `validate_command()` dangerous returned None misleadingly — now returns require-confirm string

**Files modified in Phase 4**:
- `python/continuum_sdk/tools/_security.py` (new)
- `python/continuum_sdk/tools/file_ops.py`
- `python/continuum_sdk/tools/bash.py`
- `python/continuum_sdk/security/path_validator.py` (bug fix: Windows case sensitivity)
- `python/continuum_sdk/security/permission_checker.py` (bug fix: Windows directory read check)
- `python/README.md`
- `README.md` (root)
- `python/tests/test_builtin.py` (updated dangerous command test)

**Deferred to Phase 5/7** (P3 items, non-blocking):
- P3-1: Edge case tests for symlinks / Windows UNC / `~user/xxx` → Phase 7 quality gates
- P3-4: file_ops edit_file backup_path variable style → later cleanup
- P3-5: ChangePreviewer not integrated → out of scope
- P3-6: Prefix command bypass (`env sudo`, `nice sudo`, `nohup sudo`, `xargs sudo`, `find -exec sudo`) → Phase 5 or 7
- P3-7: `find -exec` bypass → Phase 5 or 7

---

## Phase 5: Placeholder and Experimental Feature Cleanup

### Execution Status

| Task | Status | Result |
|------|--------|--------|
| 5.1 Rust plugin/WASM false success | ✅ Complete | Unknown plugin extensions and WASM modules without entry points now return explicit errors. |
| 5.2 Experimental Rust features | ✅ Complete | Sandbox tool execution and S3 storage return contextual `[experimental]` errors; Memory storage is implemented for real. |
| 5.3 Web search fallback | ✅ Complete | DuckDuckGo no-result responses return an empty result list, not fabricated results. |
| 5.4 CLI config loader | ✅ Complete | Loader delegates to `ConfigManager` for file/env/default/full config loading. |
| 5.5 Python public API cleanup | ✅ Complete | Public unavailable tool paths raise `ToolNotAvailableError`; abstract `pass` statements remain only where intentional. |

**Verification**:
- `python -m pytest python/tests/test_builtin.py python/tests/test_builtin_coverage.py -q` → 121 passed.
- `cargo test --manifest-path rust/layer3/Cargo.toml` → 214 passed, doc-tests passed/ignored as expected.
- `cargo test --manifest-path rust/layer4/Cargo.toml plugin_loader` → 29 unit + 13 integration + 1 plugin integration passed.
- `cargo test --manifest-path cli/Cargo.toml config::loader` → 3 lib + 3 main tests passed.
- Independent Phase 5 spec review: PASS; no remaining Requirement 10 blocker.

### Initial State (Evidence)

| Location | Placeholder Type | Source |
|----------|-----------------|--------|
| Plugin loader: non-.so/.wasm files | Returned `Ok(name)` with "placeholder" log | `rust/layer4/src/plugin_loader/mod.rs` |
| WASM: no entry function | Returned `{"status": "executed"}` JSON | `rust/layer4/src/plugin_loader/wasm.rs` |
| Sandbox: tool execution | Returned generic `Err("not yet implemented")` | `rust/layer3/src/sandbox_runtime.rs` |
| Storage: Memory and S3 | Memory and S3 returned unimplemented errors | `rust/layer1/src/storage_engine.rs` |
| Web search: no DuckDuckGo results | Returned formatted placeholder result | `rust/layer3/src/builtin_tools/web_search.rs` |
| Python: builtin.py | Runtime `NotImplementedError` for unavailable public tools | `python/continuum_sdk/tools/builtin.py` |
| Python: ABC-style modules | `pass` in abstract base class methods | `python/continuum_sdk/memory`, `python/continuum_sdk/rag`, `python/continuum_sdk/llm` |
| Python: agent/checkpoint.py | Placeholder checkpoint binding class and cleanup `pass` | `python/continuum_sdk/agent/checkpoint.py` |
| CLI: config/loader.rs | Methods returned `Ok(())` while doing nothing | `cli/src/config/loader.rs` |

### Classification

**Category A: False success (must fix immediately)**
- WASM placeholder returning `status: "executed"` — misleads callers.
- Plugin loader returning `Ok(name)` for unknown extensions — hides load failure.

**Category B: Clear error already (acceptable, may need better message)**
- Sandbox `Err("not yet implemented")` — correct behavior, but should be `ExperimentalFeatureError` or similar.
- Storage `Err("not yet implemented")` — same, 10 methods.

**Category C: Abstract base classes (acceptable by design)**
- Python `pass` in ABC methods — these are intentional abstract interfaces.
- Subclasses that inherit without overriding will get `NotImplementedError` from Python's ABC mechanism or `AttributeError`.

**Category D: Fallback behavior (needs decision)**
- Python `builtin.py` `NotImplementedError` — this is the correct degraded behavior when Rust binding is unavailable. Should it be a clearer error message?
- CLI `config/loader.rs` empty — is this dead code or still referenced?

### Task 5.1: Fix False Success in Rust Plugin/WASM

**Files to modify**:
- `rust/layer4/src/plugin_loader/mod.rs:287-296`: Return `Err` for unknown extensions instead of `Ok(name)`.
- `rust/layer4/src/plugin_loader/wasm.rs:272-285`: Return `Err` when no entry function found, not `{"status": "executed"}`.

**Acceptance**:
- Unknown plugin extensions fail with clear error.
- WASM modules without entry function fail with clear error.
- Tests verify error paths.

### Task 5.2: Classify and Label Experimental Rust Features

**Files to modify**:
- `rust/layer3/src/sandbox_runtime.rs:475`: Change error to `ExperimentalFeatureError("sandbox tool execution is experimental and not fully implemented")`.
- `rust/layer1/src/storage_engine.rs:67-156`: Change errors to `ExperimentalFeatureError` for Memory and S3 variants. Add `#[cfg(feature = "experimental-storage")]` gate if appropriate.

**Acceptance**:
- Experimental features return clear, categorized errors.
- Error messages include "experimental" label.
- Tests verify error messages.

### Task 5.3: Fix Web Search Fallback

**Files to modify**:
- `rust/layer3/src/builtin_tools/web_search.rs:384-394`: Return empty results or `Err`, not a formatted placeholder that looks like a real result.

**Acceptance**:
- No DuckDuckGo results → empty result list or clear "no results" error, not fake result objects.
- Tests verify empty result handling.

### Task 5.4: Evaluate and Fix CLI config/loader.rs

**Steps**:
1. Check if `cli/src/config/loader.rs` is referenced anywhere.
2. If dead code → delete.
3. If referenced → replace with real implementation using `ConfigManager`, or mark as `// TODO: implement before release` with a tracking issue.

**Acceptance**:
- No empty `Ok(())` implementations remain in active code paths.
- Dead code is removed.

### Task 5.5: Review Python ABC `pass` Statements

**Scope**: Read-only review.

**Files**:
- `python/continuum_sdk/memory/storage.py`
- `python/continuum_sdk/rag/retriever.py`
- `python/continuum_sdk/rag/vectorstore.py`
- `python/continuum_sdk/llm/client.py`
- `python/continuum_sdk/agent/checkpoint.py`

**Decision needed**: Are these abstract base classes (acceptable) or concrete classes with missing implementations (must fix)?

If they are ABCs: add `abc.ABC` and `@abstractmethod` decorators so Python enforces implementation.
If they are concrete classes: implement or remove.

**Acceptance**:
- All `pass` in base classes are either `@abstractmethod` or have real implementations.
- No silent no-op methods in concrete classes.

### Phase 5 Dependencies

- Task 5.1 is highest priority (false success).
- Tasks 5.2, 5.3 can proceed in parallel.
- Task 5.4 depends on usage analysis.
- Task 5.5 is independent.

### Phase 5 Recommended Team

| Member | Tasks |
|--------|-------|
| **rust-cleanup-engineer** | 5.1 (plugin/WASM), 5.2 (experimental labeling), 5.3 (web search) |
| **python-cleanup-engineer** | 5.4 (CLI loader), 5.5 (Python ABC review) |

---

## Phase 6: Documentation Rebuild

### Execution Status

| Task | Status | Result |
|------|--------|--------|
| 6.1 Root README | ✅ Complete | Removed competitor analogies, simplified to product links structure |
| 6.2 Python SDK README | ✅ Complete | Fixed MemorySystem API, added illustrative markers, added return types |
| 6.3 CLI README | ✅ Complete | Full rewrite with 15 commands, TUI docs, keyboard shortcuts, provider table |
| 6.4 Historical Docs | ✅ Complete | Fixed eval → ast.literal_eval, deprecated old APIs, added benchmark disclaimers, archived historical docs |
| Code Verification | ✅ Complete | Fixed VectorStore, Memory, Checkpoint API mismatches per examples-verifier report |

**Key Changes**:
- Root README: Product positioning without competitor claims, links to SDK/CLI docs
- Python README: MemorySystem example uses correct `store(tier, content)` API; VectorStore uses `.upsert()` not `.add()`
- CLI README: 522-line comprehensive docs matching registry provider list
- API_EXAMPLES.md: Checkpoint section uses `CheckpointClient`; Memory section uses `MemorySystem` from `continuum_sdk.api`
- docs/user/*.md: Fixed Memory and Checkpoint examples to match actual API
- Security: No `eval()` in public docs, P1 issues resolved
- Archives: `docs/archive/` created for historical design docs

### Initial State (Evidence)

| Document | Initial Issues |
|----------|----------------|
| Root README | Competitor analogies (Claude Code + Aider + OpenClaw CLI) at lines 22-25 |
| Python README | MemorySystem API mismatch, missing illustrative markers |
| CLI README | Minimal 39 lines, missing 90% of features |
| `docs/API_DESIGN_DRAFT.md` | `eval` example at line 976 |
| `docs/USER_MANUAL.md` | `eval` example at line 329 |
| `docs/benchmarks/` | Unverifiable "Rust预期" claims |
| Historical docs | Not archived, mixing internal design with user docs |

### Task 6.1: Rewrite Root README

**Target**: Clean, product-focused, no competitor claims.

Structure:
1. What is Continuum (2 products: SDK + CLI/TUI)
2. Quick Start for SDK
3. Quick Start for CLI/TUI
4. Architecture overview (Rust core + Python API layer)
5. Links to SDK README and CLI README

**Acceptance**:
- No competitor analogies.
- No unverifiable performance claims.
- Clear product boundary between SDK and CLI/TUI.

### Task 6.2: Audit and Fix Python SDK README

**Target**: All examples match stable public API contract.

**Checklist**:
- All imports use `from continuum import ...` or `from continuum_sdk import ...` correctly.
- All Agent examples use `run()`, not `chat()/start()`.
- All Session examples use `add_user_message()`, `save(path)`, `Session.load(path)`.
- Security examples use `result.is_valid`, not truthy check on `ValidationResult`.
- VectorStore import uses `from continuum_sdk.rag import ...`.
- Workflow uses `DAG/Node`, not `Workflow/Step`.
- No `eval` or unsafe examples.

**Acceptance**:
- Every code block is smoke-tested or explicitly marked "illustrative".
- No code block references a non-existent API.

### Task 6.3: Rewrite CLI README

**Target**: Complete CLI/TUI documentation.

Structure:
1. Installation
2. Configuration (`continuum config init`, `add-provider`)
3. Running agents (`continuum run`)
4. TUI mode
5. Session/checkpoint
6. Supported providers (from registry, with `cli_support` flag)
7. Experimental features

**Acceptance**:
- Provider list matches registry `cli_support` status.
- All commands documented.
- No SDK-only features claimed.

### Task 6.4: Clean Up Historical Docs

**Target**: Remove or archive misleading content.

**Files**:
- `docs/API_DESIGN_DRAFT.md`: Replace `eval` example with `ast.literal_eval`.
- `docs/ARCHITECTURE_V4.md`: Move competitor references to internal design docs.
- `docs/benchmarks/`: Add reproducibility notes or mark as "estimated, not measured".
- Any docs referencing `chat()/start()/resume_session()`: Update or archive.

**Acceptance**:
- No `eval` in public docs.
- No competitor claims in user-facing docs.
- Benchmarks clearly labeled as measured or estimated.

### Phase 6 Dependencies

- Tasks 6.1, 6.2, 6.3 can proceed in parallel.
- Task 6.4 can proceed in parallel.
- All depend on Phase 3 provider registry being available for accurate provider lists.
- Task 6.2 depends on Phase 4 security docs being correct.

### Phase 6 Recommended Team

| Member | Tasks |
|--------|-------|
| **doc-writer** | 6.1 (root README), 6.3 (CLI README) |
| **doc-verifier** | 6.2 (SDK README audit), 6.4 (historical docs cleanup) |

---

## Phase 7: Quality Gates

### Task 7.1: Public API Smoke Tests

**Files to create/modify**:
- `python/tests/test_smoke.py`: Test `pip install` + `from continuum import Agent, Session, Config` + basic construction.

**Acceptance**:
- Test runs in clean venv.
- Tests cover: import, Agent construction, Session save/load, Config from_env.

### Task 7.2: CLI Smoke Tests

**Files to create/modify**:
- `cli/tests/smoke_test.sh` or equivalent: Test `continuum --help`, `continuum config init`, `continuum run --help`.

**Acceptance**:
- Test runs after `cargo build`.
- Covers: help output, config initialization, version flag.

### Task 7.3: Provider Registry Consistency Tests

**Files to create/modify**:
- `python/tests/test_provider_registry.py`: Validate Python BUILTIN_PROVIDERS matches providers.toml.
- `rust/` or `cli/` test: Validate Rust/CLI provider defaults match providers.toml.

**Acceptance**:
- CI fails if Python/Rust/CLI provider data drifts from registry.

### Task 7.4: Security Boundary Tests

**Files to create/modify**:
- `python/tests/test_security_enforcement.py`: Test PathValidator integration in file tools, shell policy, audit logging.

**Acceptance**:
- File tools enforce workspace boundaries.
- Shell tool enforces command policy.
- Audit logs contain expected entries.

### Task 7.5: README Snippet Tests

**Files to create/modify**:
- `python/tests/test_readme_snippets.py`: Extract and run code blocks from README.md.

**Acceptance**:
- All README code blocks that are not marked "illustrative" execute successfully.
- Failures block CI.

### Task 7.6: Placeholder Success Detection

**Files to create/modify**:
- Add CI check or test that searches for placeholder success patterns:
  - `status.*executed` in Rust test output
  - `Ok(name)` in plugin loader
  - `"not yet implemented"` in error messages that should be experimental markers

**Acceptance**:
- CI fails if new placeholder success paths are introduced.

### Phase 7 Dependencies

- All Phase 7 tasks depend on their corresponding Phase being complete.
- Task 7.1 depends on Phase 2.
- Task 7.3 depends on Phase 3.
- Task 7.4 depends on Phase 4.
- Task 7.5 depends on Phase 6.
- Task 7.6 depends on Phase 5.

### Phase 7 Recommended Team

| Member | Tasks |
|--------|-------|
| **qa-engineer** | 7.1–7.6 |

---

## Cross-Phase Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Changing `LlmProvider` enum breaks Rust crate internal API | Phase 3 | Do it once, update all match arms, test thoroughly |
| Security enforcement breaks existing tool users | Phase 4 | Default to warn-only mode first; make enforcement opt-in, then opt-out, then required |
| Removing placeholder success breaks callers that depend on it | Phase 5 | Audit callers first; if any depend on false success, fix the caller |
| Dual pyproject.toml (maturin vs hatchling) causes publish confusion | Cross-cutting | Decide: maturin for all builds, or hatchling for pure-Python wheel + maturin for binary wheel |
| Historical docs cleanup removes useful design context | Phase 6 | Archive rather than delete; move to `docs/archive/` |

---

## Execution Order

```
Phase 3 (Provider/Config)
    ↓
Phase 4 (Security) ── can start 3.2+ in parallel
    ↓
Phase 5 (Placeholder) ── can start in parallel with 4
    ↓
Phase 6 (Docs) ── depends on 3, 4, 5 being mostly done
    ↓
Phase 7 (Quality Gates) ── depends on all above
```

Phases 4 and 5 can proceed in parallel since they touch different code areas (Python tools vs Rust internals).

Phase 6 should wait until Phases 3-5 are mostly complete to avoid writing docs that immediately become outdated.

Phase 7 is last but individual tests can be written as each phase completes.
