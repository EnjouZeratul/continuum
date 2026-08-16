//! Self-modification policy — the guardrail layer for agent self-evolution.
//!
//! Every autonomous action an agent takes against its own configuration
//! (installing tools, saving skills, modifying core files) must pass through
//! a [`SelfModificationPolicy`] check first. Decisions are three-valued:
//! allow, deny, or require human approval — mirroring the pre-execution
//! policy gate pattern used by hardened agent harnesses.
//!
//! Pure validation only: no I/O, no async, deterministic — fuzzable.
//!
//! ## Threat model
//!
//! - LLM-generated tool names are hostile input (path separators, control
//!   chars, lookalike overrides of builtin tools)
//! - Unbounded tool installation is a resource-exhaustion vector
//! - Modifying the runtime that enforces the sandbox is privilege escalation

/// Maximum accepted length for a dynamic tool name.
pub const MAX_TOOL_NAME_LEN: usize = 64;

/// Default cap on concurrently installed dynamic tools.
pub const DEFAULT_MAX_DYNAMIC_TOOLS: usize = 64;

/// The action an agent requests against its own configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfModAction<'a> {
    /// Install a new dynamic tool with the given name.
    InstallTool { name: &'a str },
    /// Remove a dynamic tool by name.
    UninstallTool { name: &'a str },
    /// Overwrite an existing tool's implementation.
    ReplaceTool { name: &'a str },
    /// Persist a learned skill.
    SaveSkill,
    /// Write to a file under the agent's own core/config directory.
    ModifyCoreFile { path: &'a str },
}

/// The policy decision for a requested action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfModDecision {
    /// Action is permitted.
    Allow,
    /// Action is permanently refused; the reason explains why.
    Deny(&'static str),
    /// Action may proceed only after a human approves it.
    RequiresApproval(&'static str),
}

/// Policy governing what the agent may do to itself.
///
/// Defaults are the safest configuration that still permits tool creation:
/// tool installation allowed, approval required, core modification denied.
///
/// Deliberately has no serde derives: this crate stays dependency-minimal
/// for fast fuzz builds. Persistence/serialization lives in the config
/// layer that consumes this type.
#[derive(Debug, Clone)]
pub struct SelfModificationPolicy {
    /// Allow the agent to create or replace its own tools at all.
    pub allow_tool_creation: bool,
    /// New tools need explicit human approval before activation.
    pub require_approval_for_new_tools: bool,
    /// Allow writes to the agent's core/registry/policy files.
    /// Default `false` — this is the sandbox-escape vector.
    pub allow_core_modification: bool,
    /// Upper bound on installed dynamic tools (resource-exhaustion guard).
    pub max_dynamic_tools: usize,
    /// Names that dynamic tools must never shadow or remove.
    pub protected_tool_names: Vec<String>,
}

impl Default for SelfModificationPolicy {
    fn default() -> Self {
        Self {
            allow_tool_creation: true,
            require_approval_for_new_tools: true,
            allow_core_modification: false,
            max_dynamic_tools: DEFAULT_MAX_DYNAMIC_TOOLS,
            protected_tool_names: Vec::new(),
        }
    }
}

impl SelfModificationPolicy {
    /// Build the policy used in production: creation allowed, approval
    /// required, core locked, builtin tools protected.
    pub fn safe_default() -> Self {
        Self {
            protected_tool_names: DEFAULT_PROTECTED_TOOLS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..Self::default()
        }
    }

    /// Fully locked policy: nothing may self-modify. Used as a kill switch.
    pub fn locked() -> Self {
        Self {
            allow_tool_creation: false,
            require_approval_for_new_tools: true,
            allow_core_modification: false,
            max_dynamic_tools: 0,
            protected_tool_names: DEFAULT_PROTECTED_TOOLS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Decide whether `action` is permitted when `current_dynamic_tools`
    /// tools are already installed.
    pub fn decide(
        &self,
        action: &SelfModAction<'_>,
        current_dynamic_tools: usize,
    ) -> SelfModDecision {
        use SelfModAction::*;
        use SelfModDecision::*;

        // Core modification is checked first: it gates the policy itself.
        if let ModifyCoreFile { .. } = action {
            return if self.allow_core_modification {
                RequiresApproval("core modification always requires approval")
            } else {
                Deny("core modification is disabled by policy")
            };
        }

        match action {
            SaveSkill => Allow,

            InstallTool { name } => {
                if !self.allow_tool_creation {
                    return Deny("tool creation is disabled by policy");
                }
                if let Err(reason) = validate_tool_name(name) {
                    return Deny(reason);
                }
                if self.is_protected(name) {
                    return Deny("refusing to shadow a protected/builtin tool name");
                }
                if current_dynamic_tools >= self.max_dynamic_tools {
                    return Deny("dynamic tool count limit reached");
                }
                if self.require_approval_for_new_tools {
                    RequiresApproval("new tool installation requires approval")
                } else {
                    Allow
                }
            }

            ReplaceTool { name } => {
                if !self.allow_tool_creation {
                    return Deny("tool replacement is disabled by policy");
                }
                if let Err(reason) = validate_tool_name(name) {
                    return Deny(reason);
                }
                if self.is_protected(name) {
                    return Deny("refusing to replace a protected/builtin tool");
                }
                RequiresApproval("replacing an existing tool requires approval")
            }

            UninstallTool { name } => {
                if self.is_protected(name) {
                    return Deny("refusing to uninstall a protected/builtin tool");
                }
                Allow
            }

            ModifyCoreFile { .. } => unreachable!("handled above"),
        }
    }

    fn is_protected(&self, name: &str) -> bool {
        self.protected_tool_names
            .iter()
            .any(|p| p.eq_ignore_ascii_case(name))
    }
}

/// Tool names that must never be shadowed — the safety-critical builtins.
pub const DEFAULT_PROTECTED_TOOLS: &[&str] = &[
    "install_capability",
    "uninstall_capability",
    "list_dynamic_tools",
    "save_skill",
    "run_skill",
    "list_skills",
    "bash",
    "write_file",
    "read_file",
];

/// Validate an LLM-provided tool name. Names are hostile input:
/// reject path separators, traversal, control characters, whitespace,
/// reserved words, and overlong names.
pub fn validate_tool_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("tool name must not be empty");
    }
    if name.len() > MAX_TOOL_NAME_LEN {
        return Err("tool name exceeds maximum length");
    }
    if name != name.trim() {
        return Err("tool name must not have leading/trailing whitespace");
    }
    // Single characters must be alphanumeric; the rest allow _ - . but
    // never as the first character (avoids hidden/dotfiles and dashes
    // that collide with CLI flags).
    let mut chars = name.chars();
    let first = chars.next().expect("non-empty checked above");
    if !first.is_ascii_alphanumeric() && first != '_' {
        return Err("tool name must start with alphanumeric or underscore");
    }
    // Allow only [A-Za-z0-9_.-]. This simultaneously rejects path
    // separators, shell metacharacters, control chars, and non-ASCII.
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return Err("tool name contains disallowed characters");
        }
    }
    if name.contains("..") {
        return Err("tool name must not contain path traversal");
    }
    for reserved in ["con", "prn", "aux", "nul"] {
        if name.eq_ignore_ascii_case(reserved) {
            return Err("tool name collides with a reserved device name");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn safe() -> SelfModificationPolicy {
        SelfModificationPolicy::safe_default()
    }

    #[test]
    fn default_policy_allows_skill_saving() {
        let d = safe().decide(&SelfModAction::SaveSkill, 0);
        assert_eq!(d, SelfModDecision::Allow);
    }

    #[test]
    fn new_tool_requires_approval_by_default() {
        let d = safe().decide(&SelfModAction::InstallTool { name: "calc" }, 0);
        assert!(matches!(d, SelfModDecision::RequiresApproval(_)));
    }

    #[test]
    fn no_approval_mode_allows_clean_install() {
        let mut p = safe();
        p.require_approval_for_new_tools = false;
        let d = p.decide(&SelfModAction::InstallTool { name: "calc" }, 0);
        assert_eq!(d, SelfModDecision::Allow);
    }

    #[test]
    fn tool_creation_disabled_denies_everything() {
        let mut p = safe();
        p.allow_tool_creation = false;
        let d = p.decide(&SelfModAction::InstallTool { name: "calc" }, 0);
        assert!(matches!(d, SelfModDecision::Deny(_)));
    }

    #[test]
    fn dynamic_tool_cap_is_enforced() {
        let mut p = safe();
        p.require_approval_for_new_tools = false;
        p.max_dynamic_tools = 2;
        assert_eq!(
            p.decide(&SelfModAction::InstallTool { name: "a1" }, 1),
            SelfModDecision::Allow
        );
        assert!(matches!(
            p.decide(&SelfModAction::InstallTool { name: "a2" }, 2),
            SelfModDecision::Deny("dynamic tool count limit reached")
        ));
    }

    #[test]
    fn protected_builtin_cannot_be_shadowed() {
        let d = safe().decide(&SelfModAction::InstallTool { name: "bash" }, 0);
        assert!(matches!(d, SelfModDecision::Deny(_)));
    }

    #[test]
    fn protected_builtin_cannot_be_shadowed_case_insensitive() {
        let d = safe().decide(&SelfModAction::InstallTool { name: "Bash" }, 0);
        assert!(matches!(d, SelfModDecision::Deny(_)));
    }

    #[test]
    fn protected_builtin_cannot_be_replaced_or_uninstalled() {
        assert!(matches!(
            safe().decide(&SelfModAction::ReplaceTool { name: "write_file" }, 0),
            SelfModDecision::Deny(_)
        ));
        assert!(matches!(
            safe().decide(&SelfModAction::UninstallTool { name: "bash" }, 0),
            SelfModDecision::Deny(_)
        ));
    }

    #[test]
    fn core_modification_denied_by_default() {
        let d = safe().decide(
            &SelfModAction::ModifyCoreFile {
                path: ".continuum/policy/config.toml",
            },
            0,
        );
        assert_eq!(
            d,
            SelfModDecision::Deny("core modification is disabled by policy")
        );
    }

    #[test]
    fn core_modification_even_when_enabled_requires_approval() {
        let mut p = safe();
        p.allow_core_modification = true;
        let d = p.decide(&SelfModAction::ModifyCoreFile { path: "x" }, 0);
        assert!(matches!(d, SelfModDecision::RequiresApproval(_)));
    }

    #[test]
    fn locked_policy_denies_all_tool_actions() {
        let p = SelfModificationPolicy::locked();
        assert!(matches!(
            p.decide(&SelfModAction::InstallTool { name: "ok_name" }, 0),
            SelfModDecision::Deny(_)
        ));
        assert!(matches!(
            p.decide(&SelfModAction::ModifyCoreFile { path: "x" }, 0),
            SelfModDecision::Deny(_)
        ));
    }

    #[test]
    fn replace_tool_always_requires_approval() {
        let mut p = safe();
        p.require_approval_for_new_tools = false;
        let d = p.decide(&SelfModAction::ReplaceTool { name: "my_tool" }, 1);
        assert!(matches!(d, SelfModDecision::RequiresApproval(_)));
    }

    // ---- name validation ----

    #[test]
    fn valid_names_pass() {
        for name in [
            "calc", "fetch_url", "parse-csv", "my.tool", "_private", "tool2", "A1_b-C.d",
        ] {
            assert_eq!(validate_tool_name(name), Ok(()), "should accept {name}");
        }
    }

    #[test]
    fn path_traversal_rejected() {
        assert!(validate_tool_name("../etc/passwd").is_err());
        assert!(validate_tool_name("a..b").is_err());
        assert!(validate_tool_name("sub/dir").is_err());
        assert!(validate_tool_name("win\\path").is_err());
    }

    #[test]
    fn control_and_meta_characters_rejected() {
        assert!(validate_tool_name("bad name").is_err()); // whitespace inside
        assert!(validate_tool_name("evil\n").is_err());
        assert!(validate_tool_name("evil\u{0}x").is_err());
        assert!(validate_tool_name("*").is_err());
        assert!(validate_tool_name("|pipe").is_err());
        assert!(validate_tool_name("--flag").is_err()); // starts with dash
        assert!(validate_tool_name(".hidden").is_err()); // starts with dot
    }

    #[test]
    fn empty_and_overlong_names_rejected() {
        assert!(validate_tool_name("").is_err());
        assert!(validate_tool_name("   ").is_err());
        assert!(validate_tool_name(&"x".repeat(MAX_TOOL_NAME_LEN + 1)).is_err());
        assert_eq!(validate_tool_name(&"x".repeat(MAX_TOOL_NAME_LEN)), Ok(()));
    }

    #[test]
    fn windows_reserved_names_rejected() {
        assert!(validate_tool_name("con").is_err());
        assert!(validate_tool_name("NUL").is_err());
        assert!(validate_tool_name("aux").is_err());
    }

    #[test]
    fn unicode_letters_rejected_for_portability() {
        // Non-ASCII letters are rejected: tool names become file names and
        // shell-facing identifiers, so ASCII-only is the portable subset.
        assert!(validate_tool_name("工具").is_err());
        assert!(validate_tool_name("café").is_err());
    }
}
