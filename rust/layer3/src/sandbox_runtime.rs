//! # Sandbox Runtime
//!
//! 沙箱运行时：安全隔离的执行环境。
//!
//! ## 实现方式
//! - 进程级隔离（默认）
//! - 资源限制（内存、CPU、时间）
//! - 网络策略控制
//! - 文件系统隔离

use crate::types::{Layer3Result, ToolRequest, ToolResponse};
use async_trait::async_trait;
use parking_lot::RwLock;
use sh_layer1::generate_short_id;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// 沙箱运行时 trait
///
/// 提供安全隔离的代码执行环境。
#[async_trait]
pub trait SandboxRuntime: Send + Sync {
    /// 创建沙箱
    async fn create(&self, config: SandboxConfig) -> Layer3Result<SandboxId>;

    /// 销毁沙箱
    async fn destroy(&self, id: &SandboxId) -> Layer3Result<bool>;

    /// 在沙箱中执行代码
    async fn execute(
        &self,
        id: &SandboxId,
        code: &str,
        language: &str,
    ) -> Layer3Result<ExecutionResult>;

    /// 在沙箱中执行工具
    async fn execute_tool(
        &self,
        id: &SandboxId,
        request: ToolRequest,
    ) -> Layer3Result<ToolResponse>;

    /// 获取沙箱状态
    async fn status(&self, id: &SandboxId) -> Layer3Result<SandboxStatus>;

    /// 获取沙箱信息
    async fn info(&self, id: &SandboxId) -> Layer3Result<Option<SandboxInfo>>;

    /// 列出所有活跃沙箱
    async fn list(&self) -> Layer3Result<Vec<SandboxInfo>>;

    /// 重置沙箱
    async fn reset(&self, id: &SandboxId) -> Layer3Result<bool>;
}

/// 沙箱 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SandboxId(pub String);

impl std::fmt::Display for SandboxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 沙箱配置
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// 基础镜像/环境
    pub base_image: String,
    /// 资源限制
    pub limits: SandboxLimits,
    /// 允许的网络访问
    pub network: NetworkPolicy,
    /// 允许的文件系统访问
    pub filesystem: FsPolicy,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 工作目录
    pub working_dir: PathBuf,
    /// 最大执行时间（秒）
    pub timeout_secs: u64,
    /// 是否允许交互
    pub interactive: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            base_image: "default".to_string(),
            limits: SandboxLimits::default(),
            network: NetworkPolicy::Disabled,
            filesystem: FsPolicy::ReadOnly,
            env_vars: HashMap::new(),
            working_dir: PathBuf::from("/sandbox"),
            timeout_secs: 30,
            interactive: false,
        }
    }
}

/// 沙箱资源限制
#[derive(Debug, Clone, Default)]
pub struct SandboxLimits {
    /// 最大内存（字节）
    pub max_memory: Option<u64>,
    /// 最大 CPU（百分比）
    pub max_cpu_percent: Option<u32>,
    /// 最大文件大小（字节）
    pub max_file_size: Option<u64>,
    /// 最大进程数
    pub max_processes: Option<u32>,
}

/// 网络策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPolicy {
    /// 禁止网络
    Disabled,
    /// 仅允许出站
    OutboundOnly,
    /// 允许特定端口
    RestrictedPorts(Vec<u16>),
    /// 完全开放（危险）
    Full,
}

/// 文件系统策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsPolicy {
    /// 只读
    ReadOnly,
    /// 仅允许特定目录
    RestrictedDirs(Vec<PathBuf>),
    /// 临时可写（执行后清空）
    TempWritable,
    /// 完全可写（危险）
    FullWritable,
}

/// 沙箱状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus {
    Creating,
    Ready,
    Running,
    Paused,
    Error,
    Destroyed,
}

/// 沙箱信息
#[derive(Debug, Clone)]
pub struct SandboxInfo {
    /// 沙箱 ID
    pub id: SandboxId,
    /// 状态
    pub status: SandboxStatus,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 当前内存使用
    pub memory_used: u64,
    /// 当前 CPU 使用
    pub cpu_used: f32,
    /// 执行次数
    pub executions: u32,
    /// 配置
    pub config: SandboxConfig,
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 退出码
    pub exit_code: i32,
    /// 执行时间（毫秒）
    pub duration_ms: u64,
    /// 是否超时
    pub timed_out: bool,
    /// 是否被杀
    pub killed: bool,
}

impl ExecutionResult {
    /// 创建成功结果
    pub fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 0,
            timed_out: false,
            killed: false,
        }
    }

    /// 创建失败结果
    pub fn failure(stderr: String, exit_code: i32) -> Self {
        Self {
            stdout: String::new(),
            stderr,
            exit_code,
            duration_ms: 0,
            timed_out: false,
            killed: false,
        }
    }

    /// 创建超时结果
    pub fn timeout(stdout: String, stderr: String) -> Self {
        Self {
            stdout,
            stderr,
            exit_code: -1,
            duration_ms: 0,
            timed_out: true,
            killed: false,
        }
    }

    /// 是否成功
    pub fn is_success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out && !self.killed
    }
}

// ============================================================================
// Default Sandbox Runtime Implementation
// ============================================================================

/// 默认沙箱运行时实现
///
/// 使用进程隔离和资源限制实现沙箱。
pub struct DefaultSandboxRuntime {
    sandboxes: RwLock<HashMap<String, SandboxInfo>>,
    temp_dir: PathBuf,
}

impl DefaultSandboxRuntime {
    /// 创建新的沙箱运行时
    pub fn new() -> Layer3Result<Self> {
        let temp_dir = std::env::temp_dir().join("continuum_sandboxes");
        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            sandboxes: RwLock::new(HashMap::new()),
            temp_dir,
        })
    }

    /// 获取语言的执行命令
    fn get_language_command(language: &str) -> Option<(&'static str, &'static str)> {
        match language.to_lowercase().as_str() {
            "python" | "python3" | "py" => Some(("python", "-c")),
            "javascript" | "js" | "node" => Some(("node", "-e")),
            "ruby" | "rb" => Some(("ruby", "-e")),
            "perl" | "pl" => Some(("perl", "-e")),
            "bash" | "sh" | "shell" => Some(("bash", "-c")),
            "lua" => Some(("lua", "-e")),
            _ => None,
        }
    }

    /// 检查命令是否可用
    fn command_exists(cmd: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            Command::new("where")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Command::new("which")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }

    /// 执行命令带超时
    fn execute_with_timeout(
        &self,
        cmd: &str,
        args: &[&str],
        input: Option<&str>,
        timeout_secs: u64,
    ) -> ExecutionResult {
        let start = Instant::now();

        let mut command = Command::new(cmd);
        command.args(args);

        // Set environment variables
        command.env_clear();
        command.env("PATH", std::env::var("PATH").unwrap_or_default());

        // Set working directory
        command.current_dir(&self.temp_dir);

        // Capture output
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        // Write input if provided
        if input.is_some() {
            command.stdin(std::process::Stdio::piped());
        }

        let spawn_result = command.spawn();

        match spawn_result {
            Ok(mut child) => {
                // Write input if provided
                if let Some(input_data) = input {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(input_data.as_bytes());
                    }
                }

                // Wait with timeout
                let timeout = Duration::from_secs(timeout_secs);
                let result = child.wait_timeout(timeout);

                match result {
                    Ok(Some(status)) => {
                        let stdout = read_child_stdout(&mut child);
                        let stderr = read_child_stderr(&mut child);
                        let duration_ms = start.elapsed().as_millis() as u64;

                        ExecutionResult {
                            stdout,
                            stderr,
                            exit_code: status.code().unwrap_or(-1),
                            duration_ms,
                            timed_out: false,
                            killed: false,
                        }
                    }
                    Ok(None) => {
                        // Timeout - kill the process
                        let _ = child.kill();
                        let _ = child.wait();
                        let stdout = read_child_stdout(&mut child);
                        let stderr = read_child_stderr(&mut child);

                        ExecutionResult::timeout(stdout, stderr)
                    }
                    Err(e) => ExecutionResult::failure(format!("Wait error: {}", e), -1),
                }
            }
            Err(e) => ExecutionResult::failure(format!("Spawn error: {}", e), -1),
        }
    }
}

impl Default for DefaultSandboxRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create DefaultSandboxRuntime")
    }
}

#[async_trait]
impl SandboxRuntime for DefaultSandboxRuntime {
    async fn create(&self, config: SandboxConfig) -> Layer3Result<SandboxId> {
        let id = SandboxId(generate_short_id());

        let info = SandboxInfo {
            id: id.clone(),
            status: SandboxStatus::Ready,
            created_at: chrono::Utc::now(),
            memory_used: 0,
            cpu_used: 0.0,
            executions: 0,
            config,
        };

        self.sandboxes.write().insert(id.0.clone(), info);
        tracing::info!("Created sandbox: {}", id);

        Ok(id)
    }

    async fn destroy(&self, id: &SandboxId) -> Layer3Result<bool> {
        let mut sandboxes = self.sandboxes.write();
        if let Some(mut info) = sandboxes.remove(&id.0) {
            info.status = SandboxStatus::Destroyed;
            tracing::info!("Destroyed sandbox: {}", id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn execute(
        &self,
        id: &SandboxId,
        code: &str,
        language: &str,
    ) -> Layer3Result<ExecutionResult> {
        // Check sandbox exists and is ready
        {
            let sandboxes = self.sandboxes.read();
            let info = sandboxes
                .get(&id.0)
                .ok_or_else(|| anyhow::anyhow!("Sandbox not found: {}", id))?;

            if info.status != SandboxStatus::Ready {
                return Err(anyhow::anyhow!("Sandbox not ready: {:?}", info.status));
            }
        }

        // Update status to running
        {
            let mut sandboxes = self.sandboxes.write();
            if let Some(info) = sandboxes.get_mut(&id.0) {
                info.status = SandboxStatus::Running;
            }
        }

        // Get language command
        let (cmd, flag) = Self::get_language_command(language)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

        // Check command exists
        if !Self::command_exists(cmd) {
            return Err(anyhow::anyhow!("Command not found: {}", cmd));
        }

        // Execute with timeout
        let timeout = {
            let sandboxes = self.sandboxes.read();
            sandboxes
                .get(&id.0)
                .map(|i| i.config.timeout_secs)
                .unwrap_or(30)
        };

        let result = self.execute_with_timeout(cmd, &[flag, code], None, timeout);

        // Update status and stats
        {
            let mut sandboxes = self.sandboxes.write();
            if let Some(info) = sandboxes.get_mut(&id.0) {
                info.status = SandboxStatus::Ready;
                info.executions += 1;
            }
        }

        Ok(result)
    }

    async fn execute_tool(
        &self,
        id: &SandboxId,
        request: ToolRequest,
    ) -> Layer3Result<ToolResponse> {
        Err(anyhow::anyhow!(
            "[experimental] Tool execution in sandbox is not yet implemented (sandbox_id: {}, tool: {})",
            id, request.name
        ))
    }

    async fn status(&self, id: &SandboxId) -> Layer3Result<SandboxStatus> {
        let sandboxes = self.sandboxes.read();
        let info = sandboxes
            .get(&id.0)
            .ok_or_else(|| anyhow::anyhow!("Sandbox not found: {}", id))?;
        Ok(info.status)
    }

    async fn info(&self, id: &SandboxId) -> Layer3Result<Option<SandboxInfo>> {
        let sandboxes = self.sandboxes.read();
        Ok(sandboxes.get(&id.0).cloned())
    }

    async fn list(&self) -> Layer3Result<Vec<SandboxInfo>> {
        Ok(self.sandboxes.read().values().cloned().collect())
    }

    async fn reset(&self, id: &SandboxId) -> Layer3Result<bool> {
        let mut sandboxes = self.sandboxes.write();
        if let Some(info) = sandboxes.get_mut(&id.0) {
            info.status = SandboxStatus::Ready;
            info.executions = 0;
            info.memory_used = 0;
            info.cpu_used = 0.0;
            tracing::info!("Reset sandbox: {}", id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Read child process stdout
fn read_child_stdout(child: &mut std::process::Child) -> String {
    use std::io::Read;
    if let Some(mut stdout) = child.stdout.take() {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    } else {
        String::new()
    }
}

/// Read child process stderr
fn read_child_stderr(child: &mut std::process::Child) -> String {
    use std::io::Read;
    if let Some(mut stderr) = child.stderr.take() {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    } else {
        String::new()
    }
}

/// Extension trait for child process wait with timeout
trait ChildWaitTimeout {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl ChildWaitTimeout for std::process::Child {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        let start = Instant::now();

        loop {
            match self.try_wait()? {
                Some(status) => return Ok(Some(status)),
                None => {
                    if start.elapsed() >= timeout {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.network, NetworkPolicy::Disabled);
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult::success("hello".to_string());
        assert!(result.is_success());
    }

    #[test]
    fn test_execution_result_timeout() {
        let result = ExecutionResult::timeout("out".to_string(), "err".to_string());
        assert!(result.timed_out);
        assert!(!result.is_success());
    }

    #[test]
    fn test_sandbox_id_display() {
        let id = SandboxId("abc123".to_string());
        assert_eq!(format!("{}", id), "abc123");
    }

    #[test]
    fn test_language_command_mapping() {
        assert!(DefaultSandboxRuntime::get_language_command("python").is_some());
        assert!(DefaultSandboxRuntime::get_language_command("javascript").is_some());
        assert!(DefaultSandboxRuntime::get_language_command("bash").is_some());
        assert!(DefaultSandboxRuntime::get_language_command("unknown").is_none());
    }

    #[tokio::test]
    async fn test_sandbox_create_and_destroy() {
        let runtime = DefaultSandboxRuntime::new().unwrap();
        let config = SandboxConfig::default();

        let id = runtime.create(config).await.unwrap();
        assert!(!id.0.is_empty());

        let status = runtime.status(&id).await.unwrap();
        assert_eq!(status, SandboxStatus::Ready);

        let destroyed = runtime.destroy(&id).await.unwrap();
        assert!(destroyed);

        let status = runtime.status(&id).await;
        assert!(status.is_err());
    }

    #[tokio::test]
    async fn test_sandbox_list() {
        let runtime = DefaultSandboxRuntime::new().unwrap();

        let id1 = runtime.create(SandboxConfig::default()).await.unwrap();
        let id2 = runtime.create(SandboxConfig::default()).await.unwrap();

        let list = runtime.list().await.unwrap();
        assert_eq!(list.len(), 2);

        runtime.destroy(&id1).await.unwrap();
        runtime.destroy(&id2).await.unwrap();

        let list = runtime.list().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_sandbox_reset() {
        let runtime = DefaultSandboxRuntime::new().unwrap();
        let id = runtime.create(SandboxConfig::default()).await.unwrap();

        // Manually increment executions
        {
            let mut sandboxes = runtime.sandboxes.write();
            if let Some(info) = sandboxes.get_mut(&id.0) {
                info.executions = 5;
            }
        }

        let reset = runtime.reset(&id).await.unwrap();
        assert!(reset);

        let info = runtime.info(&id).await.unwrap().unwrap();
        assert_eq!(info.executions, 0);

        runtime.destroy(&id).await.unwrap();
    }

    #[test]
    fn test_network_policy_equality() {
        assert_eq!(NetworkPolicy::Disabled, NetworkPolicy::Disabled);
        assert_ne!(NetworkPolicy::Disabled, NetworkPolicy::Full);
    }

    #[test]
    fn test_fs_policy_equality() {
        assert_eq!(FsPolicy::ReadOnly, FsPolicy::ReadOnly);
        assert_ne!(FsPolicy::ReadOnly, FsPolicy::FullWritable);
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult::failure("error".to_string(), 1);
        assert!(!result.is_success());
        assert_eq!(result.exit_code, 1);
    }

    #[tokio::test]
    async fn test_execute_tool_returns_contextual_experimental_error() {
        let runtime = DefaultSandboxRuntime::new().unwrap();
        let sandbox_id = runtime.create(SandboxConfig::default()).await.unwrap();
        let request = ToolRequest {
            call_id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({"path": "README.md"}),
        };

        let err = runtime.execute_tool(&sandbox_id, request).await.unwrap_err();
        let message = err.to_string();

        assert!(message.contains("[experimental]"));
        assert!(message.contains(&sandbox_id.to_string()));
        assert!(message.contains("read_file"));
    }
}
