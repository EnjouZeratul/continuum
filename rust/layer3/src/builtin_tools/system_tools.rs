//! # System Tools
//!
//! 系统工具集：环境变量、进程管理、系统信息。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use std::collections::HashMap;

// ============================================================================
// Get Environment Variable Tool
// ============================================================================

/// 环境变量获取工具
pub struct GetEnvTool;

#[async_trait]
impl BuiltinTool for GetEnvTool {
    fn name(&self) -> &str {
        "get_env"
    }

    fn description(&self) -> &str {
        "Get an environment variable value."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Environment variable name"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let name = args["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing name parameter"))?;

        std::env::var(name)
            .map_err(|_| anyhow::anyhow!("Environment variable '{}' not found", name))
    }
}

// ============================================================================
// List Environment Variables Tool
// ============================================================================

/// 环境变量列表工具
pub struct ListEnvTool;

#[async_trait]
impl BuiltinTool for ListEnvTool {
    fn name(&self) -> &str {
        "list_env"
    }

    fn description(&self) -> &str {
        "List all environment variables."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "Optional filter pattern (e.g., 'PATH')"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let filter = args["filter"].as_str().unwrap_or("");

        let env_vars: HashMap<String, String> = std::env::vars()
            .filter(|(k, _)| filter.is_empty() || k.contains(filter))
            .collect();

        let mut result: Vec<(String, String)> = env_vars.into_iter().collect();
        result.sort_by_key(|(k, _)| k.clone());

        let output: Vec<String> = result.iter().map(|(k, v)| format!("{}={}", k, v)).collect();

        Ok(output.join("\n"))
    }
}

// ============================================================================
// Set Environment Variable Tool
// ============================================================================

/// 环境变量设置工具
pub struct SetEnvTool;

#[async_trait]
impl BuiltinTool for SetEnvTool {
    fn name(&self) -> &str {
        "set_env"
    }

    fn description(&self) -> &str {
        "Set an environment variable for the current process."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Environment variable name"
                },
                "value": {
                    "type": "string",
                    "description": "Environment variable value"
                }
            },
            "required": ["name", "value"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let name = args["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing name parameter"))?;

        let value = args["value"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing value parameter"))?;

        std::env::set_var(name, value);
        Ok(format!("Set {}={}", name, value))
    }
}

// ============================================================================
// Current Working Directory Tool
// ============================================================================

/// 当前目录获取工具
pub struct GetCwdTool;

#[async_trait]
impl BuiltinTool for GetCwdTool {
    fn name(&self) -> &str {
        "get_cwd"
    }

    fn description(&self) -> &str {
        "Get the current working directory."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, _args: serde_json::Value) -> Layer3Result<String> {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .map_err(|e| anyhow::anyhow!("Failed to get cwd: {}", e))
    }
}

// ============================================================================
// Change Directory Tool
// ============================================================================

/// 目录切换工具
pub struct ChangeDirTool;

#[async_trait]
impl BuiltinTool for ChangeDirTool {
    fn name(&self) -> &str {
        "change_dir"
    }

    fn description(&self) -> &str {
        "Change the current working directory."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to change to"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;

        std::env::set_current_dir(path)
            .map_err(|e| anyhow::anyhow!("Failed to change directory: {}", e))?;

        let new_cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.to_string());

        Ok(format!("Changed directory to: {}", new_cwd))
    }
}

// ============================================================================
// System Info Tool
// ============================================================================

/// 系统信息工具
pub struct SystemInfoTool;

#[async_trait]
impl BuiltinTool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }

    fn description(&self) -> &str {
        "Get system information (OS, architecture, version)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, _args: serde_json::Value) -> Layer3Result<String> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let sep = std::path::MAIN_SEPARATOR;

        let mut info = vec![
            format!("OS: {}", os),
            format!("Architecture: {}", arch),
            format!("Path separator: {}", sep),
        ];

        // Try to get hostname
        if let Ok(hostname) = hostname::get() {
            info.push(format!("Hostname: {}", hostname.to_string_lossy()));
        }

        // Get user info
        if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
            info.push(format!("User: {}", user));
        }

        // Get home directory
        if let Some(home) = dirs::data_local_dir() {
            info.push(format!("Data: {}", home.display()));
        }

        // Get temp directory from env
        let temp = std::env::temp_dir();
        info.push(format!("Temp: {}", temp.display()));

        Ok(info.join("\n"))
    }
}

// ============================================================================
// Process List Tool
// ============================================================================

/// 进程列表工具
pub struct ProcessListTool;

#[async_trait]
impl BuiltinTool for ProcessListTool {
    fn name(&self) -> &str {
        "process_list"
    }

    fn description(&self) -> &str {
        "List running processes (platform-dependent, limited info)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "Optional filter by process name"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let filter = args["filter"].as_str().unwrap_or("");

        // Use sysinfo crate for process listing
        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        let processes: Vec<String> = system
            .processes()
            .iter()
            .filter(|(_, proc)| filter.is_empty() || proc.name().contains(filter))
            .map(|(pid, proc)| format!("{}\t{}\t{}", pid, proc.name(), proc.cmd().join(" ")))
            .take(50) // Limit output
            .collect();

        Ok(format!("PID\tName\tCommand\n{}", processes.join("\n")))
    }
}

// ============================================================================
// Disk Usage Tool
// ============================================================================

/// 磁盘使用工具
pub struct DiskUsageTool;

#[async_trait]
impl BuiltinTool for DiskUsageTool {
    fn name(&self) -> &str {
        "disk_usage"
    }

    fn description(&self) -> &str {
        "Get disk usage information for a path."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to check (default: current directory)"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let _path = args["path"].as_str().unwrap_or(".");

        // Use sysinfo to get disk info
        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        // Get disks using the Disks API
        let disks = sysinfo::Disks::new_with_refreshed_list();

        let results: Vec<String> = disks
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total - available;
                let mount = disk.mount_point().display();
                format!(
                    "{}: {:.2} GB used of {:.2} GB ({:.0}% free)",
                    mount,
                    used as f64 / 1e9,
                    total as f64 / 1e9,
                    (available as f64 / total as f64) * 100.0
                )
            })
            .collect();

        if results.is_empty() {
            Ok("No disk information available".to_string())
        } else {
            Ok(results.join("\n"))
        }
    }
}

// ============================================================================
// Memory Usage Tool
// ============================================================================

/// 内存使用工具
pub struct MemoryUsageTool;

#[async_trait]
impl BuiltinTool for MemoryUsageTool {
    fn name(&self) -> &str {
        "memory_usage"
    }

    fn description(&self) -> &str {
        "Get memory usage information."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(&self, _args: serde_json::Value) -> Layer3Result<String> {
        let mut system = sysinfo::System::new_all();
        system.refresh_memory();

        let total = system.total_memory();
        let used = system.used_memory();
        let available = system.available_memory();

        Ok(format!(
            "Memory: {:.2} GB used of {:.2} GB ({:.0}% free)",
            used as f64 / 1e9,
            total as f64 / 1e9,
            (available as f64 / total as f64) * 100.0
        ))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_env_category() {
        let tool = GetEnvTool;
        assert_eq!(tool.category(), ToolCategory::System);
    }

    #[tokio::test]
    async fn test_get_cwd() {
        let tool = GetCwdTool;
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_env() {
        let tool = ListEnvTool;
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[tokio::test]
    async fn test_system_info() {
        let tool = SystemInfoTool;
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("OS:"));
    }

    #[tokio::test]
    async fn test_memory_usage() {
        let tool = MemoryUsageTool;
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Memory:"));
    }
}
