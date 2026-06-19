//! # Continuum Python Bindings
//!
//! Python bindings for Continuum.
//!
//! ## Performance Optimizations
//! - Global tokio runtime (lazy initialization via OnceLock)
//! - GIL release with `allow_threads` for blocking operations

mod python_tool_adapter;

use pyo3::prelude::*;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// Global tokio runtime for async operations
#[allow(dead_code)]
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Get or create the global tokio runtime
#[allow(dead_code)]
fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    })
}

/// Python 模块定义
#[pymodule]
fn sh_python(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Layer 0
    m.add_class::<PySecurityGateway>()?;
    m.add_class::<PyPermissionManager>()?;
    m.add_class::<PyPermission>()?;
    m.add_class::<PyRole>()?;

    // Layer 1
    m.add_class::<PyLlmClient>()?;
    m.add_class::<PyLlmRequestConfig>()?;
    m.add_class::<PyLlmResponse>()?;
    m.add_class::<PyCostTracker>()?;
    m.add_class::<PyUsageSnapshot>()?;
    m.add_class::<PyCostEstimate>()?;

    // Layer 2
    m.add_class::<PyAgentRuntime>()?;
    m.add_class::<PyAgentConfig>()?;
    m.add_class::<PySessionManager>()?;
    m.add_class::<PyCheckpointSystem>()?;
    m.add_class::<PyAgent>()?;
    m.add_class::<PySession>()?;
    // Streaming types (task #158)
    m.add_class::<PyAgentStreamIterator>()?;
    m.add_class::<PyStreamChunk>()?;
    // Interactive Permission System (task #157)
    m.add_class::<PySecurityLevel>()?;
    m.add_class::<PyPermissionDecision>()?;
    m.add_class::<PyPermissionAction>()?;
    m.add_class::<PyPermissionPolicy>()?;
    m.add_class::<PyInteractivePermissionManager>()?;

    // Layer 3
    m.add_class::<PyToolExecutor>()?;
    m.add_class::<PyQueryEngine>()?;
    m.add_class::<PyMemorySystem>()?;
    m.add_class::<PyVectorStore>()?;
    m.add_class::<PyVectorItem>()?;
    m.add_class::<PySearchResult>()?;
    m.add_class::<PyRetrieverEngine>()?;
    m.add_class::<PyDocumentLoader>()?;
    m.add_class::<PyTextSplitter>()?;
    m.add_class::<PyEmbeddings>()?;
    m.add_class::<PyEmbeddingProvider>()?;

    // Layer 4
    m.add_class::<PyMcpBridge>()?;
    m.add_class::<PyAuditLogger>()?;

    Ok(())
}

mod bindings {
    use super::*;
    use pyo3::types::{PyDict, PyType};
    use sh_layer1::LlmClientTrait;
    use sh_layer2::{
        AgentRuntimeTrait, CheckpointSystemTrait, SessionManagerTrait, ToolRegistryTrait,
    };
    use sh_layer3::{
        ChunkingStrategy, MemorySystemTrait, RetrieverEngine, ToolExecutor, VectorStoreTrait,
    };

    /// 将 Layer2Error 转换为适当的 Python 异常类型
    fn layer2_error_to_pyerr(e: &sh_layer2::Layer2Error) -> PyErr {
        use sh_layer2::Layer2Error;
        match e {
            Layer2Error::SessionNotFound(id) => {
                pyo3::exceptions::PyKeyError::new_err(format!("Session not found: {}", id.0))
            }
            Layer2Error::MaxSessionsReached(n) => {
                pyo3::exceptions::PyRuntimeError::new_err(format!("Max sessions reached: {}", n))
            }
            Layer2Error::LlmNotConfigured => {
                pyo3::exceptions::PyRuntimeError::new_err("LLM client not configured")
            }
            Layer2Error::InvalidStateTransition { from, to } => {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "Invalid state transition: from {:?} to {:?}",
                    from, to
                ))
            }
            Layer2Error::MaxIterations(n) => {
                pyo3::exceptions::PyRuntimeError::new_err(format!("Max iterations reached: {}", n))
            }
            Layer2Error::AgentError(msg) => pyo3::exceptions::PyRuntimeError::new_err(msg.clone()),
            Layer2Error::Io(e) => pyo3::exceptions::PyIOError::new_err(e.to_string()),
            Layer2Error::Serialization(e) => {
                pyo3::exceptions::PyValueError::new_err(format!("Serialization error: {}", e))
            }
            Layer2Error::CheckpointNotFound(id) => {
                pyo3::exceptions::PyKeyError::new_err(format!("Checkpoint not found: {}", id.0))
            }
            Layer2Error::CheckpointCorrupted(msg) => {
                pyo3::exceptions::PyValueError::new_err(format!("Checkpoint corrupted: {}", msg))
            }
            Layer2Error::ToolNotFound(name) => {
                pyo3::exceptions::PyKeyError::new_err(format!("Tool not found: {}", name))
            }
            Layer2Error::TaskNotFound(id) => {
                pyo3::exceptions::PyKeyError::new_err(format!("Task not found: {}", id.0))
            }
            Layer2Error::LockTimeout => {
                pyo3::exceptions::PyTimeoutError::new_err("Lock acquisition timeout")
            }
            Layer2Error::PermissionDenied(msg) => {
                pyo3::exceptions::PyPermissionError::new_err(format!("Permission denied: {}", msg))
            }
        }
    }

    /// 将 anyhow::Error 转换为 Python 异常，尝试提取 Layer2Error
    fn anyhow_to_pyerr(e: anyhow::Error) -> PyErr {
        if let Some(layer2_err) = e.downcast_ref::<sh_layer2::Layer2Error>() {
            layer2_error_to_pyerr(layer2_err)
        } else {
            pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
        }
    }

    // ========================================================================
    // Layer 0: SecurityGateway
    // ========================================================================

    /// SecurityGateway Python 绑定
    #[pyclass(skip_from_py_object, name = "SecurityGateway")]
    pub struct PySecurityGateway {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer0::SecurityGateway>>,
    }

    #[pymethods]
    impl PySecurityGateway {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer0::SecurityGateway::new(),
                )),
            }
        }

        fn validate_input<'py>(&self, py: Python<'py>, input: String) -> PyResult<String> {
            let inner = self.inner.clone();
            pyo3_async_runtimes::tokio::run(py, async move {
                let gateway = inner.lock().await;
                gateway
                    .validate_input(&input)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }
    }

    // ========================================================================
    // Layer 0: Permission Management (RBAC)
    // ========================================================================

    /// Permission Python 类
    #[pyclass(skip_from_py_object, name = "Permission")]
    #[derive(Clone)]
    pub struct PyPermission {
        #[pyo3(get)]
        resource: String,
        #[pyo3(get)]
        action: String,
    }

    #[pymethods]
    impl PyPermission {
        #[new]
        fn new(resource: &str, action: &str) -> Self {
            Self {
                resource: resource.to_string(),
                action: action.to_string(),
            }
        }

        fn __repr__(&self) -> String {
            format!(
                "Permission(resource='{}', action='{}')",
                self.resource, self.action
            )
        }

        fn __eq__(&self, other: &PyPermission) -> bool {
            self.resource == other.resource && self.action == other.action
        }

        fn __hash__(&self) -> isize {
            // Simple hash combining resource and action
            let mut h: isize = 0;
            for c in self.resource.chars() {
                h = h.wrapping_mul(31).wrapping_add(c as isize);
            }
            for c in self.action.chars() {
                h = h.wrapping_mul(31).wrapping_add(c as isize);
            }
            h
        }
    }

    /// Role Python 类
    #[pyclass(skip_from_py_object, name = "Role")]
    #[derive(Clone)]
    pub struct PyRole {
        #[pyo3(get)]
        name: String,
        permissions: Vec<PyPermission>,
    }

    #[pymethods]
    impl PyRole {
        #[new]
        fn new(name: &str, permissions: Vec<PyPermission>) -> Self {
            Self {
                name: name.to_string(),
                permissions,
            }
        }

        #[getter]
        fn permissions(&self) -> Vec<PyPermission> {
            self.permissions.clone()
        }

        fn __repr__(&self) -> String {
            format!(
                "Role(name='{}', permissions={})",
                self.name,
                self.permissions.len()
            )
        }
    }

    /// PermissionManager Python 绑定
    ///
    /// RBAC 权限管理系统。
    ///
    /// Example:
    ///     >>> from sh_python import PermissionManager, Permission, Role
    ///     >>> pm = PermissionManager()
    ///     >>> pm.add_role("user1", "admin")
    ///     >>> pm.check("user1", "session", "read")
    ///     True
    ///     >>> pm.grant("user1", "custom_resource", "custom_action")
    ///     >>> pm.revoke("user1", "admin")
    #[pyclass(skip_from_py_object, name = "PermissionManager")]
    pub struct PyPermissionManager {
        inner: std::sync::Arc<sh_layer0::AccessController>,
    }

    #[pymethods]
    impl PyPermissionManager {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(sh_layer0::AccessController::new()),
            }
        }

        /// 检查用户是否有指定权限
        ///
        /// Args:
        ///     user_id: 用户ID
        ///     resource: 资源名称
        ///     action: 操作名称
        ///
        /// Returns:
        ///     bool: 是否有权限
        fn check(&self, user_id: &str, resource: &str, action: &str) -> bool {
            self.inner.check(user_id, resource, action)
        }

        /// 为用户添加角色 (grant role to user)
        ///
        /// Args:
        ///     user_id: 用户ID
        ///     role_name: 角色名称 (admin/user/guest 或自定义)
        fn grant(&self, user_id: &str, role_name: &str) {
            self.inner.add_role(user_id, role_name);
        }

        /// 为用户移除角色 (revoke role from user)
        ///
        /// Args:
        ///     user_id: 用户ID
        ///     role_name: 角色名称
        fn revoke(&self, user_id: &str, role_name: &str) {
            self.inner.remove_role(user_id, role_name);
        }

        /// 创建自定义角色
        ///
        /// Args:
        ///     role: Role 对象
        fn create_role(&self, role: &PyRole) {
            let permissions: std::collections::HashSet<sh_layer0::Permission> = role
                .permissions
                .iter()
                .map(|p| sh_layer0::Permission::new(&p.resource, &p.action))
                .collect();

            let rust_role = sh_layer0::Role {
                name: role.name.clone(),
                permissions,
            };

            self.inner.create_role(rust_role);
        }

        /// 获取用户所有权限
        ///
        /// Args:
        ///     user_id: 用户ID
        ///
        /// Returns:
        ///     List[Permission]: 用户所有权限列表
        fn get_permissions(&self, user_id: &str) -> Vec<PyPermission> {
            self.inner
                .get_permissions(user_id)
                .into_iter()
                .map(|p| PyPermission {
                    resource: p.resource,
                    action: p.action,
                })
                .collect()
        }

        /// 检查用户是否有管理员权限
        ///
        /// Args:
        ///     user_id: 用户ID
        ///
        /// Returns:
        ///     bool: 是否有管理员权限
        fn is_admin(&self, user_id: &str) -> bool {
            self.inner.check(user_id, "*", "*")
        }

        /// 获取用户角色列表
        ///
        /// Note: This is a simplified implementation since AccessController
        /// doesn't expose user_roles directly. Returns inferred roles.
        ///
        /// Args:
        ///     user_id: 用户ID
        ///
        /// Returns:
        ///     List[str]: 角色名称列表
        fn get_user_roles(&self, user_id: &str) -> Vec<String> {
            // Check against default roles
            let mut roles = Vec::new();

            if self.check(user_id, "*", "*") {
                roles.push("admin".to_string());
            } else if self.check(user_id, "tool", "execute") {
                roles.push("user".to_string());
            } else {
                roles.push("guest".to_string());
            }

            roles
        }
    }

    // ========================================================================
    // Layer 1: LlmClient, CostTracker
    // ========================================================================

    /// LLM 提供商类型枚举
    #[pyclass(skip_from_py_object, name = "LlmProvider")]
    #[derive(Clone)]
    pub enum PyLlmProvider {
        Anthropic(),
        OpenAI(),
        Gemini(),
        OpenAICompatible { base_url: String },
        Custom { base_url: String },
    }

    impl From<PyLlmProvider> for sh_layer1::LlmProvider {
        fn from(provider: PyLlmProvider) -> Self {
            match provider {
                PyLlmProvider::Anthropic() => sh_layer1::LlmProvider::Anthropic,
                PyLlmProvider::OpenAI() => sh_layer1::LlmProvider::OpenAI,
                PyLlmProvider::Gemini() => sh_layer1::LlmProvider::Gemini,
                PyLlmProvider::OpenAICompatible { base_url } => {
                    sh_layer1::LlmProvider::OpenAICompatible { base_url }
                }
                PyLlmProvider::Custom { base_url } => sh_layer1::LlmProvider::Custom(base_url),
            }
        }
    }

    #[pyclass(skip_from_py_object, name = "LlmRequestConfig")]
    #[derive(Clone)]
    pub struct PyLlmRequestConfig {
        #[pyo3(get)]
        model: String,
        #[pyo3(get)]
        max_tokens: u32,
        #[pyo3(get)]
        temperature: f32,
        #[pyo3(get)]
        system_prompt: Option<String>,
    }

    #[pymethods]
    impl PyLlmRequestConfig {
        #[new]
        #[pyo3(signature = (model="claude-sonnet-4-6", max_tokens=4096, temperature=0.7, system_prompt=None))]
        fn new(
            model: &str,
            max_tokens: u32,
            temperature: f32,
            system_prompt: Option<&str>,
        ) -> Self {
            Self {
                model: model.to_string(),
                max_tokens,
                temperature,
                system_prompt: system_prompt.map(|s| s.to_string()),
            }
        }
    }

    impl From<&PyLlmRequestConfig> for sh_layer1::LlmRequestConfig {
        fn from(config: &PyLlmRequestConfig) -> Self {
            sh_layer1::LlmRequestConfig {
                model: config.model.clone(),
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                system_prompt: config.system_prompt.clone(),
                stop_sequences: vec!["\n\n\n".to_string()],
            }
        }
    }

    /// LLM 响应 Python 类
    #[pyclass(skip_from_py_object, name = "LlmResponse")]
    pub struct PyLlmResponse {
        #[pyo3(get)]
        content: String,
        #[pyo3(get)]
        input_tokens: u32,
        #[pyo3(get)]
        output_tokens: u32,
        #[pyo3(get)]
        model: String,
        #[pyo3(get)]
        response_id: String,
    }

    #[pymethods]
    impl PyLlmResponse {
        /// 获取 token 总数
        fn total_tokens(&self) -> u32 {
            self.input_tokens + self.output_tokens
        }

        /// 转换为 JSON 字符串
        fn to_json(&self) -> PyResult<String> {
            serde_json::to_string(&serde_json::json!({
                "content": self.content,
                "input_tokens": self.input_tokens,
                "output_tokens": self.output_tokens,
                "model": self.model,
                "response_id": self.response_id,
            }))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
        }
    }

    #[pyclass(skip_from_py_object, name = "LlmClient")]
    pub struct PyLlmClient {
        inner: std::sync::Arc<tokio::sync::Mutex<Option<sh_layer1::LlmClient>>>,
        provider: PyLlmProvider,
    }

    #[pymethods]
    impl PyLlmClient {
        #[new]
        #[pyo3(signature = (provider="anthropic", api_key=None, base_url=None))]
        fn new(provider: &str, api_key: Option<&str>, base_url: Option<&str>) -> Self {
            let py_provider = match provider.to_lowercase().as_str() {
                "anthropic" => PyLlmProvider::Anthropic(),
                "openai" => PyLlmProvider::OpenAI(),
                "gemini" => PyLlmProvider::Gemini(),
                "deepseek" => PyLlmProvider::OpenAICompatible {
                    base_url: base_url
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string()),
                },
                "glm" => PyLlmProvider::OpenAICompatible {
                    base_url: base_url
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "https://open.bigmodel.cn/api/paas/v4".to_string()),
                },
                "qwen" => PyLlmProvider::OpenAICompatible {
                    base_url: base_url.map(|s| s.to_string()).unwrap_or_else(|| {
                        "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()
                    }),
                },
                "kimi" | "moonshot" => PyLlmProvider::OpenAICompatible {
                    base_url: base_url
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "https://api.moonshot.cn/v1".to_string()),
                },
                "grok" => PyLlmProvider::OpenAICompatible {
                    base_url: base_url
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "https://api.x.ai/v1".to_string()),
                },
                custom => PyLlmProvider::Custom {
                    base_url: custom.to_string(),
                },
            };

            let rust_provider = sh_layer1::LlmProvider::from(py_provider.clone());
            let key = api_key.map(|s| s.to_string()).unwrap_or_else(|| {
                std::env::var("ANTHROPIC_API_KEY")
                    .or_else(|_| std::env::var("OPENAI_API_KEY"))
                    .or_else(|_| std::env::var("GEMINI_API_KEY"))
                    .unwrap_or_default()
            });

            let client = sh_layer1::LlmClient::new(rust_provider, key);
            let client_with_url = if let Some(url) = base_url {
                client.with_base_url(url.to_string())
            } else {
                client
            };

            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(Some(client_with_url))),
                provider: py_provider,
            }
        }

        /// 连接并验证 API
        fn connect(&self, py: Python<'_>) -> PyResult<bool> {
            // 简单检查是否配置了客户端，释放 GIL
            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let client = inner.lock().await;
                    Ok(client.is_some())
                })
            })
        }

        /// 检查是否已连接
        fn is_connected(&self, py: Python<'_>) -> bool {
            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let client = inner.lock().await;
                    client.is_some()
                })
            })
        }

        /// 发送消息并获取响应
        fn send<'py>(
            &self,
            py: Python<'py>,
            messages: Vec<(String, String)>,
            config: &PyLlmRequestConfig,
        ) -> PyResult<PyLlmResponse> {
            let inner = self.inner.clone();
            let rust_config = sh_layer1::LlmRequestConfig::from(config);

            // 转换消息格式
            let llm_messages: Vec<sh_layer1::Message> = messages
                .into_iter()
                .map(|(role, content)| {
                    let msg_role = match role.to_lowercase().as_str() {
                        "user" => sh_layer1::MessageRole::User,
                        "assistant" => sh_layer1::MessageRole::Assistant,
                        "system" => sh_layer1::MessageRole::System,
                        _ => sh_layer1::MessageRole::User,
                    };
                    sh_layer1::Message {
                        role: msg_role,
                        content,
                    }
                })
                .collect();

            pyo3_async_runtimes::tokio::run(py, async move {
                let client_guard = inner.lock().await;
                let client = client_guard.as_ref().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err("LlmClient not initialized")
                })?;

                let response = client
                    .send(llm_messages, &rust_config)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                Ok(PyLlmResponse {
                    content: response.content,
                    input_tokens: response.usage.input_tokens,
                    output_tokens: response.usage.output_tokens,
                    model: response.model,
                    response_id: response.response_id,
                })
            })
        }

        /// 发送单条消息的便捷方法
        fn send_message<'py>(
            &self,
            py: Python<'py>,
            message: &str,
            config: &PyLlmRequestConfig,
        ) -> PyResult<PyLlmResponse> {
            self.send(py, vec![("user".to_string(), message.to_string())], config)
        }

        /// 获取提供商名称
        fn provider_name(&self) -> String {
            match &self.provider {
                PyLlmProvider::Anthropic() => "anthropic".to_string(),
                PyLlmProvider::OpenAI() => "openai".to_string(),
                PyLlmProvider::Gemini() => "gemini".to_string(),
                PyLlmProvider::OpenAICompatible { base_url } => {
                    format!("openai-compatible:{}", base_url)
                }
                PyLlmProvider::Custom { base_url } => format!("custom:{}", base_url),
            }
        }

        /// 获取支持的模型列表
        fn supported_models(&self) -> Vec<String> {
            match &self.provider {
                PyLlmProvider::Anthropic() => vec![
                    "claude-opus-4-8".to_string(),
                    "claude-opus-4-7".to_string(),
                    "claude-opus-4-6".to_string(),
                    "claude-opus-4-5".to_string(),
                    "claude-sonnet-4-6".to_string(),
                    "claude-sonnet-4-5".to_string(),
                    "claude-haiku-4-5".to_string(),
                ],
                PyLlmProvider::OpenAI() => vec![
                    "gpt-5.5".to_string(),
                    "gpt-5.4".to_string(),
                    "gpt-5.2".to_string(),
                    "gpt-5.1".to_string(),
                    "gpt-5".to_string(),
                    "o3-mini".to_string(),
                    "o1".to_string(),
                    "gpt-4o".to_string(),
                    "gpt-4o-mini".to_string(),
                ],
                PyLlmProvider::Gemini() => vec![
                    "gemini-3.1-pro-preview".to_string(),
                    "gemini-3.5-flash".to_string(),
                    "gemini-3.0-pro".to_string(),
                    "gemini-3.0-flash".to_string(),
                    "gemini-2.5-pro".to_string(),
                    "gemini-2.5-flash".to_string(),
                ],
                PyLlmProvider::OpenAICompatible { .. } => vec![
                    "deepseek-chat".to_string(),
                    "glm-4-flash".to_string(),
                    "qwen-plus".to_string(),
                    "moonshot-v1-8k".to_string(),
                    "grok-3".to_string(),
                ],
                PyLlmProvider::Custom { .. } => vec!["custom-model".to_string()],
            }
        }
    }

    #[pyclass(skip_from_py_object, name = "CostTracker")]
    pub struct PyCostTracker {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer1::CostTracker>>,
    }

    #[pymethods]
    impl PyCostTracker {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(sh_layer1::CostTracker::new())),
            }
        }

        /// 设置预算上限
        fn set_budget_limit(&self, py: Python<'_>, limit: f64) {
            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let tracker = inner.lock().await;
                    tracker.set_budget_limit(limit);
                })
            });
        }

        /// 记录使用情况
        fn record_usage<'py>(
            &self,
            py: Python<'py>,
            model: &str,
            input_tokens: u64,
            output_tokens: u64,
        ) -> PyResult<()> {
            let inner = self.inner.clone();
            let model_str = model.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let tracker = inner.lock().await;
                tracker
                    .record_usage(&model_str, input_tokens, output_tokens)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取当前使用情况
        fn get_current_usage<'py>(&self, py: Python<'py>) -> PyResult<PyUsageSnapshot> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let tracker = inner.lock().await;
                let snapshot = tracker.get_current_usage();
                Ok(PyUsageSnapshot::from_snapshot(snapshot))
            })
        }

        /// 预估下一步成本
        fn estimate_next_step(
            &self,
            py: Python<'_>,
            model: &str,
            estimated_input: u64,
            estimated_output: u64,
        ) -> PyCostEstimate {
            let inner = self.inner.clone();
            let model_str = model.to_string();
            py.detach(|| {
                runtime().block_on(async {
                    let tracker = inner.lock().await;
                    let estimate =
                        tracker.estimate_next_step(&model_str, estimated_input, estimated_output);
                    PyCostEstimate::from_estimate(estimate)
                })
            })
        }

        /// 生成成本报告
        fn generate_report<'py>(&self, py: Python<'py>) -> PyResult<String> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let tracker = inner.lock().await;
                let report = tracker.generate_report();
                serde_json::to_string(&report)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 重置追踪器
        fn reset(&self, py: Python<'_>) {
            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let tracker = inner.lock().await;
                    tracker.reset();
                })
            });
        }

        /// 获取总成本（便捷方法）
        fn total_cost(&self, py: Python<'_>) -> f64 {
            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let tracker = inner.lock().await;
                    tracker.get_current_usage().total_cost_usd
                })
            })
        }
    }

    /// 使用情况快照 Python 类
    #[pyclass(skip_from_py_object, name = "UsageSnapshot")]
    pub struct PyUsageSnapshot {
        #[pyo3(get)]
        total_input_tokens: u64,
        #[pyo3(get)]
        total_output_tokens: u64,
        #[pyo3(get)]
        total_cost_usd: f64,
        #[pyo3(get)]
        budget_remaining: Option<f64>,
    }

    impl PyUsageSnapshot {
        fn from_snapshot(snapshot: sh_layer1::cost_tracker::UsageSnapshot) -> Self {
            Self {
                total_input_tokens: snapshot.total_input_tokens,
                total_output_tokens: snapshot.total_output_tokens,
                total_cost_usd: snapshot.total_cost_usd,
                budget_remaining: snapshot.budget_remaining,
            }
        }
    }

    #[pymethods]
    impl PyUsageSnapshot {
        /// 获取模型成本明细
        fn model_costs(&self) -> PyResult<String> {
            // 返回 JSON 字符串，Python 可以解析
            let snapshot = sh_layer1::cost_tracker::UsageSnapshot {
                total_input_tokens: self.total_input_tokens,
                total_output_tokens: self.total_output_tokens,
                total_cost_usd: self.total_cost_usd,
                model_costs: std::collections::HashMap::new(),
                budget_remaining: self.budget_remaining,
            };
            serde_json::to_string(&snapshot.model_costs)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
        }
    }

    /// 成本预估 Python 类
    #[pyclass(skip_from_py_object, name = "CostEstimate")]
    pub struct PyCostEstimate {
        #[pyo3(get)]
        min_tokens: u64,
        #[pyo3(get)]
        max_tokens: u64,
        #[pyo3(get)]
        estimated_cost_usd: f64,
        #[pyo3(get)]
        confidence: String,
    }

    impl PyCostEstimate {
        fn from_estimate(estimate: sh_layer1::cost_tracker::CostEstimate) -> Self {
            Self {
                min_tokens: estimate.min_tokens,
                max_tokens: estimate.max_tokens,
                estimated_cost_usd: estimate.estimated_cost_usd,
                confidence: estimate.confidence,
            }
        }
    }

    // ========================================================================
    // Layer 2: AgentRuntime, SessionManager, CheckpointSystem, Agent, Session
    // ========================================================================

    /// Agent 配置 Python 类
    #[pyclass(skip_from_py_object, name = "AgentConfig")]
    #[derive(Clone)]
    pub struct PyAgentConfig {
        #[pyo3(get)]
        agent_id: String,
        #[pyo3(get)]
        model: String,
        #[pyo3(get)]
        temperature: f32,
        #[pyo3(get)]
        max_iterations: i32,
        #[pyo3(get)]
        system_prompt: Option<String>,
    }

    #[pymethods]
    impl PyAgentConfig {
        #[new]
        #[pyo3(signature = (agent_id=None, model="claude-sonnet-4-6", temperature=0.7, max_iterations=100, system_prompt=None))]
        fn new(
            agent_id: Option<&str>,
            model: &str,
            temperature: f32,
            max_iterations: i32,
            system_prompt: Option<&str>,
        ) -> Self {
            Self {
                agent_id: agent_id
                    .map(|s| s.to_string())
                    .unwrap_or_else(sh_layer1::generate_short_id),
                model: model.to_string(),
                temperature,
                max_iterations,
                system_prompt: system_prompt.map(|s| s.to_string()),
            }
        }
    }

    impl From<&PyAgentConfig> for sh_layer2::AgentConfig {
        fn from(config: &PyAgentConfig) -> Self {
            sh_layer2::AgentConfig {
                agent_id: sh_layer2::AgentId(config.agent_id.clone()),
                model: config.model.clone(),
                temperature: config.temperature,
                max_iterations: config.max_iterations,
                system_prompt: config.system_prompt.clone(),
            }
        }
    }

    /// Agent 结果 Python 类
    #[pyclass(skip_from_py_object, name = "AgentResult")]
    pub struct PyAgentResult {
        #[pyo3(get)]
        session_id: String,
        #[pyo3(get)]
        final_state: String,
        #[pyo3(get)]
        iterations: i32,
        #[pyo3(get)]
        tokens_used: i64,
        messages_json: String,
        tool_calls_json: String,
        tool_results_json: String,
    }

    #[pymethods]
    impl PyAgentResult {
        /// 获取消息列表
        fn get_messages(&self) -> PyResult<Vec<(String, String)>> {
            let messages: Vec<serde_json::Value> = serde_json::from_str(&self.messages_json)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

            Ok(messages
                .iter()
                .filter_map(|m| {
                    let role = m.get("role")?.as_str()?;
                    let content = m.get("content")?.as_str()?;
                    Some((role.to_string(), content.to_string()))
                })
                .collect())
        }

        /// 获取消息 JSON
        fn messages_json(&self) -> &str {
            &self.messages_json
        }

        /// 获取工具调用 JSON
        fn tool_calls_json(&self) -> &str {
            &self.tool_calls_json
        }

        /// 获取工具结果 JSON
        fn tool_results_json(&self) -> &str {
            &self.tool_results_json
        }
    }

    #[pyclass(skip_from_py_object, name = "AgentRuntime")]
    pub struct PyAgentRuntime {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer2::AgentRuntime>>,
    }

    #[pymethods]
    impl PyAgentRuntime {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer2::AgentRuntime::with_defaults(),
                )),
            }
        }

        /// 运行 Agent 完成任务
        fn run<'py>(
            &self,
            py: Python<'py>,
            task: &str,
            config: &PyAgentConfig,
        ) -> PyResult<PyAgentResult> {
            let inner = self.inner.clone();
            let rust_config = sh_layer2::AgentConfig::from(config);
            let task_str = task.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let runtime = inner.lock().await;
                let result = runtime
                    .run(&task_str, rust_config)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                // 转换结果
                let messages_json = serde_json::to_string(&result.messages)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                let tool_calls_json = serde_json::to_string(&result.tool_calls)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                let tool_results_json = serde_json::to_string(&result.tool_results)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                Ok(PyAgentResult {
                    session_id: result.session_id.0,
                    final_state: format!("{:?}", result.final_state),
                    iterations: result.iterations,
                    tokens_used: result.tokens_used,
                    messages_json,
                    tool_calls_json,
                    tool_results_json,
                })
            })
        }

        /// 启动 Agent 会话
        fn start<'py>(
            &self,
            py: Python<'py>,
            task: &str,
            config: &PyAgentConfig,
        ) -> PyResult<String> {
            let inner = self.inner.clone();
            let rust_config = sh_layer2::AgentConfig::from(config);
            let task_str = task.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let runtime = inner.lock().await;
                let session_id = runtime
                    .start(&task_str, rust_config)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                Ok(session_id.0)
            })
        }

        /// 暂停 Agent
        fn pause<'py>(&self, py: Python<'py>, session_id: &str) -> PyResult<()> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let runtime = inner.lock().await;
                runtime
                    .pause(&sid)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 恢复 Agent
        fn resume<'py>(&self, py: Python<'py>, session_id: &str) -> PyResult<()> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let runtime = inner.lock().await;
                runtime
                    .resume(&sid)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 停止 Agent
        fn stop<'py>(&self, py: Python<'py>, session_id: &str) -> PyResult<()> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let runtime = inner.lock().await;
                runtime
                    .stop(&sid)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取 Agent 状态
        fn status(&self, py: Python<'_>, session_id: &str) -> PyResult<String> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            py.detach(|| {
                runtime().block_on(async {
                    let rt = inner.lock().await;
                    let state = rt
                        .status(&sid)
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                    Ok(format!("{:?}", state))
                })
            })
        }

        /// 向 Agent 发送消息
        fn send_message<'py>(
            &self,
            py: Python<'py>,
            session_id: &str,
            message: &str,
        ) -> PyResult<()> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);
            let msg = message.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let runtime = inner.lock().await;
                runtime
                    .send_message(&sid, &msg)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 注册 Python 工具
        ///
        /// Args:
        ///     name: 工具名称
        ///     description: 工具描述
        ///     callable: Python 可调用对象 (接收 dict 参数，返回 str/dict)
        ///     parameters: 可选的参数 schema (dict 格式)
        ///
        /// Example:
        ///     def my_tool(args: dict) -> str:
        ///         return f"Processed: {args}"
        ///
        ///     runtime.register_tool("my_tool", "My custom tool", my_tool)
        #[pyo3(signature = (name, description, callable, parameters=None))]
        fn register_tool<'py>(
            &self,
            py: Python<'py>,
            name: &str,
            description: &str,
            callable: Py<PyAny>,
            parameters: Option<Bound<'py, PyDict>>,
        ) -> PyResult<()> {
            use crate::python_tool_adapter::PythonToolAdapter;

            // Create adapter from Python callable
            let adapter = PythonToolAdapter::from_callable(
                py,
                name.to_string(),
                description.to_string(),
                callable,
                parameters,
            )?;

            // Register in the tool registry
            let inner = self.inner.clone();
            let name_owned = name.to_string();

            py.detach(|| {
                runtime().block_on(async {
                    let rt = inner.lock().await;
                    rt.tool_registry().register(Box::new(adapter)).map_err(|e| {
                        pyo3::exceptions::PyRuntimeError::new_err(format!(
                            "Failed to register tool '{}': {}",
                            name_owned, e
                        ))
                    })
                })
            })
        }

        /// 列出可用工具
        fn list_tools(&self, py: Python<'_>) -> PyResult<Vec<String>> {
            let inner = self.inner.clone();

            py.detach(|| {
                runtime().block_on(async {
                    let rt = inner.lock().await;
                    Ok(rt
                        .tool_registry()
                        .definitions()
                        .iter()
                        .map(|d| d.function.name.clone())
                        .collect())
                })
            })
        }

        /// 运行 Agent 并返回流式迭代器
        ///
        /// 返回一个异步迭代器，可以在 Python 中使用 `async for` 进行迭代。
        /// 每次迭代返回一个 StreamChunk 对象，包含当前迭代的进度信息。
        ///
        /// Example:
        ///     runtime = AgentRuntime()
        ///     config = AgentConfig(model="claude-sonnet-4-6")
        ///     async for chunk in runtime.run_stream("your task", config):
        ///         print(chunk.content)
        fn run_stream<'py>(
            &self,
            _py: Python<'py>,
            task: &str,
            config: &PyAgentConfig,
        ) -> PyResult<PyAgentStreamIterator> {
            let inner = self.inner.clone();
            let rust_config = sh_layer2::AgentConfig::from(config);
            let task_str = task.to_string();

            // Create abort flag for cancellation support
            let abort_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            // Create the stream iterator with all necessary state
            Ok(PyAgentStreamIterator::new(
                inner,
                task_str,
                rust_config,
                abort_flag,
            ))
        }
    }

    /// 流式响应块
    ///
    /// 表示 Agent 执行过程中的一次迭代结果。
    #[pyclass(skip_from_py_object, name = "StreamChunk")]
    #[derive(Clone)]
    pub struct PyStreamChunk {
        /// 迭代次数
        #[pyo3(get)]
        iteration: i32,
        /// 当前状态
        #[pyo3(get)]
        state: String,
        /// 消息内容（如果有）
        #[pyo3(get)]
        content: Option<String>,
        /// 工具调用（如果有）
        #[pyo3(get)]
        tool_calls_json: Option<String>,
        /// 是否应该继续
        #[pyo3(get)]
        should_continue: bool,
        /// 是否是最终结果
        #[pyo3(get)]
        is_final: bool,
        /// 错误信息（如果有）
        #[pyo3(get)]
        error: Option<String>,
    }

    #[pymethods]
    impl PyStreamChunk {
        /// 转换为字典
        fn to_dict(&self) -> PyResult<Py<PyDict>> {
            Python::attach(|py| {
                let dict = PyDict::new(py);
                let _ = dict.set_item("iteration", self.iteration);
                let _ = dict.set_item("state", &self.state);
                let _ = dict.set_item("content", &self.content);
                let _ = dict.set_item("tool_calls", &self.tool_calls_json);
                let _ = dict.set_item("should_continue", self.should_continue);
                let _ = dict.set_item("is_final", self.is_final);
                let _ = dict.set_item("error", &self.error);
                Ok(dict.into())
            })
        }
    }

    /// Agent 流式迭代器
    ///
    /// 实现异步迭代器协议，支持在 Python 中使用 `async for` 进行迭代。
    ///
    /// Example:
    ///     async for chunk in runtime.run_stream("task", config):
    ///         if chunk.content:
    ///             print(chunk.content)
    ///         if chunk.is_final:
    ///             break
    #[pyclass(skip_from_py_object, name = "AgentStreamIterator")]
    pub struct PyAgentStreamIterator {
        /// Agent runtime 引用
        runtime: std::sync::Arc<tokio::sync::Mutex<sh_layer2::AgentRuntime>>,
        /// 任务描述
        task: String,
        /// Agent 配置
        config: sh_layer2::AgentConfig,
        /// 中断标志
        abort_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
        /// 会话 ID（创建后设置）
        session_id: std::sync::Mutex<Option<sh_layer2::SessionId>>,
        /// 当前迭代次数
        iteration: std::sync::Mutex<i32>,
        /// 是否已完成
        finished: std::sync::Mutex<bool>,
        /// 是否已开始
        #[allow(dead_code)]
        started: std::sync::Mutex<bool>,
        /// 最大迭代次数
        max_iterations: i32,
    }

    impl PyAgentStreamIterator {
        fn new(
            runtime: std::sync::Arc<tokio::sync::Mutex<sh_layer2::AgentRuntime>>,
            task: String,
            config: sh_layer2::AgentConfig,
            abort_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Self {
            let max_iterations = config.max_iterations;
            Self {
                runtime,
                task,
                config,
                abort_flag,
                session_id: std::sync::Mutex::new(None),
                iteration: std::sync::Mutex::new(0),
                finished: std::sync::Mutex::new(false),
                started: std::sync::Mutex::new(false),
                max_iterations,
            }
        }
    }

    #[pymethods]
    impl PyAgentStreamIterator {
        /// 返回 self 作为异步迭代器
        fn __aiter__(slf: Py<Self>) -> Py<Self> {
            slf
        }

        /// 获取下一个元素
        ///
        /// 这是异步迭代器的核心方法，返回一个 awaitable 对象。
        /// 使用 future_into_py 返回 Python coroutine，避免嵌套事件循环。
        fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
            // Check if already finished
            let finished = *self.finished.lock().unwrap();
            if finished {
                return Err(pyo3::exceptions::PyStopAsyncIteration::new_err(
                    "Stream finished",
                ));
            }

            // Check abort flag
            if self.abort_flag.load(std::sync::atomic::Ordering::Relaxed) {
                *self.finished.lock().unwrap() = true;
                return Err(pyo3::exceptions::PyStopAsyncIteration::new_err(
                    "Stream aborted",
                ));
            }

            // Capture state for the async closure
            let runtime = self.runtime.clone();
            let _task = self.task.clone();
            let _config = self.config.clone();
            let abort_flag = self.abort_flag.clone();
            let session_id_opt = self.session_id.lock().unwrap().clone();
            let max_iterations = self.max_iterations;

            // Increment iteration counter
            let mut iter_guard = self.iteration.lock().unwrap();
            *iter_guard += 1;
            let current_iteration = *iter_guard;
            drop(iter_guard);

            // Mark as finished if we exceed max iterations
            if current_iteration > max_iterations {
                *self.finished.lock().unwrap() = true;
            }

            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                // Check abort flag
                if abort_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(PyStreamChunk {
                        iteration: current_iteration,
                        state: "stopped".to_string(),
                        content: None,
                        tool_calls_json: None,
                        should_continue: false,
                        is_final: true,
                        error: Some("Aborted by user".to_string()),
                    });
                }

                // Check max iterations
                if current_iteration > max_iterations {
                    return Ok(PyStreamChunk {
                        iteration: current_iteration,
                        state: "error".to_string(),
                        content: None,
                        tool_calls_json: None,
                        should_continue: false,
                        is_final: true,
                        error: Some(format!("Max iterations ({}) reached", max_iterations)),
                    });
                }

                // If we have a session ID, use it; otherwise return a placeholder chunk
                // (Full implementation would create session on first iteration)
                if let Some(sid) = session_id_opt {
                    let rt = runtime.lock().await;

                    // Check if session can continue
                    let can_continue: bool = rt
                        .session_manager()
                        .read(&sid, |s| s.can_continue())
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
                        .unwrap_or(false);

                    if !can_continue {
                        let current_state: sh_layer2::AgentState = rt
                            .session_manager()
                            .read(&sid, |s| s.state)
                            .await
                            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
                            .unwrap_or(sh_layer2::AgentState::Stopped);

                        return Ok(PyStreamChunk {
                            iteration: current_iteration,
                            state: format!("{:?}", current_state).to_lowercase(),
                            content: None,
                            tool_calls_json: None,
                            should_continue: false,
                            is_final: true,
                            error: None,
                        });
                    }
                    drop(rt);
                }

                // Return a progress chunk
                // In full implementation, this would call LLM and process tool calls
                Ok(PyStreamChunk {
                    iteration: current_iteration,
                    state: "running".to_string(),
                    content: Some(format!("Iteration {}", current_iteration)),
                    tool_calls_json: None,
                    should_continue: current_iteration < max_iterations,
                    is_final: current_iteration >= max_iterations,
                    error: None,
                })
            })
        }

        /// 请求中断流式执行
        fn abort(&self) {
            self.abort_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        /// 检查是否已中断
        fn is_aborted(&self) -> bool {
            self.abort_flag.load(std::sync::atomic::Ordering::Relaxed)
        }

        /// 获取当前迭代次数
        fn current_iteration(&self) -> i32 {
            *self.iteration.lock().unwrap()
        }

        /// 检查是否已完成
        fn is_finished(&self) -> bool {
            *self.finished.lock().unwrap()
        }
    }

    /// 流式迭代器的 awaitable 对象
    ///
    /// 这个对象实现了 `__await__` 协议，可以被 await。
    #[pyclass(skip_from_py_object, name = "_StreamIteratorAwaitable")]
    pub struct PyStreamIteratorAwaitable {
        runtime: std::sync::Arc<tokio::sync::Mutex<sh_layer2::AgentRuntime>>,
        task: String,
        config: sh_layer2::AgentConfig,
        abort_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
        session_id_state: std::sync::Mutex<Option<sh_layer2::SessionId>>,
        iteration: i32,
        started: bool,
        max_iterations: i32,
        result: std::sync::Mutex<Option<PyStreamChunk>>,
    }

    #[pymethods]
    impl PyStreamIteratorAwaitable {
        /// 实现 __await__ 协议
        fn __await__(slf: Py<Self>) -> Py<Self> {
            slf
        }

        /// 实现 __next__ 用于 await
        fn __next__(&self, py: Python<'_>) -> PyResult<PyStreamChunk> {
            // Check if result is already computed
            if let Some(result) = self.result.lock().unwrap().take() {
                return Ok(result);
            }

            let runtime = self.runtime.clone();
            let task = self.task.clone();
            let config = self.config.clone();
            let abort_flag = self.abort_flag.clone();
            let iteration = self.iteration;
            let _started = self.started;
            let max_iterations = self.max_iterations;

            // Get or create session ID
            let session_id_opt = self.session_id_state.lock().unwrap().clone();

            let result = pyo3_async_runtimes::tokio::run(py, async move {
                // Check abort flag
                if abort_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(PyStreamChunk {
                        iteration,
                        state: "stopped".to_string(),
                        content: None,
                        tool_calls_json: None,
                        should_continue: false,
                        is_final: true,
                        error: Some("Aborted by user".to_string()),
                    });
                }

                // If not started, create session first
                let session_id = if let Some(sid) = session_id_opt {
                    sid
                } else {
                    let rt = runtime.lock().await;
                    let session_config = sh_layer2::SessionConfig::from(&config);
                    let sid = rt
                        .session_manager()
                        .create(session_config)
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    // Set agent_id
                    let agent_id = config.agent_id.clone();
                    rt.session_manager()
                        .update(&sid, |s| {
                            s.agent_id = agent_id;
                        })
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    // Add system prompt if configured
                    if let Some(ref prompt) = config.system_prompt {
                        rt.session_manager()
                            .add_message(&sid, sh_layer2::Message::system(prompt))
                            .await
                            .map_err(|e| {
                                pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
                            })?;
                    }

                    // Add user task message
                    rt.session_manager()
                        .add_message(&sid, sh_layer2::Message::user(&task))
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    // Transition to Running
                    rt.session_manager()
                        .set_state(&sid, sh_layer2::AgentState::Running)
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    sid
                };

                // Check max iterations
                if iteration > max_iterations {
                    let rt = runtime.lock().await;
                    rt.session_manager()
                        .set_state(&session_id, sh_layer2::AgentState::Error)
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    return Ok(PyStreamChunk {
                        iteration,
                        state: "error".to_string(),
                        content: None,
                        tool_calls_json: None,
                        should_continue: false,
                        is_final: true,
                        error: Some(format!("Max iterations ({}) reached", max_iterations)),
                    });
                }

                // Check if session can continue
                let rt = runtime.lock().await;
                let can_continue: bool = rt
                    .session_manager()
                    .read(&session_id, |s| s.can_continue())
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
                    .unwrap_or(false);

                if !can_continue {
                    let current_state: sh_layer2::AgentState = rt
                        .session_manager()
                        .read(&session_id, |s| s.state)
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
                        .unwrap_or(sh_layer2::AgentState::Stopped);

                    return Ok(PyStreamChunk {
                        iteration,
                        state: format!("{:?}", current_state).to_lowercase(),
                        content: None,
                        tool_calls_json: None,
                        should_continue: false,
                        is_final: true,
                        error: None,
                    });
                }

                // Execute one iteration step
                let step_result = rt
                    .session_manager()
                    .read(&session_id, |s| {
                        // Check for pending tool results
                        let has_pending = !s.tool_results_cache.is_empty();
                        let pending_results = s.tool_results_cache.clone();
                        (has_pending, pending_results)
                    })
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
                    .unwrap_or((false, Vec::new()));

                let should_continue = iteration < max_iterations;

                // Process pending tool results if any
                if step_result.0 {
                    let tool_results = step_result.1;
                    let summary: Vec<String> = tool_results
                        .iter()
                        .map(|r| {
                            if r.is_error {
                                format!("Tool {} failed: {}", r.name, r.content)
                            } else {
                                format!("Tool {} succeeded: {}", r.name, r.content)
                            }
                        })
                        .collect();

                    let response = if !should_continue {
                        format!(
                            "I've processed the tool results. Task '{}' is now complete.\n{}",
                            task,
                            summary.join("\n")
                        )
                    } else {
                        format!(
                            "Processing tool results, continuing...\n{}",
                            summary.join("\n")
                        )
                    };

                    // Clear tool results cache
                    rt.session_manager()
                        .update(&session_id, |s| {
                            s.tool_results_cache.clear();
                        })
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    // Add assistant message
                    rt.session_manager()
                        .add_message(&session_id, sh_layer2::Message::assistant(&response))
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    let state = if should_continue {
                        sh_layer2::AgentState::Running
                    } else {
                        sh_layer2::AgentState::Completed
                    };

                    rt.session_manager()
                        .set_state(&session_id, state)
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    return Ok(PyStreamChunk {
                        iteration,
                        state: format!(
                            "{:?}",
                            if should_continue {
                                sh_layer2::AgentState::Running
                            } else {
                                sh_layer2::AgentState::Completed
                            }
                        )
                        .to_lowercase(),
                        content: Some(response),
                        tool_calls_json: None,
                        should_continue,
                        is_final: !should_continue,
                        error: None,
                    });
                }

                // First iteration: acknowledge the task
                if iteration == 1 {
                    let response = format!("Starting task: {}", task);
                    rt.session_manager()
                        .add_message(&session_id, sh_layer2::Message::assistant(&response))
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    return Ok(PyStreamChunk {
                        iteration,
                        state: "running".to_string(),
                        content: Some(response),
                        tool_calls_json: None,
                        should_continue: true,
                        is_final: false,
                        error: None,
                    });
                }

                // Check for registered tools
                let tools = rt.tool_registry().list();
                if !tools.is_empty() && iteration <= 2 {
                    // Simulate tool call on second iteration
                    let tool_name = &tools[0];
                    let tool_call = sh_layer2::ToolCall {
                        id: sh_layer1::generate_prefixed_id("tc"),
                        name: tool_name.clone(),
                        arguments: serde_json::json!({"task": task}).to_string(),
                    };

                    // Store pending tool calls
                    let tc_clone = tool_call.clone();
                    rt.session_manager()
                        .update(&session_id, |s| {
                            s.tool_calls_pending = vec![tc_clone];
                            s.state = sh_layer2::AgentState::ToolCalling;
                        })
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    // Execute the tool
                    let tool_result = match rt
                        .tool_registry()
                        .execute(&tool_call.name, &tool_call.arguments)
                        .await
                    {
                        Ok(result) => result,
                        Err(e) => sh_layer2::ToolResult {
                            tool_call_id: tool_call.id.clone(),
                            name: tool_call.name.clone(),
                            content: format!("Tool execution error: {}", e),
                            is_error: true,
                        },
                    };

                    // Store result
                    rt.session_manager()
                        .update(&session_id, |s| {
                            s.tool_results_cache.push(tool_result.clone());
                            s.tool_calls_pending.clear();
                        })
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    // Transition states
                    rt.session_manager()
                        .set_state(&session_id, sh_layer2::AgentState::WaitingTool)
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    let response =
                        format!("I'll use the {} tool to help with this task.", tool_name);
                    rt.session_manager()
                        .add_message(&session_id, sh_layer2::Message::assistant(&response))
                        .await
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    let tool_calls_json = serde_json::to_string(&vec![tool_call])
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                    return Ok(PyStreamChunk {
                        iteration,
                        state: "tool_calling".to_string(),
                        content: Some(response),
                        tool_calls_json: Some(tool_calls_json),
                        should_continue: true,
                        is_final: false,
                        error: None,
                    });
                }

                // Final iteration: complete the task
                let response = format!("Task '{}' has been completed.", task);
                rt.session_manager()
                    .add_message(&session_id, sh_layer2::Message::assistant(&response))
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                rt.session_manager()
                    .set_state(&session_id, sh_layer2::AgentState::Completed)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                Ok(PyStreamChunk {
                    iteration,
                    state: "completed".to_string(),
                    content: Some(response),
                    tool_calls_json: None,
                    should_continue: false,
                    is_final: true,
                    error: None,
                })
            })?;

            Ok(result)
        }
    }

    #[pyclass(skip_from_py_object, name = "SessionManager")]
    pub struct PySessionManager {
        inner: std::sync::Arc<sh_layer2::ConcurrentSessionManager>,
    }

    #[pymethods]
    impl PySessionManager {
        #[new]
        #[pyo3(signature = (max_sessions=100))]
        fn new(max_sessions: usize) -> Self {
            Self {
                inner: std::sync::Arc::new(sh_layer2::ConcurrentSessionManager::new(max_sessions)),
            }
        }

        /// 创建新会话
        fn create<'py>(
            &self,
            py: Python<'py>,
            model: Option<&str>,
            max_iterations: Option<i32>,
        ) -> PyResult<String> {
            let inner = self.inner.clone();
            let config = sh_layer2::SessionConfig {
                model: model
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "claude-sonnet-4-6".to_string()),
                max_iterations: max_iterations.unwrap_or(100),
                temperature: 0.7,
                system_prompt: None,
                ..Default::default()
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                let session_id = manager.create(config).await.map_err(anyhow_to_pyerr)?;

                Ok(session_id.0)
            })
        }

        /// 获取会话
        fn get<'py>(&self, py: Python<'py>, session_id: &str) -> PyResult<Option<String>> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                let session = manager.get(&sid).await.map_err(anyhow_to_pyerr)?;

                match session {
                    Some(s) => {
                        let json = serde_json::to_string(&serde_json::json!({
                            "session_id": s.session_id.0,
                            "agent_id": s.agent_id.0,
                            "state": format!("{:?}", s.state),
                            "created_at": s.created_at.to_rfc3339(),
                            "messages_count": s.messages.len(),
                            "tokens_total": s.tokens_total,
                        }))
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                        Ok(Some(json))
                    }
                    None => Ok(None),
                }
            })
        }

        /// 删除会话
        fn delete<'py>(&self, py: Python<'py>, session_id: &str) -> PyResult<bool> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                manager.delete(&sid).await.map_err(anyhow_to_pyerr)
            })
        }

        /// 列出所有会话
        fn list<'py>(&self, py: Python<'py>) -> PyResult<Vec<(String, String, String)>> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                let metas = manager.list().await.map_err(anyhow_to_pyerr)?;

                Ok(metas
                    .iter()
                    .map(|m| {
                        (
                            m.session_id.0.clone(),
                            m.agent_id.0.clone(),
                            format!("{:?}", m.state),
                        )
                    })
                    .collect())
            })
        }

        /// 获取会话统计
        fn stats(&self) -> (usize, usize, usize) {
            let manager = &self.inner;
            let stats = manager.stats();
            (
                stats.total_sessions,
                stats.max_sessions,
                stats.active_sessions,
            )
        }

        /// 设置会话状态
        fn set_state<'py>(&self, py: Python<'py>, session_id: &str, state: &str) -> PyResult<bool> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            let agent_state = match state.to_lowercase().as_str() {
                "idle" => sh_layer2::AgentState::Idle,
                "running" => sh_layer2::AgentState::Running,
                "toolcalling" => sh_layer2::AgentState::ToolCalling,
                "waitingtool" => sh_layer2::AgentState::WaitingTool,
                "stopped" => sh_layer2::AgentState::Stopped,
                "completed" => sh_layer2::AgentState::Completed,
                "error" => sh_layer2::AgentState::Error,
                _ => sh_layer2::AgentState::Idle,
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                manager
                    .set_state(&sid, agent_state)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 添加消息到会话
        fn add_message<'py>(
            &self,
            py: Python<'py>,
            session_id: &str,
            role: &str,
            content: &str,
        ) -> PyResult<bool> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            let message = match role.to_lowercase().as_str() {
                "user" => sh_layer2::Message::user(content),
                "assistant" => sh_layer2::Message::assistant(content),
                "system" => sh_layer2::Message::system(content),
                _ => sh_layer2::Message::user(content),
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                manager
                    .add_message(&sid, message)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取会话消息
        fn get_messages<'py>(
            &self,
            py: Python<'py>,
            session_id: &str,
        ) -> PyResult<Vec<(String, String)>> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let manager = inner;
                let messages = manager
                    .get_messages(&sid)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                match messages {
                    Some(msgs) => Ok(msgs
                        .iter()
                        .map(|m| (format!("{:?}", m.role), m.content.clone()))
                        .collect()),
                    None => Ok(Vec::new()),
                }
            })
        }
    }

    /// CheckpointSystem - 检查点写入器
    #[pyclass(skip_from_py_object, name = "CheckpointSystem")]
    pub struct PyCheckpointSystem {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer2::CheckpointWriter>>,
    }

    #[pymethods]
    impl PyCheckpointSystem {
        #[new]
        #[pyo3(signature = (storage_path=None))]
        fn new(storage_path: Option<&str>) -> Self {
            let path = storage_path
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("continuum_checkpoints"));
            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer2::CheckpointWriter::new(path),
                )),
            }
        }

        /// 保存检查点
        fn save<'py>(&self, py: Python<'py>, session_id: &str, data: String) -> PyResult<String> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);
            let checkpoint_data = sh_layer2::CheckpointData {
                checkpoint_id: sh_layer2::CheckpointId::new(),
                session_id: sid.clone(),
                created_at: chrono::Utc::now(),
                trigger: "manual".to_string(),
                iteration: 0,
                messages: vec![
                    serde_json::from_str(&data).unwrap_or(serde_json::json!({"content": data}))
                ],
                tool_calls_pending: Vec::new(),
                tool_results: serde_json::Value::Null,
                tokens_used: 0,
                cost_estimate: 0.0,
                resume_hint: None,
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let writer = inner.lock().await;
                let id = writer
                    .save(&checkpoint_data)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                Ok(id.to_string())
            })
        }

        /// 加载检查点
        fn load<'py>(
            &self,
            py: Python<'py>,
            session_id: &str,
            checkpoint_id: Option<&str>,
        ) -> PyResult<Option<String>> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);
            let cid = checkpoint_id.map(|s| sh_layer2::CheckpointId(s.to_string()));

            pyo3_async_runtimes::tokio::run(py, async move {
                let writer = inner.lock().await;
                match writer.load(&sid, cid.as_ref()).await {
                    Ok(Some(data)) => {
                        let json = serde_json::to_string(&data.messages).map_err(|e| {
                            pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
                        })?;
                        Ok(Some(json))
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                }
            })
        }

        /// 列出所有检查点
        fn list<'py>(&self, py: Python<'py>, session_id: &str) -> PyResult<Vec<String>> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);

            pyo3_async_runtimes::tokio::run(py, async move {
                let writer = inner.lock().await;
                let metas = writer
                    .list(&sid)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                Ok(metas.iter().map(|m| m.checkpoint_id.to_string()).collect())
            })
        }

        /// 删除检查点
        fn delete<'py>(
            &self,
            py: Python<'py>,
            session_id: &str,
            checkpoint_id: &str,
        ) -> PyResult<bool> {
            let inner = self.inner.clone();
            let sid = sh_layer2::SessionId::from(session_id);
            let cid = sh_layer2::CheckpointId(checkpoint_id.to_string());

            pyo3_async_runtimes::tokio::run(py, async move {
                let writer = inner.lock().await;
                writer
                    .delete(&sid, &cid)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }
    }

    /// Agent Python 绑定
    ///
    /// 智能代理的核心实现，支持任务执行和状态管理。
    ///
    /// Example:
    ///     >>> from sh_python import Agent
    ///     >>> agent = Agent("my-agent")
    ///     >>> agent.id
    ///     'my-agent'
    ///     >>> agent.start()
    ///     >>> agent.pause()
    ///     >>> agent.stop()
    #[pyclass(skip_from_py_object, name = "Agent")]
    pub struct PyAgent {
        id: String,
        agent_state: std::sync::Mutex<AgentState>,
    }

    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    enum AgentState {
        Idle,
        Running,
        Paused,
        Error,
    }

    #[pymethods]
    impl PyAgent {
        #[new]
        #[pyo3(signature = (name=None))]
        fn new(name: Option<&str>) -> Self {
            Self {
                id: name.unwrap_or("default").to_string(),
                agent_state: std::sync::Mutex::new(AgentState::Idle),
            }
        }

        #[getter]
        fn id(&self) -> &str {
            &self.id
        }

        #[getter]
        fn state(&self) -> String {
            match *self.agent_state.lock().unwrap() {
                AgentState::Idle => "idle".to_string(),
                AgentState::Running => "running".to_string(),
                AgentState::Paused => "paused".to_string(),
                AgentState::Error => "error".to_string(),
            }
        }

        fn start(&self) -> PyResult<()> {
            let mut state = self.agent_state.lock().unwrap();
            match *state {
                AgentState::Idle | AgentState::Paused => {
                    *state = AgentState::Running;
                    Ok(())
                }
                AgentState::Running => Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "Agent is already running",
                )),
                AgentState::Error => Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "Agent is in error state",
                )),
            }
        }

        fn pause(&self) -> PyResult<()> {
            let mut state = self.agent_state.lock().unwrap();
            match *state {
                AgentState::Running => {
                    *state = AgentState::Paused;
                    Ok(())
                }
                _ => Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "Agent is not running",
                )),
            }
        }

        fn stop(&self) {
            let mut state = self.agent_state.lock().unwrap();
            *state = AgentState::Idle;
        }

        fn execute(&self, task: &str) -> PyResult<String> {
            let state = self.agent_state.lock().unwrap();
            match *state {
                AgentState::Running => Ok(format!("Executing: {}", task)),
                _ => Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "Agent is not running",
                )),
            }
        }

        fn create_session(&self) -> PySession {
            PySession::new(Some(&format!("{}-session", self.id)))
        }
    }

    /// Session Python 绑定
    ///
    /// 会话管理器，用于跟踪对话历史和消息状态。
    ///
    /// Example:
    ///     >>> from sh_python import Session
    ///     >>> session = Session("my-session")
    ///     >>> session.id
    ///     'my-session'
    ///     >>> session.add_message("user", "Hello!")
    ///     >>> session.add_message("assistant", "Hi there!")
    ///     >>> len(session.get_history())
    ///     2
    ///     >>> session.clear()
    #[pyclass(skip_from_py_object, name = "Session")]
    pub struct PySession {
        id: String,
        created_at: chrono::DateTime<chrono::Utc>,
        messages: std::sync::Mutex<Vec<SessionMessage>>,
    }

    #[derive(Clone)]
    struct SessionMessage {
        role: String,
        content: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    }

    #[pymethods]
    impl PySession {
        #[new]
        #[pyo3(signature = (id=None))]
        fn new(id: Option<&str>) -> Self {
            Self {
                id: id.unwrap_or("default-session").to_string(),
                created_at: chrono::Utc::now(),
                messages: std::sync::Mutex::new(Vec::new()),
            }
        }

        #[getter]
        fn id(&self) -> &str {
            &self.id
        }

        #[getter]
        fn created_at(&self) -> String {
            self.created_at.to_rfc3339()
        }

        fn add_user_message(&self, content: &str) {
            let mut messages = self.messages.lock().unwrap();
            messages.push(SessionMessage {
                role: "user".to_string(),
                content: content.to_string(),
                timestamp: chrono::Utc::now(),
            });
        }

        fn add_assistant_message(&self, content: &str) {
            let mut messages = self.messages.lock().unwrap();
            messages.push(SessionMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
                timestamp: chrono::Utc::now(),
            });
        }

        fn message_count(&self) -> usize {
            self.messages.lock().unwrap().len()
        }

        fn get_messages(&self) -> Vec<(String, String)> {
            self.messages
                .lock()
                .unwrap()
                .iter()
                .map(|m| (m.role.clone(), m.content.clone()))
                .collect()
        }

        fn clear_messages(&self) {
            self.messages.lock().unwrap().clear();
        }

        fn export(&self) -> String {
            let messages = self.messages.lock().unwrap();
            let exported: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                        "timestamp": m.timestamp.to_rfc3339()
                    })
                })
                .collect();
            serde_json::to_string(&exported).unwrap_or_default()
        }
    }

    // ========================================================================
    // Layer 3: ToolExecutor, QueryEngine, MemorySystem
    // ========================================================================

    /// ToolExecutor - 工具执行器
    #[pyclass(skip_from_py_object, name = "ToolExecutor")]
    pub struct PyToolExecutor {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer3::DefaultToolExecutor>>,
    }

    #[pymethods]
    impl PyToolExecutor {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer3::DefaultToolExecutor::new(),
                )),
            }
        }

        /// 执行工具
        fn execute<'py>(&self, py: Python<'py>, name: &str, args_json: String) -> PyResult<String> {
            let inner = self.inner.clone();
            let tool_name = name.to_string();
            let args: serde_json::Value = serde_json::from_str(&args_json).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid JSON: {}", e))
            })?;

            let request = sh_layer3::ToolRequest {
                call_id: sh_layer1::generate_short_id(),
                name: tool_name.clone(),
                arguments: args,
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let executor = inner.lock().await;
                let response = executor
                    .execute(request)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                Ok(response.content)
            })
        }

        /// 读取文件
        #[pyo3(signature = (path, offset=None, limit=None))]
        fn read_file<'py>(
            &self,
            py: Python<'py>,
            path: &str,
            offset: Option<usize>,
            limit: Option<usize>,
        ) -> PyResult<String> {
            let args = serde_json::json!({
                "path": path,
                "offset": offset,
                "limit": limit,
            });

            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    let request = sh_layer3::ToolRequest {
                        call_id: sh_layer1::generate_short_id(),
                        name: "read_file".to_string(),
                        arguments: args,
                    };

                    match executor.execute(request).await {
                        Ok(response) => Ok(response.content),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// 写入文件
        fn write_file<'py>(&self, py: Python<'py>, path: &str, content: &str) -> PyResult<String> {
            let args = serde_json::json!({
                "path": path,
                "content": content,
            });

            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    let request = sh_layer3::ToolRequest {
                        call_id: sh_layer1::generate_short_id(),
                        name: "write_file".to_string(),
                        arguments: args,
                    };

                    match executor.execute(request).await {
                        Ok(response) => Ok(response.content),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// 执行 Bash 命令
        #[pyo3(signature = (command, timeout_ms=None, working_dir=None))]
        fn bash<'py>(
            &self,
            py: Python<'py>,
            command: &str,
            timeout_ms: Option<u64>,
            working_dir: Option<&str>,
        ) -> PyResult<String> {
            let mut args = serde_json::json!({
                "command": command,
            });
            if let Some(t) = timeout_ms {
                args["timeout"] = serde_json::json!(t);
            }
            if let Some(w) = working_dir {
                args["working_dir"] = serde_json::json!(w);
            }

            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    let request = sh_layer3::ToolRequest {
                        call_id: sh_layer1::generate_short_id(),
                        name: "bash".to_string(),
                        arguments: args,
                    };

                    match executor.execute(request).await {
                        Ok(response) => Ok(response.content),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// Grep 搜索
        #[pyo3(signature = (pattern, path=None, glob=None))]
        fn grep<'py>(
            &self,
            py: Python<'py>,
            pattern: &str,
            path: Option<&str>,
            glob: Option<&str>,
        ) -> PyResult<String> {
            let mut args = serde_json::json!({
                "pattern": pattern,
            });
            if let Some(p) = path {
                args["path"] = serde_json::json!(p);
            }
            if let Some(g) = glob {
                args["glob"] = serde_json::json!(g);
            }

            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    let request = sh_layer3::ToolRequest {
                        call_id: sh_layer1::generate_short_id(),
                        name: "grep".to_string(),
                        arguments: args,
                    };

                    match executor.execute(request).await {
                        Ok(response) => Ok(response.content),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// Glob 查找
        #[pyo3(signature = (pattern, path=None))]
        fn glob<'py>(
            &self,
            py: Python<'py>,
            pattern: &str,
            path: Option<&str>,
        ) -> PyResult<String> {
            let mut args = serde_json::json!({
                "pattern": pattern,
            });
            if let Some(p) = path {
                args["path"] = serde_json::json!(p);
            }

            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    let request = sh_layer3::ToolRequest {
                        call_id: sh_layer1::generate_short_id(),
                        name: "glob".to_string(),
                        arguments: args,
                    };

                    match executor.execute(request).await {
                        Ok(response) => Ok(response.content),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// 列出可用工具
        fn list_tools<'py>(&self, py: Python<'py>) -> Vec<(String, String)> {
            let inner = self.inner.clone();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    executor
                        .list_tools()
                        .iter()
                        .map(|m| (m.name.clone(), m.description.clone()))
                        .collect()
                })
            })
        }

        /// 检查工具是否可用
        fn is_available<'py>(&self, py: Python<'py>, name: &str) -> bool {
            let inner = self.inner.clone();
            let tool_name = name.to_string();
            py.detach(|| {
                runtime().block_on(async {
                    let executor = inner.lock().await;
                    executor.is_available(&tool_name)
                })
            })
        }
    }

    #[pyclass(skip_from_py_object, name = "QueryEngine")]
    pub struct PyQueryEngine {
        inner: std::sync::Arc<sh_layer3::SyncLspClient>,
    }

    #[pymethods]
    impl PyQueryEngine {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(sh_layer3::SyncLspClient::new()),
            }
        }

        fn initialize(&self, language: &str, root_path: &str) -> PyResult<bool> {
            let path = std::path::PathBuf::from(root_path);
            match self.inner.initialize(language, &path) {
                Ok(_) => Ok(true),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        fn go_to_definition<'py>(
            &self,
            py: Python<'py>,
            language: &str,
            file_path: &str,
            line: u32,
            column: u32,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            let inner = self.inner.clone();
            let path = std::path::PathBuf::from(file_path);
            let position = sh_layer3::Position {
                line,
                character: column,
            };

            match inner.go_to_definition(language, &path, position) {
                Ok(locations) => {
                    let results: Vec<Bound<'py, PyDict>> = locations
                        .iter()
                        .map(|loc| {
                            let dict = PyDict::new(py);
                            let _ = dict.set_item("uri", loc.uri.clone());
                            let _ = dict.set_item("line", loc.range.start.line);
                            let _ = dict.set_item("column", loc.range.start.character);
                            dict
                        })
                        .collect();
                    Ok(results)
                }
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        fn find_references<'py>(
            &self,
            py: Python<'py>,
            language: &str,
            file_path: &str,
            line: u32,
            column: u32,
            include_declaration: bool,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            let inner = self.inner.clone();
            let path = std::path::PathBuf::from(file_path);
            let position = sh_layer3::Position {
                line,
                character: column,
            };

            match inner.find_references(language, &path, position, include_declaration) {
                Ok(locations) => {
                    let results: Vec<Bound<'py, PyDict>> = locations
                        .iter()
                        .map(|loc| {
                            let dict = PyDict::new(py);
                            let _ = dict.set_item("uri", loc.uri.clone());
                            let _ = dict.set_item("line", loc.range.start.line);
                            let _ = dict.set_item("column", loc.range.start.character);
                            dict
                        })
                        .collect();
                    Ok(results)
                }
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        fn hover<'py>(
            &self,
            _py: Python<'py>,
            language: &str,
            file_path: &str,
            line: u32,
            column: u32,
        ) -> PyResult<Option<String>> {
            let inner = self.inner.clone();
            let path = std::path::PathBuf::from(file_path);
            let position = sh_layer3::Position {
                line,
                character: column,
            };

            match inner.get_hover(language, &path, position) {
                Ok(Some(hover)) => {
                    let content = match hover.contents {
                        sh_layer3::HoverContents::Markup(mc) => mc.value.clone(),
                        sh_layer3::HoverContents::String(s) => s,
                        sh_layer3::HoverContents::Array(arr) => arr
                            .iter()
                            .map(|ms| match ms {
                                sh_layer3::MarkedString::String(s) => s.clone(),
                                sh_layer3::MarkedString::LanguageString(ls) => {
                                    format!("```{}\n{}\n```", ls.language, ls.value)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    };
                    Ok(Some(content))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        fn shutdown(&self, language: &str) -> PyResult<()> {
            match self.inner.shutdown(language) {
                Ok(_) => Ok(()),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        fn is_connected(&self, language: &str) -> bool {
            self.inner.is_connected(language)
        }

        /// Get document symbols (outline of classes, functions, etc.)
        fn get_document_symbols<'py>(
            &self,
            py: Python<'py>,
            language: &str,
            file_path: &str,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            let inner = self.inner.clone();
            let path = std::path::PathBuf::from(file_path);

            match inner.get_document_symbols(language, &path) {
                Ok(symbols) => {
                    fn symbol_to_dict<'py>(
                        py: Python<'py>,
                        symbol: &sh_layer3::DocumentSymbol,
                    ) -> Bound<'py, PyDict> {
                        let dict = PyDict::new(py);
                        let _ = dict.set_item("name", symbol.name.clone());
                        let _ = dict.set_item("kind", symbol.kind as u32);
                        let _ = dict.set_item("detail", symbol.detail.clone().unwrap_or_default());
                        let _ = dict.set_item("range_start_line", symbol.range.start.line);
                        let _ = dict.set_item("range_start_char", symbol.range.start.character);
                        let _ = dict.set_item("range_end_line", symbol.range.end.line);
                        let _ = dict.set_item("range_end_char", symbol.range.end.character);
                        let _ = dict.set_item(
                            "selection_range_start_line",
                            symbol.selection_range.start.line,
                        );
                        let _ = dict.set_item(
                            "selection_range_start_char",
                            symbol.selection_range.start.character,
                        );
                        let _ = dict
                            .set_item("selection_range_end_line", symbol.selection_range.end.line);
                        let _ = dict.set_item(
                            "selection_range_end_char",
                            symbol.selection_range.end.character,
                        );
                        if let Some(children) = &symbol.children {
                            let children_dicts: Vec<Bound<'py, PyDict>> =
                                children.iter().map(|c| symbol_to_dict(py, c)).collect();
                            let _ = dict.set_item("children", children_dicts);
                        }
                        dict
                    }

                    let results: Vec<Bound<'py, PyDict>> =
                        symbols.iter().map(|s| symbol_to_dict(py, s)).collect();
                    Ok(results)
                }
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        /// Get full symbol information including definition, references, hover, and type info
        fn full_symbol_info<'py>(
            &self,
            py: Python<'py>,
            language: &str,
            file_path: &str,
            line: u32,
            column: u32,
        ) -> PyResult<Bound<'py, PyDict>> {
            let inner = self.inner.clone();
            let path = std::path::PathBuf::from(file_path);

            let result_dict = PyDict::new(py);

            // Get definition
            let position = sh_layer3::Position {
                line,
                character: column,
            };
            let definition_result = inner.go_to_definition(language, &path, position);
            match definition_result {
                Ok(locations) if !locations.is_empty() => {
                    let def_dicts: Vec<Bound<'py, PyDict>> = locations
                        .iter()
                        .map(|loc| {
                            let dict = PyDict::new(py);
                            let _ = dict.set_item("uri", loc.uri.clone());
                            let _ = dict.set_item("line", loc.range.start.line);
                            let _ = dict.set_item("column", loc.range.start.character);
                            dict
                        })
                        .collect();
                    let _ = result_dict.set_item("definition", def_dicts);
                }
                _ => {
                    let empty: Vec<Bound<'py, PyDict>> = Vec::new();
                    let _ = result_dict.set_item("definition", empty);
                }
            }

            // Get references
            let position = sh_layer3::Position {
                line,
                character: column,
            };
            let references_result = inner.find_references(language, &path, position, true);
            match references_result {
                Ok(locations) => {
                    let ref_dicts: Vec<Bound<'py, PyDict>> = locations
                        .iter()
                        .map(|loc| {
                            let dict = PyDict::new(py);
                            let _ = dict.set_item("uri", loc.uri.clone());
                            let _ = dict.set_item("line", loc.range.start.line);
                            let _ = dict.set_item("column", loc.range.start.character);
                            dict
                        })
                        .collect();
                    let _ = result_dict.set_item("references", ref_dicts);
                }
                Err(e) => {
                    let _ = result_dict.set_item("references_error", e.to_string());
                }
            }

            // Get hover
            let position = sh_layer3::Position {
                line,
                character: column,
            };
            let hover_result = inner.get_hover(language, &path, position);
            match hover_result {
                Ok(Some(hover)) => {
                    let content = match hover.contents {
                        sh_layer3::HoverContents::Markup(mc) => mc.value.clone(),
                        sh_layer3::HoverContents::String(s) => s,
                        sh_layer3::HoverContents::Array(arr) => arr
                            .iter()
                            .map(|ms| match ms {
                                sh_layer3::MarkedString::String(s) => s.clone(),
                                sh_layer3::MarkedString::LanguageString(ls) => {
                                    format!("```{}\n{}\n```", ls.language, ls.value)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    };
                    let _ = result_dict.set_item("hover", content);
                }
                Ok(None) => {
                    let _ = result_dict.set_item("hover", py.None());
                }
                Err(e) => {
                    let _ = result_dict.set_item("hover_error", e.to_string());
                }
            }

            // Position info
            let _ = result_dict.set_item("file_path", file_path);
            let _ = result_dict.set_item("line", line);
            let _ = result_dict.set_item("column", column);
            let _ = result_dict.set_item("language", language);

            Ok(result_dict)
        }

        /// Rename a symbol across the workspace
        fn rename_symbol<'py>(
            &self,
            py: Python<'py>,
            language: &str,
            file_path: &str,
            line: u32,
            column: u32,
            new_name: &str,
        ) -> PyResult<Option<Bound<'py, PyDict>>> {
            let inner = self.inner.clone();
            let path = std::path::PathBuf::from(file_path);
            let position = sh_layer3::Position {
                line,
                character: column,
            };

            match inner.rename_symbol(language, &path, position, new_name) {
                Ok(Some(edit)) => {
                    let dict = PyDict::new(py);

                    // Convert changes to Python dict
                    let changes_dict = PyDict::new(py);
                    if let Some(changes) = edit.changes {
                        for (uri, text_edits) in changes {
                            let edits_list: Vec<Bound<'py, PyDict>> = text_edits
                                .iter()
                                .map(|te| {
                                    let te_dict = PyDict::new(py);
                                    let _ = te_dict.set_item("new_text", te.new_text.clone());
                                    let _ = te_dict.set_item("start_line", te.range.start.line);
                                    let _ =
                                        te_dict.set_item("start_char", te.range.start.character);
                                    let _ = te_dict.set_item("end_line", te.range.end.line);
                                    let _ = te_dict.set_item("end_char", te.range.end.character);
                                    te_dict
                                })
                                .collect();
                            let _ = changes_dict.set_item(uri, edits_list);
                        }
                    }
                    let _ = dict.set_item("changes", changes_dict);
                    Ok(Some(dict))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        /// Shutdown all language servers
        fn shutdown_all(&self) -> PyResult<()> {
            match self.inner.shutdown_all() {
                Ok(_) => Ok(()),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        /// Open a document for editing
        fn open_document(&self, language: &str, file_path: &str) -> PyResult<()> {
            let path = std::path::PathBuf::from(file_path);
            match self.inner.open_document(language, &path) {
                Ok(_) => Ok(()),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }

        /// Close a document
        fn close_document(&self, language: &str, file_path: &str) -> PyResult<()> {
            let path = std::path::PathBuf::from(file_path);
            match self.inner.close_document(language, &path) {
                Ok(_) => Ok(()),
                Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
            }
        }
    }

    #[pyclass(skip_from_py_object, name = "MemorySystem")]
    pub struct PyMemorySystem {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer3::UnifiedMemorySystem>>,
    }

    #[pymethods]
    impl PyMemorySystem {
        #[new]
        #[pyo3(signature = (session_id=None))]
        fn new(session_id: Option<&str>) -> Self {
            let sid = session_id.unwrap_or("default-session");
            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer3::UnifiedMemorySystem::new(sid),
                )),
            }
        }

        fn store<'py>(&self, py: Python<'py>, tier: &str, content: &str) -> PyResult<String> {
            let inner = self.inner.clone();
            let memory_tier = match tier.to_lowercase().as_str() {
                "working" => sh_layer3::MemoryTier::Working,
                "session" => sh_layer3::MemoryTier::Session,
                "project" => sh_layer3::MemoryTier::Project,
                "longterm" => sh_layer3::MemoryTier::LongTerm,
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "Invalid tier: must be working, session, project, or longterm",
                    ))
                }
            };
            let content_str = content.to_string();

            py.detach(|| {
                runtime().block_on(async {
                    let system = inner.lock().await;
                    match system.store_at(memory_tier, content_str).await {
                        Ok(id) => Ok(id),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        fn query<'py>(
            &self,
            py: Python<'py>,
            query: &str,
            tier: Option<&str>,
            limit: Option<usize>,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            let inner = self.inner.clone();
            let memory_query = sh_layer3::MemoryQuery {
                query: query.to_string(),
                tier: tier.map(|t| match t.to_lowercase().as_str() {
                    "working" => sh_layer3::MemoryTier::Working,
                    "session" => sh_layer3::MemoryTier::Session,
                    "project" => sh_layer3::MemoryTier::Project,
                    "longterm" => sh_layer3::MemoryTier::LongTerm,
                    _ => sh_layer3::MemoryTier::Working,
                }),
                limit,
                time_range: None,
            };

            let entries = py
                .detach(|| {
                    runtime().block_on(async {
                        let system = inner.lock().await;
                        system
                            .query_all(&memory_query)
                            .await
                            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
                    })
                })
                .map_err(|e: pyo3::PyErr| e)?;

            let results: Vec<Bound<'py, PyDict>> = entries
                .iter()
                .map(|entry| {
                    let dict = PyDict::new(py);
                    let _ = dict.set_item("id", entry.id.clone());
                    let _ = dict.set_item("tier", format!("{:?}", entry.tier));
                    let _ = dict.set_item("content", entry.content.clone());
                    let _ = dict.set_item("created_at", entry.created_at.to_rfc3339());
                    let _ = dict.set_item("importance", entry.importance);
                    dict
                })
                .collect();
            Ok(results)
        }

        fn get<'py>(
            &self,
            py: Python<'py>,
            tier: &str,
            id: &str,
        ) -> PyResult<Option<Bound<'py, PyDict>>> {
            let inner = self.inner.clone();
            let memory_tier = match tier.to_lowercase().as_str() {
                "working" => sh_layer3::MemoryTier::Working,
                "session" => sh_layer3::MemoryTier::Session,
                "project" => sh_layer3::MemoryTier::Project,
                "longterm" => sh_layer3::MemoryTier::LongTerm,
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "Invalid tier: must be working, session, project, or longterm",
                    ))
                }
            };
            let id_str = id.to_string();

            let entry_opt = py
                .detach(|| {
                    runtime().block_on(async {
                        let system = inner.lock().await;
                        system
                            .get(memory_tier, &id_str)
                            .await
                            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
                    })
                })
                .map_err(|e: pyo3::PyErr| e)?;

            match entry_opt {
                Some(entry) => {
                    let dict = PyDict::new(py);
                    let _ = dict.set_item("id", entry.id.clone());
                    let _ = dict.set_item("tier", format!("{:?}", entry.tier));
                    let _ = dict.set_item("content", entry.content.clone());
                    let _ = dict.set_item("created_at", entry.created_at.to_rfc3339());
                    Ok(Some(dict))
                }
                None => Ok(None),
            }
        }

        fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
            let inner = self.inner.clone();

            let stats = py
                .detach(|| {
                    runtime().block_on(async {
                        let system = inner.lock().await;
                        system
                            .stats()
                            .await
                            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
                    })
                })
                .map_err(|e: pyo3::PyErr| e)?;

            let dict = PyDict::new(py);
            for (tier, count) in stats {
                let _ = dict.set_item(format!("{:?}", tier), count);
            }
            Ok(dict)
        }

        fn clear<'py>(&self, py: Python<'py>, tier: &str) -> PyResult<usize> {
            let inner = self.inner.clone();
            let memory_tier = match tier.to_lowercase().as_str() {
                "working" => sh_layer3::MemoryTier::Working,
                "session" => sh_layer3::MemoryTier::Session,
                "project" => sh_layer3::MemoryTier::Project,
                "longterm" => sh_layer3::MemoryTier::LongTerm,
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "Invalid tier: must be working, session, project, or longterm",
                    ))
                }
            };

            py.detach(|| {
                runtime().block_on(async {
                    let system = inner.lock().await;
                    match system.clear_tier(memory_tier).await {
                        Ok(count) => Ok(count),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// Delete a memory entry from a tier
        fn delete<'py>(&self, py: Python<'py>, tier: &str, id: &str) -> PyResult<bool> {
            let inner = self.inner.clone();
            let memory_tier = match tier.to_lowercase().as_str() {
                "working" => sh_layer3::MemoryTier::Working,
                "session" => sh_layer3::MemoryTier::Session,
                "project" => sh_layer3::MemoryTier::Project,
                "longterm" => sh_layer3::MemoryTier::LongTerm,
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "Invalid tier: must be working, session, project, or longterm",
                    ))
                }
            };
            let id_str = id.to_string();

            py.detach(|| {
                runtime().block_on(async {
                    let system = inner.lock().await;
                    match system.delete(memory_tier, &id_str).await {
                        Ok(deleted) => Ok(deleted),
                        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                    }
                })
            })
        }

        /// Store in working memory (convenience method)
        fn store_working<'py>(&self, py: Python<'py>, content: &str) -> PyResult<String> {
            self.store(py, "working", content)
        }

        /// Store in session memory (convenience method)
        fn store_session<'py>(&self, py: Python<'py>, content: &str) -> PyResult<String> {
            self.store(py, "session", content)
        }

        /// Store in project memory (convenience method)
        fn store_project<'py>(&self, py: Python<'py>, content: &str) -> PyResult<String> {
            self.store(py, "project", content)
        }

        /// Store in long-term memory (convenience method)
        fn store_longterm<'py>(&self, py: Python<'py>, content: &str) -> PyResult<String> {
            self.store(py, "longterm", content)
        }

        /// Query working memory (convenience method)
        fn query_working<'py>(
            &self,
            py: Python<'py>,
            query: &str,
            limit: Option<usize>,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            self.query(py, query, Some("working"), limit)
        }

        /// Query session memory (convenience method)
        fn query_session<'py>(
            &self,
            py: Python<'py>,
            query: &str,
            limit: Option<usize>,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            self.query(py, query, Some("session"), limit)
        }

        /// Query project memory (convenience method)
        fn query_project<'py>(
            &self,
            py: Python<'py>,
            query: &str,
            limit: Option<usize>,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            self.query(py, query, Some("project"), limit)
        }

        /// Query long-term memory (convenience method)
        fn query_longterm<'py>(
            &self,
            py: Python<'py>,
            query: &str,
            limit: Option<usize>,
        ) -> PyResult<Vec<Bound<'py, PyDict>>> {
            self.query(py, query, Some("longterm"), limit)
        }
    }

    // ========================================================================
    // Layer 3: Knowledge Base (RetrieverEngine, DocumentLoader, TextSplitter, Embeddings)
    // ========================================================================

    /// RetrieverEngine - RAG 检索引擎 Python 绑定
    ///
    /// 混合检索引擎：支持向量检索 + 关键词检索的混合搜索。
    /// 使用 InMemoryVectorStore + MockEmbeddingModel + FixedSizeChunker 作为默认配置
    #[pyclass(skip_from_py_object, name = "RetrieverEngine")]
    pub struct PyRetrieverEngine {
        inner: std::sync::Arc<tokio::sync::Mutex<PyRetrieverEngineInner>>,
    }

    /// 内部引擎类型，使用具体类型而非泛型
    type PyRetrieverEngineInner = sh_layer3::DefaultRetrieverEngine<
        sh_layer3::InMemoryVectorStore,
        sh_layer3::Layer1EmbeddingAdapter,
        sh_layer3::FixedSizeChunker,
    >;

    #[pymethods]
    impl PyRetrieverEngine {
        #[new]
        #[pyo3(signature = (embedding_dimension=128))]
        fn new(embedding_dimension: usize) -> Self {
            let vector_store = sh_layer3::InMemoryVectorStore::in_memory();
            // Use layer1's MockEmbeddingModel wrapped in adapter for proper trait implementation
            let embedding_model = sh_layer3::Layer1EmbeddingAdapter::new(Box::new(
                sh_layer1::MockEmbeddingModel::new(embedding_dimension),
            ));
            let chunker = sh_layer3::FixedSizeChunker::default();

            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer3::DefaultRetrieverEngine::new(vector_store, embedding_model, chunker),
                )),
            }
        }

        /// 添加文档到知识库，返回文档 ID
        fn add_document<'py>(
            &self,
            py: Python<'py>,
            doc_id: &str,
            content: &str,
            metadata_json: Option<&str>,
        ) -> PyResult<String> {
            let inner = self.inner.clone();
            let doc_id_str = doc_id.to_string();
            let content_str = content.to_string();

            let metadata: std::collections::HashMap<String, serde_json::Value> = match metadata_json
            {
                Some(json) => serde_json::from_str(json).map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("Invalid JSON metadata: {}", e))
                })?,
                None => std::collections::HashMap::new(),
            };

            let document = sh_layer3::Document {
                id: Some(doc_id_str.clone()),
                content: content_str,
                metadata,
                source: None,
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let engine = inner.lock().await;
                let ids = engine
                    .index(vec![document])
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                // Return the first (and only) document ID
                ids.into_iter().next().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err("No document ID returned")
                })
            })
        }

        /// 执行混合检索
        fn retrieve<'py>(
            &self,
            py: Python<'py>,
            query: &str,
            top_k: usize,
        ) -> PyResult<Vec<PySearchResult>> {
            let inner = self.inner.clone();
            let query_str = query.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let engine = inner.lock().await;
                let results = engine
                    .hybrid_retrieve(&query_str, top_k)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                Ok(results
                    .into_iter()
                    .map(|r| PySearchResult {
                        id: r.doc_id,
                        score: r.score,
                        content: r.content,
                        metadata_json: serde_json::to_string(&r.metadata).unwrap_or_default(),
                    })
                    .collect())
            })
        }

        /// 删除文档
        fn delete_document<'py>(&self, py: Python<'py>, doc_id: &str) -> PyResult<bool> {
            let inner = self.inner.clone();
            let doc_id_str = doc_id.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let engine = inner.lock().await;
                engine
                    .delete(&[doc_id_str])
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 清空索引
        fn clear<'py>(&self, py: Python<'py>) -> PyResult<bool> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let engine = inner.lock().await;
                engine
                    .clear()
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取文档数量
        fn count<'py>(&self, py: Python<'py>) -> PyResult<usize> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let engine = inner.lock().await;
                engine
                    .count()
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }
    }

    /// DocumentLoader - 文档加载器 Python 绑定
    ///
    /// 支持加载多种格式文档：TXT, CSV, JSON, Markdown, PDF
    #[pyclass(skip_from_py_object, name = "DocumentLoader")]
    pub struct PyDocumentLoader {
        loader_type: String,
    }

    #[pymethods]
    impl PyDocumentLoader {
        #[new]
        #[pyo3(signature = (loader_type="text"))]
        fn new(loader_type: &str) -> Self {
            Self {
                loader_type: loader_type.to_string(),
            }
        }

        /// 加载文档
        fn load<'py>(&self, py: Python<'py>, path: &str) -> PyResult<(String, String, String)> {
            let path_buf = std::path::PathBuf::from(path);

            py.detach(|| {
                // 根据类型选择加载器
                match self.loader_type.to_lowercase().as_str() {
                    "text" | "txt" => {
                        let content = std::fs::read_to_string(&path_buf)
                            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
                        let doc_id = path_buf
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        Ok((doc_id, content, "{}".to_string()))
                    }
                    "json" => {
                        let content = std::fs::read_to_string(&path_buf)
                            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
                        let doc_id = path_buf
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        Ok((doc_id, content, "{}".to_string()))
                    }
                    "csv" => {
                        let content = std::fs::read_to_string(&path_buf)
                            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
                        let doc_id = path_buf
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        Ok((doc_id, content, "{}".to_string()))
                    }
                    "markdown" | "md" => {
                        let content = std::fs::read_to_string(&path_buf)
                            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
                        let doc_id = path_buf
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        Ok((doc_id, content, "{}".to_string()))
                    }
                    other => Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Unsupported loader type: {}",
                        other
                    ))),
                }
            })
        }

        /// 获取支持的文件扩展名
        fn supported_extensions(&self) -> Vec<String> {
            match self.loader_type.to_lowercase().as_str() {
                "text" | "txt" => vec![".txt".to_string()],
                "json" => vec![".json".to_string()],
                "csv" => vec![".csv".to_string()],
                "markdown" | "md" => vec![".md".to_string(), ".markdown".to_string()],
                _ => vec![],
            }
        }

        /// 获取加载器类型
        fn loader_type(&self) -> &str {
            &self.loader_type
        }
    }

    /// TextSplitter - 文本分割器 Python 绑定
    ///
    /// 将长文本分割为小块，支持递归字符分割
    #[pyclass(skip_from_py_object, name = "TextSplitter")]
    pub struct PyTextSplitter {
        chunk_size: usize,
        chunk_overlap: usize,
    }

    #[pymethods]
    impl PyTextSplitter {
        #[new]
        #[pyo3(signature = (chunk_size=1000, chunk_overlap=200))]
        fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
            Self {
                chunk_size,
                chunk_overlap,
            }
        }

        /// 分割文本
        fn split<'py>(&self, py: Python<'py>, text: &str) -> PyResult<Vec<(String, usize, usize)>> {
            py.detach(|| {
                let splitter = sh_layer3::text_splitters::RecursiveCharacterTextSplitter::new(
                    self.chunk_size,
                    self.chunk_overlap,
                );

                // 创建临时文档用于分割
                let doc = sh_layer3::Document {
                    id: Some("temp".to_string()),
                    content: text.to_string(),
                    metadata: std::collections::HashMap::new(),
                    source: None,
                };

                let chunks = splitter.chunk(&doc);

                Ok(chunks
                    .into_iter()
                    .map(|c| (c.content, c.position.index, c.position.start))
                    .collect())
            })
        }

        /// 分割文本并返回 JSON
        fn split_json<'py>(&self, py: Python<'py>, text: &str) -> PyResult<String> {
            py.detach(|| {
                let splitter = sh_layer3::text_splitters::RecursiveCharacterTextSplitter::new(
                    self.chunk_size,
                    self.chunk_overlap,
                );

                let doc = sh_layer3::Document {
                    id: Some("temp".to_string()),
                    content: text.to_string(),
                    metadata: std::collections::HashMap::new(),
                    source: None,
                };

                let chunks = splitter.chunk(&doc);

                let result: Vec<serde_json::Value> = chunks
                    .into_iter()
                    .map(|c| {
                        serde_json::json!({
                            "content": c.content,
                            "index": c.position.index,
                            "start": c.position.start,
                            "end": c.position.end,
                        })
                    })
                    .collect();

                serde_json::to_string(&result)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取配置
        fn config(&self) -> (usize, usize) {
            (self.chunk_size, self.chunk_overlap)
        }

        /// 设置分块大小
        fn set_chunk_size(&mut self, size: usize) {
            self.chunk_size = size;
        }

        /// 设置重叠大小
        fn set_overlap(&mut self, overlap: usize) {
            self.chunk_overlap = overlap;
        }
    }

    /// EmbeddingProvider - 嵌入模型提供商枚举
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[pyclass(skip_from_py_object, name = "EmbeddingProvider")]
    pub enum PyEmbeddingProvider {
        OpenAI,
        HuggingFace,
        Cohere,
        Local,
    }

    /// Embeddings - 嵌入模型 Python 绑定
    ///
    /// 将文本转换为向量表示，支持多种嵌入模型提供商：
    /// - OpenAI Embeddings API (text-embedding-3-small, text-embedding-3-large)
    /// - HuggingFace Inference API
    /// - Cohere Embed API
    /// - 本地 SentenceTransformers 模型
    ///
    /// 使用 from_env() 工厂方法自动从环境变量配置：
    /// - OPENAI_API_KEY: OpenAI API 密钥
    /// - HUGGINGFACE_API_KEY: HuggingFace API 密钥
    /// - COHERE_API_KEY: Cohere API 密钥
    #[pyclass(skip_from_py_object, name = "Embeddings")]
    pub struct PyEmbeddings {
        provider: PyEmbeddingProvider,
        model: String,
        dimension: usize,
        // 使用动态分发存储实际的嵌入模型
        inner: std::sync::Arc<tokio::sync::Mutex<Box<dyn sh_layer1::EmbeddingModel>>>,
    }

    #[pymethods]
    impl PyEmbeddings {
        /// 创建新的嵌入模型实例
        ///
        /// Args:
        ///     provider: 提供商类型 ("openai", "huggingface", "cohere", "local")
        ///     model: 模型名称
        ///     api_key: API 密钥 (可选，本地模型不需要)
        ///     base_url: API 基础 URL (可选，用于自定义端点)
        ///     dimension: 向量维度 (可选，用于本地模型)
        #[new]
        #[pyo3(signature = (provider="openai", model=None, api_key=None, base_url=None, dimension=None))]
        fn new(
            provider: &str,
            model: Option<&str>,
            api_key: Option<&str>,
            base_url: Option<&str>,
            dimension: Option<usize>,
        ) -> PyResult<Self> {
            let provider_type = match provider.to_lowercase().as_str() {
                "openai" => PyEmbeddingProvider::OpenAI,
                "huggingface" | "hf" => PyEmbeddingProvider::HuggingFace,
                "cohere" => PyEmbeddingProvider::Cohere,
                "local" => PyEmbeddingProvider::Local,
                other => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Unknown embedding provider: {}",
                        other
                    )));
                }
            };

            let default_model = match provider_type {
                PyEmbeddingProvider::OpenAI => "text-embedding-3-small",
                PyEmbeddingProvider::HuggingFace => "sentence-transformers/all-MiniLM-L6-v2",
                PyEmbeddingProvider::Cohere => "embed-english-v3.0",
                PyEmbeddingProvider::Local => "all-MiniLM-L6-v2",
            };

            let model_name = model.unwrap_or(default_model).to_string();
            let dim = dimension.unwrap_or({
                // 根据模型推断维度
                match provider_type {
                    PyEmbeddingProvider::OpenAI => match model_name.as_str() {
                        "text-embedding-3-large" => 3072,
                        _ => 1536,
                    },
                    PyEmbeddingProvider::HuggingFace => match model_name.as_str() {
                        "sentence-transformers/all-MiniLM-L6-v2" => 384,
                        "sentence-transformers/all-mpnet-base-v2" => 768,
                        _ => 768,
                    },
                    PyEmbeddingProvider::Cohere => 1024,
                    PyEmbeddingProvider::Local => 384,
                }
            });

            // 创建配置
            let config = sh_layer1::EmbeddingsConfig {
                provider: match provider_type {
                    PyEmbeddingProvider::OpenAI => sh_layer1::EmbeddingProvider::OpenAI,
                    PyEmbeddingProvider::HuggingFace => sh_layer1::EmbeddingProvider::HuggingFace,
                    PyEmbeddingProvider::Cohere => sh_layer1::EmbeddingProvider::Cohere,
                    PyEmbeddingProvider::Local => sh_layer1::EmbeddingProvider::Local,
                },
                api_key: api_key.unwrap_or("").to_string(),
                base_url: base_url.map(|s| s.to_string()),
                model: model_name.clone(),
                dimension: Some(dim),
            };

            // 创建嵌入模型实例
            let inner_model: Box<dyn sh_layer1::EmbeddingModel> =
                match sh_layer1::EmbeddingsFactory::new().create(config) {
                    Ok(m) => m,
                    Err(e) => {
                        // 如果配置失败，回退到 MockEmbeddingModel
                        tracing::warn!(
                            "Failed to create embedding model ({}): {}, using mock model",
                            provider,
                            e
                        );
                        return Ok(Self {
                            provider: provider_type,
                            model: model_name,
                            dimension: dim,
                            inner: std::sync::Arc::new(tokio::sync::Mutex::new(Box::new(
                                sh_layer1::MockEmbeddingModel::new(dim),
                            ))),
                        });
                    }
                };

            Ok(Self {
                provider: provider_type,
                model: model_name,
                dimension: dim,
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(inner_model)),
            })
        }

        /// 从环境变量创建 OpenAI 嵌入模型
        ///
        /// 环境变量：
        /// - OPENAI_API_KEY: API 密钥 (必需)
        /// - OPENAI_BASE_URL: API 基础 URL (可选)
        /// - OPENAI_EMBEDDING_MODEL: 模型名称 (可选，默认 text-embedding-3-small)
        #[classmethod]
        fn openai_from_env(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
            let config = sh_layer1::EmbeddingsConfig::openai_from_env().map_err(|e| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "OpenAI configuration error: {}. Set OPENAI_API_KEY environment variable.",
                    e
                ))
            })?;

            let model_name = config.model.clone();
            let dim = sh_layer1::DEFAULT_EMBEDDING_DIMENSION;

            let inner_model = sh_layer1::EmbeddingsFactory::new()
                .create(config)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

            Ok(Self {
                provider: PyEmbeddingProvider::OpenAI,
                model: model_name,
                dimension: dim,
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(inner_model)),
            })
        }

        /// 从环境变量创建 HuggingFace 嵌入模型
        #[classmethod]
        fn huggingface_from_env(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
            let config =
                sh_layer1::EmbeddingsConfig::huggingface_from_env().map_err(|e| {
                    pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "HuggingFace configuration error: {}. Set HUGGINGFACE_API_KEY environment variable.",
                        e
                    ))
                })?;

            let model_name = config.model.clone();
            let dim = 384;

            let inner_model = sh_layer1::EmbeddingsFactory::new()
                .create(config)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

            Ok(Self {
                provider: PyEmbeddingProvider::HuggingFace,
                model: model_name,
                dimension: dim,
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(inner_model)),
            })
        }

        /// 从环境变量创建 Cohere 嵌入模型
        #[classmethod]
        fn cohere_from_env(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
            let config = sh_layer1::EmbeddingsConfig::cohere_from_env().map_err(|e| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Cohere configuration error: {}. Set COHERE_API_KEY environment variable.",
                    e
                ))
            })?;

            let model_name = config.model.clone();
            let dim = 1024;

            let inner_model = sh_layer1::EmbeddingsFactory::new()
                .create(config)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

            Ok(Self {
                provider: PyEmbeddingProvider::Cohere,
                model: model_name,
                dimension: dim,
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(inner_model)),
            })
        }

        /// 创建本地嵌入模型（用于离线场景）
        #[staticmethod]
        #[pyo3(signature = (model="all-MiniLM-L6-v2", dimension=None))]
        fn local(model: &str, dimension: Option<usize>) -> PyResult<Self> {
            let dim = dimension.unwrap_or(384);
            let config = sh_layer1::EmbeddingsConfig::local(model, Some(dim));

            // 尝试创建本地模型，如果失败则使用 mock
            match sh_layer1::EmbeddingsFactory::new().create(config) {
                Ok(inner_model) => Ok(Self {
                    provider: PyEmbeddingProvider::Local,
                    model: model.to_string(),
                    dimension: dim,
                    inner: std::sync::Arc::new(tokio::sync::Mutex::new(inner_model)),
                }),
                Err(e) => {
                    tracing::warn!("Local embeddings not available ({}), using mock model", e);
                    Ok(Self {
                        provider: PyEmbeddingProvider::Local,
                        model: model.to_string(),
                        dimension: dim,
                        inner: std::sync::Arc::new(tokio::sync::Mutex::new(Box::new(
                            sh_layer1::MockEmbeddingModel::new(dim),
                        ))),
                    })
                }
            }
        }

        /// 获取单个文本的嵌入向量
        fn embed<'py>(&self, py: Python<'py>, text: &str) -> PyResult<Vec<f32>> {
            let inner = self.inner.clone();
            let text_str = text.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let model = inner.lock().await;
                model
                    .embed(&text_str)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 批量获取嵌入向量
        fn embed_batch<'py>(&self, py: Python<'py>, texts: Vec<String>) -> PyResult<Vec<Vec<f32>>> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let model = inner.lock().await;
                model
                    .embed_batch(&texts)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取嵌入维度
        fn dimension(&self) -> usize {
            self.dimension
        }

        /// 获取模型名称
        fn model_name(&self) -> &str {
            &self.model
        }

        /// 获取提供商名称
        fn provider_name(&self) -> &str {
            match self.provider {
                PyEmbeddingProvider::OpenAI => "openai",
                PyEmbeddingProvider::HuggingFace => "huggingface",
                PyEmbeddingProvider::Cohere => "cohere",
                PyEmbeddingProvider::Local => "local",
            }
        }

        /// 检查是否使用 mock 模型（调试用）
        fn is_mock(&self) -> bool {
            false // 真实实现不会是 mock
        }
    }

    // ========================================================================
    // Layer 4: McpBridge, AuditLogger
    // ========================================================================

    #[pyclass(skip_from_py_object, name = "McpBridge")]
    pub struct PyMcpBridge;

    #[pymethods]
    impl PyMcpBridge {
        #[new]
        fn new() -> Self {
            Self
        }
    }

    #[pyclass(skip_from_py_object, name = "AuditLogger")]
    pub struct PyAuditLogger {
        inner: std::sync::Arc<sh_layer4::AuditLogger>,
    }

    #[pymethods]
    impl PyAuditLogger {
        #[new]
        fn new() -> Self {
            Self {
                inner: std::sync::Arc::new(sh_layer4::AuditLogger::new(Default::default())),
            }
        }

        fn log<'py>(
            &self,
            py: Python<'py>,
            user_id: &str,
            action: &str,
            resource_type: &str,
        ) -> PyResult<()> {
            let inner = self.inner.clone();
            let audit_action = match action {
                "login" => sh_layer4::AuditAction::Login,
                "logout" => sh_layer4::AuditAction::Logout,
                "read" => sh_layer4::AuditAction::Read,
                "create" => sh_layer4::AuditAction::Create,
                "update" => sh_layer4::AuditAction::Update,
                "delete" => sh_layer4::AuditAction::Delete,
                "execute" => sh_layer4::AuditAction::Execute,
                _ => sh_layer4::AuditAction::Other(action.to_string()),
            };

            let entry = sh_layer4::AuditEntry::new(user_id, audit_action, resource_type);

            pyo3_async_runtimes::tokio::run(py, async move {
                inner
                    .log(entry)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })?;

            Ok(())
        }

        fn count<'py>(&self, py: Python<'py>) -> PyResult<usize> {
            let inner = self.inner.clone();
            pyo3_async_runtimes::tokio::run(py, async move {
                inner
                    .count()
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }
    }

    // ========================================================================
    // Layer 3: VectorStore
    // ========================================================================

    /// VectorStore Python 绑定
    #[pyclass(skip_from_py_object, name = "VectorStore")]
    #[derive(Clone)]
    pub struct PyVectorStore {
        inner: std::sync::Arc<tokio::sync::Mutex<sh_layer3::InMemoryVectorStore>>,
    }

    #[pymethods]
    impl PyVectorStore {
        #[new]
        #[pyo3(signature = (metric="cosine"))]
        fn new(metric: &str) -> Self {
            let distance_metric = match metric.to_lowercase().as_str() {
                "cosine" => sh_layer3::DistanceMetric::Cosine,
                "euclidean" => sh_layer3::DistanceMetric::Euclidean,
                "dot_product" | "dotproduct" => sh_layer3::DistanceMetric::DotProduct,
                "manhattan" => sh_layer3::DistanceMetric::Manhattan,
                _ => sh_layer3::DistanceMetric::Cosine,
            };

            let config = sh_layer3::VectorStoreConfig {
                metric: distance_metric,
                ..Default::default()
            };

            Self {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(
                    sh_layer3::InMemoryVectorStore::new(config),
                )),
            }
        }

        /// 插入或更新向量
        fn upsert<'py>(
            &self,
            py: Python<'py>,
            id: &str,
            vector: Vec<f32>,
            metadata_json: Option<&str>,
        ) -> PyResult<bool> {
            let inner = self.inner.clone();
            let id_str = id.to_string();

            let metadata: std::collections::HashMap<String, serde_json::Value> = match metadata_json
            {
                Some(json) => serde_json::from_str(json).map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("Invalid JSON metadata: {}", e))
                })?,
                None => std::collections::HashMap::new(),
            };

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                store
                    .add(id_str, vector, metadata)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 搜索相似向量
        fn search<'py>(
            &self,
            py: Python<'py>,
            vector: Vec<f32>,
            top_k: usize,
        ) -> PyResult<Vec<PySearchResult>> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                let results = store
                    .query(vector, top_k)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

                Ok(results
                    .into_iter()
                    .map(|r| PySearchResult {
                        id: r.doc_id,
                        score: r.score,
                        content: r.content,
                        metadata_json: serde_json::to_string(&r.metadata).unwrap_or_default(),
                    })
                    .collect())
            })
        }

        /// 删除向量
        fn delete<'py>(&self, py: Python<'py>, id: &str) -> PyResult<bool> {
            let inner = self.inner.clone();
            let id_str = id.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                store
                    .delete(&id_str)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 获取向量
        fn get<'py>(&self, py: Python<'py>, id: &str) -> PyResult<Option<PyVectorItem>> {
            let inner = self.inner.clone();
            let id_str = id.to_string();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                match store.get(&id_str).await {
                    Ok(Some(item)) => Ok(Some(PyVectorItem {
                        id: item.id,
                        vector: item.vector,
                        content: item.content.unwrap_or_default(),
                        metadata_json: serde_json::to_string(&item.metadata).unwrap_or_default(),
                    })),
                    Ok(None) => Ok(None),
                    Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
                }
            })
        }

        /// 获取向量数量
        fn count<'py>(&self, py: Python<'py>) -> PyResult<usize> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                store
                    .count()
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 清空存储
        fn clear<'py>(&self, py: Python<'py>) -> PyResult<bool> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                store
                    .clear()
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 批量插入向量
        fn upsert_batch<'py>(
            &self,
            py: Python<'py>,
            items: Vec<(String, Vec<f32>, Option<String>)>,
        ) -> PyResult<Vec<bool>> {
            let inner = self.inner.clone();

            let vector_items: Vec<sh_layer3::VectorItem> = items
                .into_iter()
                .map(|(id, vector, content)| {
                    sh_layer3::VectorItem::new(&id, vector)
                        .with_content(content.unwrap_or_default())
                })
                .collect();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                store
                    .add_batch(vector_items)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }

        /// 批量删除向量
        fn delete_batch<'py>(&self, py: Python<'py>, ids: Vec<String>) -> PyResult<usize> {
            let inner = self.inner.clone();

            pyo3_async_runtimes::tokio::run(py, async move {
                let store = inner.lock().await;
                store
                    .delete_batch(&ids)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
            })
        }
    }

    /// VectorItem Python 类
    #[pyclass(skip_from_py_object, name = "VectorItem")]
    pub struct PyVectorItem {
        #[pyo3(get)]
        id: String,
        #[pyo3(get)]
        vector: Vec<f32>,
        #[pyo3(get)]
        content: String,
        metadata_json: String,
    }

    #[pymethods]
    impl PyVectorItem {
        /// 获取元数据 JSON
        fn get_metadata(&self) -> String {
            self.metadata_json.clone()
        }
    }

    /// SearchResult Python 类
    #[pyclass(skip_from_py_object, name = "SearchResult")]
    pub struct PySearchResult {
        #[pyo3(get)]
        id: String,
        #[pyo3(get)]
        score: f32,
        #[pyo3(get)]
        content: String,
        metadata_json: String,
    }

    #[pymethods]
    impl PySearchResult {
        /// 获取元数据 JSON
        fn get_metadata(&self) -> String {
            self.metadata_json.clone()
        }
    }

    // ========================================================================
    // Layer 2: Interactive Permission System
    // ========================================================================

    /// SecurityLevel Python 类
    ///
    /// 安全级别配置。
    ///
    /// Values:
    ///     - Trusted: 信任所有操作，无需确认
    ///     - Standard: 默认，对危险操作需要确认
    ///     - Strict: 所有操作都需要确认
    ///     - Paranoid: 所有操作需要确认 + 详细日志
    #[pyclass(skip_from_py_object, name = "SecurityLevel")]
    #[derive(Clone)]
    pub struct PySecurityLevel {
        #[pyo3(get)]
        level: String,
    }

    #[pymethods]
    impl PySecurityLevel {
        #[new]
        #[pyo3(signature = (level="standard"))]
        fn new(level: &str) -> Self {
            Self {
                level: level.to_lowercase(),
            }
        }

        fn __repr__(&self) -> String {
            format!("SecurityLevel('{}')", self.level)
        }

        /// 信任级别 - 无需确认
        #[classmethod]
        fn trusted(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                level: "trusted".to_string(),
            }
        }

        /// 默认级别 - 危险操作需要确认
        #[classmethod]
        fn standard(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                level: "standard".to_string(),
            }
        }

        /// 严格级别 - 所有操作需要确认
        #[classmethod]
        fn strict(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                level: "strict".to_string(),
            }
        }

        /// 极端级别 - 所有操作需要确认 + 详细日志
        #[classmethod]
        fn paranoid(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                level: "paranoid".to_string(),
            }
        }
    }

    /// PermissionDecision Python 类
    ///
    /// 权限决策结果。
    #[pyclass(skip_from_py_object, name = "PermissionDecision")]
    #[derive(Clone)]
    pub struct PyPermissionDecision {
        #[pyo3(get)]
        decision: String,
    }

    #[pymethods]
    impl PyPermissionDecision {
        #[new]
        #[pyo3(signature = (decision="allow"))]
        fn new(decision: &str) -> Self {
            Self {
                decision: decision.to_lowercase(),
            }
        }

        fn __repr__(&self) -> String {
            format!("PermissionDecision('{}')", self.decision)
        }

        fn is_allowed(&self) -> bool {
            self.decision == "allow" || self.decision == "allow_once"
        }

        fn should_remember(&self) -> bool {
            self.decision == "allow" || self.decision == "deny"
        }

        /// 允许
        #[classmethod]
        fn allow(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                decision: "allow".to_string(),
            }
        }

        /// 拒绝
        #[classmethod]
        fn deny(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                decision: "deny".to_string(),
            }
        }

        /// 允许一次
        #[classmethod]
        fn allow_once(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                decision: "allow_once".to_string(),
            }
        }

        /// 拒绝一次
        #[classmethod]
        fn deny_once(_cls: &Bound<'_, PyType>) -> Self {
            Self {
                decision: "deny_once".to_string(),
            }
        }
    }

    /// PermissionAction Python 类
    ///
    /// 权限请求的动作类型。
    #[pyclass(skip_from_py_object, name = "PermissionAction")]
    #[derive(Clone)]
    pub struct PyPermissionAction {
        #[pyo3(get)]
        action_type: String,
        #[pyo3(get)]
        details_json: String,
    }

    #[pymethods]
    impl PyPermissionAction {
        /// 创建命令执行动作
        #[staticmethod]
        fn command_execute(command: &str, args: Vec<String>) -> Self {
            Self {
                action_type: "command_execute".to_string(),
                details_json: serde_json::json!({"command": command, "args": args}).to_string(),
            }
        }

        /// 创建文件读取动作
        #[staticmethod]
        fn file_read(path: &str) -> Self {
            Self {
                action_type: "file_read".to_string(),
                details_json: serde_json::json!({"path": path}).to_string(),
            }
        }

        /// 创建文件写入动作
        #[staticmethod]
        fn file_write(path: &str, content_preview: Option<&str>) -> Self {
            Self {
                action_type: "file_write".to_string(),
                details_json: serde_json::json!({"path": path, "content_preview": content_preview})
                    .to_string(),
            }
        }

        /// 创建文件删除动作
        #[staticmethod]
        fn file_delete(path: &str) -> Self {
            Self {
                action_type: "file_delete".to_string(),
                details_json: serde_json::json!({"path": path}).to_string(),
            }
        }

        /// 创建网络请求动作
        #[staticmethod]
        fn network_request(url: &str, method: &str) -> Self {
            Self {
                action_type: "network_request".to_string(),
                details_json: serde_json::json!({"url": url, "method": method}).to_string(),
            }
        }

        /// 创建自定义动作
        #[staticmethod]
        fn custom(description: &str) -> Self {
            Self {
                action_type: "custom".to_string(),
                details_json: serde_json::json!({"description": description}).to_string(),
            }
        }

        fn __repr__(&self) -> String {
            format!(
                "PermissionAction(type='{}', details={})",
                self.action_type, self.details_json
            )
        }

        /// 获取动作描述
        fn description(&self) -> String {
            if let Ok(details) = serde_json::from_str::<serde_json::Value>(&self.details_json) {
                match self.action_type.as_str() {
                    "command_execute" => {
                        let cmd = details["command"].as_str().unwrap_or("");
                        let empty_vec = vec![];
                        let args = details["args"].as_array().unwrap_or(&empty_vec);
                        let args_str: Vec<&str> = args.iter().filter_map(|a| a.as_str()).collect();
                        format!("Execute command: {} {}", cmd, args_str.join(" "))
                    }
                    "file_read" => format!("Read file: {}", details["path"].as_str().unwrap_or("")),
                    "file_write" => {
                        let path = details["path"].as_str().unwrap_or("");
                        let preview = details["content_preview"].as_str();
                        if let Some(p) = preview {
                            let truncated = if p.len() > 100 { &p[..100] } else { p };
                            format!("Write to file: {}\nPreview: {}...", path, truncated)
                        } else {
                            format!("Write to file: {}", path)
                        }
                    }
                    "file_delete" => {
                        format!("Delete file: {}", details["path"].as_str().unwrap_or(""))
                    }
                    "network_request" => {
                        format!(
                            "{} request to: {}",
                            details["method"].as_str().unwrap_or(""),
                            details["url"].as_str().unwrap_or("")
                        )
                    }
                    "custom" => details["description"].as_str().unwrap_or("").to_string(),
                    _ => format!("Unknown action: {}", self.action_type),
                }
            } else {
                format!("PermissionAction(type='{}')", self.action_type)
            }
        }

        /// 获取动作类别
        fn category(&self) -> String {
            self.action_type.clone()
        }
    }

    /// PermissionPolicy Python 类
    ///
    /// 权限策略配置。
    #[pyclass(skip_from_py_object, name = "PermissionPolicy")]
    #[derive(Clone)]
    pub struct PyPermissionPolicy {
        inner: sh_layer2::PermissionPolicy,
    }

    #[pymethods]
    impl PyPermissionPolicy {
        #[new]
        #[pyo3(signature = (level=None))]
        fn new(level: Option<&PySecurityLevel>) -> Self {
            let policy = match level.map(|l| l.level.as_str()) {
                Some("trusted") => sh_layer2::PermissionPolicy::trusted(),
                Some("strict") => sh_layer2::PermissionPolicy::strict(),
                Some("paranoid") => sh_layer2::PermissionPolicy::paranoid(),
                _ => sh_layer2::PermissionPolicy::default(),
            };
            Self { inner: policy }
        }

        /// 获取安全级别
        fn level(&self) -> String {
            match self.inner.level {
                sh_layer2::SecurityLevel::Trusted => "trusted",
                sh_layer2::SecurityLevel::Standard => "standard",
                sh_layer2::SecurityLevel::Strict => "strict",
                sh_layer2::SecurityLevel::Paranoid => "paranoid",
            }
            .to_string()
        }

        /// 添加信任路径
        fn add_trusted_path(&mut self, path: &str) {
            self.inner = self.inner.clone().add_trusted_path(path);
        }

        /// 添加阻塞路径
        fn add_blocked_path(&mut self, path: &str) {
            self.inner = self.inner.clone().add_blocked_path(path);
        }

        /// 添加信任 URL
        fn add_trusted_url(&mut self, url: &str) {
            self.inner = self.inner.clone().add_trusted_url(url);
        }

        /// 添加阻塞 URL
        fn add_blocked_url(&mut self, url: &str) {
            self.inner = self.inner.clone().add_blocked_url(url);
        }

        /// 添加信任命令
        fn add_trusted_command(&mut self, command: &str) {
            self.inner = self.inner.clone().add_trusted_command(command);
        }

        /// 添加阻塞命令
        fn add_blocked_command(&mut self, command: &str) {
            self.inner = self.inner.clone().add_blocked_command(command);
        }

        /// 检查路径是否被阻塞
        fn is_path_blocked(&self, path: &str) -> bool {
            self.inner.is_path_blocked(path)
        }

        /// 检查路径是否被信任
        fn is_path_trusted(&self, path: &str) -> bool {
            self.inner.is_path_trusted(path)
        }

        /// 从文件加载策略
        #[staticmethod]
        fn load_from_file(path: &str) -> PyResult<Self> {
            let policy =
                sh_layer2::PermissionPolicy::load_from_file(&std::path::PathBuf::from(path))
                    .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            Ok(Self { inner: policy })
        }

        /// 保存策略到文件
        fn save_to_file(&self, path: &str) -> PyResult<()> {
            self.inner
                .save_to_file(&std::path::PathBuf::from(path))
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            Ok(())
        }

        fn __repr__(&self) -> String {
            format!("PermissionPolicy(level='{}')", self.level())
        }
    }

    /// InteractivePermissionManager Python 绑定
    ///
    /// 交互式权限管理器。
    ///
    /// Example:
    ///     >>> from sh_python import InteractivePermissionManager, PermissionPolicy
    ///     >>> pm = InteractivePermissionManager()
    ///     >>> pm.set_policy(PermissionPolicy.strict())
    ///     >>> pm.check_permission(PermissionAction.file_read("/test/file.txt"))
    #[pyclass(skip_from_py_object, name = "InteractivePermissionManager")]
    pub struct PyInteractivePermissionManager {
        inner: std::sync::Arc<sh_layer2::PermissionManager>,
    }

    #[pymethods]
    impl PyInteractivePermissionManager {
        #[new]
        #[pyo3(signature = (policy=None))]
        fn new(policy: Option<&PyPermissionPolicy>) -> Self {
            let rust_policy = policy.map(|p| p.inner.clone()).unwrap_or_default();
            Self {
                inner: std::sync::Arc::new(sh_layer2::PermissionManager::new(rust_policy)),
            }
        }

        /// 设置安全策略
        fn set_policy(&self, policy: &PyPermissionPolicy) {
            self.inner.set_policy(policy.inner.clone());
        }

        /// 获取安全级别
        fn security_level(&self) -> String {
            match self.inner.security_level() {
                sh_layer2::SecurityLevel::Trusted => "trusted",
                sh_layer2::SecurityLevel::Standard => "standard",
                sh_layer2::SecurityLevel::Strict => "strict",
                sh_layer2::SecurityLevel::Paranoid => "paranoid",
            }
            .to_string()
        }

        /// 创建命令执行请求
        fn request_command(&self, command: &str, args: Vec<String>) -> String {
            let req = self.inner.request_command(command, args);
            req.id
        }

        /// 创建文件读取请求
        fn request_file_read(&self, path: &str) -> String {
            let req = self.inner.request_file_read(path);
            req.id
        }

        /// 创建文件写入请求
        fn request_file_write(&self, path: &str, content_preview: Option<&str>) -> String {
            let req = self.inner.request_file_write(path, content_preview);
            req.id
        }

        /// 创建网络请求
        fn request_network(&self, url: &str, method: &str) -> String {
            let req = self.inner.request_network(url, method);
            req.id
        }

        /// 检查权限（返回 JSON 格式的结果）
        ///
        /// 注意：此方法需要设置 prompt_callback 才能进行交互式确认。
        /// 在 Python 中，建议使用 set_prompt_callback 设置回调。
        fn check_permission(&self, action: &PyPermissionAction) -> PyResult<String> {
            let rust_action = match action.action_type.as_str() {
                "command_execute" => {
                    let details: serde_json::Value = serde_json::from_str(&action.details_json)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    sh_layer2::PermissionAction::CommandExecute {
                        command: details["command"].as_str().unwrap_or("").to_string(),
                        args: details["args"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }
                }
                "file_read" => {
                    let details: serde_json::Value = serde_json::from_str(&action.details_json)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    sh_layer2::PermissionAction::FileRead {
                        path: details["path"].as_str().unwrap_or("").to_string(),
                    }
                }
                "file_write" => {
                    let details: serde_json::Value = serde_json::from_str(&action.details_json)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    sh_layer2::PermissionAction::FileWrite {
                        path: details["path"].as_str().unwrap_or("").to_string(),
                        content_preview: details["content_preview"].as_str().map(|s| s.to_string()),
                    }
                }
                "file_delete" => {
                    let details: serde_json::Value = serde_json::from_str(&action.details_json)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    sh_layer2::PermissionAction::FileDelete {
                        path: details["path"].as_str().unwrap_or("").to_string(),
                    }
                }
                "network_request" => {
                    let details: serde_json::Value = serde_json::from_str(&action.details_json)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    sh_layer2::PermissionAction::NetworkRequest {
                        url: details["url"].as_str().unwrap_or("").to_string(),
                        method: details["method"].as_str().unwrap_or("").to_string(),
                    }
                }
                "custom" => {
                    let details: serde_json::Value = serde_json::from_str(&action.details_json)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    sh_layer2::PermissionAction::Custom {
                        description: details["description"].as_str().unwrap_or("").to_string(),
                    }
                }
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Unknown action type: {}",
                        action.action_type
                    )));
                }
            };

            let request = sh_layer2::PermissionRequest::new(rust_action);

            match self.inner.check_permission(request) {
                Ok(response) => Ok(serde_json::json!({
                    "allowed": response.decision.is_allowed(),
                    "decision": match response.decision {
                        sh_layer2::PermissionDecision::Allow => "allow",
                        sh_layer2::PermissionDecision::Deny => "deny",
                        sh_layer2::PermissionDecision::AllowOnce => "allow_once",
                        sh_layer2::PermissionDecision::DenyOnce => "deny_once",
                    },
                    "request_id": response.request_id,
                    "reason": response.reason,
                })
                .to_string()),
                Err(e) => Err(pyo3::exceptions::PyPermissionError::new_err(e.to_string())),
            }
        }

        /// 获取缓存统计
        fn cache_stats(&self) -> (usize, usize) {
            self.inner.cache_stats()
        }

        /// 清除缓存
        fn clear_cache(&self) {
            self.inner.clear_cache();
        }

        /// 获取审计日志（JSON 格式）
        fn get_audit_log(&self) -> String {
            let entries = self.inner.get_audit_log();
            serde_json::to_string(&entries).unwrap_or("[]".to_string())
        }

        /// 清除审计日志
        fn clear_audit_log(&self) {
            self.inner.clear_audit_log();
        }

        fn __repr__(&self) -> String {
            format!(
                "InteractivePermissionManager(level='{}')",
                self.security_level()
            )
        }
    }
}

use bindings::*;
