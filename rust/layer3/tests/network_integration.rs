//! Integration tests for network_tools SSRF protection (Task 2.4 + round-2 audit).
//!
//! network_tools HTTP tools (HttpGet/HttpPost/DownloadFile) now have SSRF
//! validation. These tests verify SSRF rejection (localhost/metadata blocked).
//! Success-path testing for HTTP tools is covered by network.rs's HttpRequestTool
//! (which has configurable limits via with_limits).

use serde_json::json;
use sh_layer3::builtin_tools::network_tools::{HttpGetTool, HttpPostTool};
use sh_layer3::builtin_tools::BuiltinTool;

#[tokio::test]
async fn http_get_rejects_localhost_ssrf() {
    let result = HttpGetTool
        .execute(json!({"url": "http://127.0.0.1:12345/test"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("loopback"));
}

#[tokio::test]
async fn http_get_rejects_metadata_endpoint_ssrf() {
    let result = HttpGetTool
        .execute(json!({"url": "http://169.254.169.254/latest/meta-data/"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn http_get_rejects_file_scheme_ssrf() {
    let result = HttpGetTool
        .execute(json!({"url": "file:///etc/passwd"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("scheme"));
}

#[tokio::test]
async fn http_post_rejects_localhost_ssrf() {
    let result = HttpPostTool
        .execute(json!({"url": "http://127.0.0.1:8080/api", "body": "test"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("loopback"));
}

#[tokio::test]
async fn http_get_missing_url_errors() {
    let result = HttpGetTool.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn http_post_missing_url_errors() {
    let result = HttpPostTool.execute(json!({})).await;
    assert!(result.is_err());
}
