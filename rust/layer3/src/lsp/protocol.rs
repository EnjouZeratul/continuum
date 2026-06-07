//! LSP 协议实现
//!
//! 实现 LSP JSON-RPC 协议的消息编码和解码。

use super::types::*;
use super::LspError;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

/// JSON-RPC 请求 ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

/// JSON-RPC 请求消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRequest<P> {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
}

/// JSON-RPC 响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspResponse<R> {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<R>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 通知消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspNotification<P> {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
}

/// JSON-RPC 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// LSP 消息类型
#[derive(Debug, Clone)]
pub enum LspMessage {
    Request(LspRequest<serde_json::Value>),
    Response(LspResponse<serde_json::Value>),
    Notification(LspNotification<serde_json::Value>),
}

/// 消息内容类型（用于 Content-Length 头）
pub trait MessageContent {
    fn content(&self) -> Result<String, LspError>;
}

impl<P: Serialize> MessageContent for LspRequest<P> {
    fn content(&self) -> Result<String, LspError> {
        serde_json::to_string(self).map_err(LspError::Json)
    }
}

impl<R: Serialize> MessageContent for LspResponse<R> {
    fn content(&self) -> Result<String, LspError> {
        serde_json::to_string(self).map_err(LspError::Json)
    }
}

impl<P: Serialize> MessageContent for LspNotification<P> {
    fn content(&self) -> Result<String, LspError> {
        serde_json::to_string(self).map_err(LspError::Json)
    }
}

/// 写消息到输出流
pub fn write_message<W: Write, M: MessageContent>(
    output: &mut W,
    message: &M,
) -> Result<(), LspError> {
    let content = message.content()?;
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    output.write_all(header.as_bytes())?;
    output.write_all(content.as_bytes())?;
    output.flush()?;
    Ok(())
}

/// 从输入流读取消息
pub fn read_message<R: BufRead>(input: &mut R) -> Result<LspMessage, LspError> {
    // 读取 Content-Length 头
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        input.read_line(&mut line)?;

        let line = line.trim();
        if line.is_empty() {
            break;
        }

        if let Some(stripped) = line.strip_prefix("Content-Length:") {
            let value = stripped.trim();
            content_length = Some(
                value
                    .parse()
                    .map_err(|_| LspError::InvalidMessage("Invalid Content-Length".to_string()))?,
            );
        }
    }

    let content_length = content_length
        .ok_or_else(|| LspError::InvalidMessage("Missing Content-Length header".to_string()))?;

    // 读取内容
    let mut content = vec![0u8; content_length];
    input.read_exact(&mut content)?;

    // 解析消息
    let content_str = String::from_utf8(content)
        .map_err(|_| LspError::InvalidMessage("Invalid UTF-8 content".to_string()))?;

    // 尝试解析为各种消息类型
    let value: serde_json::Value = serde_json::from_str(&content_str)?;

    if let Some(_id) = value.get("id") {
        if value.get("error").is_some() {
            let response: LspResponse<serde_json::Value> = serde_json::from_str(&content_str)?;
            Ok(LspMessage::Response(response))
        } else {
            let request: LspRequest<serde_json::Value> = serde_json::from_str(&content_str)?;
            Ok(LspMessage::Request(request))
        }
    } else {
        let notification: LspNotification<serde_json::Value> = serde_json::from_str(&content_str)?;
        Ok(LspMessage::Notification(notification))
    }
}

/// 创建初始化请求
pub fn create_initialize_request(
    id: RequestId,
    root_uri: Option<String>,
    client_name: &str,
    client_version: &str,
) -> LspRequest<InitializeParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "initialize".to_string(),
        params: Some(InitializeParams {
            process_id: Some(std::process::id()),
            client_info: Some(ClientInfo {
                name: client_name.to_string(),
                version: Some(client_version.to_string()),
            }),
            root_uri,
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    completion: Some(CompletionClientCapabilities {
                        completion_item: Some(CompletionItemCapability {
                            documentation_format: Some(vec!["markdown".to_string()]),
                        }),
                    }),
                    hover: Some(HoverClientCapabilities {
                        content_format: Some(vec!["markdown".to_string()]),
                    }),
                }),
            },
        }),
    }
}

/// 创建 didOpen 通知
pub fn create_did_open_notification(
    uri: DocumentUri,
    language_id: &str,
    version: i32,
    text: &str,
) -> LspNotification<DidOpenTextDocumentParams> {
    LspNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version,
                text: text.to_string(),
            },
        }),
    }
}

/// 创建 didChange 通知
pub fn create_did_change_notification(
    uri: DocumentUri,
    version: i32,
    changes: Vec<TextDocumentContentChangeEvent>,
) -> LspNotification<DidChangeTextDocumentParams> {
    LspNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didChange".to_string(),
        params: Some(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: changes,
        }),
    }
}

/// 创建 didClose 通知
pub fn create_did_close_notification(
    uri: DocumentUri,
) -> LspNotification<DidCloseTextDocumentParams> {
    LspNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didClose".to_string(),
        params: Some(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        }),
    }
}

/// 创建定义请求
pub fn create_definition_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<DefinitionParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/definition".to_string(),
        params: Some(DefinitionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}

/// 创建引用请求
pub fn create_references_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
    include_declaration: bool,
) -> LspRequest<ReferenceParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/references".to_string(),
        params: Some(ReferenceParams {
            text_document: TextDocumentIdentifier { uri },
            position,
            context: ReferenceContext {
                include_declaration,
            },
        }),
    }
}

/// 创建 Hover 请求
pub fn create_hover_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<HoverParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/hover".to_string(),
        params: Some(HoverParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}

/// 创建重命名请求
pub fn create_rename_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
    new_name: &str,
) -> LspRequest<RenameParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/rename".to_string(),
        params: Some(RenameParams {
            text_document: TextDocumentIdentifier { uri },
            position,
            new_name: new_name.to_string(),
        }),
    }
}

/// 创建文档符号请求
pub fn create_document_symbol_request(
    id: RequestId,
    uri: DocumentUri,
) -> LspRequest<DocumentSymbolParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/documentSymbol".to_string(),
        params: Some(DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
        }),
    }
}

// ============================================================================
// 参数类型
// ============================================================================

/// didOpen 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidOpenTextDocumentParams {
    pub text_document: TextDocumentItem,
}

/// didChange 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidChangeTextDocumentParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<TextDocumentContentChangeEvent>,
}

/// didClose 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidCloseTextDocumentParams {
    pub text_document: TextDocumentIdentifier,
}

/// 定义请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 引用请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub context: ReferenceContext,
}

/// Hover 请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 重命名请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub new_name: String,
}

/// 文档符号请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbolParams {
    pub text_document: TextDocumentIdentifier,
}

/// 工作区符号请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSymbolParams {
    pub query: String,
}

/// 代码动作请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeActionParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    pub context: CodeActionContext,
}

/// 签名帮助请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureHelpParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 文档高亮请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentHighlightParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 格式化请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub options: FormattingOptions,
}

/// 范围格式化请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRangeFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    pub options: FormattingOptions,
}

/// 创建工作区符号请求
pub fn create_workspace_symbol_request(
    id: RequestId,
    query: &str,
) -> LspRequest<WorkspaceSymbolParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "workspace/symbol".to_string(),
        params: Some(WorkspaceSymbolParams {
            query: query.to_string(),
        }),
    }
}

/// 创建代码动作请求
pub fn create_code_action_request(
    id: RequestId,
    uri: DocumentUri,
    range: Range,
    context: CodeActionContext,
) -> LspRequest<CodeActionParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/codeAction".to_string(),
        params: Some(CodeActionParams {
            text_document: TextDocumentIdentifier { uri },
            range,
            context,
        }),
    }
}

/// 创建签名帮助请求
pub fn create_signature_help_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<SignatureHelpParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/signatureHelp".to_string(),
        params: Some(SignatureHelpParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}

/// 创建文档高亮请求
pub fn create_document_highlight_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<DocumentHighlightParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/documentHighlight".to_string(),
        params: Some(DocumentHighlightParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}

/// 创建格式化请求
pub fn create_formatting_request(
    id: RequestId,
    uri: DocumentUri,
    options: FormattingOptions,
) -> LspRequest<DocumentFormattingParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/formatting".to_string(),
        params: Some(DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options,
        }),
    }
}

/// 创建完成请求
pub fn create_completion_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<CompletionParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/completion".to_string(),
        params: Some(CompletionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}

/// 完成请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 实现请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 类型定义请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefinitionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// 创建实现请求
pub fn create_implementation_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<ImplementationParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/implementation".to_string(),
        params: Some(ImplementationParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}

/// 创建类型定义请求
pub fn create_type_definition_request(
    id: RequestId,
    uri: DocumentUri,
    position: Position,
) -> LspRequest<TypeDefinitionParams> {
    LspRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "textDocument/typeDefinition".to_string(),
        params: Some(TypeDefinitionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        }),
    }
}
