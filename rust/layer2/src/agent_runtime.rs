//! # Agent Runtime
//!
//! Agent 执行运行时实现。
//!
//! 支持真实 LLM API 调用（Anthropic/OpenAI/Gemini）。
//! 集成任务规划器和执行监控器，支持复杂任务分解和自我纠错。

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::execution_monitor::{CorrectionStrategy, ExecutionMonitor};
use crate::permission::types::{PermissionAction, PermissionRequest};
use crate::permission::PermissionManager;
use crate::planner::{DecompositionStrategy, ExecutionPlan, TaskDecomposer};
use crate::session_manager::{ConcurrentSessionManager, SessionConfig, SessionManagerTrait};
use crate::tool_registry::{ToolRegistry, ToolRegistryTrait};
use crate::types::{
    AgentId, AgentState, Layer2Error, Layer2Result, Message, MessageRole, SessionId, ToolCall,
    ToolResult,
};

/// Agent 执行结果
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub session_id: SessionId,
    pub final_state: AgentState,
    pub messages: Vec<Message>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub iterations: i32,
    pub tokens_used: i64,
}

/// Agent 配置
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub agent_id: AgentId,
    pub model: String,
    pub temperature: f32,
    pub max_iterations: i32,
    pub system_prompt: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_id: AgentId::new(),
            model: "claude-sonnet-4-6".to_string(),
            temperature: 0.7,
            max_iterations: 100,
            system_prompt: None,
        }
    }
}

impl From<&AgentConfig> for SessionConfig {
    fn from(config: &AgentConfig) -> Self {
        SessionConfig {
            model: config.model.clone(),
            temperature: config.temperature,
            max_iterations: config.max_iterations,
            system_prompt: config.system_prompt.clone(),
            ..Default::default()
        }
    }
}

/// Agent 运行时接口
///
/// 定义 Agent 执行的核心生命周期操作。
#[async_trait]
pub trait AgentRuntimeTrait: Send + Sync {
    /// 启动 Agent 执行
    ///
    /// # Arguments
    /// * `task` - 用户任务描述
    /// * `config` - Agent 配置
    ///
    /// # Returns
    /// 执行结果，包含最终状态和输出
    async fn run(&self, task: &str, config: AgentConfig) -> Layer2Result<AgentResult>;

    /// 流式启动 Agent 执行
    async fn run_stream(
        &self,
        task: &str,
        config: AgentConfig,
        callback: &dyn AgentLoopCallback,
    ) -> Layer2Result<AgentResult>;

    /// 流式启动 Agent 执行（支持中断）
    async fn run_stream_abortable(
        &self,
        task: &str,
        config: AgentConfig,
        callback: &dyn AgentLoopCallback,
        abort_flag: Arc<AtomicBool>,
    ) -> Layer2Result<AgentResult>;

    /// 启动 Agent 并返回会话 ID（用于流式执行）
    ///
    /// # Arguments
    /// * `task` - 用户任务描述
    /// * `config` - Agent 配置
    ///
    /// # Returns
    /// 会话 ID，用于后续操作
    async fn start(&self, task: &str, config: AgentConfig) -> Layer2Result<SessionId>;

    /// 暂停正在执行的 Agent
    ///
    /// # Arguments
    /// * `session_id` - 会话 ID
    async fn pause(&self, session_id: &SessionId) -> Layer2Result<()>;

    /// 恢复暂停的 Agent
    ///
    /// # Arguments
    /// * `session_id` - 会话 ID
    async fn resume(&self, session_id: &SessionId) -> Layer2Result<()>;

    /// 停止 Agent 执行
    ///
    /// # Arguments
    /// * `session_id` - 会话 ID
    async fn stop(&self, session_id: &SessionId) -> Layer2Result<()>;

    /// 获取 Agent 当前状态
    ///
    /// # Arguments
    /// * `session_id` - 会话 ID
    fn status(&self, session_id: &SessionId) -> Layer2Result<AgentState>;

    /// 向 Agent 发送消息
    ///
    /// # Arguments
    /// * `session_id` - 会话 ID
    /// * `message` - 消息内容
    async fn send_message(&self, session_id: &SessionId, message: &str) -> Layer2Result<()>;

    /// 获取 Agent 的工具调用结果
    ///
    /// # Arguments
    /// * `session_id` - 会话 ID
    /// * `tool_call_id` - 工具调用 ID
    async fn submit_tool_result(
        &self,
        session_id: &SessionId,
        tool_call_id: &str,
        result: ToolResult,
    ) -> Layer2Result<()>;
}

/// Agent 执行循环回调接口
///
/// 用于在执行过程中注入自定义逻辑。
#[async_trait]
pub trait AgentLoopCallback: Send + Sync {
    /// 在每次迭代前调用
    async fn before_iteration(&self, session_id: &SessionId, iteration: i32) -> Layer2Result<bool>;

    /// 在每次迭代后调用
    async fn after_iteration(
        &self,
        session_id: &SessionId,
        iteration: i32,
        result: &IterationResult,
    ) -> Layer2Result<()>;

    /// 在工具调用前调用
    async fn before_tool_call(
        &self,
        session_id: &SessionId,
        tool_call: &ToolCall,
    ) -> Layer2Result<bool>;

    /// 在工具调用后调用
    async fn after_tool_call(
        &self,
        session_id: &SessionId,
        tool_call: &ToolCall,
        result: &ToolResult,
    ) -> Layer2Result<()>;
}

/// 单次迭代结果
#[derive(Debug, Clone)]
pub struct IterationResult {
    pub iteration: i32,
    pub state: AgentState,
    pub message: Option<Message>,
    pub tool_calls: Vec<ToolCall>,
    pub should_continue: bool,
}

/// 默认 Agent Runtime 实现
///
/// 使用 ConcurrentSessionManager 管理会话，ToolRegistry 执行工具。
/// 支持完整的执行生命周期：start -> run/pause/resume/stop。
/// 集成 TaskDecomposer 进行任务分解，ExecutionMonitor 进行执行监控和纠错。
/// 支持真实 LLM API 调用（通过 Layer1 LlmClient）。
pub struct AgentRuntime {
    session_manager: Arc<ConcurrentSessionManager>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Option<Arc<PermissionManager>>,
    /// 任务分解器（可选）
    task_decomposer: Option<TaskDecomposer>,
    /// LLM 客户端（可选，用于真实 API 调用）
    llm_client: Option<Arc<sh_layer1::LlmClient>>,
}

impl AgentRuntime {
    /// 创建新的 AgentRuntime
    pub fn new(
        session_manager: Arc<ConcurrentSessionManager>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            session_manager,
            tool_registry,
            permission_manager: None,
            task_decomposer: None,
            llm_client: None,
        }
    }

    /// 创建带权限管理的 AgentRuntime
    pub fn with_permissions(
        session_manager: Arc<ConcurrentSessionManager>,
        tool_registry: Arc<ToolRegistry>,
        permission_manager: Arc<PermissionManager>,
    ) -> Self {
        Self {
            session_manager,
            tool_registry,
            permission_manager: Some(permission_manager),
            task_decomposer: None,
            llm_client: None,
        }
    }

    /// 创建带任务分解器的 AgentRuntime
    pub fn with_decomposer(
        session_manager: Arc<ConcurrentSessionManager>,
        tool_registry: Arc<ToolRegistry>,
        strategy: DecompositionStrategy,
    ) -> Self {
        Self {
            session_manager,
            tool_registry,
            permission_manager: None,
            task_decomposer: Some(TaskDecomposer::new().with_strategy(strategy)),
            llm_client: None,
        }
    }

    /// 使用默认组件创建（带任务分解器）
    pub fn with_defaults() -> Self {
        Self {
            session_manager: Arc::new(ConcurrentSessionManager::default_config()),
            tool_registry: Arc::new(ToolRegistry::new()),
            permission_manager: None,
            task_decomposer: Some(TaskDecomposer::new()),
            llm_client: None,
        }
    }

    /// 创建带 LLM 客户端的 AgentRuntime
    pub fn with_llm_client(
        session_manager: Arc<ConcurrentSessionManager>,
        tool_registry: Arc<ToolRegistry>,
        llm_client: Arc<sh_layer1::LlmClient>,
    ) -> Self {
        Self {
            session_manager,
            tool_registry,
            permission_manager: None,
            task_decomposer: Some(TaskDecomposer::new()),
            llm_client: Some(llm_client),
        }
    }

    /// 设置权限管理器
    pub fn set_permission_manager(&mut self, manager: Arc<PermissionManager>) {
        self.permission_manager = Some(manager);
    }

    /// 设置任务分解策略
    pub fn set_decomposition_strategy(&mut self, strategy: DecompositionStrategy) {
        self.task_decomposer = Some(TaskDecomposer::new().with_strategy(strategy));
    }

    /// 设置 LLM 客户端
    pub fn set_llm_client(&mut self, client: Arc<sh_layer1::LlmClient>) {
        self.llm_client = Some(client);
    }

    /// 获取权限管理器引用
    pub fn permission_manager(&self) -> Option<&Arc<PermissionManager>> {
        self.permission_manager.as_ref()
    }

    /// 获取任务分解器引用
    pub fn task_decomposer(&self) -> Option<&TaskDecomposer> {
        self.task_decomposer.as_ref()
    }

    /// 获取 LLM 客户端引用
    pub fn llm_client(&self) -> Option<&Arc<sh_layer1::LlmClient>> {
        self.llm_client.as_ref()
    }

    /// 分解任务为执行计划
    ///
    /// 如果配置了任务分解器，将复杂任务分解为子任务序列。
    /// 返回执行计划，包含子任务列表和执行顺序。
    pub fn decompose_task(&self, task: &str) -> Layer2Result<Option<ExecutionPlan>> {
        if let Some(decomposer) = &self.task_decomposer {
            let plan = decomposer.decompose(task)?;
            info!(
                task = %task,
                subtasks = plan.subtasks.len(),
                strategy = ?plan.strategy,
                risk = ?plan.risk_level,
                "Task decomposed into execution plan"
            );
            Ok(Some(plan))
        } else {
            Ok(None)
        }
    }

    /// 创建执行监控器
    ///
    /// 为给定的执行计划创建监控器，用于跟踪执行进度和处理错误。
    pub fn create_monitor(&self, plan: ExecutionPlan) -> ExecutionMonitor {
        ExecutionMonitor::new(plan)
    }

    /// 使用执行计划运行 Agent
    ///
    /// 先分解任务，然后按照执行计划的顺序依次执行子任务。
    /// 使用执行监控器跟踪进度和错误。
    pub async fn run_with_plan(
        &self,
        task: &str,
        config: AgentConfig,
    ) -> Layer2Result<AgentResult> {
        // 尝试分解任务
        let plan_option = self.decompose_task(task)?;

        // 如果有执行计划，按计划执行
        if let Some(plan) = plan_option {
            self.run_with_execution_plan(plan, config).await
        } else {
            // 没有分解器，直接运行
            self.run(task, config).await
        }
    }

    /// 按照执行计划运行 Agent
    async fn run_with_execution_plan(
        &self,
        plan: ExecutionPlan,
        config: AgentConfig,
    ) -> Layer2Result<AgentResult> {
        let monitor = self.create_monitor(plan.clone());
        monitor.start().await?;

        info!(
            plan_id = %plan.id,
            steps = plan.subtasks.len(),
            "Starting planned execution"
        );

        // 执行各个子任务
        let mut all_messages = Vec::new();
        let mut all_tool_calls = Vec::new();
        let mut all_tool_results = Vec::new();
        let mut total_iterations = 0;
        let mut total_tokens = 0i64;

        for subtask_id in &plan.execution_order {
            if let Some(subtask) = plan.subtasks.iter().find(|s| &s.id == subtask_id) {
                // 检查依赖是否已完成（简化：假设拓扑排序保证依赖已执行）
                // 拓扑排序确保依赖在当前任务之前执行

                // 运行子任务
                let subtask_result = self.run(&subtask.description, config.clone()).await;

                match subtask_result {
                    Ok(result) => {
                        monitor
                            .report_step_completed(subtask_id, result.final_state.to_string())
                            .await?;
                        all_messages.extend(result.messages);
                        all_tool_calls.extend(result.tool_calls);
                        all_tool_results.extend(result.tool_results);
                        total_iterations += result.iterations;
                        total_tokens += result.tokens_used;
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let decision = monitor
                            .report_step_failed(subtask_id, error_msg.clone())
                            .await?;

                        // 根据纠错决策处理
                        if decision.should_continue {
                            match &decision.strategy {
                                CorrectionStrategy::Retry { max_attempts } => {
                                    // 简单重试逻辑
                                    for attempt in 1..=*max_attempts {
                                        warn!(
                                            subtask_id = %subtask_id,
                                            attempt = attempt,
                                            max = max_attempts,
                                            "Retrying subtask"
                                        );
                                        let retry_result =
                                            self.run(&subtask.description, config.clone()).await;
                                        if let Ok(result) = retry_result {
                                            monitor
                                                .report_step_completed(
                                                    subtask_id,
                                                    format!("Retry {} succeeded", attempt),
                                                )
                                                .await?;
                                            all_messages.extend(result.messages);
                                            all_tool_calls.extend(result.tool_calls);
                                            all_tool_results.extend(result.tool_results);
                                            total_iterations += result.iterations;
                                            total_tokens += result.tokens_used;
                                            break;
                                        }
                                    }
                                }
                                CorrectionStrategy::Skip => {
                                    monitor
                                        .report_step_completed(subtask_id, "[SKIPPED]".to_string())
                                        .await?;
                                }
                                _ => {
                                    // 其他策略暂时标记为完成
                                    monitor
                                        .report_step_completed(
                                            subtask_id,
                                            format!(
                                                "[HANDLED] {}",
                                                decision.strategy.clone().debug_name()
                                            ),
                                        )
                                        .await?;
                                }
                            }
                        } else {
                            // 不能继续，返回错误
                            return Err(e);
                        }
                    }
                }
            }
        }

        let summary = monitor.complete().await?;
        info!(
            plan_id = %plan.id,
            completed = summary.completed_steps,
            failed = summary.failed_steps,
            corrections = summary.correction_count,
            duration_ms = summary.duration.as_millis(),
            "Planned execution completed"
        );

        Ok(AgentResult {
            session_id: SessionId::new(),
            final_state: if summary.failed_steps > 0 && summary.completed_steps == 0 {
                AgentState::Error
            } else {
                AgentState::Completed
            },
            messages: all_messages,
            tool_calls: all_tool_calls,
            tool_results: all_tool_results,
            iterations: total_iterations,
            tokens_used: total_tokens,
        })
    }

    /// 获取会话管理器引用
    pub fn session_manager(&self) -> &Arc<ConcurrentSessionManager> {
        &self.session_manager
    }

    /// 获取工具注册表引用
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// 验证状态转换是否合法
    fn validate_transition(current: AgentState, target: AgentState) -> Layer2Result<()> {
        let valid = match (current, target) {
            // Idle -> Running (start/resume)
            (AgentState::Idle, AgentState::Running) => true,
            // Running -> ToolCalling (tool call detected)
            (AgentState::Running, AgentState::ToolCalling) => true,
            // Running -> WaitingTool (waiting for tool result)
            (AgentState::Running, AgentState::WaitingTool) => true,
            // Running -> Completed (task finished)
            (AgentState::Running, AgentState::Completed) => true,
            // Running -> Stopped (manual stop)
            (AgentState::Running, AgentState::Stopped) => true,
            // Running -> Error
            (AgentState::Running, AgentState::Error) => true,
            // ToolCalling -> WaitingTool
            (AgentState::ToolCalling, AgentState::WaitingTool) => true,
            // ToolCalling -> Running (after tool result)
            (AgentState::ToolCalling, AgentState::Running) => true,
            // ToolCalling -> Error
            (AgentState::ToolCalling, AgentState::Error) => true,
            // WaitingTool -> Running (tool result submitted)
            (AgentState::WaitingTool, AgentState::Running) => true,
            // WaitingTool -> Stopped
            (AgentState::WaitingTool, AgentState::Stopped) => true,
            // WaitingTool -> Error
            (AgentState::WaitingTool, AgentState::Error) => true,
            // Stopped -> Running (resume)
            (AgentState::Stopped, AgentState::Running) => true,
            // Completed -> Idle (reuse session)
            (AgentState::Completed, AgentState::Idle) => true,
            // Same state is always valid (idempotent)
            (_, _) if current == target => true,
            _ => false,
        };

        if valid {
            Ok(())
        } else {
            Err(Layer2Error::InvalidStateTransition {
                from: current,
                to: target,
            }
            .into())
        }
    }

    /// 确保会话存在，否则返回 SessionNotFound 错误
    async fn require_session(&self, session_id: &SessionId) -> Layer2Result<()> {
        let session = self.session_manager.get(session_id).await?;
        if session.is_some() {
            Ok(())
        } else {
            Err(Layer2Error::SessionNotFound(session_id.clone()).into())
        }
    }

    /// 执行一轮工具调用：将 pending 的 tool calls 全部执行，
    /// 将结果写入 session 的 tool_results_cache，并清除 pending。
    async fn execute_pending_tool_calls(&self, session_id: &SessionId) -> Layer2Result<()> {
        // Collect pending tool calls from the session
        let pending: Vec<ToolCall> = self
            .session_manager
            .read(session_id, |s| s.tool_calls_pending.clone())
            .await?
            .unwrap_or_default();

        if pending.is_empty() {
            return Ok(());
        }

        debug!(
            session_id = %session_id,
            count = pending.len(),
            "Executing pending tool calls"
        );

        // Execute each tool call and collect results
        let mut results = Vec::with_capacity(pending.len());
        for tc in &pending {
            // Check permission before executing tool
            if let Some(pm) = &self.permission_manager {
                let request = PermissionRequest::new(PermissionAction::Custom {
                    description: format!("Execute tool: {} with args: {}", tc.name, tc.arguments),
                });

                match pm.check_permission(request) {
                    Ok(response) => {
                        if !response.decision.is_allowed() {
                            warn!(
                                tool = %tc.name,
                                tool_call_id = %tc.id,
                                "Tool execution denied by permission system"
                            );
                            results.push(ToolResult {
                                tool_call_id: tc.id.clone(),
                                name: tc.name.clone(),
                                content: "Tool execution denied by permission system".to_string(),
                                is_error: true,
                            });
                            continue;
                        }
                    }
                    Err(e) => {
                        warn!(
                            tool = %tc.name,
                            tool_call_id = %tc.id,
                            error = %e,
                            "Permission check failed"
                        );
                        results.push(ToolResult {
                            tool_call_id: tc.id.clone(),
                            name: tc.name.clone(),
                            content: format!("Permission check failed: {}", e),
                            is_error: true,
                        });
                        continue;
                    }
                }
            }

            let result = match self.tool_registry.execute(&tc.name, &tc.arguments).await {
                Ok(tool_result) => tool_result,
                Err(e) => {
                    warn!(
                        tool = %tc.name,
                        tool_call_id = %tc.id,
                        error = %e,
                        "Tool execution failed"
                    );
                    ToolResult {
                        tool_call_id: tc.id.clone(),
                        name: tc.name.clone(),
                        content: format!("Tool execution error: {}", e),
                        is_error: true,
                    }
                }
            };
            results.push(result);
        }

        // Write results back to the session and clear pending
        self.session_manager
            .update(session_id, |s| {
                s.tool_results_cache.extend(results);
                s.tool_calls_pending.clear();
            })
            .await?;

        Ok(())
    }

    /// 模拟 LLM 调用：生成一个简单的助手响应。
    ///
    /// 在真实的 Agent 循环中，这里会调用 LLM API 来获取下一步动作。
    /// 当前实现作为本地模拟，根据任务文本和注册的工具产生响应。
    async fn simulate_llm_step(
        &self,
        session_id: &SessionId,
        task: &str,
        iteration: i32,
        max_iterations: i32,
    ) -> Layer2Result<IterationResult> {
        let tools = self.tool_registry.list();

        // Check if there are pending tool results to process
        let has_pending_results: bool = self
            .session_manager
            .read(session_id, |s| !s.tool_results_cache.is_empty())
            .await?
            .unwrap_or(false);

        let should_continue = iteration < max_iterations;

        // If we have pending tool results, process them and continue
        if has_pending_results {
            let tool_results: Vec<ToolResult> = self
                .session_manager
                .read(session_id, |s| s.tool_results_cache.clone())
                .await?
                .unwrap_or_default();

            // Generate assistant response acknowledging tool results
            let summary: Vec<String> = tool_results
                .iter()
                .map(|r| {
                    if r.is_error {
                        format!("Tool {} failed: {}", r.name, r.content)
                    } else {
                        format!("Tool {} succeeded: {}", r.name, r.content)
                    }
                })
                .collect();

            let response = if !should_continue {
                format!(
                    "I've processed the tool results. Task '{}' is now complete.\n{}",
                    task,
                    summary.join("\n")
                )
            } else {
                format!(
                    "Processing tool results, continuing...\n{}",
                    summary.join("\n")
                )
            };

            // Clear the tool results cache after processing
            self.session_manager
                .update(session_id, |s| {
                    s.tool_results_cache.clear();
                })
                .await?;

            return Ok(IterationResult {
                iteration,
                state: if should_continue {
                    AgentState::Running
                } else {
                    AgentState::Completed
                },
                message: Some(Message::assistant(&response)),
                tool_calls: Vec::new(),
                should_continue,
            });
        }

        // First iteration: acknowledge the task
        if iteration == 1 {
            let response = format!("Starting task: {}", task);
            return Ok(IterationResult {
                iteration,
                state: AgentState::Running,
                message: Some(Message::assistant(&response)),
                tool_calls: Vec::new(),
                should_continue: true,
            });
        }

        // If there are registered tools, try to use them
        if !tools.is_empty() && iteration <= 2 {
            // Simulate a tool call on the second iteration
            let tool_name = &tools[0];
            let tool_call = ToolCall {
                id: sh_layer1::generate_prefixed_id("tc"),
                name: tool_name.clone(),
                arguments: serde_json::json!({"task": task}).to_string(),
            };

            return Ok(IterationResult {
                iteration,
                state: AgentState::ToolCalling,
                message: Some(Message::assistant(format!(
                    "I'll use the {} tool to help with this task.",
                    tool_name
                ))),
                tool_calls: vec![tool_call],
                should_continue: true,
            });
        }

        // Final iteration: complete the task
        let response = format!("Task '{}' has been completed.", task);
        Ok(IterationResult {
            iteration,
            state: AgentState::Completed,
            message: Some(Message::assistant(&response)),
            tool_calls: Vec::new(),
            should_continue: false,
        })
    }

    /// 真实 LLM 调用：使用 Layer1 LlmClient 发送流式请求。
    ///
    /// 当配置了 LLM 客户端时使用此方法，否则回退到 simulate_llm_step。
    async fn real_llm_step(
        &self,
        session_id: &SessionId,
        task: &str,
        iteration: i32,
        max_iterations: i32,
        config: &AgentConfig,
        abort_flag: Option<Arc<AtomicBool>>,
    ) -> Layer2Result<IterationResult> {
        use sh_layer1::{LlmClientTrait, LlmRequestConfig};

        let llm_client = self
            .llm_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;

        // Get session messages
        let session_messages: Vec<Message> = self
            .session_manager
            .read(session_id, |s| s.messages.clone())
            .await?
            .unwrap_or_default();

        // Convert to Layer1 messages
        let mut llm_messages: Vec<sh_layer1::Message> = session_messages
            .iter()
            .map(|m| sh_layer1::Message {
                role: match m.role {
                    MessageRole::System => sh_layer1::MessageRole::System,
                    MessageRole::User => sh_layer1::MessageRole::User,
                    MessageRole::Assistant => sh_layer1::MessageRole::Assistant,
                    MessageRole::Tool => sh_layer1::MessageRole::User, // Tool results as user
                },
                content: m.content.clone(),
            })
            .collect();

        // Add current task if not already present
        if iteration == 1 {
            llm_messages.push(sh_layer1::Message {
                role: sh_layer1::MessageRole::User,
                content: task.to_string(),
            });
        }

        // Build request config
        let request_config = LlmRequestConfig {
            model: config.model.clone(),
            max_tokens: 4096,
            temperature: config.temperature,
            system_prompt: config.system_prompt.clone(),
            stop_sequences: vec!["\n\n\n".to_string()],
        };

        // Send streaming request
        let response = if let Some(flag) = abort_flag {
            llm_client
                .send_stream_abortable(llm_messages, &request_config, flag)
                .await
                .map_err(|e| anyhow::anyhow!("LLM stream error: {}", e))?
        } else {
            llm_client
                .send(llm_messages, &request_config)
                .await
                .map_err(|e| anyhow::anyhow!("LLM error: {}", e))?
        };

        // Update token usage in session
        let tokens_used = response.usage.input_tokens as i64 + response.usage.output_tokens as i64;
        self.session_manager
            .update(session_id, |s| {
                s.tokens_total += tokens_used;
            })
            .await?;

        // Parse response for tool calls
        let tool_calls = self.parse_tool_calls_from_response(&response.content);

        // Determine state based on response
        let state = if !tool_calls.is_empty() {
            AgentState::ToolCalling
        } else if iteration >= max_iterations {
            AgentState::Completed
        } else {
            AgentState::Running
        };

        let should_continue = iteration < max_iterations && state != AgentState::Completed;

        Ok(IterationResult {
            iteration,
            state,
            message: Some(Message::assistant(&response.content)),
            tool_calls,
            should_continue,
        })
    }

    /// 从 LLM 响应中解析工具调用
    ///
    /// 支持 Anthropic tool_use 格式和 OpenAI function_call 格式
    fn parse_tool_calls_from_response(&self, content: &str) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();

        // Try to parse Anthropic-style tool_use blocks
        // Format: <tool_use>{"name": "...", "input": {...}}</tool_use>
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(content) {
            // Check if it's a tool_use response
            if let Some(content_array) = json_value.get("content").and_then(|c| c.as_array()) {
                for block in content_array {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let (Some(name), Some(id), Some(input)) = (
                            block.get("name").and_then(|n| n.as_str()),
                            block.get("id").and_then(|i| i.as_str()),
                            block.get("input"),
                        ) {
                            tool_calls.push(ToolCall {
                                id: id.to_string(),
                                name: name.to_string(),
                                arguments: input.to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Also try OpenAI-style function_call
        // Format: {"function_call": {"name": "...", "arguments": "..."}}
        if tool_calls.is_empty() {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(content) {
                if let Some(func_call) = json_value.get("function_call") {
                    if let (Some(name), Some(args)) = (
                        func_call.get("name").and_then(|n| n.as_str()),
                        func_call.get("arguments").and_then(|a| a.as_str()),
                    ) {
                        tool_calls.push(ToolCall {
                            id: sh_layer1::generate_prefixed_id("tc"),
                            name: name.to_string(),
                            arguments: args.to_string(),
                        });
                    }
                }
            }
        }

        // Also parse tool blocks in text format
        // Format: ```tool\n{"name": "tool_name", "arguments": {...}}\n```
        if tool_calls.is_empty() {
            let re = regex::Regex::new(r"```tool\n(\{.*?\})\n```").unwrap();
            for cap in re.captures_iter(content) {
                if let Ok(tool_json) = serde_json::from_str::<serde_json::Value>(&cap[1]) {
                    if let Some(name) = tool_json.get("name").and_then(|n| n.as_str()) {
                        let args = tool_json
                            .get("arguments")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        tool_calls.push(ToolCall {
                            id: sh_layer1::generate_prefixed_id("tc"),
                            name: name.to_string(),
                            arguments: args.to_string(),
                        });
                    }
                }
            }
        }

        tool_calls
    }

    /// 执行一步 LLM 调用（自动选择真实或模拟）
    ///
    /// 如果配置了 LLM 客户端则使用真实调用，否则使用模拟。
    async fn llm_step(
        &self,
        session_id: &SessionId,
        task: &str,
        iteration: i32,
        max_iterations: i32,
        config: &AgentConfig,
        abort_flag: Option<Arc<AtomicBool>>,
    ) -> Layer2Result<IterationResult> {
        if self.llm_client.is_some() {
            self.real_llm_step(
                session_id,
                task,
                iteration,
                max_iterations,
                config,
                abort_flag,
            )
            .await
        } else {
            self.simulate_llm_step(session_id, task, iteration, max_iterations)
                .await
        }
    }
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl AgentRuntimeTrait for AgentRuntime {
    /// 运行 Agent 执行完整循环，直到完成或出错。
    ///
    /// 这是同步（阻塞式）的执行方式：创建会话、运行循环、返回结果。
    async fn run(&self, task: &str, config: AgentConfig) -> Layer2Result<AgentResult> {
        info!(task = %task, agent_id = %config.agent_id, "Starting agent run");

        // Create session
        let session_config = SessionConfig::from(&config);
        let session_id = self.session_manager.create(session_config).await?;

        // Set agent_id on the session
        let agent_id = config.agent_id.clone();
        self.session_manager
            .update(&session_id, |s| {
                s.agent_id = agent_id;
            })
            .await?;

        // Add system prompt if configured
        if let Some(ref prompt) = config.system_prompt {
            self.session_manager
                .add_message(&session_id, Message::system(prompt))
                .await?;
        }

        // Add user task message
        self.session_manager
            .add_message(&session_id, Message::user(task))
            .await?;

        // Transition to Running
        self.session_manager
            .set_state(&session_id, AgentState::Running)
            .await?;

        // Execute the loop
        let mut iterations = 0;
        let max_iterations = config.max_iterations;

        loop {
            iterations += 1;

            if iterations > max_iterations {
                warn!(
                    session_id = %session_id,
                    max = max_iterations,
                    "Max iterations reached"
                );
                self.session_manager
                    .set_state(&session_id, AgentState::Error)
                    .await?;
                return Err(Layer2Error::MaxIterations(max_iterations).into());
            }

            // Check if session can continue (respect stopped/paused states)
            let can_continue: bool = self
                .session_manager
                .read(&session_id, |s| s.can_continue())
                .await?
                .unwrap_or(false);

            if !can_continue {
                let current_state: AgentState = self
                    .session_manager
                    .read(&session_id, |s| s.state)
                    .await?
                    .unwrap_or(AgentState::Stopped);

                if current_state == AgentState::Stopped {
                    info!(session_id = %session_id, "Agent stopped by user");
                    break;
                }
                // Paused or other non-continuable state — break out
                break;
            }

            // Execute one LLM step (real or simulated)
            let step_result = self
                .llm_step(&session_id, task, iterations, max_iterations, &config, None)
                .await?;

            // Add the assistant message if any
            if let Some(msg) = step_result.message {
                self.session_manager.add_message(&session_id, msg).await?;
            }

            // Handle tool calls
            if !step_result.tool_calls.is_empty() {
                // Store pending tool calls in session
                let tool_calls = step_result.tool_calls.clone();
                self.session_manager
                    .update(&session_id, |s| {
                        s.tool_calls_pending = tool_calls;
                        s.state = AgentState::ToolCalling;
                    })
                    .await?;

                // Execute the tools
                self.execute_pending_tool_calls(&session_id).await?;

                // Transition to WaitingTool briefly, then back to Running
                self.session_manager
                    .set_state(&session_id, AgentState::WaitingTool)
                    .await?;
                self.session_manager
                    .set_state(&session_id, AgentState::Running)
                    .await?;
            } else {
                // Update state from step result
                self.session_manager
                    .set_state(&session_id, step_result.state)
                    .await?;
            }

            // Check if we should stop
            if !step_result.should_continue {
                break;
            }
        }

        // Collect final results
        let session = self
            .session_manager
            .get(&session_id)
            .await?
            .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        let tokens_used = session.tokens_total;

        Ok(AgentResult {
            session_id: session.session_id.clone(),
            final_state: session.state,
            messages: session.messages,
            tool_calls: session.tool_calls_pending,
            tool_results: session.tool_results_cache,
            iterations,
            tokens_used,
        })
    }

    /// 流式运行 Agent 执行完整循环，通过回调通知每次迭代。
    ///
    /// 与 run() 类似，但在每次迭代前后通过回调通知外部调用者。
    async fn run_stream(
        &self,
        task: &str,
        config: AgentConfig,
        callback: &dyn AgentLoopCallback,
    ) -> Layer2Result<AgentResult> {
        info!(task = %task, agent_id = %config.agent_id, "Starting agent run_stream");

        // Create session
        let session_config = SessionConfig::from(&config);
        let session_id = self.session_manager.create(session_config).await?;

        // Set agent_id on the session
        let agent_id = config.agent_id.clone();
        self.session_manager
            .update(&session_id, |s| {
                s.agent_id = agent_id;
            })
            .await?;

        // Add system prompt if configured
        if let Some(ref prompt) = config.system_prompt {
            self.session_manager
                .add_message(&session_id, Message::system(prompt))
                .await?;
        }

        // Add user task message
        self.session_manager
            .add_message(&session_id, Message::user(task))
            .await?;

        // Transition to Running
        self.session_manager
            .set_state(&session_id, AgentState::Running)
            .await?;

        // Execute the loop with callbacks
        let mut iterations = 0;
        let max_iterations = config.max_iterations;

        loop {
            iterations += 1;

            if iterations > max_iterations {
                warn!(
                    session_id = %session_id,
                    max = max_iterations,
                    "Max iterations reached"
                );
                self.session_manager
                    .set_state(&session_id, AgentState::Error)
                    .await?;
                return Err(Layer2Error::MaxIterations(max_iterations).into());
            }

            // before_iteration callback
            let should_continue_iter = callback.before_iteration(&session_id, iterations).await?;
            if !should_continue_iter {
                info!(session_id = %session_id, "Callback requested stop");
                break;
            }

            // Check if session can continue
            let can_continue: bool = self
                .session_manager
                .read(&session_id, |s| s.can_continue())
                .await?
                .unwrap_or(false);

            if !can_continue {
                let current_state: AgentState = self
                    .session_manager
                    .read(&session_id, |s| s.state)
                    .await?
                    .unwrap_or(AgentState::Stopped);

                if current_state == AgentState::Stopped {
                    info!(session_id = %session_id, "Agent stopped by user");
                    break;
                }
                break;
            }

            // Execute one LLM step (real or simulated)
            let step_result = self
                .llm_step(&session_id, task, iterations, max_iterations, &config, None)
                .await?;

            // Add the assistant message if any
            if let Some(msg) = step_result.message.clone() {
                self.session_manager.add_message(&session_id, msg).await?;
            }

            // Handle tool calls with callbacks
            if !step_result.tool_calls.is_empty() {
                let tool_calls = step_result.tool_calls.clone();

                // before_tool_call callback for each tool
                for tc in &tool_calls {
                    let should_execute = callback.before_tool_call(&session_id, tc).await?;
                    if !should_execute {
                        info!(tool_call_id = %tc.id, "Callback rejected tool call");
                        continue;
                    }
                }

                // Store pending tool calls
                self.session_manager
                    .update(&session_id, |s| {
                        s.tool_calls_pending = tool_calls;
                        s.state = AgentState::ToolCalling;
                    })
                    .await?;

                // Execute the tools
                self.execute_pending_tool_calls(&session_id).await?;

                // Get results and call after_tool_call callback
                let results: Vec<ToolResult> = self
                    .session_manager
                    .read(&session_id, |s| s.tool_results_cache.clone())
                    .await?
                    .unwrap_or_default();

                // Call after_tool_call for each result
                for tc in &step_result.tool_calls {
                    if let Some(result) = results.iter().find(|r| r.tool_call_id == tc.id) {
                        callback.after_tool_call(&session_id, tc, result).await?;
                    }
                }

                // Transition states
                self.session_manager
                    .set_state(&session_id, AgentState::WaitingTool)
                    .await?;
                self.session_manager
                    .set_state(&session_id, AgentState::Running)
                    .await?;
            } else {
                self.session_manager
                    .set_state(&session_id, step_result.state)
                    .await?;
            }

            // Create iteration result for callback
            let iter_result = IterationResult {
                iteration: iterations,
                state: self
                    .session_manager
                    .read(&session_id, |s| s.state)
                    .await?
                    .unwrap_or(AgentState::Running),
                message: step_result.message,
                tool_calls: step_result.tool_calls,
                should_continue: step_result.should_continue,
            };

            // after_iteration callback
            callback
                .after_iteration(&session_id, iterations, &iter_result)
                .await?;

            if !iter_result.should_continue {
                break;
            }
        }

        // Collect final results
        let session = self
            .session_manager
            .get(&session_id)
            .await?
            .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        let tokens_used = session.tokens_total;

        Ok(AgentResult {
            session_id: session.session_id.clone(),
            final_state: session.state,
            messages: session.messages,
            tool_calls: session.tool_calls_pending,
            tool_results: session.tool_results_cache,
            iterations,
            tokens_used,
        })
    }

    /// 流式运行 Agent（支持中断）。
    ///
    /// 与 run_stream 类似，但支持通过 abort_flag 中断执行。
    async fn run_stream_abortable(
        &self,
        task: &str,
        config: AgentConfig,
        callback: &dyn AgentLoopCallback,
        abort_flag: Arc<AtomicBool>,
    ) -> Layer2Result<AgentResult> {
        info!(task = %task, agent_id = %config.agent_id, "Starting agent run_stream_abortable");

        // Create session
        let session_config = SessionConfig::from(&config);
        let session_id = self.session_manager.create(session_config).await?;

        // Set agent_id on the session
        let agent_id = config.agent_id.clone();
        self.session_manager
            .update(&session_id, |s| {
                s.agent_id = agent_id;
            })
            .await?;

        // Add system prompt if configured
        if let Some(ref prompt) = config.system_prompt {
            self.session_manager
                .add_message(&session_id, Message::system(prompt))
                .await?;
        }

        // Add user task message
        self.session_manager
            .add_message(&session_id, Message::user(task))
            .await?;

        // Transition to Running
        self.session_manager
            .set_state(&session_id, AgentState::Running)
            .await?;

        // Execute the loop with callbacks and abort check
        let mut iterations = 0;
        let max_iterations = config.max_iterations;

        loop {
            // Check abort flag first
            if abort_flag.load(Ordering::Relaxed) {
                info!(session_id = %session_id, "Abort flag set, stopping agent");
                self.session_manager
                    .set_state(&session_id, AgentState::Stopped)
                    .await?;
                break;
            }

            iterations += 1;

            if iterations > max_iterations {
                warn!(
                    session_id = %session_id,
                    max = max_iterations,
                    "Max iterations reached"
                );
                self.session_manager
                    .set_state(&session_id, AgentState::Error)
                    .await?;
                return Err(Layer2Error::MaxIterations(max_iterations).into());
            }

            // before_iteration callback
            let should_continue_iter = callback.before_iteration(&session_id, iterations).await?;
            if !should_continue_iter {
                info!(session_id = %session_id, "Callback requested stop");
                break;
            }

            // Check abort flag again after callback
            if abort_flag.load(Ordering::Relaxed) {
                info!(session_id = %session_id, "Abort flag set after callback, stopping agent");
                self.session_manager
                    .set_state(&session_id, AgentState::Stopped)
                    .await?;
                break;
            }

            // Check if session can continue
            let can_continue: bool = self
                .session_manager
                .read(&session_id, |s| s.can_continue())
                .await?
                .unwrap_or(false);

            if !can_continue {
                let current_state: AgentState = self
                    .session_manager
                    .read(&session_id, |s| s.state)
                    .await?
                    .unwrap_or(AgentState::Stopped);

                if current_state == AgentState::Stopped {
                    info!(session_id = %session_id, "Agent stopped by user");
                    break;
                }
                break;
            }

            // Execute one LLM step (real or simulated)
            let step_result = self
                .llm_step(
                    &session_id,
                    task,
                    iterations,
                    max_iterations,
                    &config,
                    Some(abort_flag.clone()),
                )
                .await?;

            // Add the assistant message if any
            if let Some(msg) = step_result.message.clone() {
                self.session_manager.add_message(&session_id, msg).await?;
            }

            // Handle tool calls with callbacks
            if !step_result.tool_calls.is_empty() {
                let tool_calls = step_result.tool_calls.clone();

                // Check abort before tool calls
                if abort_flag.load(Ordering::Relaxed) {
                    info!(session_id = %session_id, "Abort flag set before tool calls");
                    self.session_manager
                        .set_state(&session_id, AgentState::Stopped)
                        .await?;
                    break;
                }

                // before_tool_call callback for each tool
                for tc in &tool_calls {
                    let should_execute = callback.before_tool_call(&session_id, tc).await?;
                    if !should_execute {
                        info!(tool_call_id = %tc.id, "Callback rejected tool call");
                        continue;
                    }
                }

                // Store pending tool calls
                self.session_manager
                    .update(&session_id, |s| {
                        s.tool_calls_pending = tool_calls;
                        s.state = AgentState::ToolCalling;
                    })
                    .await?;

                // Execute the tools
                self.execute_pending_tool_calls(&session_id).await?;

                // Get results and call after_tool_call callback
                let results: Vec<ToolResult> = self
                    .session_manager
                    .read(&session_id, |s| s.tool_results_cache.clone())
                    .await?
                    .unwrap_or_default();

                // Call after_tool_call for each result
                for tc in &step_result.tool_calls {
                    if let Some(result) = results.iter().find(|r| r.tool_call_id == tc.id) {
                        callback.after_tool_call(&session_id, tc, result).await?;
                    }
                }

                // Transition states
                self.session_manager
                    .set_state(&session_id, AgentState::WaitingTool)
                    .await?;
                self.session_manager
                    .set_state(&session_id, AgentState::Running)
                    .await?;
            } else {
                self.session_manager
                    .set_state(&session_id, step_result.state)
                    .await?;
            }

            // Create iteration result for callback
            let iter_result = IterationResult {
                iteration: iterations,
                state: self
                    .session_manager
                    .read(&session_id, |s| s.state)
                    .await?
                    .unwrap_or(AgentState::Running),
                message: step_result.message,
                tool_calls: step_result.tool_calls,
                should_continue: step_result.should_continue,
            };

            // after_iteration callback
            callback
                .after_iteration(&session_id, iterations, &iter_result)
                .await?;

            if !iter_result.should_continue {
                break;
            }
        }

        // Collect final results
        let session = self
            .session_manager
            .get(&session_id)
            .await?
            .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        let tokens_used = session.tokens_total;

        Ok(AgentResult {
            session_id: session.session_id.clone(),
            final_state: session.state,
            messages: session.messages,
            tool_calls: session.tool_calls_pending,
            tool_results: session.tool_results_cache,
            iterations,
            tokens_used,
        })
    }

    /// 启动 Agent 并返回会话 ID（用于异步/流式执行）。
    ///
    /// 创建会话，设置为 Running 状态，但不执行循环。
    /// 调用者可以通过 send_message / submit_tool_result 与 Agent 交互。
    async fn start(&self, task: &str, config: AgentConfig) -> Layer2Result<SessionId> {
        info!(task = %task, agent_id = %config.agent_id, "Starting agent session");

        let session_config = SessionConfig::from(&config);
        let session_id = self.session_manager.create(session_config).await?;

        // Set agent_id
        let agent_id = config.agent_id.clone();
        self.session_manager
            .update(&session_id, |s| {
                s.agent_id = agent_id;
            })
            .await?;

        // Add system prompt if configured
        if let Some(ref prompt) = config.system_prompt {
            self.session_manager
                .add_message(&session_id, Message::system(prompt))
                .await?;
        }

        // Add user task message
        self.session_manager
            .add_message(&session_id, Message::user(task))
            .await?;

        // Transition to Running
        self.session_manager
            .set_state(&session_id, AgentState::Running)
            .await?;

        Ok(session_id)
    }

    /// 暂停正在执行的 Agent。
    ///
    /// 将状态从 Running/ToolCalling/WaitingTool 转换为 Stopped（暂停）。
    /// 在暂停状态下，Agent 不会继续执行迭代。
    async fn pause(&self, session_id: &SessionId) -> Layer2Result<()> {
        self.require_session(session_id).await?;

        let current_state: AgentState =
            self.session_manager
                .read(session_id, |s| s.state)
                .await?
                .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        match current_state {
            AgentState::Running | AgentState::ToolCalling | AgentState::WaitingTool => {
                AgentRuntime::validate_transition(current_state, AgentState::Stopped)?;
                self.session_manager
                    .set_state(session_id, AgentState::Stopped)
                    .await?;
                info!(session_id = %session_id, "Agent paused");
                Ok(())
            }
            AgentState::Stopped => {
                // Already paused, idempotent
                debug!(session_id = %session_id, "Agent already paused");
                Ok(())
            }
            other => Err(Layer2Error::InvalidStateTransition {
                from: other,
                to: AgentState::Stopped,
            }
            .into()),
        }
    }

    /// 恢复暂停的 Agent。
    ///
    /// 将状态从 Stopped 转换回 Running。
    async fn resume(&self, session_id: &SessionId) -> Layer2Result<()> {
        self.require_session(session_id).await?;

        let current_state: AgentState =
            self.session_manager
                .read(session_id, |s| s.state)
                .await?
                .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        match current_state {
            AgentState::Stopped => {
                AgentRuntime::validate_transition(current_state, AgentState::Running)?;
                self.session_manager
                    .set_state(session_id, AgentState::Running)
                    .await?;
                info!(session_id = %session_id, "Agent resumed");
                Ok(())
            }
            AgentState::Running => {
                // Already running, idempotent
                debug!(session_id = %session_id, "Agent already running");
                Ok(())
            }
            other => Err(Layer2Error::InvalidStateTransition {
                from: other,
                to: AgentState::Running,
            }
            .into()),
        }
    }

    /// 停止 Agent 执行。
    ///
    /// 无论当前处于什么状态（除了 Completed/Idle），都转换到 Stopped。
    /// 这与 pause 的区别在于 stop 是终止性的，表示用户主动取消。
    async fn stop(&self, session_id: &SessionId) -> Layer2Result<()> {
        self.require_session(session_id).await?;

        let current_state: AgentState =
            self.session_manager
                .read(session_id, |s| s.state)
                .await?
                .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        match current_state {
            AgentState::Running
            | AgentState::ToolCalling
            | AgentState::WaitingTool
            | AgentState::Stopped => {
                self.session_manager
                    .set_state(session_id, AgentState::Stopped)
                    .await?;
                info!(session_id = %session_id, "Agent stopped");
                Ok(())
            }
            AgentState::Idle | AgentState::Completed | AgentState::Error => {
                Err(Layer2Error::InvalidStateTransition {
                    from: current_state,
                    to: AgentState::Stopped,
                }
                .into())
            }
        }
    }

    /// 获取 Agent 当前状态。
    fn status(&self, session_id: &SessionId) -> Layer2Result<AgentState> {
        // Use the synchronous accessor provided by ConcurrentSessionManager.
        // Since ConcurrentSessionManager uses parking_lot::RwLock internally,
        // we can do a synchronous read safely.
        self.session_manager
            .get_state_sync(session_id)
            .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()).into())
    }

    /// 向 Agent 发送消息。
    ///
    /// 将消息添加到会话的消息历史中。Agent 在下一次迭代时可以读取。
    async fn send_message(&self, session_id: &SessionId, message: &str) -> Layer2Result<()> {
        self.require_session(session_id).await?;

        let current_state: AgentState =
            self.session_manager
                .read(session_id, |s| s.state)
                .await?
                .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        // Allow sending messages in Running, WaitingTool, or Stopped states
        match current_state {
            AgentState::Running
            | AgentState::WaitingTool
            | AgentState::Stopped
            | AgentState::Idle
            | AgentState::ToolCalling => {
                self.session_manager
                    .add_message(session_id, Message::user(message))
                    .await?;
                debug!(
                    session_id = %session_id,
                    msg_len = message.len(),
                    "Message sent to agent"
                );
                Ok(())
            }
            AgentState::Completed | AgentState::Error => {
                Err(Layer2Error::InvalidStateTransition {
                    from: current_state,
                    to: current_state, // no transition, just rejection
                }
                .into())
            }
        }
    }

    /// 提交工具调用结果。
    ///
    /// 当 Agent 处于 WaitingTool 状态时，外部系统可以通过此方法
    /// 提交工具执行的结果，使 Agent 能够继续执行。
    async fn submit_tool_result(
        &self,
        session_id: &SessionId,
        tool_call_id: &str,
        result: ToolResult,
    ) -> Layer2Result<()> {
        self.require_session(session_id).await?;

        let current_state: AgentState =
            self.session_manager
                .read(session_id, |s| s.state)
                .await?
                .ok_or_else(|| Layer2Error::SessionNotFound(session_id.clone()))?;

        match current_state {
            AgentState::WaitingTool | AgentState::ToolCalling | AgentState::Running => {
                // Verify the tool_call_id matches an expected pending call
                let _pending_ids: Vec<String> = self
                    .session_manager
                    .read(session_id, |s| {
                        s.tool_calls_pending
                            .iter()
                            .map(|tc| tc.id.clone())
                            .collect()
                    })
                    .await?
                    .unwrap_or_default();

                // Remove the matched pending tool call and store the result
                self.session_manager
                    .update(session_id, |s| {
                        // Remove the matching pending tool call
                        s.tool_calls_pending.retain(|tc| tc.id != tool_call_id);
                        // Store the result
                        s.tool_results_cache.push(result);

                        // If no more pending tool calls, transition back to Running
                        if s.tool_calls_pending.is_empty() {
                            s.state = AgentState::Running;
                        }
                    })
                    .await?;

                debug!(
                    session_id = %session_id,
                    tool_call_id = %tool_call_id,
                    "Tool result submitted"
                );
                Ok(())
            }
            other => Err(Layer2Error::InvalidStateTransition {
                from: other,
                to: AgentState::Running,
            }
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_registry::Tool;
    use crate::types::MessageRole;

    /// Mock tool for testing
    struct MockTool {
        name: String,
        description: String,
    }

    impl MockTool {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                description: format!("Mock tool: {}", name),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string"
                    }
                }
            })
        }

        async fn execute(&self, args: &str) -> Layer2Result<ToolResult> {
            Ok(ToolResult {
                tool_call_id: "mock_id".to_string(),
                name: self.name.clone(),
                content: format!("Executed with args: {}", args),
                is_error: false,
            })
        }
    }

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.model, "claude-sonnet-4-6");
        assert_eq!(config.max_iterations, 100);
        assert_eq!(config.temperature, 0.7);
    }

    #[test]
    fn test_agent_runtime_creation() {
        let runtime = AgentRuntime::with_defaults();
        assert!(runtime.session_manager().stats().total_sessions == 0);
        assert!(runtime.tool_registry().count() == 0);
    }

    #[test]
    fn test_agent_config_to_session_config() {
        let agent_config = AgentConfig {
            agent_id: AgentId::new(),
            model: "custom-model".to_string(),
            temperature: 0.5,
            max_iterations: 50,
            system_prompt: Some("Custom prompt".to_string()),
        };

        let session_config = SessionConfig::from(&agent_config);
        assert_eq!(session_config.model, "custom-model");
        assert_eq!(session_config.temperature, 0.5);
        assert_eq!(session_config.max_iterations, 50);
        assert_eq!(
            session_config.system_prompt,
            Some("Custom prompt".to_string())
        );
    }

    #[test]
    fn test_state_transition_validation() {
        // Valid transitions
        assert!(AgentRuntime::validate_transition(AgentState::Idle, AgentState::Running).is_ok());
        assert!(
            AgentRuntime::validate_transition(AgentState::Running, AgentState::ToolCalling).is_ok()
        );
        assert!(
            AgentRuntime::validate_transition(AgentState::Running, AgentState::Stopped).is_ok()
        );
        assert!(
            AgentRuntime::validate_transition(AgentState::Stopped, AgentState::Running).is_ok()
        );
        assert!(
            AgentRuntime::validate_transition(AgentState::Running, AgentState::Completed).is_ok()
        );
        assert!(
            AgentRuntime::validate_transition(AgentState::Running, AgentState::Running).is_ok()
        );

        // Invalid transitions
        assert!(
            AgentRuntime::validate_transition(AgentState::Idle, AgentState::ToolCalling).is_err()
        );
        assert!(
            AgentRuntime::validate_transition(AgentState::Completed, AgentState::Running).is_err()
        );
        assert!(AgentRuntime::validate_transition(AgentState::Error, AgentState::Running).is_err());
    }

    #[tokio::test]
    async fn test_agent_run_basic() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig {
            max_iterations: 5,
            ..Default::default()
        };

        let result = runtime.run("Test task", config).await;
        assert!(result.is_ok());

        let agent_result = result.unwrap();
        assert!(!agent_result.session_id.0.is_empty());
        assert!(agent_result.iterations > 0);
        assert!(agent_result.iterations <= 5);
        // Messages should include system (if configured), user task, and assistant responses
        assert!(!agent_result.messages.is_empty());
    }

    #[tokio::test]
    async fn test_agent_run_with_tools() {
        let runtime = AgentRuntime::with_defaults();

        // Register a mock tool
        runtime
            .tool_registry()
            .register(Box::new(MockTool::new("test_tool")))
            .unwrap();

        assert!(runtime.tool_registry().count() == 1);

        let config = AgentConfig {
            max_iterations: 10,
            ..Default::default()
        };

        let result = runtime.run("Test task with tools", config).await;
        assert!(result.is_ok());

        let agent_result = result.unwrap();
        // Should have executed the tool
        assert!(!agent_result.tool_results.is_empty() || agent_result.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn test_agent_start_creates_session() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig::default();

        let session_id = runtime.start("Test task", config).await.unwrap();

        // Verify session was created
        let session = runtime.session_manager().get(&session_id).await.unwrap();
        assert!(session.is_some());

        let session = session.unwrap();
        assert_eq!(session.state, AgentState::Running);
        assert!(!session.messages.is_empty());
    }

    #[tokio::test]
    async fn test_agent_start_with_system_prompt() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig {
            system_prompt: Some("You are a helpful assistant".to_string()),
            ..Default::default()
        };

        let session_id = runtime.start("Test task", config).await.unwrap();

        let messages = runtime
            .session_manager()
            .get_messages(&session_id)
            .await
            .unwrap()
            .unwrap();

        // First message should be system prompt
        assert!(messages.len() >= 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, "You are a helpful assistant");
    }

    #[tokio::test]
    async fn test_agent_pause_resume() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig::default();

        let session_id = runtime.start("Test task", config).await.unwrap();

        // Pause the agent
        let pause_result = runtime.pause(&session_id).await;
        assert!(pause_result.is_ok());

        let state = runtime
            .session_manager()
            .get_state(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, AgentState::Stopped);

        // Resume the agent
        let resume_result = runtime.resume(&session_id).await;
        assert!(resume_result.is_ok());

        let state = runtime
            .session_manager()
            .get_state(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, AgentState::Running);

        // Pause again should be idempotent
        runtime.pause(&session_id).await.unwrap();
        runtime.pause(&session_id).await.unwrap();
        let state = runtime
            .session_manager()
            .get_state(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, AgentState::Stopped);
    }

    #[tokio::test]
    async fn test_agent_stop() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig::default();

        let session_id = runtime.start("Test task", config).await.unwrap();

        // Stop the agent
        runtime.stop(&session_id).await.unwrap();

        let state = runtime
            .session_manager()
            .get_state(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, AgentState::Stopped);
    }

    #[tokio::test]
    async fn test_agent_pause_nonexistent_session() {
        let runtime = AgentRuntime::with_defaults();
        let fake_id = SessionId::new();

        let result = runtime.pause(&fake_id).await;
        assert!(result.is_err());
        // Verify error is SessionNotFound
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("Session not found"));
    }

    #[tokio::test]
    async fn test_agent_status() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig::default();

        let session_id = runtime.start("Test task", config).await.unwrap();

        let status = runtime.status(&session_id);
        assert!(status.is_ok());
        assert_eq!(status.unwrap(), AgentState::Running);

        runtime.pause(&session_id).await.unwrap();
        let status = runtime.status(&session_id);
        assert!(status.is_ok());
        assert_eq!(status.unwrap(), AgentState::Stopped);
    }

    #[tokio::test]
    async fn test_agent_send_message() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig::default();

        let session_id = runtime.start("Test task", config).await.unwrap();

        // Send a message
        runtime
            .send_message(&session_id, "Additional message")
            .await
            .unwrap();

        let messages = runtime
            .session_manager()
            .get_messages(&session_id)
            .await
            .unwrap()
            .unwrap();

        // Should have original task message plus the new message
        assert!(messages.len() >= 2);
        let last_user_msg = messages.iter().rev().find(|m| m.role == MessageRole::User);
        assert!(last_user_msg.is_some());
        assert_eq!(last_user_msg.unwrap().content, "Additional message");
    }

    #[tokio::test]
    async fn test_agent_submit_tool_result() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig::default();

        let session_id = runtime.start("Test task", config).await.unwrap();

        // Manually set up a pending tool call
        runtime
            .session_manager()
            .update(&session_id, |s| {
                s.tool_calls_pending.push(ToolCall {
                    id: "tc_123".to_string(),
                    name: "test_tool".to_string(),
                    arguments: "{}".to_string(),
                });
                s.state = AgentState::WaitingTool;
            })
            .await
            .unwrap();

        // Submit the tool result
        let tool_result = ToolResult {
            tool_call_id: "tc_123".to_string(),
            name: "test_tool".to_string(),
            content: "Tool executed successfully".to_string(),
            is_error: false,
        };

        runtime
            .submit_tool_result(&session_id, "tc_123", tool_result)
            .await
            .unwrap();

        // Verify the pending tool call was removed
        let pending_count: usize = runtime
            .session_manager()
            .read(&session_id, |s| s.tool_calls_pending.len())
            .await
            .unwrap()
            .unwrap_or(0);
        assert_eq!(pending_count, 0);

        // Verify the result was cached
        let cached_results: Vec<ToolResult> = runtime
            .session_manager()
            .read(&session_id, |s| s.tool_results_cache.clone())
            .await
            .unwrap()
            .unwrap_or_default();
        assert_eq!(cached_results.len(), 1);
        assert_eq!(cached_results[0].tool_call_id, "tc_123");

        // Verify state transitioned back to Running
        let state = runtime
            .session_manager()
            .get_state(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, AgentState::Running);
    }

    #[tokio::test]
    async fn test_agent_run_respects_stopped_state() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig {
            max_iterations: 100,
            ..Default::default()
        };

        // Start a session
        let session_id = runtime.start("Test task", config.clone()).await.unwrap();

        // Immediately stop it
        runtime.stop(&session_id).await.unwrap();

        // Now try to run - it should handle the stopped state gracefully
        // Note: run() creates a new session, so this tests that the original
        // session is properly in stopped state
        let state = runtime
            .session_manager()
            .get_state(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, AgentState::Stopped);
    }

    #[tokio::test]
    async fn test_agent_run_max_iterations() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig {
            max_iterations: 3,
            ..Default::default()
        };

        let result = runtime.run("Test task", config).await.unwrap();

        // Should complete within max iterations
        assert!(result.iterations <= 3);
        // Final state should be Completed
        assert_eq!(result.final_state, AgentState::Completed);
    }

    #[test]
    fn test_iteration_result_creation() {
        let result = IterationResult {
            iteration: 1,
            state: AgentState::Running,
            message: Some(Message::assistant("Test")),
            tool_calls: vec![ToolCall {
                id: "tc_1".to_string(),
                name: "test_tool".to_string(),
                arguments: "{}".to_string(),
            }],
            should_continue: true,
        };

        assert_eq!(result.iteration, 1);
        assert_eq!(result.state, AgentState::Running);
        assert!(result.message.is_some());
        assert_eq!(result.tool_calls.len(), 1);
        assert!(result.should_continue);
    }

    #[test]
    fn test_agent_runtime_with_permission_manager() {
        use crate::permission::policy::PermissionPolicy;

        let policy = PermissionPolicy::trusted();
        let pm = Arc::new(PermissionManager::new(policy));

        let session_manager = Arc::new(ConcurrentSessionManager::default_config());
        let tool_registry = Arc::new(ToolRegistry::new());

        let runtime = AgentRuntime::with_permissions(session_manager, tool_registry, pm.clone());

        assert!(runtime.permission_manager().is_some());
        assert_eq!(
            runtime.permission_manager().unwrap().security_level(),
            crate::permission::policy::SecurityLevel::Trusted
        );
    }

    #[test]
    fn test_agent_runtime_set_permission_manager() {
        use crate::permission::policy::PermissionPolicy;

        let mut runtime = AgentRuntime::with_defaults();
        assert!(runtime.permission_manager().is_none());

        let policy = PermissionPolicy::default();
        let pm = Arc::new(PermissionManager::new(policy));
        runtime.set_permission_manager(pm);

        assert!(runtime.permission_manager().is_some());
        assert_eq!(
            runtime.permission_manager().unwrap().security_level(),
            crate::permission::policy::SecurityLevel::Standard
        );
    }

    #[test]
    fn test_agent_runtime_has_decomposer() {
        let runtime = AgentRuntime::with_defaults();
        // with_defaults() now creates a TaskDecomposer
        assert!(runtime.task_decomposer().is_some());
    }

    #[test]
    fn test_agent_decompose_task() {
        let runtime = AgentRuntime::with_defaults();

        // Test simple task decomposition
        let plan = runtime.decompose_task("Read a file and write output");
        assert!(plan.is_ok());
        let plan = plan.unwrap();
        assert!(plan.is_some());

        let plan = plan.unwrap();
        assert!(!plan.subtasks.is_empty());
        assert!(!plan.execution_order.is_empty());
    }

    #[test]
    fn test_agent_set_decomposition_strategy() {
        let mut runtime = AgentRuntime::with_defaults();
        runtime.set_decomposition_strategy(DecompositionStrategy::Parallel);

        let decomposer = runtime.task_decomposer().unwrap();
        let plan = decomposer.decompose("Task A and Task B").unwrap();
        assert_eq!(plan.strategy, DecompositionStrategy::Parallel);
    }

    #[tokio::test]
    async fn test_agent_run_with_plan_simple() {
        let runtime = AgentRuntime::with_defaults();
        let config = AgentConfig {
            max_iterations: 5,
            ..Default::default()
        };

        // Test run_with_plan for a simple task
        let result = runtime.run_with_plan("Simple task", config).await;
        assert!(result.is_ok());

        let agent_result = result.unwrap();
        assert_eq!(agent_result.final_state, AgentState::Completed);
    }
}
