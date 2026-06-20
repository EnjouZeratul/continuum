//! # Code Analysis Tools
//!
//! 代码分析工具集：基于 LSP 的符号查找工具。
//!
//! ## 版本说明
//!
//! **当前版本**: v2 - LSP 集成版
//!
//! ### 能力范围
//! - 查找函数、类、结构体、变量定义（跨模块）
//! - 查找符号引用（项目级）
//! - 获取类型信息和文档
//! - 重命名符号（重构）
//! - 多语言支持（Rust, Python, JavaScript/TypeScript, Java, Go, C/C++）
//! - LSP 服务器自动管理
//!
//! ### 后备策略
//! - 当 LSP 服务器不可用时，自动回退到正则匹配
//! - 确保在任何环境下都能提供基本功能

use crate::builtin_tools::path_safety::check_path_danger;
use crate::builtin_tools::BuiltinTool;
use crate::lsp::{LspClient, Position};
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

/// Validate file path for code analysis tools — canonicalize + critical-path
/// check + size limit. Prevents path traversal and reading sensitive files
/// (aligned with OWASP LLM Top 10 "tool input validation" + Hermes CVE-2026-7396 lesson).
fn validate_code_file(path_str: &str) -> Layer3Result<PathBuf> {
    let canonical = fs::canonicalize(path_str)
        .map_err(|e| anyhow::anyhow!("File not accessible '{}': {}", path_str, e))?;
    let danger = check_path_danger(&canonical);
    if danger.is_critical {
        return Err(anyhow::anyhow!(
            "code tool rejected: path '{}' is critical ({})",
            canonical.display(),
            danger.reason
        ));
    }
    let meta = fs::metadata(&canonical)?;
    if meta.len() > 10 * 1024 * 1024 {
        return Err(anyhow::anyhow!(
            "code tool rejected: file too large ({} bytes > 10 MiB limit)",
            meta.len()
        ));
    }
    Ok(canonical)
}

/// 工具版本标识
pub const CODE_ANALYSIS_VERSION: &str = "v2-lsp-integrated";

/// 全局 LSP 客户端（懒加载）
static GLOBAL_LSP_CLIENT: OnceCell<Arc<Mutex<LspClient>>> = OnceCell::const_new();

/// 获取或创建全局 LSP 客户端
async fn get_lsp_client() -> Arc<Mutex<LspClient>> {
    GLOBAL_LSP_CLIENT
        .get_or_init(|| async { Arc::new(Mutex::new(LspClient::new())) })
        .await
        .clone()
}

// ============================================================================
// Go to Definition Tool
// ============================================================================

/// Go to Definition Tool (LSP 版)
///
/// 使用 LSP 服务器查找符号定义位置，支持跨模块解析。
///
/// **优先级**:
/// 1. 尝试使用 LSP 服务器（支持跨模块）
/// 2. 回退到正则匹配（仅同目录）
pub struct GoToDefinitionTool;

#[async_trait]
impl BuiltinTool for GoToDefinitionTool {
    fn name(&self) -> &str {
        "go_to_definition"
    }

    fn description(&self) -> &str {
        "Find the definition of a symbol at a given location. \
         [LSP VERSION] Uses Language Server Protocol for cross-module resolution. \
         Automatically falls back to regex matching when LSP server is unavailable."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "The file path"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (1-based)"
                },
                "column": {
                    "type": "integer",
                    "description": "Column number (1-based)"
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional: the symbol name to search for"
                }
            },
            "required": ["file", "line", "column"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::CodeAnalysis
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let file_path = args["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing file parameter"))?;

        let line = args["line"].as_u64().unwrap_or(1) as usize;
        let column = args["column"].as_u64().unwrap_or(1) as usize;
        let symbol = args["symbol"].as_str();

        let path = validate_code_file(file_path)?;

        // 尝试使用 LSP
        match self.execute_with_lsp(&path, line, column).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::debug!("LSP failed, falling back to regex: {}", e);
            }
        }

        // 回退到正则匹配
        self.execute_with_regex(&path, line, column, symbol).await
    }
}

impl GoToDefinitionTool {
    /// 使用 LSP 查找定义
    async fn execute_with_lsp(
        &self,
        file_path: &Path,
        line: usize,
        column: usize,
    ) -> Layer3Result<String> {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow::anyhow!("Unknown file extension"))?;

        let language = crate::lsp::server::LanguageServerManager::get_language_from_extension(ext)
            .ok_or_else(|| anyhow::anyhow!("No LSP server for extension: {}", ext))?;

        let client = get_lsp_client().await;
        let client = client.lock().await;

        // 初始化服务器（如果未初始化）
        let root_path = file_path.parent().unwrap_or(Path::new("."));

        if !client.is_connected(language).await {
            client
                .initialize(language, root_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize LSP server: {}", e))?;
        }

        // 打开文档
        client
            .open_document(language, file_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open document: {}", e))?;

        // LSP 使用 0-based 行列
        let position = Position::new((line - 1) as u32, (column - 1) as u32);

        // 查找定义
        let locations = client
            .go_to_definition(language, file_path, position)
            .await
            .map_err(|e| anyhow::anyhow!("LSP request failed: {}", e))?;

        if locations.is_empty() {
            return Err(anyhow::anyhow!("No definition found via LSP"));
        }

        // 格式化输出
        let mut results = Vec::new();
        for loc in locations {
            let loc_path = loc
                .uri
                .strip_prefix("file://")
                .unwrap_or(&loc.uri)
                .strip_prefix('/')
                .unwrap_or(&loc.uri);
            results.push(format!(
                "Definition found in {} at line {}, column {}:\n  [LSP cross-module result]",
                loc_path,
                loc.range.start.line + 1,
                loc.range.start.character + 1
            ));
        }

        Ok(results.join("\n"))
    }

    /// 使用正则回退查找
    async fn execute_with_regex(
        &self,
        file_path: &Path,
        line: usize,
        column: usize,
        symbol: Option<&str>,
    ) -> Layer3Result<String> {
        // 读取文件
        let content = fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {:?}: {}", file_path, e))?;

        let lines: Vec<&str> = content.lines().collect();

        // 获取当前行的符号
        let current_line = lines.get(line - 1).copied().unwrap_or("");
        let target_symbol = symbol
            .map(|s| s.to_string())
            .unwrap_or_else(|| extract_symbol_at_position(current_line, column));

        if target_symbol.is_empty() {
            return Ok("No symbol found at specified location".to_string());
        }

        // 根据文件类型确定定义模式
        let file_ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let definition_patterns = get_definition_patterns(file_ext, &target_symbol);

        // 搜索定义
        for pattern_str in definition_patterns {
            let pattern =
                Regex::new(&pattern_str).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;

            // 先在当前文件中搜索
            for (line_num, line_content) in lines.iter().enumerate() {
                if pattern.is_match(line_content) {
                    let match_info = pattern.find(line_content).unwrap();
                    return Ok(format!(
                        "Definition found in {} at line {}, column {}:\n{}",
                        file_path.display(),
                        line_num + 1,
                        match_info.start() + 1,
                        line_content.trim()
                    ));
                }
            }

            // 如果当前文件没找到，搜索同目录的其他文件
            let dir = file_path.parent().unwrap_or(Path::new("."));
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_file() && entry_path != *file_path {
                        let ext = entry_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("");
                        if ext == file_ext {
                            if let Ok(other_content) = fs::read_to_string(&entry_path) {
                                for (line_num, line_content) in other_content.lines().enumerate() {
                                    if pattern.is_match(line_content) {
                                        return Ok(format!(
                                            "Definition found in {} at line {}:\n{}",
                                            entry_path.display(),
                                            line_num + 1,
                                            line_content.trim()
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(format!("No definition found for symbol: {}", target_symbol))
    }
}

// ============================================================================
// Find References Tool
// ============================================================================

/// Find References Tool (LSP 版)
///
/// 使用 LSP 服务器查找符号的所有引用位置，支持项目级搜索。
pub struct FindReferencesTool;

#[async_trait]
impl BuiltinTool for FindReferencesTool {
    fn name(&self) -> &str {
        "find_references"
    }

    fn description(&self) -> &str {
        "Find all references to a symbol at a given location. \
         [LSP VERSION] Uses Language Server Protocol for project-wide search. \
         Automatically falls back to regex matching when LSP server is unavailable."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "The file path"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (1-based)"
                },
                "column": {
                    "type": "integer",
                    "description": "Column number (1-based)"
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional: the symbol name to search for"
                },
                "include_declaration": {
                    "type": "boolean",
                    "description": "Include declaration in results (default: true)"
                }
            },
            "required": ["file", "line", "column"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::CodeAnalysis
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let file_path = args["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing file parameter"))?;

        let line = args["line"].as_u64().unwrap_or(1) as usize;
        let column = args["column"].as_u64().unwrap_or(1) as usize;
        let symbol = args["symbol"].as_str();
        let include_declaration = args["include_declaration"].as_bool().unwrap_or(true);

        let path = validate_code_file(file_path)?;

        // 尝试使用 LSP
        match self
            .execute_with_lsp(&path, line, column, include_declaration)
            .await
        {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::debug!("LSP failed, falling back to regex: {}", e);
            }
        }

        // 回退到正则匹配
        self.execute_with_regex(&path, line, column, symbol, include_declaration)
            .await
    }
}

impl FindReferencesTool {
    /// 使用 LSP 查找引用
    async fn execute_with_lsp(
        &self,
        file_path: &Path,
        line: usize,
        column: usize,
        include_declaration: bool,
    ) -> Layer3Result<String> {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow::anyhow!("Unknown file extension"))?;

        let language = crate::lsp::server::LanguageServerManager::get_language_from_extension(ext)
            .ok_or_else(|| anyhow::anyhow!("No LSP server for extension: {}", ext))?;

        let client = get_lsp_client().await;
        let client = client.lock().await;

        let root_path = file_path.parent().unwrap_or(Path::new("."));

        if !client.is_connected(language).await {
            client
                .initialize(language, root_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize LSP server: {}", e))?;
        }

        client
            .open_document(language, file_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open document: {}", e))?;

        let position = Position::new((line - 1) as u32, (column - 1) as u32);

        let locations = client
            .find_references(language, file_path, position, include_declaration)
            .await
            .map_err(|e| anyhow::anyhow!("LSP request failed: {}", e))?;

        if locations.is_empty() {
            return Err(anyhow::anyhow!("No references found via LSP"));
        }

        let mut results = Vec::new();
        for loc in locations {
            let loc_path = loc
                .uri
                .strip_prefix("file://")
                .unwrap_or(&loc.uri)
                .strip_prefix('/')
                .unwrap_or(&loc.uri);
            results.push(format!(
                "{}:{}:{}",
                loc_path,
                loc.range.start.line + 1,
                loc.range.start.character + 1
            ));
        }

        Ok(format!(
            "Found {} references (LSP project-wide search):\n{}",
            results.len(),
            results.join("\n")
        ))
    }

    /// 使用正则回退查找
    async fn execute_with_regex(
        &self,
        file_path: &Path,
        line: usize,
        column: usize,
        symbol: Option<&str>,
        include_declaration: bool,
    ) -> Layer3Result<String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {:?}: {}", file_path, e))?;

        let lines: Vec<&str> = content.lines().collect();
        let current_line = lines.get(line - 1).copied().unwrap_or("");

        let target_symbol = symbol
            .map(|s| s.to_string())
            .unwrap_or_else(|| extract_symbol_at_position(current_line, column));

        if target_symbol.is_empty() {
            return Ok("No symbol found at specified location".to_string());
        }

        let reference_pattern = Regex::new(&format!(r"\b{}\b", target_symbol))
            .map_err(|e| anyhow::anyhow!("Invalid regex for symbol: {}", e))?;

        let mut results = Vec::new();

        // 搜索当前文件
        for (line_num, line_content) in lines.iter().enumerate() {
            if reference_pattern.is_match(line_content) {
                let is_declaration = is_definition_line(line_content, &target_symbol);
                if include_declaration || !is_declaration {
                    let matches: Vec<_> = reference_pattern.find_iter(line_content).collect();
                    for m in matches {
                        results.push(format!(
                            "{}:{}:{} - {}",
                            file_path.display(),
                            line_num + 1,
                            m.start() + 1,
                            line_content.trim()
                        ));
                    }
                }
            }
        }

        // 搜索同目录的其他文件
        let dir = file_path.parent().unwrap_or(Path::new("."));
        let file_ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_file() && entry_path != *file_path {
                    let ext = entry_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ext == file_ext {
                        if let Ok(other_content) = fs::read_to_string(&entry_path) {
                            for (line_num, line_content) in other_content.lines().enumerate() {
                                if reference_pattern.is_match(line_content) {
                                    let is_decl = is_definition_line(line_content, &target_symbol);
                                    if include_declaration || !is_decl {
                                        let matches: Vec<_> =
                                            reference_pattern.find_iter(line_content).collect();
                                        for m in matches {
                                            results.push(format!(
                                                "{}:{}:{} - {}",
                                                entry_path.display(),
                                                line_num + 1,
                                                m.start() + 1,
                                                line_content.trim()
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(format!("No references found for symbol: {}", target_symbol))
        } else {
            Ok(format!(
                "Found {} references:\n{}",
                results.len(),
                results.join("\n")
            ))
        }
    }
}

// ============================================================================
// Get Hover Tool
// ============================================================================

/// Get Hover Tool (LSP 版)
///
/// 获取符号的类型信息和文档。
pub struct GetHoverTool;

#[async_trait]
impl BuiltinTool for GetHoverTool {
    fn name(&self) -> &str {
        "get_hover"
    }

    fn description(&self) -> &str {
        "Get type information and documentation for a symbol at a given location. \
         [LSP VERSION] Uses Language Server Protocol for accurate type information."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "The file path"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (1-based)"
                },
                "column": {
                    "type": "integer",
                    "description": "Column number (1-based)"
                }
            },
            "required": ["file", "line", "column"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::CodeAnalysis
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let file_path = args["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing file parameter"))?;

        let line = args["line"].as_u64().unwrap_or(1) as usize;
        let column = args["column"].as_u64().unwrap_or(1) as usize;

        let path = validate_code_file(file_path)?;

        // 尝试使用 LSP
        match self.execute_with_lsp(&path, line, column).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::debug!("LSP failed, falling back to basic info: {}", e);
            }
        }

        // 回退到基本信息
        self.execute_basic(&path, line, column).await
    }
}

impl GetHoverTool {
    async fn execute_with_lsp(
        &self,
        file_path: &Path,
        line: usize,
        column: usize,
    ) -> Layer3Result<String> {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow::anyhow!("Unknown file extension"))?;

        let language = crate::lsp::server::LanguageServerManager::get_language_from_extension(ext)
            .ok_or_else(|| anyhow::anyhow!("No LSP server for extension: {}", ext))?;

        let client = get_lsp_client().await;
        let client = client.lock().await;

        let root_path = file_path.parent().unwrap_or(Path::new("."));

        if !client.is_connected(language).await {
            client
                .initialize(language, root_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize LSP server: {}", e))?;
        }

        client
            .open_document(language, file_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open document: {}", e))?;

        let position = Position::new((line - 1) as u32, (column - 1) as u32);

        let hover = client
            .get_hover(language, file_path, position)
            .await
            .map_err(|e| anyhow::anyhow!("LSP request failed: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No hover information available"))?;

        // 格式化 hover 内容
        let content = match hover.contents {
            crate::lsp::HoverContents::Markup(markup) => {
                format!(
                    "```{}\n{}\n```",
                    match markup.kind {
                        crate::lsp::MarkupKind::PlainText => "",
                        crate::lsp::MarkupKind::Markdown => "markdown",
                    },
                    markup.value
                )
            }
            crate::lsp::HoverContents::String(s) => s,
            crate::lsp::HoverContents::Array(arr) => arr
                .iter()
                .map(|item| match item {
                    crate::lsp::MarkedString::String(s) => s.clone(),
                    crate::lsp::MarkedString::LanguageString(ls) => {
                        format!("```{}\n{}\n```", ls.language, ls.value)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        };

        Ok(content)
    }

    async fn execute_basic(
        &self,
        file_path: &Path,
        line: usize,
        column: usize,
    ) -> Layer3Result<String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

        let lines: Vec<&str> = content.lines().collect();
        let current_line = lines.get(line - 1).copied().unwrap_or("");

        let symbol = extract_symbol_at_position(current_line, column);

        if symbol.is_empty() {
            return Ok(format!("Line {}: {}", line, current_line.trim()));
        }

        Ok(format!(
            "**{}**\n\n```\n{}\n```",
            symbol,
            current_line.trim()
        ))
    }
}

// ============================================================================
// Rename Symbol Tool
// ============================================================================

/// Rename Symbol Tool (LSP 版)
///
/// 重命名符号，支持跨文件重构。
pub struct RenameSymbolTool;

#[async_trait]
impl BuiltinTool for RenameSymbolTool {
    fn name(&self) -> &str {
        "rename_symbol"
    }

    fn description(&self) -> &str {
        "Rename a symbol across the entire project. \
         [LSP VERSION] Uses Language Server Protocol for project-wide refactoring."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "The file path"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (1-based)"
                },
                "column": {
                    "type": "integer",
                    "description": "Column number (1-based)"
                },
                "new_name": {
                    "type": "string",
                    "description": "The new name for the symbol"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, only preview changes without applying them (default: true)"
                }
            },
            "required": ["file", "line", "column", "new_name"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::CodeAnalysis
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let file_path = args["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing file parameter"))?;

        let line = args["line"].as_u64().unwrap_or(1) as usize;
        let column = args["column"].as_u64().unwrap_or(1) as usize;
        let new_name = args["new_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing new_name parameter"))?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(true);

        let path = validate_code_file(file_path)?;

        // 重命名必须使用 LSP
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow::anyhow!("Unknown file extension"))?;

        let language = crate::lsp::server::LanguageServerManager::get_language_from_extension(ext)
            .ok_or_else(|| anyhow::anyhow!("No LSP server for extension: {}", ext))?;

        let client = get_lsp_client().await;
        let client = client.lock().await;

        let root_path = path.parent().unwrap_or(Path::new("."));

        if !client.is_connected(language).await {
            client
                .initialize(language, root_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize LSP server: {}", e))?;
        }

        client
            .open_document(language, &path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open document: {}", e))?;

        let position = Position::new((line - 1) as u32, (column - 1) as u32);

        let workspace_edit = client
            .rename_symbol(language, &path, position, new_name)
            .await
            .map_err(|e| anyhow::anyhow!("LSP rename request failed: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("Cannot rename symbol at this location"))?;

        // 格式化变更
        let mut changes = Vec::new();

        if let Some(ref change_map) = workspace_edit.changes {
            for (uri, edits) in change_map {
                let file = uri
                    .strip_prefix("file://")
                    .unwrap_or(uri.as_str())
                    .strip_prefix('/')
                    .unwrap_or(uri.as_str());
                for edit in edits {
                    changes.push(format!(
                        "{}:{}:{} -> {}",
                        file,
                        edit.range.start.line + 1,
                        edit.range.start.character + 1,
                        edit.new_text
                    ));
                }
            }
        }

        if dry_run {
            Ok(format!(
                "Preview: {} changes would be made to rename symbol to '{}':\n{}",
                changes.len(),
                new_name,
                changes.join("\n")
            ))
        } else {
            // 应用变更
            self.apply_changes(&workspace_edit)?;
            Ok(format!(
                "Successfully renamed symbol to '{}' ({} changes applied)",
                new_name,
                changes.len()
            ))
        }
    }
}

impl RenameSymbolTool {
    fn apply_changes(&self, workspace_edit: &crate::lsp::WorkspaceEdit) -> Layer3Result<()> {
        if let Some(changes) = &workspace_edit.changes {
            for (uri, edits) in changes {
                let file_path = uri
                    .strip_prefix("file://")
                    .unwrap_or(uri.as_str())
                    .strip_prefix('/')
                    .unwrap_or(uri.as_str());

                let file_path = if cfg!(windows) {
                    file_path.replace('/', "\\")
                } else {
                    file_path.to_string()
                };

                let content = fs::read_to_string(&file_path)?;
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

                // 按位置倒序排列，以便从后往前修改
                let mut sorted_edits = edits.clone();
                sorted_edits.sort_by(|a, b| {
                    b.range
                        .start
                        .line
                        .cmp(&a.range.start.line)
                        .then(b.range.start.character.cmp(&a.range.start.character))
                });

                for edit in sorted_edits {
                    let line_idx = edit.range.start.line as usize;
                    if line_idx < lines.len() {
                        let line = &mut lines[line_idx];
                        let start = edit.range.start.character as usize;
                        let end = edit.range.end.character as usize;

                        if end <= line.len() {
                            line.replace_range(start..end, &edit.new_text);
                        }
                    }
                }

                fs::write(&file_path, lines.join("\n"))?;
            }
        }

        Ok(())
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 从指定位置提取符号名
fn extract_symbol_at_position(line: &str, column: usize) -> String {
    let line_bytes = line.as_bytes();
    if column == 0 || column > line.len() {
        return String::new();
    }

    // 找到符号的起始位置
    let start = line_bytes[..column - 1]
        .iter()
        .rposition(|&b| !is_identifier_char(b))
        .map(|p| p + 1)
        .unwrap_or(0);

    // 找到符号的结束位置
    let end = line_bytes[column - 1..]
        .iter()
        .position(|&b| !is_identifier_char(b))
        .map(|p| column - 1 + p)
        .unwrap_or(line.len());

    line[start..end].to_string()
}

/// 检查是否是标识符字符
fn is_identifier_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':'
}

/// 检查是否是定义行
fn is_definition_line(line: &str, symbol: &str) -> bool {
    let patterns = [
        Regex::new(&format!(r"\bfn\s+{}\s*\(", symbol)).ok(),
        Regex::new(&format!(r"\bdef\s+{}\s*\(", symbol)).ok(),
        Regex::new(&format!(r"\bclass\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\bstruct\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\benum\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\bimpl\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\btrait\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\binterface\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\btype\s+{}\s*=", symbol)).ok(),
        Regex::new(&format!(r"\bconst\s+{}", symbol)).ok(),
        Regex::new(&format!(r"\blet\s+{}\s*=", symbol)).ok(),
        Regex::new(&format!(r"\bvar\s+{}\s*=", symbol)).ok(),
        Regex::new(&format!(r"\bpublic\s+{}\s*\(", symbol)).ok(),
        Regex::new(&format!(r"\bprivate\s+{}\s*\(", symbol)).ok(),
    ];

    patterns
        .iter()
        .any(|p| p.as_ref().is_some_and(|r| r.is_match(line)))
}

/// 根据文件类型获取定义匹配模式
fn get_definition_patterns(file_ext: &str, symbol: &str) -> Vec<String> {
    match file_ext {
        "rs" => vec![
            format!(r"\bfn\s+{}\s*[<(]", symbol),
            format!(r"\bstruct\s+{}\s*[{{<\s]", symbol),
            format!(r"\benum\s+{}\s*[{{<\s]", symbol),
            format!(r"\btrait\s+{}\s*[{{<\s]", symbol),
            format!(r"\bimpl\s+(?:\w+\s+for\s+)?{}|impl\s+{}", symbol, symbol),
            format!(r"\btype\s+{}\s*=", symbol),
            format!(r"\bconst\s+{}\s*:", symbol),
            format!(r"\bstatic\s+{}\s*:", symbol),
            format!(r"\bmacro_rules!\s+{}", symbol),
        ],
        "py" => vec![
            format!(r"\bdef\s+{}\s*\(", symbol),
            format!(r"\bclass\s+{}\s*[:\(]", symbol),
            format!(r"\basync\s+def\s+{}\s*\(", symbol),
        ],
        "js" | "ts" | "tsx" => vec![
            format!(r"\bfunction\s+{}\s*\(", symbol),
            format!(r"\bclass\s+{}\s*[{{extends\s]", symbol),
            format!(r"\bconst\s+{}\s*=", symbol),
            format!(r"\blet\s+{}\s*=", symbol),
            format!(r"\bvar\s+{}\s*=", symbol),
            format!(r"\binterface\s+{}\s*[{{extends\s]", symbol),
            format!(r"\btype\s+{}\s*=", symbol),
            format!(r"\bexport\s+(?:default\s+)?(?:function|class)\s+{}", symbol),
        ],
        "java" | "kt" => vec![
            format!(r"\bclass\s+{}\s*[{{extends\s]", symbol),
            format!(r"\binterface\s+{}\s*[{{extends\s]", symbol),
            format!(
                r"\b(?:public|private|protected)\s+(?:static\s+)?(?:\w+\s+)?{}\s*\(",
                symbol
            ),
            format!(r"\benum\s+{}\s*[{{]", symbol),
        ],
        "go" => vec![
            format!(r"\bfunc\s+{}\s*\(", symbol),
            format!(r"\bfunc\s+\(\w+\s*\*?\w*\)\s+{}\s*\(", symbol),
            format!(r"\btype\s+{}\s+struct", symbol),
            format!(r"\btype\s+{}\s+interface", symbol),
            format!(r"\bvar\s+{}\s*=", symbol),
            format!(r"\bconst\s+{}\s*=", symbol),
        ],
        "c" | "cpp" | "h" | "hpp" => vec![
            format!(
                r"\b(?:void|int|char|float|double|auto|struct|class)\s+{}\s*\(",
                symbol
            ),
            format!(r"\bstruct\s+{}\s*[{{]", symbol),
            format!(r"\bclass\s+{}\s*[{{:]", symbol),
            format!(r"\btypedef\s+.*\s+{}\s*;", symbol),
        ],
        _ => vec![
            format!(r"\b{}\s*[:=]", symbol),
            format!(r"\b{}\s*\(", symbol),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_goto_definition_category() {
        let tool = GoToDefinitionTool;
        assert_eq!(tool.category(), ToolCategory::CodeAnalysis);
    }

    #[test]
    fn test_find_references_category() {
        let tool = FindReferencesTool;
        assert_eq!(tool.category(), ToolCategory::CodeAnalysis);
    }

    #[test]
    fn test_get_hover_category() {
        let tool = GetHoverTool;
        assert_eq!(tool.category(), ToolCategory::CodeAnalysis);
    }

    #[test]
    fn test_rename_symbol_category() {
        let tool = RenameSymbolTool;
        assert_eq!(tool.category(), ToolCategory::CodeAnalysis);
        assert!(tool.requires_confirmation());
    }

    #[test]
    fn test_extract_symbol() {
        let line = "let my_variable = 42;";
        let symbol = extract_symbol_at_position(line, 10);
        assert_eq!(symbol, "my_variable");
    }

    #[test]
    fn test_extract_symbol_empty() {
        let line = "    = 42;";
        let symbol = extract_symbol_at_position(line, 5);
        assert_eq!(symbol, "");
    }

    #[test]
    fn test_is_definition_line() {
        assert!(is_definition_line("fn my_func() {", "my_func"));
        assert!(is_definition_line("struct MyStruct {", "MyStruct"));
        assert!(!is_definition_line("my_func();", "my_func"));
    }

    #[test]
    fn test_get_definition_patterns_rust() {
        let patterns = get_definition_patterns("rs", "foo");
        assert!(patterns.iter().any(|p| p.contains("fn")));
        assert!(patterns.iter().any(|p| p.contains("struct")));
    }

    #[tokio::test]
    async fn test_goto_definition_missing_file() {
        let tool = GoToDefinitionTool;
        let result = tool
            .execute(json!({"file": "nonexistent.rs", "line": 1, "column": 1}))
            .await;
        // Should fail due to missing file (regex fallback also fails)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_find_references_missing_file() {
        let tool = FindReferencesTool;
        let result = tool
            .execute(json!({"file": "nonexistent.rs", "line": 1, "column": 1}))
            .await;
        // Should fail due to missing file (regex fallback also fails)
        assert!(result.is_err());
    }
}
