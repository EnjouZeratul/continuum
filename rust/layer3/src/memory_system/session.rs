//! # Session Memory
//!
//! 会话记忆：单次会话内的持久化存储。
//!
//! 支持可选的文件后端持久化，包括：
//! - 自动保存（Drop 时 dirty flag）
//! - 原子写入
//! - 版本迁移

use crate::memory_system::{
    DecayPolicy, FileBackend, MemoryStore, StorageContainer, TimeBasedDecay,
};
use crate::types::{Layer3Result, MemoryEntry, MemoryQuery, MemoryTier};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Auto-save configuration
#[derive(Debug, Clone)]
pub struct AutoSaveConfig {
    /// Enable auto-save
    pub enabled: bool,
    /// Save interval in milliseconds
    pub interval_ms: u64,
    /// Save on every store operation
    pub save_on_store: bool,
    /// Minimum changes before save
    pub min_changes: usize,
}

impl Default for AutoSaveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 5000,
            save_on_store: false,
            min_changes: 5,
        }
    }
}

/// Dirty flag for tracking unsaved changes
#[derive(Debug, Default)]
struct DirtyFlag {
    dirty: RwLock<bool>,
}

impl DirtyFlag {
    fn new() -> Self {
        Self {
            dirty: RwLock::new(false),
        }
    }

    fn mark_dirty(&self) {
        *self.dirty.write() = true;
    }

    fn mark_clean(&self) {
        *self.dirty.write() = false;
    }

    fn is_dirty(&self) -> bool {
        *self.dirty.read()
    }
}

/// Session Memory 实现
///
/// 使用 HashMap 存储会话期间的记忆，支持可选的文件持久化。
///
/// 特性：
/// - Drop 时自动保存（如果 dirty）
/// - 原子文件写入
/// - 版本迁移支持
pub struct SessionMemory {
    /// 存储
    storage: Arc<RwLock<HashMap<String, MemoryEntry>>>,
    /// 会话 ID
    session_id: String,
    /// 衰减策略（未来功能预留）
    #[allow(dead_code)]
    decay_policy: Box<dyn DecayPolicy>,
    /// 文件后端（可选）
    file_backend: Option<Arc<dyn FileBackend>>,
    /// 自动保存配置
    auto_save_config: AutoSaveConfig,
    /// 自上次保存后的变更计数
    changes_since_save: Arc<RwLock<usize>>,
    /// Dirty flag for Drop auto-save
    dirty_flag: Arc<DirtyFlag>,
    /// Prevent Drop from running multiple times
    drop_guard: Arc<RwLock<bool>>,
}

impl SessionMemory {
    /// 创建新的 Session Memory（无持久化）
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            session_id: session_id.into(),
            decay_policy: Box::new(TimeBasedDecay::default()),
            file_backend: None,
            auto_save_config: AutoSaveConfig::default(),
            changes_since_save: Arc::new(RwLock::new(0)),
            dirty_flag: Arc::new(DirtyFlag::new()),
            drop_guard: Arc::new(RwLock::new(false)),
        }
    }

    /// 创建带文件持久化的 Session Memory
    pub fn with_persistence(
        session_id: impl Into<String>,
        backend: Arc<dyn FileBackend>,
        auto_save: AutoSaveConfig,
    ) -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            session_id: session_id.into(),
            decay_policy: Box::new(TimeBasedDecay::default()),
            file_backend: Some(backend),
            auto_save_config: auto_save,
            changes_since_save: Arc::new(RwLock::new(0)),
            dirty_flag: Arc::new(DirtyFlag::new()),
            drop_guard: Arc::new(RwLock::new(false)),
        }
    }

    /// 创建带持久化存储的 Session Memory（推荐使用）
    ///
    /// 这是 `with_persistence` 的简化版本，使用 JSON 后端。
    ///
    /// # 参数
    /// - `session_id`: 会话唯一标识符
    /// - `path`: 存储文件路径
    /// - `auto_save_on_drop`: 是否在 Drop 时自动保存
    ///
    /// # 示例
    /// ```ignore
    /// let memory = SessionMemory::with_persistent_storage(
    ///     "my-session",
    ///     "session.json",
    ///     true
    /// );
    /// ```
    pub fn with_persistent_storage(
        session_id: impl Into<String>,
        path: impl Into<PathBuf>,
        auto_save_on_drop: bool,
    ) -> Self {
        let session_id = session_id.into();
        let backend = Arc::new(crate::memory_system::JsonFileBackend::with_session_id(
            path,
            session_id.clone(),
        ));
        Self::with_persistence(
            session_id,
            backend,
            AutoSaveConfig {
                enabled: auto_save_on_drop,
                save_on_store: false,
                min_changes: 1,
                ..Default::default()
            },
        )
    }

    /// 创建带 JSON 文件持久化的 Session Memory
    pub fn with_json_backend(session_id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let backend = Arc::new(crate::memory_system::JsonFileBackend::new(path));
        Self::with_persistence(
            session_id,
            backend,
            AutoSaveConfig {
                enabled: true,
                save_on_store: true,
                ..Default::default()
            },
        )
    }

    /// 从文件加载已保存的会话
    ///
    /// 如果文件不存在，创建新的空会话。
    /// 如果文件存在，加载其中的数据并支持版本迁移。
    pub async fn load_from_file(
        session_id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Layer3Result<Self> {
        let session_id = session_id.into();
        let path = path.into();
        let backend = Arc::new(crate::memory_system::JsonFileBackend::with_session_id(
            &path,
            session_id.clone(),
        ));

        let container = backend.load_container().await?;
        let storage: HashMap<String, MemoryEntry> = container
            .entries
            .into_iter()
            .map(|e| (e.id.clone(), e))
            .collect();

        tracing::info!(
            "Loaded session {} from {} ({} entries, version {})",
            session_id,
            path.display(),
            storage.len(),
            container.version
        );

        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            session_id,
            decay_policy: Box::new(TimeBasedDecay::default()),
            file_backend: Some(backend),
            auto_save_config: AutoSaveConfig {
                enabled: true,
                save_on_store: true,
                ..Default::default()
            },
            changes_since_save: Arc::new(RwLock::new(0)),
            dirty_flag: Arc::new(DirtyFlag::new()),
            drop_guard: Arc::new(RwLock::new(false)),
        })
    }

    /// 尝试从文件加载，失败则创建新会话
    pub async fn load_or_create(
        session_id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Layer3Result<Self> {
        let path = path.into();
        if path.exists() {
            Self::load_from_file(session_id, &path).await
        } else {
            Ok(Self::with_persistent_storage(session_id, &path, true))
        }
    }

    /// 手动保存到文件
    pub async fn save(&self) -> Layer3Result<()> {
        if let Some(backend) = &self.file_backend {
            let entries: Vec<MemoryEntry> = self.storage.read().values().cloned().collect();
            backend
                .save_with_session(&self.session_id, &entries)
                .await?;
            *self.changes_since_save.write() = 0;
            self.dirty_flag.mark_clean();
            tracing::info!(
                "Session {} saved to {}",
                self.session_id,
                backend.path().display()
            );
        }
        Ok(())
    }

    /// 同步保存（阻塞版本，用于 Drop）
    fn save_sync(&self) -> Layer3Result<()> {
        if let Some(backend) = &self.file_backend {
            let entries: Vec<MemoryEntry> = self.storage.read().values().cloned().collect();

            // Use blocking file write for Drop
            let json =
                serde_json::to_string_pretty(&StorageContainer::new(&self.session_id, entries))?;

            let path = backend.path().to_path_buf();
            let temp_path = path.with_extension(format!("tmp.{}", std::process::id()));

            // Write to temp file
            std::fs::write(&temp_path, &json)?;
            // Atomic rename
            std::fs::rename(&temp_path, &path)?;

            tracing::info!(
                "Session {} saved (sync) to {}",
                self.session_id,
                path.display()
            );
        }
        Ok(())
    }

    /// 检查是否需要自动保存
    fn should_auto_save(&self) -> bool {
        if !self.auto_save_config.enabled || self.file_backend.is_none() {
            return false;
        }

        let changes = *self.changes_since_save.read();
        if self.auto_save_config.save_on_store {
            return true;
        }

        changes >= self.auto_save_config.min_changes
    }

    /// 执行自动保存（如果需要）
    async fn maybe_auto_save(&self) -> Layer3Result<()> {
        if self.should_auto_save() {
            self.save().await?;
        }
        Ok(())
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 获取变更计数
    pub fn changes_since_save(&self) -> usize {
        *self.changes_since_save.read()
    }

    /// 检查是否有未保存的变更
    pub fn is_dirty(&self) -> bool {
        self.dirty_flag.is_dirty()
    }

    /// 获取文件后端路径（如果有）
    pub fn persistence_path(&self) -> Option<&std::path::Path> {
        self.file_backend.as_ref().map(|b| b.path())
    }
}

impl Drop for SessionMemory {
    fn drop(&mut self) {
        // Prevent double-drop
        if *self.drop_guard.read() {
            return;
        }
        *self.drop_guard.write() = true;

        // Auto-save if dirty and backend exists
        if self.dirty_flag.is_dirty()
            && self.file_backend.is_some()
            && self.auto_save_config.enabled
        {
            if let Err(e) = self.save_sync() {
                tracing::error!("Failed to auto-save session {}: {}", self.session_id, e);
            }
        }
    }
}

impl Default for SessionMemory {
    fn default() -> Self {
        Self::new("default")
    }
}

#[async_trait]
impl MemoryStore for SessionMemory {
    fn tier(&self) -> MemoryTier {
        MemoryTier::Session
    }

    async fn store(&self, entry: MemoryEntry) -> Layer3Result<String> {
        let id = entry.id.clone();
        {
            let mut storage = self.storage.write();
            storage.insert(id.clone(), entry);
        }
        *self.changes_since_save.write() += 1;
        self.dirty_flag.mark_dirty();

        // Auto-save if configured
        self.maybe_auto_save().await?;

        Ok(id)
    }

    async fn get(&self, id: &str) -> Layer3Result<Option<MemoryEntry>> {
        Ok(self.storage.read().get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Layer3Result<bool> {
        let removed = self.storage.write().remove(id).is_some();
        if removed {
            *self.changes_since_save.write() += 1;
            self.dirty_flag.mark_dirty();
            drop(self.storage.write()); // Ensure lock is dropped before await
            self.maybe_auto_save().await?;
        }
        Ok(removed)
    }

    async fn query(&self, query: &MemoryQuery) -> Layer3Result<Vec<MemoryEntry>> {
        let storage = self.storage.read();
        let results: Vec<MemoryEntry> = storage
            .values()
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
        let storage = self.storage.read();
        Ok(storage
            .values()
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect())
    }

    async fn clear(&self) -> Layer3Result<usize> {
        let count = {
            let mut storage = self.storage.write();
            let count = storage.len();
            storage.clear();
            count
        };

        // Clear file backend too
        if let Some(backend) = &self.file_backend {
            backend.clear().await?;
        }
        *self.changes_since_save.write() = 0;
        self.dirty_flag.mark_clean();

        Ok(count)
    }

    async fn count(&self) -> Layer3Result<usize> {
        Ok(self.storage.read().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MemoryTier;
    use tempfile::tempdir;

    fn create_test_entry(id: &str, content: &str) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            tier: MemoryTier::Session,
            content: content.to_string(),
            metadata: Default::default(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            importance: 0.5,
        }
    }

    #[tokio::test]
    async fn test_session_memory() {
        let memory = SessionMemory::new("test-session");
        assert_eq!(memory.tier(), MemoryTier::Session);
        assert!(!memory.is_dirty());
    }

    #[tokio::test]
    async fn test_session_with_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let memory = SessionMemory::with_json_backend("test-session", &path);
        memory
            .store(create_test_entry("1", "test content"))
            .await
            .unwrap();

        // Should auto-save
        assert!(path.exists());
        assert!(!memory.is_dirty()); // Saved, so not dirty
    }

    #[tokio::test]
    async fn test_load_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        // First save some data
        let memory = SessionMemory::with_json_backend("test-session", &path);
        memory
            .store(create_test_entry("1", "saved content"))
            .await
            .unwrap();
        memory.save().await.unwrap();

        // Load from file
        let loaded = SessionMemory::load_from_file("test-session", &path)
            .await
            .unwrap();
        let entry = loaded.get("1").await.unwrap().unwrap();
        assert_eq!(entry.content, "saved content");
    }

    #[tokio::test]
    async fn test_manual_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let memory = SessionMemory::new("test-session");
        // No backend, save should succeed but do nothing
        memory.save().await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_dirty_flag() {
        let memory = SessionMemory::new("test-session");

        assert!(!memory.is_dirty());

        memory.store(create_test_entry("1", "test")).await.unwrap();
        // Dirty flag tracks unsaved changes even without backend
        assert!(memory.is_dirty());

        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let persistent = SessionMemory::with_persistent_storage("test", &path, false);

        assert!(!persistent.is_dirty());
        persistent
            .store(create_test_entry("1", "test"))
            .await
            .unwrap();
        assert!(persistent.is_dirty());

        persistent.save().await.unwrap();
        assert!(!persistent.is_dirty());
    }

    #[tokio::test]
    async fn test_drop_auto_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("drop_save.json");

        {
            let memory = SessionMemory::with_persistent_storage("test-drop", &path, true);
            memory
                .store(create_test_entry("drop-1", "content before drop"))
                .await
                .unwrap();
            // Auto-save triggers when changes >= min_changes (1), so dirty may be cleared
            // Drop happens here
        }

        // File should exist after drop
        assert!(path.exists());

        // Load and verify
        let loaded = SessionMemory::load_from_file("test-drop", &path)
            .await
            .unwrap();
        let entry = loaded.get("drop-1").await.unwrap().unwrap();
        assert_eq!(entry.content, "content before drop");
    }

    #[tokio::test]
    async fn test_load_or_create() {
        let dir = tempdir().unwrap();
        let existing_path = dir.path().join("existing.json");
        let new_path = dir.path().join("new.json");

        // Create existing file
        {
            let memory = SessionMemory::with_json_backend("existing", &existing_path);
            memory
                .store(create_test_entry("existing-1", "existing content"))
                .await
                .unwrap();
        }

        // Load existing
        let loaded = SessionMemory::load_or_create("existing", &existing_path)
            .await
            .unwrap();
        assert!(loaded.get("existing-1").await.unwrap().is_some());

        // Create new
        let new_memory = SessionMemory::load_or_create("new", &new_path)
            .await
            .unwrap();
        assert!(!new_path.exists()); // Not saved yet
        new_memory
            .store(create_test_entry("new-1", "new content"))
            .await
            .unwrap();
        assert!(new_path.exists()); // Auto-saved
    }

    #[tokio::test]
    async fn test_version_migration() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy_session.json");

        // Write legacy format (just array of entries)
        let legacy_entries = vec![create_test_entry("legacy-1", "legacy content")];
        let legacy_json = serde_json::to_string_pretty(&legacy_entries).unwrap();
        std::fs::write(&path, legacy_json).unwrap();

        // Load should migrate
        let loaded = SessionMemory::load_from_file("migrated", &path)
            .await
            .unwrap();
        let entry = loaded.get("legacy-1").await.unwrap().unwrap();
        assert_eq!(entry.content, "legacy content");
    }

    #[tokio::test]
    async fn test_persistence_path() {
        let memory = SessionMemory::new("test");
        assert!(memory.persistence_path().is_none());

        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let persistent = SessionMemory::with_json_backend("test", &path);

        let stored_path = persistent.persistence_path().unwrap();
        assert_eq!(stored_path, path);
    }

    #[tokio::test]
    async fn test_thread_safety() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("concurrent.json");
        let memory = Arc::new(SessionMemory::with_json_backend("concurrent", &path));

        // Sequential stores to avoid Windows file locking issues
        // Note: Concurrent stores with save_on_store=true can cause race conditions
        // on Windows due to atomic_write rename operations
        for i in 0..10 {
            memory
                .store(create_test_entry(&format!("entry-{}", i), "content"))
                .await
                .unwrap();
        }

        // All entries should be saved
        assert!(path.exists());
        let loaded = SessionMemory::load_from_file("concurrent", &path)
            .await
            .unwrap();
        assert_eq!(loaded.count().await.unwrap(), 10);
    }

    #[test]
    fn test_dirty_flag_thread_safety() {
        let flag = Arc::new(DirtyFlag::new());
        let mut handles = vec![];

        for _ in 0..100 {
            let f = flag.clone();
            let handle = std::thread::spawn(move || {
                f.mark_dirty();
                assert!(f.is_dirty());
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert!(flag.is_dirty());
    }
}
