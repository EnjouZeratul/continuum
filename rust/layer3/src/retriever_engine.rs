//! # Retriever Engine
//!
//! 检索引擎：向量相似度检索和 RAG 支持。
//!
//! ## 功能
//!
//! - 文档索引与检索
//! - 多种分块策略（固定大小、段落、代码）
//! - 混合检索（向量 + 关键词）
//! - RAG Pipeline（带重排序）
//! - OpenAI Embeddings 集成

use crate::types::Layer3Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use sh_layer2::generate_short_id;
use std::collections::HashMap;
use std::sync::Arc;

/// 检索引擎 trait
///
/// 提供向量相似度检索能力。
#[async_trait]
pub trait RetrieverEngine: Send + Sync {
    /// 索引文档
    async fn index(&self, documents: Vec<Document>) -> Layer3Result<Vec<String>>;

    /// 检索相似文档
    async fn retrieve(&self, query: &str, top_k: usize) -> Layer3Result<Vec<RetrievalResult>>;

    /// 混合检索（向量 + 关键词）
    async fn hybrid_retrieve(
        &self,
        query: &str,
        top_k: usize,
    ) -> Layer3Result<Vec<RetrievalResult>>;

    /// 带配置的混合检索
    async fn hybrid_retrieve_with_config(
        &self,
        query: &str,
        top_k: usize,
        config: &HybridSearchConfig,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let _ = config;
        self.hybrid_retrieve(query, top_k).await
    }

    /// 带过滤条件的检索
    async fn retrieve_with_filter(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<crate::vector_store::MetadataFilter>,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let _ = filter;
        self.retrieve(query, top_k).await
    }

    /// 删除文档
    async fn delete(&self, doc_ids: &[String]) -> Layer3Result<bool>;

    /// 清空索引
    async fn clear(&self) -> Layer3Result<bool>;

    /// 获取文档数量
    async fn count(&self) -> Layer3Result<usize>;
}

/// 文档结构
#[derive(Debug, Clone)]
pub struct Document {
    /// 文档 ID（可选，自动生成）
    pub id: Option<String>,
    /// 文档内容
    pub content: String,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 来源（文件路径、URL 等）
    pub source: Option<String>,
}

impl Document {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            metadata: HashMap::new(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// 检索结果
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    /// 文档 ID
    pub doc_id: String,
    /// 文档内容
    pub content: String,
    /// 相似度分数 (0.0-1.0)
    pub score: f32,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 来源
    pub source: Option<String>,
}

// ============================================================================
// Hybrid Search Configuration
// ============================================================================

/// 混合检索权重配置
#[derive(Debug, Clone, Copy)]
pub struct HybridWeights {
    /// 向量搜索权重
    pub vector: f32,
    /// 关键词搜索权重
    pub keyword: f32,
}

impl HybridWeights {
    /// 创建新的权重配置
    pub fn new(vector: f32, keyword: f32) -> Self {
        let total = vector + keyword;
        Self {
            vector: vector / total,
            keyword: keyword / total,
        }
    }

    /// 默认权重：70% 向量 + 30% 关键词
    pub fn default_weights() -> Self {
        Self {
            vector: 0.7,
            keyword: 0.3,
        }
    }

    /// 仅向量搜索
    pub fn vector_only() -> Self {
        Self {
            vector: 1.0,
            keyword: 0.0,
        }
    }

    /// 仅关键词搜索
    pub fn keyword_only() -> Self {
        Self {
            vector: 0.0,
            keyword: 1.0,
        }
    }

    /// 均衡权重
    pub fn balanced() -> Self {
        Self {
            vector: 0.5,
            keyword: 0.5,
        }
    }
}

impl Default for HybridWeights {
    fn default() -> Self {
        Self::default_weights()
    }
}

/// 混合检索配置
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// 权重配置
    pub weights: HybridWeights,
    /// 是否启用短语匹配
    pub phrase_matching: bool,
    /// 是否启用 RRIF 重排序
    pub use_rrif: bool,
    /// RRIF 参数 K（控制排名衰减）
    pub rrif_k: f32,
    /// 候选结果数量倍数（top_k * candidates_multiplier）
    pub candidates_multiplier: usize,
}

impl HybridSearchConfig {
    pub fn new() -> Self {
        Self {
            weights: HybridWeights::default(),
            phrase_matching: true,
            use_rrif: true,
            rrif_k: 60.0,
            candidates_multiplier: 2,
        }
    }

    pub fn with_weights(mut self, weights: HybridWeights) -> Self {
        self.weights = weights;
        self
    }

    pub fn with_phrase_matching(mut self, enabled: bool) -> Self {
        self.phrase_matching = enabled;
        self
    }

    pub fn with_rrif(mut self, enabled: bool, k: f32) -> Self {
        self.use_rrif = enabled;
        self.rrif_k = k;
        self
    }
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Embedding 模型 trait
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// 生成文本嵌入向量
    async fn embed(&self, text: &str) -> Layer3Result<Vec<f32>>;

    /// 批量生成嵌入向量
    async fn embed_batch(&self, texts: &[String]) -> Layer3Result<Vec<Vec<f32>>>;

    /// 获取向量维度
    fn dimension(&self) -> usize;

    /// 模型名称
    fn model_name(&self) -> &str;
}

/// 分块策略 trait
pub trait ChunkingStrategy: Send + Sync {
    /// 分块文档
    fn chunk(&self, document: &Document) -> Vec<Chunk>;
}

/// 文档分块
#[derive(Debug, Clone)]
pub struct Chunk {
    /// 分块 ID
    pub id: String,
    /// 文档 ID
    pub doc_id: String,
    /// 分块内容
    pub content: String,
    /// 在原文中的位置
    pub position: ChunkPosition,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 分块位置
#[derive(Debug, Clone, Copy)]
pub struct ChunkPosition {
    /// 起始字符位置
    pub start: usize,
    /// 结束字符位置
    pub end: usize,
    /// 分块索引
    pub index: usize,
    /// 总分块数
    pub total: usize,
}

/// 固定大小分块策略
#[derive(Debug, Clone)]
pub struct FixedSizeChunker {
    /// 分块大小（字符数）
    pub chunk_size: usize,
    /// 重叠大小
    pub overlap: usize,
}

impl FixedSizeChunker {
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        Self {
            chunk_size,
            overlap,
        }
    }
}

impl Default for FixedSizeChunker {
    fn default() -> Self {
        Self {
            chunk_size: 500,
            overlap: 50,
        }
    }
}

impl ChunkingStrategy for FixedSizeChunker {
    fn chunk(&self, document: &Document) -> Vec<Chunk> {
        let content = &document.content;
        if content.len() <= self.chunk_size {
            return vec![Chunk {
                id: format!("{}-0", document.id.as_deref().unwrap_or("doc")),
                doc_id: document.id.clone().unwrap_or_default(),
                content: content.clone(),
                position: ChunkPosition {
                    start: 0,
                    end: content.len(),
                    index: 0,
                    total: 1,
                },
                metadata: document.metadata.clone(),
            }];
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let mut index = 0;

        while start < content.len() {
            let end = (start + self.chunk_size).min(content.len());
            chunks.push(Chunk {
                id: format!("{}-{}", document.id.as_deref().unwrap_or("doc"), index),
                doc_id: document.id.clone().unwrap_or_default(),
                content: content[start..end].to_string(),
                position: ChunkPosition {
                    start,
                    end,
                    index,
                    total: 0, // 将在最后更新
                },
                metadata: document.metadata.clone(),
            });
            // 防止死循环：到达末尾时直接设置 start = end
            start = if end < content.len() {
                end.saturating_sub(self.overlap)
            } else {
                end
            };
            index += 1;
        }

        let total = chunks.len();
        for chunk in &mut chunks {
            chunk.position.total = total;
        }

        chunks
    }
}

// ============================================================================
// Paragraph Chunking Strategy
// ============================================================================

/// 段落分块策略
///
/// 按自然段落边界分块，保持语义完整性。
#[derive(Debug, Clone)]
pub struct ParagraphChunker {
    max_chunk_size: usize,
    min_chunk_size: usize,
}

impl ParagraphChunker {
    pub fn new(max_chunk_size: usize, min_chunk_size: usize) -> Self {
        Self {
            max_chunk_size,
            min_chunk_size,
        }
    }
}

impl Default for ParagraphChunker {
    fn default() -> Self {
        Self {
            max_chunk_size: 1000,
            min_chunk_size: 100,
        }
    }
}

impl ChunkingStrategy for ParagraphChunker {
    fn chunk(&self, document: &Document) -> Vec<Chunk> {
        let content = &document.content;
        let paragraphs: Vec<&str> = content
            .split('\n')
            .filter(|p| !p.trim().is_empty())
            .collect();

        if paragraphs.is_empty() {
            return vec![Chunk {
                id: format!("{}-0", document.id.as_deref().unwrap_or("doc")),
                doc_id: document.id.clone().unwrap_or_default(),
                content: content.clone(),
                position: ChunkPosition {
                    start: 0,
                    end: content.len(),
                    index: 0,
                    total: 1,
                },
                metadata: document.metadata.clone(),
            }];
        }

        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut start = 0;
        let mut index = 0;

        for paragraph in paragraphs {
            if current_chunk.len() + paragraph.len() < self.max_chunk_size {
                if !current_chunk.is_empty() {
                    current_chunk.push('\n');
                }
                current_chunk.push_str(paragraph);
            } else {
                if current_chunk.len() >= self.min_chunk_size {
                    let end = start + current_chunk.len();
                    chunks.push(Chunk {
                        id: format!("{}-{}", document.id.as_deref().unwrap_or("doc"), index),
                        doc_id: document.id.clone().unwrap_or_default(),
                        content: current_chunk.clone(),
                        position: ChunkPosition {
                            start,
                            end,
                            index,
                            total: 0,
                        },
                        metadata: document.metadata.clone(),
                    });
                    start = end;
                    index += 1;
                }
                current_chunk = paragraph.to_string();
            }
        }

        if current_chunk.len() >= self.min_chunk_size {
            chunks.push(Chunk {
                id: format!("{}-{}", document.id.as_deref().unwrap_or("doc"), index),
                doc_id: document.id.clone().unwrap_or_default(),
                content: current_chunk,
                position: ChunkPosition {
                    start,
                    end: content.len(),
                    index,
                    total: 0,
                },
                metadata: document.metadata.clone(),
            });
        }

        let total = chunks.len().max(1);
        for chunk in &mut chunks {
            chunk.position.total = total;
        }

        if chunks.is_empty() {
            vec![Chunk {
                id: format!("{}-0", document.id.as_deref().unwrap_or("doc")),
                doc_id: document.id.clone().unwrap_or_default(),
                content: content.clone(),
                position: ChunkPosition {
                    start: 0,
                    end: content.len(),
                    index: 0,
                    total: 1,
                },
                metadata: document.metadata.clone(),
            }]
        } else {
            chunks
        }
    }
}

// ============================================================================
// Recursive Chunking Strategy
// ============================================================================

/// 递归分块策略
///
/// 依次尝试多种分隔符，从大到小。
#[derive(Debug, Clone)]
pub struct RecursiveChunker {
    max_chunk_size: usize,
    separators: Vec<String>,
}

impl RecursiveChunker {
    pub fn new(max_chunk_size: usize) -> Self {
        Self {
            max_chunk_size,
            separators: vec![
                "\n\n\n".to_string(),
                "\n\n".to_string(),
                "\n".to_string(),
                ". ".to_string(),
                " ".to_string(),
                "".to_string(),
            ],
        }
    }
}

impl Default for RecursiveChunker {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl ChunkingStrategy for RecursiveChunker {
    fn chunk(&self, document: &Document) -> Vec<Chunk> {
        self._recursive_split(document, &document.content, 0, 0)
    }
}

impl RecursiveChunker {
    fn _recursive_split(
        &self,
        document: &Document,
        text: &str,
        start_offset: usize,
        initial_index: usize,
    ) -> Vec<Chunk> {
        if text.len() <= self.max_chunk_size {
            return vec![Chunk {
                id: format!(
                    "{}-{}",
                    document.id.as_deref().unwrap_or("doc"),
                    initial_index
                ),
                doc_id: document.id.clone().unwrap_or_default(),
                content: text.to_string(),
                position: ChunkPosition {
                    start: start_offset,
                    end: start_offset + text.len(),
                    index: initial_index,
                    total: 1,
                },
                metadata: document.metadata.clone(),
            }];
        }

        for separator in &self.separators {
            if separator.is_empty() {
                let mut chunks = Vec::new();
                let mut start = 0;
                let mut index = initial_index;

                while start < text.len() {
                    let end = (start + self.max_chunk_size).min(text.len());
                    chunks.push(Chunk {
                        id: format!("{}-{}", document.id.as_deref().unwrap_or("doc"), index),
                        doc_id: document.id.clone().unwrap_or_default(),
                        content: text[start..end].to_string(),
                        position: ChunkPosition {
                            start: start_offset + start,
                            end: start_offset + end,
                            index,
                            total: 0,
                        },
                        metadata: document.metadata.clone(),
                    });
                    start = end;
                    index += 1;
                }

                let total = chunks.len();
                for chunk in &mut chunks {
                    chunk.position.total = total;
                }
                return chunks;
            }

            if text.contains(separator) {
                let parts: Vec<&str> = text.split(separator).collect();
                let mut chunks = Vec::new();
                let mut current_chunk = String::new();
                let mut current_start = start_offset;
                let mut index = initial_index;

                for (i, part) in parts.iter().enumerate() {
                    let part_with_sep = if i < parts.len() - 1 {
                        format!("{}{}", part, separator)
                    } else {
                        part.to_string()
                    };

                    if current_chunk.len() + part_with_sep.len() <= self.max_chunk_size {
                        current_chunk.push_str(&part_with_sep);
                    } else {
                        if !current_chunk.is_empty() {
                            chunks.push(Chunk {
                                id: format!(
                                    "{}-{}",
                                    document.id.as_deref().unwrap_or("doc"),
                                    index
                                ),
                                doc_id: document.id.clone().unwrap_or_default(),
                                content: current_chunk.clone(),
                                position: ChunkPosition {
                                    start: current_start,
                                    end: current_start + current_chunk.len(),
                                    index,
                                    total: 0,
                                },
                                metadata: document.metadata.clone(),
                            });
                            current_start += current_chunk.len();
                            index += 1;
                        }

                        if part_with_sep.len() > self.max_chunk_size {
                            let sub_chunks = self._recursive_split(
                                document,
                                &part_with_sep,
                                current_start,
                                index,
                            );
                            for sub in sub_chunks {
                                current_start = sub.position.end;
                                index += 1;
                                chunks.push(sub);
                            }
                        } else {
                            current_chunk = part_with_sep;
                        }
                    }
                }

                if !current_chunk.is_empty() {
                    chunks.push(Chunk {
                        id: format!("{}-{}", document.id.as_deref().unwrap_or("doc"), index),
                        doc_id: document.id.clone().unwrap_or_default(),
                        content: current_chunk,
                        position: ChunkPosition {
                            start: current_start,
                            end: start_offset + text.len(),
                            index,
                            total: 0,
                        },
                        metadata: document.metadata.clone(),
                    });
                }

                let total = chunks.len().max(1);
                for chunk in &mut chunks {
                    chunk.position.total = total;
                }
                return chunks;
            }
        }

        vec![Chunk {
            id: format!(
                "{}-{}",
                document.id.as_deref().unwrap_or("doc"),
                initial_index
            ),
            doc_id: document.id.clone().unwrap_or_default(),
            content: text.to_string(),
            position: ChunkPosition {
                start: start_offset,
                end: start_offset + text.len(),
                index: initial_index,
                total: 1,
            },
            metadata: document.metadata.clone(),
        }]
    }
}

// ============================================================================
// Default Retriever Engine Implementation
// ============================================================================

use crate::vector_store::{VectorItem, VectorStore};

/// 默认检索引擎实现
///
/// 结合 Embedding 模型、分块策略和向量存储提供完整的 RAG 功能。
pub struct DefaultRetrieverEngine<VS, EM, CS>
where
    VS: VectorStore,
    EM: EmbeddingModel,
    CS: ChunkingStrategy,
{
    /// 向量存储
    vector_store: VS,
    /// Embedding 模型
    embedding_model: EM,
    /// 分块策略
    chunking_strategy: CS,
    /// 文档索引（文档 ID -> 分块 ID 列表）
    doc_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// 分块内容缓存（分块 ID -> 内容）
    chunk_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl<VS, EM, CS> DefaultRetrieverEngine<VS, EM, CS>
where
    VS: VectorStore,
    EM: EmbeddingModel,
    CS: ChunkingStrategy,
{
    /// 创建新的检索引擎
    pub fn new(vector_store: VS, embedding_model: EM, chunking_strategy: CS) -> Self {
        Self {
            vector_store,
            embedding_model,
            chunking_strategy,
            doc_index: Arc::new(RwLock::new(HashMap::new())),
            chunk_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 提取关键词（分词 + 去停用词）
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        let words: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let stop_words = std::collections::HashSet::from([
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "shall", "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with",
            "at", "by", "from", "as", "into", "through", "during", "before", "after", "above",
            "below", "between", "under", "again", "further", "then", "once", "here", "there",
            "when", "where", "why", "how", "all", "each", "few", "more", "most", "other", "some",
            "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "s",
            "t", "just", "and", "but", "if", "or", "because", "until", "while", "although",
        ]);

        words
            .into_iter()
            .filter(|w| !stop_words.contains(w.as_str()) && w.len() > 1)
            .collect()
    }

    /// 计算关键词分数（BM25 风格）
    fn compute_keyword_score(
        &self,
        query_keywords: &[String],
        content: &str,
        config: &HybridSearchConfig,
    ) -> f32 {
        if query_keywords.is_empty() {
            return 0.0;
        }

        let content_lower = content.to_lowercase();

        // 短语匹配奖励
        let mut phrase_bonus: f32 = 0.0;
        if config.phrase_matching {
            for keyword in query_keywords {
                if content_lower.contains(keyword) {
                    phrase_bonus += 0.1;
                }
            }
            phrase_bonus = phrase_bonus.min(0.3);
        }

        // 计算关键词匹配数量
        let matched_keywords = query_keywords
            .iter()
            .filter(|kw| content_lower.contains(kw.as_str()))
            .count();

        // BM25 风格的饱和函数
        let k1 = 1.2;
        let content_len = content.len() as f32;
        let avg_len = 500.0;
        let len_norm = 1.0 - 0.75 + 0.75 * (content_len / avg_len);

        let bm25_score =
            (matched_keywords as f32 * (k1 + 1.0)) / (matched_keywords as f32 + k1 * len_norm);

        // 归一化到 [0, 1]
        let normalized_score = bm25_score / (query_keywords.len() as f32 + k1);
        let normalized_score = normalized_score.min(1.0);

        normalized_score + phrase_bonus
    }

    /// 仅关键词搜索
    async fn keyword_only_search(
        &self,
        query: &str,
        candidates: Vec<RetrievalResult>,
        top_k: usize,
        config: &HybridSearchConfig,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let query_keywords = self.extract_keywords(query);

        let mut scored_results: Vec<RetrievalResult> = candidates
            .into_iter()
            .map(|r| {
                let keyword_score = self.compute_keyword_score(&query_keywords, &r.content, config);
                RetrievalResult {
                    doc_id: r.doc_id,
                    content: r.content,
                    score: keyword_score,
                    metadata: r.metadata,
                    source: r.source,
                }
            })
            .collect();

        scored_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored_results.truncate(top_k);
        Ok(scored_results)
    }

    /// 应用 RRIF (Reciprocal Rank Fusion) 重排序
    fn apply_rrif(&self, results: Vec<RetrievalResult>, k: f32) -> Vec<RetrievalResult> {
        if results.is_empty() {
            return results;
        }

        results
            .into_iter()
            .enumerate()
            .map(|(idx, mut r)| {
                let rank = (idx + 1) as f32;
                let rrif_score = 1.0 / (k + rank);
                r.score = r.score * 0.5 + rrif_score * 0.5;
                r
            })
            .collect()
    }
}

#[async_trait]
impl<VS, EM, CS> RetrieverEngine for DefaultRetrieverEngine<VS, EM, CS>
where
    VS: VectorStore,
    EM: EmbeddingModel,
    CS: ChunkingStrategy,
{
    async fn index(&self, documents: Vec<Document>) -> Layer3Result<Vec<String>> {
        let mut doc_ids = Vec::new();

        for doc in documents {
            // 生成分块
            let doc_id = doc.id.clone().unwrap_or_else(generate_short_id);
            let chunks = self.chunking_strategy.chunk(&Document {
                id: Some(doc_id.clone()),
                content: doc.content.clone(),
                metadata: doc.metadata.clone(),
                source: doc.source.clone(),
            });

            // 为每个分块生成 embedding 并存储
            let chunk_ids: Vec<String> = chunks.iter().map(|c| c.id.clone()).collect();

            let chunk_contents: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();

            // 批量生成 embeddings
            let embeddings = self.embedding_model.embed_batch(&chunk_contents).await?;

            // 构建向量项并存储
            let vector_items: Vec<VectorItem> = chunks
                .into_iter()
                .zip(embeddings)
                .map(|(chunk, embedding)| {
                    let mut metadata = chunk.metadata.clone();
                    metadata.insert("doc_id".to_string(), serde_json::json!(chunk.doc_id));
                    metadata.insert(
                        "chunk_index".to_string(),
                        serde_json::json!(chunk.position.index),
                    );
                    if let Some(source) = doc.source.clone() {
                        metadata.insert("source".to_string(), serde_json::json!(source));
                    }

                    VectorItem {
                        id: chunk.id.clone(),
                        vector: embedding,
                        metadata,
                        content: Some(chunk.content.clone()),
                    }
                })
                .collect();

            // 缓存分块内容
            {
                let mut cache = self.chunk_cache.write();
                for item in &vector_items {
                    cache.insert(item.id.clone(), item.content.clone().unwrap_or_default());
                }
            }

            // 存储向量
            self.vector_store.add_batch(vector_items).await?;

            // 记录文档索引
            {
                let mut index = self.doc_index.write();
                index.insert(doc_id.clone(), chunk_ids);
            }

            doc_ids.push(doc_id);
        }

        Ok(doc_ids)
    }

    async fn retrieve(&self, query: &str, top_k: usize) -> Layer3Result<Vec<RetrievalResult>> {
        // 生成查询向量
        let query_embedding = self.embedding_model.embed(query).await?;

        // 搜索相似向量
        let results = self.vector_store.query(query_embedding, top_k).await?;

        // 补充内容（从缓存中获取完整内容）
        let cache = self.chunk_cache.read();
        let enriched_results: Vec<RetrievalResult> = results
            .into_iter()
            .map(|r| {
                let content = cache.get(&r.doc_id).cloned().unwrap_or(r.content);
                RetrievalResult {
                    doc_id: r.doc_id,
                    content,
                    score: r.score,
                    metadata: r.metadata,
                    source: r.source,
                }
            })
            .collect();

        Ok(enriched_results)
    }

    async fn hybrid_retrieve(
        &self,
        query: &str,
        top_k: usize,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        self.hybrid_retrieve_with_config(query, top_k, &HybridSearchConfig::default())
            .await
    }

    async fn hybrid_retrieve_with_config(
        &self,
        query: &str,
        top_k: usize,
        config: &HybridSearchConfig,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        // 如果仅使用向量搜索，直接返回
        if config.weights.keyword == 0.0 {
            return self.retrieve(query, top_k).await;
        }

        // 1. 向量搜索：获取更多候选结果
        let candidates_count = top_k * config.candidates_multiplier;
        let vector_results = self.retrieve(query, candidates_count).await?;

        // 如果仅使用关键词搜索
        if config.weights.vector == 0.0 {
            return self
                .keyword_only_search(query, vector_results, top_k, config)
                .await;
        }

        // 2. 提取查询关键词
        let query_keywords = self.extract_keywords(query);

        // 3. 计算混合分数
        let mut scored_results: Vec<RetrievalResult> = vector_results
            .into_iter()
            .map(|r| {
                let keyword_score = self.compute_keyword_score(&query_keywords, &r.content, config);

                // 混合分数
                let final_score =
                    r.score * config.weights.vector + keyword_score * config.weights.keyword;

                RetrievalResult {
                    doc_id: r.doc_id,
                    content: r.content,
                    score: final_score,
                    metadata: r.metadata,
                    source: r.source,
                }
            })
            .collect();

        // 4. 按分数排序
        scored_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 5. 可选：RRIF 重排序
        if config.use_rrif {
            scored_results = self.apply_rrif(scored_results, config.rrif_k);
        }

        // 6. 截断并返回
        scored_results.truncate(top_k);
        Ok(scored_results)
    }

    async fn delete(&self, doc_ids: &[String]) -> Layer3Result<bool> {
        // 先收集需要删除的分块 ID，然后释放锁
        let all_chunk_ids: Vec<String> = {
            let mut index = self.doc_index.write();
            let mut cache = self.chunk_cache.write();

            let mut ids_to_delete: Vec<String> = Vec::new();
            for doc_id in doc_ids {
                if let Some(chunk_ids) = index.remove(doc_id) {
                    for chunk_id in &chunk_ids {
                        cache.remove(chunk_id);
                    }
                    ids_to_delete.extend(chunk_ids);
                }
            }
            ids_to_delete
        };

        if all_chunk_ids.is_empty() {
            return Ok(false);
        }

        self.vector_store.delete_batch(&all_chunk_ids).await?;
        Ok(true)
    }

    async fn clear(&self) -> Layer3Result<bool> {
        self.vector_store.clear().await?;
        let mut index = self.doc_index.write();
        index.clear();
        let mut cache = self.chunk_cache.write();
        cache.clear();
        Ok(true)
    }

    async fn count(&self) -> Layer3Result<usize> {
        let index = self.doc_index.read();
        Ok(index.len())
    }
}

/// Layer1 EmbeddingModel wrapper
///
/// Wraps a layer1 embedding model to implement layer3's EmbeddingModel trait.
pub struct Layer1EmbeddingAdapter {
    inner: Box<dyn sh_layer1::EmbeddingModel>,
}

impl Layer1EmbeddingAdapter {
    /// Create a new adapter wrapping a layer1 embedding model
    pub fn new(model: Box<dyn sh_layer1::EmbeddingModel>) -> Self {
        Self { inner: model }
    }
}

#[async_trait]
impl EmbeddingModel for Layer1EmbeddingAdapter {
    async fn embed(&self, text: &str) -> Layer3Result<Vec<f32>> {
        self.inner.embed(text).await
    }

    async fn embed_batch(&self, texts: &[String]) -> Layer3Result<Vec<Vec<f32>>> {
        self.inner.embed_batch(texts).await
    }

    fn dimension(&self) -> usize {
        self.inner.dimension()
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_store::InMemoryVectorStore;

    /// 创建测试用的 Mock Embedding 模型
    /// 使用 Layer1 的 MockEmbeddingModel 通过适配器
    fn create_mock_embedding_model(dimension: usize) -> Layer1EmbeddingAdapter {
        #[cfg(any(feature = "mock", test))]
        {
            Layer1EmbeddingAdapter::new(Box::new(sh_layer1::MockEmbeddingModel::new(dimension)))
        }
        #[cfg(not(any(feature = "mock", test)))]
        {
            // 在非测试配置下，这个分支不应该被执行
            // 使用 unreachable! 来确保编译时检查
            compile_error!("MockEmbeddingModel requires 'mock' feature or test configuration")
        }
    }

    #[test]
    fn test_document_builder() {
        let doc = Document::new("test content")
            .with_source("test.txt")
            .with_metadata("key", serde_json::json!("value"));
        assert_eq!(doc.source, Some("test.txt".to_string()));
    }

    #[test]
    fn test_fixed_size_chunker() {
        let chunker = FixedSizeChunker::new(100, 20);
        let doc = Document::new("a".repeat(250));
        let chunks = chunker.chunk(&doc);
        assert!(!chunks.is_empty());
    }

    #[tokio::test]
    async fn test_default_retriever_engine_index() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);

        let doc = Document::new("This is a test document for RAG.").with_source("test.txt");

        let doc_ids = engine.index(vec![doc]).await.unwrap();
        assert_eq!(doc_ids.len(), 1);
        assert_eq!(engine.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_default_retriever_engine_retrieve() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);

        // 索引文档
        let docs = vec![
            Document::new("Rust is a systems programming language."),
            Document::new("Python is great for data science."),
        ];
        engine.index(docs).await.unwrap();

        // 检索
        let results = engine.retrieve("Rust programming", 5).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_default_retriever_engine_delete() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);

        let doc = Document::new("Test document");
        let doc_ids = engine.index(vec![doc]).await.unwrap();

        let deleted = engine.delete(&doc_ids).await.unwrap();
        assert!(deleted);
        assert_eq!(engine.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mock_embedding_model() {
        let model = create_mock_embedding_model(64);

        let embedding = model.embed("test").await.unwrap();
        assert_eq!(embedding.len(), 64);
        assert_eq!(model.dimension(), 64);
        assert_eq!(model.model_name(), "mock-embedding");

        let embeddings = model
            .embed_batch(&["test1".to_string(), "test2".to_string()])
            .await
            .unwrap();
        assert_eq!(embeddings.len(), 2);
    }

    #[tokio::test]
    async fn test_hybrid_retrieve() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);

        // 索引文档
        let docs = vec![
            Document::new("Rust is a systems programming language designed for performance."),
            Document::new("Python is widely used for data science and machine learning."),
            Document::new("JavaScript runs in the browser for web development."),
        ];
        engine.index(docs).await.unwrap();

        // 混合检索
        let results = engine
            .hybrid_retrieve("Rust programming language", 5)
            .await
            .unwrap();
        assert!(!results.is_empty());
        // Rust 相关文档应该在前面
        assert!(results[0].content.contains("Rust"));
    }

    #[tokio::test]
    async fn test_hybrid_retrieve_with_config() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);

        // 索引文档
        let docs = vec![
            Document::new("Machine learning algorithms use neural networks."),
            Document::new("The database stores data for the application."),
        ];
        engine.index(docs).await.unwrap();

        // 测试仅向量搜索
        let config_vector_only =
            HybridSearchConfig::new().with_weights(HybridWeights::vector_only());
        let results = engine
            .hybrid_retrieve_with_config("neural networks", 5, &config_vector_only)
            .await
            .unwrap();
        assert!(!results.is_empty());

        // 测试仅关键词搜索
        let config_keyword_only =
            HybridSearchConfig::new().with_weights(HybridWeights::keyword_only());
        let results = engine
            .hybrid_retrieve_with_config("machine learning", 5, &config_keyword_only)
            .await
            .unwrap();
        assert!(!results.is_empty());

        // 测试均衡权重
        let config_balanced = HybridSearchConfig::new()
            .with_weights(HybridWeights::balanced())
            .with_rrif(true, 60.0);
        let results = engine
            .hybrid_retrieve_with_config("database", 5, &config_balanced)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_hybrid_weights() {
        let weights = HybridWeights::default_weights();
        assert_eq!(weights.vector, 0.7);
        assert_eq!(weights.keyword, 0.3);

        let vector_only = HybridWeights::vector_only();
        assert_eq!(vector_only.vector, 1.0);
        assert_eq!(vector_only.keyword, 0.0);

        let balanced = HybridWeights::balanced();
        assert_eq!(balanced.vector, 0.5);
        assert_eq!(balanced.keyword, 0.5);
    }

    #[test]
    fn test_extract_keywords() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);

        // 测试关键词提取
        let keywords = engine.extract_keywords("The Rust programming language");
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"programming".to_string()));
        assert!(keywords.contains(&"language".to_string()));
        // 停用词应该被过滤
        assert!(!keywords.contains(&"the".to_string()));
    }

    #[test]
    fn test_bm25_keyword_score() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);
        let chunker = FixedSizeChunker::default();

        let engine = DefaultRetrieverEngine::new(vector_store, embedding_model, chunker);
        let config = HybridSearchConfig::new();

        let keywords = vec!["rust".to_string(), "programming".to_string()];

        // 高匹配内容
        let score_high = engine.compute_keyword_score(
            &keywords,
            "Rust programming language for systems",
            &config,
        );

        // 低匹配内容
        let score_low =
            engine.compute_keyword_score(&keywords, "Python data science frameworks", &config);

        assert!(score_high > score_low);
    }
}
