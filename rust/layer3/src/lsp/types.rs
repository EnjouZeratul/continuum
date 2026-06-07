//! LSP 类型定义
//!
//! 定义 LSP 协议中使用的所有类型。

use serde::{Deserialize, Serialize};

// ============================================================================
// 基础类型
// ============================================================================

/// 文档 URI
pub type DocumentUri = String;

/// 位置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    /// 行号（0-based）
    pub line: u32,
    /// 列号（0-based, UTF-16 code units）
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// 范围
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

/// 文本编辑
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// 文档位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: DocumentUri,
    pub range: Range,
}

/// 文档链接位置（带额外信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationLink {
    pub origin_selection_range: Option<Range>,
    pub target_uri: DocumentUri,
    pub target_range: Range,
    pub target_selection_range: Range,
}

// ============================================================================
// 文档同步
// ============================================================================

/// 文本文档内容变更事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentContentChangeEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_length: Option<u32>,
    pub text: String,
}

/// 文本文档项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentItem {
    pub uri: DocumentUri,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

/// 文本文档标识符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: DocumentUri,
}

/// 版本化文本文档标识符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedTextDocumentIdentifier {
    pub uri: DocumentUri,
    pub version: i32,
}

// ============================================================================
// 诊断
// ============================================================================

/// 诊断严重程度
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

/// 诊断相关标签
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticTag {
    Unnecessary = 1,
    Deprecated = 2,
}

/// 诊断信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<DiagnosticSeverity>,
    pub code: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_description: Option<CodeDescription>,
    pub source: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<DiagnosticTag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_information: Option<Vec<DiagnosticRelatedInformation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// 代码描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeDescription {
    pub href: String,
}

/// 诊断相关信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRelatedInformation {
    pub location: Location,
    pub message: String,
}

// ============================================================================
// Hover
// ============================================================================

/// 标记内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    pub kind: MarkupKind,
    pub value: String,
}

/// 标记类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkupKind {
    PlainText,
    Markdown,
}

/// Hover 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    pub contents: HoverContents,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

/// Hover 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HoverContents {
    Markup(MarkupContent),
    String(String),
    Array(Vec<MarkedString>),
}

/// 标记字符串
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarkedString {
    String(String),
    LanguageString(LanguageString),
}

/// 语言字符串
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageString {
    pub language: String,
    pub value: String,
}

// ============================================================================
// Definition / References
// ============================================================================

/// 定义结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DefinitionResult {
    Single(Location),
    Multiple(Vec<Location>),
    Link(LocationLink),
    Links(Vec<LocationLink>),
}

impl DefinitionResult {
    /// 转换为位置列表
    pub fn to_locations(&self) -> Vec<Location> {
        match self {
            DefinitionResult::Single(loc) => vec![loc.clone()],
            DefinitionResult::Multiple(locs) => locs.clone(),
            DefinitionResult::Link(link) => vec![Location {
                uri: link.target_uri.clone(),
                range: link.target_selection_range.clone(),
            }],
            DefinitionResult::Links(links) => links
                .iter()
                .map(|link| Location {
                    uri: link.target_uri.clone(),
                    range: link.target_selection_range.clone(),
                })
                .collect(),
        }
    }
}

/// 引用上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceContext {
    /// 是否包含声明
    pub include_declaration: bool,
}

/// 引用结果
pub type ReferenceResult = Vec<Location>;

// ============================================================================
// Rename
// ============================================================================

/// 工作区编辑
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<std::collections::HashMap<DocumentUri, Vec<TextEdit>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_changes: Option<Vec<DocumentChange>>,
}

/// 文档变更
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChange {
    pub text_document: VersionedTextDocumentIdentifier,
    pub edits: Vec<TextEdit>,
}

/// 重命名结果
pub type RenameResult = WorkspaceEdit;

/// 准备重命名结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PrepareRenameResult {
    Range(Range),
    RangeWithPlaceholder { range: Range, placeholder: String },
}

// ============================================================================
// 符号
// ============================================================================

/// 符号类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

/// 符号标签
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SymbolTag {
    Deprecated = 1,
}

/// 文档符号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub kind: SymbolKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<SymbolTag>>,
    pub deprecated: Option<bool>,
    pub range: Range,
    pub selection_range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DocumentSymbol>>,
}

/// 符号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: SymbolKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<SymbolTag>>,
    pub deprecated: Option<bool>,
    pub location: Location,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

// ============================================================================
// 完成提示
// ============================================================================

/// 完成项类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

/// 完成项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CompletionItemKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<MarkupContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

/// 完成列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

/// 完成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompletionResult {
    List(CompletionList),
    Items(Vec<CompletionItem>),
}

// ============================================================================
// 服务端能力
// ============================================================================

/// 服务端能力
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document_sync: Option<TextDocumentSyncCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<CompletionOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_symbol_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename_provider: Option<RenameOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_symbol_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_action_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_help_provider: Option<SignatureHelpOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_formatting_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_range_formatting_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_highlight_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_definition_provider: Option<bool>,
}

/// 签名帮助选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureHelpOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_characters: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrigger_characters: Option<Vec<String>>,
}

/// 文档同步能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentSyncCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_close: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_save: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_save_wait_until: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
}

/// 完成选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_characters: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_provider: Option<bool>,
}

/// 重命名选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_provider: Option<bool>,
}

/// 初始化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
}

/// 服务端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// 初始化参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub process_id: Option<u32>,
    pub client_info: Option<ClientInfo>,
    pub root_uri: Option<DocumentUri>,
    pub capabilities: ClientCapabilities,
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// 客户端能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document: Option<TextDocumentClientCapabilities>,
}

/// 文档客户端能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<CompletionClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<HoverClientCapabilities>,
}

/// 完成客户端能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item: Option<CompletionItemCapability>,
}

/// 完成项能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItemCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_format: Option<Vec<String>>,
}

/// Hover 客户端能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_format: Option<Vec<String>>,
}

// ============================================================================
// Code Actions
// ============================================================================

/// 代码动作种类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodeActionKind {
    Empty,
    QuickFix,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
    Source,
    SourceOrganizeImports,
    SourceFixAll,
}

/// 代码动作上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeActionContext {
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<CodeActionKind>>,
}

/// 代码动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAction {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CodeActionKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<Diagnostic>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit: Option<WorkspaceEdit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
}

/// 命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub title: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<serde_json::Value>>,
}

// ============================================================================
// Signature Help
// ============================================================================

/// 签名信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInformation {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<MarkupContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<ParameterInformation>>,
}

/// 参数信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInformation {
    pub label: ParameterLabel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<MarkupContent>,
}

/// 参数标签
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterLabel {
    String(String),
    Range([u32; 2]),
}

/// 签名帮助
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_signature: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_parameter: Option<u32>,
}

// ============================================================================
// Document Highlight
// ============================================================================

/// 文档高亮种类
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentHighlightKind {
    Text = 1,
    Read = 2,
    Write = 3,
}

/// 文档高亮
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentHighlight {
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<DocumentHighlightKind>,
}

// ============================================================================
// Formatting
// ============================================================================

/// 格式化选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_trailing_whitespace: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_final_newline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_final_newlines: Option<bool>,
}

// ============================================================================
// Publish Diagnostics
// ============================================================================

/// 发布诊断参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishDiagnosticsParams {
    pub uri: DocumentUri,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}
