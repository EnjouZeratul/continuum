//! Integration tests for network_tools using wiremock (Task 2.4).
//!
//! HttpGetTool/HttpPostTool have NO SSRF protection (unlike network.rs tools),
//! so wiremock on localhost can test their actual request/response paths.

use serde_json::json;
use sh_layer3::builtin_tools::network_tools::{HttpGetTool, HttpPostTool};
use sh_layer3::builtin_tools::BuiltinTool;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_http_get_success_200() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello world"))
        .mount(&server)
        .await;

    let url = format!("{}/data", server.uri());
    let result = HttpGetTool.execute(json!({"url": url})).await.unwrap();
    assert!(result.contains("200"), "got: {}", result);
    assert!(result.contains("hello world"), "got: {}", result);
}

#[tokio::test]
async fn test_http_get_error_status_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let url = format!("{}/missing", server.uri());
    let result = HttpGetTool.execute(json!({"url": url})).await.unwrap();
    assert!(result.contains("404"), "got: {}", result);
}

#[tokio::test]
async fn test_http_get_missing_url_errors() {
    let result = HttpGetTool.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_http_post_body_received_by_server() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/submit"))
        .and(wiremock::matchers::body_string("payload-data"))
        .respond_with(ResponseTemplate::new(201).set_body_string("created"))
        .mount(&server)
        .await;

    let url = format!("{}/submit", server.uri());
    let result = HttpPostTool
        .execute(json!({"url": url, "body": "payload-data"}))
        .await
        .unwrap();
    assert!(result.contains("201"), "got: {}", result);
    assert!(result.contains("created"), "got: {}", result);
    // wiremock verifies the body_string matcher matched → body was sent
}

#[tokio::test]
async fn test_http_post_with_custom_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth"))
        .and(header("x-api-key", "secret123"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let url = format!("{}/auth", server.uri());
    let result = HttpPostTool
        .execute(json!({
            "url": url,
            "headers": {"x-api-key": "secret123"}
        }))
        .await
        .unwrap();
    assert!(result.contains("ok"), "got: {}", result);
    // wiremock verified header matcher → header was sent
}

#[tokio::test]
async fn test_http_post_default_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let url = format!("{}/api", server.uri());
    let result = HttpPostTool.execute(json!({"url": url})).await.unwrap();
    assert!(result.contains("200"), "got: {}", result);
}

#[tokio::test]
async fn test_http_post_missing_url_errors() {
    let result = HttpPostTool.execute(json!({})).await;
    assert!(result.is_err());
}
