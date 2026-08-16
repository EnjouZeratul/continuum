//! # Skill Tools
//!
//! Agent-facing tools over [`SkillStore`]：save / run / list / improve。
//! 这组工具构成自主学习循环的可调用面。
//!
//! - `save_skill` — 把重复模式固化为可参数化技能
//! - `run_skill`  — 渲染模板 → 经 **live ToolRegistry** 顺序执行 → 记录结果
//! - `list_skills` — 技能清单 + 使用统计
//! - `improve_skill` — 更新已有技能（保留 id 与统计）
//!
//! 注意 `run_skill` 持有 `Arc<ToolRegistry>`（Layer 2 的运行时可变注册表），
//! 因此技能步骤可以调用任何已注册工具，包括 agent 运行时通过
//! `install_capability` 安装的动态 WASM 工具 —— 技能 + 动态工具 =
//! 跨能力的复合进化。

use crate::builtin_tools::{BuiltinTool, ToolAdapter};
use crate::skill_store::{NewSkill, SkillStep, SkillStore};
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use sh_layer2::{ToolRegistry, ToolRegistryTrait};
use std::sync::Arc;

/// 每步输出进入聚合结果的最大字符数（防止单步刷屏）。
const STEP_OUTPUT_MAX_CHARS: usize = 2000;

/// `save_skill` — 保存新技能。
pub struct SaveSkillTool {
    store: Arc<SkillStore>,
}

impl SaveSkillTool {
    pub fn new(store: Arc<SkillStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl BuiltinTool for SaveSkillTool {
    fn name(&self) -> &str {
        "save_skill"
    }

    fn description(&self) -> &str {
        "Save a reusable skill: a named, parameterized sequence of tool \
         calls. Use '{{param}}' placeholders in step arguments; at run time \
         they are replaced by the params you pass to run_skill."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["name", "description", "steps"],
            "properties": {
                "name": {"type": "string", "description": "Skill name: [A-Za-z0-9_.-], max 64 chars"},
                "description": {"type": "string", "description": "What this skill accomplishes"},
                "trigger_patterns": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Kinds of tasks where this skill applies (for search)"
                },
                "steps": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 32,
                    "items": {
                        "type": "object",
                        "required": ["tool", "arguments"],
                        "properties": {
                            "tool": {"type": "string", "description": "Tool name to call"},
                            "arguments": {
                                "type": "object",
                                "description": "Arguments; strings may contain {{param}} placeholders"
                            }
                        }
                    }
                },
                "success_criteria": {"type": "string", "description": "How to tell the skill worked"}
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let Some(name) = args.get("name").and_then(|v| v.as_str()) else {
            return Err(anyhow::anyhow!("'name' is a required string"));
        };
        let Some(description) = args.get("description").and_then(|v| v.as_str()) else {
            return Err(anyhow::anyhow!("'description' is a required string"));
        };
        let Some(steps_value) = args.get("steps").and_then(|v| v.as_array()) else {
            return Err(anyhow::anyhow!("'steps' is a required array"));
        };
        let mut steps = Vec::with_capacity(steps_value.len());
        for (i, sv) in steps_value.iter().enumerate() {
            let Some(tool) = sv.get("tool").and_then(|v| v.as_str()) else {
                return Err(anyhow::anyhow!("step {} missing 'tool' string", i));
            };
            let Some(arguments) = sv.get("arguments").cloned() else {
                return Err(anyhow::anyhow!("step {} missing 'arguments'", i));
            };
            steps.push(SkillStep {
                tool: tool.to_string(),
                arguments,
            });
        }

        let new = NewSkill {
            name: name.to_string(),
            description: description.to_string(),
            trigger_patterns: args
                .get("trigger_patterns")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            steps,
            success_criteria: args
                .get("success_criteria")
                .and_then(|v| v.as_str())
                .map(String::from),
        };
        let id = self.store.save_new(new)?;
        Ok(format!(
            "saved skill '{}' (id {}). call it with run_skill.",
            name, id
        ))
    }
}

/// `run_skill` — 渲染并顺序执行技能步骤，记录成败。
pub struct RunSkillTool {
    store: Arc<SkillStore>,
    registry: Arc<ToolRegistry>,
}

impl RunSkillTool {
    pub fn new(store: Arc<SkillStore>, registry: Arc<ToolRegistry>) -> Self {
        Self { store, registry }
    }
}

#[async_trait]
impl BuiltinTool for RunSkillTool {
    fn name(&self) -> &str {
        "run_skill"
    }

    fn description(&self) -> &str {
        "Run a saved skill by name: substitutes {{param}} placeholders with \
         the given params, executes the steps in order through the live tool \
         registry (stopping at the first failure), and records the outcome \
         in the skill's usage statistics."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string", "description": "Skill name"},
                "params": {
                    "type": "object",
                    "description": "Values substituted into {{param}} placeholders"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let Some(name) = args.get("name").and_then(|v| v.as_str()) else {
            return Err(anyhow::anyhow!("'name' is a required string"));
        };
        let params = args
            .get("params")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let requests = self.store.render_steps(name, &params)?;
        let total = requests.len();

        let mut step_results = Vec::with_capacity(total);
        let mut all_ok = true;
        for req in requests {
            let resp = ToolRegistryTrait::execute_with_call_id(
                &*self.registry,
                &req.name,
                &serde_json::to_string(&req.arguments).unwrap_or_else(|_| "{}".into()),
                &req.call_id,
            )
            .await;

            match resp {
                Ok(r) if !r.is_error => {
                    step_results.push(serde_json::json!({
                        "tool": req.name,
                        "ok": true,
                        "output": crate::builtin_tools::safe_truncate::safe_truncate_chars(&r.content, STEP_OUTPUT_MAX_CHARS),
                    }));
                }
                Ok(r) => {
                    all_ok = false;
                    step_results.push(serde_json::json!({
                        "tool": req.name,
                        "ok": false,
                        "error": crate::builtin_tools::safe_truncate::safe_truncate_chars(&r.content, STEP_OUTPUT_MAX_CHARS),
                    }));
                    break; // step failure aborts the sequence
                }
                Err(e) => {
                    all_ok = false;
                    step_results.push(serde_json::json!({
                        "tool": req.name,
                        "ok": false,
                        "error": e.to_string(),
                    }));
                    break;
                }
            }
        }

        // Record the outcome regardless of success (usage stats drive
        // the improve/prune loop).
        if let Err(e) = self.store.record_outcome(name, all_ok) {
            tracing::warn!("failed to record outcome for skill '{}': {}", name, e);
        }

        let executed = step_results.len();
        let summary = serde_json::json!({
            "skill": name,
            "status": if all_ok { "success" } else { "failed" },
            "steps_executed": executed,
            "steps_total": total,
            "steps": step_results,
        });
        Ok(serde_json::to_string_pretty(&summary)?)
    }
}

/// `list_skills` — 技能清单（含搜索）。
pub struct ListSkillsTool {
    store: Arc<SkillStore>,
}

impl ListSkillsTool {
    pub fn new(store: Arc<SkillStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl BuiltinTool for ListSkillsTool {
    fn name(&self) -> &str {
        "list_skills"
    }

    fn description(&self) -> &str {
        "List saved skills with usage statistics, optionally filtered by a \
         search query matching name/description/trigger patterns."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Optional substring filter"}
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let skills = self.store.search(query);
        let out = serde_json::json!({
            "count": skills.len(),
            "skills": skills,
        });
        Ok(serde_json::to_string_pretty(&out)?)
    }
}

/// `improve_skill` — 更新已有技能（保留 id 与统计）。
pub struct ImproveSkillTool {
    store: Arc<SkillStore>,
}

impl ImproveSkillTool {
    pub fn new(store: Arc<SkillStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl BuiltinTool for ImproveSkillTool {
    fn name(&self) -> &str {
        "improve_skill"
    }

    fn description(&self) -> &str {
        "Update an existing skill's description, trigger patterns, steps, or \
         success criteria. Usage statistics and identity are preserved so \
         you can compare success rates across revisions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string", "description": "Skill to update"},
                "description": {"type": "string"},
                "trigger_patterns": {"type": "array", "items": {"type": "string"}},
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["tool", "arguments"],
                        "properties": {
                            "tool": {"type": "string"},
                            "arguments": {"type": "object"}
                        }
                    }
                },
                "success_criteria": {"type": "string"}
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let Some(name) = args.get("name").and_then(|v| v.as_str()) else {
            return Err(anyhow::anyhow!("'name' is a required string"));
        };
        let get_str = |key: &str| args.get(key).and_then(|v| v.as_str()).map(String::from);
        let get_strings = |key: &str| {
            args.get(key).and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<String>>()
            })
        };
        let steps = match args.get("steps").and_then(|v| v.as_array()) {
            Some(arr) => {
                let mut steps = Vec::with_capacity(arr.len());
                for (i, sv) in arr.iter().enumerate() {
                    let Some(tool) = sv.get("tool").and_then(|v| v.as_str()) else {
                        return Err(anyhow::anyhow!("step {} missing 'tool' string", i));
                    };
                    let Some(arguments) = sv.get("arguments").cloned() else {
                        return Err(anyhow::anyhow!("step {} missing 'arguments'", i));
                    };
                    steps.push(SkillStep {
                        tool: tool.to_string(),
                        arguments,
                    });
                }
                Some(steps)
            }
            None => None,
        };

        self.store.update(
            name,
            get_str("description"),
            get_strings("trigger_patterns"),
            steps,
            get_str("success_criteria"),
        )?;
        Ok(format!("updated skill '{}'", name))
    }
}

/// 注册全部技能工具到 Layer 2 ToolRegistry（装配时调用）。
///
/// `registry` 必须是装配处使用的同一个 Arc — run_skill 通过它执行步骤，
/// 这样技能可以调用包括动态 WASM 工具在内的任何已注册工具。
pub fn register_skill_tools(
    registry: &Arc<ToolRegistry>,
    store: Arc<SkillStore>,
) -> anyhow::Result<()> {
    ToolRegistryTrait::register(
        &**registry,
        Box::new(ToolAdapter::new(Box::new(SaveSkillTool::new(
            store.clone(),
        )))),
    )?;
    ToolRegistryTrait::register(
        &**registry,
        Box::new(ToolAdapter::new(Box::new(RunSkillTool::new(
            store.clone(),
            registry.clone(),
        )))),
    )?;
    ToolRegistryTrait::register(
        &**registry,
        Box::new(ToolAdapter::new(Box::new(ListSkillsTool::new(
            store.clone(),
        )))),
    )?;
    ToolRegistryTrait::register(
        &**registry,
        Box::new(ToolAdapter::new(Box::new(ImproveSkillTool::new(store)))),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store_in_temp() -> (Arc<SkillStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (Arc::new(SkillStore::open(dir.path()).unwrap()), dir)
    }

    /// Layer2 mock tool that records its calls and optionally fails.
    struct MockTool {
        name: String,
        fail: bool,
        calls: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl sh_layer2::Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "mock"
        }
        fn parameters(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, args: &str) -> sh_layer2::Layer2Result<sh_layer2::ToolResult> {
            self.calls.lock().unwrap().push(args.to_string());
            if self.fail {
                return Ok(sh_layer2::ToolResult {
                    tool_call_id: String::new(),
                    name: self.name.clone(),
                    content: "mock failure".into(),
                    is_error: true,
                });
            }
            Ok(sh_layer2::ToolResult {
                tool_call_id: String::new(),
                name: self.name.clone(),
                content: format!("ran({})", args),
                is_error: false,
            })
        }
    }

    #[tokio::test]
    async fn save_and_run_skill_end_to_end() {
        let (store, _d) = store_in_temp();
        let registry = Arc::new(ToolRegistry::new());
        ToolRegistryTrait::register(
            &*registry,
            Box::new(MockTool {
                name: "greet".into(),
                fail: false,
                calls: Default::default(),
            }),
        )
        .unwrap();

        // save
        let save = SaveSkillTool::new(store.clone());
        let out = save
            .execute(json!({
                "name": "greeting_flow",
                "description": "greets someone",
                "steps": [{"tool": "greet", "arguments": {"who": "{{name}}"}}],
            }))
            .await
            .unwrap();
        assert!(out.contains("saved skill 'greeting_flow'"));

        // run with params
        let run = RunSkillTool::new(store.clone(), registry.clone());
        let out = run
            .execute(json!({"name": "greeting_flow", "params": {"name": "world"}}))
            .await
            .unwrap();
        assert!(out.contains("\"status\": \"success\""), "out: {}", out);
        assert!(out.contains("steps_executed"));

        // stats recorded
        let def = store.get("greeting_flow").unwrap();
        assert_eq!(def.usage_count, 1);
        assert_eq!(def.success_count, 1);
    }

    #[tokio::test]
    async fn run_missing_skill_errors() {
        let (store, _d) = store_in_temp();
        let registry = Arc::new(ToolRegistry::new());
        let run = RunSkillTool::new(store, registry);
        let err = run.execute(json!({"name": "ghost"})).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn step_failure_aborts_and_records_failure() {
        let (store, _d) = store_in_temp();
        let registry = Arc::new(ToolRegistry::new());
        // First step OK, second step fails.
        ToolRegistryTrait::register(
            &*registry,
            Box::new(MockTool {
                name: "ok_step".into(),
                fail: false,
                calls: Default::default(),
            }),
        )
        .unwrap();
        ToolRegistryTrait::register(
            &*registry,
            Box::new(MockTool {
                name: "bad_step".into(),
                fail: true,
                calls: Default::default(),
            }),
        )
        .unwrap();
        ToolRegistryTrait::register(
            &*registry,
            Box::new(MockTool {
                name: "never_reached".into(),
                fail: false,
                calls: Default::default(),
            }),
        )
        .unwrap();

        let save = SaveSkillTool::new(store.clone());
        save.execute(json!({
            "name": "failing_flow",
            "description": "d",
            "steps": [
                {"tool": "ok_step", "arguments": {}},
                {"tool": "bad_step", "arguments": {}},
                {"tool": "never_reached", "arguments": {}}
            ]
        }))
        .await
        .unwrap();

        let run = RunSkillTool::new(store.clone(), registry);
        let out = run.execute(json!({"name": "failing_flow"})).await.unwrap();
        assert!(out.contains("\"status\": \"failed\""), "out: {}", out);
        assert!(out.contains("\"steps_executed\": 2"), "out: {}", out);
        assert!(!out.contains("never_reached"));

        let def = store.get("failing_flow").unwrap();
        assert_eq!(def.usage_count, 1);
        assert_eq!(def.success_count, 0);
    }

    #[tokio::test]
    async fn list_and_search_through_tool() {
        let (store, _d) = store_in_temp();
        SaveSkillTool::new(store.clone())
            .execute(json!({
                "name": "csv_flow",
                "description": "parse csv data",
                "trigger_patterns": ["csv parsing"],
                "steps": [{"tool": "x", "arguments": {}}]
            }))
            .await
            .unwrap();

        let list = ListSkillsTool::new(store);
        let out = list.execute(json!({})).await.unwrap();
        assert!(out.contains("csv_flow"));
        let out = list.execute(json!({"query": "csv"})).await.unwrap();
        assert!(out.contains("csv_flow"));
        let out = list
            .execute(json!({"query": "no_match_xyz"}))
            .await
            .unwrap();
        assert!(out.contains("\"count\": 0"));
    }

    #[tokio::test]
    async fn improve_preserves_stats_via_tool() {
        let (store, _d) = store_in_temp();
        SaveSkillTool::new(store.clone())
            .execute(json!({
                "name": "evolving",
                "description": "v1",
                "steps": [{"tool": "x", "arguments": {}}]
            }))
            .await
            .unwrap();
        store.record_outcome("evolving", true).unwrap();

        ImproveSkillTool::new(store.clone())
            .execute(json!({
                "name": "evolving",
                "description": "v2",
                "steps": [{"tool": "y", "arguments": {"a": "{{b}}"}}]
            }))
            .await
            .unwrap();

        let def = store.get("evolving").unwrap();
        assert_eq!(def.description, "v2");
        assert_eq!(def.steps[0].tool, "y");
        assert_eq!(def.usage_count, 1, "stats preserved through improve");
    }

    #[tokio::test]
    async fn register_skill_tools_wires_all_four() {
        let (store, _d) = store_in_temp();
        let registry = Arc::new(ToolRegistry::new());
        register_skill_tools(&registry, store).unwrap();
        for name in ["save_skill", "run_skill", "list_skills", "improve_skill"] {
            assert!(registry.exists(name), "{} registered", name);
        }
    }
}
