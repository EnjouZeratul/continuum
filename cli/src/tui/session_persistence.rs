//! 会话持久化模块

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 会话数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// 会话 ID
    pub session_id: String,
    /// 创建时间
    pub created_at: String,
    /// 最后更新时间
    pub updated_at: String,
    /// 消息列表
    pub messages: Vec<MessageData>,
    /// 会话名称（可选）
    pub name: Option<String>,
}

/// 消息数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageData {
    /// 角色
    pub role: String,
    /// 内容
    pub content: String,
}

/// 会话管理器
pub struct SessionManager {
    /// 会话存储目录
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new() -> Result<Self> {
        let sessions_dir = Self::get_sessions_dir()?;
        fs::create_dir_all(&sessions_dir)?;
        Ok(Self { sessions_dir })
    }

    /// 获取会话存储目录
    fn get_sessions_dir() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "windows") {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        };
        Ok(config_dir.join("continuum").join("sessions"))
    }

    /// 保存会话
    pub fn save_session(
        &self,
        session_id: &str,
        messages: &[(String, String)],
        name: Option<&str>,
    ) -> Result<PathBuf> {
        let now = chrono::Local::now().to_rfc3339();

        let session_data = SessionData {
            session_id: session_id.to_string(),
            created_at: now.clone(),
            updated_at: now,
            messages: messages
                .iter()
                .map(|(role, content)| MessageData {
                    role: role.clone(),
                    content: content.clone(),
                })
                .collect(),
            name: name.map(|s| s.to_string()),
        };

        let filename = if let Some(n) = name {
            format!("{}.json", Self::sanitize_name(n))
        } else {
            format!("{}.json", session_id)
        };

        let file_path = self.sessions_dir.join(filename);
        let json = serde_json::to_string_pretty(&session_data)?;
        fs::write(&file_path, json)?;

        Ok(file_path)
    }

    /// 加载会话
    pub fn load_session(&self, session_id: &str) -> Result<SessionData> {
        let file_path = self.sessions_dir.join(format!("{}.json", session_id));
        let content = fs::read_to_string(&file_path)?;
        let session: SessionData = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// 按名称加载会话
    pub fn load_session_by_name(&self, name: &str) -> Result<SessionData> {
        let file_path = self
            .sessions_dir
            .join(format!("{}.json", Self::sanitize_name(name)));
        let content = fs::read_to_string(&file_path)?;
        let session: SessionData = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// 列出所有会话
    pub fn list_sessions(&self) -> Result<Vec<SessionData>> {
        let mut sessions = Vec::new();

        if !self.sessions_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<SessionData>(&content) {
                        sessions.push(session);
                    }
                }
            }
        }

        // 按更新时间排序（最新的在前）
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(sessions)
    }

    /// 删除会话
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let file_path = self.sessions_dir.join(format!("{}.json", session_id));
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }
        Ok(())
    }

    /// 清理名称（移除不安全字符）
    fn sanitize_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// 获取会话存储路径
    pub fn sessions_dir(&self) -> &PathBuf {
        &self.sessions_dir
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            sessions_dir: PathBuf::from("."),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_data_creation() {
        let session = SessionData {
            session_id: "test-123".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            messages: vec![MessageData {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            name: Some("Test Session".to_string()),
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("test-123"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(SessionManager::sanitize_name("test session"), "testsession");
        assert_eq!(
            SessionManager::sanitize_name("test-session"),
            "test-session"
        );
        assert_eq!(SessionManager::sanitize_name("test/session"), "testsession");
        assert_eq!(SessionManager::sanitize_name("  test  "), "test");
    }

    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new();
        assert!(manager.is_ok());
        let m = manager.unwrap();
        assert!(m.sessions_dir.to_string_lossy().contains("continuum"));
    }

    #[test]
    fn test_default_session_manager() {
        let manager = SessionManager::default();
        assert!(!manager.sessions_dir.to_string_lossy().is_empty());
    }

    #[test]
    fn test_message_data_serialize() {
        let msg = MessageData {
            role: "assistant".to_string(),
            content: "Response".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assistant"));
        assert!(json.contains("Response"));
    }
}
