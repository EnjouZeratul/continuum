//! # Memory Tools
//!
//! 记忆操作工具集，使用分层记忆系统。

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
}

impl SaveMemoryTool {
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

        let tier_str = args["tier"].as_str().unwrap_or("working");
        let tier = match tier_str {
            "working" => MemoryTier::Working,
            "session" => MemoryTier::Session,
            "project" => MemoryTier::Project,
            "long_term" => MemoryTier::LongTerm,
            _ => MemoryTier::Working,
        };

        // Extract metadata as Map
        let metadata = if let Some(obj) = args["metadata"].as_object() {
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        // Create memory entry
        let entry = MemoryEntry {
            id: generate_short_id(),
            content: content.to_string(),
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
}

impl QueryMemoryTool {
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
            let output: Vec<String> = results
                .iter()
                .take(limit.unwrap_or(10))
                .map(|e| {
                    let preview = if e.content.len() > 200 {
                        format!("{}...", &e.content[..200])
                    } else {
                        e.content.clone()
                    };
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

        // Clear working memory
        let count = self.store.clear().await?;

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
