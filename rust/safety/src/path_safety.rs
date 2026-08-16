//! Critical-path detection for destructive operations.
//!
//! Used by `DeleteFileTool` / `MoveFileTool` / `WriteFileTool` to refuse
//! operations that target system-critical or user-sensitive locations.

use std::path::Path;

/// Result of a path-safety check.
#[derive(Debug, Clone)]
pub struct PathDanger {
    /// True if path is in a system-critical or user-sensitive location.
    pub is_critical: bool,
    /// Human-readable reason (empty if not critical).
    pub reason: &'static str,
}

impl PathDanger {
    /// Non-critical path (safe to operate on).
    pub fn safe() -> Self {
        Self {
            is_critical: false,
            reason: "",
        }
    }
}

/// Check whether `path` is in a system-critical location.
///
/// Critical paths include:
/// - POSIX: `/`, `/etc`, `/usr`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/boot`,
///   `/dev`, `/proc`, `/sys`, `/var/lib/docker`
/// - Windows: `C:\Windows`, `C:\Program Files`
/// - User home (any OS), plus `~/.ssh`, `~/.gnupg`, `~/.config`, `~/.aws`,
///   `~/.kube`
pub fn check_path_danger(path: &Path) -> PathDanger {
    // System-critical roots — checked via Path::starts_with for OS-correct separator handling
    let critical_roots: &[&str] = &[
        "/",
        "/etc",
        "/usr",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/boot",
        "/dev",
        "/proc",
        "/sys",
        "/var/lib/docker",
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\Program Files (x86)",
    ];
    for c in critical_roots {
        let root = Path::new(c);
        if *c == "/" {
            // "/" is prefix of ALL POSIX absolute paths — must use exact match
            // only, otherwise every /tmp, /home, etc. would be flagged critical.
            if path == root {
                return PathDanger {
                    is_critical: true,
                    reason: "system-critical path",
                };
            }
        } else if path == root || path.starts_with(root) {
            return PathDanger {
                is_critical: true,
                reason: "system-critical path",
            };
        }
    }

    // User home directory
    if let Some(home) = dirs::home_dir() {
        if path == home || path.starts_with(&home) {
            // Direct home = critical
            if path == home {
                return PathDanger {
                    is_critical: true,
                    reason: "user home directory",
                };
            }
            // Sensitive subdirectories
            const SENSITIVE_HOME_SUBDIRS: &[&str] = &[".ssh", ".gnupg", ".config", ".aws", ".kube"];
            for sub in SENSITIVE_HOME_SUBDIRS {
                let target = home.join(sub);
                if path == target || path.starts_with(&target) {
                    return PathDanger {
                        is_critical: true,
                        reason: "sensitive user directory",
                    };
                }
            }
            // Other paths inside home are not critical (safe)
        }
    }

    PathDanger::safe()
}

/// Agent-internal directory names whose contents must not be modified by
/// the agent itself (self-modification guardrail). These may appear at any
/// depth — they are matched as component sequences, not string prefixes,
/// so `.continuum/core` matches `D:/proj/.continuum/core/policy.toml`.
const AGENT_CRITICAL_COMPONENTS: &[&[&str]] = &[
    &[".continuum", "core"],
    &[".continuum", "registry"],
    &[".continuum", "policy"],
];

/// Check whether `path` targets the agent's own core configuration.
///
/// This is the path half of the self-modification guardrail: even if a
/// policy allows some self-modification, writes into the directories that
/// *define* the policy are refused. Unlike [`check_path_danger`], this
/// matches relative component sequences anywhere in the path, because
/// agent state directories are project-local.
pub fn check_agent_self_modification(path: &Path) -> PathDanger {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();

    for seq in AGENT_CRITICAL_COMPONENTS {
        if components.windows(seq.len()).any(|window| {
            window
                .iter()
                .zip(seq.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        }) {
            return PathDanger {
                is_critical: true,
                reason: "agent core configuration",
            };
        }
    }

    PathDanger::safe()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_path_is_critical() {
        let d = check_path_danger(Path::new("/"));
        assert!(d.is_critical);
        assert_eq!(d.reason, "system-critical path");
    }

    #[test]
    fn test_etc_subpath_is_critical() {
        let d = check_path_danger(Path::new("/etc/passwd"));
        assert!(d.is_critical);
        assert!(d.reason.contains("system-critical"));
    }

    #[test]
    #[cfg(unix)]
    fn test_user_home_is_critical() {
        let home = dirs::home_dir().unwrap();
        let d = check_path_danger(&home);
        assert!(d.is_critical);
        assert_eq!(d.reason, "user home directory");
    }

    #[test]
    #[cfg(unix)]
    fn test_dot_ssh_is_critical() {
        let target = dirs::home_dir().unwrap().join(".ssh");
        let d = check_path_danger(&target);
        assert!(d.is_critical);
        assert_eq!(d.reason, "sensitive user directory");
    }

    #[test]
    fn test_temp_subdir_is_safe() {
        let temp = std::env::temp_dir().join("continuum_test_safe_path_xyz_unique");
        let d = check_path_danger(&temp);
        assert!(!d.is_critical);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_program_files_critical() {
        let d = check_path_danger(Path::new("C:\\Program Files\\eviltoken"));
        assert!(d.is_critical);
    }

    #[test]
    #[cfg(not(windows))]
    fn test_windows_program_files_critical_skipped() {
        // Windows-only test — on POSIX, the path is treated as relative filename
        let d = check_path_danger(Path::new("C:\\Program Files\\eviltoken"));
        assert!(!d.is_critical); // POSIX treats this as a relative filename
    }

    #[test]
    fn test_safe_returns_non_critical() {
        let d = PathDanger::safe();
        assert!(!d.is_critical);
        assert_eq!(d.reason, "");
    }

    #[test]
    fn test_posix_tmp_not_critical() {
        // Regression: "/" starts_with matched ALL POSIX paths, causing
        // /tmp, /home, /var etc. to be flagged critical on Linux CI.
        // Fix: "/" uses exact match only.
        let d = check_path_danger(Path::new("/tmp"));
        assert!(!d.is_critical, "/tmp should NOT be critical");
        let d = check_path_danger(Path::new("/tmp/continuum_test"));
        assert!(!d.is_critical, "/tmp/xxx should NOT be critical");
        let d = check_path_danger(Path::new("/var/tmp"));
        assert!(!d.is_critical, "/var/tmp should NOT be critical");
        let d = check_path_danger(Path::new("/home/user/project"));
        assert!(!d.is_critical, "/home/user/project should NOT be critical");
    }

    // ---- agent self-modification path guardrail ----

    #[test]
    fn test_agent_core_paths_critical() {
        for p in [
            ".continuum/core/config",
            ".continuum/registry/tools.json",
            ".continuum/policy/self_mod.toml",
            // Absolute, nested, both separators on Windows
            "D:/proj/.continuum/core/x",
            "/home/u/proj/.continuum/policy/y",
        ] {
            let d = check_agent_self_modification(Path::new(p));
            assert!(d.is_critical, "{p} should be agent-critical");
            assert_eq!(d.reason, "agent core configuration");
        }
    }

    #[test]
    fn test_agent_critical_is_case_insensitive() {
        let d = check_agent_self_modification(Path::new(".CONTINUUM/Core/config"));
        assert!(d.is_critical);
    }

    #[test]
    fn test_agent_data_directories_are_not_critical() {
        // Skills, plugins, memory are legitimate agent-writable state —
        // only core/registry/policy are locked.
        for p in [
            ".continuum/skills/parse.json",
            ".continuum/plugins/my_tool.wasm",
            ".continuum/memory/session.json",
            "src/main.rs",
            ".continuum",
        ] {
            let d = check_agent_self_modification(Path::new(p));
            assert!(!d.is_critical, "{p} should NOT be agent-critical");
        }
    }

    #[test]
    fn test_lookalike_component_sequence_not_critical() {
        // `.continuum-core` is a different single component, not the
        // `.continuum/core` sequence — must not be flagged.
        let d = check_agent_self_modification(Path::new(".continuum-core/config"));
        assert!(!d.is_critical);
        let d = check_agent_self_modification(Path::new("x/.continuumcore/y"));
        assert!(!d.is_critical);
    }
}
