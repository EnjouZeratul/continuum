//! Example Dynamic Library Plugin
//!
//! This is an example plugin demonstrating the Continuum plugin API.
//!
//! # Building
//!
//! ```bash
//! cargo build --release
//! ```
//!
//! The output will be a dynamic library (.so/.dylib/.dll) in target/release.
//!
//! # Loading
//!
//! Use the PluginLoader to load this plugin:
//!
//! ```rust,ignore
//! use sh_layer4::plugin_loader::PluginLoader;
//!
//! let loader = PluginLoader::new("./plugins");
//! let name = loader.load_dylib(Path::new("./libexample_dylib_plugin.so")).await?;
//! ```

// FFI code requires static mut access
#![allow(static_mut_refs)]

use std::ffi::{c_char, CStr, CString};

// ============================================================================
// Plugin State
// ============================================================================

/// Global plugin instance
static mut PLUGIN_INSTANCE: Option<ExamplePlugin> = None;

/// Example plugin implementation
struct ExamplePlugin {
    version: String,
    initialized: bool,
    call_count: u64,
}

impl ExamplePlugin {
    fn new() -> Self {
        Self {
            version: "0.1.0".to_string(),
            initialized: false,
            call_count: 0,
        }
    }

    fn initialize(&mut self) {
        self.initialized = true;
        self.call_count = 0;
    }

    fn execute(&mut self, input: &str) -> String {
        self.call_count += 1;

        // Echo the input with a prefix
        format!(
            "[example-dylib v{}] Call #{}: {}",
            self.version, self.call_count, input
        )
    }

    fn shutdown(&mut self) {
        self.initialized = false;
    }
}

impl Default for ExamplePlugin {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// FFI Export Functions
// ============================================================================

/// Plugin metadata structure (C-compatible)
#[repr(C)]
pub struct PluginMetadata {
    pub name: *const c_char,
    pub version: *const c_char,
    pub author: *const c_char,
    pub description: *const c_char,
}

/// Create a new plugin instance.
///
/// This function must be called before any other plugin functions.
/// Returns a handle that should be passed to other plugin functions.
///
/// # Safety
///
/// This function is safe to call from C code.
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut () {
    unsafe {
        PLUGIN_INSTANCE = Some(ExamplePlugin::new());
        &mut PLUGIN_INSTANCE as *mut _ as *mut ()
    }
}

/// Destroy the plugin instance.
///
/// This function should be called when the plugin is no longer needed.
///
/// # Safety
///
/// This function is safe to call from C code.
#[no_mangle]
pub extern "C" fn plugin_destroy(_handle: *mut ()) {
    unsafe {
        if let Some(plugin) = PLUGIN_INSTANCE.as_mut() {
            plugin.shutdown();
        }
        PLUGIN_INSTANCE = None;
    }
}

/// Get plugin metadata.
///
/// Returns a PluginMetadata structure containing information about the plugin.
///
/// # Safety
///
/// This function is safe to call from C code.
/// The returned pointers are valid for the lifetime of the plugin.
#[no_mangle]
pub extern "C" fn plugin_meta() -> PluginMetadata {
    let name = CString::new("example-dylib").unwrap();
    let version = CString::new("0.1.0").unwrap();
    let author = CString::new("Continuum Team").unwrap();
    let description = CString::new("Example dynamic library plugin").unwrap();

    PluginMetadata {
        name: name.into_raw(),
        version: version.into_raw(),
        author: author.into_raw(),
        description: description.into_raw(),
    }
}

/// Initialize the plugin.
///
/// This function must be called after plugin_create and before plugin_execute.
///
/// # Safety
///
/// This function is safe to call from C code.
#[no_mangle]
pub extern "C" fn plugin_init(_handle: *mut ()) -> i32 {
    unsafe {
        if let Some(plugin) = PLUGIN_INSTANCE.as_mut() {
            plugin.initialize();
            0 // Success
        } else {
            -1 // Error: plugin not created
        }
    }
}

/// Execute the plugin with input.
///
/// Takes a JSON string input and returns a JSON string output.
/// The caller is responsible for freeing the returned string using plugin_free_string.
///
/// # Safety
///
/// The input must be a valid null-terminated C string pointer, or null.
/// The returned pointer must be freed by calling plugin_free_string.
/// This function is part of the FFI interface for C compatibility.
#[no_mangle]
pub unsafe extern "C" fn plugin_execute(_handle: *mut (), input: *const c_char) -> *mut c_char {
    let input_str = if input.is_null() {
        ""
    } else {
        CStr::from_ptr(input).to_str().unwrap_or("")
    };

    let result = if let Some(plugin) = PLUGIN_INSTANCE.as_mut() {
        plugin.execute(input_str)
    } else {
        "Error: Plugin not initialized".to_string()
    };

    let result_cstring = CString::new(result).unwrap();
    result_cstring.into_raw()
}

/// Free a string returned by plugin_execute.
///
/// This function must be called to free any strings returned by plugin_execute
/// to avoid memory leaks.
///
/// # Safety
///
/// The pointer must have been returned by plugin_execute and must be valid.
/// This function is part of the FFI interface for C compatibility.
#[no_mangle]
pub unsafe extern "C" fn plugin_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Get the plugin name.
///
/// Returns a pointer to a null-terminated string containing the plugin name.
/// The returned pointer is valid for the lifetime of the plugin.
///
/// # Safety
///
/// This function is safe to call from C code.
#[no_mangle]
pub extern "C" fn plugin_get_name(_handle: *const ()) -> *const c_char {
    static NAME: &[u8] = b"example-dylib\0";
    NAME.as_ptr() as *const c_char
}

/// Get the plugin version.
///
/// Returns a pointer to a null-terminated string containing the plugin version.
/// The returned pointer is valid for the lifetime of the plugin.
///
/// # Safety
///
/// This function is safe to call from C code.
#[no_mangle]
pub extern "C" fn plugin_get_version(_handle: *const ()) -> *const c_char {
    static VERSION: &[u8] = b"0.1.0\0";
    VERSION.as_ptr() as *const c_char
}

/// Get the plugin call count.
///
/// Returns the number of times plugin_execute has been called.
///
/// # Safety
///
/// This function is safe to call from C code.
#[no_mangle]
pub extern "C" fn plugin_get_call_count(_handle: *const ()) -> u64 {
    unsafe { PLUGIN_INSTANCE.as_ref().map(|p| p.call_count).unwrap_or(0) }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_plugin_lifecycle() {
        // Create
        let handle = plugin_create();
        assert!(!handle.is_null());

        // Get metadata
        let meta = plugin_meta();
        assert!(!meta.name.is_null());
        assert!(!meta.version.is_null());

        // Initialize
        let result = plugin_init(handle);
        assert_eq!(result, 0);

        // Execute
        let input = CString::new("test input").unwrap();
        unsafe {
            let output = plugin_execute(handle, input.as_ptr());
            let output_str = CStr::from_ptr(output).to_str().unwrap();
            assert!(output_str.contains("test input"));

            // Free output string
            plugin_free_string(output);
        }

        // Check call count
        let count = plugin_get_call_count(handle);
        assert_eq!(count, 1);

        // Destroy
        plugin_destroy(handle);
    }

    #[test]
    #[serial]
    fn test_plugin_execute_multiple() {
        let handle = plugin_create();
        plugin_init(handle);

        for i in 0..5 {
            let input = CString::new(format!("input {}", i)).unwrap();
            unsafe {
                let output = plugin_execute(handle, input.as_ptr());
                let output_str = CStr::from_ptr(output).to_str().unwrap();
                assert!(output_str.contains(&format!("input {}", i)));
                plugin_free_string(output);
            }
        }

        let count = plugin_get_call_count(handle);
        assert_eq!(count, 5);

        plugin_destroy(handle);
    }
}
