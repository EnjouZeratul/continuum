//! Plugin Loader Integration Tests

use sh_layer4::plugin_loader::{Capability, CapabilitySet, PluginLoader, PluginSandbox};
use std::path::Path;

#[tokio::test]
async fn test_plugin_loader_creation() {
    let loader = PluginLoader::with_default_dir();
    assert_eq!(loader.count(), 0);
}

#[tokio::test]
async fn test_sandbox_capabilities() {
    let sandbox = PluginSandbox::sandboxed();
    assert!(!sandbox.capabilities().check(&Capability::FsRead));
    assert!(sandbox.capabilities().check(&Capability::Clock));
}

#[tokio::test]
async fn test_unrestricted_sandbox() {
    let sandbox = PluginSandbox::unrestricted();
    assert!(sandbox.check_fs_read(Path::new("/tmp/test")).is_ok());
    assert!(sandbox.check_fs_write(Path::new("/tmp/test")).is_ok());
}

#[tokio::test]
async fn test_capability_set_builder() {
    let mut caps = CapabilitySet::new();
    caps.allow(Capability::FsRead);
    assert!(caps.check(&Capability::FsRead));
    assert!(!caps.check(&Capability::FsWrite));
}

#[tokio::test]
async fn test_capability_deny_override() {
    let mut caps = CapabilitySet::unrestricted();
    caps.deny(Capability::ProcessExec);
    assert!(!caps.check(&Capability::ProcessExec));
    assert!(caps.check(&Capability::FsRead));
}

#[test]
fn test_plugin_meta_default() {
    use sh_layer4::plugin_loader::PluginMeta;
    let meta = PluginMeta::default();
    assert_eq!(meta.name, "unknown");
    assert_eq!(meta.version, "0.1.0");
}

#[test]
fn test_plugin_registry_empty() {
    use sh_layer4::plugin_loader::PluginRegistry;
    let registry = PluginRegistry::new();
    assert_eq!(registry.count(), 0);
    assert!(registry.list().is_empty());
}

#[test]
fn test_capability_memory_limit() {
    let caps = CapabilitySet::sandboxed();
    // Check that memory limit is set
    let has_memory_limit = caps
        .allowed
        .iter()
        .any(|c| matches!(c, Capability::MemoryLimit(_)));
    assert!(has_memory_limit);
}

#[test]
fn test_capability_cpu_limit() {
    let caps = CapabilitySet::sandboxed();
    // Check that CPU limit is set
    let has_cpu_limit = caps
        .allowed
        .iter()
        .any(|c| matches!(c, Capability::CpuLimit(_)));
    assert!(has_cpu_limit);
}
