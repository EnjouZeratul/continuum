//! # Web Search Tool
//!
//! Web search functionality supporting multiple search engines.
//! Provides rate limiting, result caching, and error recovery.

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Search Engine Configuration
// ============================================================================

/// Supported search engines
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchEngine {
    /// DuckDuckGo (free, no API key required)
    #[default]
    DuckDuckGo,
    /// Google Custom Search (requires API key)
    Google,
    /// Bing Search (requires API key)
    Bing,
}

/// Search engine configuration
#[derive(Debug, Clone)]
pub struct SearchEngineConfig {
    /// Engine type
    pub engine: SearchEngine,
    /// API key (required for Google/Bing)
    pub api_key: Option<String>,
    /// Custom search engine ID (required for Google)
    pub cx: Option<String>,
    /// Maximum results per search
    pub max_results: usize,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Enable result caching
    pub enable_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
}

impl Default for SearchEngineConfig {
    fn default() -> Self {
        Self {
            engine: SearchEngine::DuckDuckGo,
            api_key: None,
            cx: None,
            max_results: 10,
            timeout_secs: 30,
            enable_cache: true,
            cache_ttl_secs: 3600,
        }
    }
}

// ============================================================================
// Search Result Types
// ============================================================================

/// A single search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result title
    pub title: String,
    /// Result URL
    pub url: String,
    /// Result snippet/description
    pub snippet: String,
    /// Source engine
    pub engine: String,
    /// Position in results
    pub position: usize,
}

/// Complete search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Search query
    pub query: String,
    /// Search results
    pub results: Vec<SearchResult>,
    /// Total results available
    pub total: usize,
    /// Search engine used
    pub engine: String,
    /// Response time in ms
    pub response_time_ms: u64,
    /// Whether results came from cache
    pub from_cache: bool,
}

// ============================================================================
// Rate Limiter
// ============================================================================

/// Simple rate limiter for API calls
struct RateLimiter {
    /// Minimum interval between requests
    min_interval: Duration,
    /// Last request time
    last_request: RwLock<Option<Instant>>,
}

impl RateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_request: RwLock::new(None),
        }
    }

    async fn acquire(&self) {
        loop {
            let now = Instant::now();
            let should_wait = {
                let last = self.last_request.read();
                if let Some(last_time) = *last {
                    let elapsed = now.duration_since(last_time);
                    elapsed < self.min_interval
                } else {
                    false
                }
            };

            if should_wait {
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                break;
            }
        }

        // Update last request time
        *self.last_request.write() = Some(Instant::now());
    }
}

// ============================================================================
// Result Cache
// ============================================================================

/// Cache entry with expiration
struct CacheEntry {
    response: SearchResponse,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        Instant::now().duration_since(self.created_at) > self.ttl
    }
}

/// Search result cache
struct SearchResultCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
}

impl SearchResultCache {
    fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    fn get(&self, key: &str) -> Option<SearchResponse> {
        let entries = self.entries.read();
        entries.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.response.clone())
            }
        })
    }

    fn put(&self, key: String, response: SearchResponse, ttl: Duration) {
        let mut entries = self.entries.write();
        entries.insert(
            key,
            CacheEntry {
                response,
                created_at: Instant::now(),
                ttl,
            },
        );

        // Cleanup expired entries
        let keys_to_remove: Vec<String> = entries
            .iter()
            .filter(|(_, e)| e.is_expired())
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys_to_remove {
            entries.remove(&key);
        }
    }
}

// ============================================================================
// Web Search Tool
// ============================================================================

/// Web Search Tool implementation
pub struct WebSearchTool {
    /// HTTP client
    client: Client,
    /// Configuration
    config: SearchEngineConfig,
    /// Rate limiter
    rate_limiter: RateLimiter,
    /// Result cache
    cache: Option<Arc<SearchResultCache>>,
}

impl WebSearchTool {
    /// Create a new web search tool with default configuration
    pub fn new() -> Self {
        Self::with_config(SearchEngineConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: SearchEngineConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .user_agent("ContinuumSDK/1.0")
            .build()
            .unwrap_or_else(|_| Client::new());

        let cache = if config.enable_cache {
            Some(Arc::new(SearchResultCache::new()))
        } else {
            None
        };

        Self {
            client,
            config,
            rate_limiter: RateLimiter::new(Duration::from_millis(500)),
            cache,
        }
    }

    /// Create with API key for Google/Bing
    pub fn with_api_key(engine: SearchEngine, api_key: String, cx: Option<String>) -> Self {
        let mut config = SearchEngineConfig {
            engine,
            api_key: Some(api_key),
            cx: cx.clone(),
            ..Default::default()
        };

        if engine == SearchEngine::Google && cx.is_none() {
            // Use a default Google Custom Search Engine ID if not provided
            config.cx = Some("017576662512468239146:omuauf_lfve".to_string());
        }

        Self::with_config(config)
    }

    /// Execute search
    pub async fn search(&self, query: &str) -> Layer3Result<SearchResponse> {
        // Check cache first
        if let Some(cache) = &self.cache {
            if let Some(cached) = cache.get(query) {
                return Ok(cached);
            }
        }

        // Rate limit
        self.rate_limiter.acquire().await;

        let start = Instant::now();
        let results = match self.config.engine {
            SearchEngine::DuckDuckGo => self.search_duckduckgo(query).await?,
            SearchEngine::Google => self.search_google(query).await?,
            SearchEngine::Bing => self.search_bing(query).await?,
        };
        let response_time_ms = start.elapsed().as_millis() as u64;

        let response = SearchResponse {
            query: query.to_string(),
            results: results.clone(),
            total: results.len(),
            engine: format!("{:?}", self.config.engine),
            response_time_ms,
            from_cache: false,
        };

        // Cache the result
        if let Some(cache) = &self.cache {
            cache.put(
                query.to_string(),
                response.clone(),
                Duration::from_secs(self.config.cache_ttl_secs),
            );
        }

        Ok(response)
    }

    /// Search using DuckDuckGo (free, no API key required)
    async fn search_duckduckgo(&self, query: &str) -> Layer3Result<Vec<SearchResult>> {
        // DuckDuckGo Instant Answer API
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
            urlencoding::encode(query)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("DuckDuckGo API error: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "DuckDuckGo API returned status: {}",
                response.status()
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse DuckDuckGo response: {}", e))?;

        Ok(self.parse_duckduckgo_results(&json))
    }

    fn parse_duckduckgo_results(&self, json: &serde_json::Value) -> Vec<SearchResult> {
        let mut results = Vec::new();

        // Parse instant answer
        if let Some(abstract_text) = json.get("AbstractText").and_then(|v| v.as_str()) {
            if !abstract_text.is_empty() {
                if let Some(abstract_url) = json.get("AbstractURL").and_then(|v| v.as_str()) {
                    if !abstract_url.is_empty() {
                        results.push(SearchResult {
                            title: json
                                .get("Heading")
                                .and_then(|v| v.as_str())
                                .unwrap_or("DuckDuckGo Result")
                                .to_string(),
                            url: abstract_url.to_string(),
                            snippet: abstract_text.to_string(),
                            engine: "DuckDuckGo".to_string(),
                            position: 1,
                        });
                    }
                }
            }
        }

        // Parse related topics
        if let Some(topics) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
            for topic in topics.iter().take(self.config.max_results - results.len()) {
                if let (Some(text), Some(url), Some(first_url)) = (
                    topic.get("Text").and_then(|v| v.as_str()),
                    topic.get("FirstURL").and_then(|v| v.as_str()),
                    topic.get("FirstURL").and_then(|v| v.as_str()),
                ) {
                    if !text.is_empty() && !first_url.is_empty() {
                        results.push(SearchResult {
                            title: text.split(" - ").next().unwrap_or(text).to_string(),
                            url: url.to_string(),
                            snippet: text.to_string(),
                            engine: "DuckDuckGo".to_string(),
                            position: results.len() + 1,
                        });
                    }
                }
            }
        }

        results
    }

    /// Search using Google Custom Search API
    async fn search_google(&self, query: &str) -> Layer3Result<Vec<SearchResult>> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Google Search requires an API key"))?;

        let cx = self.config.cx.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Google Search requires a Custom Search Engine ID (cx)")
        })?;

        let url = format!(
            "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={}",
            api_key,
            cx,
            urlencoding::encode(query),
            self.config.max_results
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Google API error: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Google API returned status {}: {}",
                status,
                error_body
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Google response: {}", e))?;

        let mut results = Vec::new();

        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            for (i, item) in items.iter().enumerate() {
                results.push(SearchResult {
                    title: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("link")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("snippet")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    engine: "Google".to_string(),
                    position: i + 1,
                });
            }
        }

        Ok(results)
    }

    /// Search using Bing Search API
    async fn search_bing(&self, query: &str) -> Layer3Result<Vec<SearchResult>> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Bing Search requires an API key"))?;

        let url = format!(
            "https://api.bing.microsoft.com/v7.0/search?q={}&count={}",
            urlencoding::encode(query),
            self.config.max_results
        );

        let response = self
            .client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", api_key)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Bing API error: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Bing API returned status: {}",
                response.status()
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Bing response: {}", e))?;

        let mut results = Vec::new();

        if let Some(web_pages) = json.get("webPages").and_then(|v| v.get("value")) {
            if let Some(items) = web_pages.as_array() {
                for (i, item) in items.iter().enumerate() {
                    results.push(SearchResult {
                        title: item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        url: item
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        snippet: item
                            .get("snippet")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        engine: "Bing".to_string(),
                        position: i + 1,
                    });
                }
            }
        }

        Ok(results)
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information using DuckDuckGo, Google, or Bing."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "engine": {
                    "type": "string",
                    "enum": ["duckduckgo", "google", "bing"],
                    "description": "Search engine to use (default: duckduckgo)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["query"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Search
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing query parameter"))?;

        // Parse engine if specified
        let engine_str = args["engine"].as_str().unwrap_or("duckduckgo");
        let engine = match engine_str.to_lowercase().as_str() {
            "google" => SearchEngine::Google,
            "bing" => SearchEngine::Bing,
            _ => SearchEngine::DuckDuckGo,
        };

        // Create a temporary tool with the requested engine
        let tool = if engine != self.config.engine {
            let mut config = self.config.clone();
            config.engine = engine;
            WebSearchTool::with_config(config)
        } else {
            // Can't clone self, so just use self
            return self.search(query).await.map(|r| {
                serde_json::to_string_pretty(&r).unwrap_or_else(|_| {
                    r.results
                        .iter()
                        .map(|r| format!("{}: {}", r.title, r.url))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            });
        };

        tool.search(query).await.map(|r| {
            serde_json::to_string_pretty(&r).unwrap_or_else(|_| {
                r.results
                    .iter()
                    .map(|r| format!("{}: {}", r.title, r.url))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        })
    }
}

// ============================================================================
// URL Encoding Helper
// ============================================================================

mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_creation() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert_eq!(tool.category(), ToolCategory::Search);
    }

    #[test]
    fn test_config_default() {
        let config = SearchEngineConfig::default();
        assert_eq!(config.engine, SearchEngine::DuckDuckGo);
        assert!(config.api_key.is_none());
        assert_eq!(config.max_results, 10);
    }

    #[test]
    fn test_cache_basic() {
        let cache = SearchResultCache::new();
        let response = SearchResponse {
            query: "test".to_string(),
            results: vec![],
            total: 0,
            engine: "DuckDuckGo".to_string(),
            response_time_ms: 100,
            from_cache: false,
        };

        cache.put(
            "test".to_string(),
            response.clone(),
            Duration::from_secs(60),
        );
        let cached = cache.get("test");
        assert!(cached.is_some());
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(Duration::from_millis(100));

        // First call should proceed immediately
        let start = Instant::now();
        limiter.acquire();
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_millis(50));
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            snippet: "Test snippet".to_string(),
            engine: "DuckDuckGo".to_string(),
            position: 1,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("example.com"));
    }

    #[test]
    fn test_duckduckgo_no_results_returns_empty_list() {
        let tool = WebSearchTool::new();
        let json = serde_json::json!({
            "AbstractText": "",
            "AbstractURL": "",
            "RelatedTopics": []
        });

        let results = tool.parse_duckduckgo_results(&json);

        assert!(results.is_empty());
    }
}
