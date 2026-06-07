# Continuum Plugin System

This document describes the plugin system architecture, security model, and development guide.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Security Model](#security-model)
4. [Plugin Types](#plugin-types)
5. [Creating Plugins](#creating-plugins)
6. [API Reference](#api-reference)
7. [Loading Plugins](#loading-plugins)
8. [Testing Plugins](#testing-plugins)
9. [Best Practices](#best-practices)
10. [Troubleshooting](#troubleshooting)

---

## Overview

The Continuum plugin system allows extending the runtime with custom functionality through dynamically loaded modules. Plugins can be written in Rust and compiled as dynamic libraries (.so, .dylib, .dll) or WebAssembly modules (.wasm).

### Key Features

- **Dynamic Loading**: Load plugins at runtime without recompiling the host
- **Type Safety**: Rust-based plugin API with strong typing
- **Sandboxing**: WASM plugins run in isolated environments
- **Hot Reload**: Reload plugins without restarting (planned)
- **Capability-based Security**: Fine-grained permission control

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Continuum Runtime                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │  PluginLoader   │    │  PluginRegistry │                │
│  └────────┬────────┘    └────────┬────────┘                │
│           │                      │                          │
│           ▼                      ▼                          │
│  ┌─────────────────────────────────────────┐               │
│  │           Plugin Interface              │               │
│  └─────────────────────────────────────────┘               │
│           │                      │                          │
│           ▼                      ▼                          │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │  DylibLoader    │    │   WasmLoader    │                │
│  │  (.so/.dll)     │    │   (.wasm)       │                │
│  └─────────────────┘    └─────────────────┘                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Components

| Component | Description |
|-----------|-------------|
| `PluginLoader` | Main entry point for loading plugins |
| `PluginRegistry` | Tracks loaded plugins and their state |
| `DylibLoader` | Loads native dynamic libraries |
| `WasmLoader` | Loads WebAssembly modules (planned) |
| `Plugin` trait | Interface all plugins must implement |

---

## Security Model

### Threat Model

Plugins are untrusted code that executes within the Continuum process. The following threats are addressed:

| Threat | Mitigation |
|--------|------------|
| Code Injection | Plugins run in isolated contexts |
| Memory Corruption | Rust memory safety + capability restrictions |
| Resource Exhaustion | Resource limits and quotas |
| Data Exfiltration | Capability-based access control |
| Privilege Escalation | Principle of least privilege |

### Capability System

Plugins must declare required capabilities:

```rust
pub struct PluginCapabilities {
    /// File system access
    pub filesystem: bool,
    /// Network access
    pub network: bool,
    /// Process execution
    pub process_execution: bool,
    /// Environment variable access
    pub env_access: bool,
    /// Maximum memory (bytes)
    pub max_memory: Option<usize>,
    /// Maximum execution time (ms)
    pub max_execution_time: Option<u64>,
}
```

### Security Levels

| Level | Description | Use Case |
|-------|-------------|----------|
| **Sandboxed** | Full isolation, no host access | Untrusted plugins |
| **Restricted** | Limited host access via capabilities | Semi-trusted plugins |
| **Trusted** | Full host access | Core plugins, verified plugins |

### Dylib Security Considerations

Dynamic libraries have full access to the host process:

1. **Trust Requirement**: Only load dylib plugins from trusted sources
2. **Code Signing**: Verify plugin signatures when available
3. **Sandboxing**: Consider using OS-level sandboxing (seccomp, pledge, etc.)
4. **Review**: Audit plugin code before loading

### WASM Security

WebAssembly plugins are sandboxed by default:

1. **Memory Isolation**: WASM memory is separate from host
2. **Capability-based**: All host functions must be explicitly imported
3. **Resource Limits**: CPU and memory limits enforced
4. **No Direct I/O**: All I/O goes through host-provided functions

---

## Plugin Types

### Dynamic Library Plugins (Dylib)

Native plugins compiled as shared libraries:

**Advantages:**
- Maximum performance
- Full Rust ecosystem access
- Direct FFI with host

**Disadvantages:**
- Platform-specific binaries
- Full trust required
- Potential for memory safety issues

**File Extensions:**
- Linux: `.so`
- macOS: `.dylib`
- Windows: `.dll`

### WebAssembly Plugins (WASM)

Plugins compiled to WebAssembly:

**Advantages:**
- Platform-independent
- Sandboxed execution
- Smaller attack surface

**Disadvantages:**
- Performance overhead
- Limited API access
- Requires WASM runtime

---

## Creating Plugins

### Step 1: Project Setup

Create a new Rust project:

```bash
cargo new --lib my_plugin
cd my_plugin
```

### Step 2: Configure Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
name = "my_plugin"
crate-type = ["cdylib"]

[dependencies]
serde_json = "1.0"
```

### Step 3: Implement Plugin

```rust
use std::ffi::{c_char, CStr, CString};

/// Global plugin instance
static mut PLUGIN_INSTANCE: Option<MyPlugin> = None;

struct MyPlugin {
    initialized: bool,
}

impl MyPlugin {
    fn new() -> Self {
        Self { initialized: false }
    }
}

/// Plugin metadata (C-compatible)
#[repr(C)]
pub struct PluginMetadata {
    pub name: *const c_char,
    pub version: *const c_char,
    pub author: *const c_char,
    pub description: *const c_char,
}

/// Create plugin instance
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut () {
    unsafe {
        PLUGIN_INSTANCE = Some(MyPlugin::new());
        &mut PLUGIN_INSTANCE as *mut _ as *mut ()
    }
}

/// Destroy plugin instance
#[no_mangle]
pub extern "C" fn plugin_destroy(_handle: *mut ()) {
    unsafe {
        PLUGIN_INSTANCE = None;
    }
}

/// Get plugin metadata
#[no_mangle]
pub extern "C" fn plugin_meta() -> PluginMetadata {
    PluginMetadata {
        name: CString::new("my-plugin").unwrap().into_raw(),
        version: CString::new("0.1.0").unwrap().into_raw(),
        author: CString::new("Your Name").unwrap().into_raw(),
        description: CString::new("My plugin description").unwrap().into_raw(),
    }
}

/// Initialize plugin
#[no_mangle]
pub extern "C" fn plugin_init(_handle: *mut ()) -> i32 {
    unsafe {
        if let Some(plugin) = PLUGIN_INSTANCE.as_mut() {
            plugin.initialized = true;
            0
        } else {
            -1
        }
    }
}

/// Execute plugin
#[no_mangle]
pub extern "C" fn plugin_execute(
    _handle: *mut (),
    input: *const c_char,
) -> *mut c_char {
    unsafe {
        let input_str = if input.is_null() {
            ""
        } else {
            CStr::from_ptr(input).to_str().unwrap_or("")
        };

        // Process input
        let result = format!("Processed: {}", input_str);

        CString::new(result).unwrap().into_raw()
    }
}

/// Free string returned by plugin_execute
#[no_mangle]
pub extern "C" fn plugin_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}
```

### Step 4: Build

```bash
cargo build --release
```

Output: `target/release/libmy_plugin.so` (Linux) or `my_plugin.dll` (Windows)

---

## API Reference

### Required FFI Functions

All plugins must export these functions:

#### `plugin_create`

```rust
pub extern "C" fn plugin_create() -> *mut ()
```

Creates a new plugin instance. Returns an opaque handle.

**Returns:** Handle to plugin instance, or null on failure.

---

#### `plugin_destroy`

```rust
pub extern "C" fn plugin_destroy(handle: *mut ())
```

Destroys the plugin instance and frees resources.

**Parameters:**
- `handle`: Handle returned by `plugin_create`

---

#### `plugin_meta`

```rust
pub extern "C" fn plugin_meta() -> PluginMetadata
```

Returns plugin metadata.

**Returns:** `PluginMetadata` structure

```rust
#[repr(C)]
pub struct PluginMetadata {
    pub name: *const c_char,
    pub version: *const c_char,
    pub author: *const c_char,
    pub description: *const c_char,
}
```

---

#### `plugin_execute`

```rust
pub extern "C" fn plugin_execute(
    handle: *mut (),
    input: *const c_char,
) -> *mut c_char
```

Executes the plugin with JSON input.

**Parameters:**
- `handle`: Handle returned by `plugin_create`
- `input`: Null-terminated JSON string

**Returns:** Null-terminated JSON string. Caller must free with `plugin_free_string`.

---

### Optional FFI Functions

#### `plugin_init`

```rust
pub extern "C" fn plugin_init(handle: *mut ()) -> i32
```

Initializes the plugin. Called after `plugin_create`.

**Returns:** 0 on success, negative on error.

---

#### `plugin_free_string`

```rust
pub extern "C" fn plugin_free_string(s: *mut c_char)
```

Frees strings returned by `plugin_execute`.

---

### Rust Plugin Trait

For Rust-to-Rust integration, use the `Plugin` trait:

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Plugin version
    fn version(&self) -> &str;

    /// Plugin description
    fn description(&self) -> &str { "" }

    /// Dependencies
    fn dependencies(&self) -> Vec<&str> { Vec::new() }

    /// Initialize plugin
    async fn initialize(&self, context: &PluginContext) -> Layer4Result<()>;

    /// Execute plugin
    async fn execute(&self, input: &Value) -> Layer4Result<Value>;

    /// Shutdown plugin
    async fn shutdown(&self) -> Layer4Result<()> { Ok(()) }
}
```

---

## Loading Plugins

### Using PluginLoader

```rust
use sh_layer4::plugin_loader::PluginLoader;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    // Create loader with plugin directory
    let loader = PluginLoader::new("./plugins");

    // Load single plugin
    let name = loader.load_dylib(Path::new("./plugins/my_plugin.so")).await?;
    println!("Loaded: {}", name);

    // Load all plugins in directory
    let loaded = loader.load_dir().await?;
    println!("Loaded {} plugins", loaded.len());

    // List plugins
    for info in loader.list() {
        println!("  - {} v{} ({:?})",
            info.meta.name,
            info.meta.version,
            info.state
        );
    }

    Ok(())
}
```

### Plugin States

```rust
pub enum PluginState {
    Unloaded,    // Not loaded
    Loaded,      // Loaded but not initialized
    Initialized, // Ready to use
    Running,     // Currently executing
    Error,       // Error state
    Shutdown,    // Shut down
}
```

---

## Testing Plugins

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_plugin_lifecycle() {
        // Create
        let handle = plugin_create();
        assert!(!handle.is_null());

        // Initialize
        assert_eq!(plugin_init(handle), 0);

        // Execute
        let input = CString::new("test").unwrap();
        let output = plugin_execute(handle, input.as_ptr());
        let result = unsafe { CStr::from_ptr(output).to_str().unwrap() };
        assert!(result.contains("test"));

        // Cleanup
        plugin_free_string(output);
        plugin_destroy(handle);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_plugin_loader() {
    let loader = PluginLoader::new("./test_plugins");

    let name = loader
        .load_dylib(Path::new("./test_plugins/libtest_plugin.so"))
        .await
        .unwrap();

    assert!(loader.is_loaded(&name));
}
```

---

## Best Practices

### 1. Memory Safety

- Always free strings with `plugin_free_string`
- Use `#[repr(C)]` for FFI structures
- Avoid panicking across FFI boundaries

### 2. Error Handling

```rust
#[no_mangle]
pub extern "C" fn plugin_execute(
    _handle: *mut (),
    input: *const c_char,
) -> *mut c_char {
    // Use catch_unwind to prevent panics
    std::panic::catch_unwind(|| {
        // Implementation
        CString::new("result").unwrap().into_raw()
    }).unwrap_or_else(|_| {
        CString::new("{\"error\": \"Plugin panicked\"}").unwrap().into_raw()
    })
}
```

### 3. Thread Safety

- Use `Send + Sync` for plugin state
- Protect shared state with mutexes
- Avoid blocking in async contexts

### 4. Resource Management

- Release resources in `plugin_destroy`
- Use RAII patterns for cleanup
- Set timeouts for long operations

### 5. Versioning

- Follow semantic versioning
- Check compatibility in `plugin_init`
- Provide backwards compatibility

---

## Troubleshooting

### Plugin Not Loading

**Symptom:** `Failed to load library`

**Solutions:**
1. Check file extension matches platform
2. Verify library dependencies are installed
3. Check file permissions
4. Use `ldd` (Linux) or `otool` (macOS) to check dependencies

### Symbol Not Found

**Symptom:** `Symbol 'plugin_create' not found`

**Solutions:**
1. Ensure functions are marked `#[no_mangle]`
2. Check `extern "C"` is used
3. Verify `crate-type = ["cdylib"]` in Cargo.toml

### Memory Leaks

**Symptom:** Memory usage grows over time

**Solutions:**
1. Always call `plugin_free_string` for returned strings
2. Implement `plugin_destroy` to clean up resources
3. Use `valgrind` or similar tools to detect leaks

### Crashes

**Symptom:** Segmentation fault when calling plugin

**Solutions:**
1. Check for null pointers
2. Verify FFI signatures match
3. Use `catch_unwind` to handle panics
4. Enable debug symbols for better stack traces

---

## Example: Complete Plugin

See `rust/examples/example_dylib/` for a complete working example.

---

## Future Roadmap

- [ ] WASM plugin support
- [ ] Hot reload
- [ ] Plugin dependencies
- [ ] Plugin marketplace
- [ ] Signed plugins
- [ ] Plugin sandboxing for dylibs

---

## References

- [libloading documentation](https://docs.rs/libloading)
- [Rust FFI Guide](https://doc.rust-lang.org/nomicon/ffi.html)
- [WebAssembly Component Model](https://github.com/WebAssembly/component-model)
