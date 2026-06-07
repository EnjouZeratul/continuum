//! Slash 命令解析和执行

use std::collections::HashMap;

/// 命令定义
#[derive(Debug, Clone)]
pub struct SlashCommand {
    /// 命令名称（不含 /）
    pub name: String,
    /// 命令描述
    pub description: String,
    /// 命令用法
    pub usage: String,
    /// 参数列表
    pub args: Vec<CommandArg>,
    /// 是否需要权限确认
    pub requires_confirmation: bool,
    /// 风险等级
    pub risk_level: RiskLevel,
}

/// 命令参数
#[derive(Debug, Clone)]
pub struct CommandArg {
    /// 参数名称
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default: Option<String>,
}

/// 风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// 低风险 - 无副作用操作
    Low,
    /// 中风险 - 可逆操作
    Medium,
    /// 高风险 - 不可逆或影响外部系统
    High,
}

/// 解析后的命令
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// 命令定义
    pub command: SlashCommand,
    /// 参数值
    pub args: HashMap<String, String>,
    /// 原始输入
    pub raw_input: String,
}

/// 命令解析器
pub struct CommandParser {
    /// 已注册的命令
    commands: HashMap<String, SlashCommand>,
}

impl CommandParser {
    /// 创建新的命令解析器
    pub fn new() -> Self {
        let mut parser = Self {
            commands: HashMap::new(),
        };
        parser.register_builtin_commands();
        parser
    }

    /// 注册内置命令
    fn register_builtin_commands(&mut self) {
        // 会话管理命令
        self.register(SlashCommand {
            name: "help".to_string(),
            description: "显示帮助信息".to_string(),
            usage: "/help [command]".to_string(),
            args: vec![CommandArg {
                name: "command".to_string(),
                description: "要查看帮助的命令名".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "clear".to_string(),
            description: "清空聊天历史".to_string(),
            usage: "/clear".to_string(),
            args: vec![],
            requires_confirmation: true,
            risk_level: RiskLevel::Medium,
        });

        self.register(SlashCommand {
            name: "save".to_string(),
            description: "保存当前会话".to_string(),
            usage: "/save [name]".to_string(),
            args: vec![CommandArg {
                name: "name".to_string(),
                description: "会话名称".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "new".to_string(),
            description: "开始新会话".to_string(),
            usage: "/new".to_string(),
            args: vec![],
            requires_confirmation: true,
            risk_level: RiskLevel::Medium,
        });

        self.register(SlashCommand {
            name: "exit".to_string(),
            description: "退出应用".to_string(),
            usage: "/exit".to_string(),
            args: vec![],
            requires_confirmation: true,
            risk_level: RiskLevel::Medium,
        });

        // 配置命令
        self.register(SlashCommand {
            name: "config".to_string(),
            description: "查看或修改配置".to_string(),
            usage: "/config [key] [value]".to_string(),
            args: vec![
                CommandArg {
                    name: "key".to_string(),
                    description: "配置键".to_string(),
                    required: false,
                    default: None,
                },
                CommandArg {
                    name: "value".to_string(),
                    description: "配置值".to_string(),
                    required: false,
                    default: None,
                },
            ],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "model".to_string(),
            description: "切换或显示当前模型".to_string(),
            usage: "/model [name]".to_string(),
            args: vec![CommandArg {
                name: "name".to_string(),
                description: "模型名称".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "provider".to_string(),
            description: "切换或显示当前提供商".to_string(),
            usage: "/provider [name]".to_string(),
            args: vec![CommandArg {
                name: "name".to_string(),
                description: "提供商名称".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        // 工具命令
        self.register(SlashCommand {
            name: "tools".to_string(),
            description: "列出可用工具".to_string(),
            usage: "/tools [filter]".to_string(),
            args: vec![CommandArg {
                name: "filter".to_string(),
                description: "工具类型过滤".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "bash".to_string(),
            description: "执行 shell 命令".to_string(),
            usage: "/bash <command>".to_string(),
            args: vec![CommandArg {
                name: "command".to_string(),
                description: "要执行的命令".to_string(),
                required: true,
                default: None,
            }],
            requires_confirmation: true,
            risk_level: RiskLevel::High,
        });

        self.register(SlashCommand {
            name: "read".to_string(),
            description: "读取文件".to_string(),
            usage: "/read <file> [--offset N] [--limit N]".to_string(),
            args: vec![
                CommandArg {
                    name: "file".to_string(),
                    description: "文件路径".to_string(),
                    required: true,
                    default: None,
                },
                CommandArg {
                    name: "offset".to_string(),
                    description: "起始行".to_string(),
                    required: false,
                    default: Some("0".to_string()),
                },
                CommandArg {
                    name: "limit".to_string(),
                    description: "行数限制".to_string(),
                    required: false,
                    default: None,
                },
            ],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "write".to_string(),
            description: "写入文件".to_string(),
            usage: "/write <file> <content>".to_string(),
            args: vec![
                CommandArg {
                    name: "file".to_string(),
                    description: "文件路径".to_string(),
                    required: true,
                    default: None,
                },
                CommandArg {
                    name: "content".to_string(),
                    description: "写入内容".to_string(),
                    required: true,
                    default: None,
                },
            ],
            requires_confirmation: true,
            risk_level: RiskLevel::High,
        });

        self.register(SlashCommand {
            name: "edit".to_string(),
            description: "编辑文件".to_string(),
            usage: "/edit <file> --old <text> --new <text>".to_string(),
            args: vec![
                CommandArg {
                    name: "file".to_string(),
                    description: "文件路径".to_string(),
                    required: true,
                    default: None,
                },
                CommandArg {
                    name: "old".to_string(),
                    description: "旧文本".to_string(),
                    required: true,
                    default: None,
                },
                CommandArg {
                    name: "new".to_string(),
                    description: "新文本".to_string(),
                    required: true,
                    default: None,
                },
            ],
            requires_confirmation: true,
            risk_level: RiskLevel::High,
        });

        self.register(SlashCommand {
            name: "grep".to_string(),
            description: "搜索文件内容".to_string(),
            usage: "/grep <pattern> [path] [--glob pattern]".to_string(),
            args: vec![
                CommandArg {
                    name: "pattern".to_string(),
                    description: "搜索模式".to_string(),
                    required: true,
                    default: None,
                },
                CommandArg {
                    name: "path".to_string(),
                    description: "搜索路径".to_string(),
                    required: false,
                    default: Some(".".to_string()),
                },
                CommandArg {
                    name: "glob".to_string(),
                    description: "文件过滤".to_string(),
                    required: false,
                    default: None,
                },
            ],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "glob".to_string(),
            description: "查找文件".to_string(),
            usage: "/glob <pattern> [path]".to_string(),
            args: vec![
                CommandArg {
                    name: "pattern".to_string(),
                    description: "文件模式".to_string(),
                    required: true,
                    default: None,
                },
                CommandArg {
                    name: "path".to_string(),
                    description: "搜索路径".to_string(),
                    required: false,
                    default: Some(".".to_string()),
                },
            ],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        // Git 命令
        self.register(SlashCommand {
            name: "git".to_string(),
            description: "Git 操作".to_string(),
            usage: "/git <subcommand> [args]".to_string(),
            args: vec![CommandArg {
                name: "subcommand".to_string(),
                description: "Git 子命令 (status, diff, commit, add, branch, pr)".to_string(),
                required: true,
                default: None,
            }],
            requires_confirmation: true,
            risk_level: RiskLevel::High,
        });

        // 调试命令
        self.register(SlashCommand {
            name: "debug".to_string(),
            description: "切换调试模式".to_string(),
            usage: "/debug".to_string(),
            args: vec![],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "tokens".to_string(),
            description: "显示 token 使用统计".to_string(),
            usage: "/tokens".to_string(),
            args: vec![],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "history".to_string(),
            description: "显示命令历史".to_string(),
            usage: "/history [count]".to_string(),
            args: vec![CommandArg {
                name: "count".to_string(),
                description: "显示数量".to_string(),
                required: false,
                default: Some("10".to_string()),
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        self.register(SlashCommand {
            name: "undo".to_string(),
            description: "撤销上一次操作".to_string(),
            usage: "/undo".to_string(),
            args: vec![],
            requires_confirmation: true,
            risk_level: RiskLevel::Medium,
        });

        self.register(SlashCommand {
            name: "checkpoint".to_string(),
            description: "创建检查点".to_string(),
            usage: "/checkpoint [message]".to_string(),
            args: vec![CommandArg {
                name: "message".to_string(),
                description: "检查点说明".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });

        // Tutorial 命令
        self.register(SlashCommand {
            name: "tutorial".to_string(),
            description: "交互式新手教程".to_string(),
            usage: "/tutorial [step]".to_string(),
            args: vec![CommandArg {
                name: "step".to_string(),
                description: "教程步骤 (1-5)".to_string(),
                required: false,
                default: None,
            }],
            requires_confirmation: false,
            risk_level: RiskLevel::Low,
        });
    }

    /// 注册命令
    pub fn register(&mut self, command: SlashCommand) {
        self.commands.insert(command.name.clone(), command);
    }

    /// 获取命令定义
    pub fn get_command(&self, name: &str) -> Option<&SlashCommand> {
        self.commands.get(name)
    }

    /// 列出所有命令
    pub fn list_commands(&self) -> Vec<&SlashCommand> {
        let mut commands: Vec<_> = self.commands.values().collect();
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        commands
    }

    /// 解析输入
    pub fn parse(&self, input: &str) -> Option<ParsedCommand> {
        let trimmed = input.trim();

        // 检查是否是命令
        if !trimmed.starts_with('/') {
            return None;
        }

        // 移除开头的 /
        let content = &trimmed[1..];

        // 分割命令和参数
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let command_name = parts[0];
        let command = self.commands.get(command_name)?;

        // 解析参数
        let mut args = HashMap::new();
        let mut i = 1;

        while i < parts.len() {
            let part = parts[i];

            // 检查是否是命名参数 (--key value 或 -k value)
            if let Some(key) = part.strip_prefix("--") {
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    args.insert(key.to_string(), parts[i + 1].to_string());
                    i += 2;
                } else {
                    // 布尔标志
                    args.insert(key.to_string(), "true".to_string());
                    i += 1;
                }
            } else if part.starts_with('-') && part.len() == 2 {
                // 短参数
                let key = &part[1..];
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    args.insert(key.to_string(), parts[i + 1].to_string());
                    i += 2;
                } else {
                    args.insert(key.to_string(), "true".to_string());
                    i += 1;
                }
            } else {
                // 位置参数 - 使用命令定义中的参数名
                let positional_args: Vec<&CommandArg> = command
                    .args
                    .iter()
                    .filter(|a| !args.contains_key(&a.name))
                    .collect();

                if let Some(arg) = positional_args.first() {
                    // 收集剩余所有部分作为该参数的值（对于 content 类参数）
                    if arg.name == "content" || arg.name == "command" {
                        let remaining = parts[i..].join(" ");
                        args.insert(arg.name.clone(), remaining);
                        break;
                    } else {
                        args.insert(arg.name.clone(), part.to_string());
                    }
                }
                i += 1;
            }
        }

        // 填充默认值
        for arg in &command.args {
            if !args.contains_key(&arg.name) {
                if let Some(default) = &arg.default {
                    args.insert(arg.name.clone(), default.clone());
                }
            }
        }

        Some(ParsedCommand {
            command: command.clone(),
            args,
            raw_input: input.to_string(),
        })
    }

    /// 获取命令补全列表
    pub fn get_completions(&self, prefix: &str) -> Vec<&SlashCommand> {
        if !prefix.starts_with('/') {
            return vec![];
        }

        let prefix_name = &prefix[1..];
        self.commands
            .values()
            .filter(|cmd| cmd.name.starts_with(prefix_name))
            .collect()
    }

    /// 生成帮助文本
    pub fn generate_help(&self, command_name: Option<&str>) -> String {
        if let Some(name) = command_name {
            if let Some(cmd) = self.commands.get(name) {
                let mut help = format!("{}\n\nUsage: {}\n\nArgs:", cmd.description, cmd.usage);

                if cmd.args.is_empty() {
                    help.push_str("\n  (无参数)");
                } else {
                    for arg in &cmd.args {
                        let required = if arg.required { " (必需)" } else { "" };
                        let default = arg
                            .default
                            .as_ref()
                            .map(|d| format!(" [默认: {}]", d))
                            .unwrap_or_default();
                        help.push_str(&format!(
                            "\n  {}{}{} - {}",
                            arg.name, required, default, arg.description
                        ));
                    }
                }

                let risk = match cmd.risk_level {
                    RiskLevel::Low => "低",
                    RiskLevel::Medium => "中",
                    RiskLevel::High => "高",
                };
                help.push_str(&format!("\n\n风险等级: {}", risk));

                if cmd.requires_confirmation {
                    help.push_str("\n需要确认: 是");
                }

                help
            } else {
                format!("未找到命令: {}", name)
            }
        } else {
            let mut help = "可用命令:\n".to_string();

            for cmd in self.list_commands() {
                help.push_str(&format!("  /{:<12} - {}\n", cmd.name, cmd.description));
            }

            help.push_str("\n输入 /help <command> 查看详细信息");
            help
        }
    }
}

impl Default for CommandParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 命令执行结果
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// 成功，带输出
    Success(String),
    /// 需要确认
    NeedsConfirmation {
        command: ParsedCommand,
        message: String,
    },
    /// 错误
    Error(String),
    /// 退出应用
    Exit,
    /// 无操作
    NoOp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = CommandParser::new();
        assert!(!parser.commands.is_empty());
    }

    #[test]
    fn test_parse_help_command() {
        let parser = CommandParser::new();
        let result = parser.parse("/help");
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.command.name, "help");
    }

    #[test]
    fn test_parse_bash_command() {
        let parser = CommandParser::new();
        let result = parser.parse("/bash ls -la");
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.command.name, "bash");
        assert_eq!(parsed.args.get("command"), Some(&"ls -la".to_string()));
    }

    #[test]
    fn test_parse_named_args() {
        let parser = CommandParser::new();
        let result = parser.parse("/read test.txt --offset 10 --limit 20");
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.command.name, "read");
        assert_eq!(parsed.args.get("file"), Some(&"test.txt".to_string()));
        assert_eq!(parsed.args.get("offset"), Some(&"10".to_string()));
        assert_eq!(parsed.args.get("limit"), Some(&"20".to_string()));
    }

    #[test]
    fn test_not_a_command() {
        let parser = CommandParser::new();
        let result = parser.parse("hello world");
        assert!(result.is_none());
    }

    #[test]
    fn test_unknown_command() {
        let parser = CommandParser::new();
        let result = parser.parse("/unknown");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_completions() {
        let parser = CommandParser::new();
        let completions = parser.get_completions("/ba");
        assert!(!completions.is_empty());
        // 应该包含 bash
        assert!(completions.iter().any(|c| c.name == "bash"));
    }

    #[test]
    fn test_generate_help() {
        let parser = CommandParser::new();
        let help = parser.generate_help(None);
        assert!(help.contains("可用命令"));
        assert!(help.contains("/help"));
    }

    #[test]
    fn test_generate_command_help() {
        let parser = CommandParser::new();
        let help = parser.generate_help(Some("bash"));
        assert!(help.contains("执行 shell 命令"));
        assert!(help.contains("风险等级"));
    }

    #[test]
    fn test_risk_level() {
        let parser = CommandParser::new();
        let help_cmd = parser.get_command("help").unwrap();
        assert_eq!(help_cmd.risk_level, RiskLevel::Low);

        let bash_cmd = parser.get_command("bash").unwrap();
        assert_eq!(bash_cmd.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_requires_confirmation() {
        let parser = CommandParser::new();
        let help_cmd = parser.get_command("help").unwrap();
        assert!(!help_cmd.requires_confirmation);

        let bash_cmd = parser.get_command("bash").unwrap();
        assert!(bash_cmd.requires_confirmation);
    }
}
