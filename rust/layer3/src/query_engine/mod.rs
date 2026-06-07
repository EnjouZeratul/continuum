//! # Query Engine
//!
//! 代码查询引擎：基于 LSP 的代码分析能力。

pub mod lsp_query_engine;

use crate::types::{CodeLocation, CodeRange, Layer3Result, QueryResult, QueryType};
use async_trait::async_trait;
use std::path::PathBuf;

// Re-export the LSP implementation
pub use lsp_query_engine::{LspCodeAnalyzer, LspQueryEngine};

/// 查询引擎 trait
///
/// 提供代码符号查询能力。
#[async_trait]
pub trait QueryEngine: Send + Sync {
    /// 查询符号定义
    async fn go_to_definition(&self, location: CodeLocation) -> Layer3Result<Option<QueryResult>>;

    /// 查询符号引用
    async fn find_references(&self, location: CodeLocation) -> Layer3Result<Vec<QueryResult>>;

    /// 查询接口实现
    async fn go_to_implementation(&self, location: CodeLocation) -> Layer3Result<Vec<QueryResult>>;

    /// 查询类型定义
    async fn go_to_type_definition(
        &self,
        location: CodeLocation,
    ) -> Layer3Result<Option<QueryResult>>;

    /// 获取悬停信息
    async fn hover(&self, location: CodeLocation) -> Layer3Result<Option<String>>;

    /// 列出文档符号
    async fn document_symbols(&self, file: PathBuf) -> Layer3Result<Vec<SymbolInfo>>;

    /// 工作区符号搜索
    async fn workspace_symbols(&self, query: &str) -> Layer3Result<Vec<SymbolInfo>>;

    /// 通用查询方法
    async fn query(
        &self,
        query_type: QueryType,
        location: CodeLocation,
    ) -> Layer3Result<Vec<QueryResult>>;
}

/// 符号信息
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// 符号名称
    pub name: String,
    /// 符号类型
    pub kind: SymbolKind,
    /// 定义位置
    pub location: CodeLocation,
    /// 符号范围
    pub range: Option<CodeRange>,
    /// 所属容器（如类名）
    pub container_name: Option<String>,
}

/// 符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Macro,
    Other,
}

/// 代码分析器 trait
///
/// 提供更高级的代码分析能力。
#[async_trait]
pub trait CodeAnalyzer: Send + Sync {
    /// 分析代码结构
    async fn analyze_structure(&self, file: PathBuf) -> Layer3Result<CodeStructure>;

    /// 查找相似代码
    async fn find_similar(&self, snippet: &str, threshold: f32) -> Layer3Result<Vec<CodeMatch>>;

    /// 检测代码模式
    async fn detect_patterns(&self, file: PathBuf) -> Layer3Result<Vec<DetectedPattern>>;
}

/// 代码结构分析结果
#[derive(Debug, Clone)]
pub struct CodeStructure {
    /// 文件路径
    pub file: PathBuf,
    /// 导入/依赖
    pub imports: Vec<String>,
    /// 导出符号
    pub exports: Vec<String>,
    /// 定义的所有符号
    pub symbols: Vec<SymbolInfo>,
    /// 代码行数
    pub lines: usize,
}

/// 代码匹配结果
#[derive(Debug, Clone)]
pub struct CodeMatch {
    /// 匹配位置
    pub location: CodeLocation,
    /// 匹配内容
    pub content: String,
    /// 相似度分数
    pub similarity: f32,
}

/// 检测到的代码模式
#[derive(Debug, Clone)]
pub struct DetectedPattern {
    /// 模式名称
    pub name: String,
    /// 模式类型
    pub pattern_type: PatternType,
    /// 出现位置
    pub locations: Vec<CodeLocation>,
}

/// 代码模式类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    /// 设计模式
    DesignPattern,
    /// 反模式
    AntiPattern,
    /// 安全漏洞
    SecurityIssue,
    /// 性能问题
    PerformanceIssue,
    /// 代码风格
    CodeStyle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_info_creation() {
        let info = SymbolInfo {
            name: "test_function".to_string(),
            kind: SymbolKind::Function,
            location: CodeLocation {
                file: PathBuf::from("test.rs"),
                line: 1,
                column: 1,
            },
            range: None,
            container_name: None,
        };
        assert_eq!(info.name, "test_function");
    }
}
