//! LSP 服务器管理
//!
//! 管理 LSP 服务器的启动、停止和生命周期。

use super::LspError;
use super::LspResult;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 语言服务器配置
#[derive(Debug, Clone)]
pub struct LanguageServerConfig {
    /// 服务器名称
    pub name: String,
    /// 命令路径
    pub command: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 支持的文件扩展名
    pub extensions: Vec<String>,
    /// 初始化超时（秒）
    pub init_timeout_secs: u64,
}

impl LanguageServerConfig {
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
        extensions: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args,
            extensions,
            init_timeout_secs: 30,
        }
    }
}

/// 语言服务器实例
#[derive(Debug)]
pub struct LanguageServer {
    /// 配置
    pub config: LanguageServerConfig,
    /// 子进程
    pub process: Option<Child>,
    /// 根目录
    pub root_path: PathBuf,
}

impl LanguageServer {
    /// 启动服务器
    pub fn new(config: LanguageServerConfig, root_path: PathBuf) -> LspResult<Self> {
        Ok(Self {
            config,
            process: None,
            root_path,
        })
    }

    /// 启动服务器进程
    pub fn start(&mut self) -> LspResult<()> {
        let mut command = Command::new(&self.config.command);

        // 设置工作目录
        command.current_dir(&self.root_path);

        // 设置参数
        for arg in &self.config.args {
            command.arg(arg);
        }

        // 设置标准输入/输出
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command.spawn().map_err(|e| {
            LspError::ServerCrashed(format!("Failed to start {}: {}", self.config.name, e))
        })?;

        self.process = Some(child);
        tracing::info!("Started LSP server: {}", self.config.name);

        Ok(())
    }

    /// 停止服务器
    pub fn stop(&mut self) -> LspResult<()> {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            tracing::info!("Stopped LSP server: {}", self.config.name);
        }
        Ok(())
    }

    /// 获取配置
    pub fn config(&self) -> &LanguageServerConfig {
        &self.config
    }

    /// 获取进程 stdin
    pub fn stdin(&self) -> Option<&std::process::ChildStdin> {
        self.process.as_ref().and_then(|p| p.stdin.as_ref())
    }

    /// 获取进程 stdout
    pub fn stdout(&self) -> Option<&std::process::ChildStdout> {
        self.process.as_ref().and_then(|p| p.stdout.as_ref())
    }

    /// 检查服务器是否运行
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    /// 获取根路径
    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// 语言服务器管理器
pub struct LanguageServerManager {
    /// 已启动的服务器
    pub servers: Arc<Mutex<HashMap<String, LanguageServer>>>,
    /// 服务器配置
    configs: HashMap<String, LanguageServerConfig>,
}

impl LanguageServerManager {
    /// 创建新的管理器，使用默认配置
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        // Rust - rust-analyzer
        configs.insert(
            "rust".to_string(),
            LanguageServerConfig::new(
                "rust-analyzer",
                "rust-analyzer",
                vec![],
                vec!["rs".to_string()],
            ),
        );

        // Python - pyright
        configs.insert(
            "python".to_string(),
            LanguageServerConfig::new(
                "pyright",
                "pyright-langserver",
                vec!["--stdio".to_string()],
                vec!["py".to_string()],
            ),
        );

        // TypeScript/JavaScript
        configs.insert(
            "typescript".to_string(),
            LanguageServerConfig::new(
                "typescript-language-server",
                "typescript-language-server",
                vec!["--stdio".to_string()],
                vec![
                    "ts".to_string(),
                    "tsx".to_string(),
                    "js".to_string(),
                    "jsx".to_string(),
                ],
            ),
        );

        // Go
        configs.insert(
            "go".to_string(),
            LanguageServerConfig::new(
                "gopls",
                "gopls",
                vec!["serve".to_string()],
                vec!["go".to_string()],
            ),
        );

        // C/C++
        configs.insert(
            "cpp".to_string(),
            LanguageServerConfig::new(
                "clangd",
                "clangd",
                vec!["--background-index".to_string()],
                vec![
                    "c".to_string(),
                    "cpp".to_string(),
                    "h".to_string(),
                    "hpp".to_string(),
                ],
            ),
        );

        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
            configs,
        }
    }

    /// 根据文件扩展名获取语言名称
    pub fn get_language_from_extension(ext: &str) -> Option<&'static str> {
        match ext {
            "rs" => Some("rust"),
            "py" => Some("python"),
            "ts" | "tsx" => Some("typescript"),
            "js" | "jsx" => Some("typescript"), // JS 也用 typescript-language-server
            "go" => Some("go"),
            "c" | "cpp" | "h" | "hpp" => Some("cpp"),
            _ => None,
        }
    }

    /// 获取服务器配置
    pub fn get_config(&self, language: &str) -> Option<&LanguageServerConfig> {
        self.configs.get(language)
    }

    /// 启动服务器
    pub async fn start_server(&self, language: &str, root_path: PathBuf) -> LspResult<()> {
        let config = self
            .configs
            .get(language)
            .ok_or_else(|| LspError::ServerNotFound(language.to_string()))?
            .clone();

        let mut server = LanguageServer::new(config, root_path)?;
        server.start()?;

        let mut servers = self.servers.lock().await;
        servers.insert(language.to_string(), server);

        Ok(())
    }

    /// 获取服务器
    pub async fn get_server(&self, language: &str) -> Option<LanguageServerHandle> {
        let servers = self.servers.lock().await;
        servers.get(language).map(|_s| LanguageServerHandle {
            language: language.to_string(),
            servers: self.servers.clone(),
        })
    }

    /// 停止服务器
    pub async fn stop_server(&self, language: &str) -> LspResult<()> {
        let mut servers = self.servers.lock().await;
        if let Some(mut server) = servers.remove(language) {
            server.stop()?;
        }
        Ok(())
    }

    /// 停止所有服务器
    pub async fn stop_all(&self) -> LspResult<()> {
        let mut servers = self.servers.lock().await;
        for mut server in servers.drain().map(|(_, server)| server) {
            let _ = server.stop();
        }
        Ok(())
    }

    /// 检查服务器是否运行
    pub async fn is_running(&self, language: &str) -> bool {
        let servers = self.servers.lock().await;
        servers
            .get(language)
            .map(|s| s.is_running())
            .unwrap_or(false)
    }

    /// 添加自定义服务器配置
    pub fn add_custom_config(&mut self, language: String, config: LanguageServerConfig) {
        self.configs.insert(language, config);
    }
}

impl Default for LanguageServerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 语言服务器句柄（用于安全访问）
pub struct LanguageServerHandle {
    language: String,
    servers: Arc<Mutex<HashMap<String, LanguageServer>>>,
}

impl LanguageServerHandle {
    /// 获取服务器名称
    pub fn name(&self) -> String {
        self.language.clone()
    }

    /// 获取服务器配置
    pub async fn config(&self) -> Option<LanguageServerConfig> {
        let servers = self.servers.lock().await;
        servers.get(&self.language).map(|s| s.config().clone())
    }

    /// 获取根路径
    pub async fn root_path(&self) -> Option<PathBuf> {
        let servers = self.servers.lock().await;
        servers.get(&self.language).map(|s| s.root_path().clone())
    }
}

// ============================================================================
// 预定义的 LSP 服务器配置
// ============================================================================

/// 默认的 rust-analyzer 配置
pub fn rust_analyzer_config() -> LanguageServerConfig {
    LanguageServerConfig::new(
        "rust-analyzer",
        "rust-analyzer",
        vec!["--stdio".to_string()],
        vec!["rs".to_string()],
    )
}

/// 默认的 pyright 配置
pub fn pyright_config() -> LanguageServerConfig {
    LanguageServerConfig::new(
        "pyright",
        "pyright-langserver",
        vec!["--stdio".to_string()],
        vec!["py".to_string()],
    )
}

/// 默认的 TypeScript 语言服务器配置
pub fn typescript_config() -> LanguageServerConfig {
    LanguageServerConfig::new(
        "typescript-language-server",
        "typescript-language-server",
        vec!["--stdio".to_string()],
        vec![
            "ts".to_string(),
            "tsx".to_string(),
            "js".to_string(),
            "jsx".to_string(),
        ],
    )
}

/// 默认的 gopls 配置
pub fn gopls_config() -> LanguageServerConfig {
    LanguageServerConfig::new(
        "gopls",
        "gopls",
        vec!["serve".to_string(), "--stdio".to_string()],
        vec!["go".to_string()],
    )
}

/// 默认的 clangd 配置
pub fn clangd_config() -> LanguageServerConfig {
    LanguageServerConfig::new(
        "clangd",
        "clangd",
        vec![
            "--background-index".to_string(),
            "--header-insertion=iwyu".to_string(),
        ],
        vec![
            "c".to_string(),
            "cpp".to_string(),
            "h".to_string(),
            "hpp".to_string(),
        ],
    )
}

/// pylance 配置（VS Code Python 插件）
pub fn pylance_config() -> LanguageServerConfig {
    LanguageServerConfig::new(
        "pylance",
        "pylance",
        vec!["--stdio".to_string()],
        vec!["py".to_string()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = LanguageServerManager::new();
        assert!(manager.get_config("rust").is_some());
        assert!(manager.get_config("python").is_some());
        assert!(manager.get_config("typescript").is_some());
        assert!(manager.get_config("go").is_some());
        assert!(manager.get_config("cpp").is_some());
    }

    #[test]
    fn test_get_language_from_extension() {
        assert_eq!(
            LanguageServerManager::get_language_from_extension("rs"),
            Some("rust")
        );
        assert_eq!(
            LanguageServerManager::get_language_from_extension("py"),
            Some("python")
        );
        assert_eq!(
            LanguageServerManager::get_language_from_extension("ts"),
            Some("typescript")
        );
        assert_eq!(
            LanguageServerManager::get_language_from_extension("go"),
            Some("go")
        );
        assert_eq!(
            LanguageServerManager::get_language_from_extension("cpp"),
            Some("cpp")
        );
        assert_eq!(
            LanguageServerManager::get_language_from_extension("xyz"),
            None
        );
    }

    #[test]
    fn test_config_creation() {
        let config = rust_analyzer_config();
        assert_eq!(config.name, "rust-analyzer");
        assert_eq!(config.command, "rust-analyzer");
        assert_eq!(config.extensions, vec!["rs".to_string()]);
    }
}
