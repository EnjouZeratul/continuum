//! # Network Tools
//!
//! 网络工具集：HTTP 请求、文件下载、WebSocket 等。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

// ============================================================================
// HTTP GET Tool
// ============================================================================

/// HTTP GET 请求工具
pub struct HttpGetTool;

#[async_trait]
impl BuiltinTool for HttpGetTool {
    fn name(&self) -> &str {
        "http_get"
    }

    fn description(&self) -> &str {
        "Make an HTTP GET request and return the response body."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to request"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Request timeout in seconds (default: 30)"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional headers to include"
                }
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;

        let timeout_secs = args["timeout"].as_u64().unwrap_or(30);

        let mut request = reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(timeout_secs));

        if let Some(headers) = args["headers"].as_object() {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read response body: {}", e))?;

        Ok(format!("Status: {}\n\n{}", status, body))
    }
}

// ============================================================================
// HTTP POST Tool
// ============================================================================

/// HTTP POST 请求工具
pub struct HttpPostTool;

#[async_trait]
impl BuiltinTool for HttpPostTool {
    fn name(&self) -> &str {
        "http_post"
    }

    fn description(&self) -> &str {
        "Make an HTTP POST request with optional body."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to request"
                },
                "body": {
                    "type": "string",
                    "description": "Request body (JSON string or plain text)"
                },
                "content_type": {
                    "type": "string",
                    "description": "Content-Type header (default: application/json)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Request timeout in seconds (default: 30)"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional headers to include"
                }
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;

        let timeout_secs = args["timeout"].as_u64().unwrap_or(30);
        let content_type = args["content_type"].as_str().unwrap_or("application/json");
        let body = args["body"].as_str().unwrap_or("");

        let mut request = reqwest::Client::new()
            .post(url)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .header("Content-Type", content_type)
            .body(body.to_string());

        if let Some(headers) = args["headers"].as_object() {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read response body: {}", e))?;

        Ok(format!("Status: {}\n\n{}", status, response_body))
    }
}

// ============================================================================
// Download File Tool
// ============================================================================

/// 文件下载工具
pub struct DownloadFileTool;

#[async_trait]
impl BuiltinTool for DownloadFileTool {
    fn name(&self) -> &str {
        "download_file"
    }

    fn description(&self) -> &str {
        "Download a file from URL and save to local path."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to download from"
                },
                "path": {
                    "type": "string",
                    "description": "Local path to save the file"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Download timeout in seconds (default: 60)"
                }
            },
            "required": ["url", "path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;

        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path parameter"))?;

        let timeout_secs = args["timeout"].as_u64().unwrap_or(60);

        let response = reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Download failed with status: {}",
                response.status()
            ));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read response: {}", e))?;

        // Create parent directory if needed
        let file_path = std::path::Path::new(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))?;
        }

        std::fs::write(file_path, &bytes)
            .map_err(|e| anyhow::anyhow!("Failed to write file: {}", e))?;

        Ok(format!("Downloaded {} bytes to {}", bytes.len(), path))
    }
}

// ============================================================================
// WebSocket Connect Tool
// ============================================================================

/// WebSocket 连接工具
pub struct WebSocketConnectTool;

#[async_trait]
impl BuiltinTool for WebSocketConnectTool {
    fn name(&self) -> &str {
        "websocket_connect"
    }

    fn description(&self) -> &str {
        "Connect to a WebSocket server and send/receive messages. Returns initial messages."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "WebSocket URL (ws:// or wss://)"
                },
                "message": {
                    "type": "string",
                    "description": "Initial message to send"
                },
                "receive_count": {
                    "type": "integer",
                    "description": "Number of messages to receive (default: 1)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 10)"
                }
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;

        let message = args["message"].as_str().unwrap_or("");
        let receive_count = args["receive_count"].as_u64().unwrap_or(1).min(100) as usize;
        let timeout_secs = args["timeout"].as_u64().unwrap_or(10);

        // Connect to WebSocket with timeout
        let connect_future = connect_async(url);
        let connection = tokio::time::timeout(Duration::from_secs(timeout_secs), connect_future)
            .await
            .map_err(|_| anyhow::anyhow!("WebSocket connection timeout after {}s", timeout_secs))?
            .map_err(|e| anyhow::anyhow!("WebSocket connection failed: {}", e))?;

        let (mut ws_stream, _response) = connection;

        // Send initial message if provided
        if !message.is_empty() {
            ws_stream
                .send(WsMessage::Text(message.into()))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
        }

        // Receive messages
        let mut received_messages: Vec<String> = Vec::new();
        let receive_timeout = Duration::from_secs(timeout_secs);

        for _ in 0..receive_count {
            match tokio::time::timeout(receive_timeout, ws_stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    match msg {
                        WsMessage::Text(text) => received_messages.push(text.to_string()),
                        WsMessage::Binary(data) => {
                            received_messages.push(format!("<binary: {} bytes>", data.len()));
                        }
                        WsMessage::Ping(ping) => {
                            // Respond to ping with pong
                            let _ = ws_stream.send(WsMessage::Pong(ping)).await;
                        }
                        WsMessage::Close(_) => {
                            received_messages.push("<connection closed>".to_string());
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    return Err(anyhow::anyhow!("WebSocket error: {}", e));
                }
                Ok(None) => {
                    received_messages.push("<stream ended>".to_string());
                    break;
                }
                Err(_) => {
                    if received_messages.is_empty() {
                        return Err(anyhow::anyhow!(
                            "No message received within {}s",
                            timeout_secs
                        ));
                    }
                    break;
                }
            }
        }

        // Close connection
        let _ = ws_stream.close(None).await;

        if received_messages.is_empty() {
            Ok(format!(
                "WebSocket connected to {} (no messages received)",
                url
            ))
        } else {
            Ok(format!(
                "WebSocket connected to {}:\n{}",
                url,
                received_messages.join("\n")
            ))
        }
    }
}

// ============================================================================
// Ping Tool
// ============================================================================

/// Ping 工具
pub struct PingTool;

#[async_trait]
impl BuiltinTool for PingTool {
    fn name(&self) -> &str {
        "ping"
    }

    fn description(&self) -> &str {
        "Check if a host is reachable via HTTP HEAD request."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to ping"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 5)"
                }
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;

        let timeout_secs = args["timeout"].as_u64().unwrap_or(5);

        let start = std::time::Instant::now();

        let response = reqwest::Client::new()
            .head(url)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .send()
            .await;

        let elapsed_ms = start.elapsed().as_millis();

        match response {
            Ok(resp) => {
                let status = resp.status();
                Ok(format!(
                    "Ping successful: {} (status: {}, {}ms)",
                    url, status, elapsed_ms
                ))
            }
            Err(e) => Ok(format!(
                "Ping failed: {} (error: {}, {}ms)",
                url, e, elapsed_ms
            )),
        }
    }
}

// ============================================================================
// DNS Lookup Tool
// ============================================================================

/// DNS 解析工具
pub struct DnsLookupTool;

#[async_trait]
impl BuiltinTool for DnsLookupTool {
    fn name(&self) -> &str {
        "dns_lookup"
    }

    fn description(&self) -> &str {
        "Resolve DNS for a hostname. Returns IP addresses."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "hostname": {
                    "type": "string",
                    "description": "Hostname to resolve"
                }
            },
            "required": ["hostname"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let hostname = args["hostname"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing hostname parameter"))?;

        use tokio::net::lookup_host;

        let addresses = lookup_host(hostname)
            .await
            .map_err(|e| anyhow::anyhow!("DNS lookup failed: {}", e))?;

        let results: Vec<String> = addresses.map(|addr| addr.ip().to_string()).collect();

        Ok(format!("Resolved {} to:\n{}", hostname, results.join("\n")))
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
    fn test_http_get_category() {
        let tool = HttpGetTool;
        assert_eq!(tool.category(), ToolCategory::Network);
    }

    #[test]
    fn test_http_post_category() {
        let tool = HttpPostTool;
        assert_eq!(tool.category(), ToolCategory::Network);
    }

    #[test]
    fn test_download_file_requires_confirmation() {
        let tool = DownloadFileTool;
        assert!(tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_ping_format() {
        let tool = PingTool;
        let result = tool.execute(json!({"url": "https://example.com"})).await;
        // Will succeed or fail based on network, but should be formatted correctly
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Ping") || output.contains("example.com"));
    }
}
