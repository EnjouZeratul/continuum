//! # CLI Application
//!
//! CLI 应用状态和生命周期管理。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use super::args::Commands;
use crate::integration::audit::AuditService;
use crate::integration::mcp::McpService;

/// CLI 应用配置
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// 配置文件路径
    pub config_path: Option<PathBuf>,
    /// 是否启用调试模式
    pub debug: bool,
    /// 工作目录
    pub working_dir: PathBuf,
    /// 是否启用审计
    pub enable_audit: bool,
    /// 是否启用 MCP
    pub enable_mcp: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            config_path: None,
            debug: false,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            enable_audit: true,
            enable_mcp: false,
        }
    }
}

impl CliConfig {
    /// 创建新的配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置配置文件路径
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// 启用调试模式
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// 设置工作目录
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = dir.into();
        self
    }

    /// 设置审计开关
    pub fn with_audit(mut self, enable: bool) -> Self {
        self.enable_audit = enable;
        self
    }

    /// 设置 MCP 开关
    pub fn with_mcp(mut self, enable: bool) -> Self {
        self.enable_mcp = enable;
        self
    }
}

/// CLI 应用状态
pub struct CliApp {
    /// 应用配置
    config: CliConfig,
    /// 当前命令
    command: Commands,
    /// 是否正在运行
    running: bool,
    /// 会话状态（用于持久化）
    session_state: Arc<RwLock<SessionState>>,
    /// 审计服务
    audit_service: Option<Arc<AuditService>>,
    /// MCP 服务
    mcp_service: Option<Arc<McpService>>,
}

/// 会话状态
#[derive(Debug, Default, Clone)]
pub struct SessionState {
    /// 当前会话 ID
    pub current_session_id: Option<String>,
    /// 用户 ID
    pub user_id: String,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            current_session_id: None,
            user_id: whoami::username(),
            created_at: chrono::Utc::now(),
        }
    }
}

impl CliApp {
    /// 创建新的 CLI 应用
    ///
    /// 如果 command 为 None，默认启动 TUI
    pub fn new(command: Option<Commands>, config: CliConfig) -> Self {
        // 处理默认行为：无参数时进入TUI
        let command = command.unwrap_or(Commands::Tui { session: None });

        let audit_service = if config.enable_audit {
            Some(Arc::new(AuditService::new()))
        } else {
            None
        };

        let mcp_service = if config.enable_mcp {
            Some(Arc::new(McpService::new()))
        } else {
            None
        };

        Self {
            config,
            command,
            running: true,
            session_state: Arc::new(RwLock::new(SessionState::new())),
            audit_service,
            mcp_service,
        }
    }

    /// 运行应用
    pub async fn run(&mut self) -> Result<()> {
        if self.config.debug {
            tracing::debug!("Debug mode enabled");
        }

        // 记录启动审计
        if let Some(ref audit) = self.audit_service {
            let state = self.session_state.read().await;
            use sh_core::layer4::AuditAction;
            audit
                .log_success(&state.user_id, AuditAction::Login, "cli", None)
                .await?;
        }

        // 分发到对应的子命令处理器
        let result = match &self.command {
            Commands::Run {
                task,
                session,
                budget,
                non_interactive,
            } => {
                self.run_command(task.clone(), session.clone(), *budget, *non_interactive)
                    .await
            }
            Commands::Session { cmd } => self.session_command(cmd.clone()).await,
            Commands::Config { cmd } => self.config_command(cmd.clone()).await,
            Commands::Tools { filter, verbose } => {
                self.tools_command(filter.clone(), *verbose).await
            }
            Commands::Tui { session } => self.tui_command(session.clone()).await,
            Commands::Checkpoint { cmd } => self.checkpoint_command(cmd.clone()).await,
            Commands::Dashboard { cmd } => self.dashboard_command(cmd.clone()).await,

            // 工具链命令
            Commands::Bash {
                command,
                cwd,
                timeout,
                capture_stderr,
            } => {
                self.bash_command(command.clone(), cwd.clone(), *timeout, *capture_stderr)
                    .await
            }
            Commands::Read {
                file,
                offset,
                limit,
                line_numbers,
            } => {
                self.read_command(file.clone(), *offset, *limit, *line_numbers)
                    .await
            }
            Commands::Write {
                file,
                content,
                append,
                backup,
            } => {
                self.write_command(file.clone(), content.clone(), *append, *backup)
                    .await
            }
            Commands::Edit {
                file,
                old,
                new,
                replace_all,
            } => {
                self.edit_command(file.clone(), old.clone(), new.clone(), *replace_all)
                    .await
            }
            Commands::Grep {
                pattern,
                path,
                glob,
                ignore_case,
                line_numbers,
                context,
            } => {
                self.grep_command(
                    pattern.clone(),
                    path.clone(),
                    glob.clone(),
                    *ignore_case,
                    *line_numbers,
                    *context,
                )
                .await
            }
            Commands::Glob { pattern, path } => {
                self.glob_command(pattern.clone(), path.clone()).await
            }
            Commands::Lsp { cmd } => self.lsp_command(cmd.clone()).await,

            // Git 命令
            Commands::Git { cmd } => self.git_command(cmd.clone()).await,
        };

        // 记录退出审计
        if let Some(ref audit) = self.audit_service {
            let state = self.session_state.read().await;
            use sh_core::layer4::AuditAction;
            audit
                .log_success(&state.user_id, AuditAction::Logout, "cli", None)
                .await?;
        }

        self.running = false;
        result
    }

    /// 执行 run 子命令
    async fn run_command(
        &self,
        task: Option<String>,
        session: Option<String>,
        budget: Option<f64>,
        non_interactive: bool,
    ) -> Result<()> {
        use crate::commands::run;

        // 更新会话状态
        if let Some(ref session_id) = session {
            let mut state = self.session_state.write().await;
            state.current_session_id = Some(session_id.clone());
        }

        // 记录执行审计
        if let Some(ref audit) = self.audit_service {
            let state = self.session_state.read().await;
            use sh_core::layer4::AuditAction;
            audit
                .log_success(
                    &state.user_id,
                    AuditAction::Execute,
                    "task",
                    task.as_deref(),
                )
                .await?;
        }

        run::execute(
            task,
            self.config
                .config_path
                .as_ref()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string()),
            budget,
            self.config.debug,
            non_interactive,
        )
    }

    /// 执行 session 子命令
    async fn session_command(&self, cmd: super::args::SessionCmd) -> Result<()> {
        use crate::commands::session;
        session::execute(cmd)
    }

    /// 执行 config 子命令
    async fn config_command(&self, cmd: super::args::ConfigCmd) -> Result<()> {
        use crate::commands::config;
        config::execute(cmd)
    }

    /// 执行 tools 子命令
    async fn tools_command(&self, filter: Option<String>, verbose: bool) -> Result<()> {
        use crate::commands::tools;

        // 如果启用 MCP，显示 MCP 工具
        if let Some(ref mcp) = self.mcp_service {
            let mcp_tools = mcp.list_tools().await?;
            println!("MCP Tools:");
            for tool in mcp_tools {
                if verbose {
                    println!("  {} [mcp] - {}", tool.name, tool.description);
                } else {
                    println!("  {} - {}", tool.name, tool.description);
                }
            }
        }

        tools::execute(filter, verbose)
    }

    /// 执行 tui 子命令
    async fn tui_command(&self, session: Option<String>) -> Result<()> {
        use crate::tui;
        tui::run_with_session(session)
    }

    /// 执行 checkpoint 子命令
    /// [EXPERIMENTAL] checkpoint 命令尚未完全实现
    async fn checkpoint_command(&self, cmd: super::args::CheckpointCmd) -> Result<()> {
        // checkpoint 命令在 commands 模块中实现
        match cmd {
            super::args::CheckpointCmd::List { session } => {
                println!("Listing checkpoints for session: {:?}", session);
                println!("[EXPERIMENTAL] Checkpoint listing not fully implemented");
            }
            super::args::CheckpointCmd::Restore { checkpoint_id } => {
                println!("Restoring checkpoint: {}", checkpoint_id);
                println!("[EXPERIMENTAL] Checkpoint restore not fully implemented");
            }
            super::args::CheckpointCmd::Delete { checkpoint_id } => {
                println!("Deleting checkpoint: {}", checkpoint_id);
                println!("[EXPERIMENTAL] Checkpoint delete not fully implemented");
            }
        }
        Ok(())
    }

    /// 执行 dashboard 子命令
    async fn dashboard_command(&self, cmd: super::args::DashboardCmd) -> Result<()> {
        use crate::commands::dashboard;
        dashboard::execute(cmd)
    }

    // ===== 工具链命令处理 =====

    /// 执行 bash 命令
    async fn bash_command(
        &self,
        command: String,
        cwd: Option<String>,
        timeout: u64,
        capture_stderr: bool,
    ) -> Result<()> {
        use crate::commands::tool_exec;

        let result = tool_exec::execute_bash(&command, cwd.as_deref(), timeout, capture_stderr)?;

        println!("{}", result.stdout);
        if capture_stderr && !result.stderr.is_empty() {
            eprintln!("{}", result.stderr);
        }

        println!("\n---");
        println!("Exit code: {}", result.exit_code);
        println!("Duration: {:?}", result.duration);
        if result.timed_out {
            println!("⚠️ Command timed out");
        }

        if result.exit_code != 0 {
            std::process::exit(result.exit_code);
        }
        Ok(())
    }

    /// 执行 read 命令
    async fn read_command(
        &self,
        file: String,
        offset: Option<usize>,
        limit: Option<usize>,
        line_numbers: bool,
    ) -> Result<()> {
        use crate::commands::tool_exec;

        let result = tool_exec::execute_read(&file, offset, limit, line_numbers)?;
        println!("{}", result);
        Ok(())
    }

    /// 执行 write 命令
    async fn write_command(
        &self,
        file: String,
        content: Option<String>,
        append: bool,
        backup: bool,
    ) -> Result<()> {
        use crate::commands::tool_exec;

        // 如果没有提供内容，从 stdin 读取
        let content = if let Some(c) = content {
            c
        } else {
            use std::io::{self, Read};
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        };

        let result = tool_exec::execute_write(&file, Some(&content), append, backup)?;
        println!("{}", result);
        Ok(())
    }

    /// 执行 edit 命令
    async fn edit_command(
        &self,
        file: String,
        old: String,
        new: String,
        replace_all: bool,
    ) -> Result<()> {
        use crate::commands::tool_exec;

        let result = tool_exec::execute_edit(&file, &old, &new, replace_all)?;
        println!("{}", result);
        Ok(())
    }

    /// 执行 grep 命令
    async fn grep_command(
        &self,
        pattern: String,
        path: String,
        glob: Option<String>,
        ignore_case: bool,
        line_numbers: bool,
        context: Option<usize>,
    ) -> Result<()> {
        use crate::commands::tool_exec;

        let results = tool_exec::execute_grep(
            &pattern,
            &path,
            glob.as_deref(),
            ignore_case,
            line_numbers,
            context,
        )?;

        if results.is_empty() {
            println!("No matches found");
        } else {
            for m in results {
                if line_numbers {
                    println!("{}:{}:{}", m.file, m.line_number, m.line);
                } else {
                    println!("{}:{}", m.file, m.line);
                }
            }
        }

        Ok(())
    }

    /// 执行 glob 命令
    async fn glob_command(&self, pattern: String, path: String) -> Result<()> {
        use crate::commands::tool_exec;

        let results = tool_exec::execute_glob(&pattern, &path)?;

        if results.is_empty() {
            println!("No files found matching pattern: {}", pattern);
        } else {
            for file in results {
                println!("{}", file);
            }
        }

        Ok(())
    }

    /// 执行 LSP 命令
    /// [EXPERIMENTAL] LSP 集成尚未完成
    async fn lsp_command(&self, cmd: super::args::LspCmd) -> Result<()> {
        match cmd {
            super::args::LspCmd::Definition { file, line, column } => {
                println!("Finding definition in {} at {}:{}", file, line, column);
                println!("[EXPERIMENTAL] LSP definition lookup not yet implemented");
            }
            super::args::LspCmd::References { file, line, column } => {
                println!("Finding references in {} at {}:{}", file, line, column);
                println!("[EXPERIMENTAL] LSP references lookup not yet implemented");
            }
            super::args::LspCmd::Hover { file, line, column } => {
                println!("Getting hover info in {} at {}:{}", file, line, column);
                println!("[EXPERIMENTAL] LSP hover not yet implemented");
            }
            super::args::LspCmd::Symbols { file } => {
                println!("Listing symbols in {}", file);
                println!("[EXPERIMENTAL] LSP symbols not yet implemented");
            }
        }

        Ok(())
    }

    // ===== Git 命令处理 =====

    /// 执行 Git 命令
    async fn git_command(&self, cmd: super::args::GitCmd) -> Result<()> {
        use crate::git::GitCommands;

        let git = GitCommands::from_cwd().map_err(|e| anyhow::anyhow!("Git error: {}", e))?;

        match cmd {
            super::args::GitCmd::Status { short } => {
                if short {
                    println!("{}", git.status_short()?);
                } else {
                    println!("{}", git.status()?);
                }
            }

            super::args::GitCmd::Diff { staged, paths } => {
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                println!("{}", git.diff(staged, &path_refs)?);
            }

            super::args::GitCmd::Commit {
                message,
                amend,
                add_all,
            } => {
                if add_all {
                    git.add(&["."])?;
                }
                let result = git.commit(message.as_deref(), amend)?;
                println!("{}", result);
            }

            super::args::GitCmd::Add { paths } => {
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                git.add(&path_refs)?;
                println!("Staged {} file(s)", paths.len());
            }

            super::args::GitCmd::Branch { cmd } => {
                self.git_branch_command(&git, cmd).await?;
            }

            super::args::GitCmd::Pr { cmd } => {
                self.git_pr_command(&git, cmd).await?;
            }
        }

        Ok(())
    }

    /// 执行 Git Branch 子命令
    async fn git_branch_command(
        &self,
        git: &crate::git::GitCommands,
        cmd: super::args::GitBranchCmd,
    ) -> Result<()> {
        match cmd {
            super::args::GitBranchCmd::List { all } => {
                println!("{}", git.branch_list(all)?);
            }

            super::args::GitBranchCmd::Create { name, switch } => {
                git.branch_create(&name, switch)?;
                if switch {
                    println!("Created and switched to branch: {}", name);
                } else {
                    println!("Created branch: {}", name);
                }
            }

            super::args::GitBranchCmd::Switch { name } => {
                git.branch_switch(&name)?;
                println!("Switched to branch: {}", name);
            }

            super::args::GitBranchCmd::Delete { name, force } => {
                git.branch_delete(&name, force)?;
                println!("Deleted branch: {}", name);
            }
        }

        Ok(())
    }

    /// 执行 Git PR 子命令
    async fn git_pr_command(
        &self,
        git: &crate::git::GitCommands,
        cmd: super::args::GitPrCmd,
    ) -> Result<()> {
        match cmd {
            super::args::GitPrCmd::Create {
                title,
                body,
                base,
                draft,
            } => {
                let title = title.unwrap_or_else(|| {
                    // 尝试从当前分支名生成标题
                    git.branch_current()
                        .unwrap_or_else(|_| "New feature".to_string())
                });

                let result = git.pr_create(&title, body.as_deref(), &base, draft)?;
                println!("{}", result);
            }

            super::args::GitPrCmd::List { state } => {
                use crate::git::pr::PrState;
                let pr_state = match state.to_lowercase().as_str() {
                    "open" => PrState::Open,
                    "closed" => PrState::Closed,
                    "merged" => PrState::Merged,
                    _ => PrState::Open,
                };

                let prs = git.pr_list(pr_state)?;
                if prs.is_empty() {
                    println!("No {} PRs found", state);
                } else {
                    for pr in prs {
                        let draft_str = if pr.draft { " [draft]" } else { "" };
                        println!("#{}{} {} - {}", pr.number, draft_str, pr.title, pr.url);
                    }
                }
            }
        }

        Ok(())
    }

    /// 检查应用是否正在运行
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 获取配置
    pub fn config(&self) -> &CliConfig {
        &self.config
    }

    /// 获取会话状态
    pub async fn session_state(&self) -> SessionState {
        self.session_state.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_config_default() {
        let config = CliConfig::default();
        assert!(!config.debug);
        assert!(config.config_path.is_none());
    }

    #[test]
    fn test_cli_config_builder() {
        let config = CliConfig::new()
            .with_debug(true)
            .with_config_path("/tmp/config.toml");

        assert!(config.debug);
        assert!(config.config_path.is_some());
    }

    #[test]
    fn test_session_state_creation() {
        let state = SessionState::new();
        assert!(state.current_session_id.is_none());
        assert!(!state.user_id.is_empty());
    }
}
