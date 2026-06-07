//! # CLI 参数定义
//!
//! 命令行参数结构和解析。

use clap::{Parser, Subcommand};

/// Continuum CLI - 终端 Agent 产品
#[derive(Parser, Debug)]
#[command(name = "continuum")]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// 启用调试日志
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// 配置文件路径
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    /// 子命令（可选，默认进入TUI）
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// 子命令
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// 启动 Agent 执行任务
    Run {
        /// 任务描述
        task: Option<String>,
        /// 使用指定会话 ID
        #[arg(short, long)]
        session: Option<String>,
        /// 设置预算上限
        #[arg(short, long)]
        budget: Option<f64>,
        /// 非交互模式
        #[arg(short, long)]
        non_interactive: bool,
    },

    /// 管理会话
    Session {
        #[command(subcommand)]
        cmd: SessionCmd,
    },

    /// 管理配置
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },

    /// 列出可用工具
    Tools {
        /// 筛选工具类型
        #[arg(short, long)]
        filter: Option<String>,
        /// 显示详细信息
        #[arg(short, long)]
        verbose: bool,
    },

    /// 启动 TUI 界面（默认行为）
    Tui {
        /// 恢复指定会话
        #[arg(short, long)]
        session: Option<String>,
    },

    /// 检查点管理
    Checkpoint {
        #[command(subcommand)]
        cmd: CheckpointCmd,
    },

    /// 启动本地 Dashboard（可观测性界面）
    Dashboard {
        #[command(subcommand)]
        cmd: DashboardCmd,
    },

    // ===== 工具链命令 =====
    /// 执行 shell 命令
    Bash {
        /// 要执行的命令
        command: String,
        /// 工作目录
        #[arg(short = 'w', long)]
        cwd: Option<String>,
        /// 超时时间（秒）
        #[arg(short, long, default_value = "120")]
        timeout: u64,
        /// 捕获标准错误
        #[arg(short = 'e', long)]
        capture_stderr: bool,
    },

    /// 读取文件内容
    Read {
        /// 文件路径
        file: String,
        /// 起始行号
        #[arg(short, long)]
        offset: Option<usize>,
        /// 读取行数
        #[arg(short, long)]
        limit: Option<usize>,
        /// 显示行号
        #[arg(short = 'n', long)]
        line_numbers: bool,
    },

    /// 写入文件
    Write {
        /// 文件路径
        file: String,
        /// 要写入的内容（或从 stdin 读取）
        content: Option<String>,
        /// 追加模式
        #[arg(short, long)]
        append: bool,
        /// 创建备份
        #[arg(short, long)]
        backup: bool,
    },

    /// 编辑文件（精确替换）
    Edit {
        /// 文件路径
        file: String,
        /// 旧内容
        #[arg(short, long)]
        old: String,
        /// 新内容
        #[arg(short, long)]
        new: String,
        /// 替换所有匹配
        #[arg(short, long)]
        replace_all: bool,
    },

    /// 搜索文件内容
    Grep {
        /// 搜索模式（正则表达式）
        pattern: String,
        /// 搜索路径
        #[arg(short, long, default_value = ".")]
        path: String,
        /// 文件模式过滤
        #[arg(short, long)]
        glob: Option<String>,
        /// 忽略大小写
        #[arg(short, long)]
        ignore_case: bool,
        /// 显示行数
        #[arg(short = 'n', long)]
        line_numbers: bool,
        /// 显示上下文行数
        #[arg(short = 'C', long)]
        context: Option<usize>,
    },

    /// 查找文件
    Glob {
        /// 文件模式
        pattern: String,
        /// 搜索路径
        #[arg(short, long, default_value = ".")]
        path: String,
    },

    /// LSP 代码智能
    Lsp {
        #[command(subcommand)]
        cmd: LspCmd,
    },

    // ===== Git 命令 =====
    /// Git 工作流集成
    Git {
        #[command(subcommand)]
        cmd: GitCmd,
    },
}

/// LSP 子命令
#[derive(Subcommand, Debug, Clone)]
pub enum LspCmd {
    /// 跳转到定义
    Definition {
        /// 文件路径
        file: String,
        /// 行号（1-based）
        line: usize,
        /// 列号（1-based）
        column: usize,
    },
    /// 查找引用
    References {
        /// 文件路径
        file: String,
        /// 行号（1-based）
        line: usize,
        /// 列号（1-based）
        column: usize,
    },
    /// 悬停信息
    Hover {
        /// 文件路径
        file: String,
        /// 行号（1-based）
        line: usize,
        /// 列号（1-based）
        column: usize,
    },
    /// 文档符号
    Symbols {
        /// 文件路径
        file: String,
    },
}

/// Git 子命令
#[derive(Subcommand, Debug, Clone)]
pub enum GitCmd {
    /// 显示工作区状态
    Status {
        /// 简洁格式
        #[arg(short, long)]
        short: bool,
    },

    /// 显示变更差异
    Diff {
        /// 显示暂存区差异
        #[arg(short, long)]
        staged: bool,
        /// 文件路径（可选）
        paths: Vec<String>,
    },

    /// 提交变更
    Commit {
        /// commit 消息（自动生成如果未指定）
        #[arg(short, long)]
        message: Option<String>,
        /// 修改上次提交
        #[arg(short, long)]
        amend: bool,
        /// 自动暂存所有变更
        #[arg(short = 'A', long)]
        add_all: bool,
    },

    /// 暂存文件
    Add {
        /// 要暂存的文件路径
        paths: Vec<String>,
    },

    /// 分支管理
    Branch {
        #[command(subcommand)]
        cmd: GitBranchCmd,
    },

    /// 创建 Pull Request
    Pr {
        #[command(subcommand)]
        cmd: GitPrCmd,
    },
}

/// Git Branch 子命令
#[derive(Subcommand, Debug, Clone)]
pub enum GitBranchCmd {
    /// 列出分支
    List {
        /// 包含远程分支
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// 创建分支
    Create {
        /// 分支名称
        name: String,
        /// 创建后切换
        #[arg(short, long)]
        switch: bool,
    },

    /// 切换分支
    Switch {
        /// 分支名称
        name: String,
    },

    /// 删除分支
    Delete {
        /// 分支名称
        name: String,
        /// 强制删除
        #[arg(short, long)]
        force: bool,
    },
}

/// Git PR 子命令
#[derive(Subcommand, Debug, Clone)]
pub enum GitPrCmd {
    /// 创建 PR
    Create {
        /// PR 标题
        #[arg(short, long)]
        title: Option<String>,
        /// PR 描述
        #[arg(short, long)]
        body: Option<String>,
        /// 目标分支
        #[arg(short, long, default_value = "main")]
        base: String,
        /// 创建为草稿
        #[arg(short, long)]
        draft: bool,
    },

    /// 列出 PR
    List {
        /// PR 状态
        #[arg(short, long, default_value = "open")]
        state: String,
    },
}

/// 会话子命令
#[derive(Subcommand, Debug, Clone)]
pub enum SessionCmd {
    /// 列出所有会话
    List {
        /// 显示已完成的会话
        #[arg(short, long)]
        all: bool,
    },
    /// 恢复指定会话
    Resume {
        /// 会话 ID
        session_id: String,
    },
    /// 删除指定会话
    Delete {
        /// 会话 ID
        session_id: String,
        /// 强制删除
        #[arg(short, long)]
        force: bool,
    },
    /// 显示会话详情
    Show {
        /// 会话 ID
        session_id: String,
    },
}

/// 配置子命令
#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCmd {
    /// 显示当前配置
    Show {
        /// 配置键（可选）
        key: Option<String>,
    },
    /// 设置配置项
    Set {
        /// 配置键
        key: String,
        /// 配置值
        value: String,
    },
    /// 获取配置项
    Get {
        /// 配置键
        key: String,
    },
    /// 初始化默认配置文件
    Init {
        /// 强制覆盖已存在的配置
        #[arg(short, long)]
        force: bool,
    },
    /// 列出所有配置键
    Keys,
    /// 列出所有提供商
    List,
    /// 添加提供商
    AddProvider {
        /// 提供商名称
        name: String,
        /// API 密钥
        #[arg(short, long)]
        key: String,
        /// API 基础 URL
        #[arg(short, long)]
        url: Option<String>,
        /// 默认模型
        #[arg(short, long)]
        model: Option<String>,
    },
    /// 切换提供商
    Use {
        /// 提供商名称
        provider: String,
    },
}

/// 检查点子命令
#[derive(Subcommand, Debug, Clone)]
pub enum CheckpointCmd {
    /// 列出检查点
    List {
        /// 会话 ID
        #[arg(short, long)]
        session: Option<String>,
    },
    /// 恢复到指定检查点
    Restore {
        /// 检查点 ID
        checkpoint_id: String,
    },
    /// 删除检查点
    Delete {
        /// 检查点 ID
        checkpoint_id: String,
    },
}

/// Dashboard 子命令
#[derive(Subcommand, Debug, Clone)]
pub enum DashboardCmd {
    /// 启动 Dashboard Web UI
    Start {
        /// 监听端口
        #[arg(short, long, default_value = "8080")]
        port: u16,
        /// 监听地址
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,
    },
    /// 显示 Dashboard 状态
    Status,
}

impl CliArgs {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args_version() {
        // 版本命令会触发 clap 的 DisplayVersion 退出，无法正常返回
        // 使用 clap::error::ErrorKind::DisplayVersion 检测
        let args = CliArgs::try_parse_from(["continuum", "--version"]);
        match args {
            Ok(_) => {} // 某些版本允许成功
            Err(e) => {
                // 验证是版本显示退出而非解析错误
                assert!(matches!(e.kind(), clap::error::ErrorKind::DisplayVersion));
            }
        }
    }

    #[test]
    fn test_cli_args_help() {
        // 帮助命令会触发 clap 的 DisplayHelp 退出
        let args = CliArgs::try_parse_from(["continuum", "--help"]);
        match args {
            Ok(_) => {} // 某些版本允许成功
            Err(e) => {
                // 验证是帮助显示退出而非解析错误
                assert!(matches!(e.kind(), clap::error::ErrorKind::DisplayHelp));
            }
        }
    }

    #[test]
    fn test_default_tui_behavior() {
        // 测试无参数时默认进入TUI
        let args = CliArgs::try_parse_from(["continuum"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        // command 应该是 None
        assert!(args.command.is_none());
    }

    #[test]
    fn test_explicit_tui_command() {
        // 测试显式tui命令
        let args = CliArgs::try_parse_from(["continuum", "tui"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(matches!(args.command, Some(Commands::Tui { .. })));
    }

    #[test]
    fn test_run_command() {
        // 测试run子命令
        let args = CliArgs::try_parse_from(["continuum", "run", "test task"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Run { task, .. }) = args.command {
            assert_eq!(task, Some("test task".to_string()));
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_session_list_command() {
        // 测试session list子命令
        let args = CliArgs::try_parse_from(["continuum", "session", "list"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(matches!(args.command, Some(Commands::Session { .. })));
    }

    #[test]
    fn test_config_show_command() {
        // 测试config show子命令
        let args = CliArgs::try_parse_from(["continuum", "config", "show"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(matches!(args.command, Some(Commands::Config { .. })));
    }

    #[test]
    fn test_tools_command() {
        // 测试tools子命令
        let args = CliArgs::try_parse_from(["continuum", "tools", "--verbose"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Tools { verbose, .. }) = args.command {
            assert!(verbose);
        } else {
            panic!("Expected Tools command");
        }
    }

    #[test]
    fn test_checkpoint_list_command() {
        // 测试checkpoint list子命令
        let args = CliArgs::try_parse_from(["continuum", "checkpoint", "list"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(matches!(args.command, Some(Commands::Checkpoint { .. })));
    }

    #[test]
    fn test_bash_command() {
        let args = CliArgs::try_parse_from(["continuum", "bash", "ls -la"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Bash { command, .. }) = args.command {
            assert_eq!(command, "ls -la");
        } else {
            panic!("Expected Bash command");
        }
    }

    #[test]
    fn test_read_command() {
        let args = CliArgs::try_parse_from([
            "continuum",
            "read",
            "test.txt",
            "--offset",
            "10",
            "--limit",
            "20",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Read {
            file,
            offset,
            limit,
            ..
        }) = args.command
        {
            assert_eq!(file, "test.txt");
            assert_eq!(offset, Some(10));
            assert_eq!(limit, Some(20));
        } else {
            panic!("Expected Read command");
        }
    }

    #[test]
    fn test_write_command() {
        let args =
            CliArgs::try_parse_from(["continuum", "write", "test.txt", "hello world", "--backup"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Write {
            file,
            content,
            backup,
            ..
        }) = args.command
        {
            assert_eq!(file, "test.txt");
            assert_eq!(content, Some("hello world".to_string()));
            assert!(backup);
        } else {
            panic!("Expected Write command");
        }
    }

    #[test]
    fn test_edit_command() {
        let args = CliArgs::try_parse_from([
            "continuum",
            "edit",
            "test.txt",
            "--old",
            "foo",
            "--new",
            "bar",
            "--replace-all",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Edit {
            file,
            old,
            new,
            replace_all,
        }) = args.command
        {
            assert_eq!(file, "test.txt");
            assert_eq!(old, "foo");
            assert_eq!(new, "bar");
            assert!(replace_all);
        } else {
            panic!("Expected Edit command");
        }
    }

    #[test]
    fn test_grep_command() {
        let args = CliArgs::try_parse_from([
            "continuum",
            "grep",
            "pattern",
            "--path",
            "src/",
            "--ignore-case",
            "--context",
            "2",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Grep {
            pattern,
            path,
            ignore_case,
            context,
            ..
        }) = args.command
        {
            assert_eq!(pattern, "pattern");
            assert_eq!(path, "src/");
            assert!(ignore_case);
            assert_eq!(context, Some(2));
        } else {
            panic!("Expected Grep command");
        }
    }

    #[test]
    fn test_glob_command() {
        let args = CliArgs::try_parse_from(["continuum", "glob", "**/*.rs", "--path", "src/"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Glob { pattern, path }) = args.command {
            assert_eq!(pattern, "**/*.rs");
            assert_eq!(path, "src/");
        } else {
            panic!("Expected Glob command");
        }
    }

    #[test]
    fn test_lsp_definition_command() {
        let args =
            CliArgs::try_parse_from(["continuum", "lsp", "definition", "test.rs", "10", "5"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        if let Some(Commands::Lsp { cmd }) = args.command {
            if let LspCmd::Definition { file, line, column } = cmd {
                assert_eq!(file, "test.rs");
                assert_eq!(line, 10);
                assert_eq!(column, 5);
            } else {
                panic!("Expected Definition subcommand");
            }
        } else {
            panic!("Expected Lsp command");
        }
    }
}
