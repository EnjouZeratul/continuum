//! Plugin-to-Tool Adapter
//!
//! Bridges Layer4 WASM plugins to Layer2 Tool trait, enabling runtime
//! installation of agent-authored capabilities into the mutable ToolRegistry.
//!
//! This is the critical missing link for self-evolving agents:
//!   WASM sandbox (layer4) → PluginToolAdapter → ToolRegistry (layer2)

use crate::plugin_loader::Plugin;
use sh_layer2::{Layer2Result, Tool as Layer2Tool, ToolResult};
use std::sync::Arc;

/// Adapter: wraps a plugin as a Layer2 Tool
pub struct PluginToolAdapter<P: Plugin + 'static> {
    inner: Arc<P>,
    tool_name: String,
    tool_description: String,
    parameters_schema: serde_json::Value,
}

impl<P: Plugin + 'static> PluginToolAdapter<P> {
    /// Create adapter from plugin instance with tool metadata
    pub fn new(
        plugin: Arc<P>,
        tool_name: String,
        tool_description: String,
        parameters_schema: serde_json::Value,
    ) -> Self {
        Self {
            inner: plugin,
            tool_name,
            tool_description,
            parameters_schema,
        }
    }

    /// Get reference to inner plugin (for lifecycle management)
    pub fn plugin(&self) -> &P {
        &self.inner
    }
}

#[async_trait::async_trait]
impl<P: Plugin + 'static> Layer2Tool for PluginToolAdapter<P> {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters(&self) -> serde_json::Value {
        self.parameters_schema.clone()
    }

    async fn execute(&self, args: &str) -> Layer2Result<ToolResult> {
        // Parse args as JSON (empty string → empty object)
        let input: serde_json::Value = if args.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(args).map_err(|e| {
                sh_layer2::Layer2Error::AgentError(format!("Parse args error: {}", e))
            })?
        };

        // Execute plugin (runs in WASM sandbox with capability restrictions)
        let output = self
            .inner
            .execute(&input)
            .await
            .map_err(|e| sh_layer2::Layer2Error::AgentError(format!("Plugin error: {}", e)))?;

        // Serialize output
        let content = serde_json::to_string(&output)
            .unwrap_or_else(|_| output.to_string());

        Ok(ToolResult {
            tool_call_id: String::new(),
            name: self.tool_name.clone(),
            content,
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_loader::{Plugin, PluginContext};
    use crate::types::Layer4Result;

    /// Mock plugin for testing the adapter
    struct MockPlugin {
        name: String,
    }

    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn name(&self) -> &str {
            &self.name
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn description(&self) -> &str {
            "Mock plugin for testing"
        }
        fn dependencies(&self) -> Vec<&str> {
            vec![]
        }
        async fn initialize(&self, _ctx: &PluginContext) -> Layer4Result<()> {
            Ok(())
        }
        async fn execute(&self, input: &serde_json::Value) -> Layer4Result<serde_json::Value> {
            Ok(serde_json::json!({
                "echo": input,
                "plugin": self.name,
            }))
        }
        async fn shutdown(&self) -> Layer4Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_plugin_tool_adapter_basic() {
        let plugin = Arc::new(MockPlugin {
            name: "test_plugin".to_string(),
        });
        let adapter = PluginToolAdapter::new(
            plugin,
            "test_tool".to_string(),
            "Test tool".to_string(),
            serde_json::json!({"type": "object"}),
        );

        assert_eq!(adapter.name(), "test_tool");
        assert_eq!(adapter.description(), "Test tool");
        assert!(adapter.parameters().is_object());
    }

    #[tokio::test]
    async fn test_plugin_tool_adapter_execute() {
        let plugin = Arc::new(MockPlugin {
            name: "echo".to_string(),
        });
        let adapter = PluginToolAdapter::new(
            plugin,
            "echo_tool".to_string(),
            "Echo".to_string(),
            serde_json::json!({"type": "object"}),
        );

        let result = adapter.execute(r#"{"msg":"hello"}"#).await.unwrap();
        assert_eq!(result.name, "echo_tool");
        assert!(!result.is_error);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_plugin_tool_adapter_empty_args() {
        let plugin = Arc::new(MockPlugin {
            name: "noop".to_string(),
        });
        let adapter = PluginToolAdapter::new(
            plugin,
            "noop".to_string(),
            "No-op".to_string(),
            serde_json::json!({"type": "object"}),
        );

        let result = adapter.execute("").await.unwrap();
        assert!(!result.is_error);
    }
}
