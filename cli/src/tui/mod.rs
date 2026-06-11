//! TUI 界面模块

use anyhow::Result;
use crossterm::{
    event::{
        self as crossterm_event, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use sh_core::layer1::ConfigManager;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub mod app;
pub mod components;
pub mod error_messages;
mod event;
pub mod first_run;
pub mod session_persistence;
pub mod setup;
pub mod slash_commands;
pub mod tutorial;
pub mod ui;

use crate::agent::{AgentClient, AgentError};
use app::App;
use components::{
    ChatComponent, ConfirmationDialog, InputComponent, KeyHintsComponent, PermissionManager,
    PermissionPopup, SetupWizard, StatusComponent, ToolDisplayComponent,
};
use error_messages::ErrorFriendlyizer;
use setup::ConfigDetector;
use slash_commands::{CommandParser, CommandResult, ParsedCommand};

/// TUI 组件容器（可变引用）
struct TuiComponentsMut<'a> {
    app: &'a mut App,
    chat: &'a mut ChatComponent,
    input: &'a mut InputComponent,
    status: &'a mut StatusComponent,
    tools: &'a mut ToolDisplayComponent,
    key_hints: &'a mut KeyHintsComponent,
    confirmation: &'a mut ConfirmationDialog,
    permissions: &'a mut PermissionManager,
    permission_popup: &'a mut PermissionPopup,
    setup_wizard: &'a mut SetupWizard,
}

/// 命令执行上下文
struct CommandExecContext<'a> {
    chat: &'a mut ChatComponent,
    status: &'a mut StatusComponent,
    app: &'a mut App,
    permissions: &'a mut PermissionManager,
}

/// 键盘事件处理上下文
struct KeyEventHandlerContext<'a> {
    app: &'a mut App,
    chat: &'a mut ChatComponent,
    input: &'a mut InputComponent,
    status: &'a mut StatusComponent,
    key_hints: &'a mut KeyHintsComponent,
    confirmation: &'a mut ConfirmationDialog,
    permissions: &'a mut PermissionManager,
    show_completions: &'a mut bool,
}

/// 运行 TUI 界面
pub fn run() -> Result<()> {
    run_with_session(None)
}

/// 运行 TUI 界面（带指定会话）
pub fn run_with_session(session: Option<String>) -> Result<()> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用和 Agent 客户端
    let mut app = App::new();
    let agent = Arc::new(RwLock::new(AgentClient::new()));

    // 如果有指定会话，设置会话 ID
    if let Some(session_id) = session {
        app.set_session_id(session_id);
    }

    // 创建组件
    let mut chat = ChatComponent::new();
    let mut input = InputComponent::new();
    let mut status = StatusComponent::new();
    let mut tools = ToolDisplayComponent::new();
    let mut confirmation = ConfirmationDialog::new();
    let mut permissions = PermissionManager::new();
    let mut permission_popup = PermissionPopup::new();
    let mut key_hints = KeyHintsComponent::new();
    let mut setup_wizard = SetupWizard::new();
    let command_parser = CommandParser::new();

    // 检测首次启动状态
    let mut first_run_state = first_run::FirstRunState::new().unwrap_or_default();

    // 检测配置状态
    let config_detector = ConfigDetector::new();
    let detection_result = config_detector
        .detect()
        .unwrap_or_else(|_| setup::DetectionResult {
            providers: vec![],
            has_valid_config: false,
            config_file_path: None,
        });

    // 决定是否显示配置向导
    let show_setup_wizard = !detection_result.has_valid_config && first_run_state.needs_setup();

    // 添加欢迎消息（根据首次启动状态定制）
    let welcome_msg = if show_setup_wizard {
        // 将由配置向导处理
        String::new()
    } else if first_run_state.is_first_run {
        first_run::FirstRunState::get_welcome_message()
    } else {
        "Welcome back! Agent initializing...".to_string()
    };

    if !welcome_msg.is_empty() {
        chat.add_message(app::Message {
            role: app::Role::System,
            content: welcome_msg,
        });
        status.set_message_count(chat.message_count());
    }

    // 如果需要配置向导，显示它
    if show_setup_wizard {
        setup_wizard.show();
    }

    // 首次启动时自动显示教程提示（仅在配置完成后）
    if !show_setup_wizard && first_run_state.is_first_run && !first_run_state.tutorial_completed {
        chat.add_message(app::Message {
            role: app::Role::System,
            content: first_run::FirstRunState::get_first_run_hint(),
        });
        status.set_message_count(chat.message_count());
    }

    // 主循环
    let res = run_app(
        &mut terminal,
        TuiComponentsMut {
            app: &mut app,
            chat: &mut chat,
            input: &mut input,
            status: &mut status,
            tools: &mut tools,
            key_hints: &mut key_hints,
            confirmation: &mut confirmation,
            permissions: &mut permissions,
            permission_popup: &mut permission_popup,
            setup_wizard: &mut setup_wizard,
        },
        &command_parser,
        agent,
        &mut first_run_state,
    );

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

/// 运行应用主循环
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut components: TuiComponentsMut<'_>,
    command_parser: &CommandParser,
    agent: Arc<RwLock<AgentClient>>,
    first_run_state: &mut first_run::FirstRunState,
) -> Result<()> {
    let TuiComponentsMut {
        app,
        chat,
        input,
        status,
        tools,
        key_hints,
        confirmation,
        permissions,
        permission_popup,
        setup_wizard,
    } = &mut components;

    // 创建 tokio runtime 用于异步操作
    let rt = tokio::runtime::Runtime::new()?;

    // 初始化 Agent
    let init_result = rt.block_on(async {
        let agent_guard = agent.read().await;
        agent_guard.init_from_config().await
    });

    match init_result {
        Ok(()) => {
            // 获取配置信息
            let config_info = rt.block_on(async {
                let agent_guard = agent.read().await;
                let provider = agent_guard.current_provider().await;
                let config = agent_guard.config_info().await;
                (provider, config)
            });

            chat.add_message(app::Message {
                role: app::Role::System,
                content:
                    "Agent initialized successfully. Press Enter to send a message, Ctrl+C to quit."
                        .to_string(),
            });

            // 设置状态栏信息
            status.set_connected(true);
            status.set_provider(Some(config_info.0));
            // 从 config_info 解析 model
            if config_info.1.contains("Model:") {
                let parts: Vec<&str> = config_info.1.split("Model:").collect();
                if parts.len() > 1 {
                    let model_part = parts[1].split("|").next().unwrap_or("").trim();
                    status.set_model(Some(model_part.to_string()));
                }
            }
        }
        Err(AgentError::Config(msg)) => {
            let friendly = ErrorFriendlyizer::config_error("no_providers", &msg);
            chat.add_message(app::Message {
                role: app::Role::System,
                content: friendly.format(),
            });
            status.set_connected(false);
        }
        Err(e) => {
            let friendly = ErrorFriendlyizer::from_raw_error(&e.to_string());
            chat.add_message(app::Message {
                role: app::Role::System,
                content: friendly.format(),
            });
            status.set_connected(false);
        }
    }
    status.set_message_count(chat.message_count());

    // 用于跟踪是否有正在进行的请求
    let mut processing = false;
    // 是否显示工具面板
    let mut show_tools = false;
    // 是否显示命令补全
    let mut show_completions = false;
    // 是否等待首次启动教程响应
    let mut waiting_tutorial_response =
        first_run_state.is_first_run && !first_run_state.tutorial_completed;

    // 设置初始快捷键提示上下文
    key_hints.set_context(components::HintContext::Normal);

    loop {
        // 更新快捷键提示上下文
        if processing {
            key_hints.set_context(components::HintContext::Processing);
        } else if show_tools {
            key_hints.set_context(components::HintContext::ToolsVisible);
        } else if !input.get_input().is_empty() {
            key_hints.set_context(components::HintContext::Input);
        } else {
            key_hints.set_context(components::HintContext::Normal);
        }

        // 绘制界面
        terminal.draw(|f| {
            ui::render(
                f,
                app,
                ui::RenderContext {
                    chat,
                    input,
                    status,
                    tools,
                    key_hints,
                },
                show_tools,
            );

            // 渲染权限确认弹窗（如果可见）
            if permission_popup.is_visible() {
                permission_popup.render(f, f.area());
            }
            // 渲染确认对话框（如果可见）
            else if confirmation.is_visible() {
                confirmation.render(f, f.area());
            }
            // 渲染配置向导（如果可见）
            else if setup_wizard.is_visible() {
                setup_wizard.render(f, f.area());
            }
        })?;

        // 处理事件（带超时）
        if crossterm_event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm_event::read()? {
                // 如果权限确认弹窗可见，优先处理权限弹窗的按键
                if permission_popup.is_visible() {
                    let action = permission_popup.handle_key(key.code);
                    match action {
                        components::PermissionAction::Allow => {
                            // 处理允许操作
                            if let Some(req) = permission_popup.get_request() {
                                let perm_key = PermissionManager::get_permission_key(
                                    &req.tool_name,
                                    &req.action,
                                );
                                permissions.grant_permission(&perm_key, false);
                                chat.add_message(app::Message {
                                    role: app::Role::System,
                                    content: format!("已允许: {} - {}", req.tool_name, req.action),
                                });
                                status.set_message_count(chat.message_count());
                            }
                            permission_popup.hide();
                        }
                        components::PermissionAction::Deny => {
                            // 处理拒绝操作
                            if let Some(req) = permission_popup.get_request() {
                                let perm_key = PermissionManager::get_permission_key(
                                    &req.tool_name,
                                    &req.action,
                                );
                                permissions.deny_permission(&perm_key);
                                chat.add_message(app::Message {
                                    role: app::Role::System,
                                    content: format!("已拒绝: {} - {}", req.tool_name, req.action),
                                });
                                status.set_message_count(chat.message_count());
                            }
                            permission_popup.hide();
                        }
                        components::PermissionAction::AlwaysAllow => {
                            // 处理始终允许操作
                            if let Some(req) = permission_popup.get_request() {
                                let perm_key = PermissionManager::get_permission_key(
                                    &req.tool_name,
                                    &req.action,
                                );
                                permissions.grant_permission(&perm_key, true);
                                chat.add_message(app::Message {
                                    role: app::Role::System,
                                    content: format!(
                                        "已永久允许: {} - {}",
                                        req.tool_name, req.action
                                    ),
                                });
                                status.set_message_count(chat.message_count());
                            }
                            permission_popup.hide();
                        }
                        components::PermissionAction::None => {}
                    }
                    continue;
                }

                // 如果确认对话框可见，优先处理确认对话框的按键
                if confirmation.is_visible() {
                    let action = confirmation.handle_key(key.code);
                    match action {
                        components::ConfirmAction::Confirmed => {
                            // 执行确认的命令
                            if let Some(cmd) = confirmation.get_pending_command() {
                                let result = execute_slash_command_with_agent(
                                    cmd.clone(),
                                    &mut CommandExecContext {
                                        chat,
                                        status,
                                        app,
                                        permissions,
                                    },
                                    agent.clone(),
                                    &rt,
                                );
                                handle_command_result(result, chat, status, app);
                            }
                            confirmation.hide();
                        }
                        components::ConfirmAction::Cancelled => {
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: "操作已取消".to_string(),
                            });
                            status.set_message_count(chat.message_count());
                            confirmation.hide();
                        }
                        components::ConfirmAction::None => {}
                    }
                    continue;
                }

                // 如果配置向导可见，处理配置向导的按键
                if setup_wizard.is_visible() {
                    match setup_wizard.current_step {
                        components::WizardStep::Welcome => {
                            match key.code {
                                KeyCode::Enter => {
                                    setup_wizard.next_step();
                                }
                                KeyCode::Esc => {
                                    // Skip setup
                                    setup_wizard.hide();
                                    first_run_state.mark_setup_skipped().ok();
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: "Setup skipped. You can configure later with /config or set environment variables.".to_string(),
                                    });
                                    status.set_message_count(chat.message_count());
                                }
                                _ => {}
                            }
                        }
                        components::WizardStep::ProviderSelection => match key.code {
                            KeyCode::Up => {
                                let providers = components::Provider::all();
                                let current_idx = setup_wizard
                                    .selected_provider
                                    .map(|p| providers.iter().position(|&x| x == p).unwrap_or(0))
                                    .unwrap_or(0);
                                let new_idx = if current_idx == 0 {
                                    providers.len() - 1
                                } else {
                                    current_idx - 1
                                };
                                setup_wizard.select_provider(providers[new_idx]);
                            }
                            KeyCode::Down => {
                                let providers = components::Provider::all();
                                let current_idx = setup_wizard
                                    .selected_provider
                                    .map(|p| providers.iter().position(|&x| x == p).unwrap_or(0))
                                    .unwrap_or(0);
                                let new_idx = (current_idx + 1) % providers.len();
                                setup_wizard.select_provider(providers[new_idx]);
                            }
                            KeyCode::Enter if setup_wizard.selected_provider.is_some() => {
                                setup_wizard.next_step();
                            }
                            KeyCode::Esc => {
                                setup_wizard.prev_step();
                            }
                            _ => {}
                        },
                        components::WizardStep::ApiKeyInput => match key.code {
                            KeyCode::Char(c) => {
                                setup_wizard.push_char(c);
                            }
                            KeyCode::Backspace => {
                                setup_wizard.pop_char();
                            }
                            KeyCode::Tab => {
                                setup_wizard.toggle_visibility();
                            }
                            KeyCode::Enter if !setup_wizard.api_key_input.is_empty() => {
                                setup_wizard.next_step();
                            }
                            KeyCode::Esc => {
                                setup_wizard.prev_step();
                            }
                            _ => {}
                        },
                        components::WizardStep::ConnectionTest => {
                            match key.code {
                                KeyCode::Enter => {
                                    // Skip test and save
                                    if let Some(provider) = setup_wizard.selected_provider {
                                        if let Err(e) = save_provider_config(
                                            provider.name(),
                                            &setup_wizard.api_key_input,
                                        ) {
                                            setup_wizard.error_message = Some(e.to_string());
                                        } else {
                                            first_run_state
                                                .mark_setup_completed(provider.name())
                                                .ok();
                                            setup_wizard.next_step();
                                        }
                                    }
                                }
                                KeyCode::Esc => {
                                    setup_wizard.prev_step();
                                }
                                _ => {}
                            }
                        }
                        components::WizardStep::Complete => {
                            if key.code == KeyCode::Enter {
                                setup_wizard.hide();
                                // Re-initialize agent with new config
                                let init_result = rt.block_on(async {
                                    let agent_guard = agent.read().await;
                                    agent_guard.init_from_config().await
                                });
                                if init_result.is_ok() {
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: "Configuration saved! You can now start using Continuum.".to_string(),
                                    });
                                    status.set_connected(true);
                                    if let Some(provider) = setup_wizard.selected_provider {
                                        status.set_provider(Some(provider.name().to_string()));
                                    }
                                }
                                status.set_message_count(chat.message_count());
                            }
                        }
                    }
                    continue;
                }

                // 如果等待教程响应，处理 Y/N
                if waiting_tutorial_response {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            waiting_tutorial_response = false;
                            first_run_state.mark_tutorial_completed().ok();
                            // 启动教程
                            let mut tutorial = tutorial::Tutorial::new();
                            if let Some(step) = tutorial.start() {
                                chat.add_message(app::Message {
                                    role: app::Role::System,
                                    content: tutorial::Tutorial::format_step(step),
                                });
                                status.set_message_count(chat.message_count());
                            }
                            continue;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            waiting_tutorial_response = false;
                            first_run_state.mark_first_run_done().ok();
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: "Tutorial skipped. Type /tutorial anytime to start."
                                    .to_string(),
                            });
                            status.set_message_count(chat.message_count());
                            continue;
                        }
                        _ => {}
                    }
                }

                // 如果正在处理，只处理 Ctrl+C
                if processing {
                    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                        app.running = false;
                        break;
                    }
                    continue;
                }

                match handle_key_event(
                    key,
                    &mut KeyEventHandlerContext {
                        app,
                        chat,
                        input,
                        status,
                        key_hints,
                        confirmation,
                        permissions,
                        show_completions: &mut show_completions,
                    },
                    command_parser,
                    Some(agent.clone()),
                    &rt,
                ) {
                    Ok(action) => {
                        match action {
                            KeyAction::Exit => break,
                            KeyAction::SendMessage(content) => {
                                // Check if setup was skipped and needs configuration
                                if first_run_state.needs_setup() {
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: "You need to configure an API key to use Continuum.\nPress Enter to open setup wizard, or set environment variables (ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY).".to_string(),
                                    });
                                    status.set_message_count(chat.message_count());
                                    setup_wizard.show();
                                    continue;
                                }

                                status.set_processing(true);

                                // 添加工具调用显示（模拟工具执行）
                                let tool_idx = tools.add_tool_call(
                                    "llm_request".to_string(),
                                    serde_json::to_string(&serde_json::json!({
                                        "prompt": content,
                                    }))
                                    .unwrap_or_default(),
                                );
                                tools.set_running(tool_idx);

                                // 在后台处理 Agent 调用
                                let agent_clone = agent.clone();
                                let response = rt.block_on(async {
                                    let agent_guard = agent_clone.read().await;
                                    if agent_guard.is_initialized().await {
                                        agent_guard.send_message(&content).await
                                    } else {
                                        Err(AgentError::Config("Agent not initialized".to_string()))
                                    }
                                });

                                match response {
                                    Ok(response_content) => {
                                        tools.complete_tool_call(
                                            tool_idx,
                                            "LLM response received".to_string(),
                                            false,
                                        );
                                        chat.add_message(app::Message {
                                            role: app::Role::Assistant,
                                            content: response_content,
                                        });
                                    }
                                    Err(e) => {
                                        let friendly =
                                            ErrorFriendlyizer::from_raw_error(&e.to_string());
                                        tools.complete_tool_call(
                                            tool_idx,
                                            friendly.title.clone(),
                                            true,
                                        );
                                        chat.add_message(app::Message {
                                            role: app::Role::Assistant,
                                            content: friendly.format(),
                                        });
                                    }
                                }
                                status.set_message_count(chat.message_count());
                                status.set_processing(false);
                                processing = false;

                                // 自动显示工具面板当有工具调用时
                                show_tools = tools.count() > 0;
                            }
                            KeyAction::ClearScreen => {
                                chat.clear();
                                tools.clear();
                                show_tools = false;
                                status.set_message_count(0);
                            }
                            KeyAction::SaveSession => {
                                // 使用 SessionManager 保存会话
                                let session_manager =
                                    session_persistence::SessionManager::new().unwrap_or_default();
                                let session_id = match app.session_id() {
                                    Some(id) => id.to_string(),
                                    None => {
                                        let id = uuid::Uuid::new_v4().to_string();
                                        app.set_session_id(id.clone());
                                        id
                                    }
                                };

                                // 从 chat 组件收集消息
                                let messages: Vec<(String, String)> = chat
                                    .get_messages()
                                    .iter()
                                    .map(|m| {
                                        let role = match m.role {
                                            app::Role::User => "user",
                                            app::Role::Assistant => "assistant",
                                            app::Role::System => "system",
                                        };
                                        (role.to_string(), m.content.clone())
                                    })
                                    .collect();

                                let save_result =
                                    session_manager.save_session(&session_id, &messages, None);

                                match save_result {
                                    Ok(path) => {
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: format!(
                                                "Session saved to: {}",
                                                path.display()
                                            ),
                                        });
                                    }
                                    Err(e) => {
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: format!("Failed to save session: {}", e),
                                        });
                                    }
                                }
                                status.set_message_count(chat.message_count());
                            }
                            KeyAction::NewSession => {
                                // 清空当前会话并开始新会话
                                chat.clear();
                                tools.clear();
                                show_tools = false;
                                rt.block_on(async {
                                    let agent_guard = agent.read().await;
                                    agent_guard.clear_history().await;
                                });
                                chat.add_message(app::Message {
                                    role: app::Role::System,
                                    content: "New session started".to_string(),
                                });
                                status.set_message_count(chat.message_count());
                                // 生成新的会话 ID
                                let new_session_id = uuid::Uuid::new_v4().to_string();
                                app.set_session_id(new_session_id.clone());
                                status.set_session_id(Some(new_session_id));
                            }
                            KeyAction::ToggleTools => {
                                show_tools = !show_tools;
                            }
                            KeyAction::ShowHelp => {
                                let parser = CommandParser::new();
                                let help_text = parser.generate_help(None);
                                chat.add_message(app::Message {
                                    role: app::Role::System,
                                    content: help_text,
                                });
                                status.set_message_count(chat.message_count());
                            }
                            KeyAction::CancelTool => {
                                // Cancel the currently running tool (last running tool in list)
                                if tools.has_running() {
                                    // Find the last running tool index
                                    for (idx, call) in tools.tool_calls.iter().enumerate().rev() {
                                        if call.status == components::ToolStatus::Running {
                                            if tools.cancel_tool(idx) {
                                                chat.add_message(app::Message {
                                                    role: app::Role::System,
                                                    content: "Tool execution cancelled."
                                                        .to_string(),
                                                });
                                                status.set_message_count(chat.message_count());
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                            KeyAction::SlashCommand(cmd) => {
                                let result = execute_slash_command_with_agent(
                                    cmd,
                                    &mut CommandExecContext {
                                        chat,
                                        status,
                                        app,
                                        permissions,
                                    },
                                    agent.clone(),
                                    &rt,
                                );
                                handle_command_result(result, chat, status, app);
                            }
                            KeyAction::None => {}
                        }
                    }
                    Err(e) => {
                        tracing::error!("Key handling error: {}", e);
                    }
                }
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}

/// 键盘动作
enum KeyAction {
    /// 无动作
    None,
    /// 退出
    Exit,
    /// 发送消息
    SendMessage(String),
    /// 执行 Slash 命令
    SlashCommand(ParsedCommand),
    /// 清屏
    ClearScreen,
    /// 保存会话
    SaveSession,
    /// 新建会话
    NewSession,
    /// 切换工具面板
    ToggleTools,
    /// 显示帮助
    ShowHelp,
    /// 取消当前运行的工具
    CancelTool,
}

/// 执行 Slash 命令（带 Agent 访问）
fn execute_slash_command_with_agent(
    cmd: ParsedCommand,
    ctx: &mut CommandExecContext<'_>,
    agent: Arc<RwLock<AgentClient>>,
    rt: &tokio::runtime::Runtime,
) -> CommandResult {
    execute_slash_command(cmd, ctx, Some(agent), rt)
}

/// 执行 Slash 命令
fn execute_slash_command(
    cmd: ParsedCommand,
    ctx: &mut CommandExecContext<'_>,
    agent: Option<Arc<RwLock<AgentClient>>>,
    rt: &tokio::runtime::Runtime,
) -> CommandResult {
    let CommandExecContext {
        chat,
        status,
        app,
        permissions: _permissions,
    } = &mut *ctx;
    match cmd.command.name.as_str() {
        "help" => {
            let parser = CommandParser::new();
            let help_text = parser.generate_help(cmd.args.get("command").map(|s| s.as_str()));
            chat.add_message(app::Message {
                role: app::Role::System,
                content: help_text,
            });
            status.set_message_count(chat.message_count());
            CommandResult::Success("Help displayed".to_string())
        }
        "clear" => {
            chat.clear();
            status.set_message_count(0);
            CommandResult::Success("Screen cleared".to_string())
        }
        "new" => {
            chat.clear();
            status.set_message_count(0);
            let new_session_id = uuid::Uuid::new_v4().to_string();
            app.set_session_id(new_session_id.clone());
            status.set_session_id(Some(new_session_id));
            chat.add_message(app::Message {
                role: app::Role::System,
                content: "New session started".to_string(),
            });
            status.set_message_count(chat.message_count());
            CommandResult::Success("New session".to_string())
        }
        "exit" => {
            app.running = false;
            CommandResult::Exit
        }
        "tokens" => {
            let token_info = if let Some(agent_ref) = &agent {
                rt.block_on(async {
                    let agent_guard = agent_ref.read().await;
                    let usage = agent_guard.get_token_usage().await;
                    usage.format_report()
                })
            } else {
                "Token statistics unavailable (agent not initialized)".to_string()
            };
            chat.add_message(app::Message {
                role: app::Role::System,
                content: token_info,
            });
            status.set_message_count(chat.message_count());
            CommandResult::Success("Tokens info".to_string())
        }
        "debug" => {
            // 切换调试模式
            let new_state = app.toggle_debug_mode();
            let state_str = if new_state { "enabled" } else { "disabled" };

            // 更新状态栏
            status.set_debug_mode(new_state);

            // 根据调试模式设置日志级别
            if new_state {
                tracing::debug!("Debug mode enabled in TUI");
            }

            chat.add_message(app::Message {
                role: app::Role::System,
                content: format!(
                    "Debug mode {}.\n\nWhen enabled:\n  - Detailed logs shown in console\n  - Verbose error messages\n  - Internal state visible\n\nCurrent log level: {}",
                    state_str,
                    if new_state { "DEBUG" } else { "INFO" }
                ),
            });
            status.set_message_count(chat.message_count());
            CommandResult::Success(format!("Debug mode {}", state_str))
        }
        "config" => {
            let config_path = ConfigManager::default_config_path();
            let mut config = ConfigManager::new();

            if config_path.exists() {
                if let Err(e) = config.load_from_file_sync(&config_path) {
                    chat.add_message(app::Message {
                        role: app::Role::System,
                        content: format!("Failed to load config: {}", e),
                    });
                    status.set_message_count(chat.message_count());
                    return CommandResult::Error(format!("Config load error: {}", e));
                }
            }

            config.resolve_env_refs();

            let key_opt = cmd.args.get("key").cloned();
            let value_opt = cmd.args.get("value").cloned();

            if let Some(key) = key_opt {
                if let Some(value) = value_opt {
                    // 设置配置项
                    if key.starts_with("settings.") {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() >= 2 {
                            let field = parts[1];
                            let result: Result<String, anyhow::Error> = match field {
                                "session_auto_save" => value
                                    .parse::<bool>()
                                    .map(|v| {
                                        config.settings.session_auto_save = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "session_max_history" => value
                                    .parse::<usize>()
                                    .map(|v| {
                                        config.settings.session_max_history = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "checkpoint_enabled" => value
                                    .parse::<bool>()
                                    .map(|v| {
                                        config.settings.checkpoint_enabled = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "checkpoint_interval" => value
                                    .parse::<u32>()
                                    .map(|v| {
                                        config.settings.checkpoint_interval_sec = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "audit_enabled" => value
                                    .parse::<bool>()
                                    .map(|v| {
                                        config.settings.audit_enabled = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "mcp_enabled" => value
                                    .parse::<bool>()
                                    .map(|v| {
                                        config.settings.mcp_enabled = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                _ => Err(anyhow::anyhow!("Unknown setting: {}", field)),
                            };

                            match result {
                                Ok(display_value) => {
                                    if let Err(e) = config.save_sync(&config_path) {
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: format!("Failed to save config: {}", e),
                                        });
                                    } else {
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: format!("Set {} = {}", key, display_value),
                                        });
                                    }
                                }
                                Err(e) => {
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: format!("Invalid value: {}", e),
                                    });
                                }
                            }
                        }
                    } else if key.starts_with("provider.") {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() >= 3 {
                            let provider_name = parts[1];
                            let field = parts[2];
                            let provider = config
                                .providers
                                .entry(provider_name.to_string())
                                .or_default();
                            let result: Result<String, anyhow::Error> = match field {
                                "base_url" => {
                                    provider.base_url = value.clone();
                                    Ok(value)
                                }
                                "model" => {
                                    provider.model = value.clone();
                                    Ok(value)
                                }
                                "max_tokens" => value
                                    .parse::<u32>()
                                    .map(|v| {
                                        provider.default_max_tokens = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "temperature" => value
                                    .parse::<f32>()
                                    .map(|v| {
                                        provider.default_temperature = v;
                                        v.to_string()
                                    })
                                    .map_err(|e| anyhow::anyhow!("{}", e)),
                                "api_key" => {
                                    provider.api_key = value.clone();
                                    Ok("(set)".to_string())
                                }
                                _ => Err(anyhow::anyhow!("Unknown field: {}", field)),
                            };

                            match result {
                                Ok(display_value) => {
                                    if let Err(e) = config.save_sync(&config_path) {
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: format!("Failed to save config: {}", e),
                                        });
                                    } else {
                                        chat.add_message(app::Message {
                                            role: app::Role::System,
                                            content: format!("Set {} = {}", key, display_value),
                                        });
                                    }
                                }
                                Err(e) => {
                                    chat.add_message(app::Message {
                                        role: app::Role::System,
                                        content: format!("Invalid value: {}", e),
                                    });
                                }
                            }
                        }
                    } else {
                        config.set(key.clone(), value.clone());
                        if let Err(e) = config.save_sync(&config_path) {
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: format!("Failed to save config: {}", e),
                            });
                        } else {
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: format!("Set {} = {}", key, value),
                            });
                        }
                    }
                } else {
                    // 显示特定配置项
                    let display = if key.starts_with("provider.") {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() >= 2 {
                            let provider_name = parts[1];
                            if let Some(provider) = config.providers.get(provider_name) {
                                if parts.len() == 2 {
                                    format!("Provider '{}':\n  base_url: {}\n  model: {}\n  max_tokens: {}\n  temperature: {}",
                                        provider_name, provider.base_url, provider.model,
                                        provider.default_max_tokens, provider.default_temperature)
                                } else if parts.len() >= 3 {
                                    let field = parts[2];
                                    match field {
                                        "api_key" => "api_key: (set)".to_string(),
                                        "base_url" => format!("base_url: {}", provider.base_url),
                                        "model" => format!("model: {}", provider.model),
                                        "max_tokens" => {
                                            format!("max_tokens: {}", provider.default_max_tokens)
                                        }
                                        "temperature" => {
                                            format!("temperature: {}", provider.default_temperature)
                                        }
                                        _ => format!("Unknown field: {}", field),
                                    }
                                } else {
                                    format!("Provider '{}' not found", provider_name)
                                }
                            } else {
                                format!("Provider '{}' not found", provider_name)
                            }
                        } else {
                            format!("Invalid key: {}", key)
                        }
                    } else if key.starts_with("settings.") {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() >= 2 {
                            let field = parts[1];
                            match field {
                                "session_auto_save" => format!(
                                    "session_auto_save: {}",
                                    config.settings.session_auto_save
                                ),
                                "session_max_history" => format!(
                                    "session_max_history: {}",
                                    config.settings.session_max_history
                                ),
                                "checkpoint_enabled" => format!(
                                    "checkpoint_enabled: {}",
                                    config.settings.checkpoint_enabled
                                ),
                                "checkpoint_interval" => format!(
                                    "checkpoint_interval: {}s",
                                    config.settings.checkpoint_interval_sec
                                ),
                                "audit_enabled" => {
                                    format!("audit_enabled: {}", config.settings.audit_enabled)
                                }
                                "mcp_enabled" => {
                                    format!("mcp_enabled: {}", config.settings.mcp_enabled)
                                }
                                _ => format!("Unknown setting: {}", field),
                            }
                        } else {
                            format!("Invalid key: {}", key)
                        }
                    } else {
                        match config.get(&key) {
                            Some(v) => format!("{}: {}", key, v),
                            None => format!("Key '{}' not found", key),
                        }
                    };
                    chat.add_message(app::Message {
                        role: app::Role::System,
                        content: display,
                    });
                }
            } else {
                // 显示所有配置
                let mut content = format!("Active provider: {}\n\n", config.active_provider);
                content.push_str("Providers:\n");
                for (name, provider) in &config.providers {
                    let marker = if name == &config.active_provider {
                        " (active)"
                    } else {
                        ""
                    };
                    content.push_str(&format!("  [{}]{}\n", name, marker));
                    content.push_str(&format!("    base_url: {}\n", provider.base_url));
                    content.push_str(&format!("    model: {}\n", provider.model));
                    content.push_str(&format!(
                        "    max_tokens: {}\n",
                        provider.default_max_tokens
                    ));
                    content.push_str(&format!(
                        "    temperature: {}\n",
                        provider.default_temperature
                    ));
                }
                content.push_str("\nSettings:\n");
                content.push_str(&format!(
                    "  session_auto_save: {}\n",
                    config.settings.session_auto_save
                ));
                content.push_str(&format!(
                    "  session_max_history: {}\n",
                    config.settings.session_max_history
                ));
                content.push_str(&format!(
                    "  checkpoint_enabled: {}\n",
                    config.settings.checkpoint_enabled
                ));
                content.push_str(&format!(
                    "  checkpoint_interval: {}s\n",
                    config.settings.checkpoint_interval_sec
                ));
                content.push_str(&format!(
                    "  audit_enabled: {}\n",
                    config.settings.audit_enabled
                ));
                content.push_str(&format!("  mcp_enabled: {}\n", config.settings.mcp_enabled));

                if !config.extra.is_empty() {
                    content.push_str("\nExtra config:\n");
                    for (k, v) in &config.extra {
                        content.push_str(&format!("  {}: {}\n", k, v));
                    }
                }

                chat.add_message(app::Message {
                    role: app::Role::System,
                    content,
                });
            }
            status.set_message_count(chat.message_count());
            CommandResult::Success("Config displayed".to_string())
        }
        "model" => {
            if let Some(agent_ref) = &agent {
                if let Some(model_name) = cmd.args.get("name") {
                    // 切换模型
                    let switch_result = rt.block_on(async {
                        let agent_guard = agent_ref.read().await;
                        agent_guard.set_model(model_name).await
                    });

                    match switch_result {
                        Ok(()) => {
                            // 更新状态栏显示
                            status.set_model(Some(model_name.clone()));
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: format!("Model switched to: {}", model_name),
                            });
                        }
                        Err(e) => {
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: format!("Failed to switch model: {}", e),
                            });
                        }
                    }
                } else {
                    // 显示当前模型和可用模型列表
                    let current_model = rt.block_on(async {
                        let agent_guard = agent_ref.read().await;
                        agent_guard.current_model().await
                    });

                    let available_models = rt.block_on(async {
                        let agent_guard = agent_ref.read().await;
                        agent_guard.list_available_models().await
                    });

                    let content = if let Some(model) = current_model {
                        let models_list = available_models.join(", ");
                        format!(
                            "Current model: {}\nAvailable models: {}",
                            model, models_list
                        )
                    } else {
                        "Agent not initialized. Please configure first.".to_string()
                    };

                    chat.add_message(app::Message {
                        role: app::Role::System,
                        content,
                    });
                }
            } else {
                chat.add_message(app::Message {
                    role: app::Role::System,
                    content: "Agent not available".to_string(),
                });
            }
            status.set_message_count(chat.message_count());
            CommandResult::Success("Model info".to_string())
        }
        "provider" => {
            if let Some(agent_ref) = &agent {
                if let Some(provider_name) = cmd.args.get("name") {
                    // 切换提供商
                    let switch_result = rt.block_on(async {
                        let agent_guard = agent_ref.read().await;
                        agent_guard.set_provider(provider_name).await
                    });

                    match switch_result {
                        Ok(()) => {
                            // 更新状态栏显示
                            status.set_provider(Some(provider_name.clone()));
                            // 更新模型显示（切换提供商后模型可能变化）
                            let current_model = rt.block_on(async {
                                let agent_guard = agent_ref.read().await;
                                agent_guard.current_model().await
                            });
                            status.set_model(current_model);
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: format!("Provider switched to: {}", provider_name),
                            });
                        }
                        Err(e) => {
                            chat.add_message(app::Message {
                                role: app::Role::System,
                                content: format!("Failed to switch provider: {}", e),
                            });
                        }
                    }
                } else {
                    // 显示当前提供商和已配置的提供商列表
                    let current_provider = rt.block_on(async {
                        let agent_guard = agent_ref.read().await;
                        agent_guard.current_provider_name().await
                    });

                    let providers = rt.block_on(async {
                        let agent_guard = agent_ref.read().await;
                        agent_guard.list_providers().await
                    });

                    let providers_list = providers.join(", ");
                    chat.add_message(app::Message {
                        role: app::Role::System,
                        content: format!(
                            "Current provider: {}\nConfigured providers: {}",
                            current_provider, providers_list
                        ),
                    });
                }
            } else {
                chat.add_message(app::Message {
                    role: app::Role::System,
                    content: "Agent not available".to_string(),
                });
            }
            status.set_message_count(chat.message_count());
            CommandResult::Success("Provider info".to_string())
        }
        "tools" => {
            if let Some(agent_ref) = &agent {
                // 获取工具列表（同步方法）
                let tools = rt.block_on(async {
                    let agent_guard = agent_ref.read().await;
                    agent_guard.list_tools()
                });

                if tools.is_empty() {
                    chat.add_message(app::Message {
                        role: app::Role::System,
                        content: "No tools available.".to_string(),
                    });
                } else {
                    // 按类别分组
                    let mut categories: std::collections::HashMap<String, Vec<_>> =
                        std::collections::HashMap::new();

                    for (name, desc, category) in tools {
                        categories.entry(category).or_default().push((name, desc));
                    }

                    let mut content = String::from("Available tools:\n");

                    // 按类别输出
                    let mut sorted_categories: Vec<_> = categories.keys().collect();
                    sorted_categories.sort();

                    for category in sorted_categories {
                        content.push_str(&format!("\n[{}]\n", category));
                        if let Some(tools_in_cat) = categories.get(category) {
                            for (name, desc) in tools_in_cat {
                                content.push_str(&format!("  {} - {}\n", name, desc));
                            }
                        }
                    }

                    content.push_str(&format!(
                        "\nTotal: {} tools",
                        categories.values().map(|v| v.len()).sum::<usize>()
                    ));

                    chat.add_message(app::Message {
                        role: app::Role::System,
                        content,
                    });
                }
            } else {
                chat.add_message(app::Message {
                    role: app::Role::System,
                    content: "Agent not available".to_string(),
                });
            }
            status.set_message_count(chat.message_count());
            CommandResult::Success("Tools listed".to_string())
        }
        "tutorial" => {
            let tutorial_text = if let Some(step_str) = cmd.args.get("step") {
                if let Ok(step_num) = step_str.parse::<usize>() {
                    let mut tutorial = tutorial::Tutorial::new();
                    if let Some(step) = tutorial.jump_to(step_num) {
                        tutorial::Tutorial::format_step(step)
                    } else {
                        "Invalid step number. Use /tutorial 1-5 or /tutorial for overview."
                            .to_string()
                    }
                } else {
                    "Invalid step number. Use /tutorial 1-5 or /tutorial for overview.".to_string()
                }
            } else {
                tutorial::Tutorial::overview()
            };
            chat.add_message(app::Message {
                role: app::Role::System,
                content: tutorial_text,
            });
            status.set_message_count(chat.message_count());
            CommandResult::Success("Tutorial displayed".to_string())
        }
        "bash" | "write" | "edit" | "git" => {
            // 高风险命令需要确认
            CommandResult::NeedsConfirmation {
                command: cmd,
                message: "This operation may modify files or execute commands.".to_string(),
            }
        }
        _ => CommandResult::Error(format!("Unknown command: /{}", cmd.command.name)),
    }
}

/// 处理命令执行结果
fn handle_command_result(
    result: CommandResult,
    chat: &mut ChatComponent,
    status: &mut StatusComponent,
    app: &mut App,
) {
    match result {
        CommandResult::Success(msg) => {
            tracing::debug!("Command success: {}", msg);
        }
        CommandResult::NeedsConfirmation {
            command: _,
            message,
        } => {
            chat.add_message(app::Message {
                role: app::Role::System,
                content: format!("Waiting for confirmation: {}", message),
            });
            status.set_message_count(chat.message_count());
        }
        CommandResult::Error(msg) => {
            let friendly = ErrorFriendlyizer::from_raw_error(&msg);
            chat.add_message(app::Message {
                role: app::Role::System,
                content: friendly.format(),
            });
            status.set_message_count(chat.message_count());
        }
        CommandResult::Exit => {
            app.running = false;
        }
        CommandResult::NoOp => {}
    }
}

/// 处理键盘事件
fn handle_key_event(
    key: KeyEvent,
    ctx: &mut KeyEventHandlerContext<'_>,
    command_parser: &CommandParser,
    agent: Option<Arc<RwLock<AgentClient>>>,
    rt: &tokio::runtime::Runtime,
) -> Result<KeyAction> {
    let KeyEventHandlerContext {
        app,
        chat,
        input,
        status,
        key_hints,
        confirmation,
        permissions,
        show_completions,
    } = &mut *ctx;

    match (key.modifiers, key.code) {
        // Ctrl+C: 退出
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            app.running = false;
            Ok(KeyAction::Exit)
        }

        // Ctrl+D: 退出
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            app.running = false;
            Ok(KeyAction::Exit)
        }

        // Ctrl+L: 清屏
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => Ok(KeyAction::ClearScreen),

        // Ctrl+S: 保存会话
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Ok(KeyAction::SaveSession),

        // Ctrl+N: 新建会话
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Ok(KeyAction::NewSession),

        // Ctrl+T: 切换工具面板
        (KeyModifiers::CONTROL, KeyCode::Char('t')) => Ok(KeyAction::ToggleTools),

        // Ctrl+? 或 F1: 切换快捷键提示展开
        (KeyModifiers::CONTROL, KeyCode::Char('?')) | (KeyModifiers::NONE, KeyCode::F(1)) => {
            key_hints.toggle_expanded();
            Ok(KeyAction::None)
        }

        // Ctrl+H: 显示帮助
        (KeyModifiers::CONTROL, KeyCode::Char('h')) => Ok(KeyAction::ShowHelp),

        // Ctrl+X: 取消当前运行的工具
        (KeyModifiers::CONTROL, KeyCode::Char('x')) => Ok(KeyAction::CancelTool),

        // Ctrl+W: 删除前一个单词
        (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
            input.delete_word();
            Ok(KeyAction::None)
        }

        // Ctrl+A: 移动到行首
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
            input.move_cursor_to_start();
            Ok(KeyAction::None)
        }

        // Ctrl+E: 移动到行尾
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
            input.move_cursor_to_end();
            Ok(KeyAction::None)
        }

        // Alt+B: 后退一个单词
        (KeyModifiers::ALT, KeyCode::Char('b')) => {
            input.move_word_left();
            Ok(KeyAction::None)
        }

        // Alt+F: 前进一个单词
        (KeyModifiers::ALT, KeyCode::Char('f')) => {
            input.move_word_right();
            Ok(KeyAction::None)
        }

        // Alt+Enter 或 Shift+Enter: 插入换行
        (KeyModifiers::ALT, KeyCode::Enter) | (KeyModifiers::SHIFT, KeyCode::Enter) => {
            input.insert_newline();
            Ok(KeyAction::None)
        }

        // Tab: 命令补全
        (KeyModifiers::NONE, KeyCode::Tab) => {
            let current_input = input.get_input();
            if current_input.starts_with('/') {
                let completions = command_parser.get_completions(current_input);
                if !completions.is_empty() {
                    // 补全到第一个匹配项
                    if let Some(cmd) = completions.first() {
                        input.set_input(format!("/{} ", cmd.name));
                    }
                }
            }
            Ok(KeyAction::None)
        }

        // Enter: 发送消息或执行命令
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if input.get_input().is_empty() {
                return Ok(KeyAction::None);
            }

            let content = input.get_input().to_string();

            // 检查是否是 Slash 命令
            if let Some(parsed) = command_parser.parse(&content) {
                // 添加到历史记录
                input.add_to_history(&content);
                input.clear();

                // 显示用户输入的命令
                chat.add_message(app::Message {
                    role: app::Role::User,
                    content: content.clone(),
                });
                status.set_message_count(chat.message_count());

                // 检查是否需要确认
                if parsed.command.requires_confirmation {
                    let details = vec![
                        format!("命令: /{}", parsed.command.name),
                        format!("描述: {}", parsed.command.description),
                    ];
                    confirmation.show(parsed, "执行命令", details);
                    return Ok(KeyAction::None);
                }

                // 执行命令
                let result = execute_slash_command(
                    parsed,
                    &mut CommandExecContext {
                        chat,
                        status,
                        app,
                        permissions,
                    },
                    agent,
                    rt,
                );
                handle_command_result(result, chat, status, app);
                Ok(KeyAction::None)
            } else {
                // 普通消息
                chat.add_message(app::Message {
                    role: app::Role::User,
                    content: content.clone(),
                });
                input.add_to_history(&content);
                input.clear();
                status.set_message_count(chat.message_count());
                Ok(KeyAction::SendMessage(content))
            }
        }

        // Esc: 取消/清空输入
        (KeyModifiers::NONE, KeyCode::Esc) => {
            input.clear();
            **show_completions = false;
            Ok(KeyAction::None)
        }

        // Backspace: 删除字符
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            input.delete_char();
            Ok(KeyAction::None)
        }

        // Delete: 删除字符
        (KeyModifiers::NONE, KeyCode::Delete) => {
            input.delete_char();
            Ok(KeyAction::None)
        }

        // 左箭头: 移动光标左
        (KeyModifiers::NONE, KeyCode::Left) => {
            input.move_cursor_left();
            Ok(KeyAction::None)
        }

        // 右箭头: 移动光标右
        (KeyModifiers::NONE, KeyCode::Right) => {
            input.move_cursor_right();
            Ok(KeyAction::None)
        }

        // Home: 移动到开始
        (KeyModifiers::NONE, KeyCode::Home) => {
            input.move_cursor_to_start();
            Ok(KeyAction::None)
        }

        // End: 移动到结束
        (KeyModifiers::NONE, KeyCode::End) => {
            input.move_cursor_to_end();
            Ok(KeyAction::None)
        }

        // 上箭头: 向上滚动或历史记录
        (KeyModifiers::NONE, KeyCode::Up) => {
            if input.get_input().is_empty() {
                chat.scroll_up(1);
            } else {
                input.history_prev();
            }
            Ok(KeyAction::None)
        }

        // 下箭头: 向下滚动或历史记录
        (KeyModifiers::NONE, KeyCode::Down) => {
            if input.get_input().is_empty() {
                chat.scroll_down(1);
            } else {
                input.history_next();
            }
            Ok(KeyAction::None)
        }

        // Page Up: 向上滚动一页
        (KeyModifiers::NONE, KeyCode::PageUp) => {
            chat.scroll_up(10);
            Ok(KeyAction::None)
        }

        // Page Down: 向下滚动一页
        (KeyModifiers::NONE, KeyCode::PageDown) => {
            chat.scroll_down(10);
            Ok(KeyAction::None)
        }

        // 普通字符输入
        (KeyModifiers::NONE, KeyCode::Char(c)) => {
            input.push_char(c);

            // 检测是否输入了 / 开头，显示补全
            let current_input = input.get_input();
            **show_completions = current_input.starts_with('/') && current_input.len() > 1;

            Ok(KeyAction::None)
        }

        // 忽略其他组合键
        _ => Ok(KeyAction::None),
    }
}

/// Save provider configuration to config file
fn save_provider_config(provider: &str, api_key: &str) -> anyhow::Result<()> {
    use sh_core::layer1::ProviderConfig;

    let config_path = sh_core::layer1::ConfigManager::default_config_path();
    let mut config = sh_core::layer1::ConfigManager::new();

    // Load existing config if exists
    if config_path.exists() {
        config.load_from_file_sync(&config_path)?;
    }

    // Create provider config with default values
    let provider_config = match provider {
        "anthropic" => ProviderConfig {
            api_key: api_key.to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            api_format: "anthropic".to_string(),
            default_max_tokens: 4096,
            default_temperature: 1.0,
        },
        "openai" => ProviderConfig {
            api_key: api_key.to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-5.5".to_string(),
            api_format: "openai".to_string(),
            default_max_tokens: 4096,
            default_temperature: 1.0,
        },
        "google" => ProviderConfig {
            api_key: api_key.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            model: "gemini-3.0-pro".to_string(),
            api_format: "google".to_string(),
            default_max_tokens: 8192,
            default_temperature: 1.0,
        },
        _ => {
            return Err(anyhow::anyhow!("Unknown provider: {}", provider));
        }
    };

    // Add/update the provider
    config.add_provider(provider, provider_config);

    // Set as active provider
    config.use_provider(provider)?;

    // Save the config
    config.save_sync(&config_path)?;

    // Also set environment variable for current session
    match provider {
        "anthropic" => std::env::set_var("ANTHROPIC_API_KEY", api_key),
        "openai" => std::env::set_var("OPENAI_API_KEY", api_key),
        "google" => std::env::set_var("GOOGLE_API_KEY", api_key),
        _ => {}
    }

    Ok(())
}
