//! # Network Tools
//!
//! HTTP request + web fetch tools with SSRF protection and size bounds.

use crate::builtin_tools::limits::FileOpsLimits;
use crate::builtin_tools::network_safety::{DefaultUrlValidator, UrlValidator};
use crate::builtin_tools::safe_truncate::safe_truncate_bytes;
use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const SENSITIVE_RESPONSE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "proxy-authorization",
];

/// HTTP Request Tool — full method + headers + body support.
pub struct HttpRequestTool {
    limits: Arc<FileOpsLimits>,
    validator: Arc<dyn UrlValidator>,
}

impl HttpRequestTool {
    pub fn new() -> Self {
        let limits = Arc::new(FileOpsLimits::default());
        let validator = Arc::new(DefaultUrlValidator::new(limits.clone()));
        Self { limits, validator }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        let validator = Arc::new(DefaultUrlValidator::new(limits.clone()));
        Self { limits, validator }
    }
}

impl Default for HttpRequestTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for HttpRequestTool {
    fn name(&self) -> &str {
        "http_request"
    }

    fn description(&self) -> &str {
        "HTTP request with SSRF protection, size limits, and header redaction."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string"},
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE", "HEAD", "PATCH"]
                },
                "headers": {"type": "object"},
                "body": {"type": "string"},
                "timeout": {"type": "integer", "description": "Seconds (default 30, max 300)"}
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url_str = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;
        let url =
            Url::parse(url_str).map_err(|e| anyhow::anyhow!("Invalid URL '{}': {}", url_str, e))?;

        // N1, N2: SSRF + scheme
        self.validator.validate(&url).await?;

        let method = args["method"].as_str().unwrap_or("GET").to_uppercase();
        let timeout_secs = args["timeout"]
            .as_u64()
            .unwrap_or(self.limits.http_default_timeout_secs)
            .min(self.limits.http_max_timeout_secs);

        // N3: redirect policy
        let redirect_policy = reqwest::redirect::Policy::limited(self.limits.max_http_redirect);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(redirect_policy)
            .user_agent("Continuum/1.0")
            .build()
            .map_err(|e| anyhow::anyhow!("HTTP client build failed: {}", e))?;

        let mut request = match method.as_str() {
            "GET" => client.get(url.clone()),
            "POST" => client.post(url.clone()),
            "PUT" => client.put(url.clone()),
            "DELETE" => client.delete(url.clone()),
            "HEAD" => client.head(url.clone()),
            "PATCH" => client.patch(url.clone()),
            _ => client.get(url.clone()),
        };

        if let Some(headers) = args["headers"].as_object() {
            for (k, v) in headers {
                if let Some(s) = v.as_str() {
                    request = request.header(k, s);
                }
            }
        }

        // N9: request body size
        if let Some(body) = args["body"].as_str() {
            if body.len() as u64 > self.limits.max_http_request_body_bytes {
                return Err(anyhow::anyhow!(
                    "http_request rejected: body {} bytes > limit {} bytes",
                    body.len(),
                    self.limits.max_http_request_body_bytes,
                ));
            }
            request = request.body(body.to_string());
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        let headers = response.headers().clone();

        // N6: header count cap
        if headers.len() > self.limits.max_http_header_count {
            return Err(anyhow::anyhow!(
                "http_request rejected: response has {} headers > limit {}",
                headers.len(),
                self.limits.max_http_header_count,
            ));
        }

        // N4: response body cap via streaming
        let body = if method == "HEAD" {
            String::new()
        } else {
            read_response_with_limit(response, self.limits.max_http_response_bytes).await?
        };

        let mut result = format!(
            "Status: {} {}\nHeaders:\n",
            status.as_u16(),
            status.canonical_reason().unwrap_or("")
        );
        for (name, value) in headers.iter() {
            // N8: redact sensitive response headers
            let display =
                if SENSITIVE_RESPONSE_HEADERS.contains(&name.as_str().to_lowercase().as_str()) {
                    "<redacted>".to_string()
                } else {
                    value.to_str().unwrap_or("<binary>").to_string()
                };
            result.push_str(&format!("  {}: {}\n", name, display));
        }
        if !body.is_empty() {
            result.push_str(&format!(
                "\nBody ({} bytes):\n{}",
                body.len(),
                safe_truncate_bytes(&body, 5000),
            ));
        }
        Ok(result)
    }
}

async fn read_response_with_limit(
    response: reqwest::Response,
    max_bytes: u64,
) -> Layer3Result<String> {
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(8192.min(max_bytes as usize));
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("Stream error: {}", e))?;
        buf.extend_from_slice(&chunk);
        if buf.len() as u64 > max_bytes {
            return Err(anyhow::anyhow!(
                "http_request rejected: response exceeded {} bytes limit",
                max_bytes,
            ));
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// WebFetch Tool — text extraction with same SSRF / size / UTF-8 safety.
pub struct WebFetchTool {
    limits: Arc<FileOpsLimits>,
    validator: Arc<dyn UrlValidator>,
}

impl WebFetchTool {
    pub fn new() -> Self {
        let limits = Arc::new(FileOpsLimits::default());
        let validator = Arc::new(DefaultUrlValidator::new(limits.clone()));
        Self { limits, validator }
    }

    pub fn with_limits(limits: Arc<FileOpsLimits>) -> Self {
        let validator = Arc::new(DefaultUrlValidator::new(limits.clone()));
        Self { limits, validator }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinTool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch webpage text content with SSRF protection and size limits."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string"}
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url_str = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url parameter"))?;
        let url =
            Url::parse(url_str).map_err(|e| anyhow::anyhow!("Invalid URL '{}': {}", url_str, e))?;

        self.validator.validate(&url).await?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.limits.http_default_timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(
                self.limits.max_http_redirect,
            ))
            .user_agent("Continuum/1.0")
            .build()?;

        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
        }

        let body = read_response_with_limit(response, self.limits.max_http_response_bytes).await?;
        let text = extract_text_from_html(&body);

        Ok(safe_truncate_bytes(&text, 10000).to_string())
    }
}

/// Naive HTML text extractor (strip script/style + tags).
fn extract_text_from_html(html: &str) -> String {
    let mut result = html.to_string();

    // Strip script tags (handle attribute-bearing opening tags)
    loop {
        let start = result.find("<script");
        let end = result.find("</script>").map(|e| e + "</script>".len());
        match (start, end) {
            (Some(s), Some(e)) if e > s => result.replace_range(s..e, ""),
            _ => break,
        }
    }
    loop {
        let start = result.find("<style");
        let end = result.find("</style>").map(|e| e + "</style>".len());
        match (start, end) {
            (Some(s), Some(e)) if e > s => result.replace_range(s..e, ""),
            _ => break,
        }
    }

    // Strip remaining tags
    let mut text = String::with_capacity(result.len());
    let mut in_tag = false;
    for c in result.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_tool_category() {
        let tool = HttpRequestTool::new();
        assert_eq!(tool.category(), ToolCategory::Network);
    }

    #[test]
    fn test_web_fetch_tool_category() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.category(), ToolCategory::Network);
    }

    #[test]
    fn test_extract_text_from_html() {
        let html = "<html><body><h1>Title</h1><p>Content here</p></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Content"));
    }

    #[test]
    fn test_extract_text_strips_script() {
        let html = r#"<p>before</p><script>alert("xss")</script><p>after</p>"#;
        let text = extract_text_from_html(html);
        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert!(!text.contains("alert"));
    }

    #[tokio::test]
    async fn test_http_request_missing_url() {
        let tool = HttpRequestTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing url"));
    }

    #[tokio::test]
    async fn test_http_request_rejects_localhost() {
        let tool = HttpRequestTool::new();
        let result = tool.execute(json!({"url": "http://127.0.0.1/"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("loopback"));
    }

    #[tokio::test]
    async fn test_http_request_rejects_aws_metadata() {
        let tool = HttpRequestTool::new();
        let result = tool
            .execute(json!({"url": "http://169.254.169.254/latest/meta-data/"}))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("metadata") || err.contains("link-local"));
    }

    #[tokio::test]
    async fn test_http_request_rejects_file_scheme() {
        let tool = HttpRequestTool::new();
        let result = tool.execute(json!({"url": "file:///etc/passwd"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("scheme"));
    }

    #[tokio::test]
    async fn test_web_fetch_rejects_localhost() {
        let tool = WebFetchTool::new();
        let result = tool.execute(json!({"url": "http://127.0.0.1/"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_http_request_rejects_oversized_body() {
        let tool = HttpRequestTool::new();
        let big_body = "x".repeat(2 * 1024 * 1024); // 2 MiB > default 1 MiB
        let result = tool
            .execute(json!({
                "url": "https://example.com/",
                "body": big_body,
            }))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("body") || err.contains("DNS") || err.contains("network"),
            "got: {}",
            err
        );
    }
}
