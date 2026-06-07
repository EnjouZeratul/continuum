//! # LSP (Language Server Protocol) 模块
//!
//! 完整的 LSP 客户端实现，支持多种语言服务器。
//!
//! ## 支持的语言服务器
//! - rust-analyzer (Rust)
//! - pyright/pylance (Python)
//! - typescript-language-server (TypeScript/JavaScript)
//! - gopls (Go)
//! - clangd (C/C++)
//!
//! ## 支持的功能
//! - go_to_definition: 跳转到定义（跨模块）
//! - find_references: 查找引用（项目级）
//! - get_hover: 获取类型信息
//! - rename_symbol: 重命名符号（重构）
//! - get_document_symbols: 获取文档符号
//! - get_workspace_symbols: 获取工作区符号
//! - get_code_actions: 获取代码操作（快速修复）
//! - get_signature_help: 获取签名帮助
//! - get_completions: 获取代码补全
//! - format_document: 格式化文档

pub mod client;
pub mod protocol;
pub mod server;
pub mod types;

pub use client::{LspClient, SyncLspClient};
pub use protocol::{
    create_code_action_request, create_completion_request, create_definition_request,
    create_did_change_notification, create_did_close_notification, create_did_open_notification,
    create_document_highlight_request, create_document_symbol_request, create_formatting_request,
    create_hover_request, create_initialize_request, create_references_request,
    create_rename_request, create_signature_help_request, create_workspace_symbol_request,
    LspMessage, LspNotification, LspRequest, LspResponse,
};
pub use server::{
    clangd_config, gopls_config, pylance_config, pyright_config, rust_analyzer_config,
    typescript_config, LanguageServer, LanguageServerConfig, LanguageServerManager,
};
pub use types::*;

/// LSP 错误类型
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("LSP server not found for language: {0}")]
    ServerNotFound(String),

    #[error("LSP server initialization failed: {0}")]
    InitializationFailed(String),

    #[error("LSP request failed: {0}")]
    RequestFailed(String),

    #[error("LSP timeout")]
    Timeout,

    #[error("LSP server crashed: {0}")]
    ServerCrashed(String),

    #[error("Invalid LSP message: {0}")]
    InvalidMessage(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// LSP 结果类型
pub type LspResult<T> = std::result::Result<T, LspError>;
