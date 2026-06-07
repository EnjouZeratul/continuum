//! # Hybrid Retriever
//!
//! 混合检索器：结合 BM25 关键词检索和向量相似度检索。
//!
//! ## 功能
//!
//! - BM25 关键词检索
//! - 向量相似度检索
//! - Reciprocal Rank Fusion (RRF) 融合算法
//! - 可配置的权重支持

use crate::retriever_engine::{Document, EmbeddingModel, RetrievalResult};
use crate::types::Layer3Result;
use crate::vector_store::{MetadataFilter, VectorStore};
use async_trait::async_trait;
use parking_lot::RwLock;
use sh_layer2::generate_short_id;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::instrument;

// ============================================================================
// BM25 Implementation
// ============================================================================

/// BM25 检索器
///
/// 实现经典的 BM25 排序算法，用于关键词检索。
pub struct BM25Index {
    /// 文档 ID -> 文档内容
    documents: Arc<RwLock<HashMap<String, String>>>,
    /// 文档 ID -> 词项频率
    term_frequencies: Arc<RwLock<HashMap<String, HashMap<String, usize>>>>,
    /// 逆文档频率缓存
    idf_cache: Arc<RwLock<HashMap<String, f64>>>,
    /// 文档平均长度
    avg_doc_length: Arc<RwLock<f64>>,
    /// 总文档数
    doc_count: Arc<RwLock<usize>>,
    /// BM25 参数 k1 (词项饱和参数)
    k1: f64,
    /// BM25 参数 b (文档长度归一化参数)
    b: f64,
}

impl BM25Index {
    /// 创建新的 BM25 索引
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
            term_frequencies: Arc::new(RwLock::new(HashMap::new())),
            idf_cache: Arc::new(RwLock::new(HashMap::new())),
            avg_doc_length: Arc::new(RwLock::new(0.0)),
            doc_count: Arc::new(RwLock::new(0)),
            k1: 1.2,
            b: 0.75,
        }
    }

    /// 使用自定义参数创建 BM25 索引
    pub fn with_params(k1: f64, b: f64) -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
            term_frequencies: Arc::new(RwLock::new(HashMap::new())),
            idf_cache: Arc::new(RwLock::new(HashMap::new())),
            avg_doc_length: Arc::new(RwLock::new(0.0)),
            doc_count: Arc::new(RwLock::new(0)),
            k1,
            b,
        }
    }

    /// 添加文档到索引
    pub fn add_document(&self, doc_id: String, content: &str) {
        let tokens = self.tokenize(content);
        let mut tf: HashMap<String, usize> = HashMap::new();

        for token in tokens {
            *tf.entry(token).or_insert(0) += 1;
        }

        let doc_length = content.split_whitespace().count();

        {
            let mut documents = self.documents.write();
            documents.insert(doc_id.clone(), content.to_lowercase());
        }

        {
            let mut term_frequencies = self.term_frequencies.write();
            term_frequencies.insert(doc_id, tf);
        }

        // 更新统计信息
        {
            let mut avg_len = self.avg_doc_length.write();
            let mut count = self.doc_count.write();

            let old_count = *count;
            let old_avg = *avg_len;
            let new_count = old_count + 1;
            *avg_len = (old_avg * old_count as f64 + doc_length as f64) / new_count as f64;
            *count = new_count;
        }

        // 清除 IDF 缓存（需要重新计算）
        self.idf_cache.write().clear();
    }

    /// 批量添加文档
    pub fn add_documents(&self, docs: Vec<(String, String)>) {
        for (doc_id, content) in docs {
            self.add_document(doc_id, &content);
        }
    }

    /// 删除文档
    pub fn remove_document(&self, doc_id: &str) -> bool {
        let removed = {
            let mut documents = self.documents.write();
            documents.remove(doc_id).is_some()
        };

        if removed {
            let mut term_frequencies = self.term_frequencies.write();
            term_frequencies.remove(doc_id);

            // 更新文档计数
            {
                let mut count = self.doc_count.write();
                if *count > 0 {
                    *count -= 1;
                }
            }

            // 清除 IDF 缓存
            self.idf_cache.write().clear();
        }

        removed
    }

    /// 清空索引
    pub fn clear(&self) {
        self.documents.write().clear();
        self.term_frequencies.write().clear();
        self.idf_cache.write().clear();
        *self.avg_doc_length.write() = 0.0;
        *self.doc_count.write() = 0;
    }

    /// BM25 搜索
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(String, f64)> {
        let query_tokens = self.tokenize(query);

        if query_tokens.is_empty() {
            return Vec::new();
        }

        let documents = self.documents.read();
        let term_frequencies = self.term_frequencies.read();
        let avg_doc_length = *self.avg_doc_length.read();
        let doc_count = *self.doc_count.read();

        if doc_count == 0 {
            return Vec::new();
        }

        let mut scores: Vec<(String, f64)> = documents
            .keys()
            .filter_map(|doc_id| {
                let score = self.compute_bm25_score(
                    doc_id,
                    &query_tokens,
                    &term_frequencies,
                    avg_doc_length,
                    doc_count,
                );
                if score > 0.0 {
                    Some((doc_id.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);

        scores
    }

    /// 计算 BM25 分数
    fn compute_bm25_score(
        &self,
        doc_id: &str,
        query_tokens: &[String],
        term_frequencies: &HashMap<String, HashMap<String, usize>>,
        avg_doc_length: f64,
        doc_count: usize,
    ) -> f64 {
        let doc_tf = match term_frequencies.get(doc_id) {
            Some(tf) => tf,
            None => return 0.0,
        };

        let documents = self.documents.read();
        let doc_content = match documents.get(doc_id) {
            Some(content) => content,
            None => return 0.0,
        };

        let doc_length = doc_content.split_whitespace().count() as f64;
        let mut idf_cache = self.idf_cache.write();

        let mut score = 0.0;

        for token in query_tokens {
            let tf = *doc_tf.get(token).unwrap_or(&0) as f64;

            if tf == 0.0 {
                continue;
            }

            // 计算 IDF
            let idf = *idf_cache.entry(token.clone()).or_insert_with(|| {
                let df = self.compute_document_frequency(token);
                let n = doc_count as f64;
                ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
            });

            // BM25 公式
            let numerator = tf * (self.k1 + 1.0);
            let denominator =
                tf + self.k1 * (1.0 - self.b + self.b * (doc_length / avg_doc_length));

            score += idf * (numerator / denominator);
        }

        score
    }

    /// 计算词项的文档频率
    fn compute_document_frequency(&self, term: &str) -> f64 {
        let term_frequencies = self.term_frequencies.read();
        term_frequencies
            .values()
            .filter(|tf| tf.contains_key(term))
            .count() as f64
    }

    /// 分词
    fn tokenize(&self, text: &str) -> Vec<String> {
        let stop_words: HashSet<&str> = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "shall", "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with",
            "at", "by", "from", "as", "into", "through", "during", "before", "after", "above",
            "below", "between", "under", "again", "further", "then", "once", "here", "there",
            "when", "where", "why", "how", "all", "each", "few", "more", "most", "other", "some",
            "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "s",
            "t", "just", "and", "but", "if", "or", "because", "until", "while", "although",
        ]
        .iter()
        .cloned()
        .collect();

        text.to_lowercase()
            .split_whitespace()
            .filter(|w| !stop_words.contains(*w) && w.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// 获取文档数量
    pub fn doc_count(&self) -> usize {
        *self.doc_count.read()
    }
}

impl Default for BM25Index {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Reciprocal Rank Fusion
// ============================================================================

/// Reciprocal Rank Fusion (RRF) 融合器
///
/// 将多个检索结果列表融合为一个排序结果。
pub struct ReciprocalRankFusion {
    /// RRF 参数 K (控制排名衰减)
    k: f64,
}

impl ReciprocalRankFusion {
    /// 创建新的 RRF 融合器
    pub fn new(k: f64) -> Self {
        Self { k }
    }

    /// 使用默认参数创建 (k=60)
    pub fn default_fusion() -> Self {
        Self::new(60.0)
    }

    /// 融合多个检索结果
    ///
    /// # Arguments
    /// * `result_lists` - 多个检索结果列表，每个列表包含 (doc_id, score) 元组
    /// * `top_k` - 返回的结果数量
    ///
    /// # Returns
    /// 融合后的排序结果列表
    pub fn fuse(&self, result_lists: &[Vec<(String, f64)>], top_k: usize) -> Vec<(String, f64)> {
        let mut rrf_scores: HashMap<String, f64> = HashMap::new();

        for results in result_lists {
            for (rank, (doc_id, _original_score)) in results.iter().enumerate() {
                let rrf_score = 1.0 / (self.k + (rank + 1) as f64);
                *rrf_scores.entry(doc_id.clone()).or_insert(0.0) += rrf_score;
            }
        }

        let mut fused: Vec<(String, f64)> = rrf_scores.into_iter().collect();
        fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        fused.truncate(top_k);

        fused
    }

    /// 融合并保留原始分数权重
    ///
    /// # Arguments
    /// * `result_lists` - 多个检索结果列表
    /// * `weights` - 每个结果列表的权重
    /// * `top_k` - 返回的结果数量
    pub fn fuse_with_weights(
        &self,
        result_lists: &[Vec<(String, f64)>],
        weights: &[f64],
        top_k: usize,
    ) -> Vec<(String, f64)> {
        if result_lists.len() != weights.len() {
            panic!("Result lists and weights must have the same length");
        }

        let mut combined_scores: HashMap<String, f64> = HashMap::new();

        for (results, weight) in result_lists.iter().zip(weights.iter()) {
            for (rank, (doc_id, original_score)) in results.iter().enumerate() {
                let rrf_score = 1.0 / (self.k + (rank + 1) as f64);
                let weighted_score = (rrf_score + original_score * 0.1) * weight;
                *combined_scores.entry(doc_id.clone()).or_insert(0.0) += weighted_score;
            }
        }

        let mut fused: Vec<(String, f64)> = combined_scores.into_iter().collect();
        fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        fused.truncate(top_k);

        fused
    }
}

impl Default for ReciprocalRankFusion {
    fn default() -> Self {
        Self::default_fusion()
    }
}

// ============================================================================
// Hybrid Retriever Configuration
// ============================================================================

/// 混合检索配置
#[derive(Debug, Clone)]
pub struct HybridRetrieverConfig {
    /// 向量检索权重
    pub vector_weight: f64,
    /// BM25 检索权重
    pub bm25_weight: f64,
    /// RRF 参数 K
    pub rrf_k: f64,
    /// 是否启用 RRF 融合
    pub use_rrf: bool,
    /// 候选结果扩展倍数
    pub candidate_multiplier: usize,
    /// 最小分数阈值
    pub min_score_threshold: f64,
}

impl HybridRetrieverConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self {
            vector_weight: 0.7,
            bm25_weight: 0.3,
            rrf_k: 60.0,
            use_rrf: true,
            candidate_multiplier: 2,
            min_score_threshold: 0.0,
        }
    }

    /// 创建仅向量检索配置
    pub fn vector_only() -> Self {
        Self {
            vector_weight: 1.0,
            bm25_weight: 0.0,
            ..Self::new()
        }
    }

    /// 创建仅 BM25 检索配置
    pub fn bm25_only() -> Self {
        Self {
            vector_weight: 0.0,
            bm25_weight: 1.0,
            ..Self::new()
        }
    }

    /// 创建均衡配置
    pub fn balanced() -> Self {
        Self {
            vector_weight: 0.5,
            bm25_weight: 0.5,
            ..Self::new()
        }
    }

    /// 设置权重
    pub fn with_weights(mut self, vector: f64, bm25: f64) -> Self {
        let total = vector + bm25;
        self.vector_weight = vector / total;
        self.bm25_weight = bm25 / total;
        self
    }

    /// 设置 RRF 参数
    pub fn with_rrf(mut self, enabled: bool, k: f64) -> Self {
        self.use_rrf = enabled;
        self.rrf_k = k;
        self
    }

    /// 设置候选扩展倍数
    pub fn with_candidate_multiplier(mut self, multiplier: usize) -> Self {
        self.candidate_multiplier = multiplier;
        self
    }

    /// 设置最小分数阈值
    pub fn with_min_score(mut self, threshold: f64) -> Self {
        self.min_score_threshold = threshold;
        self
    }

    /// 归一化权重
    pub fn normalize_weights(&mut self) {
        let total = self.vector_weight + self.bm25_weight;
        if total > 0.0 {
            self.vector_weight /= total;
            self.bm25_weight /= total;
        }
    }
}

impl Default for HybridRetrieverConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Hybrid Retriever Trait
// ============================================================================

/// 混合检索器 trait
#[async_trait]
pub trait HybridRetriever: Send + Sync {
    /// 索引文档
    async fn index_documents(&self, documents: Vec<Document>) -> Layer3Result<Vec<String>>;

    /// 混合检索
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        config: Option<&HybridRetrieverConfig>,
    ) -> Layer3Result<Vec<RetrievalResult>>;

    /// 带过滤条件的混合检索
    async fn retrieve_with_filter(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<MetadataFilter>,
        config: Option<&HybridRetrieverConfig>,
    ) -> Layer3Result<Vec<RetrievalResult>>;

    /// 删除文档
    async fn delete_documents(&self, doc_ids: &[String]) -> Layer3Result<bool>;

    /// 清空索引
    async fn clear(&self) -> Layer3Result<bool>;

    /// 获取文档数量
    async fn count(&self) -> Layer3Result<usize>;
}

// ============================================================================
// Default Hybrid Retriever Implementation
// ============================================================================

/// 默认混合检索器实现
///
/// 结合 BM25 和向量检索，使用 RRF 融合结果。
pub struct DefaultHybridRetriever<VS, EM>
where
    VS: VectorStore,
    EM: EmbeddingModel,
{
    /// 向量存储
    vector_store: VS,
    /// Embedding 模型
    embedding_model: EM,
    /// BM25 索引
    bm25_index: BM25Index,
    /// 文档内容缓存
    doc_cache: Arc<RwLock<HashMap<String, (String, HashMap<String, serde_json::Value>)>>>,
    /// 默认配置
    default_config: HybridRetrieverConfig,
}

impl<VS, EM> DefaultHybridRetriever<VS, EM>
where
    VS: VectorStore,
    EM: EmbeddingModel,
{
    /// 创建新的混合检索器
    pub fn new(vector_store: VS, embedding_model: EM) -> Self {
        Self {
            vector_store,
            embedding_model,
            bm25_index: BM25Index::new(),
            doc_cache: Arc::new(RwLock::new(HashMap::new())),
            default_config: HybridRetrieverConfig::new(),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(
        vector_store: VS,
        embedding_model: EM,
        config: HybridRetrieverConfig,
    ) -> Self {
        Self {
            vector_store,
            embedding_model,
            bm25_index: BM25Index::new(),
            doc_cache: Arc::new(RwLock::new(HashMap::new())),
            default_config: config,
        }
    }

    /// 向量检索
    #[instrument(skip(self))]
    async fn vector_search(&self, query: &str, top_k: usize) -> Layer3Result<Vec<(String, f64)>> {
        let query_embedding = self.embedding_model.embed(query).await?;
        let results = self.vector_store.query(query_embedding, top_k).await?;

        Ok(results
            .into_iter()
            .map(|r| (r.doc_id, r.score as f64))
            .collect())
    }

    /// BM25 检索
    #[instrument(skip(self))]
    fn bm25_search(&self, query: &str, top_k: usize) -> Vec<(String, f64)> {
        self.bm25_index.search(query, top_k)
    }

    /// 获取文档内容
    fn get_document_content(
        &self,
        doc_id: &str,
    ) -> Option<(String, HashMap<String, serde_json::Value>)> {
        self.doc_cache.read().get(doc_id).cloned()
    }

    /// 应用分数阈值过滤
    fn apply_threshold(&self, results: Vec<(String, f64)>, threshold: f64) -> Vec<(String, f64)> {
        results
            .into_iter()
            .filter(|(_, score)| *score >= threshold)
            .collect()
    }
}

#[async_trait]
impl<VS, EM> HybridRetriever for DefaultHybridRetriever<VS, EM>
where
    VS: VectorStore,
    EM: EmbeddingModel,
{
    #[instrument(skip(self, documents))]
    async fn index_documents(&self, documents: Vec<Document>) -> Layer3Result<Vec<String>> {
        use crate::vector_store::VectorItem;

        let mut doc_ids = Vec::new();
        let mut vector_items = Vec::new();
        let mut bm25_docs = Vec::new();

        for doc in documents {
            let doc_id = doc.id.unwrap_or_else(generate_short_id);

            // 缓存文档内容
            {
                let mut cache = self.doc_cache.write();
                cache.insert(doc_id.clone(), (doc.content.clone(), doc.metadata.clone()));
            }

            // 添加到 BM25 索引
            bm25_docs.push((doc_id.clone(), doc.content.clone()));

            // 生成 embedding
            let embedding = self.embedding_model.embed(&doc.content).await?;

            let mut metadata = doc.metadata.clone();
            if let Some(source) = doc.source {
                metadata.insert("source".to_string(), serde_json::json!(source));
            }

            vector_items.push(VectorItem {
                id: doc_id.clone(),
                vector: embedding,
                metadata,
                content: Some(doc.content),
            });

            doc_ids.push(doc_id);
        }

        // 批量添加到 BM25 索引
        self.bm25_index.add_documents(bm25_docs);

        // 批量添加到向量存储
        self.vector_store.add_batch(vector_items).await?;

        Ok(doc_ids)
    }

    #[instrument(skip(self))]
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        config: Option<&HybridRetrieverConfig>,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let config = config.unwrap_or(&self.default_config);

        // 计算候选结果数量
        let candidates = top_k * config.candidate_multiplier;

        // 收集检索结果
        let mut result_lists: Vec<Vec<(String, f64)>> = Vec::new();
        let mut weights: Vec<f64> = Vec::new();

        // 向量检索
        if config.vector_weight > 0.0 {
            let vector_results = self.vector_search(query, candidates).await?;
            result_lists.push(vector_results);
            weights.push(config.vector_weight);
        }

        // BM25 检索
        if config.bm25_weight > 0.0 {
            let bm25_results = self.bm25_search(query, candidates);
            result_lists.push(bm25_results);
            weights.push(config.bm25_weight);
        }

        // 如果只有一个检索器，直接返回
        if result_lists.len() == 1 {
            let results = result_lists.remove(0);
            let final_results: Vec<RetrievalResult> = results
                .into_iter()
                .take(top_k)
                .filter_map(|(doc_id, score)| {
                    let (content, metadata) = self.get_document_content(&doc_id)?;
                    let source = metadata
                        .get("source")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    Some(RetrievalResult {
                        doc_id,
                        content,
                        score: score as f32,
                        metadata,
                        source,
                    })
                })
                .collect();

            return Ok(final_results);
        }

        // 融合结果
        let fused_results = if config.use_rrf {
            let rrf = ReciprocalRankFusion::new(config.rrf_k);
            rrf.fuse_with_weights(&result_lists, &weights, top_k)
        } else {
            // 简单加权融合
            let mut combined: HashMap<String, f64> = HashMap::new();
            for (results, weight) in result_lists.iter().zip(weights.iter()) {
                for (doc_id, score) in results {
                    *combined.entry(doc_id.clone()).or_insert(0.0) += score * weight;
                }
            }
            let mut fused: Vec<(String, f64)> = combined.into_iter().collect();
            fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            fused.truncate(top_k);
            fused
        };

        // 构建最终结果
        let final_results: Vec<RetrievalResult> = fused_results
            .into_iter()
            .filter_map(|(doc_id, score)| {
                let (content, metadata) = self.get_document_content(&doc_id)?;
                let source = metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                Some(RetrievalResult {
                    doc_id,
                    content,
                    score: score as f32,
                    metadata,
                    source,
                })
            })
            .collect();

        Ok(final_results)
    }

    async fn retrieve_with_filter(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<MetadataFilter>,
        config: Option<&HybridRetrieverConfig>,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let config = config.unwrap_or(&self.default_config);
        let candidates = top_k * config.candidate_multiplier * 2;

        // 获取更多候选结果
        let mut results = self.retrieve(query, candidates, Some(config)).await?;

        // 应用过滤器
        if let Some(f) = filter {
            results = results
                .into_iter()
                .filter(|r| {
                    // 简单的元数据匹配检查
                    f.must
                        .iter()
                        .all(|(key, value)| r.metadata.get(key).map_or(false, |v| v == value))
                })
                .collect();
        }

        // 应用分数阈值
        if config.min_score_threshold > 0.0 {
            results = results
                .into_iter()
                .filter(|r| r.score >= config.min_score_threshold as f32)
                .collect();
        }

        results.truncate(top_k);
        Ok(results)
    }

    async fn delete_documents(&self, doc_ids: &[String]) -> Layer3Result<bool> {
        // 从向量存储删除
        self.vector_store.delete_batch(doc_ids).await?;

        // 从 BM25 索引删除
        for doc_id in doc_ids {
            self.bm25_index.remove_document(doc_id);
        }

        // 从缓存删除
        {
            let mut cache = self.doc_cache.write();
            for doc_id in doc_ids {
                cache.remove(doc_id);
            }
        }

        Ok(true)
    }

    async fn clear(&self) -> Layer3Result<bool> {
        self.vector_store.clear().await?;
        self.bm25_index.clear();
        self.doc_cache.write().clear();
        Ok(true)
    }

    async fn count(&self) -> Layer3Result<usize> {
        Ok(self.bm25_index.doc_count())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retriever_engine::Layer1EmbeddingAdapter;
    use crate::vector_store::InMemoryVectorStore;

    /// 创建测试用的 Mock Embedding 模型
    /// 使用 Layer1 的 MockEmbeddingModel 通过适配器
    fn create_mock_embedding_model(dimension: usize) -> Layer1EmbeddingAdapter {
        Layer1EmbeddingAdapter::new(Box::new(sh_layer1::MockEmbeddingModel::new(dimension)))
    }

    #[test]
    fn test_bm25_index_basic() {
        let index = BM25Index::new();

        index.add_document("doc1".to_string(), "Rust is a systems programming language");
        index.add_document("doc2".to_string(), "Python is used for data science");
        index.add_document("doc3".to_string(), "JavaScript runs in the browser");

        let results = index.search("Rust programming", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "doc1");
    }

    #[test]
    fn test_bm25_index_scoring() {
        let index = BM25Index::new();

        index.add_document("doc1".to_string(), "machine learning algorithms");
        index.add_document("doc2".to_string(), "deep learning neural networks");
        index.add_document("doc3".to_string(), "database systems");

        let results = index.search("machine learning", 3);
        assert!(!results.is_empty());

        // doc1 和 doc2 都包含 "learning"，但 doc1 包含 "machine"
        assert!(results.iter().any(|(id, _)| id == "doc1"));
    }

    #[test]
    fn test_bm25_remove_document() {
        let index = BM25Index::new();

        index.add_document("doc1".to_string(), "test document");
        assert_eq!(index.doc_count(), 1);

        let removed = index.remove_document("doc1");
        assert!(removed);
        assert_eq!(index.doc_count(), 0);

        let removed = index.remove_document("nonexistent");
        assert!(!removed);
    }

    #[test]
    fn test_rrf_fusion() {
        let rrf = ReciprocalRankFusion::default_fusion();

        let list1 = vec![
            ("doc1".to_string(), 0.9),
            ("doc2".to_string(), 0.8),
            ("doc3".to_string(), 0.7),
        ];

        let list2 = vec![
            ("doc3".to_string(), 0.95),
            ("doc1".to_string(), 0.85),
            ("doc4".to_string(), 0.75),
        ];

        let fused = rrf.fuse(&[list1, list2], 5);

        assert!(!fused.is_empty());
        // doc1 和 doc3 都在两个列表中出现，应该排名靠前
        assert!(fused
            .iter()
            .take(2)
            .any(|(id, _)| id == "doc1" || id == "doc3"));
    }

    #[test]
    fn test_rrf_with_weights() {
        let rrf = ReciprocalRankFusion::new(60.0);

        let list1 = vec![("doc1".to_string(), 0.9)];
        let list2 = vec![("doc2".to_string(), 0.9)];

        let fused = rrf.fuse_with_weights(&[list1, list2], &[0.7, 0.3], 5);
        assert!(!fused.is_empty());
    }

    #[test]
    fn test_hybrid_retriever_config() {
        let config = HybridRetrieverConfig::new();
        assert_eq!(config.vector_weight, 0.7);
        assert_eq!(config.bm25_weight, 0.3);
        assert!(config.use_rrf);

        let vector_only = HybridRetrieverConfig::vector_only();
        assert_eq!(vector_only.vector_weight, 1.0);
        assert_eq!(vector_only.bm25_weight, 0.0);

        let balanced = HybridRetrieverConfig::balanced();
        assert_eq!(balanced.vector_weight, 0.5);
        assert_eq!(balanced.bm25_weight, 0.5);

        let custom = HybridRetrieverConfig::new().with_weights(0.8, 0.2);
        assert!((custom.vector_weight - 0.8).abs() < 0.001);
        assert!((custom.bm25_weight - 0.2).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_hybrid_retriever_index_and_search() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);

        let retriever = DefaultHybridRetriever::new(vector_store, embedding_model);

        let docs = vec![
            Document::new("Rust is a systems programming language"),
            Document::new("Python is widely used for data science"),
            Document::new("JavaScript runs in the browser"),
        ];

        let doc_ids = retriever.index_documents(docs).await.unwrap();
        assert_eq!(doc_ids.len(), 3);

        let results = retriever
            .retrieve("Rust programming", 5, None)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_retriever_with_config() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);

        let retriever = DefaultHybridRetriever::new(vector_store, embedding_model);

        retriever
            .index_documents(vec![
                Document::new("Machine learning algorithms use neural networks"),
                Document::new("Database stores data for applications"),
            ])
            .await
            .unwrap();

        // 测试仅向量检索
        let config = HybridRetrieverConfig::vector_only();
        let results = retriever
            .retrieve("neural networks", 5, Some(&config))
            .await
            .unwrap();
        assert!(!results.is_empty());

        // 测试仅 BM25 检索
        let config = HybridRetrieverConfig::bm25_only();
        let results = retriever
            .retrieve("machine learning", 5, Some(&config))
            .await
            .unwrap();
        assert!(!results.is_empty());

        // 测试均衡检索
        let config = HybridRetrieverConfig::balanced().with_rrf(true, 60.0);
        let results = retriever
            .retrieve("database", 5, Some(&config))
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_retriever_delete_and_count() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);

        let retriever = DefaultHybridRetriever::new(vector_store, embedding_model);

        let doc_ids = retriever
            .index_documents(vec![Document::new("Test document")])
            .await
            .unwrap();

        assert_eq!(retriever.count().await.unwrap(), 1);

        retriever.delete_documents(&doc_ids).await.unwrap();
        assert_eq!(retriever.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_hybrid_retriever_clear() {
        let vector_store = InMemoryVectorStore::in_memory();
        let embedding_model = create_mock_embedding_model(128);

        let retriever = DefaultHybridRetriever::new(vector_store, embedding_model);

        retriever
            .index_documents(vec![Document::new("Doc 1"), Document::new("Doc 2")])
            .await
            .unwrap();

        assert_eq!(retriever.count().await.unwrap(), 2);

        retriever.clear().await.unwrap();
        assert_eq!(retriever.count().await.unwrap(), 0);
    }
}
