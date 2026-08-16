//! # Memory System
//!
//! 分层记忆系统：Working -> Session -> Project -> LongTerm

pub mod file_backend;
pub mod long_term;
pub mod project;
pub mod session;
pub mod system;
pub mod working;

// Re-export unified system and implementations
pub use file_backend::{FileBackend, JsonFileBackend, StorageContainer};
pub use long_term::LongTermMemory;
pub use project::ProjectMemory;
pub use session::SessionMemory;
pub use system::UnifiedMemorySystem;
pub use working::WorkingMemory;

use crate::types::{Layer3Result, MemoryEntry, MemoryQuery, MemoryTier};
use async_trait::async_trait;
use std::collections::HashMap;

/// 记忆存储 trait
///
/// 定义单层记忆存储的核心接口。
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// 记忆层级
    fn tier(&self) -> MemoryTier;

    /// 存储记忆条目
    async fn store(&self, entry: MemoryEntry) -> Layer3Result<String>;

    /// 获取记忆条目
    async fn get(&self, id: &str) -> Layer3Result<Option<MemoryEntry>>;

    /// 删除记忆条目
    async fn delete(&self, id: &str) -> Layer3Result<bool>;

    /// 查询记忆（按内容搜索）
    async fn query(&self, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>>;

    /// 列出所有记忆（按时间排序）
    async fn list(&self, limit: Option<usize>) -> Layer3Result<Vec<MemoryEntry>>;

    /// 清空该层所有记忆
    async fn clear(&self) -> Layer3Result<usize>;

    /// 记忆条目数量
    async fn count(&self) -> Layer3Result<usize>;
}

/// 记忆系统 trait
///
/// 统一管理所有记忆层级的接口。
#[async_trait]
pub trait MemorySystem: Send + Sync {
    /// 存储记忆到指定层级
    async fn store(&self, tier: MemoryTier, content: String) -> Layer3Result<String>;

    /// 从指定层级获取记忆
    async fn get(&self, tier: MemoryTier, id: &str) -> Layer3Result<Option<MemoryEntry>>;

    /// 跨层级查询记忆
    ///
    /// 默认从 Working -> Session -> Project -> LongTerm 依次查询
    async fn query_all(&self, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>>;

    /// 在指定层级查询
    async fn query(&self, tier: MemoryTier, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>>;

    /// 删除指定层级记忆
    async fn delete(&self, tier: MemoryTier, id: &str) -> Layer3Result<bool>;

    /// 清空指定层级
    async fn clear(&self, tier: MemoryTier) -> Layer3Result<usize>;

    /// 获取层级统计
    async fn stats(&self) -> Layer3Result<HashMap<MemoryTier, usize>>;
}

/// 记忆重要性评估 trait
pub trait ImportanceScorer: Send + Sync {
    /// 计算记忆重要性分数 (0.0-1.0)
    fn score(&self, entry: &MemoryEntry) -> f32;
}

/// 记忆衰减策略 trait
///
/// 定义记忆如何随时间衰减重要性。
pub trait DecayPolicy: Send + Sync {
    /// 计算衰减后的重要性
    fn decay(&self, entry: &MemoryEntry, current_time: chrono::DateTime<chrono::Utc>) -> f32;

    /// 是否应该清理该记忆
    fn should_evict(&self, entry: &MemoryEntry) -> bool;
}

/// 默认衰减策略：基于时间和访问频率
pub struct TimeBasedDecay {
    /// 衰减率（每天）
    decay_rate: f32,
    /// 最低保留重要性
    min_threshold: f32,
}

impl TimeBasedDecay {
    pub fn new(decay_rate: f32, min_threshold: f32) -> Self {
        Self {
            decay_rate,
            min_threshold,
        }
    }
}

impl Default for TimeBasedDecay {
    fn default() -> Self {
        Self {
            decay_rate: 0.1,
            min_threshold: 0.1,
        }
    }
}

impl DecayPolicy for TimeBasedDecay {
    fn decay(&self, entry: &MemoryEntry, current_time: chrono::DateTime<chrono::Utc>) -> f32 {
        let days_since_access = (current_time - entry.last_accessed).num_days() as f32;
        let decayed = entry.importance * (1.0 - self.decay_rate * days_since_access);
        decayed.max(0.0)
    }

    fn should_evict(&self, entry: &MemoryEntry) -> bool {
        entry.importance < self.min_threshold
    }
}

/// 启发式重要性评分器：`ImportanceScorer` 的具体实现。
///
/// 组合三个信号（各截断到 [0,1]）：
/// - 基础分：条目自带的 `importance` 字段（保存时由工具设定）
/// - 访问加成：`access_count` 越多越重要（对数缓增，封顶 +0.2）
/// - 时效加成：最近 24h 内访问过的条目 +0.1
///
/// 用于会话结束时决定哪些 session 记忆晋升到 project 层。
pub struct HeuristicImportanceScorer {
    now: chrono::DateTime<chrono::Utc>,
}

impl HeuristicImportanceScorer {
    pub fn new() -> Self {
        Self {
            now: chrono::Utc::now(),
        }
    }
}

impl Default for HeuristicImportanceScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportanceScorer for HeuristicImportanceScorer {
    fn score(&self, entry: &MemoryEntry) -> f32 {
        let base = entry.importance.clamp(0.0, 1.0);
        let access_bonus = ((entry.access_count as f32).ln_1p() * 0.1).min(0.2);
        let recent_bonus = if (self.now - entry.last_accessed).num_hours() < 24 {
            0.1
        } else {
            0.0
        };
        (base + access_bonus + recent_bonus).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decay_policy_default() {
        let decay = TimeBasedDecay::default();
        assert_eq!(decay.decay_rate, 0.1);
        assert_eq!(decay.min_threshold, 0.1);
    }
}
