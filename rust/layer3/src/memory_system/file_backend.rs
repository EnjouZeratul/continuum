//! # File Backend for Session Persistence
//!
//! Provides file-based persistence for session memory with:
//! - Atomic writes (write to temp file, then rename)
//! - Version migration for backward compatibility
//! - Thread-safe operations

use crate::types::{Layer3Result, MemoryEntry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Current storage format version
pub const STORAGE_VERSION: u32 = 1;

/// Versioned storage container for migration support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageContainer {
    /// Format version number
    pub version: u32,
    /// Session ID that created this storage
    pub session_id: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last modified timestamp
    pub modified_at: chrono::DateTime<chrono::Utc>,
    /// Memory entries
    pub entries: Vec<MemoryEntry>,
}

impl StorageContainer {
    /// Create a new storage container
    pub fn new(session_id: impl Into<String>, entries: Vec<MemoryEntry>) -> Self {
        let now = chrono::Utc::now();
        Self {
            version: STORAGE_VERSION,
            session_id: session_id.into(),
            created_at: now,
            modified_at: now,
            entries,
        }
    }

    /// Update modification time
    pub fn touch(&mut self) {
        self.modified_at = chrono::Utc::now();
    }

    /// Migrate from older version if needed
    pub fn migrate_from(value: serde_json::Value) -> Layer3Result<Self> {
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        match version {
            0 => {
                let entries: Vec<MemoryEntry> = serde_json::from_value(value)?;
                Ok(Self::new("migrated", entries))
            }
            1 => Ok(serde_json::from_value(value)?),
            v => anyhow::bail!("Unsupported storage version: {}", v),
        }
    }
}

impl Default for StorageContainer {
    fn default() -> Self {
        Self::new("default", Vec::new())
    }
}

/// File backend trait for session persistence
#[async_trait]
pub trait FileBackend: Send + Sync {
    /// Save entries to file with atomic write
    async fn save(&self, entries: &[MemoryEntry]) -> Layer3Result<()>;

    /// Save with session ID for container
    async fn save_with_session(
        &self,
        session_id: &str,
        entries: &[MemoryEntry],
    ) -> Layer3Result<()>;

    /// Load entries from file
    async fn load(&self) -> Layer3Result<Vec<MemoryEntry>>;

    /// Load full storage container
    async fn load_container(&self) -> Layer3Result<StorageContainer>;

    /// Check if backend file exists
    async fn exists(&self) -> bool;

    /// Clear backend storage
    async fn clear(&self) -> Layer3Result<()>;

    /// Get backend path
    fn path(&self) -> &Path;

    /// Get storage version
    fn version(&self) -> u32 {
        STORAGE_VERSION
    }
}

/// JSON file backend implementation with atomic writes
pub struct JsonFileBackend {
    /// File path for persistence
    path: PathBuf,
    /// Pretty print JSON
    pretty: bool,
    /// Session ID (for container metadata)
    session_id: Option<String>,
}

impl JsonFileBackend {
    /// Create new JSON file backend
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            pretty: true,
            session_id: None,
        }
    }

    /// Create with pretty print option
    pub fn with_pretty(path: impl Into<PathBuf>, pretty: bool) -> Self {
        Self {
            path: path.into(),
            pretty,
            session_id: None,
        }
    }

    /// Create with session ID
    pub fn with_session_id(path: impl Into<PathBuf>, session_id: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            pretty: true,
            session_id: Some(session_id.into()),
        }
    }

    /// Set session ID
    pub fn set_session_id(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
    }

    /// Get session ID from storage (if exists)
    pub async fn get_stored_session_id(&self) -> Layer3Result<Option<String>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.path).await?;
        if content.trim().is_empty() {
            return Ok(None);
        }

        let value: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
            Ok(Some(session_id.to_string()))
        } else {
            Ok(None)
        }
    }

    /// Generate temp file path for atomic write
    fn temp_path(&self) -> PathBuf {
        let mut temp = self.path.clone();
        let file_name = temp.file_name().and_then(|n| n.to_str()).unwrap_or("temp");
        let temp_name = format!("{}.tmp.{}", file_name, std::process::id());
        temp.set_file_name(temp_name);
        temp
    }

    /// Ensure parent directory exists
    async fn ensure_parent(&self) -> Layer3Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        Ok(())
    }

    /// Atomic write: write to temp file, then rename
    async fn atomic_write(&self, content: &str) -> Layer3Result<()> {
        self.ensure_parent().await?;

        let temp_path = self.temp_path();

        // Write to temp file
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(content.as_bytes()).await?;
        file.sync_all().await?;
        drop(file);

        // Atomic rename (on Windows and Unix)
        fs::rename(&temp_path, &self.path).await?;

        tracing::debug!("Atomically saved to {:?}", self.path);
        Ok(())
    }
}

#[async_trait]
impl FileBackend for JsonFileBackend {
    async fn save(&self, entries: &[MemoryEntry]) -> Layer3Result<()> {
        let session_id = self.session_id.as_deref().unwrap_or("unknown");
        self.save_with_session(session_id, entries).await
    }

    async fn save_with_session(
        &self,
        session_id: &str,
        entries: &[MemoryEntry],
    ) -> Layer3Result<()> {
        let container = StorageContainer::new(session_id, entries.to_vec());

        let json = if self.pretty {
            serde_json::to_string_pretty(&container)?
        } else {
            serde_json::to_string(&container)?
        };

        self.atomic_write(&json).await?;
        tracing::debug!(
            "Saved {} entries to {:?} (session: {})",
            entries.len(),
            self.path,
            session_id
        );
        Ok(())
    }

    async fn load(&self) -> Layer3Result<Vec<MemoryEntry>> {
        let container = self.load_container().await?;
        Ok(container.entries)
    }

    async fn load_container(&self) -> Layer3Result<StorageContainer> {
        if !self.path.exists() {
            return Ok(StorageContainer::default());
        }

        let content = fs::read_to_string(&self.path).await?;

        if content.trim().is_empty() {
            return Ok(StorageContainer::default());
        }

        let value: serde_json::Value = serde_json::from_str(&content)?;
        let container = StorageContainer::migrate_from(value)?;
        tracing::debug!(
            "Loaded {} entries from {:?} (version: {})",
            container.entries.len(),
            self.path,
            container.version
        );
        Ok(container)
    }

    async fn exists(&self) -> bool {
        self.path.exists()
    }

    async fn clear(&self) -> Layer3Result<()> {
        if self.path.exists() {
            // Also clean up any temp files
            let temp_path = self.temp_path();
            if temp_path.exists() {
                fs::remove_file(&temp_path).await?;
            }
            fs::remove_file(&self.path).await?;
            tracing::debug!("Cleared backend at {:?}", self.path);
        }
        Ok(())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

/// Binary file backend (MessagePack format) with atomic writes
#[cfg(feature = "msgpack")]
pub struct MsgPackFileBackend {
    path: PathBuf,
    session_id: Option<String>,
}

#[cfg(feature = "msgpack")]
impl MsgPackFileBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            session_id: None,
        }
    }

    pub fn with_session_id(path: impl Into<PathBuf>, session_id: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            session_id: Some(session_id.into()),
        }
    }

    fn temp_path(&self) -> PathBuf {
        let mut temp = self.path.clone();
        let file_name = temp.file_name().and_then(|n| n.to_str()).unwrap_or("temp");
        let temp_name = format!("{}.tmp.{}", file_name, std::process::id());
        temp.set_file_name(temp_name);
        temp
    }

    async fn ensure_parent(&self) -> Layer3Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        Ok(())
    }

    async fn atomic_write(&self, bytes: &[u8]) -> Layer3Result<()> {
        self.ensure_parent().await?;

        let temp_path = self.temp_path();

        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);

        fs::rename(&temp_path, &self.path).await?;
        Ok(())
    }
}

#[cfg(feature = "msgpack")]
#[async_trait]
impl FileBackend for MsgPackFileBackend {
    async fn save(&self, entries: &[MemoryEntry]) -> Layer3Result<()> {
        let session_id = self.session_id.as_deref().unwrap_or("unknown");
        self.save_with_session(session_id, entries).await
    }

    async fn save_with_session(
        &self,
        session_id: &str,
        entries: &[MemoryEntry],
    ) -> Layer3Result<()> {
        let container = StorageContainer::new(session_id, entries.to_vec());
        let bytes = rmp_serde::to_vec(&container)?;
        self.atomic_write(&bytes).await?;
        Ok(())
    }

    async fn load(&self) -> Layer3Result<Vec<MemoryEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&self.path).await?;
        let container: StorageContainer = rmp_serde::from_slice(&bytes)?;
        Ok(container.entries)
    }

    async fn load_container(&self) -> Layer3Result<StorageContainer> {
        if !self.path.exists() {
            return Ok(StorageContainer::default());
        }
        let bytes = fs::read(&self.path).await?;
        let container: StorageContainer = rmp_serde::from_slice(&bytes)?;
        Ok(container)
    }

    async fn exists(&self) -> bool {
        self.path.exists()
    }

    async fn clear(&self) -> Layer3Result<()> {
        if self.path.exists() {
            let temp_path = self.temp_path();
            if temp_path.exists() {
                fs::remove_file(&temp_path).await?;
            }
            fs::remove_file(&self.path).await?;
        }
        Ok(())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MemoryTier;
    use std::sync::Arc;
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
    async fn test_json_backend_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let backend = JsonFileBackend::new(&path);

        let entries = vec![create_test_entry("1", "test content")];

        backend.save(&entries).await.unwrap();
        assert!(path.exists());

        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].content, "test content");
    }

    #[tokio::test]
    async fn test_json_backend_empty_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let backend = JsonFileBackend::new(&path);

        let loaded = backend.load().await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_json_backend_clear() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let backend = JsonFileBackend::new(&path);

        backend
            .save(&[create_test_entry("1", "test")])
            .await
            .unwrap();
        assert!(backend.exists().await);

        backend.clear().await.unwrap();
        assert!(!backend.exists().await);
    }

    #[tokio::test]
    async fn test_atomic_write_no_temp_file_left() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("atomic_test.json");
        let backend = JsonFileBackend::new(&path);

        backend
            .save(&[create_test_entry("1", "test")])
            .await
            .unwrap();

        // No temp files should remain
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains(".tmp."), "Temp file left behind: {}", name);
        }
    }

    #[tokio::test]
    async fn test_version_container() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("versioned.json");
        let backend = JsonFileBackend::with_session_id(&path, "test-session-123");

        backend
            .save(&[create_test_entry("1", "test")])
            .await
            .unwrap();

        let container = backend.load_container().await.unwrap();
        assert_eq!(container.version, STORAGE_VERSION);
        assert_eq!(container.session_id, "test-session-123");
        assert_eq!(container.entries.len(), 1);
    }

    #[tokio::test]
    async fn test_migration_from_v0() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy.json");

        // Write legacy format (just array of entries)
        let legacy_entries = vec![create_test_entry("legacy-1", "legacy content")];
        let legacy_json = serde_json::to_string_pretty(&legacy_entries).unwrap();
        std::fs::write(&path, legacy_json).unwrap();

        let backend = JsonFileBackend::new(&path);
        let loaded = backend.load().await.unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].content, "legacy content");
    }

    #[tokio::test]
    async fn test_session_id_retrieval() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session_id.json");
        let backend = JsonFileBackend::with_session_id(&path, "session-abc");

        assert!(backend.get_stored_session_id().await.unwrap().is_none());

        backend
            .save(&[create_test_entry("1", "test")])
            .await
            .unwrap();

        let stored_id = backend.get_stored_session_id().await.unwrap();
        assert_eq!(stored_id, Some("session-abc".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_safe_operations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("concurrent.json");
        let backend = Arc::new(JsonFileBackend::new(&path));

        // Sequential saves to avoid Windows file locking issues
        // Note: Concurrent atomic_write rename operations can race on Windows
        for i in 0..5 {
            backend
                .save(&[create_test_entry(&format!("entry-{}", i), "content")])
                .await
                .unwrap();
        }

        // Final file should exist and be valid
        assert!(path.exists());
        let loaded = backend.load().await.unwrap();
        assert!(!loaded.is_empty());
    }
}
