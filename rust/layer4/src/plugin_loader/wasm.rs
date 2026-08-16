//! WebAssembly Plugin Loader
//!
//! Loads and executes WASM modules in a sandboxed environment.
//!
//! ## Features
//! - Real wasmtime execution
//! - Sandboxed memory and CPU limits
//! - Capability-based security
//! - Async plugin execution
//! - WASI preview1 support with sandboxed filesystem

use super::capabilities::CapabilitySet;
use super::sandbox::PluginSandbox;
use super::{Plugin, PluginContext};
use crate::types::Layer4Result;
use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use wasmtime::*;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

/// WASM plugin configuration
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Maximum memory in bytes (default: 16MB)
    pub max_memory_bytes: u64,
    /// Maximum CPU time in milliseconds (default: 5000ms)
    pub max_cpu_time_ms: u64,
    /// Maximum table elements (default: 10000)
    pub max_table_elements: u32,
    /// Enable WASI preview1 (default: true)
    pub enable_wasi: bool,
    /// Allow async execution (default: true)
    pub enable_async: bool,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024, // 16 MB
            max_cpu_time_ms: 5000,              // 5 seconds
            max_table_elements: 10000,
            enable_wasi: true,
            enable_async: true,
        }
    }
}

/// WASM plugin state (store data)
pub struct PluginState {
    /// Security sandbox
    pub sandbox: PluginSandbox,
    /// Data directory for plugin
    pub data_dir: PathBuf,
    /// Execution start time
    pub start_time: Option<Instant>,
    /// CPU time limit
    pub cpu_limit_ms: u64,
    /// Memory tracker
    pub memory_used: u64,
    /// WASI context (for sandboxed filesystem access)
    pub wasi_ctx: Option<WasiCtx>,
}

impl PluginState {
    fn new(sandbox: PluginSandbox, data_dir: PathBuf, cpu_limit_ms: u64) -> Self {
        Self {
            sandbox,
            data_dir,
            start_time: None,
            cpu_limit_ms,
            memory_used: 0,
            wasi_ctx: None,
        }
    }

    fn with_wasi(mut self, wasi_ctx: WasiCtx) -> Self {
        self.wasi_ctx = Some(wasi_ctx);
        self
    }

    fn check_cpu_limit(&self) -> Result<()> {
        if self.cpu_limit_ms == 0 {
            return Ok(());
        }
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_millis() as u64;
            if elapsed > self.cpu_limit_ms {
                return Err(anyhow!(
                    "CPU time limit exceeded: {}ms > {}ms",
                    elapsed,
                    self.cpu_limit_ms
                ));
            }
        }
        Ok(())
    }
}

/// WASM plugin instance
pub struct WasmPlugin {
    /// Plugin name
    name: String,
    /// Plugin version
    version: String,
    /// Sandbox reference
    sandbox: PluginSandbox,
    /// Module reference
    module: Arc<Module>,
    /// Engine reference
    engine: Engine,
    /// Configuration
    config: WasmConfig,
}

impl WasmPlugin {
    /// Create new WASM plugin
    fn new(
        name: String,
        version: String,
        sandbox: PluginSandbox,
        module: Module,
        engine: Engine,
        config: WasmConfig,
    ) -> Self {
        Self {
            name,
            version,
            sandbox,
            module: Arc::new(module),
            engine,
            config,
        }
    }

    /// Get module reference
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Get engine reference
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Create a new store for execution
    pub fn create_store(&self, data_dir: PathBuf) -> Store<PluginState> {
        let cpu_limit = self.config.max_cpu_time_ms;
        let state = PluginState::new(self.sandbox.clone(), data_dir, cpu_limit);
        Store::new(&self.engine, state)
    }

    /// Instantiate the plugin in a store
    pub fn instantiate(&self, store: &mut Store<PluginState>) -> Result<Instance> {
        // Start CPU timer
        store.data_mut().start_time = Some(Instant::now());

        // Create instance
        Instance::new(store, &self.module, &[])
            .with_context(|| format!("Failed to instantiate WASM plugin: {}", self.name))
    }

    /// Execute a function by name with JSON input/output
    pub fn execute_func(
        &self,
        store: &mut Store<PluginState>,
        instance: &Instance,
        func_name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Check CPU limit before execution
        store.data().check_cpu_limit()?;

        // Get the function
        let func = instance
            .get_typed_func::<(i32, i32), (i32, i32)>(&mut *store, func_name)
            .with_context(|| format!("Function '{}' not found in plugin", func_name))?;

        // Allocate input in WASM memory
        let input_bytes = serde_json::to_vec(input)?;
        let input_len = input_bytes.len() as i32;

        // Get memory export
        let memory = instance
            .get_memory(&mut *store, "memory")
            .ok_or_else(|| anyhow!("No memory export in plugin"))?;

        // Allocate space for input (simple bump allocator)
        let input_ptr = self.allocate_in_memory(store, memory, input_len)?;

        // Write input to memory
        memory.data_mut(&mut *store)[input_ptr as usize..][..input_len as usize]
            .copy_from_slice(&input_bytes);

        // Call the function
        let (output_ptr, output_len) = func.call(&mut *store, (input_ptr, input_len))?;

        // Read output from memory
        let data = memory.data(&store);
        let output_slice = &data[output_ptr as usize..][..output_len as usize];
        let output: serde_json::Value = serde_json::from_slice(output_slice)
            .unwrap_or_else(|_| serde_json::json!({"error": "Invalid JSON output"}));

        // Check CPU limit after execution
        store.data().check_cpu_limit()?;

        Ok(output)
    }

    /// Simple bump allocator for WASM memory
    fn allocate_in_memory(
        &self,
        store: &mut Store<PluginState>,
        memory: Memory,
        size: i32,
    ) -> Result<i32> {
        let data = memory.data_mut(&mut *store);
        let current_size = data.len() as i32;

        // Simple allocation at end of current data
        // In production, you'd want a proper allocator
        let ptr = current_size;

        // Grow memory if needed
        let needed_size = ptr + size;
        let current_pages = memory.size(&store);
        let needed_pages = (needed_size / 65536) + 1;

        if needed_pages > current_pages as i32 {
            let grow_by = needed_pages - current_pages as i32;
            memory
                .grow(&mut *store, grow_by as u64)
                .with_context(|| "Failed to grow WASM memory")?;
        }

        // Track memory usage
        store.data_mut().memory_used += size as u64;

        Ok(ptr)
    }
}

#[async_trait::async_trait]
impl Plugin for WasmPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    async fn initialize(&self, _context: &PluginContext) -> Layer4Result<()> {
        // WASM initialization happens at instantiate time
        Ok(())
    }

    async fn execute(&self, input: &serde_json::Value) -> Layer4Result<serde_json::Value> {
        // Check CPU limit before execution
        self.sandbox.check_cpu_limit()?;

        // Create a new store for this execution
        let data_dir = std::env::temp_dir();
        let mut store = self.create_store(data_dir);

        // Instantiate
        let instance = self
            .instantiate(&mut store)
            .map_err(|e| anyhow!("WASM instantiation failed: {}", e))?;

        // Try to find and call a default entry point
        // Look for common function names
        for func_name in &["execute", "run", "_start", "main"] {
            if let Ok(output) = self.execute_func(&mut store, &instance, func_name, input) {
                return Ok(output);
            }
        }

        // If no entry point found, return error with available info
        Err(anyhow!(
            "WASM plugin '{}' has no callable entry point. Expected one of: execute, run, _start, main",
            self.name
        ))
    }

    async fn shutdown(&self) -> Layer4Result<()> {
        Ok(())
    }
}

/// WASM plugin loader
pub struct WasmLoader {
    /// Wasmtime engine
    engine: Engine,
    /// Loaded modules
    modules: RwLock<HashMap<String, Module>>,
    /// Plugin instances
    plugins: RwLock<HashMap<String, Arc<WasmPlugin>>>,
    /// Default configuration
    config: WasmConfig,
}

impl WasmLoader {
    /// Create new WASM loader
    pub fn new() -> Layer4Result<Self> {
        Self::with_config(WasmConfig::default())
    }

    /// Create WASM loader with custom configuration
    pub fn with_config(config: WasmConfig) -> Layer4Result<Self> {
        let mut engine_config = Config::new();
        engine_config.wasm_backtrace_details(WasmBacktraceDetails::Enable);
        engine_config.cranelift_opt_level(OptLevel::Speed);

        // Configure memory limits
        if config.max_memory_bytes > 0 {
            let max_pages = (config.max_memory_bytes / 65536) + 1;
            engine_config.wasm_memory64(true);
            engine_config.static_memory_maximum_size(max_pages * 65536);
        }

        let engine = Engine::new(&engine_config).context("Failed to create Wasmtime engine")?;

        Ok(Self {
            engine,
            modules: RwLock::new(HashMap::new()),
            plugins: RwLock::new(HashMap::new()),
            config,
        })
    }

    /// Check if file is valid WASM
    pub fn is_valid_wasm(path: &Path) -> bool {
        if !path.exists() || !path.is_file() {
            return false;
        }
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "wasm")
            .unwrap_or(false)
    }

    /// Load WASM module
    pub fn load(&self, path: &Path, capabilities: CapabilitySet) -> Layer4Result<String> {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Compile module (validates WASM)
        let module = Module::from_file(&self.engine, path)
            .with_context(|| format!("Failed to compile WASM: {:?}", path))?;

        self.insert_module(name, module, capabilities)
    }

    /// Load a WASM plugin from WAT (WebAssembly Text) source.
    ///
    /// Used by the self-evolution pipeline: the agent authors tool code as
    /// WAT text, which wasmtime compiles directly — no external toolchain,
    /// no process execution.
    pub fn load_wat(&self, name: &str, wat: &str, capabilities: CapabilitySet) -> Layer4Result<String> {
        let module = Module::new(&self.engine, wat)
            .with_context(|| format!("Failed to compile WAT source for plugin '{}'", name))?;
        self.insert_module(name.to_string(), module, capabilities)
    }

    /// Load a WASM plugin from a raw binary.
    pub fn load_binary(
        &self,
        name: &str,
        bytes: &[u8],
        capabilities: CapabilitySet,
    ) -> Layer4Result<String> {
        let module = Module::from_binary(&self.engine, bytes)
            .with_context(|| format!("Failed to parse WASM binary for plugin '{}'", name))?;
        self.insert_module(name.to_string(), module, capabilities)
    }

    /// Shared tail of all load paths: sandbox, wrap, register in maps.
    fn insert_module(
        &self,
        name: String,
        module: Module,
        capabilities: CapabilitySet,
    ) -> Layer4Result<String> {
        // Create sandbox
        let sandbox = PluginSandbox::new(capabilities);

        // Create plugin instance
        let plugin = WasmPlugin::new(
            name.clone(),
            self.extract_version(&module)
                .unwrap_or_else(|| "0.1.0".to_string()),
            sandbox,
            module.clone(),
            self.engine.clone(),
            self.config.clone(),
        );

        self.modules.write().insert(name.clone(), module);
        self.plugins.write().insert(name.clone(), Arc::new(plugin));

        tracing::info!("Loaded WASM plugin: {}", name);

        Ok(name)
    }

    /// Extract version from module exports if available
    fn extract_version(&self, _module: &Module) -> Option<String> {
        // Version extraction not directly available in wasmtime API
        // Could be stored in custom section, but that requires different approach
        // For now, return None and use default version
        None
    }

    /// Get loaded plugin
    pub fn get(&self, name: &str) -> Option<Arc<WasmPlugin>> {
        self.plugins.read().get(name).cloned()
    }

    /// Unload plugin
    pub fn unload(&self, name: &str) -> Layer4Result<()> {
        self.modules.write().remove(name);
        self.plugins.write().remove(name);
        tracing::info!("Unloaded WASM plugin: {}", name);
        Ok(())
    }

    /// List loaded plugins
    pub fn list(&self) -> Vec<String> {
        self.plugins.read().keys().cloned().collect()
    }

    /// Get engine reference
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Load and execute a WASM module in one step
    pub fn load_and_execute(
        &self,
        path: &Path,
        input: &serde_json::Value,
        capabilities: CapabilitySet,
    ) -> Layer4Result<serde_json::Value> {
        let name = self.load(path, capabilities)?;
        let plugin = self
            .get(&name)
            .ok_or_else(|| anyhow!("Plugin not found after loading: {}", name))?;

        // Use tokio runtime for async execution
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

        rt.block_on(async { plugin.execute(input).await })
    }
}

impl Default for WasmLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create WasmLoader")
    }
}

/// WASI context builder for sandboxed execution
pub struct WasiContextBuilder {
    /// Allowed preopened directories (guest_path, host_path, dir_perms, file_perms)
    preopens: Vec<(String, PathBuf, DirPerms, FilePerms)>,
    /// Environment variables
    env: HashMap<String, String>,
    /// Arguments
    args: Vec<String>,
    /// Inherit stdout/stderr
    inherit_stdio: bool,
    /// Inherit environment from host
    inherit_env: bool,
}

impl WasiContextBuilder {
    /// Create new WASI context builder
    pub fn new() -> Self {
        Self {
            preopens: Vec::new(),
            env: HashMap::new(),
            args: Vec::new(),
            inherit_stdio: true,
            inherit_env: false,
        }
    }

    /// Add a preopened directory with full permissions
    pub fn preopen(&mut self, guest_path: &str, host_path: PathBuf) -> &mut Self {
        self.preopens.push((
            guest_path.to_string(),
            host_path,
            DirPerms::all(),
            FilePerms::all(),
        ));
        self
    }

    /// Add a preopened directory with read-only permissions
    pub fn preopen_readonly(&mut self, guest_path: &str, host_path: PathBuf) -> &mut Self {
        self.preopens.push((
            guest_path.to_string(),
            host_path,
            DirPerms::READ,
            FilePerms::READ,
        ));
        self
    }

    /// Add a preopened directory with custom permissions
    pub fn preopen_with_perms(
        &mut self,
        guest_path: &str,
        host_path: PathBuf,
        dir_perms: DirPerms,
        file_perms: FilePerms,
    ) -> &mut Self {
        self.preopens
            .push((guest_path.to_string(), host_path, dir_perms, file_perms));
        self
    }

    /// Set an environment variable
    pub fn env(&mut self, key: &str, value: &str) -> &mut Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }

    /// Add multiple environment variables
    pub fn envs(&mut self, envs: &[(impl AsRef<str>, impl AsRef<str>)]) -> &mut Self {
        for (k, v) in envs {
            self.env
                .insert(k.as_ref().to_string(), v.as_ref().to_string());
        }
        self
    }

    /// Add an argument
    pub fn arg(&mut self, arg: &str) -> &mut Self {
        self.args.push(arg.to_string());
        self
    }

    /// Add multiple arguments
    pub fn args(&mut self, args: &[impl AsRef<str>]) -> &mut Self {
        for arg in args {
            self.args.push(arg.as_ref().to_string());
        }
        self
    }

    /// Inherit standard I/O (stdin, stdout, stderr)
    pub fn inherit_stdio(&mut self, inherit: bool) -> &mut Self {
        self.inherit_stdio = inherit;
        self
    }

    /// Inherit all environment variables from host process
    pub fn inherit_env(&mut self, inherit: bool) -> &mut Self {
        self.inherit_env = inherit;
        self
    }

    /// Build the WASI context
    pub fn build(&self) -> WasiCtx {
        let mut builder = WasiCtxBuilder::new();

        // Configure stdio
        if self.inherit_stdio {
            builder.inherit_stdio();
        }

        // Configure environment
        if self.inherit_env {
            builder.inherit_env();
        }

        // Add custom environment variables
        for (key, value) in &self.env {
            builder.env(key, value);
        }

        // Add arguments
        for arg in &self.args {
            builder.arg(arg);
        }

        // Configure preopened directories
        for (guest_path, host_path, dir_perms, file_perms) in &self.preopens {
            builder
                .preopened_dir(host_path, guest_path, *dir_perms, *file_perms)
                .expect("Failed to preopen directory");
        }

        builder.build()
    }
}

impl Default for WasiContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_loader_creation() {
        let loader = WasmLoader::new();
        assert!(loader.is_ok());
        let loader = loader.unwrap();
        assert!(loader.list().is_empty());
    }

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.max_memory_bytes, 16 * 1024 * 1024);
        assert_eq!(config.max_cpu_time_ms, 5000);
        assert!(config.enable_wasi);
    }

    #[test]
    fn test_is_valid_wasm() {
        let tmp = tempfile::NamedTempFile::with_suffix(".wasm").unwrap();
        assert!(WasmLoader::is_valid_wasm(tmp.path()));

        let tmp_txt = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        assert!(!WasmLoader::is_valid_wasm(tmp_txt.path()));
    }

    #[test]
    fn test_wasm_plugin_creation() {
        let loader = WasmLoader::new().unwrap();
        let sandbox = PluginSandbox::sandboxed();
        let engine = loader.engine().clone();
        let config = WasmConfig::default();

        // Create an empty module for testing
        let module = Module::new(&engine, "(module)").unwrap();
        let plugin = WasmPlugin::new(
            "test".to_string(),
            "0.1.0".to_string(),
            sandbox,
            module,
            engine,
            config,
        );

        assert_eq!(plugin.name(), "test");
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn test_wasi_context_builder() {
        let mut builder = WasiContextBuilder::new();
        builder.env("TEST", "value");
        builder.arg("--help");

        // Build WASI context
        let _ctx = builder.build();
        // WasiCtx is created successfully (no need to verify contents directly)
        // The context is now a real wasmtime-wasi WasiCtx
    }

    #[test]
    fn test_wasi_context_builder_with_preopen() {
        let mut builder = WasiContextBuilder::new();
        builder.env("HOME", "/home/user");
        builder.arg("--test");
        builder.preopen("/tmp", std::env::temp_dir());

        let _ctx = builder.build();
        // Successfully built with preopened directory
    }

    #[test]
    fn test_wasi_context_builder_readonly() {
        let mut builder = WasiContextBuilder::new();
        builder.preopen_readonly("/data", std::env::temp_dir());
        builder.inherit_stdio(true);
        builder.inherit_env(false);

        let _ctx = builder.build();
        // Successfully built with read-only preopen
    }

    #[tokio::test]
    async fn test_plugin_execute_without_entry_point_returns_error() {
        let loader = WasmLoader::new().unwrap();
        let sandbox = PluginSandbox::sandboxed();
        let engine = loader.engine().clone();
        let config = WasmConfig::default();

        // Create minimal module
        let module = Module::new(&engine, "(module)").unwrap();
        let plugin = WasmPlugin::new(
            "test".to_string(),
            "0.1.0".to_string(),
            sandbox,
            module,
            engine,
            config,
        );

        let input = serde_json::json!({"test": "input"});
        let result = plugin.execute(&input).await;
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("no callable entry point"));
        assert!(error.contains("execute, run, _start, main"));
    }
}
