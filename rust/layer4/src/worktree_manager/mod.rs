//! # Worktree Manager
//!
//! Git Worktree 管理系统，提供分支隔离环境。

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sh_layer3::generate_short_id;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::types::Layer4Result;

/// Worktree 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorktreeStatus {
    Active,
    Idle,
    Error,
    Locked,
}

impl std::fmt::Display for WorktreeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Idle => write!(f, "idle"),
            Self::Error => write!(f, "error"),
            Self::Locked => write!(f, "locked"),
        }
    }
}

/// Worktree 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeConfig {
    pub name: String,
    pub branch: String,
    pub base_branch: Option<String>,
    pub create_branch: bool,
}

impl WorktreeConfig {
    pub fn new(name: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            branch: branch.into(),
            base_branch: None,
            create_branch: true,
        }
    }

    pub fn with_base_branch(mut self, base: impl Into<String>) -> Self {
        self.base_branch = Some(base.into());
        self
    }

    pub fn create_branch(mut self, create: bool) -> Self {
        self.create_branch = create;
        self
    }
}

/// Worktree 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub status: WorktreeStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: HashMap<String, String>,
}

impl Worktree {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        path: PathBuf,
        branch: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path,
            branch: branch.into(),
            status: WorktreeStatus::Active,
            created_at: chrono::Utc::now(),
            last_used: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn touch(&mut self) {
        self.last_used = Some(chrono::Utc::now());
    }
}

/// Worktree 管理器
pub struct WorktreeManager {
    root_path: PathBuf,
    worktrees_path: PathBuf,
    worktrees: RwLock<HashMap<String, Worktree>>,
}

impl WorktreeManager {
    /// 创建新的 Worktree 管理器
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        let root = root_path.into();
        let worktrees_path = root.join(".claude").join("worktrees");

        Self {
            root_path: root,
            worktrees_path,
            worktrees: RwLock::new(HashMap::new()),
        }
    }

    /// 确保 worktrees 目录存在
    fn ensure_worktrees_dir(&self) -> Layer4Result<()> {
        std::fs::create_dir_all(&self.worktrees_path)?;
        Ok(())
    }

    /// 创建 Worktree
    pub async fn create(&self, config: &WorktreeConfig) -> Layer4Result<Worktree> {
        self.ensure_worktrees_dir()?;

        let id = generate_short_id();
        let worktree_path = self.worktrees_path.join(&config.name);

        // 使用 git worktree add 命令
        let branch_arg = if config.create_branch {
            format!("-b {}", config.branch)
        } else {
            config.branch.clone()
        };

        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                &worktree_path.to_string_lossy(),
                &branch_arg,
            ])
            .current_dir(&self.root_path)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let worktree =
                    Worktree::new(&id, &config.name, worktree_path.clone(), &config.branch);
                self.worktrees.write().insert(id.clone(), worktree.clone());
                tracing::info!("Created worktree: {} at {:?}", config.name, worktree_path);
                Ok(worktree)
            }
            Ok(o) => {
                let error = String::from_utf8_lossy(&o.stderr);
                Err(anyhow::anyhow!("Git worktree add failed: {}", error))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to execute git: {}", e)),
        }
    }

    /// 列出所有 Worktree
    pub async fn list(&self) -> Layer4Result<Vec<Worktree>> {
        Ok(self.worktrees.read().values().cloned().collect())
    }

    /// 获取 Worktree
    pub async fn get(&self, id: &str) -> Layer4Result<Option<Worktree>> {
        Ok(self.worktrees.read().get(id).cloned())
    }

    /// 按名称获取 Worktree
    pub async fn get_by_name(&self, name: &str) -> Layer4Result<Option<Worktree>> {
        Ok(self
            .worktrees
            .read()
            .values()
            .find(|w| w.name == name)
            .cloned())
    }

    /// 删除 Worktree
    pub async fn remove(&self, id: &str) -> Layer4Result<()> {
        let worktree = self.worktrees.read().get(id).cloned();

        if let Some(wt) = worktree {
            // 使用 git worktree remove 命令
            let output = Command::new("git")
                .args(["worktree", "remove", "--force", &wt.path.to_string_lossy()])
                .current_dir(&self.root_path)
                .output();

            match output {
                Ok(o) if o.status.success() => {
                    self.worktrees.write().remove(id);
                    tracing::info!("Removed worktree: {}", wt.name);
                    Ok(())
                }
                Ok(o) => {
                    let error = String::from_utf8_lossy(&o.stderr);
                    Err(anyhow::anyhow!("Git worktree remove failed: {}", error))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to execute git: {}", e)),
            }
        } else {
            Err(anyhow::anyhow!("Worktree not found: {}", id))
        }
    }

    /// 清理无效的 Worktree
    pub async fn prune(&self) -> Layer4Result<Vec<String>> {
        let mut removed = Vec::new();

        // 使用 git worktree prune 命令
        let output = Command::new("git")
            .args(["worktree", "prune", "-v"])
            .current_dir(&self.root_path)
            .output();

        if let Ok(o) = output {
            if o.status.success() {
                let stdout = String::from_utf8_lossy(&o.stdout);
                for line in stdout.lines() {
                    if line.contains("Removing") {
                        removed.push(line.to_string());
                    }
                }
            }
        }

        Ok(removed)
    }

    /// 同步 Worktree 状态
    pub async fn sync(&self) -> Layer4Result<()> {
        // 更新所有 worktree 的状态
        let worktrees = self.worktrees.read().keys().cloned().collect::<Vec<_>>();

        for id in worktrees {
            if let Some(wt) = self.worktrees.read().get(&id) {
                let path_exists = wt.path.exists();

                // 更新状态
                if let Some(w) = self.worktrees.write().get_mut(&id) {
                    w.status = if path_exists {
                        WorktreeStatus::Active
                    } else {
                        WorktreeStatus::Error
                    };
                }
            }
        }

        Ok(())
    }

    /// Worktree 数量
    pub fn count(&self) -> usize {
        self.worktrees.read().len()
    }

    /// 获取根路径
    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }

    /// 获取 worktrees 路径
    pub fn worktrees_path(&self) -> &PathBuf {
        &self.worktrees_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_config() {
        let config = WorktreeConfig::new("feature-1", "feature/test");
        assert_eq!(config.name, "feature-1");
        assert_eq!(config.branch, "feature/test");
        assert!(config.create_branch);
    }

    #[test]
    fn test_worktree_creation() {
        let wt = Worktree::new("abc123", "test", PathBuf::from("/tmp/test"), "main");
        assert_eq!(wt.id, "abc123");
        assert_eq!(wt.name, "test");
        assert_eq!(wt.status, WorktreeStatus::Active);
    }

    #[test]
    fn test_worktree_manager_creation() {
        let manager = WorktreeManager::new("/tmp/test");
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_worktree_status_display() {
        assert_eq!(format!("{}", WorktreeStatus::Active), "active");
        assert_eq!(format!("{}", WorktreeStatus::Error), "error");
    }
}
