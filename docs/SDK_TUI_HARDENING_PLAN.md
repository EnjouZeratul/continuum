# Continuum SDK/TUI Hardening Plan

## Purpose

This document records the technical hardening direction for Continuum when there is no release deadline pressure. The goal is to make the project genuinely reliable, maintainable, and honest about its capabilities.

Continuum should be treated as two related but independent products:

1. **Continuum Python SDK**: a Python API layer backed by the Rust core engine, with a pure Python fallback when Rust bindings are unavailable.
2. **Continuum CLI/TUI**: a terminal agent product powered by the Rust/agent runtime.

The Rust core engine is the primary execution backend. The Python SDK wraps it with a user-friendly API. A pure Python fallback exists so the SDK remains installable without compiled extensions, but it provides reduced functionality and is not the design target.

---

## Target Architecture

### Python SDK

The SDK should support two execution tiers:

| Tier | Description | Requirement |
|------|-------------|-------------|
| Python API + Rust core | Python SDK calls Rust engine for full functionality: runtime, session persistence, tools, security, storage | Primary |
| Pure Python fallback | Works without compiled extensions when Rust bindings are unavailable; reduced functionality | Fallback |

The stable user-facing API should be simple and consistent:

```python
from continuum import Agent, Session, Config
```

The lower-level implementation may live under `continuum_sdk`, but public examples should not mix multiple incompatible `Agent` or `Session` types.

### CLI/TUI

The CLI/TUI should have its own documented capability boundary. It should not inherit SDK provider claims unless the Rust/CLI implementation actually supports them.

The CLI/TUI documentation should clearly state:

- supported providers,
- configuration format,
- session/checkpoint behavior,
- TUI capabilities,
- unsupported or experimental features.

---

## Core Principles

1. **Reality first**: every documented capability must correspond to implemented and tested behavior.
2. **Few false promises**: incomplete features must not return success placeholders.
3. **Stable public API**: users should not need to know internal API layers for common tasks.
4. **Rust core is primary**: the Rust engine provides the real runtime, session persistence, tools, and security; Python SDK wraps it.
5. **Pure Python fallback is degraded**: the SDK must remain installable without Rust bindings, but with reduced functionality and clear user-facing warnings.
6. **Single source of truth**: provider metadata, supported models, environment variables, and defaults should not drift across Python, Rust, CLI, and docs.
7. **Security by construction**: security components should be enforced by tools, not only exposed for manual use.
8. **Docs are tests**: README examples and API snippets should be executable or explicitly marked non-executable.

---

## Architecture Decision Records

Key architectural decisions should be recorded before they are spread across implementation, tests, and documentation. The purpose is not to slow development, but to make tradeoffs explicit and prevent later contributors from rediscovering the same constraints.

Record an Architecture Decision Record for decisions such as:

- the Rust core engine as the primary backend and the role of the pure Python fallback,
- the stable public import path and public entry points,
- the boundary between SDK APIs and CLI/TUI behavior,
- the provider registry source of truth and generation or validation strategy,
- the security policy model for file access, shell execution, permissions, and audit logging,
- the compatibility expectations for configuration formats, session data, and persisted checkpoints.

Each record should include:

- context and constraints,
- decision,
- considered alternatives,
- consequences and known risks,
- expected validation through tests, examples, or documentation.

A decision can be revised, but the replacement should explain why the previous decision is no longer appropriate.

---

## API Stability Policy

The project should separate public stability promises from implementation flexibility. Before a stable release, breaking changes are acceptable when they improve correctness, safety, or coherence, but they should still include migration notes so examples, downstream experiments, and internal code can be updated deliberately.

API categories:

| Category | Examples | Compatibility expectation |
|----------|----------|---------------------------|
| Stable public API | documented imports, quick-start objects, CLI commands, config keys, provider names | Avoid breaking changes once declared stable; require migration notes for any change |
| Advanced/internal API | lower-level runtime hooks, extension points, generated registries, Rust integration surfaces | May change when needed; document only when users are expected to call it directly |
| Experimental API | sandbox execution, plugin/WASM paths, incomplete provider modes, prototype TUI features | May change or be removed; must be explicitly labeled and must not imply production support |

Rules:

- Public examples should use only stable public API unless explicitly marked experimental.
- Internal modules should not become de facto public APIs through README examples.
- Experimental APIs should fail clearly when unavailable instead of returning placeholder success.
- Breaking changes before release should be intentional, reviewed, and accompanied by migration notes.
- After a stable release, compatibility expectations should be tightened for documented SDK imports, CLI behavior, provider identifiers, and persisted user configuration.

---

## Contract-First Quality Approach

For foundational behavior, define the external contract before treating an implementation as complete. This is especially important when there is no deadline pressure: the project can afford to decide what correct behavior means before optimizing or expanding feature surface area.

Contracts to define first:

- public Python SDK imports, object lifecycle, method names, and error behavior,
- provider matrix for SDK and CLI/TUI, including supported, unsupported, and experimental states,
- security boundaries for file access, shell execution, environment propagation, permissions, and audit logging,
- CLI command behavior, exit codes, configuration loading, session/checkpoint behavior, and TUI capability boundaries,
- documentation examples that are expected to run as smoke tests.

Implementation, tests, and documentation should then be built against those contracts. When implementation differs from the contract, either the implementation should change or the contract should be revised through review; documentation should not be used to paper over the mismatch.

---

## Definition of Done

A phase is complete only when the implementation, verification, and user-facing truth are aligned.

For each phase, completion requires:

- implemented behavior or explicit unsupported/experimental status,
- tests covering the intended behavior and important failure paths,
- documentation that matches the implemented behavior,
- runnable examples or smoke tests for public quick starts,
- no placeholder success paths for incomplete functionality,
- provider, security, packaging, and CLI claims checked against the relevant contract,
- pure Python fallback behavior verified when Rust bindings are unavailable,
- CLI/TUI behavior verified independently from SDK behavior,
- a second review pass focused on drift, overclaiming, and weak tests.

A phase should not be marked done because the design is clear, the scaffolding exists, or a future implementation path is obvious. If a capability is not implemented, the done state is to mark it accurately and make unsupported behavior fail clearly.

---

## Hardening Phases

### Phase 1: Reality Audit

Create an authoritative inventory of current behavior.

Deliverables:

- Public Python API inventory.
- CLI/TUI feature inventory.
- Provider support matrix for SDK and CLI/TUI separately.
- Rust binding import and packaging inventory.
- Placeholder/stub inventory.
- Documentation claim inventory.

Acceptance criteria:

- Every public claim is marked as implemented, experimental, unsupported, or needs verification.
- No issue is closed based only on intent or architecture diagrams.

---

### Phase 2: Public API and Packaging Hardening

Stabilize the user entry points before improving secondary features.

Tasks:

- Decide whether `continuum` or `continuum_sdk` is the primary public import path.
- Make `from continuum import Agent, Session, Config` stable if it remains in README examples.
- Ensure public `Agent` and `Session` methods match documentation.
- Resolve Python package source of truth:
  - pure Python package under `python/`, with Rust core as the primary extension, or
  - unified root maturin package.
- Resolve Rust extension module name:
  - `continuum_sdk._continuum`, or
  - `sh_python`, but not both in conflicting roles.
- Remove or implement nonexistent `continuum.cli:main`.
- Align CLI crate name, binary name, and installation docs.

Acceptance criteria:

- `pip install -e python` supports documented imports.
- Rust core import failure degrades gracefully to pure Python fallback with a clear warning.
- CLI install instructions match Cargo metadata.
- README quick starts for SDK and CLI run successfully.

---

### Phase 3: Provider and Config Unification

Prevent SDK/CLI/provider drift.

Tasks:

- Define a single provider registry file or generator source.
- Include provider metadata:
  - name,
  - display name,
  - API format,
  - default base URL,
  - env key,
  - default model,
  - known models,
  - SDK support status,
  - CLI/TUI support status.
- Generate or validate Python provider registry from that source.
- Generate or validate CLI/Rust provider defaults from that source.
- Make OpenAI-compatible providers work in CLI/TUI, or explicitly mark them SDK-only.
- Align environment variable whitelist with provider registry.

Acceptance criteria:

- Python SDK, CLI/TUI, and docs report the same provider matrix.
- Unsupported providers fail with clear errors.
- Custom/OpenAI-compatible provider behavior is either implemented or explicitly unavailable.

---

### Phase 4: Security Hardening

Move from security components to enforced safe behavior.

Tasks:

- Route file tools through `PathValidator`.
- Route file and shell tools through `PermissionChecker`.
- Log tool operations through `AuditLogger` when audit is enabled.
- Avoid copying full host environment into shell commands by default.
- Add command policy for shell execution:
  - deny destructive commands by default,
  - require confirmation for dangerous operations,
  - support explicit allowlists where appropriate.
- Ensure security examples use correct return values such as `ValidationResult.is_valid`.

Acceptance criteria:

- File tools cannot access paths outside configured boundaries by default.
- Shell tool behavior is policy-controlled and auditable.
- Security docs describe enforced behavior, not just available helper classes.

---

### Phase 5: Placeholder and Experimental Feature Cleanup

Remove misleading success paths.

Tasks:

- Replace placeholder success in plugin loader and WASM paths with explicit errors or real implementations.
- Mark sandbox tool execution as experimental or implement it fully.
- Remove fake search results from web search fallback paths.
- Replace empty/simplified config loaders with real implementations or remove dead code.
- Remove runtime `NotImplementedError` paths from public APIs unless the API is explicitly abstract.

Acceptance criteria:

- Incomplete features never report successful execution.
- Experimental features are documented and isolated.
- Tests cover unsupported feature errors.

---

### Phase 6: Documentation Rebuild

Rewrite docs around product truth instead of aspiration.

Recommended structure:

| Document | Purpose |
|----------|---------|
| Root README | Explain project, product split, and links to SDK/CLI docs |
| Python SDK README | Install, quick start, stable API, providers, tools, session, security, RAG |
| CLI README | Install, config, run, TUI, session/checkpoint, supported providers |
| API Reference | Generated or contract-tested API documentation |
| Security Guide | Actual enforced security model and configuration |
| Provider Matrix | Generated from provider registry |
| Benchmark Report | Only reproducible measured data |

Documentation rules:

- No unverifiable competitor claims.
- No unverifiable performance numbers.
- No future model names unless explicitly marked as examples.
- No code snippets using nonexistent APIs.
- No unsafe examples such as `eval` in public docs.

Acceptance criteria:

- README snippets are smoke-tested.
- Public API docs match contract tests.
- SDK and CLI docs do not make conflicting claims.

---

### Phase 7: Quality Gates

Add automated checks that prevent drift from returning.

Recommended checks:

- Python import smoke tests.
- CLI `--help` and config smoke tests.
- README code snippet tests.
- Provider registry consistency tests.
- Security boundary tests for file tools.
- Shell policy tests.
- Packaging smoke tests for Rust core mode and pure Python fallback mode.
- Test coverage review for mock-only assertions.

Acceptance criteria:

- CI catches broken public examples.
- CI catches provider/default drift.
- CI catches accidental placeholder success paths.

---

## Review Findings to Track

The current full review identified these priority areas:

### P1

- Python README and public API mismatch.
- Python/Rust/PyO3 module naming mismatch.
- Root Python console script points to nonexistent `continuum.cli:main`.
- CLI provider support is narrower than Python SDK provider claims.
- Security/sandbox wording is stronger than enforced tool behavior.

### P2

- Security examples need correct result handling.
- Provider/model/base URL defaults drift across Python, CLI, Rust, and docs.
- CLI install crate name is inconsistent across docs and Cargo metadata.
- Some root docs still use competitor positioning or unverifiable product claims.
- Benchmark docs need reproducible evidence.
- Placeholder success paths exist in plugin/WASM/sandbox/search areas.
- Some tests contain mock-only or weak assertions.

### P3

- Python package version differs between `continuum` and `continuum_sdk`.
- CLI config loader has simplified empty implementations.
- Draft docs contain unsafe `eval` examples.
- TUI config detection uses fragile string matching.

---

## Recommended First Work Batch

Start with the highest leverage foundation work:

1. Fix public Python API and README consistency.
2. Decide pure Python package with Rust core as primary extension packaging strategy.
3. Resolve PyO3 module naming and import fallback.
4. Fix Python console script / CLI crate naming ambiguity.
5. Add smoke tests for SDK import, README quick start, and CLI `--help`.

This batch makes the project easier to reason about and prevents later provider, security, and documentation work from being built on unstable entry points.
