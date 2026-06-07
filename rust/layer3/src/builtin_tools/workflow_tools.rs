//! # Workflow Tools
//!
//! 工作流控制工具集，提供检查点保存和恢复功能。
//!
//! 使用 Layer2 CheckpointSystem 实现持久化。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;

// Layer2 CheckpointSystem integration
use sh_layer2::{CheckpointData, CheckpointId, CheckpointSystemTrait, CheckpointWriter, SessionId};

/// 默认检查点存储路径
fn default_checkpoint_path() -> PathBuf {
    std::env::temp_dir().join("continuum_checkpoints")
}

/// Create Checkpoint Tool
///
/// 保存当前会话状态到检查点文件。
/// 使用 Layer2 CheckpointWriter 实现原子写入和完整性验证。
pub struct CreateCheckpointTool {
    writer: Arc<CheckpointWriter>,
}

impl CreateCheckpointTool {
    /// 创建新工具，使用默认存储路径
    pub fn new() -> Self {
        Self {
            writer: Arc::new(CheckpointWriter::new(default_checkpoint_path())),
        }
    }

    /// 使用自定义存储路径
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            writer: Arc::new(CheckpointWriter::new(path)),
        }
    }
}

impl Default for CreateCheckpointTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for CreateCheckpointTool {
    fn name(&self) -> &str {
        "create_checkpoint"
    }

    fn description(&self) -> &str {
        "Create a checkpoint to save current agent state to a file."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session ID to checkpoint"
                },
                "trigger": {
                    "type": "string",
                    "description": "Optional: trigger reason for the checkpoint (default: 'manual')"
                },
                "messages": {
                    "type": "array",
                    "description": "Optional: message history to save",
                    "items": {
                        "type": "object",
                        "properties": {
                            "role": { "type": "string" },
                            "content": { "type": "string" }
                        }
                    }
                },
                "iteration": {
                    "type": "integer",
                    "description": "Optional: current iteration number (default: 0)"
                },
                "tokens_used": {
                    "type": "integer",
                    "description": "Optional: tokens used so far (default: 0)"
                }
            },
            "required": ["session_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let session_id_str = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing session_id parameter"))?;

        let session_id = SessionId::from(session_id_str);
        let trigger = args["trigger"].as_str().unwrap_or("manual");
        let iteration = args["iteration"].as_i64().unwrap_or(0) as i32;
        let tokens_used = args["tokens_used"].as_i64().unwrap_or(0);

        let messages = args["messages"].as_array().cloned().unwrap_or_default();

        let tool_calls_pending = args["tool_calls_pending"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let tool_results = args
            .get("tool_results")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        // 构建 CheckpointData (Layer2 格式)
        let checkpoint_data = CheckpointData {
            checkpoint_id: CheckpointId::new(),
            session_id: session_id.clone(),
            created_at: Utc::now(),
            trigger: trigger.to_string(),
            iteration,
            messages,
            tool_calls_pending,
            tool_results,
            tokens_used,
            cost_estimate: 0.0,
            resume_hint: None,
        };

        // 使用 Layer2 CheckpointWriter 保存
        let checkpoint_id = self.writer.save(&checkpoint_data).await?;

        Ok(format!(
            "Checkpoint created: {}\nSession: {}\nTrigger: {}\nIteration: {}",
            checkpoint_id, session_id, trigger, iteration
        ))
    }
}

/// Restore Checkpoint Tool
///
/// 从检查点文件恢复会话状态。
/// 使用 Layer2 CheckpointWriter 实现读取和验证。
pub struct RestoreCheckpointTool {
    writer: Arc<CheckpointWriter>,
}

impl RestoreCheckpointTool {
    /// 创建新工具，使用默认存储路径
    pub fn new() -> Self {
        Self {
            writer: Arc::new(CheckpointWriter::new(default_checkpoint_path())),
        }
    }

    /// 使用自定义存储路径
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            writer: Arc::new(CheckpointWriter::new(path)),
        }
    }
}

impl Default for RestoreCheckpointTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for RestoreCheckpointTool {
    fn name(&self) -> &str {
        "restore_checkpoint"
    }

    fn description(&self) -> &str {
        "Restore agent state from a checkpoint file."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session ID to restore"
                },
                "checkpoint_id": {
                    "type": "string",
                    "description": "Optional: specific checkpoint ID (default: latest)"
                }
            },
            "required": ["session_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let session_id_str = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing session_id parameter"))?;

        let session_id = SessionId::from(session_id_str);
        let checkpoint_id_opt = args["checkpoint_id"]
            .as_str()
            .map(|s| CheckpointId(s.to_string()));

        // 使用 Layer2 CheckpointWriter 加载
        let result = self
            .writer
            .load(&session_id, checkpoint_id_opt.as_ref())
            .await?;

        match result {
            Some(checkpoint) => {
                Ok(format!(
                    "Checkpoint restored: {}\nSession: {}\nTrigger: {}\nIteration: {}\nMessages: {}\nTokens used: {}",
                    checkpoint.checkpoint_id,
                    checkpoint.session_id,
                    checkpoint.trigger,
                    checkpoint.iteration,
                    checkpoint.messages.len(),
                    checkpoint.tokens_used
                ))
            }
            None => Err(anyhow::anyhow!(
                "No checkpoints found for session: {}",
                session_id_str
            )),
        }
    }
}

/// List Checkpoints Tool
///
/// 列出会话的所有检查点。
/// 使用 Layer2 CheckpointWriter 实现。
pub struct ListCheckpointsTool {
    writer: Arc<CheckpointWriter>,
}

impl ListCheckpointsTool {
    pub fn new() -> Self {
        Self {
            writer: Arc::new(CheckpointWriter::new(default_checkpoint_path())),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            writer: Arc::new(CheckpointWriter::new(path)),
        }
    }
}

impl Default for ListCheckpointsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for ListCheckpointsTool {
    fn name(&self) -> &str {
        "list_checkpoints"
    }

    fn description(&self) -> &str {
        "List all checkpoints for a session."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session ID to list checkpoints for"
                }
            },
            "required": ["session_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let session_id_str = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing session_id parameter"))?;

        let session_id = SessionId::from(session_id_str);

        // 使用 Layer2 CheckpointWriter 列出检查点
        let checkpoints = self.writer.list(&session_id).await?;

        if checkpoints.is_empty() {
            return Ok(format!(
                "No checkpoints found for session: {}",
                session_id_str
            ));
        }

        let mut result = format!("Checkpoints for session {}:\n", session_id_str);
        for (i, meta) in checkpoints.iter().enumerate() {
            result.push_str(&format!(
                "  {}. {} (created: {})\n",
                i + 1,
                meta.checkpoint_id,
                meta.created_at.format("%Y-%m-%d %H:%M:%S")
            ));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_checkpoint_tool_category() {
        let tool = CreateCheckpointTool::new();
        assert_eq!(tool.category(), ToolCategory::Workflow);
    }

    #[test]
    fn test_restore_checkpoint_tool_category() {
        let tool = RestoreCheckpointTool::new();
        assert_eq!(tool.category(), ToolCategory::Workflow);
    }

    #[tokio::test]
    async fn test_create_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let tool = CreateCheckpointTool::with_path(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "session_id": "test_session",
                "trigger": "manual",
                "messages": [{"role": "user", "content": "hello"}],
                "iteration": 1
            }))
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Checkpoint created"));
        assert!(output.contains("test_session"));
    }

    #[tokio::test]
    async fn test_restore_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let create_tool = CreateCheckpointTool::with_path(temp_dir.path().to_path_buf());

        // 先创建检查点
        create_tool
            .execute(json!({
                "session_id": "test_session",
                "messages": [{"role": "user", "content": "test"}]
            }))
            .await
            .unwrap();

        // 然后恢复
        let restore_tool = RestoreCheckpointTool::with_path(temp_dir.path().to_path_buf());
        let result = restore_tool
            .execute(json!({"session_id": "test_session"}))
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Checkpoint restored"));
    }

    #[tokio::test]
    async fn test_restore_nonexistent_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let tool = RestoreCheckpointTool::with_path(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({"session_id": "nonexistent_session"}))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No checkpoints found"));
    }

    #[tokio::test]
    async fn test_list_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let create_tool = CreateCheckpointTool::with_path(temp_dir.path().to_path_buf());

        // 创建多个检查点
        create_tool
            .execute(json!({"session_id": "test_session"}))
            .await
            .unwrap();

        let list_tool = ListCheckpointsTool::with_path(temp_dir.path().to_path_buf());
        let result = list_tool
            .execute(json!({"session_id": "test_session"}))
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Checkpoints for session"));
    }
}
