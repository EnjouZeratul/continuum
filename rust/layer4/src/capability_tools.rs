//! Capability installation tools — the agent's self-evolution surface.
//!
//! `install_capability` lets the agent author a new tool (as WAT text or a
//! WASM binary), which is compiled, sandboxed, smoke-tested, and only then
//! registered in the live `ToolRegistry`. `uninstall_capability` and
//! `list_dynamic_tools` complete the lifecycle.
//!
//! ## Pipeline (install)
//!
//! 1. **Validate** — name is hostile input (see [`validate_tool_name`]);
//!    code size is capped; schema must be a JSON object.
//! 2. **Policy gate** — [`SelfModificationPolicy::decide`] returns
//!    Allow / Deny / RequiresApproval. Deny is final; approval is relayed
//!    through the `approved` argument (the human consent channel).
//! 3. **Compile** — WAT is compiled in-process by wasmtime. No external
//!    toolchain, no shell. A module that imports host functions (WASI,
//!    etc.) fails instantiation under the sandboxed capability set — by
//!    design: dynamic tools are pure compute (no fs/net/process).
//! 4. **Smoke test** — the tool executes once with `test_input` and must
//!    return valid JSON before it is registered. Failure unloads cleanly.
//! 5. **Register** — the plugin is wrapped in [`PluginToolAdapter`] and
//!    inserted into the runtime-mutable registry.
//! 6. **Record** — provenance (format, timestamp, smoke output) is kept
//!    for audit and uninstall.

use crate::plugin_loader::capabilities::CapabilitySet;
use crate::plugin_loader::tool_adapter::PluginToolAdapter;
use crate::plugin_loader::{Plugin, WasmLoader};
use crate::types::Layer4Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use sh_layer2::{Layer2Result, Tool as Layer2Tool, ToolRegistry, ToolRegistryTrait, ToolResult};
use sh_safety::self_mod_policy::{
    validate_tool_name, SelfModAction, SelfModDecision, SelfModificationPolicy,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Upper bound on submitted tool code (WAT text or base64 WASM).
/// Bounds compile cost — agent-submitted code is untrusted input.
pub const MAX_CODE_BYTES: usize = 256 * 1024;

/// Source format of an agent-authored tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityFormat {
    /// WebAssembly Text format — compiled in-process by wasmtime.
    Wat,
    /// Base64-encoded WASM binary.
    WasmBase64,
}

impl CapabilityFormat {
    fn parse(s: &str) -> Layer4Result<Self> {
        match s {
            "wat" => Ok(Self::Wat),
            "wasm_base64" => Ok(Self::WasmBase64),
            other => Err(anyhow::anyhow!(
                "unknown format '{}': expected \"wat\" or \"wasm_base64\"",
                other
            )),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Wat => "wat",
            Self::WasmBase64 => "wasm_base64",
        }
    }
}

/// Provenance record for one installed dynamic tool.
#[derive(Debug, Clone)]
pub struct InstalledCapability {
    pub name: String,
    pub description: String,
    pub format: CapabilityFormat,
    pub parameters_schema: serde_json::Value,
    pub installed_at: DateTime<Utc>,
    /// Output of the mandatory smoke test run at install time.
    pub smoke_test_output: serde_json::Value,
}

/// Manages the dynamic-capability lifecycle against a live registry.
pub struct CapabilityManager {
    loader: Arc<WasmLoader>,
    registry: Arc<ToolRegistry>,
    policy: RwLock<SelfModificationPolicy>,
    installed: RwLock<HashMap<String, InstalledCapability>>,
}

impl CapabilityManager {
    /// Create a manager with the safe-default policy.
    pub fn new(registry: Arc<ToolRegistry>) -> Layer4Result<Self> {
        Self::with_policy(registry, SelfModificationPolicy::safe_default())
    }

    pub fn with_policy(
        registry: Arc<ToolRegistry>,
        policy: SelfModificationPolicy,
    ) -> Layer4Result<Self> {
        Ok(Self {
            loader: Arc::new(WasmLoader::new()?),
            registry,
            policy: RwLock::new(policy),
            installed: RwLock::new(HashMap::new()),
        })
    }

    /// Replace the policy at runtime (e.g. a kill switch flipping to
    /// [`SelfModificationPolicy::locked`]).
    pub fn set_policy(&self, policy: SelfModificationPolicy) {
        *self.policy.write() = policy;
    }

    pub fn policy(&self) -> SelfModificationPolicy {
        self.policy.read().clone()
    }

    /// Number of currently installed dynamic tools.
    pub fn installed_count(&self) -> usize {
        self.installed.read().len()
    }

    /// Install an agent-authored tool. See module docs for the pipeline.
    ///
    /// `approved` carries the human approval required when the policy says
    /// so; it is ignored for Allow and cannot override a Deny.
    #[allow(clippy::too_many_arguments)]
    pub async fn install(
        &self,
        name: &str,
        description: &str,
        code: &str,
        format: CapabilityFormat,
        parameters_schema: serde_json::Value,
        test_input: serde_json::Value,
        approved: bool,
    ) -> Layer4Result<serde_json::Value> {
        // 1. Validate inputs (before any policy state is touched).
        validate_tool_name(name).map_err(|r| anyhow::anyhow!(r))?;
        if code.len() > MAX_CODE_BYTES {
            return Err(anyhow::anyhow!(
                "code exceeds maximum size ({} > {} bytes)",
                code.len(),
                MAX_CODE_BYTES
            ));
        }
        if !parameters_schema.is_object() {
            return Err(anyhow::anyhow!("parameters_schema must be a JSON object"));
        }
        if !test_input.is_object() {
            return Err(anyhow::anyhow!("test_input must be a JSON object"));
        }

        // 2. Policy gate. Installing over an existing tool is a *replace*,
        //    which is gated more strictly than a fresh install.
        let existing_is_dynamic = self.installed.read().contains_key(name);
        let action = if self.registry.exists(name) {
            if !existing_is_dynamic {
                // Shadowing a builtin that isn't ours — never allowed.
                return Err(anyhow::anyhow!(
                    "a builtin tool named '{}' already exists; shadowing builtins is not permitted",
                    name
                ));
            }
            SelfModAction::ReplaceTool { name }
        } else {
            SelfModAction::InstallTool { name }
        };
        let decision = self
            .policy
            .read()
            .decide(&action, self.installed_count());
        match decision {
            SelfModDecision::Deny(reason) => {
                return Err(anyhow::anyhow!("denied by self-modification policy: {}", reason))
            }
            SelfModDecision::RequiresApproval(reason) => {
                if !approved {
                    return Err(anyhow::anyhow!(
                        "approval required: {}. Re-invoke with \"approved\": true after human review.",
                        reason
                    ));
                }
            }
            SelfModDecision::Allow => {}
        }

        // 3. Compile + load under the sandboxed capability set (least
        //    privilege: no fs, no network, no process, 16MB/5s limits).
        let capabilities = CapabilitySet::sandboxed();
        let plugin_name = match format {
            CapabilityFormat::Wat => self.loader.load_wat(name, code, capabilities)?,
            CapabilityFormat::WasmBase64 => {
                use base64::Engine as _;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(code.trim())
                    .map_err(|e| anyhow::anyhow!("invalid base64 wasm: {}", e))?;
                self.loader.load_binary(name, &bytes, capabilities)?
            }
        };
        debug_assert_eq!(plugin_name, name);

        // 4. Smoke test: must execute and return a value. On failure the
        //    module is unloaded so a broken tool leaves no residue.
        let plugin = self
            .loader
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("plugin missing after load: {}", name))?;
        let smoke_output = match plugin.execute(&test_input).await {
            Ok(out) => out,
            Err(e) => {
                let _ = self.loader.unload(name);
                return Err(anyhow::anyhow!(
                    "smoke test failed, tool not installed: {}",
                    e
                ));
            }
        };

        // 5. Register the plugin as a live Layer2 tool.
        let adapter = PluginToolAdapter::new(
            plugin,
            name.to_string(),
            description.to_string(),
            parameters_schema.clone(),
        );
        ToolRegistryTrait::register(&*self.registry, Box::new(adapter))
            .map_err(|e| anyhow::anyhow!("registry registration failed: {}", e))?;

        // 6. Record provenance.
        let record = InstalledCapability {
            name: name.to_string(),
            description: description.to_string(),
            format,
            parameters_schema,
            installed_at: Utc::now(),
            smoke_test_output: smoke_output.clone(),
        };
        self.installed.write().insert(name.to_string(), record);

        Ok(serde_json::json!({
            "installed": name,
            "format": format.as_str(),
            "smoke_test": "passed",
            "smoke_test_output": smoke_output,
            "capabilities": "sandboxed (no fs / no network / no process, 16MB, 5s)",
        }))
    }

    /// Uninstall a dynamic tool. Builtins and protected names are refused.
    pub fn uninstall(&self, name: &str) -> Layer4Result<bool> {
        let decision = self
            .policy
            .read()
            .decide(&SelfModAction::UninstallTool { name }, self.installed_count());
        if let SelfModDecision::Deny(reason) = decision {
            return Err(anyhow::anyhow!("denied by self-modification policy: {}", reason));
        }
        if !self.installed.read().contains_key(name) {
            return Ok(false); // not a dynamic tool — nothing to remove
        }
        let removed = ToolRegistryTrait::unregister(&*self.registry, name)
            .map_err(|e| anyhow::anyhow!("registry unregister failed: {}", e))?;
        let _ = self.loader.unload(name);
        self.installed.write().remove(name);
        Ok(removed)
    }

    /// List installed dynamic tools with provenance.
    pub fn list(&self) -> Vec<InstalledCapability> {
        self.installed.read().values().cloned().collect()
    }

    /// Register the capability tools themselves into the registry.
    /// Call once at assembly time.
    pub fn register_tools(self: &Arc<Self>) -> Layer2Result<()> {
        ToolRegistryTrait::register(
            &*self.registry,
            Box::new(InstallCapabilityTool {
                manager: self.clone(),
            }),
        )?;
        ToolRegistryTrait::register(
            &*self.registry,
            Box::new(UninstallCapabilityTool {
                manager: self.clone(),
            }),
        )?;
        ToolRegistryTrait::register(
            &*self.registry,
            Box::new(ListDynamicToolsTool {
                manager: self.clone(),
            }),
        )?;
        Ok(())
    }
}

/// `install_capability` — write, validate, sandbox, smoke-test, and install
/// an agent-authored tool.
struct InstallCapabilityTool {
    manager: Arc<CapabilityManager>,
}

#[async_trait]
impl Layer2Tool for InstallCapabilityTool {
    fn name(&self) -> &str {
        "install_capability"
    }

    fn description(&self) -> &str {
        "Author and install a new tool at runtime. The tool code (WAT text \
         or base64 WASM) is compiled in a sandbox with no filesystem, \
         network, or process access (16MB memory / 5s CPU), smoke-tested, \
         and only then registered. The module must export `memory` and an \
         `execute(ptr,len) -> (ptr,len)` entry point exchanging UTF-8 JSON."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["name", "description", "code", "format"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Tool name: [A-Za-z0-9_.-], max 64 chars, must not shadow builtins"
                },
                "description": {
                    "type": "string",
                    "description": "What the tool does (shown to the model)"
                },
                "code": {
                    "type": "string",
                    "description": "Tool source: WAT text, or base64-encoded WASM binary"
                },
                "format": {
                    "type": "string",
                    "enum": ["wat", "wasm_base64"]
                },
                "parameters_schema": {
                    "type": "object",
                    "description": "JSON Schema for the tool's arguments (default: empty object schema)"
                },
                "test_input": {
                    "type": "object",
                    "description": "Smoke-test input (default: {})"
                },
                "approved": {
                    "type": "boolean",
                    "description": "Set true only after human approval; required when policy demands it"
                }
            }
        })
    }

    async fn execute(&self, args: &str) -> Layer2Result<ToolResult> {
        // Failure contract: every failure the agent can act on (bad args,
        // policy denial, compile error, failed smoke test) is reported as a
        // resolved result with is_error=true — the agent must SEE the reason
        // to self-correct. Err is reserved for infrastructure failures.
        let fail = |msg: String| -> Layer2Result<ToolResult> {
            Ok(ToolResult {
                tool_call_id: String::new(),
                name: "install_capability".to_string(),
                content: msg,
                is_error: true,
            })
        };

        let args: serde_json::Value = match serde_json::from_str(args) {
            Ok(v) => v,
            Err(e) => {
                return fail(format!("invalid install_capability args: {}", e));
            }
        };
        let get_str = |key: &str| -> Option<&str> {
            args.get(key).and_then(|v| v.as_str())
        };

        let (Some(name), Some(description), Some(code), Some(format_str)) = (
            get_str("name"),
            get_str("description"),
            get_str("code"),
            get_str("format"),
        ) else {
            return fail(
                "fields 'name', 'description', 'code', 'format' are required strings".to_string(),
            );
        };
        let format = match CapabilityFormat::parse(format_str) {
            Ok(f) => f,
            Err(e) => return fail(e.to_string()),
        };
        let schema = args
            .get("parameters_schema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"type": "object"}));
        let test_input = args
            .get("test_input")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let approved = args
            .get("approved")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match self
            .manager
            .install(name, description, code, format, schema, test_input, approved)
            .await
        {
            Ok(report) => Ok(ToolResult {
                tool_call_id: String::new(),
                name: "install_capability".to_string(),
                content: serde_json::to_string_pretty(&report).unwrap_or_default(),
                is_error: false,
            }),
            Err(e) => fail(e.to_string()),
        }
    }
}

/// `uninstall_capability` — remove a dynamic tool installed earlier.
struct UninstallCapabilityTool {
    manager: Arc<CapabilityManager>,
}

#[async_trait]
impl Layer2Tool for UninstallCapabilityTool {
    fn name(&self) -> &str {
        "uninstall_capability"
    }

    fn description(&self) -> &str {
        "Remove a dynamic tool previously installed via install_capability. \
         Built-in tools cannot be removed."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string", "description": "Dynamic tool name to remove"}
            }
        })
    }

    async fn execute(&self, args: &str) -> Layer2Result<ToolResult> {
        // Same contract as install: actionable failures are is_error results.
        let fail = |msg: String| -> Layer2Result<ToolResult> {
            Ok(ToolResult {
                tool_call_id: String::new(),
                name: "uninstall_capability".to_string(),
                content: msg,
                is_error: true,
            })
        };

        let args: serde_json::Value = match serde_json::from_str(args) {
            Ok(v) => v,
            Err(e) => return fail(format!("invalid uninstall_capability args: {}", e)),
        };
        let Some(name) = args.get("name").and_then(|v| v.as_str()) else {
            return fail("field 'name' is a required string".to_string());
        };

        match self.manager.uninstall(name) {
            Ok(true) => Ok(ToolResult {
                tool_call_id: String::new(),
                name: "uninstall_capability".to_string(),
                content: format!("uninstalled {}", name),
                is_error: false,
            }),
            Ok(false) => fail(format!("'{}' is not an installed dynamic tool", name)),
            Err(e) => fail(e.to_string()),
        }
    }
}

/// `list_dynamic_tools` — inventory of installed dynamic tools.
struct ListDynamicToolsTool {
    manager: Arc<CapabilityManager>,
}

#[async_trait]
impl Layer2Tool for ListDynamicToolsTool {
    fn name(&self) -> &str {
        "list_dynamic_tools"
    }

    fn description(&self) -> &str {
        "List tools the agent has installed at runtime, with provenance \
         (format, install time, smoke-test output)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(&self, _args: &str) -> Layer2Result<ToolResult> {
        let caps: Vec<serde_json::Value> = self
            .manager
            .list()
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "description": c.description,
                    "format": c.format.as_str(),
                    "installed_at": c.installed_at.to_rfc3339(),
                })
            })
            .collect();
        Ok(ToolResult {
            tool_call_id: String::new(),
            name: "list_dynamic_tools".to_string(),
            content: serde_json::to_string_pretty(&serde_json::json!({
                "count": caps.len(),
                "tools": caps,
            }))
            .unwrap_or_default(),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh_layer2::ToolRegistry;

    /// WAT that echoes its JSON input back as output.
    const ECHO_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "execute") (param $ptr i32) (param $len i32) (result i32 i32)
    local.get $ptr
    local.get $len))"#;

    /// WAT returning a fixed JSON payload written to memory offset 0.
    fn static_json_wat(payload: &str) -> String {
        format!(
            r#"(module
  (memory (export "memory") 1)
  (data (i32.const 0) "{}")
  (func (export "execute") (param i32 i32) (result i32 i32)
    (i32.const 0)
    (i32.const {})))"#,
            payload.escape_default(),
            payload.len()
        )
    }

    fn manager_with_policy(policy: SelfModificationPolicy) -> (Arc<CapabilityManager>, Arc<ToolRegistry>) {
        let registry = Arc::new(ToolRegistry::new());
        let manager = Arc::new(CapabilityManager::with_policy(registry.clone(), policy).unwrap());
        manager.register_tools().unwrap();
        (manager, registry)
    }

    fn manager() -> (Arc<CapabilityManager>, Arc<ToolRegistry>) {
        let (m, r) = manager_with_policy(SelfModificationPolicy::safe_default());
        (m, r)
    }

    async fn install_echo(m: &CapabilityManager, name: &str, approved: bool) -> Layer4Result<serde_json::Value> {
        m.install(
            name,
            "echo test tool",
            ECHO_WAT,
            CapabilityFormat::Wat,
            serde_json::json!({"type": "object"}),
            serde_json::json!({"hello": "world"}),
            approved,
        )
        .await
    }

    #[tokio::test]
    async fn install_without_approval_is_refused() {
        let (m, _) = manager();
        let err = install_echo(&m, "echo_tool", false).await.unwrap_err();
        assert!(err.to_string().contains("approval required"));
        assert_eq!(m.installed_count(), 0);
    }

    #[tokio::test]
    async fn full_lifecycle_install_execute_uninstall() {
        let (m, registry) = manager();
        let report = install_echo(&m, "echo_tool", true).await.unwrap();
        assert_eq!(report["installed"], "echo_tool");
        assert_eq!(report["smoke_test"], "passed");
        assert_eq!(m.installed_count(), 1);

        // Registered and executable through the live registry.
        assert!(registry.exists("echo_tool"));
        let out = ToolRegistryTrait::execute(&*registry, "echo_tool", r#"{"a":1}"#)
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content.contains("\"a\":1"), "echo output: {}", out.content);

        // Uninstall removes it from both registry and manager.
        assert!(m.uninstall("echo_tool").unwrap());
        assert!(!registry.exists("echo_tool"));
        assert_eq!(m.installed_count(), 0);
    }

    #[tokio::test]
    async fn static_json_tool_roundtrip() {
        let (m, registry) = manager();
        let payload = r#"{"greeting":"hello from wasm"}"#;
        m.install(
            "greeter",
            "returns a greeting",
            &static_json_wat(payload),
            CapabilityFormat::Wat,
            serde_json::json!({"type": "object"}),
            serde_json::json!({}),
            true,
        )
        .await
        .unwrap();
        let out = ToolRegistryTrait::execute(&*registry, "greeter", "{}").await.unwrap();
        assert!(out.content.contains("hello from wasm"), "got: {}", out.content);
    }

    #[tokio::test]
    async fn invalid_wat_fails_cleanly() {
        let (m, registry) = manager();
        let err = m
            .install(
                "bad_wat",
                "broken",
                "(module (this is not wat",
                CapabilityFormat::Wat,
                serde_json::json!({"type": "object"}),
                serde_json::json!({}),
                true,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Failed to compile WAT"));
        assert_eq!(m.installed_count(), 0);
        assert!(!registry.exists("bad_wat"));
    }

    #[tokio::test]
    async fn no_entry_point_fails_smoke_test_and_leaves_no_residue() {
        let (m, registry) = manager();
        let wat = r#"(module (memory (export "memory") 1))"#;
        let err = m
            .install(
                "no_entry",
                "has no execute function",
                wat,
                CapabilityFormat::Wat,
                serde_json::json!({"type": "object"}),
                serde_json::json!({}),
                true,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("smoke test failed"), "err: {}", err);
        assert_eq!(m.installed_count(), 0);
        assert!(!registry.exists("no_entry"));
    }

    #[tokio::test]
    async fn hostile_names_are_rejected() {
        let (m, _) = manager();
        for bad in ["../evil", "sub/dir", "bash", "", "con"] {
            let err = m
                .install(
                    bad,
                    "d",
                    ECHO_WAT,
                    CapabilityFormat::Wat,
                    serde_json::json!({"type": "object"}),
                    serde_json::json!({}),
                    true,
                )
                .await;
            assert!(err.is_err(), "name {:?} should be rejected", bad);
        }
    }

    #[tokio::test]
    async fn builtin_shadowing_is_refused_even_when_not_protected() {
        let (m, registry) = manager();
        // Register an unrelated builtin-style tool directly in the registry.
        struct FakeBuiltin;
        #[async_trait]
        impl Layer2Tool for FakeBuiltin {
            fn name(&self) -> &str { "calc" }
            fn description(&self) -> &str { "builtin calc" }
            fn parameters(&self) -> serde_json::Value { serde_json::json!({}) }
            async fn execute(&self, _a: &str) -> Layer2Result<ToolResult> {
                Ok(ToolResult { tool_call_id: String::new(), name: "calc".into(), content: "ok".into(), is_error: false })
            }
        }
        ToolRegistryTrait::register(&*registry, Box::new(FakeBuiltin)).unwrap();

        let err = install_echo(&m, "calc", true).await.unwrap_err();
        assert!(err.to_string().contains("shadowing builtins is not permitted"));
    }

    #[tokio::test]
    async fn dynamic_tool_count_cap_enforced() {
        let mut policy = SelfModificationPolicy::safe_default();
        policy.require_approval_for_new_tools = false;
        policy.max_dynamic_tools = 1;
        let (m, _) = manager_with_policy(policy);

        install_echo(&m, "tool_a", false).await.unwrap();
        let err = install_echo(&m, "tool_b", false).await.unwrap_err();
        assert!(err.to_string().contains("dynamic tool count limit reached"));
    }

    #[tokio::test]
    async fn locked_policy_denies_even_with_approval() {
        let (m, _) = manager_with_policy(SelfModificationPolicy::locked());
        let err = install_echo(&m, "any_tool", true).await.unwrap_err();
        assert!(err.to_string().contains("denied by self-modification policy"));
    }

    #[tokio::test]
    async fn oversized_code_rejected() {
        let (m, _) = manager();
        let huge = " ".repeat(MAX_CODE_BYTES + 1);
        let err = m
            .install(
                "huge",
                "d",
                &huge,
                CapabilityFormat::Wat,
                serde_json::json!({"type": "object"}),
                serde_json::json!({}),
                true,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("maximum size"));
    }

    #[tokio::test]
    async fn reinstall_replaces_own_dynamic_tool() {
        let (m, registry) = manager();
        install_echo(&m, "mine", true).await.unwrap();
        // Replace with the static greeter under the same name (with approval).
        let payload = r#"{"v":2}"#;
        m.install(
            "mine",
            "replaced",
            &static_json_wat(payload),
            CapabilityFormat::Wat,
            serde_json::json!({"type": "object"}),
            serde_json::json!({}),
            true,
        )
        .await
        .unwrap();
        assert_eq!(m.installed_count(), 1);
        let out = ToolRegistryTrait::execute(&*registry, "mine", "{}").await.unwrap();
        assert!(out.content.contains("\"v\":2"), "replaced tool output: {}", out.content);
    }

    #[tokio::test]
    async fn uninstall_builtin_refused() {
        let (m, _) = manager();
        let err = m.uninstall("bash").unwrap_err();
        assert!(err.to_string().contains("denied by self-modification policy"));
    }

    #[tokio::test]
    async fn uninstall_unknown_returns_false() {
        let (m, _) = manager();
        assert!(!m.uninstall("never_installed").unwrap());
    }

    #[tokio::test]
    async fn tools_exposed_through_registry_as_layer2_tools() {
        let (_, registry) = manager();
        for name in ["install_capability", "uninstall_capability", "list_dynamic_tools"] {
            assert!(registry.exists(name), "{} should be registered", name);
        }
        // list_dynamic_tools works end-to-end through the registry.
        let out = ToolRegistryTrait::execute(&*registry, "list_dynamic_tools", "{}")
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content.contains("\"count\": 0"), "content: {}", out.content);

        // install_capability tool rejects missing fields with is_error output.
        let out = ToolRegistryTrait::execute(&*registry, "install_capability", "{}")
            .await
            .unwrap();
        assert!(out.is_error);
        assert!(
            out.content.contains("required strings"),
            "content: {}",
            out.content
        );
    }

    #[tokio::test]
    async fn wasm_base64_format_installs() {
        let (m, registry) = manager();
        // Encode the echo WAT as a real wasm binary (not a wasmtime
        // precompiled artifact) and install it via the base64 path.
        use base64::Engine as _;
        let bytes = wat::parse_str(ECHO_WAT).expect("WAT → wasm");
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        m.install(
            "from_binary",
            "binary install",
            &b64,
            CapabilityFormat::WasmBase64,
            serde_json::json!({"type": "object"}),
            serde_json::json!({"x": 1}),
            true,
        )
        .await
        .unwrap();
        let out = ToolRegistryTrait::execute(&*registry, "from_binary", r#"{"x":1}"#)
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content.contains("\"x\":1"), "echo output: {}", out.content);
    }

    #[tokio::test]
    async fn invalid_base64_rejected() {
        let (m, _) = manager();
        let err = m
            .install(
                "bad_b64",
                "d",
                "!!!not-base64!!!",
                CapabilityFormat::WasmBase64,
                serde_json::json!({"type": "object"}),
                serde_json::json!({}),
                true,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid base64"));
    }

    #[test]
    fn format_parse_rejects_unknown() {
        assert!(CapabilityFormat::parse("wat").is_ok());
        assert!(CapabilityFormat::parse("wasm_base64").is_ok());
        assert!(CapabilityFormat::parse("rust").is_err());
    }
}
