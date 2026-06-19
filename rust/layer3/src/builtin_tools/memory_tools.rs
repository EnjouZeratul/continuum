//! # Memory Tools
//!
//! 记忆操作工具集，使用分层记忆系统。

use crate::builtin_tools::safe_truncate::safe_truncate_chars;
use crate::builtin_tools::secret_scrub::SecretScrubber;
use crate::builtin_tools::BuiltinTool;
use crate::memory_system::{MemoryStore, WorkingMemory};
use crate::types::{Layer3Result, MemoryEntry, MemoryQuery, MemoryTier, ToolCategory};
use async_trait::async_trait;
use chrono::Utc;
use sh_layer2::generate_short_id;
use std::sync::Arc;

/// Save Memory Tool
pub struct SaveMemoryTool {
    store: Arc<WorkingMemory>,
    scrubber: Arc<SecretScrubber>,
}

impl SaveMemoryTool {
    pub fn new() -> Self {
        Self {
            store: Arc::new(WorkingMemory::default()),
            scrubber: Arc::new(SecretScrubber::new()),
        }
    }

    /// 使用指定的 store 创建
    pub fn with_store(store: Arc<WorkingMemory>) -> Self {
        Self {
            store,
            scrubber: Arc::new(SecretScrubber::new()),
        }
    }
}

impl Default for SaveMemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for SaveMemoryTool {
    fn name(&self) -> &str {
        "save_memory"
    }

    fn description(&self) -> &str {
        "Save a memory entry to the memory system."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The content to remember"
                },
                "tier": {
                    "type": "string",
                    "enum": ["working", "session", "project", "long_term"],
                    "description": "Memory tier to store in (default: working)"
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional: additional metadata"
                }
            },
            "required": ["content"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing content parameter"))?;

        // SM1: Content size cap (1 MiB)
        if content.len() > 1024 * 1024 {
            return Err(anyhow::anyhow!(
                "save_memory rejected: content {} bytes > 1 MiB limit",
                content.len(),
            ));
        }

        // SM3: Secret scrubbing before storing
        let safe_content = self.scrubber.scrub(content);
        if let Some(kind) = self.scrubber.contains_secret(content) {
            tracing::warn!(
                target: "continuum.tools.memory",
                memory_secret_detected = %kind,
                "save_memory: scrubbed secret of kind '{}' before storing",
                kind,
            );
        }

        let tier_str = args["tier"].as_str().unwrap_or("working");
        let tier = match tier_str {
            "working" => MemoryTier::Working,
            "session" => MemoryTier::Session,
            "project" => MemoryTier::Project,
            "long_term" => MemoryTier::LongTerm,
            _ => MemoryTier::Working,
        };

        // SM2: Metadata size cap
        let metadata = if let Some(obj) = args["metadata"].as_object() {
            let serialized = serde_json::to_string(obj)?;
            if serialized.len() > 64 * 1024 {
                return Err(anyhow::anyhow!(
                    "save_memory rejected: metadata {} bytes > 64 KiB limit",
                    serialized.len(),
                ));
            }
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        // Create memory entry
        let entry = MemoryEntry {
            id: generate_short_id(),
            content: safe_content,
            tier,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            importance: 0.5,
            metadata,
            access_count: 0,
        };

        // Store in working memory
        let id = self.store.store(entry).await?;

        Ok(format!("Memory saved to {} tier with ID: {}", tier_str, id))
    }
}

/// Query Memory Tool
pub struct QueryMemoryTool {
    store: Arc<WorkingMemory>,
    scrubber: Arc<SecretScrubber>,
}

impl QueryMemoryTool {
    pub fn new() -> Self {
        Self {
            store: Arc::new(WorkingMemory::default()),
            scrubber: Arc::new(SecretScrubber::new()),
        }
    }

    /// 使用指定的 store 创建
    pub fn with_store(store: Arc<WorkingMemory>) -> Self {
        Self {
            store,
            scrubber: Arc::new(SecretScrubber::new()),
        }
    }
}

impl Default for QueryMemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for QueryMemoryTool {
    fn name(&self) -> &str {
        "query_memory"
    }

    fn description(&self) -> &str {
        "Query the memory system for relevant memories."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query text"
                },
                "tier": {
                    "type": "string",
                    "enum": ["working", "session", "project", "long_term"],
                    "description": "Optional: limit to specific tier"
                },
                "limit": {
                    "type": "integer",
                    "description": "Optional: maximum number of results (default: 10)"
                }
            },
            "required": ["query"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let query_text = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing query parameter"))?;

        // QM2: Query length cap
        if query_text.chars().count() > 1000 {
            return Err(anyhow::anyhow!(
                "query_memory rejected: query too long ({} > 1000 chars)",
                query_text.chars().count(),
            ));
        }

        let limit = args["limit"].as_u64().map(|l| l as usize);
        let tier = args["tier"].as_str().and_then(|t| match t {
            "working" => Some(MemoryTier::Working),
            "session" => Some(MemoryTier::Session),
            "project" => Some(MemoryTier::Project),
            "long_term" => Some(MemoryTier::LongTerm),
            _ => None,
        });

        let query = MemoryQuery {
            query: query_text.to_string(),
            tier,
            limit,
            time_range: None,
        };

        // Query working memory
        let results = self.store.query(&query).await?;

        if results.is_empty() {
            Ok("(no memories found)".to_string())
        } else {
            let max = limit.unwrap_or(10).min(100); // QM3: hard cap
            let scrubber = &self.scrubber;
            let output: Vec<String> = results
                .iter()
                .take(max)
                .map(|e| {
                    // QM1: UTF-8-safe truncation
                    let preview = safe_truncate_chars(&e.content, 200);
                    // QM4: Scrub secrets from output (defense-in-depth even though SaveMemory scrubs)
                    let preview = scrubber.scrub(preview);
                    format!("{}: {}", e.id, preview)
                })
                .collect();
            Ok(output.join("\n"))
        }
    }
}

/// Clear Memory Tool
pub struct ClearMemoryTool {
    store: Arc<WorkingMemory>,
}

impl ClearMemoryTool {
    pub fn new() -> Self {
        Self {
            store: Arc::new(WorkingMemory::default()),
        }
    }

    /// 使用指定的 store 创建
    pub fn with_store(store: Arc<WorkingMemory>) -> Self {
        Self { store }
    }
}

impl Default for ClearMemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for ClearMemoryTool {
    fn name(&self) -> &str {
        "clear_memory"
    }

    fn description(&self) -> &str {
        "Clear all memories from a specific tier."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tier": {
                    "type": "string",
                    "enum": ["working", "session", "project", "long_term"],
                    "description": "Memory tier to clear (default: working)"
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to actually clear (default: false)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let tier_str = args["tier"].as_str().unwrap_or("working");
        let confirm = args["confirm"].as_bool().unwrap_or(false);

        // CM2: Require confirmation
        if !confirm {
            return Ok(format!(
                "clear_memory: pass confirm=true to clear '{}' tier.",
                tier_str,
            ));
        }

        let tier = match tier_str {
            "working" => MemoryTier::Working,
            "session" => MemoryTier::Session,
            "project" => MemoryTier::Project,
            "long_term" => MemoryTier::LongTerm,
            _ => MemoryTier::Working,
        };

        // CM1 fix: actually clear only the specified tier
        let count = self.store.clear_tier(tier).await?;

        // CM3: Audit log
        tracing::info!(
            target: "continuum.tools.memory",
            memory_tier = %tier_str,
            memory_cleared_count = count,
            "clear_memory: cleared {} entries from {} tier",
            count,
            tier_str,
        );

        Ok(format!("Cleared {} memories from {} tier", count, tier_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_memory_tool_category() {
        let tool = SaveMemoryTool::new();
        assert_eq!(tool.category(), ToolCategory::Memory);
    }

    #[test]
    fn test_query_memory_tool_category() {
        let tool = QueryMemoryTool::new();
        assert_eq!(tool.category(), ToolCategory::Memory);
    }

    #[tokio::test]
    async fn test_save_memory() {
        let tool = SaveMemoryTool::new();
        let result = tool.execute(json!({"content": "test memory"})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Memory saved"));
    }

    #[tokio::test]
    async fn test_query_memory_empty() {
        let tool = QueryMemoryTool::new();
        let result = tool.execute(json!({"query": "nonexistent"})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("no memories"));
    }

    #[tokio::test]
    async fn test_save_and_query_memory() {
        let store = Arc::new(WorkingMemory::default());

        let save_tool = SaveMemoryTool::with_store(store.clone());
        save_tool
            .execute(json!({"content": "important fact: the sky is blue"}))
            .await
            .unwrap();

        let query_tool = QueryMemoryTool::with_store(store);
        let result = query_tool.execute(json!({"query": "sky"})).await.unwrap();
        assert!(result.contains("sky is blue"));
    }
}
