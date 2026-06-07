//! Plugin Capability-Based Security Model
//!
//! Defines permissions for plugin operations.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Plugin capability/permission
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// File system read access
    FsRead,
    /// File system write access
    FsWrite,
    /// Network outbound access
    NetworkOut,
    /// Process execution
    ProcessExec,
    /// Environment variable read
    EnvRead,
    /// Environment variable write
    EnvWrite,
    /// Access to host clock
    Clock,
    /// Random number generation
    Random,
    /// Memory limit in bytes
    MemoryLimit(u64),
    /// CPU time limit in milliseconds
    CpuLimit(u64),
}

/// Capability set with allow/deny lists
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    /// Allowed capabilities
    pub allowed: HashSet<Capability>,
    /// Explicitly denied (overrides allowed)
    pub denied: HashSet<Capability>,
}

impl CapabilitySet {
    /// Create empty capability set (no permissions)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create unrestricted capability set (all permissions)
    pub fn unrestricted() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(Capability::FsRead);
        allowed.insert(Capability::FsWrite);
        allowed.insert(Capability::NetworkOut);
        allowed.insert(Capability::ProcessExec);
        allowed.insert(Capability::EnvRead);
        allowed.insert(Capability::EnvWrite);
        allowed.insert(Capability::Clock);
        allowed.insert(Capability::Random);
        Self {
            allowed,
            denied: HashSet::new(),
        }
    }

    /// Create sandboxed capability set (minimal permissions)
    pub fn sandboxed() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(Capability::Clock);
        allowed.insert(Capability::Random);
        allowed.insert(Capability::MemoryLimit(16 * 1024 * 1024)); // 16MB
        allowed.insert(Capability::CpuLimit(5000)); // 5 seconds
        Self {
            allowed,
            denied: HashSet::new(),
        }
    }

    /// Add a capability to allowed set
    pub fn allow(&mut self, cap: Capability) -> &mut Self {
        self.allowed.insert(cap.clone());
        self.denied.remove(&cap);
        self
    }

    /// Add a capability to denied set
    pub fn deny(&mut self, cap: Capability) -> &mut Self {
        self.denied.insert(cap.clone());
        self.allowed.remove(&cap);
        self
    }

    /// Check if capability is granted
    pub fn check(&self, cap: &Capability) -> bool {
        self.allowed.contains(cap) && !self.denied.contains(cap)
    }

    /// Merge with another capability set
    pub fn merge(&mut self, other: &CapabilitySet) -> &mut Self {
        for cap in &other.allowed {
            if !self.denied.contains(cap) {
                self.allowed.insert(cap.clone());
            }
        }
        for cap in &other.denied {
            self.allowed.remove(cap);
            self.denied.insert(cap.clone());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_capability_set() {
        let caps = CapabilitySet::new();
        assert!(!caps.check(&Capability::FsRead));
        assert!(!caps.check(&Capability::NetworkOut));
    }

    #[test]
    fn test_unrestricted_capability_set() {
        let caps = CapabilitySet::unrestricted();
        assert!(caps.check(&Capability::FsRead));
        assert!(caps.check(&Capability::NetworkOut));
        assert!(caps.check(&Capability::ProcessExec));
    }

    #[test]
    fn test_sandboxed_capability_set() {
        let caps = CapabilitySet::sandboxed();
        assert!(!caps.check(&Capability::FsRead));
        assert!(!caps.check(&Capability::FsWrite));
        assert!(caps.check(&Capability::Clock));
        assert!(caps.check(&Capability::Random));
    }

    #[test]
    fn test_deny_overrides_allow() {
        let mut caps = CapabilitySet::unrestricted();
        caps.deny(Capability::FsWrite);
        assert!(!caps.check(&Capability::FsWrite));
        assert!(caps.check(&Capability::FsRead));
    }

    #[test]
    fn test_capability_merge() {
        let mut caps1 = CapabilitySet::new();
        caps1.allow(Capability::FsRead);

        let caps2 = CapabilitySet::sandboxed();

        caps1.merge(&caps2);
        assert!(caps1.check(&Capability::FsRead));
        assert!(caps1.check(&Capability::Clock));
    }
}
