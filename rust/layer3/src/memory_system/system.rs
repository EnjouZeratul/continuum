//! # Unified Memory System
//!
//! 整合四层记忆的统一接口。

use crate::memory_system::{session::SessionMemory, working::WorkingMemory, ImportanceScorer, MemoryStore};
use crate::types::{Layer3Result, MemoryEntry, MemoryQuery, MemoryTier};
use async_trait::async_trait;
use sh_layer2::generate_short_id;
use std::collections::HashMap;
use std::sync::Arc;

/// 统一记忆系统
///
/// 整合 Working, Session, Project, LongTerm 四层记忆。
#[allow(dead_code)]
pub struct UnifiedMemorySystem {
    /// 工作记忆（内存环形缓冲）
    working: Arc<WorkingMemory>,
    /// 会话记忆（会话级别 HashMap）
    session: Arc<SessionMemory>,
    /// 项目记忆（文件持久化）
    project: Option<Arc<dyn MemoryStore>>,
    /// 长期记忆（向量检索）
    long_term: Option<Arc<dyn MemoryStore>>,
    /// 当前会话 ID
    #[allow(dead_code)]
    session_id: String,
}

impl UnifiedMemorySystem {
    /// 创建新的记忆系统
    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        Self {
            working: Arc::new(WorkingMemory::new(100)),
            session: Arc::new(SessionMemory::new(&session_id)),
            project: None,
            long_term: None,
            session_id,
        }
    }

    /// 设置项目记忆存储
    pub fn with_project(mut self, project: Arc<dyn MemoryStore>) -> Self {
        self.project = Some(project);
        self
    }

    /// 设置长期记忆存储
    pub fn with_long_term(mut self, long_term: Arc<dyn MemoryStore>) -> Self {
        self.long_term = Some(long_term);
        self
    }

    /// 获取工作记忆
    pub fn working(&self) -> &WorkingMemory {
        &self.working
    }

    /// 获取会话记忆
    pub fn session(&self) -> &SessionMemory {
        &self.session
    }

    /// 存储到指定层级
    pub async fn store_at(
        &self,
        tier: MemoryTier,
        content: impl Into<String>,
    ) -> Layer3Result<String> {
        let entry = MemoryEntry {
            id: generate_short_id(),
            tier,
            content: content.into(),
            metadata: Default::default(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            importance: 0.5,
        };
        self.store_entry(entry).await
    }

    /// 存储完整条目（保留 metadata/importance，按 `entry.tier` 路由）。
    ///
    /// Project/LongTerm 后端未配置时降级到 session（与 store_at 一致）。
    pub async fn store_entry(&self, entry: MemoryEntry) -> Layer3Result<String> {
        match entry.tier {
            MemoryTier::Working => self.working.store(entry).await,
            MemoryTier::Session => self.session.store(entry).await,
            MemoryTier::Project => {
                if let Some(ref project) = self.project {
                    project.store(entry).await
                } else {
                    self.session.store(entry).await
                }
            }
            MemoryTier::LongTerm => {
                if let Some(ref long_term) = self.long_term {
                    long_term.store(entry).await
                } else {
                    self.session.store(entry).await
                }
            }
        }
    }

    /// 会话结束晋升：把 session 层中评分 ≥ `threshold` 的条目复制到
    /// project 层（跨会话可见、持久化）。
    ///
    /// - 复制而非移动：session 层随会话销毁，project 副本独立存活
    /// - 晋升副本的 `tier` 改写为 `Project`，id 不变（幂等：重复晋升
    ///   同一条目会得到同名文件，内容一致）
    /// - 未配置 project 后端时返回 Ok(0)（无持久层可晋升）
    ///
    /// 返回晋升条数。
    pub async fn promote_session_end(&self, threshold: f32) -> Layer3Result<usize> {
        let Some(ref project) = self.project else {
            return Ok(0);
        };
        let scorer = crate::memory_system::HeuristicImportanceScorer::new();
        let candidates = self.session.list(None).await?;
        let mut promoted = 0usize;
        for mut entry in candidates {
            if scorer.score(&entry) >= threshold {
                entry.tier = MemoryTier::Project;
                project.store(entry).await?;
                promoted += 1;
            }
        }
        Ok(promoted)
    }

    /// 跨层级查询
    ///
    /// 按 Working -> Session -> Project -> LongTerm 顺序查询
    pub async fn query_all(&self, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>> {
        let mut results = Vec::new();
        let limit = query.limit.unwrap_or(10);

        // Working
        let working_results = self.working.query(query).await?;
        results.extend(working_results);
        if results.len() >= limit {
            return Ok(results.into_iter().take(limit).collect());
        }

        // Session
        let session_results = self.session.query(query).await?;
        results.extend(session_results);
        if results.len() >= limit {
            return Ok(results.into_iter().take(limit).collect());
        }

        // Project
        if let Some(ref project) = self.project {
            let project_results = project.query(query).await?;
            results.extend(project_results);
            if results.len() >= limit {
                return Ok(results.into_iter().take(limit).collect());
            }
        }

        // LongTerm
        if let Some(ref long_term) = self.long_term {
            let long_term_results = long_term.query(query).await?;
            results.extend(long_term_results);
        }

        Ok(results.into_iter().take(limit).collect())
    }

    /// 获取层级统计
    pub async fn stats(&self) -> Layer3Result<HashMap<MemoryTier, usize>> {
        let mut stats = HashMap::new();
        stats.insert(MemoryTier::Working, self.working.count().await?);
        stats.insert(MemoryTier::Session, self.session.count().await?);
        if let Some(ref project) = self.project {
            stats.insert(MemoryTier::Project, project.count().await?);
        }
        if let Some(ref long_term) = self.long_term {
            stats.insert(MemoryTier::LongTerm, long_term.count().await?);
        }
        Ok(stats)
    }

    /// 清空指定层级
    pub async fn clear_tier(&self, tier: MemoryTier) -> Layer3Result<usize> {
        match tier {
            MemoryTier::Working => self.working.clear().await,
            MemoryTier::Session => self.session.clear().await,
            MemoryTier::Project => {
                if let Some(ref project) = self.project {
                    project.clear().await
                } else {
                    Ok(0)
                }
            }
            MemoryTier::LongTerm => {
                if let Some(ref long_term) = self.long_term {
                    long_term.clear().await
                } else {
                    Ok(0)
                }
            }
        }
    }
}

/// MemorySystem trait 实现
#[async_trait]
impl crate::memory_system::MemorySystem for UnifiedMemorySystem {
    async fn store(&self, tier: MemoryTier, content: String) -> Layer3Result<String> {
        self.store_at(tier, content).await
    }

    async fn get(&self, tier: MemoryTier, id: &str) -> Layer3Result<Option<MemoryEntry>> {
        match tier {
            MemoryTier::Working => self.working.get(id).await,
            MemoryTier::Session => self.session.get(id).await,
            MemoryTier::Project => {
                if let Some(ref project) = self.project {
                    project.get(id).await
                } else {
                    Ok(None)
                }
            }
            MemoryTier::LongTerm => {
                if let Some(ref long_term) = self.long_term {
                    long_term.get(id).await
                } else {
                    Ok(None)
                }
            }
        }
    }

    async fn query_all(&self, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>> {
        self.query_all(query).await
    }

    async fn query(&self, tier: MemoryTier, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>> {
        match tier {
            MemoryTier::Working => self.working.query(query).await,
            MemoryTier::Session => self.session.query(query).await,
            MemoryTier::Project => {
                if let Some(ref project) = self.project {
                    project.query(query).await
                } else {
                    Ok(Vec::new())
                }
            }
            MemoryTier::LongTerm => {
                if let Some(ref long_term) = self.long_term {
                    long_term.query(query).await
                } else {
                    Ok(Vec::new())
                }
            }
        }
    }

    async fn delete(&self, tier: MemoryTier, id: &str) -> Layer3Result<bool> {
        match tier {
            MemoryTier::Working => self.working.delete(id).await,
            MemoryTier::Session => self.session.delete(id).await,
            MemoryTier::Project => {
                if let Some(ref project) = self.project {
                    project.delete(id).await
                } else {
                    Ok(false)
                }
            }
            MemoryTier::LongTerm => {
                if let Some(ref long_term) = self.long_term {
                    long_term.delete(id).await
                } else {
                    Ok(false)
                }
            }
        }
    }

    async fn clear(&self, tier: MemoryTier) -> Layer3Result<usize> {
        self.clear_tier(tier).await
    }

    async fn stats(&self) -> Layer3Result<HashMap<MemoryTier, usize>> {
        self.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_memory_system() {
        let system = UnifiedMemorySystem::new("test-session");

        // 测试工作记忆
        let id = system
            .store_at(MemoryTier::Working, "test working memory")
            .await
            .unwrap();
        assert!(!id.is_empty());

        // 测试统计
        let stats = system.stats().await.unwrap();
        assert!(stats.contains_key(&MemoryTier::Working));
    }

    #[test]
    fn test_memory_system_creation() {
        let system = UnifiedMemorySystem::new("test");
        assert!(system.project.is_none());
        assert!(system.long_term.is_none());
    }
}
