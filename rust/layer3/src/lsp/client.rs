//! LSP 客户端实现
//!
//! 完整的 LSP 客户端，支持与语言服务器通信。

use super::protocol::*;
use super::server::{LanguageServer, LanguageServerManager};
use super::types::*;
use super::{LspError, LspResult};

// Re-export protocol functions needed for implementations
use super::protocol::{create_implementation_request, create_type_definition_request};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// LSP 服务器连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 未连接
    Disconnected,
    /// 正在初始化
    Initializing,
    /// 已连接并就绪
    Ready,
    /// 已关闭
    Shutdown,
    /// 错误状态
    Error,
}

/// LSP 服务器连接
#[allow(dead_code)]
struct ServerConnection {
    /// 语言名称
    language: String,
    /// 根路径
    root_path: PathBuf,
    /// stdin 写入器
    stdin: std::process::ChildStdin,
    /// stdout 读取器（需要互斥访问）
    stdout: std::io::BufReader<std::process::ChildStdout>,
    /// 服务器能力
    capabilities: Option<ServerCapabilities>,
    /// 文档版本计数器
    doc_versions: HashMap<String, i32>,
    /// 已打开的文档
    open_docs: HashMap<String, String>,
}

/// LSP 客户端
pub struct LspClient {
    /// 服务器管理器
    manager: Arc<LanguageServerManager>,
    /// 请求 ID 计数器
    request_id: AtomicI64,
    /// 服务器连接（按语言区分）
    connections: Arc<RwLock<HashMap<String, ServerConnection>>>,
    /// 连接状态
    states: Arc<RwLock<HashMap<String, ConnectionState>>>,
    /// 超时时间（毫秒）
    timeout_ms: u64,
}

impl LspClient {
    /// 创建新的 LSP 客户端
    pub fn new() -> Self {
        Self {
            manager: Arc::new(LanguageServerManager::new()),
            request_id: AtomicI64::new(1),
            connections: Arc::new(RwLock::new(HashMap::new())),
            states: Arc::new(RwLock::new(HashMap::new())),
            timeout_ms: 30000,
        }
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// 获取下一个请求 ID
    fn next_id(&self) -> RequestId {
        RequestId::Number(self.request_id.fetch_add(1, Ordering::SeqCst))
    }

    /// 获取连接状态
    pub async fn get_state(&self, language: &str) -> ConnectionState {
        self.states
            .read()
            .await
            .get(language)
            .copied()
            .unwrap_or(ConnectionState::Disconnected)
    }

    /// 检查是否已连接
    pub async fn is_connected(&self, language: &str) -> bool {
        self.get_state(language).await == ConnectionState::Ready
    }

    /// 启动语言服务器并初始化
    pub async fn initialize(
        &self,
        language: &str,
        root_path: &Path,
    ) -> LspResult<InitializeResult> {
        let root_path = root_path
            .canonicalize()
            .unwrap_or_else(|_| root_path.to_path_buf());

        // 设置状态为初始化中
        self.states
            .write()
            .await
            .insert(language.to_string(), ConnectionState::Initializing);

        // 获取服务器配置
        let config = self
            .manager
            .get_config(language)
            .ok_or_else(|| LspError::ServerNotFound(language.to_string()))?
            .clone();

        // 启动服务器进程
        let mut server = LanguageServer::new(config.clone(), root_path.clone())?;
        server.start()?;

        // 获取 stdin/stdout
        let stdin = server
            .process
            .as_mut()
            .and_then(|p| p.stdin.take())
            .ok_or_else(|| LspError::RequestFailed("Failed to get stdin".to_string()))?;

        let stdout = server
            .process
            .as_mut()
            .and_then(|p| p.stdout.take())
            .ok_or_else(|| LspError::RequestFailed("Failed to get stdout".to_string()))?;

        // 创建连接
        let root_uri = self.path_to_uri(&root_path);
        let request = create_initialize_request(
            self.next_id(),
            Some(root_uri),
            "continuum",
            env!("CARGO_PKG_VERSION"),
        );

        // 发送初始化请求
        let mut writer = std::io::BufWriter::new(stdin);
        let mut reader = std::io::BufReader::new(stdout);

        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<InitializeResult>(&mut reader)?;

        // 发送 initialized 通知
        let initialized_notification = LspNotification {
            jsonrpc: "2.0".to_string(),
            method: "initialized".to_string(),
            params: Some(serde_json::json!({})),
        };
        write_message(&mut writer, &initialized_notification)?;
        writer.flush().map_err(LspError::Io)?;

        // 保存连接
        let connection = ServerConnection {
            language: language.to_string(),
            root_path: root_path.clone(),
            stdin: writer
                .into_inner()
                .map_err(|e| LspError::RequestFailed(format!("Failed to get stdin: {}", e)))?,
            stdout: reader,
            capabilities: Some(
                response
                    .result
                    .clone()
                    .map(|r| r.capabilities)
                    .unwrap_or_default(),
            ),
            doc_versions: HashMap::new(),
            open_docs: HashMap::new(),
        };

        self.connections
            .write()
            .await
            .insert(language.to_string(), connection);
        self.states
            .write()
            .await
            .insert(language.to_string(), ConnectionState::Ready);

        // 注册服务器
        self.manager
            .servers
            .lock()
            .await
            .insert(language.to_string(), server);

        Ok(response.result.unwrap_or_else(|| InitializeResult {
            capabilities: ServerCapabilities::default(),
            server_info: None,
        }))
    }

    /// 打开文档
    pub async fn open_document(&self, language: &str, file_path: &Path) -> LspResult<()> {
        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        let uri = self.path_to_uri(file_path);

        // 检查是否已打开
        if conn.open_docs.contains_key(&uri) {
            return Ok(());
        }

        // 读取文件内容
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| LspError::RequestFailed(format!("Failed to read file: {}", e)))?;

        let version = conn.doc_versions.entry(uri.clone()).or_insert(0);
        *version += 1;

        // 发送 didOpen 通知
        let notification = create_did_open_notification(uri.clone(), language, *version, &content);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &notification)?;
        writer.flush().map_err(LspError::Io)?;

        conn.open_docs.insert(uri, content);

        Ok(())
    }

    /// 关闭文档
    pub async fn close_document(&self, language: &str, file_path: &Path) -> LspResult<()> {
        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        let uri = self.path_to_uri(file_path);

        // 发送 didClose 通知
        let notification = create_did_close_notification(uri.clone());

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &notification)?;
        writer.flush().map_err(LspError::Io)?;

        conn.open_docs.remove(&uri);
        conn.doc_versions.remove(&uri);

        Ok(())
    }

    /// 跳转到定义
    pub async fn go_to_definition(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<Location>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.definition_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Err(LspError::RequestFailed(
                "Server does not support go to definition".to_string(),
            ));
        }

        let uri = self.path_to_uri(file_path);
        let request = create_definition_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<DefinitionResult>>(&mut conn.stdout)?;

        match response.result {
            Some(Some(result)) => Ok(result.to_locations()),
            Some(None) => Ok(Vec::new()),
            None => Ok(Vec::new()),
        }
    }

    /// 查找引用
    pub async fn find_references(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
        include_declaration: bool,
    ) -> LspResult<Vec<Location>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.references_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Err(LspError::RequestFailed(
                "Server does not support find references".to_string(),
            ));
        }

        let uri = self.path_to_uri(file_path);
        let request = create_references_request(self.next_id(), uri, position, include_declaration);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<ReferenceResult>>(&mut conn.stdout)?;

        Ok(response.result.flatten().unwrap_or_default())
    }

    /// 获取 Hover 信息
    pub async fn get_hover(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Option<Hover>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.hover_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Ok(None);
        }

        let uri = self.path_to_uri(file_path);
        let request = create_hover_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<Hover>>(&mut conn.stdout)?;

        Ok(response.result.flatten())
    }

    /// 重命名符号
    pub async fn rename_symbol(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
        new_name: &str,
    ) -> LspResult<Option<WorkspaceEdit>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if conn
            .capabilities
            .as_ref()
            .map(|c| c.rename_provider.is_none())
            .unwrap_or(true)
        {
            return Err(LspError::RequestFailed(
                "Server does not support rename".to_string(),
            ));
        }

        let uri = self.path_to_uri(file_path);
        let request = create_rename_request(self.next_id(), uri, position, new_name);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<RenameResult>>(&mut conn.stdout)?;

        Ok(response.result.flatten())
    }

    /// 获取文档符号
    pub async fn get_document_symbols(
        &self,
        language: &str,
        file_path: &Path,
    ) -> LspResult<Vec<DocumentSymbol>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.document_symbol_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let uri = self.path_to_uri(file_path);
        let request = create_document_symbol_request(self.next_id(), uri);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<Vec<DocumentSymbol>>>(&mut conn.stdout)?;

        Ok(response.result.flatten().unwrap_or_default())
    }

    /// 获取工作区符号
    pub async fn get_workspace_symbols(
        &self,
        language: &str,
        query: &str,
    ) -> LspResult<Vec<SymbolInformation>> {
        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.workspace_symbol_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let request = create_workspace_symbol_request(self.next_id(), query);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<Vec<SymbolInformation>>>(&mut conn.stdout)?;

        Ok(response.result.flatten().unwrap_or_default())
    }

    /// 获取代码操作
    pub async fn get_code_actions(
        &self,
        language: &str,
        file_path: &Path,
        range: Range,
        diagnostics: Vec<Diagnostic>,
        only: Option<Vec<CodeActionKind>>,
    ) -> LspResult<Vec<CodeAction>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if conn
            .capabilities
            .as_ref()
            .map(|c| c.code_action_provider.is_none())
            .unwrap_or(true)
        {
            return Ok(Vec::new());
        }

        let uri = self.path_to_uri(file_path);
        let request = create_code_action_request(
            self.next_id(),
            uri,
            range,
            CodeActionContext { diagnostics, only },
        );

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<Vec<CodeAction>>>(&mut conn.stdout)?;

        Ok(response.result.flatten().unwrap_or_default())
    }

    /// 获取签名帮助
    pub async fn get_signature_help(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Option<SignatureHelp>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if conn
            .capabilities
            .as_ref()
            .map(|c| c.signature_help_provider.is_none())
            .unwrap_or(true)
        {
            return Ok(None);
        }

        let uri = self.path_to_uri(file_path);
        let request = create_signature_help_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<SignatureHelp>>(&mut conn.stdout)?;

        Ok(response.result.flatten())
    }

    /// 获取文档高亮
    pub async fn get_document_highlights(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<DocumentHighlight>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        let uri = self.path_to_uri(file_path);
        let request = create_document_highlight_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<Vec<DocumentHighlight>>>(&mut conn.stdout)?;

        Ok(response.result.flatten().unwrap_or_default())
    }

    /// 格式化文档
    pub async fn format_document(
        &self,
        language: &str,
        file_path: &Path,
        tab_size: u32,
        insert_spaces: bool,
    ) -> LspResult<Vec<TextEdit>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.document_formatting_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let uri = self.path_to_uri(file_path);
        let request = create_formatting_request(
            self.next_id(),
            uri,
            FormattingOptions {
                tab_size,
                insert_spaces,
                trim_trailing_whitespace: Some(true),
                insert_final_newline: Some(true),
                trim_final_newlines: Some(true),
            },
        );

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<Vec<TextEdit>>>(&mut conn.stdout)?;

        Ok(response.result.flatten().unwrap_or_default())
    }

    /// 获取代码补全
    pub async fn get_completions(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<CompletionItem>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if conn
            .capabilities
            .as_ref()
            .map(|c| c.completion_provider.is_none())
            .unwrap_or(true)
        {
            return Ok(Vec::new());
        }

        let uri = self.path_to_uri(file_path);
        let request = create_completion_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<CompletionResult>>(&mut conn.stdout)?;

        match response.result {
            Some(Some(CompletionResult::List(list))) => Ok(list.items),
            Some(Some(CompletionResult::Items(items))) => Ok(items),
            _ => Ok(Vec::new()),
        }
    }

    /// 跳转到实现
    pub async fn go_to_implementation(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<Location>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.implementation_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let uri = self.path_to_uri(file_path);
        let request = create_implementation_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<DefinitionResult>>(&mut conn.stdout)?;

        match response.result {
            Some(Some(result)) => Ok(result.to_locations()),
            _ => Ok(Vec::new()),
        }
    }

    /// 跳转到类型定义
    pub async fn go_to_type_definition(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Option<Location>> {
        // 确保文档已打开
        self.open_document(language, file_path).await?;

        let mut connections = self.connections.write().await;
        let conn = connections.get_mut(language).ok_or_else(|| {
            LspError::RequestFailed("Language server not initialized".to_string())
        })?;

        // 检查能力
        if !conn
            .capabilities
            .as_ref()
            .map(|c| c.type_definition_provider.unwrap_or(false))
            .unwrap_or(false)
        {
            return Ok(None);
        }

        let uri = self.path_to_uri(file_path);
        let request = create_type_definition_request(self.next_id(), uri, position);

        let mut writer = std::io::BufWriter::new(&mut conn.stdin);
        write_message(&mut writer, &request)?;
        writer.flush().map_err(LspError::Io)?;

        // 读取响应
        let response = self.read_response::<Option<DefinitionResult>>(&mut conn.stdout)?;

        match response.result {
            Some(Some(DefinitionResult::Single(loc))) => Ok(Some(loc)),
            Some(Some(DefinitionResult::Multiple(locs))) => Ok(locs.first().cloned()),
            Some(Some(DefinitionResult::Link(link))) => Ok(Some(Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })),
            Some(Some(DefinitionResult::Links(links))) => Ok(links.first().map(|link| Location {
                uri: link.target_uri.clone(),
                range: link.target_selection_range.clone(),
            })),
            _ => Ok(None),
        }
    }

    /// 关闭语言服务器
    pub async fn shutdown(&self, language: &str) -> LspResult<()> {
        let mut states = self.states.write().await;
        states.insert(language.to_string(), ConnectionState::Shutdown);

        // 发送 shutdown 请求
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.get_mut(language) {
                let request = LspRequest {
                    jsonrpc: "2.0".to_string(),
                    id: self.next_id(),
                    method: "shutdown".to_string(),
                    params: None::<()>,
                };

                let mut writer = std::io::BufWriter::new(&mut conn.stdin);
                write_message(&mut writer, &request)?;
                writer.flush().ok();

                // 发送 exit 通知
                let notification = LspNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "exit".to_string(),
                    params: None::<()>,
                };
                write_message(&mut writer, &notification)?;
                writer.flush().ok();
            }
            connections.remove(language);
        }

        self.manager.stop_server(language).await?;

        Ok(())
    }

    /// 关闭所有语言服务器
    pub async fn shutdown_all(&self) -> LspResult<()> {
        let languages: Vec<String> = self.connections.read().await.keys().cloned().collect();
        for lang in languages {
            self.shutdown(&lang).await.ok();
        }
        Ok(())
    }

    /// 路径转 URI
    fn path_to_uri(&self, path: &Path) -> String {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = path.display().to_string();
        if cfg!(windows) {
            format!("file:///{}", path_str.replace('\\', "/"))
        } else {
            format!("file://{}", path_str)
        }
    }

    /// URI 转路径
    #[allow(dead_code)]
    fn uri_to_path(&self, uri: &str) -> PathBuf {
        let path = uri.strip_prefix("file://").unwrap_or(uri);
        let path = path.strip_prefix('/').unwrap_or(path);
        if cfg!(windows) {
            PathBuf::from(path.replace('/', "\\"))
        } else {
            PathBuf::from(path)
        }
    }

    /// 读取响应
    fn read_response<R: serde::de::DeserializeOwned>(
        &self,
        reader: &mut impl BufRead,
    ) -> LspResult<LspResponse<R>> {
        let message = read_message(reader)?;

        match message {
            LspMessage::Response(response) => {
                let response: LspResponse<R> =
                    serde_json::from_value(serde_json::to_value(response)?)?;
                if let Some(error) = &response.error {
                    return Err(LspError::RequestFailed(format!(
                        "LSP error {}: {}",
                        error.code, error.message
                    )));
                }
                Ok(response)
            }
            LspMessage::Request(_) => Err(LspError::InvalidMessage(
                "Expected response, got request".to_string(),
            )),
            LspMessage::Notification(_) => {
                // 忽略通知，继续读取
                self.read_response(reader)
            }
        }
    }

    /// 根据文件扩展名获取语言
    pub fn get_language_from_extension(ext: &str) -> Option<&'static str> {
        LanguageServerManager::get_language_from_extension(ext)
    }

    /// 获取服务器管理器
    pub fn manager(&self) -> Arc<LanguageServerManager> {
        self.manager.clone()
    }
}

impl Default for LspClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 同步 LSP 客户端（简化版，用于单次调用）
// ============================================================================

/// 同步 LSP 客户端
pub struct SyncLspClient {
    inner: LspClient,
    runtime: tokio::runtime::Runtime,
}

impl SyncLspClient {
    pub fn new() -> Self {
        Self {
            inner: LspClient::new(),
            runtime: tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"),
        }
    }

    pub fn initialize(&self, language: &str, root_path: &Path) -> LspResult<InitializeResult> {
        self.runtime
            .block_on(self.inner.initialize(language, root_path))
    }

    pub fn go_to_definition(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<Location>> {
        self.runtime
            .block_on(self.inner.go_to_definition(language, file_path, position))
    }

    pub fn find_references(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
        include_declaration: bool,
    ) -> LspResult<Vec<Location>> {
        self.runtime.block_on(self.inner.find_references(
            language,
            file_path,
            position,
            include_declaration,
        ))
    }

    pub fn get_hover(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Option<Hover>> {
        self.runtime
            .block_on(self.inner.get_hover(language, file_path, position))
    }

    pub fn rename_symbol(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
        new_name: &str,
    ) -> LspResult<Option<WorkspaceEdit>> {
        self.runtime.block_on(
            self.inner
                .rename_symbol(language, file_path, position, new_name),
        )
    }

    pub fn get_document_symbols(
        &self,
        language: &str,
        file_path: &Path,
    ) -> LspResult<Vec<DocumentSymbol>> {
        self.runtime
            .block_on(self.inner.get_document_symbols(language, file_path))
    }

    pub fn open_document(&self, language: &str, file_path: &Path) -> LspResult<()> {
        self.runtime
            .block_on(self.inner.open_document(language, file_path))
    }

    pub fn close_document(&self, language: &str, file_path: &Path) -> LspResult<()> {
        self.runtime
            .block_on(self.inner.close_document(language, file_path))
    }

    pub fn shutdown(&self, language: &str) -> LspResult<()> {
        self.runtime.block_on(self.inner.shutdown(language))
    }

    pub fn shutdown_all(&self) -> LspResult<()> {
        self.runtime.block_on(self.inner.shutdown_all())
    }

    pub fn is_connected(&self, language: &str) -> bool {
        self.runtime.block_on(self.inner.is_connected(language))
    }

    pub fn get_workspace_symbols(
        &self,
        language: &str,
        query: &str,
    ) -> LspResult<Vec<SymbolInformation>> {
        self.runtime
            .block_on(self.inner.get_workspace_symbols(language, query))
    }

    pub fn get_code_actions(
        &self,
        language: &str,
        file_path: &Path,
        range: Range,
        diagnostics: Vec<Diagnostic>,
        only: Option<Vec<CodeActionKind>>,
    ) -> LspResult<Vec<CodeAction>> {
        self.runtime.block_on(self.inner.get_code_actions(
            language,
            file_path,
            range,
            diagnostics,
            only,
        ))
    }

    pub fn get_signature_help(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Option<SignatureHelp>> {
        self.runtime
            .block_on(self.inner.get_signature_help(language, file_path, position))
    }

    pub fn get_document_highlights(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<DocumentHighlight>> {
        self.runtime.block_on(
            self.inner
                .get_document_highlights(language, file_path, position),
        )
    }

    pub fn format_document(
        &self,
        language: &str,
        file_path: &Path,
        tab_size: u32,
        insert_spaces: bool,
    ) -> LspResult<Vec<TextEdit>> {
        self.runtime.block_on(self.inner.format_document(
            language,
            file_path,
            tab_size,
            insert_spaces,
        ))
    }

    pub fn get_completions(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<CompletionItem>> {
        self.runtime
            .block_on(self.inner.get_completions(language, file_path, position))
    }

    pub fn go_to_implementation(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Vec<Location>> {
        self.runtime.block_on(
            self.inner
                .go_to_implementation(language, file_path, position),
        )
    }

    pub fn go_to_type_definition(
        &self,
        language: &str,
        file_path: &Path,
        position: Position,
    ) -> LspResult<Option<Location>> {
        self.runtime.block_on(
            self.inner
                .go_to_type_definition(language, file_path, position),
        )
    }
}

impl Default for SyncLspClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速跳转到定义（自动检测语言并初始化服务器）
pub async fn quick_go_to_definition(
    file_path: &Path,
    position: Position,
) -> LspResult<Vec<Location>> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| LspError::RequestFailed("Unknown file extension".to_string()))?;

    let language = LanguageServerManager::get_language_from_extension(ext)
        .ok_or_else(|| LspError::ServerNotFound(format!("No LSP server for extension: {}", ext)))?;

    let client = LspClient::new();
    let root_path = file_path.parent().unwrap_or(Path::new("."));

    client.initialize(language, root_path).await?;
    let result = client.go_to_definition(language, file_path, position).await;
    client.shutdown(language).await?;

    result
}

/// 快速查找引用
pub async fn quick_find_references(
    file_path: &Path,
    position: Position,
    include_declaration: bool,
) -> LspResult<Vec<Location>> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| LspError::RequestFailed("Unknown file extension".to_string()))?;

    let language = LanguageServerManager::get_language_from_extension(ext)
        .ok_or_else(|| LspError::ServerNotFound(format!("No LSP server for extension: {}", ext)))?;

    let client = LspClient::new();
    let root_path = file_path.parent().unwrap_or(Path::new("."));

    client.initialize(language, root_path).await?;
    let result = client
        .find_references(language, file_path, position, include_declaration)
        .await;
    client.shutdown(language).await?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = LspClient::new();
        assert!(client.manager().get_config("rust").is_some());
    }

    #[test]
    fn test_sync_client_creation() {
        let client = SyncLspClient::new();
        assert!(!client.is_connected("rust"));
    }

    #[test]
    fn test_request_id_increment() {
        let client = LspClient::new();
        let id1 = client.next_id();
        let id2 = client.next_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_path_to_uri() {
        let client = LspClient::new();
        let path = Path::new("/home/user/test.rs");
        let uri = client.path_to_uri(path);
        assert!(uri.starts_with("file://"));
    }

    #[test]
    fn test_connection_state() {
        use tokio::runtime::Runtime;
        let rt = Runtime::new().unwrap();
        let client = LspClient::new();

        rt.block_on(async {
            let state = client.get_state("rust").await;
            assert_eq!(state, ConnectionState::Disconnected);
        });
    }
}
