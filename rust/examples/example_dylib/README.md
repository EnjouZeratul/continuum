# Example Dynamic Library Plugin

This is an example plugin demonstrating how to create a dynamic library plugin for Continuum.

## Building

```bash
cargo build --release
```

The output will be a dynamic library:
- Linux: `libexample_dylib_plugin.so`
- macOS: `libexample_dylib_plugin.dylib`
- Windows: `example_dylib_plugin.dll`

## Plugin API

Every plugin must export the following functions:

### Required Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `plugin_create` | `extern "C" fn() -> *mut ()` | Create a new plugin instance |
| `plugin_destroy` | `extern "C" fn(*mut ())` | Destroy the plugin instance |
| `plugin_meta` | `extern "C" fn() -> PluginMetadata` | Get plugin metadata |
| `plugin_execute` | `extern "C" fn(*mut (), *const c_char) -> *mut c_char` | Execute the plugin |

### Optional Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `plugin_init` | `extern "C" fn(*mut ()) -> i32` | Initialize the plugin |
| `plugin_free_string` | `extern "C" fn(*mut c_char)` | Free strings returned by execute |
| `plugin_get_name` | `extern "C" fn(*const ()) -> *const c_char` | Get plugin name |
| `plugin_get_version` | `extern "C" fn(*const ()) -> *const c_char` | Get plugin version |

## PluginMetadata Structure

```c
typedef struct {
    const char* name;
    const char* version;
    const char* author;
    const char* description;
} PluginMetadata;
```

## Loading in Continuum

```rust
use sh_layer4::plugin_loader::PluginLoader;
use std::path::Path;

let loader = PluginLoader::new("./plugins");
let name = loader.load_dylib(Path::new("./libexample_dylib_plugin.so")).await?;
println!("Loaded plugin: {}", name);
```

## Example Usage

```rust
// Create plugin instance
let handle = plugin_create();

// Initialize
plugin_init(handle);

// Execute with JSON input
let input = CString::new(r#"{"action": "echo", "data": "hello"}"#).unwrap();
let output = plugin_execute(handle, input.as_ptr());
let result = CStr::from_ptr(output).to_str().unwrap();
println!("Result: {}", result);

// Free output string
plugin_free_string(output);

// Destroy plugin
plugin_destroy(handle);
```

## Testing

```bash
cargo test
```

## Safety Considerations

1. **Memory Management**: Always call `plugin_free_string` for strings returned by `plugin_execute`
2. **Thread Safety**: Plugins should be thread-safe if used in multi-threaded environments
3. **Error Handling**: Return appropriate error codes or null pointers on failure
4. **Resource Cleanup**: Always call `plugin_destroy` when done with a plugin

## Creating Your Own Plugin

1. Copy this example as a template
2. Modify the `ExamplePlugin` struct with your data
3. Implement your logic in the `execute` method
4. Update metadata in `plugin_meta`
5. Build as `cdylib`

```toml
[lib]
crate-type = ["cdylib"]
```
