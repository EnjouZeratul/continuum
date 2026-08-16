//! # Plugin Loader
//!
//! 插件动态加载和管理系统。
//!
//! 支持两种加载方式:
//! - **dylib**: 动态库加载 (.so/.dylib/.dll)
//! - **wasm**: WebAssembly 模块加载 (沙箱隔离)
//!
//! ## 示例
//!
//! ```rust,ignore
//! use sh_layer4::plugin_loader::{PluginLoader, DylibLoader};
//!
//! // 加载动态库插件
//! let loader = PluginLoader::new("./plugins");
//! let name = loader.load_dylib(Path::new("./plugins/my_plugin.so")).await?;
//!
//! // 初始化并执行
//! loader.initialize(&name, &context).await?;
//! let result = loader.execute(&name, &input).await?;
//! ```

pub mod abi;
pub mod capabilities;
pub mod dylib;
pub mod sandbox;
pub mod tool_adapter;
pub mod wasm;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::types::Layer4Result;
pub use abi::StablePluginMeta;
pub use capabilities::{Capability, CapabilitySet};
pub use dylib::{DylibLoader, PluginCreateFn, PluginDestroyFn, PluginMetaFn};
pub use sandbox::PluginSandbox;
pub use wasm::WasmLoader;

/// 插件接口
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 插件名称
    fn name(&self) -> &str;

    /// 插件版本
    fn version(&self) -> &str;

    /// 插件描述
    fn description(&self) -> &str {
        ""
    }

    /// 依赖列表
    fn dependencies(&self) -> Vec<&str> {
        Vec::new()
    }

    /// 初始化插件
    async fn initialize(&self, context: &PluginContext) -> Layer4Result<()>;

    /// 执行插件
    async fn execute(&self, input: &serde_json::Value) -> Layer4Result<serde_json::Value>;

    /// 关闭插件
    async fn shutdown(&self) -> Layer4Result<()> {
        Ok(())
    }
}

/// 插件元数据
///
/// 注意: 此结构体用于 Rust 插件元数据序列化，不直接用于 FFI 边界。
/// FFI 插件应使用 C-compatible 的 PluginMetadata 结构体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
#[allow(improper_ctypes_definitions)] // 用于 Rust 内部序列化，非直接 FFI
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub entry_point: String,
}

impl Default for PluginMeta {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            version: "0.1.0".to_string(),
            author: "unknown".to_string(),
            description: String::new(),
            dependencies: Vec::new(),
            entry_point: "main".to_string(),
        }
    }
}

/// 插件状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Unloaded,
    Loaded,
    Initialized,
    Running,
    Error,
    Shutdown,
}

/// 插件信息
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub meta: PluginMeta,
    pub state: PluginState,
    pub path: std::path::PathBuf,
    pub loaded_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 插件上下文
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub plugin_name: String,
    pub config: serde_json::Value,
    pub data_dir: std::path::PathBuf,
}

impl PluginContext {
    pub fn new(plugin_name: impl Into<String>, data_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            plugin_name: plugin_name.into(),
            config: serde_json::Value::Null,
            data_dir: data_dir.into(),
        }
    }

    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }
}

/// 插件注册表
pub struct PluginRegistry {
    plugins: RwLock<HashMap<String, PluginInfo>>,
    instances: RwLock<HashMap<String, Arc<dyn Plugin>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            instances: RwLock::new(HashMap::new()),
        }
    }

    /// 注册插件
    pub fn register(&self, plugin: impl Plugin + 'static, path: &Path) -> Layer4Result<()> {
        let plugin: Arc<dyn Plugin> = Arc::new(plugin);
        let name = plugin.name().to_string();
        let meta = PluginMeta {
            name: name.clone(),
            version: plugin.version().to_string(),
            description: plugin.description().to_string(),
            dependencies: plugin
                .dependencies()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        };

        let info = PluginInfo {
            meta,
            state: PluginState::Loaded,
            path: path.to_path_buf(),
            loaded_at: Some(chrono::Utc::now()),
        };

        self.plugins.write().insert(name.clone(), info);
        self.instances.write().insert(name, plugin);

        Ok(())
    }

    /// 注销插件
    pub fn unregister(&self, name: &str) -> Layer4Result<bool> {
        self.plugins.write().remove(name);
        Ok(self.instances.write().remove(name).is_some())
    }

    /// 获取插件信息
    pub fn get_info(&self, name: &str) -> Option<PluginInfo> {
        self.plugins.read().get(name).cloned()
    }

    /// 获取插件实例（fix: now returns Arc clone from instances map）
    pub fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        self.instances.read().get(name).cloned()
    }

    /// 列出所有插件
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins.read().values().cloned().collect()
    }

    /// 更新插件状态
    pub fn update_state(&self, name: &str, state: PluginState) {
        if let Some(info) = self.plugins.write().get_mut(name) {
            info.state = state;
        }
    }

    /// 插件数量
    pub fn count(&self) -> usize {
        self.plugins.read().len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 插件加载器
pub struct PluginLoader {
    registry: PluginRegistry,
    dylib_loader: DylibLoader,
    wasm_loader: WasmLoader,
    plugin_dir: std::path::PathBuf,
}

impl PluginLoader {
    /// 创建新的插件加载器
    pub fn new(plugin_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            registry: PluginRegistry::new(),
            dylib_loader: DylibLoader::new(),
            wasm_loader: WasmLoader::new().expect("Failed to create WasmLoader"),
            plugin_dir: plugin_dir.into(),
        }
    }

    /// 使用默认目录创建
    pub fn with_default_dir() -> Self {
        Self::new("~/.continuum/plugins")
    }

    /// 加载动态库插件
    ///
    /// # Safety
    ///
    /// 动态库加载涉及不安全操作，请确保库来源可信
    pub async fn load_dylib(&self, path: &Path) -> Layer4Result<String> {
        let (name, meta) = self.dylib_loader.load_safe(path)?;

        let info = PluginInfo {
            meta,
            state: PluginState::Loaded,
            path: path.to_path_buf(),
            loaded_at: Some(chrono::Utc::now()),
        };

        self.registry.plugins.write().insert(name.clone(), info);
        self.registry.update_state(&name, PluginState::Loaded);

        Ok(name)
    }

    /// 加载单个插件（自动检测类型）
    pub async fn load(&self, path: &Path) -> Layer4Result<String> {
        // 检测插件类型
        let ext = path.extension().and_then(|e| e.to_str());

        match ext {
            Some("so") | Some("dylib") | Some("dll") => self.load_dylib(path).await,
            Some("wasm") => self.load_wasm(path).await,
            _ => {
                let ext_display = ext.unwrap_or("(no extension)");
                Err(anyhow::anyhow!(
                    "Unsupported plugin extension '{}'. Supported formats: .so, .dylib, .dll (native), .wasm (WebAssembly)",
                    ext_display
                ))
            }
        }
    }

    /// 加载 WASM 插件
    pub async fn load_wasm(&self, path: &Path) -> Layer4Result<String> {
        let capabilities = CapabilitySet::sandboxed();
        let name = self.wasm_loader.load(path, capabilities)?;
        self.registry.update_state(&name, PluginState::Loaded);
        Ok(name)
    }

    /// 加载目录中的所有插件
    pub async fn load_dir(&self) -> Layer4Result<Vec<String>> {
        let mut loaded = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str());

                // 支持的插件类型
                if matches!(ext, Some("so") | Some("dylib") | Some("dll") | Some("wasm")) {
                    if let Ok(name) = self.load(&path).await {
                        loaded.push(name);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// 获取插件
    pub fn get(&self, name: &str) -> Option<PluginInfo> {
        self.registry.get_info(name)
    }

    /// 获取插件元数据
    pub fn get_meta(&self, name: &str) -> Option<PluginMeta> {
        self.dylib_loader.get_meta(name)
    }

    /// 初始化插件
    pub async fn initialize(&self, name: &str, context: &PluginContext) -> Layer4Result<()> {
        // 调用插件的 FFI 初始化函数（如果存在）
        let config_json = serde_json::to_string(&context.config).unwrap_or_default();
        match self.dylib_loader.call_initialize(name, &config_json) {
            Some(true) => {
                tracing::debug!("Plugin {} initialized via FFI", name);
            }
            Some(false) => {
                tracing::warn!("Plugin {} FFI initialize returned failure", name);
            }
            None => {
                // 插件未导出 plugin_initialize 函数，跳过（可选）
                tracing::debug!("Plugin {} has no FFI initialize function", name);
            }
        }

        self.registry.update_state(name, PluginState::Initialized);
        Ok(())
    }

    /// 重新加载插件
    pub async fn reload(&self, name: &str) -> Layer4Result<()> {
        // 先卸载
        self.dylib_loader.unload(name)?;

        // 重新加载
        let info = self.registry.get_info(name);
        if let Some(info) = info {
            self.load_dylib(&info.path).await?;
        }

        Ok(())
    }

    /// 卸载插件
    pub async fn unload(&self, name: &str) -> Layer4Result<()> {
        self.registry.update_state(name, PluginState::Shutdown);
        self.dylib_loader.unload(name)?;
        self.registry.unregister(name)?;
        Ok(())
    }

    /// 列出所有插件
    pub fn list(&self) -> Vec<PluginInfo> {
        self.registry.list()
    }

    /// 插件数量
    pub fn count(&self) -> usize {
        self.registry.count()
    }

    /// 渲染插件状态
    pub fn render_status(&self) -> String {
        let plugins = self.registry.list();
        let mut output = String::new();

        output.push_str("Plugins:\n");

        if plugins.is_empty() {
            output.push_str("  No plugins loaded\n");
        } else {
            for info in plugins {
                let status = match info.state {
                    PluginState::Unloaded => "⚪",
                    PluginState::Loaded => "🔵",
                    PluginState::Initialized => "🟢",
                    PluginState::Running => "🟡",
                    PluginState::Error => "🔴",
                    PluginState::Shutdown => "⚫",
                };
                output.push_str(&format!(
                    "  {} {} v{}\n",
                    status, info.meta.name, info.meta.version
                ));
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry_creation() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_plugin_context_creation() {
        let ctx = PluginContext::new("test-plugin", "/tmp/plugins");
        assert_eq!(ctx.plugin_name, "test-plugin");
    }

    #[test]
    fn test_plugin_loader_creation() {
        let loader = PluginLoader::with_default_dir();
        assert_eq!(loader.count(), 0);
    }

    #[test]
    fn test_plugin_meta_default() {
        let meta = PluginMeta::default();
        assert_eq!(meta.name, "unknown");
        assert_eq!(meta.version, "0.1.0");
    }

    #[tokio::test]
    async fn test_unknown_plugin_extension_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_path = dir.path().join("plugin.txt");
        std::fs::write(&plugin_path, b"not a plugin").unwrap();

        let loader = PluginLoader::new(dir.path());
        let err = loader.load(&plugin_path).await.unwrap_err();
        let message = err.to_string();

        assert!(message.contains("Unsupported plugin extension"));
        assert!(message.contains(".wasm"));
    }
}
