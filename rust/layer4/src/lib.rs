//! # Continuum Layer 4: Integration
//!
//! 外部系统和协议集成。
//!
//! ## 模块结构
//! - `types`: 核心类型定义
//! - `channel_gateway`: 多渠道网关
//! - `plugin_loader`: 插件加载器
//! - `worktree_manager`: Git Worktree 管理
//! - `mcp_bridge`: MCP 协议桥接 (Terminal 1)
//! - `audit_logger`: 审计日志 (Terminal 1)
//! - `compliance_checker`: 合规检查器

// 允许未使用的代码（测试和未来功能）
#![allow(dead_code)]

pub mod audit_logger;
pub mod channel_gateway;
pub mod compliance_checker;
pub mod mcp_bridge;
pub mod plugin_loader;
pub mod types;
pub mod worktree_manager;

// Re-export Layer 3 (and transitively Layer 2, Layer 1, Layer 0) for upper layers
pub use sh_layer3;

// 导出核心类型
pub use types::{IntegrationConfig, Layer4Error, Layer4Result, MessagePriority, PluginPermission};

// 导出渠道网关
pub use channel_gateway::{
    Channel, ChannelGateway, ChannelType, InboundMessage, MessageRouter, MessageTarget,
    MessageType, OutboundMessage,
};

// 导出渠道适配器
pub use channel_gateway::adapter::{CliChannel, HttpChannel, WebSocketChannel};

// 导出插件加载器
pub use plugin_loader::{
    Plugin, PluginContext, PluginInfo, PluginLoader, PluginMeta, PluginRegistry, PluginState,
};

// 导出 Worktree 管理器
pub use worktree_manager::{Worktree, WorktreeConfig, WorktreeManager, WorktreeStatus};

// 导出 MCP 桥接 (Terminal 1)
pub use mcp_bridge::{
    ContentBlock, DefaultHandler, McpBridge, McpBridgeConfig, McpClientManager, McpError,
    McpHandler, McpMessage, McpServerConfig, McpTransport, McpTransportType, MemoryTransport,
    SimpleToolExecutor, StdioTransport, TcpTransport, ToolDefinition, ToolExecutor, ToolResult,
    MCP_VERSION,
};

// 导出审计日志 (Terminal 1)
pub use audit_logger::{
    AuditAction, AuditConfig, AuditEntry, AuditFilter, AuditLogger, AuditResult, AuditStorage,
    ExportFormat, FileStorage, MemoryStorage,
};

// 导出合规检查器
pub use compliance_checker::{
    CheckContext, CheckResult, ComplianceChecker, ComplianceConfig, ComplianceReport,
    ComplianceRule, ComplianceStandard, ComplianceStatus, ComplianceSummary, QuickCheckResult,
    ReportFormat, ResourceInfo, RuleCategory, RuleChecker, RuleSeverity, Violation,
};

// 导出 traits
pub mod traits {
    pub use super::channel_gateway::Channel;
    pub use super::plugin_loader::Plugin;
}
