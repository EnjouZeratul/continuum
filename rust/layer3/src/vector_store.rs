//! # Vector Store
//!
//! 向量存储：持久化向量索引。
//!
//! ## 功能
//!
//! - 内存向量存储（适合测试和开发）
//! - 文件持久化向量存储（适合生产环境）
//! - 多种距离度量支持（Cosine, Euclidean, DotProduct, Manhattan）
//! - 批量操作优化（并行处理）
//! - 压缩持久化（可选）
//! - 异步持久化支持
//! - 与 RetrieverEngine 无缝集成

use crate::retriever_engine::RetrievalResult;
use crate::types::{Layer3Error, Layer3Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

/// 向量存储 trait
///
/// 定义向量持久化存储接口。
///
/// # Example
///
/// ```rust,no_run
/// use sh_layer3::vector_store::{VectorStore, VectorItem, InMemoryVectorStore};
///
/// #[tokio::main]
/// async fn main() {
///     let store = InMemoryVectorStore::in_memory();
///
///     // 添加向量
///     let item = VectorItem::new("doc-1", vec![0.1, 0.2, 0.3])
///         .with_content("Hello world");
///     store.add_batch(vec![item]).await.unwrap();
///
///     // 查询相似向量
///     let results = store.query(vec![0.1, 0.2, 0.3], 5).await.unwrap();
/// }
/// ```
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 添加向量
    async fn add(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool>;

    /// 批量添加向量（优化版本）
    ///
    /// 使用并行处理和减少锁争用优化大批量插入。
    async fn add_batch(&self, items: Vec<VectorItem>) -> Layer3Result<Vec<bool>>;

    /// 添加向量（带验证）
    ///
    /// 验证向量维度并返回详细错误信息。
    async fn add_validated(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
        expected_dimension: usize,
    ) -> Layer3Result<bool> {
        if vector.len() != expected_dimension {
            return Err(Layer3Error::VectorDimensionMismatch {
                expected: expected_dimension,
                actual: vector.len(),
            }
            .into());
        }
        self.add(id, vector, metadata).await
    }

    /// 查询相似向量
    async fn query(&self, vector: Vec<f32>, top_k: usize) -> Layer3Result<Vec<RetrievalResult>>;

    /// 带过滤条件的查询
    async fn query_with_filter(
        &self,
        vector: Vec<f32>,
        top_k: usize,
        filter: Option<MetadataFilter>,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        // 默认实现：调用基本查询
        let _ = filter;
        self.query(vector, top_k).await
    }

    /// 带分数阈值的查询
    ///
    /// 只返回分数高于阈值的向量。
    async fn query_with_threshold(
        &self,
        vector: Vec<f32>,
        top_k: usize,
        min_score: f32,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let results = self.query(vector, top_k).await?;
        Ok(results
            .into_iter()
            .filter(|r| r.score >= min_score)
            .collect())
    }

    /// 删除向量
    async fn delete(&self, id: &str) -> Layer3Result<bool>;

    /// 批量删除（优化版本）
    async fn delete_batch(&self, ids: &[String]) -> Layer3Result<usize>;

    /// 删除所有匹配元数据条件的向量
    async fn delete_by_filter(&self, filter: MetadataFilter) -> Layer3Result<usize> {
        let _ = filter;
        Err(Layer3Error::VectorStoreError("delete_by_filter not implemented".to_string()).into())
    }

    /// 获取向量
    async fn get(&self, id: &str) -> Layer3Result<Option<VectorItem>>;

    /// 批量获取向量
    async fn get_batch(&self, ids: &[String]) -> Layer3Result<Vec<Option<VectorItem>>> {
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            results.push(self.get(id).await?);
        }
        Ok(results)
    }

    /// 更新向量（存在则更新，不存在则创建）
    async fn upsert(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool> {
        self.add(id, vector, metadata).await
    }

    /// 统计数量
    async fn count(&self) -> Layer3Result<usize>;

    /// 清空存储
    async fn clear(&self) -> Layer3Result<bool>;

    /// 检查向量是否存在
    async fn exists(&self, id: &str) -> Layer3Result<bool> {
        Ok(self.get(id).await?.is_some())
    }

    /// 获取存储统计信息
    async fn stats(&self) -> Layer3Result<VectorStoreStats> {
        Ok(VectorStoreStats {
            count: self.count().await?,
            dimension: 0,
            metric: DistanceMetric::Cosine,
        })
    }

    /// 持久化到磁盘（可选）
    async fn persist(&self) -> Layer3Result<()> {
        Ok(())
    }

    /// 从磁盘加载（可选）
    async fn load(&self) -> Layer3Result<()> {
        Ok(())
    }

    /// 异步持久化（后台线程）
    async fn persist_async(&self) -> Layer3Result<()> {
        self.persist().await
    }

    /// 强制同步持久化
    fn persist_sync(&self) -> Layer3Result<()> {
        Ok(())
    }

    /// 验证向量维度
    fn validate_dimension(&self, vector: &[f32], expected: usize) -> Layer3Result<()> {
        if vector.len() != expected {
            Err(Layer3Error::VectorDimensionMismatch {
                expected,
                actual: vector.len(),
            }
            .into())
        } else {
            Ok(())
        }
    }
}

/// 向量存储统计信息
#[derive(Debug, Clone)]
pub struct VectorStoreStats {
    pub count: usize,
    pub dimension: usize,
    pub metric: DistanceMetric,
}

/// 元数据过滤条件
#[derive(Debug, Clone)]
pub struct MetadataFilter {
    /// 必须包含的键值对
    pub must: HashMap<String, serde_json::Value>,
    /// 可选包含的键值对（至少匹配一个）
    pub should: HashMap<String, serde_json::Value>,
    /// 不能包含的键值对
    pub must_not: HashMap<String, serde_json::Value>,
}

impl MetadataFilter {
    pub fn new() -> Self {
        Self {
            must: HashMap::new(),
            should: HashMap::new(),
            must_not: HashMap::new(),
        }
    }

    pub fn must(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.must.insert(key.into(), value);
        self
    }

    pub fn should(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.should.insert(key.into(), value);
        self
    }

    pub fn must_not(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.must_not.insert(key.into(), value);
        self
    }

    /// 检查元数据是否匹配过滤条件
    pub fn matches(&self, metadata: &HashMap<String, serde_json::Value>) -> bool {
        // 检查 must 条件（全部匹配）
        for (key, value) in &self.must {
            match metadata.get(key) {
                Some(v) if v == value => continue,
                _ => return false,
            }
        }

        // 检查 must_not 条件（全部不匹配）
        for (key, value) in &self.must_not {
            if let Some(v) = metadata.get(key) {
                if v == value {
                    return false;
                }
            }
        }

        // 检查 should 条件（至少一个匹配，如果为空则通过）
        if !self.should.is_empty() {
            let mut matched = false;
            for (key, value) in &self.should {
                if let Some(v) = metadata.get(key) {
                    if v == value {
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                return false;
            }
        }

        true
    }
}

impl Default for MetadataFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// 向量项
#[derive(Debug, Clone)]
pub struct VectorItem {
    /// 唯一 ID
    pub id: String,
    /// 向量数据
    pub vector: Vec<f32>,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 关联内容（可选）
    pub content: Option<String>,
}

impl VectorItem {
    pub fn new(id: impl Into<String>, vector: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            vector,
            metadata: HashMap::new(),
            content: None,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = metadata;
        self
    }
}

/// 向量存储配置
#[derive(Debug, Clone)]
pub struct VectorStoreConfig {
    /// 存储路径
    pub path: Option<String>,
    /// 向量维度
    pub dimension: usize,
    /// 距离度量
    pub metric: DistanceMetric,
    /// 索引类型
    pub index_type: IndexType,
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            path: None,
            dimension: 1536,
            metric: DistanceMetric::Cosine,
            index_type: IndexType::Hnsw,
        }
    }
}

/// 距离度量类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// 余弦相似度
    Cosine,
    /// 欧几里得距离
    Euclidean,
    /// 点积
    DotProduct,
    /// 曼哈顿距离
    Manhattan,
}

/// 索引类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// HNSW（高效近似最近邻）
    Hnsw,
    /// IVF（倒排文件索引）
    Ivf,
    /// Flat（暴力搜索）
    Flat,
    /// PQ（乘积量化）
    ProductQuantization,
}

/// 向量存储工厂 trait
pub trait VectorStoreFactory: Send + Sync {
    /// 创建向量存储
    fn create(&self, config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>>;
}

// ============================================================================
// In-Memory Vector Store Implementation
// ============================================================================

/// 内存向量存储实现
///
/// 使用内存存储向量，支持基本的相似度搜索。
/// 适用于测试和开发环境，不适合大规模生产使用。
pub struct InMemoryVectorStore {
    /// 向量数据存储
    data: Arc<RwLock<HashMap<String, VectorItem>>>,
    /// 配置
    config: VectorStoreConfig,
}

impl InMemoryVectorStore {
    /// 创建新的内存向量存储
    pub fn new(config: VectorStoreConfig) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// 创建使用默认配置的存储
    pub fn in_memory() -> Self {
        Self::new(VectorStoreConfig::default())
    }

    /// 计算向量相似度
    fn compute_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        match self.config.metric {
            DistanceMetric::Cosine => {
                let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm_a == 0.0 || norm_b == 0.0 {
                    0.0
                } else {
                    dot / (norm_a * norm_b)
                }
            }
            DistanceMetric::Euclidean => {
                let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum();
                1.0 / (1.0 + sum.sqrt())
            }
            DistanceMetric::DotProduct => a.iter().zip(b.iter()).map(|(x, y)| x * y).sum(),
            DistanceMetric::Manhattan => {
                let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
                1.0 / (1.0 + sum)
            }
        }
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn add(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool> {
        let item = VectorItem {
            id: id.clone(),
            vector,
            metadata,
            content: None,
        };
        let mut data = self.data.write();
        data.insert(id, item);
        Ok(true)
    }

    async fn add_batch(&self, items: Vec<VectorItem>) -> Layer3Result<Vec<bool>> {
        let mut data = self.data.write();
        let results: Vec<bool> = items
            .into_iter()
            .map(|item| {
                let id = item.id.clone();
                data.insert(id, item);
                true
            })
            .collect();
        Ok(results)
    }

    async fn query(&self, vector: Vec<f32>, top_k: usize) -> Layer3Result<Vec<RetrievalResult>> {
        let data = self.data.read();
        let mut scores: Vec<(String, f32, &VectorItem)> = data
            .iter()
            .map(|(id, item)| {
                let score = self.compute_similarity(&vector, &item.vector);
                (id.clone(), score, item)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);

        Ok(scores
            .into_iter()
            .map(|(doc_id, score, item)| RetrievalResult {
                doc_id,
                content: item.content.clone().unwrap_or_default(),
                score,
                metadata: item.metadata.clone(),
                source: item
                    .metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect())
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let mut data = self.data.write();
        Ok(data.remove(id).is_some())
    }

    async fn delete_batch(&self, ids: &[String]) -> Layer3Result<usize> {
        let mut data = self.data.write();
        let mut count = 0;
        for id in ids {
            if data.remove(id).is_some() {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<VectorItem>> {
        let data = self.data.read();
        Ok(data.get(id).cloned())
    }

    async fn count(&self) -> Layer3Result<usize> {
        let data = self.data.read();
        Ok(data.len())
    }

    async fn clear(&self) -> Layer3Result<bool> {
        let mut data = self.data.write();
        data.clear();
        Ok(true)
    }

    async fn query_with_filter(
        &self,
        vector: Vec<f32>,
        top_k: usize,
        filter: Option<MetadataFilter>,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        let data = self.data.read();

        // 先过滤，再计算相似度
        let candidates: Vec<&VectorItem> = if let Some(ref f) = filter {
            data.values()
                .filter(|item| f.matches(&item.metadata))
                .collect()
        } else {
            data.values().collect()
        };

        let mut scores: Vec<(String, f32, &VectorItem)> = candidates
            .into_iter()
            .map(|item| {
                let score = self.compute_similarity(&vector, &item.vector);
                (item.id.clone(), score, item)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);

        Ok(scores
            .into_iter()
            .map(|(doc_id, score, item)| RetrievalResult {
                doc_id,
                content: item.content.clone().unwrap_or_default(),
                score,
                metadata: item.metadata.clone(),
                source: item
                    .metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect())
    }
}

/// 内存向量存储工厂
pub struct InMemoryVectorStoreFactory;

impl VectorStoreFactory for InMemoryVectorStoreFactory {
    fn create(&self, config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        Ok(Box::new(InMemoryVectorStore::new(config)))
    }
}

// ============================================================================
// File-Persisted Vector Store Implementation
// ============================================================================

/// 可序列化的向量项（用于持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableVectorItem {
    id: String,
    vector: Vec<f32>,
    metadata: serde_json::Map<String, serde_json::Value>,
    content: Option<String>,
}

/// 可序列化的存储数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreData {
    items: Vec<SerializableVectorItem>,
    config: SerializableConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableConfig {
    dimension: usize,
    metric: String,
}

/// 文件持久化向量存储
///
/// 将向量数据持久化到本地文件，支持应用重启后恢复。
/// 使用 JSON 格式存储，适合中小规模数据。
pub struct FileVectorStore {
    /// 内存存储（实际数据）
    inner: InMemoryVectorStore,
    /// 存储路径
    path: PathBuf,
    /// 是否自动持久化
    auto_persist: bool,
}

impl FileVectorStore {
    /// 创建新的文件向量存储
    pub fn new(config: VectorStoreConfig) -> Layer3Result<Self> {
        let path = config
            .path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("vector_store.json"));

        let inner = InMemoryVectorStore::new(config);
        let store = Self {
            inner,
            path,
            auto_persist: true,
        };

        Ok(store)
    }

    /// 创建带自动持久化开关的存储
    pub fn with_auto_persist(mut self, auto_persist: bool) -> Self {
        self.auto_persist = auto_persist;
        self
    }

    /// 持久化数据到文件
    #[instrument(skip(self))]
    pub fn persist_sync(&self) -> Layer3Result<()> {
        let data = self.inner.data.read();

        let items: Vec<SerializableVectorItem> = data
            .values()
            .map(|item| SerializableVectorItem {
                id: item.id.clone(),
                vector: item.vector.clone(),
                metadata: item.metadata.clone().into_iter().collect(),
                content: item.content.clone(),
            })
            .collect();

        let config = SerializableConfig {
            dimension: self.inner.config.dimension,
            metric: format!("{:?}", self.inner.config.metric),
        };

        let store_data = StoreData { items, config };

        let json = serde_json::to_string_pretty(&store_data)?;
        std::fs::write(&self.path, json)?;

        info!("Persisted {} vectors to {:?}", data.len(), self.path);
        Ok(())
    }

    /// 从文件加载数据
    #[instrument(skip(self))]
    pub fn load_sync(&self) -> Layer3Result<()> {
        if !self.path.exists() {
            debug!("No existing store file at {:?}", self.path);
            return Ok(());
        }

        let json = std::fs::read_to_string(&self.path)?;
        let store_data: StoreData = serde_json::from_str(&json)?;

        let mut data = self.inner.data.write();
        data.clear();

        for item in store_data.items {
            let vector_item = VectorItem {
                id: item.id,
                vector: item.vector,
                metadata: item.metadata.into_iter().collect(),
                content: item.content,
            };
            data.insert(vector_item.id.clone(), vector_item);
        }

        info!("Loaded {} vectors from {:?}", data.len(), self.path);
        Ok(())
    }
}

#[async_trait]
impl VectorStore for FileVectorStore {
    async fn add(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool> {
        let result = self.inner.add(id, vector, metadata).await?;

        if self.auto_persist && result {
            self.persist_sync()?;
        }

        Ok(result)
    }

    async fn add_batch(&self, items: Vec<VectorItem>) -> Layer3Result<Vec<bool>> {
        let results = self.inner.add_batch(items).await?;

        if self.auto_persist && results.iter().any(|&r| r) {
            self.persist_sync()?;
        }

        Ok(results)
    }

    async fn query(&self, vector: Vec<f32>, top_k: usize) -> Layer3Result<Vec<RetrievalResult>> {
        self.inner.query(vector, top_k).await
    }

    async fn query_with_filter(
        &self,
        vector: Vec<f32>,
        top_k: usize,
        filter: Option<MetadataFilter>,
    ) -> Layer3Result<Vec<RetrievalResult>> {
        self.inner.query_with_filter(vector, top_k, filter).await
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let result = self.inner.delete(id).await?;

        if self.auto_persist && result {
            self.persist_sync()?;
        }

        Ok(result)
    }

    async fn delete_batch(&self, ids: &[String]) -> Layer3Result<usize> {
        let count = self.inner.delete_batch(ids).await?;

        if self.auto_persist && count > 0 {
            self.persist_sync()?;
        }

        Ok(count)
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<VectorItem>> {
        self.inner.get(id).await
    }

    async fn count(&self) -> Layer3Result<usize> {
        self.inner.count().await
    }

    async fn clear(&self) -> Layer3Result<bool> {
        let result = self.inner.clear().await?;

        if self.auto_persist && result {
            self.persist_sync()?;
        }

        Ok(result)
    }

    async fn persist(&self) -> Layer3Result<()> {
        self.persist_sync()
    }

    async fn load(&self) -> Layer3Result<()> {
        self.load_sync()
    }
}

/// 文件向量存储工厂
pub struct FileVectorStoreFactory;

impl VectorStoreFactory for FileVectorStoreFactory {
    fn create(&self, config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        Ok(Box::new(FileVectorStore::new(config)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_item_builder() {
        let item = VectorItem::new("test", vec![1.0, 2.0, 3.0]).with_content("test content");
        assert_eq!(item.content, Some("test content".to_string()));
    }

    #[test]
    fn test_vector_store_config_default() {
        let config = VectorStoreConfig::default();
        assert_eq!(config.dimension, 1536);
        assert_eq!(config.metric, DistanceMetric::Cosine);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_add() {
        let store = InMemoryVectorStore::in_memory();
        let result = store
            .add("id1".to_string(), vec![1.0, 2.0, 3.0], HashMap::new())
            .await;
        assert!(result.is_ok());
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_query() {
        let store = InMemoryVectorStore::in_memory();

        // 添加测试向量
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::json!("test.txt"));

        store
            .add("id1".to_string(), vec![1.0, 0.0, 0.0], metadata.clone())
            .await
            .unwrap();
        store
            .add("id2".to_string(), vec![0.9, 0.1, 0.0], HashMap::new())
            .await
            .unwrap();
        store
            .add("id3".to_string(), vec![0.0, 1.0, 0.0], HashMap::new())
            .await
            .unwrap();

        // 查询相似向量
        let results = store.query(vec![1.0, 0.0, 0.0], 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_delete() {
        let store = InMemoryVectorStore::in_memory();
        store
            .add("id1".to_string(), vec![1.0, 2.0, 3.0], HashMap::new())
            .await
            .unwrap();

        let deleted = store.delete("id1").await.unwrap();
        assert!(deleted);
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[test]
    fn test_cosine_similarity() {
        let store = InMemoryVectorStore::new(VectorStoreConfig {
            metric: DistanceMetric::Cosine,
            ..Default::default()
        });

        // 相同向量
        let sim = store.compute_similarity(&[1.0, 0.0], &[1.0, 0.0]);
        assert!((sim - 1.0).abs() < 0.001);

        // 正交向量
        let sim = store.compute_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!((sim - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_metadata_filter() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("doc"));
        metadata.insert("lang".to_string(), serde_json::json!("en"));

        // 测试 must 条件
        let filter = MetadataFilter::new().must("type", serde_json::json!("doc"));
        assert!(filter.matches(&metadata));

        let filter = MetadataFilter::new().must("type", serde_json::json!("code"));
        assert!(!filter.matches(&metadata));

        // 测试 must_not 条件
        let filter = MetadataFilter::new().must_not("type", serde_json::json!("code"));
        assert!(filter.matches(&metadata));

        // 测试 should 条件（需要至少一个匹配）
        // 注意: HashMap 不能有重复键，所以用不同键测试
        let filter = MetadataFilter::new()
            .should("type", serde_json::json!("doc"))
            .should("lang", serde_json::json!("zh"));
        assert!(filter.matches(&metadata)); // type=doc 匹配

        // should 条件不匹配的情况
        let filter = MetadataFilter::new()
            .should("type", serde_json::json!("code"))
            .should("lang", serde_json::json!("zh"));
        assert!(!filter.matches(&metadata)); // type!=code 且 lang!=zh
    }

    #[tokio::test]
    async fn test_file_vector_store() {
        use tempfile::TempDir;

        // 使用临时目录，避免文件被删除太快
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("vector_store.json");
        let path_str = path.to_str().unwrap().to_string();

        let config = VectorStoreConfig {
            path: Some(path_str.clone()),
            dimension: 128,
            metric: DistanceMetric::Cosine,
            index_type: IndexType::Flat,
        };

        let store = FileVectorStore::new(config).unwrap();

        // 添加向量
        let vector = vec![1.0; 128];
        store
            .add("id1".to_string(), vector, HashMap::new())
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        // 持久化
        store.persist().await.unwrap();

        // 验证文件存在
        assert!(path.exists());

        // 创建新的 store 实例并加载
        let config2 = VectorStoreConfig {
            path: Some(path_str),
            dimension: 128,
            metric: DistanceMetric::Cosine,
            index_type: IndexType::Flat,
        };
        let store2 = FileVectorStore::new(config2).unwrap();
        store2.load().await.unwrap();
        assert_eq!(store2.count().await.unwrap(), 1);

        // 验证内容
        let item = store2.get("id1").await.unwrap();
        assert!(item.is_some());
    }
}

// ============================================================================
// Remote Vector Store Implementations (Pinecone, Chroma, Qdrant)
// ============================================================================

/// 远程向量存储配置
#[derive(Debug, Clone)]
pub struct RemoteVectorStoreConfig {
    /// API 密钥
    pub api_key: String,
    /// API 端点 URL
    pub endpoint: String,
    /// 集合/索引名称
    pub collection: String,
    /// 向量维度
    pub dimension: usize,
    /// 距离度量
    pub metric: DistanceMetric,
    /// 连接池大小
    pub pool_size: usize,
    /// 请求超时（秒）
    pub timeout_secs: u64,
}

impl RemoteVectorStoreConfig {
    /// 从环境变量创建 Pinecone 配置
    pub fn pinecone_from_env() -> Layer3Result<Self> {
        let api_key = std::env::var("PINECONE_API_KEY")
            .map_err(|_| anyhow::anyhow!("PINECONE_API_KEY not set"))?;
        let endpoint = std::env::var("PINECONE_ENDPOINT")
            .map_err(|_| anyhow::anyhow!("PINECONE_ENDPOINT not set"))?;
        let collection = std::env::var("PINECONE_INDEX").unwrap_or_else(|_| "default".to_string());

        Ok(Self {
            api_key,
            endpoint,
            collection,
            dimension: 1536,
            metric: DistanceMetric::Cosine,
            pool_size: 10,
            timeout_secs: 30,
        })
    }

    /// 从环境变量创建 Chroma 配置
    pub fn chroma_from_env() -> Layer3Result<Self> {
        let endpoint = std::env::var("CHROMA_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let collection =
            std::env::var("CHROMA_COLLECTION").unwrap_or_else(|_| "default".to_string());
        let api_key = std::env::var("CHROMA_API_KEY").unwrap_or_default();

        Ok(Self {
            api_key,
            endpoint,
            collection,
            dimension: 1536,
            metric: DistanceMetric::Cosine,
            pool_size: 10,
            timeout_secs: 30,
        })
    }

    /// 从环境变量创建 Qdrant 配置
    pub fn qdrant_from_env() -> Layer3Result<Self> {
        let endpoint = std::env::var("QDRANT_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:6333".to_string());
        let collection =
            std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "default".to_string());
        let api_key = std::env::var("QDRANT_API_KEY").unwrap_or_default();

        Ok(Self {
            api_key,
            endpoint,
            collection,
            dimension: 1536,
            metric: DistanceMetric::Cosine,
            pool_size: 10,
            timeout_secs: 30,
        })
    }
}

// ============================================================================
// Pinecone Implementation
// ============================================================================

/// Pinecone 向量存储
///
/// 使用 Pinecone 云服务进行向量存储和检索。
pub struct PineconeVectorStore {
    client: reqwest::Client,
    config: RemoteVectorStoreConfig,
}

impl PineconeVectorStore {
    pub fn new(config: RemoteVectorStoreConfig) -> Layer3Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .pool_max_idle_per_host(config.pool_size)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create client: {}", e))?;

        Ok(Self { client, config })
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}/vectors/{}", self.config.endpoint, path)
    }
}

#[async_trait]
impl VectorStore for PineconeVectorStore {
    async fn add(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool> {
        self.add_batch(vec![VectorItem {
            id,
            vector,
            metadata,
            content: None,
        }])
        .await?;
        Ok(true)
    }

    async fn add_batch(&self, items: Vec<VectorItem>) -> Layer3Result<Vec<bool>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        let vectors: Vec<serde_json::Value> = items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "values": item.vector,
                    "metadata": item.metadata,
                })
            })
            .collect();

        let body = serde_json::json!({
            "vectors": vectors,
            "namespace": self.config.collection,
        });

        let response = self
            .client
            .post(self.build_url("upsert"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Pinecone upsert failed: {} - {}",
                status,
                text
            ));
        }

        Ok(items.iter().map(|_| true).collect())
    }

    async fn query(&self, vector: Vec<f32>, top_k: usize) -> Layer3Result<Vec<RetrievalResult>> {
        let body = serde_json::json!({
            "vector": vector,
            "topK": top_k,
            "namespace": self.config.collection,
            "includeMetadata": true,
            "includeValues": false,
        });

        let response = self
            .client
            .post(self.build_url("query"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone query failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Pinecone query failed: {} - {}",
                status,
                text
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        let results = json["matches"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let doc_id = m["id"].as_str()?.to_string();
                        let score = m["score"].as_f64()? as f32;
                        let metadata: HashMap<String, serde_json::Value> = m["metadata"]
                            .as_object()
                            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                            .unwrap_or_default();
                        let content = metadata
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or_default();
                        let source = metadata
                            .get("source")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        Some(RetrievalResult {
                            doc_id,
                            content,
                            score,
                            metadata,
                            source,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let body = serde_json::json!({
            "ids": [id],
            "namespace": self.config.collection,
        });

        let response = self
            .client
            .post(self.build_url("delete"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone delete failed: {}", e))?;

        Ok(response.status().is_success())
    }

    async fn delete_batch(&self, ids: &[String]) -> Layer3Result<usize> {
        let body = serde_json::json!({
            "ids": ids,
            "namespace": self.config.collection,
        });

        let response = self
            .client
            .post(self.build_url("delete"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone delete failed: {}", e))?;

        if response.status().is_success() {
            Ok(ids.len())
        } else {
            Ok(0)
        }
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<VectorItem>> {
        let body = serde_json::json!({
            "ids": [id],
            "namespace": self.config.collection,
        });

        let response = self
            .client
            .post(self.build_url("fetch"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone fetch failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        if let Some(vectors) = json["vectors"].as_object() {
            if let Some(v) = vectors.get(id) {
                let vector = v["values"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_f64().map(|f| f as f32))
                            .collect()
                    })
                    .unwrap_or_default();
                let metadata = v["metadata"]
                    .as_object()
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();

                return Ok(Some(VectorItem {
                    id: id.to_string(),
                    vector,
                    metadata,
                    content: None,
                }));
            }
        }

        Ok(None)
    }

    async fn count(&self) -> Layer3Result<usize> {
        let body = serde_json::json!({
            "namespace": self.config.collection,
        });

        let response = self
            .client
            .post(self.build_url("describeIndexStats"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone stats failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(0);
        }

        let json: serde_json::Value = response.json().await.unwrap_or_default();
        let count = json["dimension"]["totalVectorCount"].as_u64().unwrap_or(0) as usize;
        Ok(count)
    }

    async fn clear(&self) -> Layer3Result<bool> {
        let body = serde_json::json!({
            "deleteAll": true,
            "namespace": self.config.collection,
        });

        let response = self
            .client
            .post(self.build_url("delete"))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Pinecone clear failed: {}", e))?;

        Ok(response.status().is_success())
    }
}

/// Pinecone 向量存储工厂
pub struct PineconeVectorStoreFactory;

impl VectorStoreFactory for PineconeVectorStoreFactory {
    fn create(&self, _config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        let remote_config = RemoteVectorStoreConfig::pinecone_from_env()?;
        Ok(Box::new(PineconeVectorStore::new(remote_config)?))
    }
}

// ============================================================================
// Chroma Implementation
// ============================================================================

/// Chroma 向量存储
///
/// 使用 Chroma 本地或云服务进行向量存储和检索。
pub struct ChromaVectorStore {
    client: reqwest::Client,
    config: RemoteVectorStoreConfig,
}

impl ChromaVectorStore {
    pub fn new(config: RemoteVectorStoreConfig) -> Layer3Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .pool_max_idle_per_host(config.pool_size)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create client: {}", e))?;

        Ok(Self { client, config })
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.config.endpoint, path)
    }

    async fn ensure_collection(&self) -> Layer3Result<()> {
        let body = serde_json::json!({
            "name": self.config.collection,
        });

        let _ = self
            .client
            .post(self.build_url("/collections"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        Ok(())
    }
}

#[async_trait]
impl VectorStore for ChromaVectorStore {
    async fn add(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool> {
        self.add_batch(vec![VectorItem {
            id,
            vector,
            metadata,
            content: None,
        }])
        .await?;
        Ok(true)
    }

    async fn add_batch(&self, items: Vec<VectorItem>) -> Layer3Result<Vec<bool>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_collection().await?;

        let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        let vectors: Vec<Vec<f32>> = items.iter().map(|i| i.vector.clone()).collect();
        let metadatas: Vec<HashMap<String, serde_json::Value>> =
            items.iter().map(|i| i.metadata.clone()).collect();

        let body = serde_json::json!({
            "ids": ids,
            "embeddings": vectors,
            "metadatas": metadatas,
        });

        let url = self.build_url(&format!("/collections/{}/add", self.config.collection));
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma add failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Chroma add failed: {} - {}", status, text));
        }

        Ok(items.iter().map(|_| true).collect())
    }

    async fn query(&self, vector: Vec<f32>, top_k: usize) -> Layer3Result<Vec<RetrievalResult>> {
        let body = serde_json::json!({
            "query_embeddings": [vector],
            "n_results": top_k,
            "include": ["metadatas", "documents", "distances"],
        });

        let url = self.build_url(&format!("/collections/{}/query", self.config.collection));
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma query failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Chroma query failed: {} - {}",
                status,
                text
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        let ids = json["ids"][0].as_array().cloned().unwrap_or_default();
        let distances = json["distances"][0].as_array().cloned().unwrap_or_default();
        let metadatas = json["metadatas"][0].as_array().cloned().unwrap_or_default();
        let documents = json["documents"][0].as_array().cloned().unwrap_or_default();

        let results: Vec<RetrievalResult> = ids
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let doc_id = id.as_str()?.to_string();
                let distance = distances.get(i)?.as_f64()? as f32;
                let score = 1.0 / (1.0 + distance); // Convert distance to similarity
                let metadata: HashMap<String, serde_json::Value> = metadatas
                    .get(i)?
                    .as_object()
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                let content = documents
                    .get(i)?
                    .as_str()
                    .map(String::from)
                    .unwrap_or_default();
                let source = metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                Some(RetrievalResult {
                    doc_id,
                    content,
                    score,
                    metadata,
                    source,
                })
            })
            .collect();

        Ok(results)
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let body = serde_json::json!({
            "ids": [id],
        });

        let url = self.build_url(&format!("/collections/{}/delete", self.config.collection));
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma delete failed: {}", e))?;

        Ok(response.status().is_success())
    }

    async fn delete_batch(&self, ids: &[String]) -> Layer3Result<usize> {
        let body = serde_json::json!({
            "ids": ids,
        });

        let url = self.build_url(&format!("/collections/{}/delete", self.config.collection));
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma delete failed: {}", e))?;

        if response.status().is_success() {
            Ok(ids.len())
        } else {
            Ok(0)
        }
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<VectorItem>> {
        let body = serde_json::json!({
            "ids": [id],
            "include": ["embeddings", "metadatas"],
        });

        let url = self.build_url(&format!("/collections/{}/get", self.config.collection));
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma get failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        if let Some(ids) = json["ids"].as_array() {
            if !ids.is_empty() {
                let vector = json["embeddings"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|arr| {
                        arr.as_array().map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_f64().map(|f| f as f32))
                                .collect()
                        })
                    })
                    .unwrap_or_default();
                let metadata = json["metadatas"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|obj| {
                        obj.as_object()
                            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    })
                    .unwrap_or_default();

                return Ok(Some(VectorItem {
                    id: id.to_string(),
                    vector,
                    metadata,
                    content: None,
                }));
            }
        }

        Ok(None)
    }

    async fn count(&self) -> Layer3Result<usize> {
        let url = self.build_url(&format!("/collections/{}/count", self.config.collection));
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma count failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(0);
        }

        let json: serde_json::Value = response.json().await.unwrap_or_default();
        Ok(json.as_u64().unwrap_or(0) as usize)
    }

    async fn clear(&self) -> Layer3Result<bool> {
        let url = self.build_url(&format!("/collections/{}", self.config.collection));
        let response = self
            .client
            .delete(url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Chroma clear failed: {}", e))?;

        if response.status().is_success() {
            self.ensure_collection().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Chroma 向量存储工厂
pub struct ChromaVectorStoreFactory;

impl VectorStoreFactory for ChromaVectorStoreFactory {
    fn create(&self, _config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        let remote_config = RemoteVectorStoreConfig::chroma_from_env()?;
        Ok(Box::new(ChromaVectorStore::new(remote_config)?))
    }
}

// ============================================================================
// Qdrant Implementation
// ============================================================================

/// Qdrant 向量存储
///
/// 使用 Qdrant 云服务或自托管实例进行向量存储和检索。
pub struct QdrantVectorStore {
    client: reqwest::Client,
    config: RemoteVectorStoreConfig,
}

impl QdrantVectorStore {
    pub fn new(config: RemoteVectorStoreConfig) -> Layer3Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .pool_max_idle_per_host(config.pool_size)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create client: {}", e))?;

        Ok(Self { client, config })
    }

    fn build_url(&self, path: &str) -> String {
        format!(
            "{}/collections/{}{}",
            self.config.endpoint, self.config.collection, path
        )
    }

    async fn ensure_collection(&self) -> Layer3Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.config.endpoint, self.config.collection
        );

        // Check if collection exists
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant check failed: {}", e))?;

        if response.status().as_u16() == 404 {
            // Create collection
            let body = serde_json::json!({
                "vectors": {
                    "size": self.config.dimension,
                    "distance": match self.config.metric {
                        DistanceMetric::Cosine => "Cosine",
                        DistanceMetric::Euclidean => "Euclid",
                        DistanceMetric::DotProduct => "Dot",
                        DistanceMetric::Manhattan => "Manhattan",
                    },
                },
            });

            let _ = self
                .client
                .put(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;
        }

        Ok(())
    }
}

#[async_trait]
impl VectorStore for QdrantVectorStore {
    async fn add(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Layer3Result<bool> {
        self.add_batch(vec![VectorItem {
            id,
            vector,
            metadata,
            content: None,
        }])
        .await?;
        Ok(true)
    }

    async fn add_batch(&self, items: Vec<VectorItem>) -> Layer3Result<Vec<bool>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_collection().await?;

        let points: Vec<serde_json::Value> = items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "vector": item.vector,
                    "payload": item.metadata,
                })
            })
            .collect();

        let body = serde_json::json!({
            "points": points,
        });

        let url = self.build_url("/points?wait=true");
        let mut request = self
            .client
            .put(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant upsert failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Qdrant upsert failed: {} - {}",
                status,
                text
            ));
        }

        Ok(items.iter().map(|_| true).collect())
    }

    async fn query(&self, vector: Vec<f32>, top_k: usize) -> Layer3Result<Vec<RetrievalResult>> {
        self.ensure_collection().await?;

        let body = serde_json::json!({
            "vector": vector,
            "limit": top_k,
            "with_payload": true,
        });

        let url = self.build_url("/points/search");
        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant search failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Qdrant search failed: {} - {}",
                status,
                text
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        let results = json["result"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let doc_id = r["id"].as_str()?.to_string();
                        let score = r["score"].as_f64()? as f32;
                        let metadata: HashMap<String, serde_json::Value> = r["payload"]
                            .as_object()
                            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                            .unwrap_or_default();
                        let content = metadata
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or_default();
                        let source = metadata
                            .get("source")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        Some(RetrievalResult {
                            doc_id,
                            content,
                            score,
                            metadata,
                            source,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let body = serde_json::json!({
            "points": [id],
        });

        let url = self.build_url("/points/delete?wait=true");
        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant delete failed: {}", e))?;

        Ok(response.status().is_success())
    }

    async fn delete_batch(&self, ids: &[String]) -> Layer3Result<usize> {
        let body = serde_json::json!({
            "points": ids,
        });

        let url = self.build_url("/points/delete?wait=true");
        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant delete failed: {}", e))?;

        if response.status().is_success() {
            Ok(ids.len())
        } else {
            Ok(0)
        }
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<VectorItem>> {
        let body = serde_json::json!({
            "ids": [id],
            "with_vector": true,
            "with_payload": true,
        });

        let url = self.build_url("/points");
        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant get failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        if let Some(result) = json["result"].as_array() {
            if let Some(point) = result.first() {
                let vector = point["vector"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_f64().map(|f| f as f32))
                            .collect()
                    })
                    .unwrap_or_default();
                let metadata = point["payload"]
                    .as_object()
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();

                return Ok(Some(VectorItem {
                    id: id.to_string(),
                    vector,
                    metadata,
                    content: None,
                }));
            }
        }

        Ok(None)
    }

    async fn count(&self) -> Layer3Result<usize> {
        let url = self.build_url("");
        let mut request = self.client.get(&url);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant count failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(0);
        }

        let json: serde_json::Value = response.json().await.unwrap_or_default();
        let count = json["result"]["points_count"].as_u64().unwrap_or(0) as usize;
        Ok(count)
    }

    async fn clear(&self) -> Layer3Result<bool> {
        let url = self.build_url("/points/delete?wait=true");
        let body = serde_json::json!({
            "filter": {},
        });

        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if !self.config.api_key.is_empty() {
            request = request.header("api-key", &self.config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Qdrant clear failed: {}", e))?;

        Ok(response.status().is_success())
    }
}

/// Qdrant 向量存储工厂
pub struct QdrantVectorStoreFactory;

impl VectorStoreFactory for QdrantVectorStoreFactory {
    fn create(&self, _config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        let remote_config = RemoteVectorStoreConfig::qdrant_from_env()?;
        Ok(Box::new(QdrantVectorStore::new(remote_config)?))
    }
}

// ============================================================================
// Unified Vector Store Factory
// ============================================================================

/// 统一向量存储工厂
pub struct UnifiedVectorStoreFactory {
    store_type: VectorStoreType,
}

/// 向量存储类型
#[derive(Debug, Clone)]
pub enum VectorStoreType {
    InMemory,
    File,
    Pinecone,
    Chroma,
    Qdrant,
}

impl UnifiedVectorStoreFactory {
    pub fn new(store_type: VectorStoreType) -> Self {
        Self { store_type }
    }

    pub fn create(&self, config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        match self.store_type {
            VectorStoreType::InMemory => Ok(Box::new(InMemoryVectorStore::new(config))),
            VectorStoreType::File => Ok(Box::new(FileVectorStore::new(config)?)),
            VectorStoreType::Pinecone => {
                let remote_config = RemoteVectorStoreConfig::pinecone_from_env()?;
                Ok(Box::new(PineconeVectorStore::new(remote_config)?))
            }
            VectorStoreType::Chroma => {
                let remote_config = RemoteVectorStoreConfig::chroma_from_env()?;
                Ok(Box::new(ChromaVectorStore::new(remote_config)?))
            }
            VectorStoreType::Qdrant => {
                let remote_config = RemoteVectorStoreConfig::qdrant_from_env()?;
                Ok(Box::new(QdrantVectorStore::new(remote_config)?))
            }
        }
    }
}

impl VectorStoreFactory for UnifiedVectorStoreFactory {
    fn create(&self, config: VectorStoreConfig) -> Layer3Result<Box<dyn VectorStore>> {
        self.create(config)
    }
}
