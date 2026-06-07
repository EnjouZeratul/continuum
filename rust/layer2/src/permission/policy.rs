//! # Permission Policy
//!
//! Security policy configuration for the permission system.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Security level for permission policies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SecurityLevel {
    /// Trust everything - no permission prompts
    Trusted,
    /// Default - prompt for potentially dangerous actions
    #[default]
    Standard,
    /// Strict - prompt for all actions
    Strict,
    /// Paranoid - prompt for all actions and log everything
    Paranoid,
}

/// Rule for allowing or denying actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Pattern to match (glob pattern)
    pub pattern: String,
    /// Whether to allow or deny
    pub allow: bool,
    /// Optional description
    pub description: Option<String>,
}

impl PermissionRule {
    /// Create an allow rule
    pub fn allow(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            allow: true,
            description: None,
        }
    }

    /// Create a deny rule
    pub fn deny(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            allow: false,
            description: None,
        }
    }

    /// Add description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Check if a value matches this rule's pattern
    pub fn matches(&self, value: &str) -> bool {
        // Simple glob matching: * matches anything, ? matches single char
        let pattern_parts: Vec<&str> = self.pattern.split('*').collect();
        if pattern_parts.len() == 1 {
            return value == self.pattern;
        }

        // Check prefix and suffix
        if !value.starts_with(pattern_parts[0]) {
            return false;
        }
        if !value.ends_with(pattern_parts.last().unwrap()) {
            return false;
        }

        // Check middle parts in order
        let mut search_start = pattern_parts[0].len();
        for part in pattern_parts.iter().skip(1).take(pattern_parts.len() - 2) {
            if let Some(pos) = value[search_start..].find(part) {
                search_start += pos + part.len();
            } else {
                return false;
            }
        }

        true
    }
}

/// Permission policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// Security level
    pub level: SecurityLevel,
    /// List of allowed rules
    pub allow_rules: Vec<PermissionRule>,
    /// List of denied rules
    pub deny_rules: Vec<PermissionRule>,
    /// Categories that are always allowed
    pub trusted_categories: HashSet<String>,
    /// Categories that are always denied
    pub blocked_categories: HashSet<String>,
    /// Paths that are trusted (read/write without prompts)
    pub trusted_paths: HashSet<String>,
    /// Paths that are blocked (never allowed)
    pub blocked_paths: HashSet<String>,
    /// URLs that are trusted (network requests without prompts)
    pub trusted_urls: HashSet<String>,
    /// URLs that are blocked
    pub blocked_urls: HashSet<String>,
    /// Commands that are trusted
    pub trusted_commands: HashSet<String>,
    /// Commands that are blocked
    pub blocked_commands: HashSet<String>,
    /// Whether to cache permission decisions
    pub enable_cache: bool,
    /// Cache expiration time in seconds
    pub cache_expire_seconds: u64,
    /// Whether to audit log all decisions
    pub audit_enabled: bool,
    /// Maximum audit entries to keep
    pub max_audit_entries: usize,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            level: SecurityLevel::Standard,
            allow_rules: Vec::new(),
            deny_rules: Vec::new(),
            trusted_categories: HashSet::new(),
            blocked_categories: HashSet::new(),
            trusted_paths: HashSet::new(),
            blocked_paths: Self::default_blocked_paths(),
            trusted_urls: HashSet::new(),
            blocked_urls: HashSet::new(),
            trusted_commands: HashSet::new(),
            blocked_commands: Self::default_blocked_commands(),
            enable_cache: true,
            cache_expire_seconds: 3600, // 1 hour
            audit_enabled: true,
            max_audit_entries: 10000,
        }
    }
}

impl PermissionPolicy {
    /// Create a trusted policy (no prompts)
    pub fn trusted() -> Self {
        Self {
            level: SecurityLevel::Trusted,
            ..Self::default()
        }
    }

    /// Create a strict policy (prompt for everything)
    pub fn strict() -> Self {
        Self {
            level: SecurityLevel::Strict,
            ..Self::default()
        }
    }

    /// Create a paranoid policy (prompt for everything, log everything)
    pub fn paranoid() -> Self {
        Self {
            level: SecurityLevel::Paranoid,
            audit_enabled: true,
            ..Self::strict()
        }
    }

    /// Default blocked paths (sensitive files)
    fn default_blocked_paths() -> HashSet<String> {
        let mut paths = HashSet::new();
        // Environment files
        paths.insert(".env".to_string());
        paths.insert(".env.local".to_string());
        paths.insert(".env.*.local".to_string());
        // Credentials
        paths.insert("**/credentials.json".to_string());
        paths.insert("**/secrets.json".to_string());
        paths.insert("**/api_keys.json".to_string());
        // SSH keys
        paths.insert("~/.ssh/id_rsa".to_string());
        paths.insert("~/.ssh/id_ed25519".to_string());
        // System sensitive files
        paths.insert("/etc/shadow".to_string());
        paths.insert("/etc/passwd".to_string());
        paths
    }

    /// Default blocked commands
    fn default_blocked_commands() -> HashSet<String> {
        let mut commands = HashSet::new();
        // Dangerous commands
        commands.insert("rm -rf /".to_string());
        commands.insert("rm -rf ~".to_string());
        commands.insert("mkfs".to_string());
        commands.insert("dd if=/dev/zero".to_string());
        commands.insert(":(){ :|:& };:".to_string()); // Fork bomb
        commands.insert("chmod 777".to_string());
        commands
    }

    /// Add a trusted path
    pub fn add_trusted_path(mut self, path: impl Into<String>) -> Self {
        self.trusted_paths.insert(path.into());
        self
    }

    /// Add a blocked path
    pub fn add_blocked_path(mut self, path: impl Into<String>) -> Self {
        self.blocked_paths.insert(path.into());
        self
    }

    /// Add a trusted URL
    pub fn add_trusted_url(mut self, url: impl Into<String>) -> Self {
        self.trusted_urls.insert(url.into());
        self
    }

    /// Add a blocked URL
    pub fn add_blocked_url(mut self, url: impl Into<String>) -> Self {
        self.blocked_urls.insert(url.into());
        self
    }

    /// Add a trusted command
    pub fn add_trusted_command(mut self, command: impl Into<String>) -> Self {
        self.trusted_commands.insert(command.into());
        self
    }

    /// Add a blocked command
    pub fn add_blocked_command(mut self, command: impl Into<String>) -> Self {
        self.blocked_commands.insert(command.into());
        self
    }

    /// Check if a path is trusted
    pub fn is_path_trusted(&self, path: &str) -> bool {
        self.trusted_paths
            .iter()
            .any(|p| path.starts_with(p) || PermissionRule::allow(p.clone()).matches(path))
    }

    /// Check if a path is blocked
    pub fn is_path_blocked(&self, path: &str) -> bool {
        self.blocked_paths
            .iter()
            .any(|p| path.starts_with(p) || PermissionRule::deny(p.clone()).matches(path))
    }

    /// Check if an action should be auto-approved based on security level
    pub fn should_auto_approve(&self, action_category: &str) -> bool {
        match self.level {
            SecurityLevel::Trusted => true,
            SecurityLevel::Standard => self.trusted_categories.contains(action_category),
            SecurityLevel::Strict | SecurityLevel::Paranoid => false,
        }
    }

    /// Check if a category is blocked
    pub fn is_category_blocked(&self, category: &str) -> bool {
        self.blocked_categories.contains(category)
    }

    /// Load policy from a TOML file
    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let policy: Self = toml::from_str(&content)?;
        Ok(policy)
    }

    /// Save policy to a TOML file
    pub fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_rule_matching() {
        let rule = PermissionRule::allow("/home/user/*.txt");
        assert!(rule.matches("/home/user/test.txt"));
        assert!(rule.matches("/home/user/another.txt"));
        assert!(!rule.matches("/home/other/test.txt"));
    }

    #[test]
    fn test_default_blocked_paths() {
        let policy = PermissionPolicy::default();
        assert!(policy.is_path_blocked(".env"));
        assert!(policy.is_path_blocked("/etc/shadow"));
    }

    #[test]
    fn test_security_level_auto_approve() {
        let trusted_policy = PermissionPolicy::trusted();
        assert!(trusted_policy.should_auto_approve("file_read"));

        let strict_policy = PermissionPolicy::strict();
        assert!(!strict_policy.should_auto_approve("file_read"));
    }
}
