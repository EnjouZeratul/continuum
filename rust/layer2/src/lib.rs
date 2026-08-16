//! # Continuum Layer 2: Core Engine
//!
//! Agent 运行时核心引擎。
//!
//! ## 模块结构
//! - `types`: 核心类型定义
//! - `agent_runtime`: Agent 运行时接口
//! - `session_manager`: 会话管理（并发安全）
//! - `tool_registry`: 工具注册发现
//! - `workflow_engine`: DAG 工作流引擎
//! - `hook_system`: 生命周期钩子
//! - `checkpoint_system`: 检查点持久化
//! - `tasks`: 任务队列管理
//! - `prompts`: 提示词管理
//! - `permission`: 交互式权限系统
//! - `planner`: 任务分解和执行计划
//! - `execution_monitor`: 执行监控和自我纠错

// Re-export Layer 1 (and transitively Layer 0) for upper layers
pub use sh_layer1;

// Re-export ID generation functions for Layer 3
pub use sh_layer1::{generate_prefixed_id, generate_short_id};

pub mod agent_runtime;
pub mod checkpoint_system;
pub mod execution_monitor;
pub mod hook_system;
pub mod permission;
pub mod planner;
pub mod planner_llm;
pub mod prompts;
pub mod session_manager;
pub mod tasks;
pub mod tool_registry;
pub mod types;
pub mod workflow_engine;

// 导出核心类型
pub use types::{
    AgentId, AgentState, CheckpointId, CheckpointMeta, HookEvent, Layer2Error, Layer2Result,
    Message, MessageRole, SessionId, SessionMeta, TaskId, ToolCall, ToolResult, WorkflowNode,
};

// 导出主要组件
pub use agent_runtime::{
    AgentConfig, AgentLoopCallback, AgentResult, AgentRuntime, AgentRuntimeTrait, IterationResult,
};

pub use session_manager::{
    ConcurrentSessionManager, ExecutionContext, ReadWriteLock, Session, SessionConfig,
    SessionManagerTrait, SessionStats,
};

pub use tool_registry::{
    FunctionDefinition, Tool, ToolDefinition, ToolMeta, ToolRegistry, ToolRegistryTrait,
    ToolRequest,
};

pub use workflow_engine::{
    Dag, Node, NodeExecutor, NodeResult, NodeStatus, WorkflowEngineTrait, WorkflowExecutor,
    WorkflowInput, WorkflowOutput, WorkflowStatus,
};

pub use hook_system::{HookCallback, HookContext, HookSystem, HookSystemTrait};

pub use checkpoint_system::{
    AtomicFileWriter, CheckpointData, CheckpointSystemTrait, CheckpointWriter, ChecksumUtils,
    CrashRecovery, ErrorCategory, ErrorRecovery, FallbackStrategy, InterruptedSession,
    RecoveryAction, RecoveryLayer, RecoveryResult, RetryPolicy, SessionRecovery,
};

pub use tasks::{Task, TaskManager, TaskManagerTrait, TaskPriority, TaskStatus};

pub use prompts::{PromptManager, PromptManagerTrait, PromptTemplate};

pub use permission::{
    AuditEntry, CachedPermission, PermissionAction, PermissionContext, PermissionDecision,
    PermissionError, PermissionManager, PermissionPolicy, PermissionRequest, PermissionResponse,
    PermissionResult, PermissionRule, SecurityLevel,
};

pub use planner::{
    DecompositionStrategy, ExecutionPlan, PlanResult, RiskLevel, SubTask, TaskDecomposer,
};

pub use execution_monitor::{
    CorrectionDecision, CorrectionRecord, CorrectionStrategy, ExecutionMonitor, ExecutionStatus,
    ExecutionSummary, SelfCorrector, StepResult,
};

// 导出 trait 以便外部实现
pub mod traits {
    pub use super::agent_runtime::AgentRuntimeTrait;
    pub use super::checkpoint_system::CheckpointSystemTrait;
    pub use super::hook_system::HookSystemTrait;
    pub use super::prompts::PromptManagerTrait;
    pub use super::session_manager::SessionManagerTrait;
    pub use super::tasks::TaskManagerTrait;
    pub use super::tool_registry::Tool;
    pub use super::tool_registry::ToolRegistryTrait;
    pub use super::workflow_engine::WorkflowEngineTrait;
}
