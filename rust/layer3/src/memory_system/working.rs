//! # Working Memory
//!
//! 工作记忆：当前对话上下文，临时存储。

use crate::memory_system::{DecayPolicy, MemoryStore, TimeBasedDecay};
use crate::types::{Layer3Result, MemoryEntry, MemoryQuery, MemoryTier};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

/// Working Memory 实现
///
/// 使用环形缓冲区存储最近 N 条记忆。
#[allow(dead_code)]
pub struct WorkingMemory {
    /// 存储缓冲区
    buffer: Arc<RwLock<VecDeque<MemoryEntry>>>,
    /// 最大容量
    max_size: usize,
    /// 衰减策略
    #[allow(dead_code)]
    decay_policy: Box<dyn DecayPolicy>,
}

impl WorkingMemory {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
            decay_policy: Box::new(TimeBasedDecay::default()),
        }
    }

    /// Clear only entries matching the given tier (CM1 fix).
    /// Returns count of removed entries.
    pub async fn clear_tier(&self, tier: MemoryTier) -> Layer3Result<usize> {
        let mut buffer = self.buffer.write();
        let len_before = buffer.len();
        buffer.retain(|e| e.tier != tier);
        Ok(len_before - buffer.len())
    }

    /// Count entries in a specific tier.
    pub async fn count_tier(&self, tier: MemoryTier) -> Layer3Result<usize> {
        Ok(self.buffer.read().iter().filter(|e| e.tier == tier).count())
    }
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self::new(100)
    }
}

#[async_trait]
impl MemoryStore for WorkingMemory {
    fn tier(&self) -> MemoryTier {
        MemoryTier::Working
    }

    async fn store(&self, entry: MemoryEntry) -> Layer3Result<String> {
        let mut buffer = self.buffer.write();
        if buffer.len() >= self.max_size {
            buffer.pop_front();
        }
        let id = entry.id.clone();
        buffer.push_back(entry);
        Ok(id)
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<MemoryEntry>> {
        let buffer = self.buffer.read();
        Ok(buffer.iter().find(|e| e.id == id).cloned())
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let mut buffer = self.buffer.write();
        let len_before = buffer.len();
        buffer.retain(|e| e.id != id);
        Ok(buffer.len() < len_before)
    }

    async fn query(&self, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>> {
        let buffer = self.buffer.read();
        let results: Vec<MemoryEntry> = buffer
            .iter()
            .filter(|e| {
                if let Some(tier) = query.tier {
                    if e.tier != tier {
                        return false;
                    }
                }
                e.content.contains(&query.query)
            })
            .take(query.limit.unwrap_or(10))
            .cloned()
            .collect();
        Ok(results)
    }

    async fn list(&self, limit: Option<usize>) -> Layer3Result<Vec<MemoryEntry>> {
        let buffer = self.buffer.read();
        Ok(buffer
            .iter()
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect())
    }

    async fn clear(&self) -> Layer3Result<usize> {
        let mut buffer = self.buffer.write();
        let count = buffer.len();
        buffer.clear();
        Ok(count)
    }

    async fn count(&self) -> Layer3Result<usize> {
        Ok(self.buffer.read().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_working_memory_store() {
        let memory = WorkingMemory::new(10);
        let entry = MemoryEntry {
            id: "test-1".to_string(),
            tier: MemoryTier::Working,
            content: "test content".to_string(),
            metadata: Default::default(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            importance: 0.5,
        };
        memory.store(entry).await.unwrap();
        assert_eq!(memory.count().await.unwrap(), 1);
    }
}
