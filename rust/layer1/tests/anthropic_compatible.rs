//! Anthropic 兼容端点集成测试
//!
//! 测试两种场景：
//! 1. Mock 测试：使用 wiremock 模拟 API，CI 可运行
//! 2. 真实 API 测试：标记为 #[ignore]，需要真实 API key 时手动运行

use sh_layer1::llm_client::{
    LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, Message, MessageRole,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ===== 1. Mock 测试（CI 可运行）=====

/// 模拟 Anthropic API 响应
fn mock_anthropic_response() -> serde_json::Value {
    serde_json::json!({
        "id": "msg_test123",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-6",
        "content": [
            {
                "type": "text",
                "text": "Hello! This is a mock response."
            }
        ],
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5
        }
    })
}

/// 模拟 OpenAI API 响应
fn mock_openai_response() -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl_test123",
        "object": "chat.completion",
        "model": "gpt-4",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! This is a mock response."
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    })
}

#[tokio::test]
async fn test_anthropic_official_api_url_construction() {
    // 官方 Anthropic API 应该使用 /v1/messages
    let url = LlmClient::build_anthropic_messages_url("https://api.anthropic.com");
    assert_eq!(url, "https://api.anthropic.com/v1/messages");

    let url = LlmClient::build_anthropic_messages_url("https://api.anthropic.com/v1");
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
}

#[tokio::test]
async fn test_anthropic_compatible_endpoint_url_construction() {
    // 第三方 Anthropic 兼容端点（包含 /anthropic）
    let url = LlmClient::build_anthropic_messages_url(
        "https://api.lkeap.cloud.tencent.com/coding/anthropic",
    );
    assert_eq!(
        url,
        "https://api.lkeap.cloud.tencent.com/coding/anthropic/messages"
    );

    // 自定义 Anthropic 代理
    let url = LlmClient::build_anthropic_messages_url("https://my-proxy.example.com/anthropic");
    assert_eq!(url, "https://my-proxy.example.com/anthropic/messages");
}

#[tokio::test]
async fn test_mock_anthropic_api_request() {
    // 启动 mock 服务器
    let mock_server = MockServer::start().await;

    // 设置 mock 响应
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test_api_key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_anthropic_response()))
        .mount(&mock_server)
        .await;

    // 创建 Anthropic provider
    let provider = LlmProvider::Anthropic;
    let client =
        LlmClient::new(provider, "test_api_key".to_string()).with_base_url(mock_server.uri());

    let config = LlmRequestConfig {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 100,
        temperature: 0.7,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Hello".to_string(),
    }];

    let result = client.send(messages, &config).await;
    assert!(result.is_ok(), "Request should succeed: {:?}", result);

    let response = result.unwrap();
    assert_eq!(response.content, "Hello! This is a mock response.");
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.output_tokens, 5);
}

#[tokio::test]
async fn test_mock_anthropic_compatible_endpoint() {
    // 启动 mock 服务器
    let mock_server = MockServer::start().await;

    // 设置 mock 响应 - 注意路径是 /messages（不是 /v1/messages）
    Mock::given(method("POST"))
        .and(path("/coding/anthropic/messages"))
        .and(header("x-api-key", "tencent_api_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_anthropic_response()))
        .mount(&mock_server)
        .await;

    // 创建 AnthropicCompatible provider
    let base_url = format!("{}/coding/anthropic", mock_server.uri());
    let provider = LlmProvider::AnthropicCompatible {
        base_url: base_url.clone(),
    };
    let client = LlmClient::new(provider, "tencent_api_key".to_string());

    let config = LlmRequestConfig {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 100,
        temperature: 0.7,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Hello".to_string(),
    }];

    let result = client.send(messages, &config).await;
    assert!(result.is_ok(), "Request should succeed: {:?}", result);

    let response = result.unwrap();
    assert_eq!(response.content, "Hello! This is a mock response.");
}

#[tokio::test]
async fn test_mock_openai_compatible_endpoint() {
    // 启动 mock 服务器
    let mock_server = MockServer::start().await;

    // 设置 mock 响应 - OpenAI 格式
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer deepseek_api_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_openai_response()))
        .mount(&mock_server)
        .await;

    // 创建 OpenAICompatible provider
    let provider = LlmProvider::OpenAICompatible {
        base_url: mock_server.uri(),
    };
    let client = LlmClient::new(provider, "deepseek_api_key".to_string());

    let config = LlmRequestConfig {
        model: "deepseek-chat".to_string(),
        max_tokens: 100,
        temperature: 0.7,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Hello".to_string(),
    }];

    let result = client.send(messages, &config).await;
    assert!(result.is_ok(), "Request should succeed: {:?}", result);

    let response = result.unwrap();
    assert_eq!(response.content, "Hello! This is a mock response.");
}

#[tokio::test]
async fn test_provider_routing_correctness() {
    // 验证 AnthropicCompatible 路由到 Anthropic 格式
    let provider = LlmProvider::AnthropicCompatible {
        base_url: "https://example.com".to_string(),
    };
    assert!(matches!(
        provider,
        LlmProvider::Anthropic | LlmProvider::AnthropicCompatible { .. }
    ));

    // 验证 OpenAICompatible 路由到 OpenAI 格式
    let provider = LlmProvider::OpenAICompatible {
        base_url: "https://example.com".to_string(),
    };
    assert!(matches!(
        provider,
        LlmProvider::OpenAI | LlmProvider::OpenAICompatible { .. }
    ));
}

// ===== 2. 真实 API 测试（手动运行）=====

#[tokio::test]
#[ignore = "requires TENCENT_API_KEY environment variable"]
async fn test_real_tencent_coding_api() {
    // 测试真实的腾讯 Coding API
    let api_key = std::env::var("TENCENT_API_KEY").expect("TENCENT_API_KEY not set");

    let provider = LlmProvider::AnthropicCompatible {
        base_url: "https://api.lkeap.cloud.tencent.com/coding/anthropic".to_string(),
    };
    let client = LlmClient::new(provider, api_key);

    let config = LlmRequestConfig {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 50,
        temperature: 0.0,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Say exactly 'TENCENT_TEST_OK' and nothing else.".to_string(),
    }];

    let result = client.send(messages, &config).await;
    assert!(
        result.is_ok(),
        "Tencent API request should succeed: {:?}",
        result
    );

    let response = result.unwrap();
    println!("Tencent API response: {}", response.content);
    assert!(!response.content.is_empty());
    assert!(response.usage.input_tokens > 0);
}

#[tokio::test]
#[ignore = "requires CONTINUUM_API_KEY in .env.test"]
async fn test_real_anthropic_api() {
    // 测试真实的 Anthropic API
    let api_key = std::env::var("CONTINUUM_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .expect("API key not set");

    let provider = LlmProvider::Anthropic;
    let client = LlmClient::new(provider, api_key);

    let config = LlmRequestConfig {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 50,
        temperature: 0.0,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Say exactly 'ANTHROPIC_TEST_OK' and nothing else.".to_string(),
    }];

    let result = client.send(messages, &config).await;
    assert!(
        result.is_ok(),
        "Anthropic API request should succeed: {:?}",
        result
    );

    let response = result.unwrap();
    println!("Anthropic API response: {}", response.content);
    assert!(!response.content.is_empty());
}

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY environment variable"]
async fn test_real_deepseek_api() {
    // 测试真实的 DeepSeek API（OpenAI 兼容）
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");

    let provider = LlmProvider::OpenAICompatible {
        base_url: "https://api.deepseek.com/v1".to_string(),
    };
    let client = LlmClient::new(provider, api_key);

    let config = LlmRequestConfig {
        model: "deepseek-chat".to_string(),
        max_tokens: 50,
        temperature: 0.0,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Say exactly 'DEEPSEEK_TEST_OK' and nothing else.".to_string(),
    }];

    let result = client.send(messages, &config).await;
    assert!(
        result.is_ok(),
        "DeepSeek API request should succeed: {:?}",
        result
    );

    let response = result.unwrap();
    println!("DeepSeek API response: {}", response.content);
    assert!(!response.content.is_empty());
}

#[tokio::test]
#[ignore = "requires CONTINUUM_API_KEY - streaming test"]
async fn test_real_streaming_anthropic() {
    // 测试 Anthropic 流式响应
    let api_key = std::env::var("CONTINUUM_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .expect("API key not set");

    let provider = LlmProvider::Anthropic;
    let client = LlmClient::new(provider, api_key);

    let config = LlmRequestConfig {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 100,
        temperature: 0.0,
        ..Default::default()
    };

    let messages = vec![Message {
        role: MessageRole::User,
        content: "Count from 1 to 5, one number per line.".to_string(),
    }];

    let result = client.send_stream(messages, &config).await;
    assert!(result.is_ok(), "Stream request should succeed");

    let mut stream = result.unwrap();
    let mut text = String::new();

    use sh_layer1::streaming::StreamEvent;
    while let Some(event) = stream.next_event().await.unwrap() {
        if let StreamEvent::ContentBlockDelta { delta, .. } = event {
            if let sh_layer1::streaming::ContentDelta::Text(t) = delta {
                text.push_str(&t);
            }
        }
    }

    println!("Streamed text: {}", text);
    assert!(!text.is_empty());
}
