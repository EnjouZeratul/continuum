//! 存储引擎模块
//!
//! 统一的存储抽象，支持文件、对象存储等。
//!
//! **功能状态：**
//! - `[STABLE]` FileSystem 存储 - 已完整实现
//! - `[STABLE]` Memory 存储 - 内存 HashMap 存储
//! - `[EXPERIMENTAL]` S3 存储 - 尚未实现

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// 存储类型
    pub storage_type: StorageType,
    /// 基础路径
    pub base_path: String,
    /// 最大文件大小（字节）
    pub max_file_size: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageType::FileSystem,
            base_path: "./data".to_string(),
            max_file_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// 存储类型
///
/// - `FileSystem` - [STABLE] 本地文件系统存储
/// - `Memory` - [STABLE] 内存 HashMap 存储
/// - `S3` - [EXPERIMENTAL] S3 对象存储，尚未实现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageType {
    FileSystem,
    Memory,
    S3,
}

/// 存储引擎
pub struct StorageEngine {
    config: StorageConfig,
    memory: RwLock<HashMap<String, Vec<u8>>>,
}

impl StorageEngine {
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            memory: RwLock::new(HashMap::new()),
        }
    }

    /// 读取数据
    pub async fn read(&self, key: &str) -> Result<Vec<u8>> {
        match self.config.storage_type {
            StorageType::FileSystem => {
                let path = PathBuf::from(&self.config.base_path).join(key);
                let data = tokio::fs::read(&path).await?;
                Ok(data)
            }
            StorageType::Memory => self
                .memory
                .read()
                .get(key)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Memory storage key not found: {}", key)),
            StorageType::S3 => Err(Self::s3_unimplemented("read", key)),
        }
    }

    /// 写入数据
    pub async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        match self.config.storage_type {
            StorageType::FileSystem => {
                let path = PathBuf::from(&self.config.base_path).join(key);

                // 确保目录存在
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                tokio::fs::write(&path, data).await?;
                Ok(())
            }
            StorageType::Memory => {
                self.memory.write().insert(key.to_string(), data.to_vec());
                Ok(())
            }
            StorageType::S3 => Err(Self::s3_unimplemented("write", key)),
        }
    }

    /// 删除数据
    pub async fn delete(&self, key: &str) -> Result<()> {
        match self.config.storage_type {
            StorageType::FileSystem => {
                let path = PathBuf::from(&self.config.base_path).join(key);
                tokio::fs::remove_file(&path).await?;
                Ok(())
            }
            StorageType::Memory => {
                self.memory.write().remove(key);
                Ok(())
            }
            StorageType::S3 => Err(Self::s3_unimplemented("delete", key)),
        }
    }

    /// 检查是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self.config.storage_type {
            StorageType::FileSystem => {
                let path = PathBuf::from(&self.config.base_path).join(key);
                Ok(path.exists())
            }
            StorageType::Memory => Ok(self.memory.read().contains_key(key)),
            StorageType::S3 => Err(Self::s3_unimplemented("exists", key)),
        }
    }

    /// 列出所有键
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        match self.config.storage_type {
            StorageType::FileSystem => {
                let path = PathBuf::from(&self.config.base_path).join(prefix);
                let mut entries = Vec::new();

                if path.is_dir() {
                    let mut dir = tokio::fs::read_dir(&path).await?;
                    while let Some(entry) = dir.next_entry().await? {
                        if let Some(name) = entry.file_name().to_str() {
                            entries.push(name.to_string());
                        }
                    }
                }

                Ok(entries)
            }
            StorageType::Memory => Ok(self
                .memory
                .read()
                .keys()
                .filter(|key| key.starts_with(prefix))
                .cloned()
                .collect()),
            StorageType::S3 => Err(Self::s3_unimplemented("list", prefix)),
        }
    }

    fn s3_unimplemented(operation: &str, key: &str) -> anyhow::Error {
        anyhow::anyhow!(
            "[experimental] S3 storage operation '{}' is not yet implemented (key: {})",
            operation,
            key
        )
    }
}

impl Default for StorageEngine {
    fn default() -> Self {
        Self::new(StorageConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert!(matches!(config.storage_type, StorageType::FileSystem));
        assert_eq!(config.base_path, "./data");
    }

    #[test]
    fn test_storage_type_filesystem() {
        let config = StorageConfig {
            storage_type: StorageType::FileSystem,
            ..Default::default()
        };
        assert!(matches!(config.storage_type, StorageType::FileSystem));
    }

    #[tokio::test]
    async fn test_filesystem_write_and_read() {
        let dir = TempDir::new().unwrap();
        let config = StorageConfig {
            storage_type: StorageType::FileSystem,
            base_path: dir.path().to_str().unwrap().to_string(),
            max_file_size: 1024 * 1024,
        };
        let engine = StorageEngine::new(config);

        engine.write("test.txt", b"hello world").await.unwrap();
        let data = engine.read("test.txt").await.unwrap();
        assert_eq!(data, b"hello world");
    }

    #[tokio::test]
    async fn test_filesystem_exists() {
        let dir = TempDir::new().unwrap();
        let config = StorageConfig {
            storage_type: StorageType::FileSystem,
            base_path: dir.path().to_str().unwrap().to_string(),
            max_file_size: 1024 * 1024,
        };
        let engine = StorageEngine::new(config);

        assert!(!engine.exists("test.txt").await.unwrap());
        engine.write("test.txt", b"hello").await.unwrap();
        assert!(engine.exists("test.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_filesystem_delete() {
        let dir = TempDir::new().unwrap();
        let config = StorageConfig {
            storage_type: StorageType::FileSystem,
            base_path: dir.path().to_str().unwrap().to_string(),
            max_file_size: 1024 * 1024,
        };
        let engine = StorageEngine::new(config);

        engine.write("test.txt", b"hello").await.unwrap();
        engine.delete("test.txt").await.unwrap();
        assert!(!engine.exists("test.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_storage_write_read_list_delete() {
        let config = StorageConfig {
            storage_type: StorageType::Memory,
            ..Default::default()
        };
        let engine = StorageEngine::new(config);

        engine.write("test/a.txt", b"hello").await.unwrap();
        engine.write("other.txt", b"world").await.unwrap();

        assert_eq!(engine.read("test/a.txt").await.unwrap(), b"hello");
        assert!(engine.exists("test/a.txt").await.unwrap());
        assert_eq!(
            engine.list("test/").await.unwrap(),
            vec!["test/a.txt".to_string()]
        );

        engine.delete("test/a.txt").await.unwrap();
        assert!(!engine.exists("test/a.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_s3_storage_not_implemented() {
        let config = StorageConfig {
            storage_type: StorageType::S3,
            ..Default::default()
        };
        let engine = StorageEngine::new(config);

        let result = engine.write("test", b"data").await;
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("[experimental]"));
        assert!(error.contains("S3 storage operation 'write'"));
    }
}
