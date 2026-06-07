//! Dynamic Library Plugin Loader
//!
//! 使用 libloading 实现动态库 (.so/.dylib/.dll) 插件加载。

use super::PluginMeta;
use crate::types::Layer4Result;
use anyhow::{anyhow, Context};
use libloading::{Library, Symbol};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// 插件入口函数签名
/// 插件必须导出 `plugin_create` 函数返回 Plugin 实例
pub type PluginCreateFn = extern "C" fn() -> *mut ();

/// 插件名称函数签名
pub type PluginNameFn = extern "C" fn(*const ()) -> *const std::ffi::c_char;

/// 插件版本函数签名
pub type PluginVersionFn = extern "C" fn(*const ()) -> *const std::ffi::c_char;

/// 插件元数据函数签名
/// Note: PluginMeta contains String which is not FFI-safe by default,
/// but we accept this limitation for the plugin ABI design.
#[allow(improper_ctypes_definitions)]
pub type PluginMetaFn = extern "C" fn() -> PluginMeta;

/// 插件初始化函数签名
/// 接收插件句柄和JSON配置字符串，返回0表示成功，非0表示失败
pub type PluginInitializeFn = extern "C" fn(*mut (), *const std::ffi::c_char) -> i32;

/// 插件销毁函数签名
pub type PluginDestroyFn = extern "C" fn(*mut ());

/// 动态库插件加载器
pub struct DylibLoader {
    /// 已加载的库
    libraries: RwLock<HashMap<String, Arc<Library>>>,
    /// 插件句柄
    handles: RwLock<HashMap<String, *mut ()>>,
    /// 插件元数据
    metas: RwLock<HashMap<String, PluginMeta>>,
}

// SAFETY: DylibLoader 内部使用 RwLock 保护所有共享状态
unsafe impl Send for DylibLoader {}
unsafe impl Sync for DylibLoader {}

impl DylibLoader {
    /// 创建新的加载器
    pub fn new() -> Self {
        Self {
            libraries: RwLock::new(HashMap::new()),
            handles: RwLock::new(HashMap::new()),
            metas: RwLock::new(HashMap::new()),
        }
    }

    /// 检查文件是否为有效动态库
    pub fn is_valid_library(path: &Path) -> bool {
        if !path.exists() || !path.is_file() {
            return false;
        }

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext, "so" | "dylib" | "dll"))
            .unwrap_or(false)
    }

    /// 加载动态库插件
    ///
    /// # Safety
    ///
    /// 加载动态库是不安全的，因为:
    /// 1. 库代码可能在加载时执行任意代码
    /// 2. 符号可能不匹配预期签名
    /// 3. 库可能有数据竞争或内存安全问题
    pub unsafe fn load(&self, path: &Path) -> Layer4Result<(String, PluginMeta)> {
        let path_str = path.to_string_lossy().to_string();

        // 加载库
        let library =
            Library::new(path).with_context(|| format!("Failed to load library: {}", path_str))?;

        // 获取元数据
        let meta: Symbol<PluginMetaFn> = library
            .get(b"plugin_meta")
            .with_context(|| "Symbol 'plugin_meta' not found")?;
        let meta = meta();

        // 获取创建函数
        let create: Symbol<PluginCreateFn> = library
            .get(b"plugin_create")
            .with_context(|| "Symbol 'plugin_create' not found")?;

        // 创建插件实例
        let handle = create();

        let name = meta.name.clone();

        // 存储
        self.libraries
            .write()
            .insert(name.clone(), Arc::new(library));
        self.handles.write().insert(name.clone(), handle);
        self.metas.write().insert(name.clone(), meta.clone());

        tracing::info!(
            "Loaded dylib plugin: {} v{} from {}",
            meta.name,
            meta.version,
            path_str
        );

        Ok((name, meta))
    }

    /// 安全加载（带验证）
    pub fn load_safe(&self, path: &Path) -> Layer4Result<(String, PluginMeta)> {
        if !Self::is_valid_library(path) {
            return Err(anyhow!("Invalid library file: {:?}", path));
        }

        // SAFETY: 我们已验证文件扩展名，但仍需信任库代码
        unsafe { self.load(path) }
    }

    /// 获取插件名称
    pub fn get_name(&self, name: &str) -> Option<String> {
        let meta = self.metas.read().get(name).cloned()?;
        Some(meta.name)
    }

    /// 获取插件版本
    pub fn get_version(&self, name: &str) -> Option<String> {
        let meta = self.metas.read().get(name).cloned()?;
        Some(meta.version)
    }

    /// 获取插件元数据
    pub fn get_meta(&self, name: &str) -> Option<PluginMeta> {
        self.metas.read().get(name).cloned()
    }

    /// 调用插件的初始化函数
    ///
    /// 如果插件导出了 `plugin_initialize` 函数，则调用它。
    /// 返回 Some(true) 表示成功，Some(false) 表示失败，None 表示函数不存在。
    pub fn call_initialize(&self, name: &str, config_json: &str) -> Option<bool> {
        let library = self.libraries.read().get(name).cloned()?;
        let handle = *self.handles.read().get(name)?;

        // SAFETY: 我们持有有效的库和句柄
        unsafe {
            let init_fn: Symbol<PluginInitializeFn> = library.get(b"plugin_initialize").ok()?;
            let config_cstr = std::ffi::CString::new(config_json).ok()?;
            let result = init_fn(handle, config_cstr.as_ptr());
            Some(result == 0)
        }
    }

    /// 卸载插件
    pub fn unload(&self, name: &str) -> Layer4Result<()> {
        // 获取库和句柄
        let library = self.libraries.write().remove(name);
        let handle = self.handles.write().remove(name);
        self.metas.write().remove(name);

        if let (Some(library), Some(handle)) = (library, handle) {
            // 尝试调用销毁函数
            if let Ok(lib) = Arc::try_unwrap(library) {
                // SAFETY: 我们正在销毁插件
                unsafe {
                    if let Ok(destroy) = lib.get::<Symbol<PluginDestroyFn>>(b"plugin_destroy") {
                        destroy(handle);
                    }
                }
            }
        }

        tracing::info!("Unloaded dylib plugin: {}", name);
        Ok(())
    }

    /// 列出已加载的插件
    pub fn list(&self) -> Vec<String> {
        self.metas.read().keys().cloned().collect()
    }

    /// 检查插件是否已加载
    pub fn is_loaded(&self, name: &str) -> bool {
        self.metas.read().contains_key(name)
    }

    /// 获取插件数量
    pub fn count(&self) -> usize {
        self.metas.read().len()
    }
}

impl Default for DylibLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 插件创建宏（供插件开发者使用）
///
/// # 示例
///
/// ```rust,ignore
/// use sh_layer4::plugin_loader::{Plugin, PluginContext};
/// use sh_layer4::declare_plugin;
///
/// struct MyPlugin;
///
/// #[async_trait::async_trait]
/// impl Plugin for MyPlugin {
///     fn name(&self) -> &str { "my_plugin" }
///     fn version(&self) -> &str { "0.1.0" }
///     async fn initialize(&self, _ctx: &PluginContext) -> anyhow::Result<()> { Ok(()) }
///     async fn execute(&self, input: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
///         Ok(input.clone())
///     }
/// }
///
/// declare_plugin!(MyPlugin, "my_plugin", "0.1.0");
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $name:expr, $version:expr) => {
        static mut PLUGIN_INSTANCE: Option<Box<$plugin_type>> = None;

        /// 插件创建
        #[no_mangle]
        pub extern "C" fn plugin_create() -> *mut () {
            unsafe {
                PLUGIN_INSTANCE = Some(Box::new(<$plugin_type>::default()));
                PLUGIN_INSTANCE.as_mut().unwrap().as_mut() as *mut () as *mut ()
            }
        }

        /// 插件销毁
        #[no_mangle]
        pub extern "C" fn plugin_destroy(_ptr: *mut ()) {
            unsafe {
                PLUGIN_INSTANCE = None;
            }
        }

        /// 插件元数据
        #[no_mangle]
        pub extern "C" fn plugin_meta() -> $crate::plugin_loader::PluginMeta {
            $crate::plugin_loader::PluginMeta {
                name: $name.to_string(),
                version: $version.to_string(),
                ..Default::default()
            }
        }

        /// 插件初始化
        /// 接收JSON配置字符串，返回0表示成功，非0表示失败
        #[no_mangle]
        pub extern "C" fn plugin_initialize(
            _ptr: *mut (),
            config_json: *const std::ffi::c_char,
        ) -> i32 {
            // SAFETY: config_json should be a valid C string from the caller
            unsafe {
                if let Some(instance) = PLUGIN_INSTANCE.as_ref() {
                    // 尝试解析配置
                    let config_str = if config_json.is_null() {
                        "{}"
                    } else {
                        std::ffi::CStr::from_ptr(config_json)
                            .to_str()
                            .unwrap_or("{}")
                    };

                    // 插件可以在 Default 实现中处理初始化逻辑
                    // 对于简单的同步初始化，我们在这里执行
                    tracing::info!("Plugin {} initialized with config: {}", $name, config_str);
                    0 // 成功
                } else {
                    1 // 实例不存在
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dylib_loader_creation() {
        let loader = DylibLoader::new();
        assert!(loader.list().is_empty());
        assert_eq!(loader.count(), 0);
    }

    #[test]
    fn test_is_valid_library() {
        // 创建临时文件测试扩展名检查
        let tmp_so = tempfile::NamedTempFile::with_suffix(".so").unwrap();
        assert!(DylibLoader::is_valid_library(tmp_so.path()));

        let tmp_dll = tempfile::NamedTempFile::with_suffix(".dll").unwrap();
        assert!(DylibLoader::is_valid_library(tmp_dll.path()));

        let tmp_dylib = tempfile::NamedTempFile::with_suffix(".dylib").unwrap();
        assert!(DylibLoader::is_valid_library(tmp_dylib.path()));

        let tmp_txt = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        assert!(!DylibLoader::is_valid_library(tmp_txt.path()));

        // 不存在的文件
        assert!(!DylibLoader::is_valid_library(Path::new("/nonexistent.so")));
    }

    #[test]
    fn test_plugin_meta_default() {
        let meta = PluginMeta::default();
        assert_eq!(meta.name, "unknown");
        assert_eq!(meta.version, "0.1.0");
    }

    #[test]
    fn test_loader_default() {
        let loader = DylibLoader::default();
        assert!(loader.list().is_empty());
    }
}
