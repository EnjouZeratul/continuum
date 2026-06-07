//! Plugin Security Sandbox
//!
//! Enforces capability restrictions on plugin operations.

use super::capabilities::{Capability, CapabilitySet};
use crate::types::Layer4Result;
use anyhow::anyhow;
use std::path::Path;
use std::time::Instant;

/// Security sandbox for plugin execution
#[derive(Clone)]
pub struct PluginSandbox {
    /// Capability set for this sandbox
    capabilities: CapabilitySet,
    /// Execution start time (for CPU limit)
    start_time: Option<Instant>,
    /// Memory usage tracker
    memory_used: u64,
}

impl PluginSandbox {
    /// Create new sandbox with given capabilities
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self {
            capabilities,
            start_time: None,
            memory_used: 0,
        }
    }

    /// Create unrestricted sandbox (use with caution)
    pub fn unrestricted() -> Self {
        Self::new(CapabilitySet::unrestricted())
    }

    /// Create sandboxed environment
    pub fn sandboxed() -> Self {
        Self::new(CapabilitySet::sandboxed())
    }

    /// Start execution timer
    pub fn start_execution(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// Check if execution is within CPU limit
    pub fn check_cpu_limit(&self) -> Layer4Result<()> {
        let cpu_limit = self.get_cpu_limit();
        if cpu_limit == 0 {
            return Ok(());
        }

        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_millis() as u64;
            if elapsed > cpu_limit {
                return Err(anyhow!(
                    "CPU time limit exceeded: {}ms > {}ms",
                    elapsed,
                    cpu_limit
                ));
            }
        }
        Ok(())
    }

    /// Track memory allocation
    pub fn track_memory(&mut self, size: u64) -> Layer4Result<()> {
        let memory_limit = self.get_memory_limit();
        if memory_limit == 0 {
            self.memory_used += size;
            return Ok(());
        }

        if self.memory_used + size > memory_limit {
            return Err(anyhow!(
                "Memory limit exceeded: {} + {} > {}",
                self.memory_used,
                size,
                memory_limit
            ));
        }
        self.memory_used += size;
        Ok(())
    }

    /// Check file read permission
    pub fn check_fs_read(&self, path: &Path) -> Layer4Result<()> {
        if !self.capabilities.check(&Capability::FsRead) {
            return Err(anyhow!("File read denied: {:?}", path));
        }
        Ok(())
    }

    /// Check file write permission
    pub fn check_fs_write(&self, path: &Path) -> Layer4Result<()> {
        if !self.capabilities.check(&Capability::FsWrite) {
            return Err(anyhow!("File write denied: {:?}", path));
        }
        Ok(())
    }

    /// Check network access permission
    pub fn check_network(&self, url: &str) -> Layer4Result<()> {
        if !self.capabilities.check(&Capability::NetworkOut) {
            return Err(anyhow!("Network access denied: {}", url));
        }
        Ok(())
    }

    /// Check process execution permission
    pub fn check_process(&self, cmd: &str) -> Layer4Result<()> {
        if !self.capabilities.check(&Capability::ProcessExec) {
            return Err(anyhow!("Process execution denied: {}", cmd));
        }
        Ok(())
    }

    /// Get memory limit in bytes (0 = unlimited)
    fn get_memory_limit(&self) -> u64 {
        for cap in &self.capabilities.allowed {
            if let Capability::MemoryLimit(limit) = cap {
                return *limit;
            }
        }
        0
    }

    /// Get CPU limit in milliseconds (0 = unlimited)
    fn get_cpu_limit(&self) -> u64 {
        for cap in &self.capabilities.allowed {
            if let Capability::CpuLimit(limit) = cap {
                return *limit;
            }
        }
        0
    }

    /// Reset execution state
    pub fn reset(&mut self) {
        self.start_time = None;
        self.memory_used = 0;
    }

    /// Get capabilities reference
    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let sandbox = PluginSandbox::sandboxed();
        assert!(sandbox.capabilities().check(&Capability::Clock));
    }

    #[test]
    fn test_fs_read_check() {
        let sandbox = PluginSandbox::sandboxed();
        let result = sandbox.check_fs_read(Path::new("/etc/passwd"));
        assert!(result.is_err());
    }

    #[test]
    fn test_unrestricted_sandbox() {
        let sandbox = PluginSandbox::unrestricted();
        assert!(sandbox.check_fs_read(Path::new("/tmp/test")).is_ok());
        assert!(sandbox.check_fs_write(Path::new("/tmp/test")).is_ok());
        assert!(sandbox.check_network("https://example.com").is_ok());
    }

    #[test]
    fn test_cpu_limit_check() {
        let mut sandbox = PluginSandbox::sandboxed();
        sandbox.start_execution();
        // Should pass immediately
        assert!(sandbox.check_cpu_limit().is_ok());
    }

    #[test]
    fn test_memory_tracking() {
        let mut sandbox = PluginSandbox::sandboxed();
        assert!(sandbox.track_memory(1024).is_ok());
        assert!(sandbox.track_memory(1024 * 1024).is_ok());
    }
}
