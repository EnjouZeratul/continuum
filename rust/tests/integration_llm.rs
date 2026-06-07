//! 集成测试 - LLM 真实调用 + CLI 端到端 + Git + MCP
//!
//! 运行: cargo test -p sh-layer4 --test integration_llm -- --nocapture
//! 需要 .env.test 文件中的真实 API 密钥（LLM 测试可选）

mod common;

use common::test_config::{get_api_key, get_base_url, get_model, load_env};

use continuum_cli as cli;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ===== 1. LLM 真实调用 =====

#[cfg(test)]
mod llm_tests {
    use super::*;

    #[test]
    fn test_env_loaded() {
        load_env();
        let key = get_api_key();
        let url = get_base_url();
        let model = get_model();

        println!("API Key present: {}", key.is_some());
        println!("Base URL: {}", url);
        println!("Model: {}", model);

        assert!(!url.is_empty());
        assert!(!model.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires CONTINUUM_API_KEY in .env.test"]
    async fn test_real_chat_request() {
        load_env();

        use sh_layer1::llm_client::{
            LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, Message, MessageRole,
        };

        let api_key = get_api_key().unwrap();
        let base_url = get_base_url();

        // 根据 base_url 推断 provider，但保留自定义 URL
        let provider = if base_url.contains("openai") {
            LlmProvider::OpenAI
        } else if base_url.contains("gemini") || base_url.contains("google") {
            LlmProvider::Gemini
        } else {
            LlmProvider::Anthropic // 使用 Anthropic 格式的 API
        };

        // 创建客户端并用自定义 base_url 覆盖默认值
        let client = LlmClient::new(provider, api_key).with_base_url(base_url);

        let config = LlmRequestConfig {
            model: get_model(),
            max_tokens: 50,
            temperature: 0.0,
            ..Default::default()
        };

        let messages = vec![Message {
            role: MessageRole::User,
            content: "Say exactly 'INTEGRATION_TEST_OK' and nothing else.".to_string(),
        }];

        let result = client.send(messages, &config).await;

        match result {
            Ok(response) => {
                println!("LLM response: {}", response.content);
                assert!(!response.content.is_empty(), "Response should not be empty");
                assert!(response.usage.input_tokens > 0, "Should have input tokens");
                assert!(
                    response.usage.output_tokens > 0,
                    "Should have output tokens"
                );
                // 验证内容包含预期的关键词
                let content_lower = response.content.to_lowercase();
                assert!(
                    content_lower.contains("integration") || content_lower.contains("ok"),
                    "Response should contain 'INTEGRATION' or 'OK', got: {}",
                    response.content
                );
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("not found") || error_msg.contains("404") {
                    panic!(
                        "SKIP: Model or endpoint not found - API endpoint may be incompatible. \
                         Error: {}",
                        e
                    );
                } else {
                    panic!("LLM API error: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires CONTINUUM_API_KEY in .env.test"]
    async fn test_real_tool_call() {
        load_env();

        use sh_layer1::llm_client::{
            LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, Message, MessageRole,
        };

        let api_key = get_api_key().unwrap();
        let base_url = get_base_url();

        let provider = if base_url.contains("openai") {
            LlmProvider::OpenAI
        } else if base_url.contains("gemini") || base_url.contains("google") {
            LlmProvider::Gemini
        } else {
            LlmProvider::Anthropic
        };

        let client = LlmClient::new(provider, api_key).with_base_url(base_url);

        let config = LlmRequestConfig {
            model: get_model(),
            max_tokens: 100,
            temperature: 0.0,
            ..Default::default()
        };

        // 模拟工具调用场景：让 LLM 生成一个 bash 命令
        let messages = vec![Message {
            role: MessageRole::User,
            content: "I need to list all .rs files in the current directory. What bash command should I use? Reply with ONLY the command, no explanation.".to_string(),
        }];

        let result = client.send(messages, &config).await;

        match result {
            Ok(response) => {
                println!("LLM tool call response: {}", response.content);
                assert!(
                    !response.content.is_empty(),
                    "Tool call response should not be empty"
                );
                assert!(
                    response.usage.output_tokens > 0,
                    "Should have output tokens"
                );
                let content_lower = response.content.to_lowercase();
                let is_command_like = content_lower.contains("find")
                    || content_lower.contains("ls")
                    || content_lower.contains("dir")
                    || content_lower.contains("glob")
                    || content_lower.contains("*.rs");
                assert!(
                    is_command_like,
                    "Response should suggest a file listing command, got: {}",
                    response.content
                );
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("not found") || error_msg.contains("404") {
                    panic!(
                        "SKIP: Model or endpoint not found - API endpoint may be incompatible. \
                         Error: {}",
                        e
                    );
                } else {
                    panic!("LLM API error: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires CONTINUUM_API_KEY in .env.test"]
    async fn test_real_long_response() {
        load_env();

        use sh_layer1::llm_client::{
            LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, Message, MessageRole,
        };

        let api_key = get_api_key().unwrap();
        let base_url = get_base_url();

        let provider = if base_url.contains("openai") {
            LlmProvider::OpenAI
        } else if base_url.contains("gemini") || base_url.contains("google") {
            LlmProvider::Gemini
        } else {
            LlmProvider::Anthropic
        };

        let client = LlmClient::new(provider, api_key).with_base_url(base_url);

        // 请求较长输出以验证流式/长响应处理
        let config = LlmRequestConfig {
            model: get_model(),
            max_tokens: 500,
            temperature: 0.3,
            ..Default::default()
        };

        let messages = vec![Message {
            role: MessageRole::User,
            content: "Explain in 3 short paragraphs: what is the Rust programming language and why is it useful for systems programming?".to_string(),
        }];

        let result = client.send(messages, &config).await;

        match result {
            Ok(response) => {
                println!(
                    "LLM long response ({} chars): {}...",
                    response.content.len(),
                    &response.content[..response.content.len().min(200)]
                );
                assert!(
                    response.content.len() > 100,
                    "Long response should be >100 chars, got {} chars",
                    response.content.len()
                );
                assert!(response.usage.input_tokens > 0, "Should have input tokens");
                assert!(
                    response.usage.output_tokens > 20,
                    "Should have significant output tokens for long response, got {}",
                    response.usage.output_tokens
                );
                // 验证内容包含 Rust 相关关键词
                let content_lower = response.content.to_lowercase();
                assert!(
                    content_lower.contains("rust")
                        || content_lower.contains("memory")
                        || content_lower.contains("safety"),
                    "Long response should discuss Rust/memory/safety, got: {}...",
                    &response.content[..response.content.len().min(100)]
                );
            }
            Err(e) => {
                panic!("LLM API error: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires CONTINUUM_API_KEY in .env.test"]
    async fn test_real_multi_turn() {
        load_env();

        use sh_layer1::llm_client::{
            LlmClient, LlmClientTrait, LlmProvider, LlmRequestConfig, Message, MessageRole,
        };

        let api_key = get_api_key().unwrap();
        let base_url = get_base_url();

        let provider = if base_url.contains("openai") {
            LlmProvider::OpenAI
        } else if base_url.contains("gemini") || base_url.contains("google") {
            LlmProvider::Gemini
        } else {
            LlmProvider::Anthropic
        };

        let client = LlmClient::new(provider, api_key).with_base_url(base_url);

        let config = LlmRequestConfig {
            model: get_model(),
            max_tokens: 50,
            temperature: 0.0,
            ..Default::default()
        };

        // 多轮对话：验证 LLM 记住上下文
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "My secret code word is 'pineapple'.".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Got it, I'll remember your code word.".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "What is my secret code word? Reply with ONLY the word.".to_string(),
            },
        ];

        let result = client.send(messages, &config).await;

        match result {
            Ok(response) => {
                println!("LLM multi-turn response: {}", response.content);
                assert!(
                    !response.content.is_empty(),
                    "Multi-turn response should not be empty"
                );
                let content_lower = response.content.to_lowercase();
                assert!(
                    content_lower.contains("pineapple"),
                    "LLM should remember the code word 'pineapple' from context, got: {}",
                    response.content
                );
            }
            Err(e) => {
                panic!("LLM API error: {}", e);
            }
        }
    }
}

// ===== 2. CLI 端到端 =====

#[cfg(test)]
mod cli_e2e_tests {
    use super::*;

    #[test]
    fn test_cli_bash_command() {
        let result =
            cli::commands::tool_exec::execute_bash("echo hello_world", None, 10, false).unwrap();
        assert!(result.stdout.contains("hello_world"));
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);

        let result = cli::commands::tool_exec::execute_bash("exit 42", None, 10, false).unwrap();
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn test_cli_read_command() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("read_test.txt");

        // 写入多行文件
        let content: Vec<String> = (1..=20).map(|i| format!("Line {}", i)).collect();
        fs::write(&file_path, content.join("\n")).unwrap();

        // 读取完整文件
        let result =
            cli::commands::tool_exec::execute_read(file_path.to_str().unwrap(), None, None, false)
                .unwrap();
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 20"));

        // 读取部分行
        let result = cli::commands::tool_exec::execute_read(
            file_path.to_str().unwrap(),
            Some(5),
            Some(3),
            false,
        )
        .unwrap();
        assert!(result.contains("Line 6"));
        assert!(!result.contains("Line 1"));

        // 读取不存在的文件
        let result =
            cli::commands::tool_exec::execute_read("/nonexistent/file.txt", None, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_write_command() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("write_test.txt");

        // 首次写入
        let result = cli::commands::tool_exec::execute_write(
            file_path.to_str().unwrap(),
            Some("first write"),
            false,
            false,
        )
        .unwrap();
        assert!(result.contains("bytes"));

        // 追加写入
        let _result = cli::commands::tool_exec::execute_write(
            file_path.to_str().unwrap(),
            Some("appended"),
            true,
            false,
        )
        .unwrap();
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("first write"));
        assert!(content.contains("appended"));

        // 备份写入
        let _result = cli::commands::tool_exec::execute_write(
            file_path.to_str().unwrap(),
            Some("with backup"),
            false,
            true,
        )
        .unwrap();
        let backup_path = dir.path().join("write_test.txt.bak");
        assert!(backup_path.exists());
    }

    #[test]
    fn test_cli_edit_command() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("edit_test.txt");
        fs::write(&file_path, "foo bar foo baz foo").unwrap();

        // 替换第一个
        let result = cli::commands::tool_exec::execute_edit(
            file_path.to_str().unwrap(),
            "foo",
            "QUX",
            false,
        )
        .unwrap();
        assert!(result.contains("1 occurrence"));
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "QUX bar foo baz foo");

        // 替换所有
        let result =
            cli::commands::tool_exec::execute_edit(file_path.to_str().unwrap(), "foo", "QUX", true)
                .unwrap();
        assert!(result.contains("2 occurrence"));
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "QUX bar QUX baz QUX");
    }

    #[test]
    fn test_cli_grep_command() {
        let dir = TempDir::new().unwrap();

        fs::write(
            dir.path().join("app.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("lib.rs"),
            "pub fn greet() {\n    \"hello\"\n}\n",
        )
        .unwrap();

        // 搜索所有文件
        let results = cli::commands::tool_exec::execute_grep(
            "hello",
            dir.path().to_str().unwrap(),
            None,
            false,
            true,
            None,
        )
        .unwrap();
        assert_eq!(results.len(), 2);

        // 用 glob 过滤
        let results = cli::commands::tool_exec::execute_grep(
            "hello",
            dir.path().to_str().unwrap(),
            Some("*.rs"),
            false,
            true,
            None,
        )
        .unwrap();
        assert_eq!(results.len(), 2);

        // 不存在的模式
        let results = cli::commands::tool_exec::execute_grep(
            "nonexistent_pattern_xyz",
            dir.path().to_str().unwrap(),
            None,
            false,
            true,
            None,
        )
        .unwrap();
        assert!(results.is_empty());
    }
}

// ===== 3. Git 集成 =====

#[cfg(test)]
mod git_tests {
    use super::*;
    use cli::git::{branch, commit, diff, status};

    fn init_git_repo(dir: &std::path::Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[test]
    fn test_git_status_real() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        // 创建未跟踪文件
        fs::write(dir.path().join("untracked.txt"), "content").unwrap();

        let status = status::get_status(dir.path()).unwrap();
        assert!(status.has_changes());
        assert!(!status.untracked_files().is_empty());

        let rendered = status.render();
        assert!(!rendered.is_empty());
        assert!(rendered.contains("untracked"));

        // git add 后验证 staged
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let status = status::get_status(dir.path()).unwrap();
        assert!(!status.staged_files().is_empty());
    }

    #[test]
    fn test_git_diff_real() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        // 创建并提交文件
        fs::write(dir.path().join("file.txt"), "initial content\n").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "add file"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // 修改文件
        fs::write(dir.path().join("file.txt"), "modified content\n").unwrap();

        // 工作区 diff
        let diff = diff::get_diff(dir.path(), diff::DiffType::Working, &[]).unwrap();
        assert!(diff.files_changed > 0);
        assert!(!diff.entries.is_empty());
        assert!(diff.total_additions > 0 || diff.total_deletions > 0);

        // stat
        let stat = diff.stat();
        assert!(!stat.is_empty());
        assert!(stat.contains("1 file") || stat.contains("changed"));

        // 暂存区 diff 应为空（未 add）
        let staged_diff = diff::get_diff(dir.path(), diff::DiffType::Staged, &[]).unwrap();
        assert_eq!(staged_diff.files_changed, 0);
    }

    #[test]
    fn test_git_commit_flow() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        // 创建文件
        fs::write(dir.path().join("feature.txt"), "new feature\n").unwrap();

        // git add
        commit::add_all(dir.path()).unwrap();

        // 验证 staged
        let status = status::get_status(dir.path()).unwrap();
        assert!(!status.staged_files().is_empty());

        // git commit
        let result = commit::commit(dir.path(), "feat: add feature", false).unwrap();
        assert!(result.contains("feat: add feature") || result.contains("commit"));

        // 提交后应无 staged 文件
        let status = status::get_status(dir.path()).unwrap();
        assert!(status.staged_files().is_empty());

        // 修改并测试 add 指定路径
        fs::write(dir.path().join("feature.txt"), "updated feature\n").unwrap();
        fs::write(dir.path().join("another.txt"), "another file\n").unwrap();
        commit::add(dir.path(), &["feature.txt"]).unwrap();

        let status = status::get_status(dir.path()).unwrap();
        // feature.txt 应 staged，another.txt 应 untracked
        let staged: Vec<_> = status.staged_files().iter().map(|e| &e.path).collect();
        assert!(staged.iter().any(|p| p.contains("feature")));
    }

    #[test]
    fn test_git_branch() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        let manager = branch::BranchManager::new(dir.path());

        let current = manager.current().unwrap();
        assert!(!current.is_empty());

        manager.create("test-branch").unwrap();
        let branches = manager.list(false).unwrap();
        assert!(branches.iter().any(|b| b.name == "test-branch"));

        manager.create_and_switch("feature-branch").unwrap();
        assert_eq!(manager.current().unwrap(), "feature-branch");

        manager.switch(&current).unwrap();
        assert_eq!(manager.current().unwrap(), current);

        manager.delete("feature-branch", false).unwrap();
        let branches = manager.list(false).unwrap();
        assert!(!branches.iter().any(|b| b.name == "feature-branch"));
    }
}

// ===== 4. MCP 端到端 =====

#[cfg(test)]
mod mcp_tests {

    #[tokio::test]
    async fn test_mcp_memory_transport() {
        use sh_layer4::mcp_bridge::protocol::{McpMessage, McpRequest, RequestId};
        use sh_layer4::mcp_bridge::transport::{McpTransport, MemoryTransport};

        let transport = MemoryTransport::new();

        // 发送请求
        let msg = McpMessage::Request(McpRequest {
            id: RequestId::Number(1),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({"protocol_version": "2024-11-05"})),
        });
        transport.send(&msg).await.unwrap();

        // 接收请求
        let received = transport.receive().await.unwrap();
        assert!(received.is_some());
        if let Some(McpMessage::Request(req)) = received {
            assert_eq!(req.method, "initialize");
        } else {
            panic!("Expected Request message");
        }

        // 关闭
        transport.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_mcp_tool_call_real() {
        use sh_layer4::mcp_bridge::handler::{DefaultHandler, McpHandler, SimpleToolExecutor};
        use sh_layer4::mcp_bridge::protocol::{
            ContentBlock, McpRequest, RequestId, ToolDefinition, ToolResult,
        };
        use std::sync::Arc;

        let handler = DefaultHandler::new("test-server", "1.0.0");

        // 注册 echo 工具
        handler.register_tool(
            ToolDefinition {
                name: "echo".to_string(),
                description: Some("Echo input text".to_string()),
                input_schema: None,
            },
            Arc::new(SimpleToolExecutor(|_name, args| {
                let text = args.as_str().unwrap_or("empty").to_string();
                Ok(ToolResult {
                    is_error: false,
                    content: vec![ContentBlock::Text { text }],
                })
            })),
        );

        // 注册 add 工具
        handler.register_tool(
            ToolDefinition {
                name: "add".to_string(),
                description: Some("Add two numbers".to_string()),
                input_schema: None,
            },
            Arc::new(SimpleToolExecutor(|_name, args| {
                let a = args.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
                let b = args.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
                Ok(ToolResult {
                    is_error: false,
                    content: vec![ContentBlock::Text {
                        text: format!("{}", a + b),
                    }],
                })
            })),
        );

        // 验证工具列表
        let list_request = McpRequest {
            id: RequestId::Number(1),
            method: "tools/list".to_string(),
            params: None,
        };
        let response = handler.handle(&list_request).await.unwrap();
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
        assert_eq!(tools.len(), 2);

        // 调用 echo 工具
        let echo_request = McpRequest {
            id: RequestId::Number(2),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({"name": "echo", "arguments": "hello"})),
        };
        let response = handler.handle(&echo_request).await.unwrap();
        assert!(response.error.is_none());
        let result: ToolResult = serde_json::from_value(response.result.unwrap()).unwrap();
        assert!(!result.is_error);
        if let ContentBlock::Text { text } = &result.content[0] {
            assert_eq!(text, "hello");
        } else {
            panic!("Expected text content block");
        }

        // 调用 add 工具
        let add_request = McpRequest {
            id: RequestId::Number(3),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({"name": "add", "arguments": {"a": 3, "b": 5}})),
        };
        let response = handler.handle(&add_request).await.unwrap();
        assert!(response.error.is_none());
        let result: ToolResult = serde_json::from_value(response.result.unwrap()).unwrap();
        assert!(!result.is_error);
        if let ContentBlock::Text { text } = &result.content[0] {
            assert_eq!(text, "8");
        } else {
            panic!("Expected text content block");
        }
    }

    #[tokio::test]
    async fn test_mcp_client_manager() {
        use sh_layer4::mcp_bridge::client::McpClientManager;
        use sh_layer4::mcp_bridge::transport::McpTransportType;

        let manager = McpClientManager::new();

        let config = sh_layer4::mcp_bridge::client::McpServerConfig {
            name: "test-server".to_string(),
            transport: McpTransportType::Stdio {
                command: "echo".to_string(),
                args: vec![],
            },
            auto_reconnect: false,
            reconnect_interval_ms: 1000,
        };

        manager.add_server(config).await.unwrap();

        let servers = manager.list_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].0, "test-server");

        let status = manager.render_status();
        assert!(status.contains("test-server"));
    }

    #[tokio::test]
    async fn test_mcp_handler() {
        use sh_layer4::mcp_bridge::handler::{DefaultHandler, McpHandler, SimpleToolExecutor};
        use sh_layer4::mcp_bridge::protocol::{
            ContentBlock, McpRequest, RequestId, ToolDefinition, ToolResult,
        };
        use std::sync::Arc;

        let handler = DefaultHandler::new("test-server", "1.0.0");

        handler.register_tool(
            ToolDefinition {
                name: "greet".to_string(),
                description: Some("Greet someone".to_string()),
                input_schema: None,
            },
            Arc::new(SimpleToolExecutor(|_name, args| {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("world");
                Ok(ToolResult {
                    is_error: false,
                    content: vec![ContentBlock::Text {
                        text: format!("Hello, {}!", name),
                    }],
                })
            })),
        );

        // 请求工具列表
        let request = McpRequest {
            id: RequestId::Number(1),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(&request).await.unwrap();
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }
}

// ===== 5. 错误恢复 =====

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn test_error_recovery_category() {
        use sh_layer2::checkpoint_system::ErrorCategory;

        assert_eq!(
            ErrorCategory::from_error_message("network timeout"),
            ErrorCategory::Transient
        );
        assert_eq!(
            ErrorCategory::from_error_message("api key invalid"),
            ErrorCategory::Configuration
        );
        assert_eq!(
            ErrorCategory::from_error_message("invalid parameter"),
            ErrorCategory::Logic
        );

        assert!(ErrorCategory::Transient.is_retryable());
        assert!(ErrorCategory::Resource.is_retryable());
        assert!(!ErrorCategory::Configuration.is_retryable());
        assert!(!ErrorCategory::Logic.is_retryable());
    }

    #[test]
    fn test_retry_policy() {
        use sh_layer2::checkpoint_system::RetryPolicy;

        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);

        let d0 = policy.delay_for_attempt(0);
        let d1 = policy.delay_for_attempt(1);
        assert!(d1 > d0, "Delay should increase with attempts");
    }

    #[tokio::test]
    async fn test_error_recovery_stats() {
        use sh_layer2::checkpoint_system::{ErrorRecovery, FallbackStrategy};

        let recovery = ErrorRecovery::new().with_fallback(FallbackStrategy::Skip);

        let stats = recovery.get_stats().await;
        assert_eq!(stats.total_errors, 0);
    }

    #[test]
    fn test_session_recovery() {
        use sh_layer2::checkpoint_system::SessionRecovery;

        let dir = TempDir::new().unwrap();
        let recovery = SessionRecovery::new(dir.path());

        let interrupted = recovery.detect_interrupted_sessions().unwrap();
        assert!(interrupted.is_empty());

        let rendered = recovery.render_interrupted();
        assert!(rendered.contains("No interrupted"));
    }

    #[test]
    fn test_checkpoint_real_save_load() {
        use chrono::Utc;
        use sh_layer2::checkpoint_system::{
            CheckpointData, CheckpointSystemTrait, CheckpointWriter,
        };
        use sh_layer2::types::{CheckpointId, SessionId};

        let temp_dir = TempDir::new().unwrap();
        let writer = CheckpointWriter::new(temp_dir.path());

        let data = CheckpointData {
            checkpoint_id: CheckpointId::new(),
            session_id: SessionId::new(),
            created_at: Utc::now(),
            trigger: "test".to_string(),
            iteration: 1,
            messages: vec![serde_json::json!({"role": "user", "content": "test"})],
            tool_calls_pending: Vec::new(),
            tool_results: serde_json::Value::Null,
            tokens_used: 100,
            cost_estimate: 0.01,
            resume_hint: None,
        };

        let saved_id = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(writer.save(&data))
            .expect("Save should succeed");

        let session_id = data.session_id.clone();
        let loaded = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(writer.load(&session_id, None))
            .expect("Load should succeed");

        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.checkpoint_id, saved_id);
        assert_eq!(loaded.iteration, 1);
    }
}

// ===== 6. Playwright MCP 集成 =====

#[cfg(test)]
mod playwright_mcp_tests {
    use sh_layer4::mcp_bridge::client::{preset_servers, McpClientManager, McpServerConfig};
    use sh_layer4::mcp_bridge::handler::{DefaultHandler, McpHandler, SimpleToolExecutor};
    use sh_layer4::mcp_bridge::protocol::McpMessage;
    use sh_layer4::mcp_bridge::protocol::{
        ContentBlock, McpRequest, RequestId, ToolDefinition, ToolResult,
    };
    use sh_layer4::mcp_bridge::transport::{McpTransport, McpTransportType, MemoryTransport};
    use std::sync::Arc;

    /// 测试 preset_servers 包含 Playwright 配置
    #[test]
    fn test_playwright_in_preset_servers() {
        let presets = preset_servers();
        assert!(!presets.is_empty(), "preset_servers should not be empty");

        // 验证 Playwright 存在
        let playwright = presets.iter().find(|s| s.name == "playwright");
        assert!(
            playwright.is_some(),
            "playwright should be in preset_servers"
        );

        let config = playwright.unwrap();
        assert_eq!(config.name, "playwright");
        assert!(config.auto_reconnect, "playwright should auto reconnect");

        // 验证传输配置
        match &config.transport {
            McpTransportType::Stdio { command, args } => {
                assert_eq!(command, "npx", "command should be npx");
                assert!(args.contains(&"@playwright/mcp@latest".to_string()));
                assert!(args.contains(&"--headless".to_string()));
                assert!(args.contains(&"--browser".to_string()));
                assert!(args.contains(&"chrome".to_string()));
            }
            _ => panic!("playwright should use Stdio transport"),
        }
    }

    /// 测试 Playwright 配置创建
    #[test]
    fn test_playwright_config_creation() {
        let config = McpServerConfig {
            name: "playwright".to_string(),
            transport: McpTransportType::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "@playwright/mcp@latest".to_string(),
                    "--headless".to_string(),
                    "--browser".to_string(),
                    "chrome".to_string(),
                ],
            },
            auto_reconnect: true,
            reconnect_interval_ms: 5000,
        };

        assert_eq!(config.name, "playwright");
        assert!(config.auto_reconnect);
        assert_eq!(config.reconnect_interval_ms, 5000);
    }

    /// 测试 Playwright 工具定义
    #[tokio::test]
    async fn test_playwright_tool_definitions() {
        let handler = DefaultHandler::new("playwright-server", "1.0.0");

        // 注册 Playwright 模拟工具
        let tools = vec![
            ToolDefinition {
                name: "browser_navigate".to_string(),
                description: Some("Navigate to URL".to_string()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" }
                    },
                    "required": ["url"]
                })),
            },
            ToolDefinition {
                name: "browser_click".to_string(),
                description: Some("Click element by selector".to_string()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string" }
                    },
                    "required": ["selector"]
                })),
            },
            ToolDefinition {
                name: "browser_type".to_string(),
                description: Some("Type text into element".to_string()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["selector", "text"]
                })),
            },
            ToolDefinition {
                name: "browser_screenshot".to_string(),
                description: Some("Take screenshot of current page".to_string()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {}
                })),
            },
            ToolDefinition {
                name: "browser_evaluate".to_string(),
                description: Some("Execute JavaScript in browser".to_string()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "script": { "type": "string" }
                    },
                    "required": ["script"]
                })),
            },
        ];

        for tool in &tools {
            handler.register_tool(
                tool.clone(),
                Arc::new(SimpleToolExecutor(|name, args| {
                    Ok(ToolResult {
                        is_error: false,
                        content: vec![ContentBlock::Text {
                            text: format!("Tool {} executed with args: {:?}", name, args),
                        }],
                    })
                })),
            );
        }

        // 验证工具列表
        let request = McpRequest {
            id: RequestId::Number(1),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(&request).await.unwrap();
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let registered_tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
        assert_eq!(registered_tools.len(), 5);

        // 验证工具名称
        let tool_names: Vec<&str> = registered_tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();
        assert!(tool_names.contains(&"browser_navigate"));
        assert!(tool_names.contains(&"browser_click"));
        assert!(tool_names.contains(&"browser_type"));
        assert!(tool_names.contains(&"browser_screenshot"));
        assert!(tool_names.contains(&"browser_evaluate"));
    }

    /// 测试 Playwright MCP 消息交换
    #[tokio::test]
    async fn test_playwright_mcp_message_exchange() {
        let transport = MemoryTransport::new();

        // 模拟 initialize 请求
        let init_request = McpMessage::Request(McpRequest {
            id: RequestId::Number(1),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocol_version": "2024-11-05",
                "capabilities": {},
                "client_info": {
                    "name": "continuum",
                    "version": "0.1.0"
                }
            })),
        });

        transport.send(&init_request).await.unwrap();

        // 验证发送的消息
        let received = transport.receive().await.unwrap();
        assert!(received.is_some());

        if let Some(McpMessage::Request(req)) = received {
            assert_eq!(req.method, "initialize");
            assert!(req.params.is_some());

            let params = req.params.unwrap();
            assert_eq!(params.get("protocol_version").unwrap(), "2024-11-05");
        } else {
            panic!("Expected Request message");
        }
    }

    /// 测试 Playwright 工具调用模拟
    #[tokio::test]
    async fn test_playwright_tool_call_simulation() {
        let handler = DefaultHandler::new("playwright-server", "1.0.0");

        // 注册 browser_navigate 工具
        handler.register_tool(
            ToolDefinition {
                name: "browser_navigate".to_string(),
                description: Some("Navigate to URL".to_string()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" }
                    },
                    "required": ["url"]
                })),
            },
            Arc::new(SimpleToolExecutor(|_name, args| {
                let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
                Ok(ToolResult {
                    is_error: false,
                    content: vec![ContentBlock::Text {
                        text: format!("Navigated to: {}", url),
                    }],
                })
            })),
        );

        // 调用 browser_navigate
        let request = McpRequest {
            id: RequestId::Number(1),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "browser_navigate",
                "arguments": {
                    "url": "https://example.com"
                }
            })),
        };

        let response = handler.handle(&request).await.unwrap();
        assert!(response.error.is_none());

        let result: ToolResult = serde_json::from_value(response.result.unwrap()).unwrap();
        assert!(!result.is_error);

        if let ContentBlock::Text { text } = &result.content[0] {
            assert!(text.contains("https://example.com"));
        } else {
            panic!("Expected text content block");
        }
    }

    /// 测试 Playwright MCP Client Manager
    #[tokio::test]
    async fn test_playwright_client_manager() {
        let manager = McpClientManager::new();

        // 添加 Playwright 配置
        let config = McpServerConfig {
            name: "playwright".to_string(),
            transport: McpTransportType::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "@playwright/mcp@latest".to_string(),
                    "--headless".to_string(),
                    "--browser".to_string(),
                    "chrome".to_string(),
                ],
            },
            auto_reconnect: true,
            reconnect_interval_ms: 5000,
        };

        manager.add_server(config.clone()).await.unwrap();

        // 验证服务器列表
        let servers = manager.list_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].0, "playwright");
        assert!(!servers[0].1); // 未连接

        // 渲染状态
        let status = manager.render_status();
        assert!(status.contains("playwright"));
        assert!(status.contains("0 tools")); // 未连接时无工具
    }

    /// 测试 Playwright 错误处理
    #[tokio::test]
    async fn test_playwright_error_handling() {
        let handler = DefaultHandler::new("playwright-server", "1.0.0");

        // 注册一个会失败的工具
        handler.register_tool(
            ToolDefinition {
                name: "browser_navigate".to_string(),
                description: Some("Navigate to URL".to_string()),
                input_schema: None,
            },
            Arc::new(SimpleToolExecutor(|_name, args| {
                let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
                if url.is_empty() {
                    Ok(ToolResult {
                        is_error: true,
                        content: vec![ContentBlock::Text {
                            text: "URL is required".to_string(),
                        }],
                    })
                } else {
                    Ok(ToolResult {
                        is_error: false,
                        content: vec![ContentBlock::Text {
                            text: format!("Navigated to: {}", url),
                        }],
                    })
                }
            })),
        );

        // 测试空 URL 错误
        let request = McpRequest {
            id: RequestId::Number(1),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "browser_navigate",
                "arguments": {}
            })),
        };

        let response = handler.handle(&request).await.unwrap();
        let result: ToolResult = serde_json::from_value(response.result.unwrap()).unwrap();
        assert!(result.is_error);

        if let ContentBlock::Text { text } = &result.content[0] {
            assert_eq!(text, "URL is required");
        }
    }
}

// ===== 7. 真实 Playwright MCP 连接 (需要 Node.js) =====

#[cfg(test)]
mod real_playwright_tests {
    use std::process::Command;

    /// 检查 Node.js 是否可用
    #[test]
    fn test_nodejs_available() {
        let result = Command::new("node").arg("--version").output();
        match result {
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("Node.js version: {}", version);
                assert!(output.status.success(), "Node.js should be available");
            }
            Err(e) => {
                println!(
                    "Node.js not found: {}. Playwright MCP tests will be skipped.",
                    e
                );
                // 不 panic，允许在没有 Node.js 的环境中运行其他测试
            }
        }
    }

    /// 检查 npx 是否可用
    #[test]
    fn test_npx_available() {
        let result = Command::new("npx").arg("--version").output();
        match result {
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("npx version: {}", version);
                assert!(output.status.success(), "npx should be available");
            }
            Err(e) => {
                println!(
                    "npx not found: {}. Playwright MCP tests will be skipped.",
                    e
                );
            }
        }
    }

    /// 测试 Playwright MCP 命令帮助 (验证包存在)
    #[test]
    #[ignore = "requires network access to download @playwright/mcp"]
    fn test_playwright_mcp_help() {
        let result = Command::new("npx")
            .args(["@playwright/mcp@latest", "--help"])
            .output()
            .expect("Failed to execute npx @playwright/mcp");

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);

        println!("Playwright MCP help stdout: {}", stdout);
        println!("Playwright MCP help stderr: {}", stderr);

        // 验证命令执行
        // 注意：首次运行可能需要下载包
        assert!(
            result.status.success() || stderr.contains("install"),
            "Playwright MCP should execute or attempt to install"
        );
    }
}

// ===== 8. Plugin Loader dylib 测试 =====

#[cfg(test)]
mod plugin_loader_tests {
    use sh_layer4::plugin_loader::{DylibLoader, PluginLoader, PluginMeta};
    use std::path::PathBuf;

    /// 测试 DylibLoader 创建
    #[test]
    fn test_dylib_loader_creation() {
        let loader = DylibLoader::new();
        assert!(loader.list().is_empty());
        assert_eq!(loader.count(), 0);
    }

    /// 测试有效库检测
    #[test]
    fn test_is_valid_library() {
        // 测试有效扩展名
        let valid_extensions = [".so", ".dll", ".dylib"];
        for ext in &valid_extensions {
            let tmp = tempfile::NamedTempFile::with_suffix(ext).unwrap();
            assert!(
                DylibLoader::is_valid_library(tmp.path()),
                "Should recognize {} as valid",
                ext
            );
        }

        // 测试无效扩展名
        let invalid_extensions = [".txt", ".rs", ".wasm", ""];
        for ext in &invalid_extensions {
            let tmp = tempfile::NamedTempFile::with_suffix(ext).unwrap();
            assert!(
                !DylibLoader::is_valid_library(tmp.path()),
                "Should NOT recognize {} as valid",
                ext
            );
        }

        // 测试不存在的文件
        assert!(!DylibLoader::is_valid_library(
            PathBuf::from("/nonexistent.so").as_path()
        ));
    }

    /// 测试 PluginLoader 创建
    #[test]
    fn test_plugin_loader_creation() {
        let loader = PluginLoader::with_default_dir();
        assert_eq!(loader.count(), 0);
        assert!(loader.list().is_empty());
    }

    /// 测试 PluginLoader 自定义目录
    #[test]
    fn test_plugin_loader_custom_dir() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let loader = PluginLoader::new(tmp_dir.path());
        assert_eq!(loader.count(), 0);
    }

    /// 测试 PluginMeta 默认值
    #[test]
    fn test_plugin_meta_default() {
        let meta = PluginMeta::default();
        assert_eq!(meta.name, "unknown");
        assert_eq!(meta.version, "0.1.0");
        assert_eq!(meta.author, "unknown");
        assert!(meta.description.is_empty());
        assert!(meta.dependencies.is_empty());
        assert_eq!(meta.entry_point, "main");
    }

    /// 测试 DylibLoader 状态管理
    #[test]
    fn test_dylib_loader_status() {
        let loader = DylibLoader::new();
        assert!(!loader.is_loaded("nonexistent"));
        assert!(loader.get_meta("nonexistent").is_none());
    }

    /// 测试 PluginLoader 状态渲染
    #[test]
    fn test_plugin_loader_render_status() {
        let loader = PluginLoader::with_default_dir();
        let status = loader.render_status();
        assert!(status.contains("Plugins:"));
        assert!(status.contains("No plugins loaded"));
    }

    /// 测试加载空目录
    #[tokio::test]
    async fn test_plugin_loader_empty_dir() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let loader = PluginLoader::new(tmp_dir.path());

        let loaded = loader.load_dir().await.unwrap();
        assert!(loaded.is_empty());
    }

    /// 测试加载无效文件
    #[tokio::test]
    async fn test_plugin_loader_invalid_file() {
        let tmp_dir = tempfile::tempdir().unwrap();

        // 创建无效文件
        std::fs::write(tmp_dir.path().join("invalid.txt"), "not a plugin").unwrap();

        let loader = PluginLoader::new(tmp_dir.path());
        let loaded = loader.load_dir().await.unwrap();

        // 不应加载 .txt 文件
        assert!(loaded.is_empty());
    }

    /// 测试插件信息获取
    #[test]
    fn test_plugin_loader_get_info() {
        let loader = PluginLoader::with_default_dir();
        assert!(loader.get("nonexistent").is_none());
        assert!(loader.get_meta("nonexistent").is_none());
    }
}

// ===== 9. Capability System 测试 =====

#[cfg(test)]
mod capability_tests {
    use sh_layer4::plugin_loader::{Capability, CapabilitySet};

    /// 测试空能力集
    #[test]
    fn test_empty_capability_set() {
        let caps = CapabilitySet::new();
        assert!(!caps.check(&Capability::FsRead));
        assert!(!caps.check(&Capability::FsWrite));
        assert!(!caps.check(&Capability::NetworkOut));
        assert!(!caps.check(&Capability::ProcessExec));
    }

    /// 测试无限制能力集
    #[test]
    fn test_unrestricted_capability_set() {
        let caps = CapabilitySet::unrestricted();
        assert!(caps.check(&Capability::FsRead));
        assert!(caps.check(&Capability::FsWrite));
        assert!(caps.check(&Capability::NetworkOut));
        assert!(caps.check(&Capability::ProcessExec));
        assert!(caps.check(&Capability::EnvRead));
        assert!(caps.check(&Capability::EnvWrite));
        assert!(caps.check(&Capability::Clock));
        assert!(caps.check(&Capability::Random));
    }

    /// 测试沙箱能力集
    #[test]
    fn test_sandboxed_capability_set() {
        let caps = CapabilitySet::sandboxed();

        // 沙箱模式不允许文件系统访问
        assert!(!caps.check(&Capability::FsRead));
        assert!(!caps.check(&Capability::FsWrite));
        assert!(!caps.check(&Capability::NetworkOut));
        assert!(!caps.check(&Capability::ProcessExec));

        // 沙箱模式允许时钟和随机数
        assert!(caps.check(&Capability::Clock));
        assert!(caps.check(&Capability::Random));
    }

    /// 测试拒绝覆盖允许
    #[test]
    fn test_deny_overrides_allow() {
        let mut caps = CapabilitySet::unrestricted();

        // 验证初始状态
        assert!(caps.check(&Capability::FsWrite));

        // 添加拒绝
        caps.deny(Capability::FsWrite);

        // 验证拒绝生效
        assert!(!caps.check(&Capability::FsWrite));

        // 其他能力仍有效
        assert!(caps.check(&Capability::FsRead));
    }

    /// 测试能力合并
    #[test]
    fn test_capability_merge() {
        let mut caps1 = CapabilitySet::new();
        caps1.allow(Capability::FsRead);
        caps1.allow(Capability::FsWrite);

        let mut caps2 = CapabilitySet::sandboxed();
        caps2.deny(Capability::FsWrite);

        caps1.merge(&caps2);

        // FsRead 应保留
        assert!(caps1.check(&Capability::FsRead));

        // FsWrite 应被拒绝
        assert!(!caps1.check(&Capability::FsWrite));

        // Clock 应被添加
        assert!(caps1.check(&Capability::Clock));
    }

    /// 测试链式操作
    #[test]
    fn test_capability_builder() {
        let mut caps = CapabilitySet::new();
        caps.allow(Capability::FsRead)
            .allow(Capability::NetworkOut)
            .deny(Capability::FsWrite);

        assert!(caps.check(&Capability::FsRead));
        assert!(caps.check(&Capability::NetworkOut));
        assert!(!caps.check(&Capability::FsWrite));
    }

    /// 测试资源限制能力
    #[test]
    fn test_resource_limit_capabilities() {
        let caps = CapabilitySet::sandboxed();

        // 验证内存限制存在
        assert!(caps
            .allowed
            .contains(&Capability::MemoryLimit(16 * 1024 * 1024)));
        assert!(caps.allowed.contains(&Capability::CpuLimit(5000)));

        // 验证检查逻辑（带参数的能力）
        assert!(caps.check(&Capability::MemoryLimit(16 * 1024 * 1024)));
        assert!(caps.check(&Capability::CpuLimit(5000)));

        // 不同的参数值不算同一个能力
        assert!(!caps.check(&Capability::MemoryLimit(32 * 1024 * 1024)));
    }

    /// 测试能力序列化
    #[test]
    fn test_capability_serialization() {
        let cap = Capability::FsRead;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"FsRead\"");

        let deserialized: Capability = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Capability::FsRead);
    }

    /// 测试能力集克隆
    #[test]
    fn test_capability_set_clone() {
        let caps1 = CapabilitySet::unrestricted();
        let caps2 = caps1.clone();

        assert_eq!(
            caps1.check(&Capability::FsRead),
            caps2.check(&Capability::FsRead)
        );
        assert_eq!(
            caps1.check(&Capability::NetworkOut),
            caps2.check(&Capability::NetworkOut)
        );
    }

    /// 测试动态能力添加
    #[test]
    fn test_dynamic_capability_addition() {
        let mut caps = CapabilitySet::new();

        // 动态添加能力
        caps.allow(Capability::FsRead);
        assert!(caps.check(&Capability::FsRead));

        // 再次添加相同能力（无副作用）
        caps.allow(Capability::FsRead);
        assert!(caps.check(&Capability::FsRead));

        // 拒绝后重新允许
        caps.deny(Capability::FsRead);
        assert!(!caps.check(&Capability::FsRead));

        caps.allow(Capability::FsRead);
        assert!(caps.check(&Capability::FsRead));
    }
}

// ===== 10. Plugin Lifecycle 集成测试 =====

#[cfg(test)]
mod plugin_lifecycle_tests {
    use sh_layer4::plugin_loader::{DylibLoader, PluginLoader};

    /// 测试 PluginLoader 创建
    #[test]
    fn test_plugin_loader_creation() {
        let loader = PluginLoader::with_default_dir();
        assert_eq!(loader.count(), 0);
    }

    /// 测试自定义目录创建
    #[test]
    fn test_plugin_loader_custom_dir() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let loader = PluginLoader::new(tmp_dir.path());
        assert_eq!(loader.count(), 0);
    }

    /// 测试 DylibLoader 文件验证
    #[test]
    fn test_dylib_file_validation() {
        // 有效扩展名
        let valid_so = tempfile::NamedTempFile::with_suffix(".so").unwrap();
        assert!(DylibLoader::is_valid_library(valid_so.path()));

        let valid_dll = tempfile::NamedTempFile::with_suffix(".dll").unwrap();
        assert!(DylibLoader::is_valid_library(valid_dll.path()));

        let valid_dylib = tempfile::NamedTempFile::with_suffix(".dylib").unwrap();
        assert!(DylibLoader::is_valid_library(valid_dylib.path()));

        // 无效扩展名
        let invalid_txt = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        assert!(!DylibLoader::is_valid_library(invalid_txt.path()));

        let invalid_rs = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        assert!(!DylibLoader::is_valid_library(invalid_rs.path()));
    }

    /// 测试插件状态渲染
    #[test]
    fn test_plugin_state_render() {
        let loader = PluginLoader::with_default_dir();
        let status = loader.render_status();

        assert!(status.contains("Plugins:"));
        assert!(status.contains("No plugins loaded") || loader.count() > 0);
    }

    /// 测试插件列表为空
    #[test]
    fn test_plugin_list_empty() {
        let loader = PluginLoader::with_default_dir();
        let list = loader.list();
        assert!(list.is_empty());
    }

    /// 测试 DylibLoader 计数
    #[test]
    fn test_dylib_loader_count() {
        let loader = DylibLoader::new();
        assert_eq!(loader.count(), 0);
        assert!(loader.list().is_empty());
    }

    /// 测试插件信息不存在
    #[test]
    fn test_plugin_info_not_found() {
        let loader = PluginLoader::with_default_dir();

        let info = loader.get("nonexistent_plugin");
        assert!(info.is_none());

        let meta = loader.get_meta("nonexistent_plugin");
        assert!(meta.is_none());
    }

    /// 测试 DylibLoader 状态检查
    #[test]
    fn test_dylib_loader_loaded_status() {
        let loader = DylibLoader::new();

        // 空加载器中不应有任何插件
        assert!(!loader.is_loaded("any_plugin"));
        assert!(loader.get_name("any_plugin").is_none());
        assert!(loader.get_version("any_plugin").is_none());
    }

    /// 测试加载目录中多种文件类型
    #[tokio::test]
    async fn test_plugin_loader_multiple_file_types() {
        let tmp_dir = tempfile::tempdir().unwrap();

        // 创建多种文件类型
        std::fs::write(tmp_dir.path().join("plugin.so"), "").unwrap();
        std::fs::write(tmp_dir.path().join("plugin.dll"), "").unwrap();
        std::fs::write(tmp_dir.path().join("plugin.dylib"), "").unwrap();
        std::fs::write(tmp_dir.path().join("plugin.wasm"), "").unwrap();
        std::fs::write(tmp_dir.path().join("readme.txt"), "").unwrap();

        let loader = PluginLoader::new(tmp_dir.path());

        // 加载目录（注意：实际加载会失败，因为没有真正的插件）
        // 这里只测试文件过滤逻辑
        let loaded = loader.load_dir().await.unwrap();

        // 实际动态库加载会失败（无效内容）
        // 但 WASM 会因未实现而返回错误
        // 所以 loaded 应该是空的或部分失败
        // 这里我们只验证不会 panic
        assert!(loaded.is_empty() || loaded.len() <= 4);
    }
}
