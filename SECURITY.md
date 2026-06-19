# Security Policy

## Supported Versions

Continuum is pre-1.0. Security fixes are applied to the latest release only.

| Version | Supported |
|---------|-----------|
| latest (1.x) | ✅ |
| < latest | ❌ |

## Reporting a Vulnerability

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please report via one of:

1. **GitHub Security Advisory** (preferred): Use the "Report a vulnerability" button under the Security tab.
2. **Email**: Open an issue requesting a security contact (we'll provide a private channel).

### What to include

- Description of the vulnerability
- Affected component (tool name, module path)
- Steps to reproduce / proof of concept
- Potential impact
- Suggested fix (optional)

### Response SLA

| Stage | Target |
|-------|--------|
| Acknowledgment | 48 hours |
| Initial assessment | 7 days |
| Fix or mitigation | 30 days (severity-dependent) |
| Public disclosure | After fix released, coordinated with reporter |

## Security Model

Continuum enforces security **at the tool layer** (not relying solely on external sandbox). See [`docs/SECURITY_INVARIANTS.md`](./docs/SECURITY_INVARIANTS.md) for the complete list of security invariants.

Key design principle: **tools reject dangerous operations before execution**, enabling safe operation even without an external sandbox (server-side / embedded deployment scenarios).

### Hardened tools

- `BashTool`: command denylist, output size cap, binary detection
- `ReadFileTool` / `WriteFileTool` / `EditFileTool`: size limits, binary detection, stale-read prevention, uniqueness checks
- `DeleteFileTool`: critical-path detection, symlink rejection, dry-run mode
- `HttpRequestTool` / `WebFetchTool`: SSRF protection (OWASP-aligned)
- `SetEnvTool`: dangerous env var denylist (LD_PRELOAD, GIT_DIR, etc.)
- `GetEnvTool` / `ListEnvTool`: secret scrubbing (8+ patterns)

### Dependency security

- `cargo-deny` runs in CI (`security-audit` job) + locally (`cargo deny check`)
- Known advisories tracked in `deny.toml`
- Report found via local audit (RUSTSEC-2026-0176/0177) are documented in `docs/METRICS.md`

## Known Limitations

- **Fuzz testing**: infrastructure ready but execution requires Linux/WSL (Windows ASan limitation)
- **Wasmtime 25.x**: project tracks 20+ known wasmtime advisories (sandboxed execution context, see `deny.toml`)
- **Cross-process sandbox**: not provided (use OS-level isolation for multi-tenant)

## Acknowledgments

Security researchers and contributors who report vulnerabilities responsibly will be acknowledged here (with permission).
