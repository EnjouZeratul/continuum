//! # LSP Query Engine
//!
//! 基于 LSP 的代码查询引擎实现。

use crate::lsp::client::LspClient;
use crate::lsp::types::SymbolKind as LspSymbolKind;
use crate::lsp::types::{DocumentSymbol, Hover, HoverContents, Location, MarkedString, Position};
use crate::query_engine::{
    CodeAnalyzer, CodeMatch, CodeStructure, DetectedPattern, PatternType, QueryEngine, SymbolInfo,
    SymbolKind,
};
use crate::types::{CodeLocation, CodeRange, Layer3Result, QueryResult, QueryType};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// LSP 查询引擎
pub struct LspQueryEngine {
    client: Arc<LspClient>,
}

impl LspQueryEngine {
    /// 创建新的 LSP 查询引擎
    pub fn new() -> Self {
        Self {
            client: Arc::new(LspClient::new()),
        }
    }

    /// 使用现有的 LSP 客户端创建
    pub fn with_client(client: Arc<LspClient>) -> Self {
        Self { client }
    }

    /// 获取 LSP 客户端引用
    pub fn client(&self) -> &Arc<LspClient> {
        &self.client
    }

    /// 确保语言服务器已连接
    async fn ensure_connected(&self, file: &Path) -> Layer3Result<String> {
        let language = detect_language(file);

        if !self.client.is_connected(&language).await {
            self.client
                .initialize(&language, file.parent().unwrap_or(Path::new(".")))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize LSP server: {}", e))?;
        }

        Ok(language)
    }

    /// 将 LSP Location 转换为 QueryResult
    fn location_to_result(loc: &Location, query_type: QueryType) -> QueryResult {
        let file = PathBuf::from(loc.uri.replace("file://", ""));
        QueryResult {
            query_type,
            location: CodeLocation {
                file: file.clone(),
                line: loc.range.start.line,
                column: loc.range.start.character,
            },
            range: Some(CodeRange {
                start: CodeLocation {
                    file: file.clone(),
                    line: loc.range.start.line,
                    column: loc.range.start.character,
                },
                end: CodeLocation {
                    file,
                    line: loc.range.end.line,
                    column: loc.range.end.character,
                },
            }),
            display_text: String::new(),
            snippet: None,
        }
    }

    /// 将 DocumentSymbol 转换为 SymbolInfo
    fn doc_symbol_to_info(symbol: &DocumentSymbol, file: &Path) -> SymbolInfo {
        let file = file.to_path_buf();
        SymbolInfo {
            name: symbol.name.clone(),
            kind: lsp_kind_to_query_kind(symbol.kind),
            location: CodeLocation {
                file: file.clone(),
                line: symbol.range.start.line,
                column: symbol.range.start.character,
            },
            range: Some(CodeRange {
                start: CodeLocation {
                    file: file.clone(),
                    line: symbol.range.start.line,
                    column: symbol.range.start.character,
                },
                end: CodeLocation {
                    file,
                    line: symbol.range.end.line,
                    column: symbol.range.end.character,
                },
            }),
            container_name: None,
        }
    }
}

impl Default for LspQueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueryEngine for LspQueryEngine {
    async fn go_to_definition(&self, location: CodeLocation) -> Layer3Result<Option<QueryResult>> {
        let language = self.ensure_connected(&location.file).await?;

        let result = self
            .client
            .go_to_definition(
                &language,
                &location.file,
                Position::new(location.line, location.column),
            )
            .await
            .map_err(|e| anyhow::anyhow!("LSP go_to_definition failed: {}", e))?;

        Ok(result
            .first()
            .map(|loc| Self::location_to_result(loc, QueryType::Definition)))
    }

    async fn find_references(&self, location: CodeLocation) -> Layer3Result<Vec<QueryResult>> {
        let language = self.ensure_connected(&location.file).await?;

        let result = self
            .client
            .find_references(
                &language,
                &location.file,
                Position::new(location.line, location.column),
                true,
            )
            .await
            .map_err(|e| anyhow::anyhow!("LSP find_references failed: {}", e))?;

        Ok(result
            .iter()
            .map(|loc| Self::location_to_result(loc, QueryType::References))
            .collect())
    }

    async fn go_to_implementation(&self, location: CodeLocation) -> Layer3Result<Vec<QueryResult>> {
        let language = self.ensure_connected(&location.file).await?;

        let result = self
            .client
            .go_to_implementation(
                &language,
                &location.file,
                Position::new(location.line, location.column),
            )
            .await
            .map_err(|e| anyhow::anyhow!("LSP go_to_implementation failed: {}", e))?;

        Ok(result
            .iter()
            .map(|loc| Self::location_to_result(loc, QueryType::Implementations))
            .collect())
    }

    async fn go_to_type_definition(
        &self,
        location: CodeLocation,
    ) -> Layer3Result<Option<QueryResult>> {
        let language = self.ensure_connected(&location.file).await?;

        let result = self
            .client
            .go_to_type_definition(
                &language,
                &location.file,
                Position::new(location.line, location.column),
            )
            .await
            .map_err(|e| anyhow::anyhow!("LSP go_to_type_definition failed: {}", e))?;

        Ok(result.map(|loc| Self::location_to_result(&loc, QueryType::TypeDefinition)))
    }

    async fn hover(&self, location: CodeLocation) -> Layer3Result<Option<String>> {
        let language = self.ensure_connected(&location.file).await?;

        let result = self
            .client
            .get_hover(
                &language,
                &location.file,
                Position::new(location.line, location.column),
            )
            .await
            .map_err(|e| anyhow::anyhow!("LSP hover failed: {}", e))?;

        match result {
            Some(Hover {
                contents: HoverContents::Markup(markup),
                ..
            }) => Ok(Some(markup.value)),
            Some(Hover {
                contents: HoverContents::String(s),
                ..
            }) => Ok(Some(s)),
            Some(Hover {
                contents: HoverContents::Array(arr),
                ..
            }) => Ok(Some(
                arr.iter()
                    .map(|ms| match ms {
                        MarkedString::String(s) => s.clone(),
                        MarkedString::LanguageString(ls) => {
                            format!("```{}\n{}\n```", ls.language, ls.value)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            None => Ok(None),
        }
    }

    async fn document_symbols(&self, file: PathBuf) -> Layer3Result<Vec<SymbolInfo>> {
        let language = self.ensure_connected(&file).await?;

        let result = self
            .client
            .get_document_symbols(&language, &file)
            .await
            .map_err(|e| anyhow::anyhow!("LSP document_symbols failed: {}", e))?;

        Ok(result
            .iter()
            .map(|s| Self::doc_symbol_to_info(s, &file))
            .collect())
    }

    async fn workspace_symbols(&self, query: &str) -> Layer3Result<Vec<SymbolInfo>> {
        // Try to get workspace symbols from any connected language server
        let languages = vec!["rust", "python", "typescript", "go", "java"];

        for language in languages {
            if self.client.is_connected(language).await {
                let result = self
                    .client
                    .get_workspace_symbols(language, query)
                    .await
                    .map_err(|e| anyhow::anyhow!("LSP workspace_symbols failed: {}", e))?;

                return Ok(result
                    .iter()
                    .map(|s| SymbolInfo {
                        name: s.name.clone(),
                        kind: lsp_kind_to_query_kind(s.kind),
                        location: CodeLocation {
                            file: PathBuf::from(s.location.uri.replace("file://", "")),
                            line: s.location.range.start.line,
                            column: s.location.range.start.character,
                        },
                        range: Some(CodeRange {
                            start: CodeLocation {
                                file: PathBuf::from(s.location.uri.replace("file://", "")),
                                line: s.location.range.start.line,
                                column: s.location.range.start.character,
                            },
                            end: CodeLocation {
                                file: PathBuf::from(s.location.uri.replace("file://", "")),
                                line: s.location.range.end.line,
                                column: s.location.range.end.character,
                            },
                        }),
                        container_name: s.container_name.clone(),
                    })
                    .collect());
            }
        }

        Ok(Vec::new())
    }

    async fn query(
        &self,
        query_type: QueryType,
        location: CodeLocation,
    ) -> Layer3Result<Vec<QueryResult>> {
        match query_type {
            QueryType::Definition => {
                let result = self.go_to_definition(location).await?;
                Ok(result.map(|r| vec![r]).unwrap_or_default())
            }
            QueryType::References => self.find_references(location).await,
            QueryType::Implementations => self.go_to_implementation(location).await,
            QueryType::TypeDefinition => {
                let result = self.go_to_type_definition(location).await?;
                Ok(result.map(|r| vec![r]).unwrap_or_default())
            }
            QueryType::DocumentSymbols => {
                let symbols = self.document_symbols(location.file.clone()).await?;
                Ok(symbols
                    .iter()
                    .map(|s| QueryResult {
                        query_type: QueryType::DocumentSymbols,
                        location: s.location.clone(),
                        range: s.range.clone(),
                        display_text: s.name.clone(),
                        snippet: None,
                    })
                    .collect())
            }
            QueryType::WorkspaceSymbols => {
                let symbols = self
                    .workspace_symbols(&location.file.to_string_lossy())
                    .await?;
                Ok(symbols
                    .iter()
                    .map(|s| QueryResult {
                        query_type: QueryType::WorkspaceSymbols,
                        location: s.location.clone(),
                        range: s.range.clone(),
                        display_text: s.name.clone(),
                        snippet: None,
                    })
                    .collect())
            }
            QueryType::Hover => {
                let result = self.hover(location.clone()).await?;
                Ok(result
                    .map(|content| {
                        vec![QueryResult {
                            query_type: QueryType::Hover,
                            location,
                            range: None,
                            display_text: content,
                            snippet: None,
                        }]
                    })
                    .unwrap_or_default())
            }
        }
    }
}

/// 根据 LSP SymbolKind 转换为查询引擎 SymbolKind
fn lsp_kind_to_query_kind(kind: LspSymbolKind) -> SymbolKind {
    match kind {
        LspSymbolKind::File => SymbolKind::File,
        LspSymbolKind::Module => SymbolKind::Module,
        LspSymbolKind::Namespace => SymbolKind::Namespace,
        LspSymbolKind::Package => SymbolKind::Package,
        LspSymbolKind::Class => SymbolKind::Class,
        LspSymbolKind::Method => SymbolKind::Method,
        LspSymbolKind::Property => SymbolKind::Property,
        LspSymbolKind::Field => SymbolKind::Field,
        LspSymbolKind::Constructor => SymbolKind::Constructor,
        LspSymbolKind::Enum => SymbolKind::Enum,
        LspSymbolKind::Interface => SymbolKind::Interface,
        LspSymbolKind::Function => SymbolKind::Function,
        LspSymbolKind::Variable => SymbolKind::Variable,
        LspSymbolKind::Constant => SymbolKind::Constant,
        LspSymbolKind::String => SymbolKind::String,
        LspSymbolKind::Number => SymbolKind::Number,
        LspSymbolKind::Boolean => SymbolKind::Boolean,
        LspSymbolKind::Array => SymbolKind::Array,
        LspSymbolKind::Object => SymbolKind::Object,
        LspSymbolKind::Key => SymbolKind::Key,
        LspSymbolKind::Null => SymbolKind::Null,
        LspSymbolKind::EnumMember => SymbolKind::EnumMember,
        LspSymbolKind::Struct => SymbolKind::Struct,
        LspSymbolKind::Event => SymbolKind::Event,
        LspSymbolKind::Operator => SymbolKind::Operator,
        LspSymbolKind::TypeParameter => SymbolKind::TypeParameter,
    }
}

/// 根据文件扩展名检测语言
fn detect_language(file: &Path) -> String {
    match file.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") => "cpp",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("md") => "markdown",
        Some("html") => "html",
        Some("css") => "css",
        Some("sql") => "sql",
        _ => "text",
    }
    .to_string()
}

/// LSP 代码分析器
pub struct LspCodeAnalyzer {
    engine: Arc<LspQueryEngine>,
}

impl LspCodeAnalyzer {
    /// 创建新的代码分析器
    pub fn new(engine: Arc<LspQueryEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl CodeAnalyzer for LspCodeAnalyzer {
    async fn analyze_structure(&self, file: PathBuf) -> Layer3Result<CodeStructure> {
        let symbols = self.engine.document_symbols(file.clone()).await?;

        let imports = symbols
            .iter()
            .filter(|s| matches!(s.kind, SymbolKind::Module | SymbolKind::Namespace))
            .map(|s| s.name.clone())
            .collect();

        let exports = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function || s.kind == SymbolKind::Class)
            .map(|s| s.name.clone())
            .collect();

        let lines = std::fs::read_to_string(&file)
            .map(|content| content.lines().count())
            .unwrap_or(0);

        Ok(CodeStructure {
            file,
            imports,
            exports,
            symbols,
            lines,
        })
    }

    async fn find_similar(&self, snippet: &str, threshold: f32) -> Layer3Result<Vec<CodeMatch>> {
        let mut matches = Vec::new();

        let symbols = self.engine.workspace_symbols(snippet).await?;

        for symbol in symbols {
            let similarity = calculate_similarity(snippet, &symbol.name);
            if similarity >= threshold {
                matches.push(CodeMatch {
                    location: symbol.location,
                    content: symbol.name,
                    similarity,
                });
            }
        }

        Ok(matches)
    }

    async fn detect_patterns(&self, file: PathBuf) -> Layer3Result<Vec<DetectedPattern>> {
        let symbols = self.engine.document_symbols(file.clone()).await?;

        let mut patterns = Vec::new();

        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        if classes.len() == 1 {
            patterns.push(DetectedPattern {
                name: "Potential Singleton".to_string(),
                pattern_type: PatternType::DesignPattern,
                locations: classes.iter().map(|c| c.location.clone()).collect(),
            });
        }

        for symbol in &symbols {
            if symbol.kind == SymbolKind::Function {
                if let Some(range) = &symbol.range {
                    let lines = range.end.line - range.start.line;
                    if lines > 50 {
                        patterns.push(DetectedPattern {
                            name: format!("Long Function ({})", symbol.name),
                            pattern_type: PatternType::AntiPattern,
                            locations: vec![symbol.location.clone()],
                        });
                    }
                }
            }
        }

        Ok(patterns)
    }
}

/// 计算字符串相似度 (Jaccard-based)
fn calculate_similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }

    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let words_a: Vec<_> = a.split_whitespace().collect();
    let words_b: Vec<_> = b.split_whitespace().collect();

    let intersection = words_a.iter().filter(|w| words_b.contains(w)).count();
    let union = words_a.len() + words_b.len() - intersection;

    if union == 0 {
        return 0.0;
    }

    intersection as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(&PathBuf::from("test.rs")), "rust");
        assert_eq!(detect_language(&PathBuf::from("test.py")), "python");
        assert_eq!(detect_language(&PathBuf::from("test.ts")), "typescript");
        assert_eq!(detect_language(&PathBuf::from("test.go")), "go");
    }

    #[test]
    fn test_similarity_calculation() {
        assert_eq!(calculate_similarity("test", "test"), 1.0);
        assert!((calculate_similarity("hello world", "hello") - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_lsp_query_engine_creation() {
        let _engine = LspQueryEngine::new();
        // Basic creation test - no async needed for new()
    }
}
