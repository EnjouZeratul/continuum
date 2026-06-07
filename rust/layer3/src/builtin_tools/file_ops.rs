//! # File Operations Tools
//!
//! 文件操作工具集：读写、编辑、创建、删除等。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;

/// Read File Tool
pub struct ReadFileTool;

#[async_trait]
impl BuiltinTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file from the filesystem."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Optional: line number to start reading from"
                },
                "limit": {
                    "type": "integer",
                    "description": "Optional: number of lines to read"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;

        let content = tokio::fs::read_to_string(path).await?;

        // 处理分页参数
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        if offset == 0 && limit.is_none() {
            // 无分页，返回完整内容
            return Ok(content);
        }

        // 按行分页
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // 验证 offset
        if offset > total_lines {
            return Err(anyhow::anyhow!(
                "Offset {} exceeds total lines {}",
                offset,
                total_lines
            ));
        }

        // 计算分页范围
        let end = limit.map_or(total_lines, |l| (offset + l).min(total_lines));
        let page_lines = &lines[offset..end];

        // 构建返回内容，包含分页元信息
        let result = if page_lines.is_empty() {
            format!(
                "[Lines {}-{} of {} total lines]\n(No content in this range)",
                offset, end, total_lines
            )
        } else {
            format!(
                "[Lines {}-{} of {} total lines]\n{}",
                offset,
                end,
                total_lines,
                page_lines.join("\n")
            )
        };

        Ok(result)
    }
}

/// Write File Tool
pub struct WriteFileTool;

#[async_trait]
impl BuiltinTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, creating it if it doesn't exist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing content parameter"))?;
        tokio::fs::write(path, content).await?;
        Ok(format!("Successfully wrote to {}", path))
    }
}

/// Edit File Tool (search and replace)
pub struct EditFileTool;

#[async_trait]
impl BuiltinTool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing specific text with new text."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to search for and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace with"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let old_string = args["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing old_string parameter"))?;
        let new_string = args["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing new_string parameter"))?;

        let content = tokio::fs::read_to_string(path).await?;
        if !content.contains(old_string) {
            return Err(anyhow::anyhow!("old_string not found in file"));
        }

        let new_content = content.replace(old_string, new_string);
        tokio::fs::write(path, new_content).await?;
        Ok(format!("Successfully edited {}", path))
    }
}

/// List Directory Tool
pub struct ListDirectoryTool;

#[async_trait]
impl BuiltinTool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files and directories in a given path."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let mut entries = tokio::fs::read_dir(path).await?;
        let mut result = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = if entry.file_type().await?.is_dir() {
                "dir"
            } else {
                "file"
            };
            result.push(format!("{} [{}]", name, file_type));
        }

        Ok(result.join("\n"))
    }
}

/// Move File Tool
///
/// 移动或重命名文件/目录。
pub struct MoveFileTool;

#[async_trait]
impl BuiltinTool for MoveFileTool {
    fn name(&self) -> &str {
        "move_file"
    }

    fn description(&self) -> &str {
        "Move or rename a file or directory."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The source path to move from"
                },
                "destination": {
                    "type": "string",
                    "description": "The destination path to move to"
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Optional: overwrite destination if exists (default: false)"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing source parameter"))?;
        let destination = args["destination"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing destination parameter"))?;
        let overwrite = args["overwrite"].as_bool().unwrap_or(false);

        // 验证源文件/目录存在
        let source_meta = tokio::fs::metadata(source)
            .await
            .map_err(|e| anyhow::anyhow!("Source path not found: {} ({})", source, e))?;

        // 检查目标是否存在
        let dest_exists = tokio::fs::try_exists(destination).await.unwrap_or(false);

        if dest_exists && !overwrite {
            return Err(anyhow::anyhow!(
                "Destination already exists: {}. Use overwrite=true to replace.",
                destination
            ));
        }

        // 如果目标存在且要覆盖，先删除目标
        if dest_exists && overwrite {
            if tokio::fs::metadata(destination).await?.is_dir() {
                tokio::fs::remove_dir_all(destination).await?;
            } else {
                tokio::fs::remove_file(destination).await?;
            }
        }

        // 确保目标父目录存在
        if let Some(parent) = std::path::Path::new(destination).parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // 执行移动操作
        tokio::fs::rename(source, destination).await.map_err(|e| {
            // 如果跨文件系统移动失败，提示用户
            if e.raw_os_error() == Some(18) {
                // EXDEV: cross-device link
                anyhow::anyhow!(
                    "Cross-device move not supported directly. Use copy + delete instead."
                )
            } else {
                anyhow::anyhow!("Failed to move file: {}", e)
            }
        })?;

        let file_type = if source_meta.is_dir() {
            "directory"
        } else {
            "file"
        };
        Ok(format!(
            "Successfully moved {} from {} to {}",
            file_type, source, destination
        ))
    }
}

/// Copy File Tool
///
/// 复制文件或目录。
pub struct CopyFileTool;

#[async_trait]
impl BuiltinTool for CopyFileTool {
    fn name(&self) -> &str {
        "copy_file"
    }

    fn description(&self) -> &str {
        "Copy a file or directory to a new location."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The source path to copy from"
                },
                "destination": {
                    "type": "string",
                    "description": "The destination path to copy to"
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Optional: overwrite destination if exists (default: false)"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing source parameter"))?;
        let destination = args["destination"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing destination parameter"))?;
        let overwrite = args["overwrite"].as_bool().unwrap_or(false);

        // 验证源文件/目录存在
        let source_meta = tokio::fs::metadata(source)
            .await
            .map_err(|e| anyhow::anyhow!("Source path not found: {} ({})", source, e))?;

        // 检查目标是否存在
        let dest_exists = tokio::fs::try_exists(destination).await.unwrap_or(false);

        if dest_exists && !overwrite {
            return Err(anyhow::anyhow!(
                "Destination already exists: {}. Use overwrite=true to replace.",
                destination
            ));
        }

        // 确保目标父目录存在
        if let Some(parent) = std::path::Path::new(destination).parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // 执行复制操作
        if source_meta.is_dir() {
            // 目录复制：使用 copy_dir_all
            copy_dir_all(source.to_string(), destination.to_string()).await?;
            Ok(format!(
                "Successfully copied directory from {} to {}",
                source, destination
            ))
        } else {
            // 文件复制
            tokio::fs::copy(source, destination)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to copy file: {}", e))?;
            Ok(format!(
                "Successfully copied file from {} to {}",
                source, destination
            ))
        }
    }
}

/// 递归复制目录
///
/// 使用 Box::pin 处理递归异步函数
fn copy_dir_all(
    source: String,
    destination: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Layer3Result<()>> + Send>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(&destination).await?;

        let mut entries = tokio::fs::read_dir(&source).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ty = entry.file_type().await?;
            let src_path = entry.path();
            let dest_path = std::path::Path::new(&destination).join(entry.file_name());

            if ty.is_dir() {
                copy_dir_all(
                    src_path.to_string_lossy().to_string(),
                    dest_path.to_string_lossy().to_string(),
                )
                .await?;
            } else {
                tokio::fs::copy(&src_path, &dest_path).await?;
            }
        }

        Ok(())
    })
}

/// Create Directory Tool
///
/// 创建目录（包括父目录）。
pub struct CreateDirectoryTool;

#[async_trait]
impl BuiltinTool for CreateDirectoryTool {
    fn name(&self) -> &str {
        "create_directory"
    }

    fn description(&self) -> &str {
        "Create a directory and all parent directories if they don't exist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to create"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;

        tokio::fs::create_dir_all(path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))?;

        Ok(format!("Successfully created directory: {}", path))
    }
}

/// Delete File Tool
///
/// 删除文件或目录。
pub struct DeleteFileTool;

#[async_trait]
impl BuiltinTool for DeleteFileTool {
    fn name(&self) -> &str {
        "delete_file"
    }

    fn description(&self) -> &str {
        "Delete a file or directory from the filesystem."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file or directory to delete"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Optional: for directories, delete recursively (default: false)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileOps
    }

    fn is_dangerous(&self) -> bool {
        true
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;
        let recursive = args["recursive"].as_bool().unwrap_or(false);

        // 验证路径存在
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| anyhow::anyhow!("Path not found: {} ({})", path, e))?;

        if meta.is_dir() {
            // 目录删除
            if recursive {
                tokio::fs::remove_dir_all(path)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to delete directory: {}", e))?;
                Ok(format!("Successfully deleted directory: {}", path))
            } else {
                // 非递归删除，要求目录为空
                tokio::fs::remove_dir(path).await
                    .map_err(|e| anyhow::anyhow!(
                        "Failed to delete directory (may not be empty): {}. Use recursive=true for non-empty directories.",
                        e
                    ))?;
                Ok(format!("Successfully deleted directory: {}", path))
            }
        } else {
            // 文件删除
            tokio::fs::remove_file(path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to delete file: {}", e))?;
            Ok(format!("Successfully deleted file: {}", path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_tool_meta() {
        let tool = ReadFileTool;
        assert_eq!(tool.name(), "read_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
    }

    #[test]
    fn test_write_file_tool_dangerous() {
        let tool = WriteFileTool;
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[test]
    fn test_move_file_tool_meta() {
        let tool = MoveFileTool;
        assert_eq!(tool.name(), "move_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_move_file_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // 创建源文件
        tokio::fs::write(&source, "test content").await.unwrap();

        let tool = MoveFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(dest.exists());
        assert!(!source.exists());
    }

    #[tokio::test]
    async fn test_move_file_missing_source() {
        let tool = MoveFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": "/nonexistent/file.txt",
                "destination": "/tmp/dest.txt"
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Source path not found"));
    }

    #[tokio::test]
    async fn test_move_file_destination_exists_no_overwrite() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, "source content").await.unwrap();
        tokio::fs::write(&dest, "dest content").await.unwrap();

        let tool = MoveFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_move_file_destination_exists_with_overwrite() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, "source content").await.unwrap();
        tokio::fs::write(&dest, "dest content").await.unwrap();

        let tool = MoveFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap(),
                "overwrite": true
            }))
            .await;

        assert!(result.is_ok());
        // 验证目标文件内容是源文件内容
        let content = tokio::fs::read_to_string(&dest).await.unwrap();
        assert_eq!(content, "source content");
    }

    #[test]
    fn test_copy_file_tool_meta() {
        let tool = CopyFileTool;
        assert_eq!(tool.name(), "copy_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_copy_file_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // 创建源文件
        tokio::fs::write(&source, "test content").await.unwrap();

        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(dest.exists());
        assert!(source.exists()); // 复制后源文件仍然存在
        let content = tokio::fs::read_to_string(&dest).await.unwrap();
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn test_copy_file_missing_source() {
        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": "/nonexistent/file.txt",
                "destination": "/tmp/dest.txt"
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Source path not found"));
    }

    #[tokio::test]
    async fn test_copy_file_destination_exists_no_overwrite() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, "source content").await.unwrap();
        tokio::fs::write(&dest, "dest content").await.unwrap();

        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_copy_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_dir = temp_dir.path().join("dest_dir");

        // 创建源目录和文件
        tokio::fs::create_dir_all(&source_dir).await.unwrap();
        tokio::fs::write(source_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        tokio::fs::write(source_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let tool = CopyFileTool;
        let result = tool
            .execute(serde_json::json!({
                "source": source_dir.to_str().unwrap(),
                "destination": dest_dir.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(dest_dir.exists());
        assert!(dest_dir.join("file1.txt").exists());
        assert!(dest_dir.join("file2.txt").exists());
        assert!(source_dir.exists()); // 源目录仍然存在
    }

    #[test]
    fn test_delete_file_tool_meta() {
        let tool = DeleteFileTool;
        assert_eq!(tool.name(), "delete_file");
        assert_eq!(tool.category(), ToolCategory::FileOps);
        assert!(tool.is_dangerous());
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_delete_file_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");

        // 创建文件
        tokio::fs::write(&file, "test content").await.unwrap();

        let tool = DeleteFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn test_delete_file_missing_path() {
        let tool = DeleteFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": "/nonexistent/file.txt"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path not found"));
    }

    #[tokio::test]
    async fn test_delete_empty_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("empty_dir");

        // 创建空目录
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let tool = DeleteFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": dir.to_str().unwrap()
            }))
            .await;

        assert!(result.is_ok());
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn test_delete_non_empty_directory_without_recursive() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("non_empty_dir");

        // 创建非空目录
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("file.txt"), "content")
            .await
            .unwrap();

        let tool = DeleteFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": dir.to_str().unwrap()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("may not be empty"));
    }

    #[tokio::test]
    async fn test_delete_non_empty_directory_with_recursive() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("non_empty_dir");

        // 创建非空目录
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("file.txt"), "content")
            .await
            .unwrap();
        tokio::fs::create_dir_all(dir.join("subdir")).await.unwrap();
        tokio::fs::write(dir.join("subdir/nested.txt"), "nested")
            .await
            .unwrap();

        let tool = DeleteFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": dir.to_str().unwrap(),
                "recursive": true
            }))
            .await;

        assert!(result.is_ok());
        assert!(!dir.exists());
    }

    // ========== ReadFileTool 分页测试 ==========

    #[tokio::test]
    async fn test_read_file_no_pagination() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_eq!(result, "line1\nline2\nline3\n");
    }

    #[tokio::test]
    async fn test_read_file_with_offset() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\nline4\nline5\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 2
            }))
            .await
            .unwrap();

        assert!(result.contains("[Lines 2-5 of 5 total lines]"));
        assert!(result.contains("line3"));
        assert!(result.contains("line5"));
        assert!(!result.contains("line1"));
    }

    #[tokio::test]
    async fn test_read_file_with_limit() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\nline4\nline5\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "limit": 2
            }))
            .await
            .unwrap();

        assert!(result.contains("[Lines 0-2 of 5 total lines]"));
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
        assert!(!result.contains("line3"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset_and_limit() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        let content = "line1\nline2\nline3\nline4\nline5\n";
        tokio::fs::write(&file, content).await.unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 1,
                "limit": 2
            }))
            .await
            .unwrap();

        assert!(result.contains("[Lines 1-3 of 5 total lines]"));
        assert!(result.contains("line2"));
        assert!(result.contains("line3"));
        assert!(!result.contains("line1"));
        assert!(!result.contains("line4"));
    }

    #[tokio::test]
    async fn test_read_file_offset_exceeds_total() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        tokio::fs::write(&file, "line1\nline2\n").await.unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 10
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds total lines"));
    }

    #[tokio::test]
    async fn test_read_file_empty_range() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        tokio::fs::write(&file, "line1\nline2\nline3\n")
            .await
            .unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 3,
                "limit": 5
            }))
            .await
            .unwrap();

        assert!(result.contains("No content in this range"));
    }

    #[tokio::test]
    async fn test_read_file_single_line_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        tokio::fs::write(&file, "single line content")
            .await
            .unwrap();

        let tool = ReadFileTool;
        let result = tool
            .execute(serde_json::json!({
                "path": file.to_str().unwrap(),
                "offset": 0,
                "limit": 10
            }))
            .await
            .unwrap();

        assert!(result.contains("[Lines 0-1 of 1 total lines]"));
        assert!(result.contains("single line content"));
    }
}
