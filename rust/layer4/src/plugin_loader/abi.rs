//! Stable ABI for Plugin Interface
//!
//! Uses abi_stable to provide a stable ABI across Rust versions.

use abi_stable::std_types::RString;
use abi_stable::StableAbi;
use serde::{Deserialize, Serialize};

/// Plugin metadata with stable ABI
#[repr(C)]
#[derive(Debug, Clone, StableAbi, Serialize, Deserialize)]
pub struct StablePluginMeta {
    pub name: RString,
    pub version: RString,
    pub author: RString,
    pub description: RString,
}

impl Default for StablePluginMeta {
    fn default() -> Self {
        Self {
            name: RString::from("unknown"),
            version: RString::from("0.1.0"),
            author: RString::from("unknown"),
            description: RString::new(),
        }
    }
}

impl From<super::PluginMeta> for StablePluginMeta {
    fn from(meta: super::PluginMeta) -> Self {
        Self {
            name: RString::from(meta.name),
            version: RString::from(meta.version),
            author: RString::from(meta.author),
            description: RString::from(meta.description),
        }
    }
}

impl From<StablePluginMeta> for super::PluginMeta {
    fn from(meta: StablePluginMeta) -> Self {
        Self {
            name: meta.name.into_string(),
            version: meta.version.into_string(),
            author: meta.author.into_string(),
            description: meta.description.into_string(),
            ..Default::default()
        }
    }
}

/// Plugin entry point (exported by plugin)
#[repr(C)]
pub struct PluginEntryPoint {
    /// Create plugin instance
    pub create: extern "C" fn() -> *mut std::ffi::c_void,
    /// Destroy plugin instance
    pub destroy: extern "C" fn(*mut std::ffi::c_void),
    /// Get plugin metadata
    pub meta: extern "C" fn() -> StablePluginMeta,
}

/// Macro for creating stable ABI plugin exports
#[macro_export]
macro_rules! declare_stable_plugin {
    ($plugin_type:ty, $name:expr, $version:expr) => {
        static mut PLUGIN_INSTANCE: Option<Box<$plugin_type>> = None;

        #[no_mangle]
        pub extern "C" fn plugin_create() -> *mut std::ffi::c_void {
            unsafe {
                PLUGIN_INSTANCE = Some(Box::new(<$plugin_type>::default()));
                PLUGIN_INSTANCE.as_mut().unwrap().as_mut() as *mut _ as *mut std::ffi::c_void
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_destroy(_ptr: *mut std::ffi::c_void) {
            unsafe {
                PLUGIN_INSTANCE = None;
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_meta() -> $crate::plugin_loader::abi::StablePluginMeta {
            $crate::plugin_loader::abi::StablePluginMeta {
                name: abi_stable::std_types::RString::from($name),
                version: abi_stable::std_types::RString::from($version),
                ..Default::default()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_plugin_meta_default() {
        let meta = StablePluginMeta::default();
        assert_eq!(meta.name.as_str(), "unknown");
        assert_eq!(meta.version.as_str(), "0.1.0");
    }

    #[test]
    fn test_stable_plugin_meta_conversion() {
        let original = super::super::PluginMeta {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            description: "test plugin".to_string(),
            ..Default::default()
        };

        let stable: StablePluginMeta = original.clone().into();
        assert_eq!(stable.name.as_str(), "test");

        let back: super::super::PluginMeta = stable.into();
        assert_eq!(back.name, "test");
        assert_eq!(back.version, "1.0.0");
    }
}
