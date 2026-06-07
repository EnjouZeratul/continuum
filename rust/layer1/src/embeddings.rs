//! 嵌入模型模块
//!
//! 文本嵌入、批量处理、缓存。
//!
//! 支持多种嵌入模型提供商：
//! - OpenAI Embeddings API
//! - HuggingFace Inference API
//! - Cohere Embed API
//! - 本地 SentenceTransformers 模型

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ============================================================================
// 常量定义
// ============================================================================

/// 默认嵌入模型
pub const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-ada-002";

/// 默认嵌入维度
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 1536;

/// 缓存默认 TTL（秒）
pub const DEFAULT_CACHE_TTL_SECS: u64 = 3600;

/// 缓存默认最大条目数
pub const DEFAULT_CACHE_MAX_ENTRIES: usize = 10000;

// ============================================================================
// 统一 EmbeddingModel Trait
// ============================================================================

/// 嵌入模型统一接口
///
/// 所有嵌入模型必须实现此 trait。
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// 生成单个文本的嵌入向量
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// 批量生成嵌入向量
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// 获取向量维度
    fn dimension(&self) -> usize;

    /// 获取模型名称
    fn model_name(&self) -> &str;

    /// 获取提供商名称
    fn provider(&self) -> &str;
}

// ============================================================================
// 嵌入缓存
// ============================================================================

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 嵌入向量
    embedding: Vec<f32>,
    /// 创建时间
    created_at: Instant,
    /// 访问计数
    access_count: usize,
}

/// 嵌入缓存
///
/// 使用 LRU 策略管理缓存条目。
#[derive(Debug)]
pub struct EmbeddingCache {
    /// 缓存存储
    store: RwLock<HashMap<String, CacheEntry>>,
    /// 最大条目数
    max_entries: usize,
    /// TTL（秒）
    ttl_secs: u64,
}

impl EmbeddingCache {
    /// 创建新的缓存实例
    pub fn new(max_entries: usize, ttl_secs: u64) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            max_entries,
            ttl_secs,
        }
    }

    /// 使用默认配置创建缓存
    pub fn default_cache() -> Self {
        Self::new(DEFAULT_CACHE_MAX_ENTRIES, DEFAULT_CACHE_TTL_SECS)
    }

    /// 生成缓存键
    fn cache_key(provider: &str, model: &str, text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        provider.hash(&mut hasher);
        model.hash(&mut hasher);
        text.hash(&mut hasher);
        format!("{}:{}:{:016x}", provider, model, hasher.finish())
    }

    /// 获取缓存的嵌入向量
    pub async fn get(&self, provider: &str, model: &str, text: &str) -> Option<Vec<f32>> {
        let key = Self::cache_key(provider, model, text);
        let mut store = self.store.write().await;

        if let Some(entry) = store.get_mut(&key) {
            // 检查是否过期
            if entry.created_at.elapsed() > Duration::from_secs(self.ttl_secs) {
                store.remove(&key);
                return None;
            }

            entry.access_count += 1;
            return Some(entry.embedding.clone());
        }

        None
    }

    /// 存储嵌入向量到缓存
    pub async fn put(&self, provider: &str, model: &str, text: &str, embedding: Vec<f32>) {
        let key = Self::cache_key(provider, model, text);
        let mut store = self.store.write().await;

        // 如果达到最大条目数，移除最少访问的条目
        if store.len() >= self.max_entries {
            if let Some((lru_key, _)) = store
                .iter()
                .min_by_key(|(_, e)| e.access_count)
                .map(|(k, v)| (k.clone(), v.access_count))
            {
                store.remove(&lru_key);
            }
        }

        store.insert(
            key,
            CacheEntry {
                embedding,
                created_at: Instant::now(),
                access_count: 0,
            },
        );
    }

    /// 批量获取缓存的嵌入向量
    pub async fn get_batch(
        &self,
        provider: &str,
        model: &str,
        texts: &[String],
    ) -> Vec<Option<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        let mut store = self.store.write().await;

        for text in texts {
            let key = Self::cache_key(provider, model, text);

            if let Some(entry) = store.get_mut(&key) {
                if entry.created_at.elapsed() > Duration::from_secs(self.ttl_secs) {
                    store.remove(&key);
                    results.push(None);
                } else {
                    entry.access_count += 1;
                    results.push(Some(entry.embedding.clone()));
                }
            } else {
                results.push(None);
            }
        }

        results
    }

    /// 清空缓存
    pub async fn clear(&self) {
        let mut store = self.store.write().await;
        store.clear();
    }

    /// 获取缓存统计信息
    pub async fn stats(&self) -> CacheStats {
        let store = self.store.read().await;
        let total_entries = store.len();
        let total_access: usize = store.values().map(|e| e.access_count).sum();

        CacheStats {
            total_entries,
            total_access,
            max_entries: self.max_entries,
            ttl_secs: self.ttl_secs,
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_access: usize,
    pub max_entries: usize,
    pub ttl_secs: u64,
}

// ============================================================================
// 模型配置
// ============================================================================

/// 嵌入模型提供商类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingProvider {
    OpenAI,
    HuggingFace,
    Cohere,
    Local,
    /// Mock 提供商，用于测试和安全默认值
    Mock,
}

impl EmbeddingProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::HuggingFace => "huggingface",
            Self::Cohere => "cohere",
            Self::Local => "local",
            Self::Mock => "mock",
        }
    }
}

/// 嵌入模型配置
#[derive(Debug, Clone)]
pub struct EmbeddingsConfig {
    /// 提供商类型
    pub provider: EmbeddingProvider,
    /// API 密钥（本地模型可为空）
    pub api_key: String,
    /// API 基础 URL（可选）
    pub base_url: Option<String>,
    /// 模型名称
    pub model: String,
    /// 向量维度（可选，用于本地模型）
    pub dimension: Option<usize>,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        // 安全默认值：使用 Mock 提供商
        // 这样可以避免在未配置环境下意外调用外部 API
        Self {
            provider: EmbeddingProvider::Mock,
            api_key: String::new(),
            base_url: None,
            model: "mock-embedding".to_string(),
            dimension: Some(DEFAULT_EMBEDDING_DIMENSION),
        }
    }
}

impl EmbeddingsConfig {
    /// 从环境变量创建 OpenAI 配置
    pub fn openai_from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow!("OPENAI_API_KEY environment variable not set"))?;

        let base_url = std::env::var("OPENAI_BASE_URL")
            .ok()
            .or_else(|| Some("https://api.openai.com/v1".to_string()));

        let model = std::env::var("OPENAI_EMBEDDING_MODEL")
            .unwrap_or_else(|_| DEFAULT_EMBEDDING_MODEL.to_string());

        Ok(Self {
            provider: EmbeddingProvider::OpenAI,
            api_key,
            base_url,
            model,
            dimension: None,
        })
    }

    /// 从环境变量创建 HuggingFace 配置
    pub fn huggingface_from_env() -> Result<Self> {
        let api_key = std::env::var("HUGGINGFACE_API_KEY")
            .map_err(|_| anyhow!("HUGGINGFACE_API_KEY environment variable not set"))?;

        let model = std::env::var("HUGGINGFACE_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "sentence-transformers/all-MiniLM-L6-v2".to_string());

        Ok(Self {
            provider: EmbeddingProvider::HuggingFace,
            api_key,
            base_url: Some(
                "https://api-inference.huggingface.co/pipeline/feature-extraction".to_string(),
            ),
            model,
            dimension: None,
        })
    }

    /// 从环境变量创建 Cohere 配置
    pub fn cohere_from_env() -> Result<Self> {
        let api_key = std::env::var("COHERE_API_KEY")
            .map_err(|_| anyhow!("COHERE_API_KEY environment variable not set"))?;

        let model = std::env::var("COHERE_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "embed-english-v3.0".to_string());

        Ok(Self {
            provider: EmbeddingProvider::Cohere,
            api_key,
            base_url: Some("https://api.cohere.ai/v1".to_string()),
            model,
            dimension: None,
        })
    }

    /// 创建本地模型配置
    pub fn local(model: impl Into<String>, dimension: Option<usize>) -> Self {
        Self {
            provider: EmbeddingProvider::Local,
            api_key: String::new(),
            base_url: None,
            model: model.into(),
            dimension,
        }
    }

    /// 检查配置是否有效（本地模型和 Mock 不需要 API key）
    pub fn is_valid(&self) -> bool {
        matches!(
            self.provider,
            EmbeddingProvider::Local | EmbeddingProvider::Mock
        ) || !self.api_key.is_empty()
    }
}

// ============================================================================
// OpenAI 实现
// ============================================================================

/// OpenAI 嵌入模型
#[derive(Debug)]
pub struct OpenAIEmbeddings {
    client: Client,
    config: EmbeddingsConfig,
    cache: Option<Arc<EmbeddingCache>>,
}

impl OpenAIEmbeddings {
    pub fn new(config: EmbeddingsConfig) -> Result<Self> {
        if !config.is_valid() {
            return Err(anyhow!("OpenAI Embeddings API not configured"));
        }

        Ok(Self {
            client: Client::new(),
            config,
            cache: None,
        })
    }

    pub fn with_cache(config: EmbeddingsConfig, cache: Arc<EmbeddingCache>) -> Result<Self> {
        let mut embeddings = Self::new(config)?;
        embeddings.cache = Some(cache);
        Ok(embeddings)
    }

    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }
}

#[async_trait]
impl EmbeddingModel for OpenAIEmbeddings {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned"))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 检查缓存
        if let Some(cache) = &self.cache {
            let cached = cache.get_batch("openai", &self.config.model, texts).await;
            let all_cached = cached.iter().all(|c| c.is_some());
            if all_cached {
                return Ok(cached.into_iter().map(|c| c.unwrap()).collect());
            }
        }

        let url = format!("{}/embeddings", self.base_url());

        let request_body = OpenAiEmbeddingRequest {
            model: self.config.model.clone(),
            input: texts.to_vec(),
            encoding_format: Some("float".to_string()),
        };

        tracing::debug!("Sending OpenAI embedding request for {} texts", texts.len());

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            tracing::error!("OpenAI Embedding API error: {} - {}", status, response_text);
            return Err(anyhow!(
                "OpenAI Embedding API request failed with status {}: {}",
                status,
                response_text
            ));
        }

        let response_body: OpenAiEmbeddingResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                anyhow!(
                    "Failed to parse OpenAI embedding response: {} - {}",
                    e,
                    response_text
                )
            })?;

        // 按 index 排序并提取向量
        let mut embeddings: Vec<(usize, Vec<f32>)> = response_body
            .data
            .into_iter()
            .map(|item| (item.index, item.embedding))
            .collect();
        embeddings.sort_by_key(|(idx, _)| *idx);
        let result: Vec<Vec<f32>> = embeddings.into_iter().map(|(_, emb)| emb).collect();

        // 存入缓存
        if let Some(cache) = &self.cache {
            for (text, embedding) in texts.iter().zip(result.iter()) {
                cache
                    .put("openai", &self.config.model, text, embedding.clone())
                    .await;
            }
        }

        Ok(result)
    }

    fn dimension(&self) -> usize {
        match self.config.model.as_str() {
            "text-embedding-ada-002" => 1536,
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            _ => DEFAULT_EMBEDDING_DIMENSION,
        }
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn provider(&self) -> &str {
        "openai"
    }
}

#[derive(Serialize)]
struct OpenAiEmbeddingRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding_format: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    usage: OpenAiEmbeddingUsage,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
    #[allow(dead_code)]
    object: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiEmbeddingUsage {
    prompt_tokens: u32,
    total_tokens: u32,
}

// ============================================================================
// HuggingFace 实现
// ============================================================================

/// HuggingFace 嵌入模型
#[derive(Debug)]
pub struct HuggingFaceEmbeddings {
    client: Client,
    config: EmbeddingsConfig,
    cache: Option<Arc<EmbeddingCache>>,
}

impl HuggingFaceEmbeddings {
    pub fn new(config: EmbeddingsConfig) -> Result<Self> {
        if !config.is_valid() {
            return Err(anyhow!("HuggingFace API not configured"));
        }

        Ok(Self {
            client: Client::new(),
            config,
            cache: None,
        })
    }

    pub fn with_cache(config: EmbeddingsConfig, cache: Arc<EmbeddingCache>) -> Result<Self> {
        let mut embeddings = Self::new(config)?;
        embeddings.cache = Some(cache);
        Ok(embeddings)
    }
}

#[async_trait]
impl EmbeddingModel for HuggingFaceEmbeddings {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // HuggingFace API 返回格式取决于模型，通常需要单独调用
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned from HuggingFace"))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 检查缓存
        if let Some(cache) = &self.cache {
            let cached = cache
                .get_batch("huggingface", &self.config.model, texts)
                .await;
            let all_cached = cached.iter().all(|c| c.is_some());
            if all_cached {
                return Ok(cached.into_iter().map(|c| c.unwrap()).collect());
            }
        }

        let url = format!(
            "https://api-inference.huggingface.co/pipeline/feature-extraction/{}",
            self.config.model
        );

        tracing::debug!(
            "Sending HuggingFace embedding request for {} texts",
            texts.len()
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "inputs": texts }))
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            tracing::error!("HuggingFace API error: {} - {}", status, response_text);
            return Err(anyhow!(
                "HuggingFace API request failed with status {}: {}",
                status,
                response_text
            ));
        }

        // HuggingFace 返回格式: [[f32, f32, ...], ...] 或 [[f32], [f32], ...]
        let embeddings: Vec<Vec<f32>> = serde_json::from_str(&response_text).map_err(|e| {
            anyhow!(
                "Failed to parse HuggingFace response: {} - {}",
                e,
                response_text
            )
        })?;

        // 存入缓存
        if let Some(cache) = &self.cache {
            for (text, embedding) in texts.iter().zip(embeddings.iter()) {
                cache
                    .put("huggingface", &self.config.model, text, embedding.clone())
                    .await;
            }
        }

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        // 常见模型的维度
        match self.config.model.as_str() {
            "sentence-transformers/all-MiniLM-L6-v2" => 384,
            "sentence-transformers/all-mpnet-base-v2" => 768,
            "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2" => 384,
            _ => self.config.dimension.unwrap_or(768),
        }
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn provider(&self) -> &str {
        "huggingface"
    }
}

// ============================================================================
// Cohere 实现
// ============================================================================

/// Cohere 嵌入模型
#[derive(Debug)]
pub struct CohereEmbeddings {
    client: Client,
    config: EmbeddingsConfig,
    cache: Option<Arc<EmbeddingCache>>,
}

impl CohereEmbeddings {
    pub fn new(config: EmbeddingsConfig) -> Result<Self> {
        if !config.is_valid() {
            return Err(anyhow!("Cohere API not configured"));
        }

        Ok(Self {
            client: Client::new(),
            config,
            cache: None,
        })
    }

    pub fn with_cache(config: EmbeddingsConfig, cache: Arc<EmbeddingCache>) -> Result<Self> {
        let mut embeddings = Self::new(config)?;
        embeddings.cache = Some(cache);
        Ok(embeddings)
    }
}

#[async_trait]
impl EmbeddingModel for CohereEmbeddings {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned from Cohere"))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 检查缓存
        if let Some(cache) = &self.cache {
            let cached = cache.get_batch("cohere", &self.config.model, texts).await;
            let all_cached = cached.iter().all(|c| c.is_some());
            if all_cached {
                return Ok(cached.into_iter().map(|c| c.unwrap()).collect());
            }
        }

        let url = "https://api.cohere.ai/v1/embed";

        let request_body = CohereEmbeddingRequest {
            model: self.config.model.clone(),
            texts: texts.to_vec(),
            input_type: "search_document",
            embedding_types: Some(vec!["float".to_string()]),
        };

        tracing::debug!("Sending Cohere embedding request for {} texts", texts.len());

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            tracing::error!("Cohere API error: {} - {}", status, response_text);
            return Err(anyhow!(
                "Cohere API request failed with status {}: {}",
                status,
                response_text
            ));
        }

        let response_body: CohereEmbeddingResponse = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse Cohere response: {} - {}", e, response_text))?;

        let result = response_body.embeddings.float;

        // 存入缓存
        if let Some(cache) = &self.cache {
            for (text, embedding) in texts.iter().zip(result.iter()) {
                cache
                    .put("cohere", &self.config.model, text, embedding.clone())
                    .await;
            }
        }

        Ok(result)
    }

    fn dimension(&self) -> usize {
        match self.config.model.as_str() {
            "embed-english-v3.0" | "embed-english-light-v3.0" => 1024,
            "embed-multilingual-v3.0" => 1024,
            "embed-english-v2.0" => 4096,
            _ => self.config.dimension.unwrap_or(1024),
        }
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn provider(&self) -> &str {
        "cohere"
    }
}

#[derive(Serialize)]
struct CohereEmbeddingRequest {
    model: String,
    texts: Vec<String>,
    input_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_types: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct CohereEmbeddingResponse {
    embeddings: CohereEmbeddingsData,
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    text_type: String,
}

#[derive(Deserialize)]
struct CohereEmbeddingsData {
    float: Vec<Vec<f32>>,
}

// ============================================================================
// 本地模型实现 (SentenceTransformers)
// ============================================================================

/// 本地 SentenceTransformers 嵌入模型
///
/// 注意：此实现需要 `candle` 或 `ort` 特性启用。
/// 在纯 Rust 环境下，使用占位实现。
pub struct LocalEmbeddings {
    config: EmbeddingsConfig,
    cache: Option<Arc<EmbeddingCache>>,
    #[cfg(feature = "local-embeddings")]
    #[allow(dead_code)]
    model: Option<std::sync::Mutex<Box<dyn LocalModelBackend>>>,
}

impl std::fmt::Debug for LocalEmbeddings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalEmbeddings")
            .field("config", &self.config)
            .field("cache", &self.cache)
            .field("model", &"<model>")
            .finish()
    }
}

impl LocalEmbeddings {
    pub fn new(config: EmbeddingsConfig) -> Result<Self> {
        Ok(Self {
            config,
            cache: None,
            #[cfg(feature = "local-embeddings")]
            model: None,
        })
    }

    pub fn with_cache(config: EmbeddingsConfig, cache: Arc<EmbeddingCache>) -> Result<Self> {
        let mut embeddings = Self::new(config)?;
        embeddings.cache = Some(cache);
        Ok(embeddings)
    }

    /// 加载本地模型
    #[cfg(feature = "local-embeddings")]
    pub fn load_model(&mut self) -> Result<()> {
        // 使用 candle 或 ort 加载模型
        // 这是一个占位实现
        tracing::info!("Loading local embedding model: {}", self.config.model);
        Ok(())
    }
}

#[async_trait]
impl EmbeddingModel for LocalEmbeddings {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 检查缓存
        if let Some(cache) = &self.cache {
            if let Some(embedding) = cache.get("local", &self.config.model, text).await {
                return Ok(embedding);
            }
        }

        #[cfg(feature = "local-embeddings")]
        {
            // 实际实现使用 candle 或 ort
            // 这里是占位代码
            let embedding = vec![0.0f32; self.dimension()];

            if let Some(cache) = &self.cache {
                cache
                    .put("local", &self.config.model, text, embedding.clone())
                    .await;
            }

            Ok(embedding)
        }

        #[cfg(not(feature = "local-embeddings"))]
        {
            Err(anyhow!(
                "Local embeddings require 'local-embeddings' feature. \
                 Enable it in Cargo.toml and ensure candle or ort is available."
            ))
        }
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // 检查缓存
        if let Some(cache) = &self.cache {
            let cached = cache.get_batch("local", &self.config.model, texts).await;
            if cached.iter().all(|c| c.is_some()) {
                return Ok(cached.into_iter().map(|c| c.unwrap()).collect());
            }
        }

        #[cfg(feature = "local-embeddings")]
        {
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                results.push(self.embed(text).await?);
            }

            // 存入缓存
            if let Some(cache) = &self.cache {
                for (text, embedding) in texts.iter().zip(results.iter()) {
                    cache
                        .put("local", &self.config.model, text, embedding.clone())
                        .await;
                }
            }

            Ok(results)
        }

        #[cfg(not(feature = "local-embeddings"))]
        {
            Err(anyhow!(
                "Local embeddings require 'local-embeddings' feature"
            ))
        }
    }

    fn dimension(&self) -> usize {
        self.config.dimension.unwrap_or(384)
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn provider(&self) -> &str {
        "local"
    }
}

/// 本地模型后端 trait
#[cfg(feature = "local-embeddings")]
#[allow(dead_code)]
trait LocalModelBackend: Send + Sync {
    fn encode(&self, text: &str) -> Result<Vec<f32>>;
}

// ============================================================================
// 统一 Embeddings 工厂
// ============================================================================

/// 嵌入模型工厂
pub struct EmbeddingsFactory {
    cache: Arc<EmbeddingCache>,
}

impl EmbeddingsFactory {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(EmbeddingCache::default_cache()),
        }
    }

    pub fn with_cache(cache: Arc<EmbeddingCache>) -> Self {
        Self { cache }
    }

    /// 创建嵌入模型实例
    pub fn create(&self, config: EmbeddingsConfig) -> Result<Box<dyn EmbeddingModel>> {
        match config.provider {
            EmbeddingProvider::OpenAI => Ok(Box::new(OpenAIEmbeddings::with_cache(
                config,
                self.cache.clone(),
            )?)),
            EmbeddingProvider::HuggingFace => Ok(Box::new(HuggingFaceEmbeddings::with_cache(
                config,
                self.cache.clone(),
            )?)),
            EmbeddingProvider::Cohere => Ok(Box::new(CohereEmbeddings::with_cache(
                config,
                self.cache.clone(),
            )?)),
            EmbeddingProvider::Local => Ok(Box::new(LocalEmbeddings::with_cache(
                config,
                self.cache.clone(),
            )?)),
            EmbeddingProvider::Mock => {
                let dimension = config.dimension.unwrap_or(DEFAULT_EMBEDDING_DIMENSION);
                #[cfg(any(feature = "mock", test))]
                {
                    Ok(Box::new(MockEmbeddingModel::with_name(
                        dimension,
                        &config.model,
                    )))
                }
                #[cfg(not(any(feature = "mock", test)))]
                {
                    // 当 mock feature 未启用时，使用 LocalEmbeddings 作为回退
                    let local_config = EmbeddingsConfig::local(&config.model, Some(dimension));
                    Ok(Box::new(LocalEmbeddings::new(local_config)?))
                }
            }
        }
    }

    /// 创建安全的嵌入模型实例
    ///
    /// 如果指定的配置无效，自动回退到 Mock 模型。
    /// 这确保了即使在未配置环境下也能安全返回一个可用实例。
    pub fn create_safe(&self, config: EmbeddingsConfig) -> Box<dyn EmbeddingModel> {
        if config.is_valid() {
            self.create(config)
                .unwrap_or_else(|_| self.create_mock_default())
        } else {
            self.create_mock_default()
        }
    }

    /// 创建默认 Mock 模型
    fn create_mock_default(&self) -> Box<dyn EmbeddingModel> {
        #[cfg(any(feature = "mock", test))]
        {
            Box::new(MockEmbeddingModel::new(DEFAULT_EMBEDDING_DIMENSION))
        }
        #[cfg(not(any(feature = "mock", test)))]
        {
            // 如果没有 mock feature，使用 LocalEmbeddings 作为安全回退
            let config = EmbeddingsConfig::local("fallback", Some(DEFAULT_EMBEDDING_DIMENSION));
            Box::new(LocalEmbeddings::new(config).expect("Local embeddings should always work"))
        }
    }

    /// 创建 OpenAI 嵌入模型
    pub fn openai(&self) -> Result<Box<dyn EmbeddingModel>> {
        let config = EmbeddingsConfig::openai_from_env()?;
        self.create(config)
    }

    /// 创建 HuggingFace 嵌入模型
    pub fn huggingface(&self) -> Result<Box<dyn EmbeddingModel>> {
        let config = EmbeddingsConfig::huggingface_from_env()?;
        self.create(config)
    }

    /// 创建 Cohere 嵌入模型
    pub fn cohere(&self) -> Result<Box<dyn EmbeddingModel>> {
        let config = EmbeddingsConfig::cohere_from_env()?;
        self.create(config)
    }

    /// 创建本地嵌入模型
    pub fn local(&self, model: &str, dimension: Option<usize>) -> Result<Box<dyn EmbeddingModel>> {
        let config = EmbeddingsConfig::local(model, dimension);
        self.create(config)
    }

    /// 创建 Mock 嵌入模型（仅测试/开发使用）
    ///
    /// **安全默认值**: Mock 模型返回零向量，不调用任何外部 API。
    /// 这是在未配置环境下的安全回退选项。
    #[cfg(any(feature = "mock", test))]
    pub fn mock(&self, dimension: usize) -> Box<dyn EmbeddingModel> {
        Box::new(MockEmbeddingModel::new(dimension))
    }

    /// 获取缓存实例
    pub fn cache(&self) -> Arc<EmbeddingCache> {
        self.cache.clone()
    }
}

impl Default for EmbeddingsFactory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Mock 嵌入模型（仅测试/开发使用）
// ============================================================================

/// Mock 嵌入模型
///
/// 用于测试场景或作为回退，返回固定维度的零向量。
///
/// **注意**: 此类型仅在启用 `mock` feature 或测试配置下可用。
/// 生产代码不应使用此类型。
#[cfg(any(feature = "mock", test))]
pub struct MockEmbeddingModel {
    dimension: usize,
    model_name: String,
}

#[cfg(any(feature = "mock", test))]
impl MockEmbeddingModel {
    /// 创建新的 Mock 模型
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            model_name: "mock-embedding".to_string(),
        }
    }

    /// 使用自定义模型名创建
    pub fn with_name(dimension: usize, model_name: impl Into<String>) -> Self {
        Self {
            dimension,
            model_name: model_name.into(),
        }
    }
}

#[cfg(any(feature = "mock", test))]
#[async_trait]
impl EmbeddingModel for MockEmbeddingModel {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(vec![0.0; self.dimension])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0; self.dimension]).collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn provider(&self) -> &str {
        "mock"
    }
}

// ============================================================================
// 向后兼容：保留原有 Embeddings 类型别名
// ============================================================================

/// 向后兼容的 Embeddings 类型
///
/// 默认使用 OpenAI。
pub type Embeddings = OpenAIEmbeddings;

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // 缓存测试
    // ==========================================================================

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = EmbeddingCache::new(100, 3600);

        // 测试 put 和 get
        let embedding = vec![0.1f32, 0.2, 0.3];
        cache
            .put("openai", "test-model", "hello", embedding.clone())
            .await;

        let cached = cache.get("openai", "test-model", "hello").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), embedding);

        // 测试未命中的情况
        let not_cached = cache.get("openai", "test-model", "not-exists").await;
        assert!(not_cached.is_none());
    }

    #[tokio::test]
    async fn test_cache_batch_operations() {
        let cache = EmbeddingCache::new(100, 3600);

        let texts: Vec<String> = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let embeddings: Vec<Vec<f32>> = texts.iter().map(|t| vec![t.len() as f32]).collect();

        for (text, emb) in texts.iter().zip(embeddings.iter()) {
            cache.put("test", "model", text, emb.clone()).await;
        }

        let cached = cache.get_batch("test", "model", &texts).await;
        assert!(cached.iter().all(|c| c.is_some()));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = EmbeddingCache::new(100, 3600);

        cache.put("test", "model", "a", vec![1.0f32]).await;
        cache.put("test", "model", "b", vec![2.0]).await;

        let _ = cache.get("test", "model", "a").await;
        let _ = cache.get("test", "model", "a").await;

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.total_access, 2);
    }

    // ==========================================================================
    // 配置测试
    // ==========================================================================

    #[test]
    fn test_config_openai_from_env() {
        std::env::set_var("OPENAI_API_KEY", "test_key");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("OPENAI_EMBEDDING_MODEL");

        let config = EmbeddingsConfig::openai_from_env().unwrap();
        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.model, DEFAULT_EMBEDDING_MODEL);

        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_config_huggingface_from_env() {
        std::env::set_var("HUGGINGFACE_API_KEY", "hf_test");
        std::env::remove_var("HUGGINGFACE_EMBEDDING_MODEL");

        let config = EmbeddingsConfig::huggingface_from_env().unwrap();
        assert_eq!(config.api_key, "hf_test");
        assert!(config.model.contains("sentence-transformers"));

        std::env::remove_var("HUGGINGFACE_API_KEY");
    }

    #[test]
    fn test_config_cohere_from_env() {
        std::env::set_var("COHERE_API_KEY", "cohere_test");
        std::env::remove_var("COHERE_EMBEDDING_MODEL");

        let config = EmbeddingsConfig::cohere_from_env().unwrap();
        assert_eq!(config.api_key, "cohere_test");
        assert!(config.model.starts_with("embed-"));

        std::env::remove_var("COHERE_API_KEY");
    }

    #[test]
    fn test_config_local() {
        let config = EmbeddingsConfig::local("all-MiniLM-L6-v2", Some(384));
        assert_eq!(config.provider, EmbeddingProvider::Local);
        assert!(config.api_key.is_empty());
        assert!(config.is_valid()); // 本地模型不需要 API key
    }

    // ==========================================================================
    // 维度测试
    // ==========================================================================

    #[test]
    fn test_openai_dimension() {
        let config = EmbeddingsConfig {
            provider: EmbeddingProvider::OpenAI,
            api_key: "test".to_string(),
            base_url: None,
            model: "text-embedding-ada-002".to_string(),
            dimension: None,
        };
        let embeddings = OpenAIEmbeddings::new(config).unwrap();
        assert_eq!(embeddings.dimension(), 1536);

        let config = EmbeddingsConfig {
            provider: EmbeddingProvider::OpenAI,
            api_key: "test".to_string(),
            base_url: None,
            model: "text-embedding-3-large".to_string(),
            dimension: None,
        };
        let embeddings = OpenAIEmbeddings::new(config).unwrap();
        assert_eq!(embeddings.dimension(), 3072);
    }

    #[test]
    fn test_huggingface_dimension() {
        let config = EmbeddingsConfig {
            provider: EmbeddingProvider::HuggingFace,
            api_key: "test".to_string(),
            base_url: None,
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            dimension: None,
        };
        let embeddings = HuggingFaceEmbeddings::new(config).unwrap();
        assert_eq!(embeddings.dimension(), 384);
    }

    #[test]
    fn test_cohere_dimension() {
        let config = EmbeddingsConfig {
            provider: EmbeddingProvider::Cohere,
            api_key: "test".to_string(),
            base_url: None,
            model: "embed-english-v3.0".to_string(),
            dimension: None,
        };
        let embeddings = CohereEmbeddings::new(config).unwrap();
        assert_eq!(embeddings.dimension(), 1024);
    }

    // ==========================================================================
    // 工厂测试
    // ==========================================================================

    #[test]
    fn test_factory_create_openai() {
        std::env::set_var("OPENAI_API_KEY", "test_key");

        let factory = EmbeddingsFactory::new();
        let model = factory.openai().unwrap();
        assert_eq!(model.provider(), "openai");

        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_factory_create_local() {
        let factory = EmbeddingsFactory::new();
        let model = factory.local("test-model", Some(384)).unwrap();
        assert_eq!(model.provider(), "local");
        assert_eq!(model.dimension(), 384);
    }

    #[test]
    fn test_factory_create_mock() {
        let factory = EmbeddingsFactory::new();
        let model = factory.mock(512);
        assert_eq!(model.provider(), "mock");
        assert_eq!(model.dimension(), 512);
    }

    #[test]
    fn test_factory_create_safe_with_invalid_config() {
        let factory = EmbeddingsFactory::new();
        // 使用空 api_key 的 OpenAI 配置是无效的
        let config = EmbeddingsConfig {
            provider: EmbeddingProvider::OpenAI,
            api_key: String::new(),
            base_url: None,
            model: "test".to_string(),
            dimension: None,
        };
        let model = factory.create_safe(config);
        // 应该回退到 mock
        assert_eq!(model.provider(), "mock");
    }

    #[test]
    fn test_factory_create_safe_with_valid_config() {
        std::env::set_var("OPENAI_API_KEY", "test_key");
        let factory = EmbeddingsFactory::new();
        let config = EmbeddingsConfig::openai_from_env().unwrap();
        let model = factory.create_safe(config);
        assert_eq!(model.provider(), "openai");
        std::env::remove_var("OPENAI_API_KEY");
    }

    // ==========================================================================
    // 安全默认值测试
    // ==========================================================================

    #[test]
    fn test_config_default_is_safe() {
        let config = EmbeddingsConfig::default();
        // 默认配置应该使用 Mock 提供商
        assert_eq!(config.provider, EmbeddingProvider::Mock);
        // Mock 提供商不需要 API key，所以应该有效
        assert!(config.is_valid());
    }

    #[test]
    fn test_provider_mock_is_valid() {
        let config = EmbeddingsConfig {
            provider: EmbeddingProvider::Mock,
            api_key: String::new(),
            base_url: None,
            model: "mock-test".to_string(),
            dimension: Some(256),
        };
        assert!(config.is_valid());
    }

    #[test]
    fn test_embeddings_factory_mock_default_dimension() {
        let factory = EmbeddingsFactory::new();
        let model = factory.mock(DEFAULT_EMBEDDING_DIMENSION);
        assert_eq!(model.dimension(), DEFAULT_EMBEDDING_DIMENSION);
    }

    // ==========================================================================
    // 向后兼容测试
    // ==========================================================================

    #[test]
    fn test_backward_compatible_embeddings() {
        std::env::set_var("OPENAI_API_KEY", "test_key");

        let config = EmbeddingsConfig::openai_from_env().unwrap();
        let embeddings = Embeddings::new(config).unwrap();
        assert_eq!(embeddings.provider(), "openai");

        std::env::remove_var("OPENAI_API_KEY");
    }
}
