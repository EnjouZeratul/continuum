//! Stale-read prevention: tracks last-read file hashes.
//!
//! Records SHA-256 of file content when `ReadFileTool` reads. `EditFileTool` /
//! `WriteFileTool` (when overwriting) verify that the caller's last-read hash
//! matches the current on-disk hash; mismatches produce an error.
//!
//! # v1.1.0 minimal scope
//!
//! This is a process-wide store keyed by canonical path (no session_id yet).
//! v1.1.1+ will thread `ExecutionContext` through `BuiltinTool::execute` so
//! the store can be session-scoped. For now, all read/write/edit calls in the
//! process share a single store, which is correct for single-session agent
//! loops (the common case).

use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tokio::fs;

/// Entry recording the last-read state of a path.
#[derive(Debug, Clone)]
pub struct ReadStateEntry {
    pub content_sha256: [u8; 32],
    pub size: u64,
}

/// Process-wide read-state tracker.
#[derive(Default)]
pub struct ReadStateStore {
    inner: RwLock<HashMap<PathBuf, ReadStateEntry>>,
}

impl ReadStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wrap in `Arc` for sharing across tool instances.
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Record a read: store SHA-256 of the file's current contents.
    pub async fn record_read(&self, canonical_path: PathBuf) -> std::io::Result<()> {
        let bytes = fs::read(&canonical_path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = hasher.finalize().into();
        let mut inner = self.inner.write();
        inner.insert(
            canonical_path,
            ReadStateEntry {
                content_sha256: hash,
                size: bytes.len() as u64,
            },
        );
        Ok(())
    }

    /// Verify the file's current contents match the last-recorded read.
    ///
    /// Returns:
    /// - `Ok(())` — file matches last read (or no prior read recorded, which
    ///   means caller did not `read_file` first — see `require_prior_read`).
    /// - `Err(StaleReadError::NotRead)` — no prior read recorded.
    /// - `Err(StaleReadError::Modified)` — file changed since last read.
    pub async fn verify(
        &self,
        canonical_path: &std::path::Path,
        require_prior_read: bool,
    ) -> Result<(), StaleReadError> {
        let bytes = fs::read(canonical_path).await.map_err(StaleReadError::Io)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let current: [u8; 32] = hasher.finalize().into();

        let inner = self.inner.read();
        match inner.get(canonical_path) {
            None => {
                if require_prior_read {
                    Err(StaleReadError::NotRead)
                } else {
                    Ok(())
                }
            }
            Some(entry) => {
                if entry.content_sha256 == current {
                    Ok(())
                } else {
                    Err(StaleReadError::Modified)
                }
            }
        }
    }

    /// Remove an entry (e.g., after file deleted).
    pub fn forget(&self, canonical_path: &std::path::Path) {
        self.inner.write().remove(canonical_path);
    }
}

/// Stale-read verification error.
#[derive(Debug, thiserror::Error)]
pub enum StaleReadError {
    #[error("file not previously read in this session — call read_file first")]
    NotRead,
    #[error("file modified on disk since last read — call read_file again to refresh")]
    Modified,
    #[error("I/O error during stale-read check: {0}")]
    Io(#[from] std::io::Error),
}

/// Process-wide singleton (lazy).
static GLOBAL_STORE: LazyLock<Arc<ReadStateStore>> =
    LazyLock::new(|| ReadStateStore::new().into_arc());

/// Get the process-wide `ReadStateStore`. Tools use this when callers don't
/// inject their own (v1.1.1+ will allow per-session stores via `ExecutionContext`).
pub fn global_store() -> Arc<ReadStateStore> {
    GLOBAL_STORE.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_record_and_verify_unchanged() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("a.txt");
        fs::write(&path, "hello").await.unwrap();
        let canonical = fs::canonicalize(&path).await.unwrap();

        let store = ReadStateStore::new();
        store.record_read(canonical.clone()).await.unwrap();

        let result = store.verify(&canonical, true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_detects_modification() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("b.txt");
        fs::write(&path, "v1").await.unwrap();
        let canonical = fs::canonicalize(&path).await.unwrap();

        let store = ReadStateStore::new();
        store.record_read(canonical.clone()).await.unwrap();

        // External modification
        fs::write(&path, "v2-modified").await.unwrap();

        let result = store.verify(&canonical, true).await;
        assert!(matches!(result, Err(StaleReadError::Modified)));
    }

    #[tokio::test]
    async fn test_verify_without_prior_read() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("c.txt");
        fs::write(&path, "content").await.unwrap();
        let canonical = fs::canonicalize(&path).await.unwrap();

        let store = ReadStateStore::new();
        // require_prior_read=false → no entry is fine
        let result = store.verify(&canonical, false).await;
        assert!(result.is_ok());

        // require_prior_read=true → no entry is error
        let result = store.verify(&canonical, true).await;
        assert!(matches!(result, Err(StaleReadError::NotRead)));
    }

    #[tokio::test]
    async fn test_global_store_singleton() {
        let s1 = global_store();
        let s2 = global_store();
        assert!(Arc::ptr_eq(&s1, &s2));
    }
}
