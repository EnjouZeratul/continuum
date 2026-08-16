//! # Memory Tools
//!
//! 记忆操作工具集，使用分层记忆系统。
//!
//! v1.1.0：三个工具共享**同一个** `Arc<UnifiedMemorySystem>` —— 修复了
//! 之前每个工具持有私有 `WorkingMemory` 导致 save 存的记忆 query 看不见
//! 的"裂脑"问题。query 走 `query_all` 跨全部层级检索；持久化由装配处
//! 通过 `with_system` 注入带 ProjectMemory 的系统实现。

use crate::builtin_tools::safe_truncate::safe_truncate_chars;
use crate::builtin_tools::secret_scrub::SecretScrubber;
use crate::builtin_tools::BuiltinTool;
use crate::memory_system::UnifiedMemorySystem;
use crate::types::{Layer3Result, MemoryEntry, MemoryQuery, MemoryTier, ToolCategory};
use async_trait::async_trait;
use chrono::Utc;
use sh_layer2::generate_short_id;
use std::sync::Arc;

/// Save Memory Tool
pub struct SaveMemoryTool {
    system: Arc<UnifiedMemorySystem>,
    scrubber: Arc<SecretScrubber>,
}

impl SaveMemoryTool {
    /// 临时（非持久）系统 —— 单元测试用。生产装配请用 [`Self::with_system`]
    /// 注入共享系统，否则每个工具各自持有独立内存。
    pub fn new() -> Self {
        Self {
            system: Arc::new(UnifiedMemorySystem::new("default")),
            scrubber: Arc::new(SecretScrubber::new()),
        }
    }

    /// 使用共享的统一记忆系统创建
    pub fn with_system(system: Arc<UnifiedMemorySystem>) -> Self {
        Self {
            system,
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
                "importance": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Importance score 0-1 (default: 0.5); high-importance session memories are promoted to project tier at session end"
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
        let tier = parse_tier(tier_str).unwrap_or(MemoryTier::Working);

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

        let importance = args["importance"]
            .as_f64()
            .map(|v| v.clamp(0.0, 1.0) as f32)
            .unwrap_or(0.5);

        // Create memory entry
        let entry = MemoryEntry {
            id: generate_short_id(),
            content: safe_content,
            tier,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            importance,
            metadata,
            access_count: 0,
        };

        // Store through the unified system (routes by entry.tier)
        let id = self.system.store_entry(entry).await?;

        Ok(format!("Memory saved to {} tier with ID: {}", tier_str, id))
    }
}

/// Query Memory Tool
pub struct QueryMemoryTool {
    system: Arc<UnifiedMemorySystem>,
    scrubber: Arc<SecretScrubber>,
}

impl QueryMemoryTool {
    /// 临时（非持久）系统 —— 单元测试用。
    pub fn new() -> Self {
        Self {
            system: Arc::new(UnifiedMemorySystem::new("default")),
            scrubber: Arc::new(SecretScrubber::new()),
        }
    }

    /// 使用共享的统一记忆系统创建
    pub fn with_system(system: Arc<UnifiedMemorySystem>) -> Self {
        Self {
            system,
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
        "Query the memory system for relevant memories across all tiers."
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
        let tier = args["tier"].as_str().and_then(parse_tier);

        let query = MemoryQuery {
            query: query_text.to_string(),
            tier,
            limit,
            time_range: None,
        };

        // 跨层级查询：Working → Session → Project → LongTerm
        let results = self.system.query_all(&query).await?;

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
    system: Arc<UnifiedMemorySystem>,
}

impl ClearMemoryTool {
    /// 临时（非持久）系统 —— 单元测试用。
    pub fn new() -> Self {
        Self {
            system: Arc::new(UnifiedMemorySystem::new("default")),
        }
    }

    /// 使用共享的统一记忆系统创建
    pub fn with_system(system: Arc<UnifiedMemorySystem>) -> Self {
        Self { system }
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

        let tier = parse_tier(tier_str).unwrap_or(MemoryTier::Working);

        // CM1 fix: actually clear only the specified tier
        let count = self.system.clear_tier(tier).await?;

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

fn parse_tier(s: &str) -> Option<MemoryTier> {
    match s {
        "working" => Some(MemoryTier::Working),
        "session" => Some(MemoryTier::Session),
        "project" => Some(MemoryTier::Project),
        "long_term" => Some(MemoryTier::LongTerm),
        _ => None,
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
        let system = Arc::new(UnifiedMemorySystem::new("test"));

        let save_tool = SaveMemoryTool::with_system(system.clone());
        save_tool
            .execute(json!({"content": "important fact: the sky is blue"}))
            .await
            .unwrap();

        let query_tool = QueryMemoryTool::with_system(system);
        let result = query_tool.execute(json!({"query": "sky"})).await.unwrap();
        assert!(result.contains("sky is blue"));
    }

    /// 跨层可见性：save 到 project 层（持久），query 从另一个工具实例
    /// （同系统）能查到 —— 修复"裂脑"的核心回归测试。
    #[tokio::test]
    async fn test_shared_system_cross_tier_visibility() {
        let dir = tempfile::tempdir().unwrap();
        let project = Arc::new(crate::memory_system::ProjectMemory::new(
            dir.path().to_path_buf(),
        ));
        let system = Arc::new(UnifiedMemorySystem::new("t").with_project(project.clone()));

        SaveMemoryTool::with_system(system.clone())
            .execute(json!({"content": "db url is postgres://prod", "tier": "project"}))
            .await
            .unwrap();

        let out = QueryMemoryTool::with_system(system)
            .execute(json!({"query": "postgres"}))
            .await
            .unwrap();
        assert!(out.contains("postgres://prod"), "out: {}", out);
    }

    /// 持久化回归：ProjectMemory 重启（新实例同目录）后 query 能查到
    /// 之前进程写入的条目 —— 修复"query 只看进程内缓存"。
    #[tokio::test]
    async fn test_project_memory_survives_restart() {
        let dir = tempfile::tempdir().unwrap();

        // "进程 1"：写入并落盘
        {
            let system = Arc::new(UnifiedMemorySystem::new("s1").with_project(Arc::new(
                crate::memory_system::ProjectMemory::new(dir.path().to_path_buf()),
            )));
            SaveMemoryTool::with_system(system)
                .execute(json!({"content": "deploy key rotation day", "tier": "project"}))
                .await
                .unwrap();
        }

        // "进程 2"：全新实例，同目录 —— 必须能查到
        {
            let system = Arc::new(UnifiedMemorySystem::new("s2").with_project(Arc::new(
                crate::memory_system::ProjectMemory::new(dir.path().to_path_buf()),
            )));
            let out = QueryMemoryTool::with_system(system)
                .execute(json!({"query": "rotation"}))
                .await
                .unwrap();
            assert!(out.contains("deploy key rotation day"), "out: {}", out);
        }
    }

    /// 会话结束晋升：高重要性 session 记忆进入 project 层，低重要性留下。
    #[tokio::test]
    async fn test_promote_session_end() {
        let dir = tempfile::tempdir().unwrap();
        let system = Arc::new(UnifiedMemorySystem::new("s").with_project(Arc::new(
            crate::memory_system::ProjectMemory::new(dir.path().to_path_buf()),
        )));

        let save = SaveMemoryTool::with_system(system.clone());
        save.execute(json!({"content": "critical insight worth keeping", "tier": "session", "importance": 0.95}))
            .await
            .unwrap();
        save.execute(
            json!({"content": "trivial scratch note", "tier": "session", "importance": 0.1}),
        )
        .await
        .unwrap();

        let promoted = system.promote_session_end(0.6).await.unwrap();
        assert_eq!(promoted, 1, "only the high-importance entry promotes");

        // Promoted entry is now queryable in project tier (cross-session)
        let query = MemoryQuery {
            query: "critical insight".into(),
            tier: Some(MemoryTier::Project),
            limit: Some(10),
            time_range: None,
        };
        let hits = system.query_all(&query).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("critical insight"));

        // Low-importance entry was NOT promoted
        let miss = MemoryQuery {
            query: "scratch note".into(),
            tier: Some(MemoryTier::Project),
            limit: Some(10),
            time_range: None,
        };
        assert!(system.query_all(&miss).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_promote_without_project_backend_is_noop() {
        let system = Arc::new(UnifiedMemorySystem::new("s"));
        system
            .store_at(MemoryTier::Session, "some session fact")
            .await
            .unwrap();
        assert_eq!(system.promote_session_end(0.0).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_importance_clamped_to_valid_range() {
        let tool = SaveMemoryTool::new();
        let out = tool
            .execute(json!({"content": "x", "importance": 42.0}))
            .await
            .unwrap();
        assert!(out.contains("Memory saved"));
    }
}
