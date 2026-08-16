//! # LLM-Driven Task Decomposition
//!
//! 用 LLM 替换 [`crate::planner::TaskDecomposer`] 的关键词启发式：
//! 生成带依赖关系的子任务 DAG，每个子任务标注所需工具；当任务需要
//! 不存在的工具时，规划器可以产出 `install_capability` 子任务
//! （自主进化闭环：规划 → 发现能力缺口 → 造工具 → 执行）。
//!
//! ## 降级策略（graceful degradation）
//!
//! 任何失败 —— 无 LLM 客户端、API 错误、输出不可解析、DAG 非法 ——
//! 都回退到启发式分解，绝不因规划层故障阻塞任务执行。
//! [`PlanSource`] 标记结果来源，供上层观测与评估。
//!
//! ## 输出契约
//!
//! 解析对 LLM 输出做了容错：markdown 围栏、缺失 id/name、多余字段、
//! 未知依赖（剔除）、超量子任务（截断到上限）都能处理；
//! 唯一硬性失败是循环依赖与完全无法解析 —— 此时回退。

use crate::planner::{ExecutionPlan, PlanResult, RiskLevel, SubTask, TaskDecomposer};
use crate::types::Layer2Result;
use sh_layer1::{LlmClient, LlmRequestConfig, Message, MessageRole};
use std::sync::Arc;

/// 规划结果来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanSource {
    /// LLM 生成的计划
    Llm,
    /// 启发式回退（无客户端 / LLM 失败 / 解析失败）
    Heuristic,
}

/// 单次分解允许的最大子任务数（防失控输出）。
pub const MAX_SUBTASKS: usize = 16;

/// LLM 驱动的任务分解器。
pub struct LlmTaskDecomposer {
    client: Option<Arc<LlmClient>>,
    config: LlmRequestConfig,
    max_subtasks: usize,
    heuristic: TaskDecomposer,
}

impl LlmTaskDecomposer {
    /// 用 LLM 客户端创建。规划配置：低温度（确定性）、2048 token 上限。
    pub fn new(client: Arc<LlmClient>) -> Self {
        let config = LlmRequestConfig {
            model: LlmRequestConfig::default().model,
            max_tokens: 2048,
            temperature: 0.2,
            system_prompt: Some(system_prompt().to_string()),
            stop_sequences: Vec::new(),
        };
        Self {
            client: Some(client),
            config,
            max_subtasks: MAX_SUBTASKS,
            heuristic: TaskDecomposer::new(),
        }
    }

    /// 无 LLM 模式：所有分解走启发式（离线 / 无 key 环境）。
    pub fn without_llm() -> Self {
        Self {
            client: None,
            config: LlmRequestConfig::default(),
            max_subtasks: MAX_SUBTASKS,
            heuristic: TaskDecomposer::new(),
        }
    }

    /// 覆盖 LLM 请求配置（模型、token 上限等）。
    pub fn with_config(mut self, config: LlmRequestConfig) -> Self {
        self.config = config;
        self
    }

    /// 分解任务。`available_tools` 是当前注册表中的工具名（含动态安装
    /// 的工具）—— 规划器据此判断能力缺口。
    pub async fn decompose(
        &self,
        task: &str,
        available_tools: &[String],
    ) -> Layer2Result<PlanResult> {
        let Some(client) = &self.client else {
            return Ok(self.fallback(task, "no LLM client configured"));
        };

        let user_message = build_user_message(task, available_tools);
        let messages = vec![Message {
            role: MessageRole::User,
            content: user_message,
        }];

        let response = match client.send_with_retry(messages, &self.config, 2).await {
            Ok(r) => r,
            Err(e) => {
                return Ok(self.fallback(task, &format!("LLM request failed: {}", e)));
            }
        };

        match parse_llm_plan(&response.content, task, self.max_subtasks) {
            Ok(plan) => Ok(PlanResult::with_source(plan, PlanSource::Llm)),
            Err(e) => Ok(self.fallback(task, &format!("LLM plan unparseable: {}", e))),
        }
    }

    /// 启发式回退（sync 路径，永不失败以外的分支）。
    fn fallback(&self, task: &str, reason: &str) -> PlanResult {
        tracing::warn!(target: "continuum.planner", reason = %reason, "falling back to heuristic decomposition");
        let mut result = PlanResult::new(
            self.heuristic
                .decompose(task)
                .expect("heuristic decomposition is infallible for valid UTF-8 tasks"),
        );
        result.source = PlanSource::Heuristic;
        result
            .suggestions
            .push(format!("planner degraded to heuristics: {}", reason));
        result
    }
}

/// 系统提示：输出契约 + 能力缺口处理规则。
fn system_prompt() -> &'static str {
    "You are a task planner for an autonomous agent. Decompose the user's \
     task into an ordered set of subtasks forming a dependency DAG.\n\
     Output ONLY a JSON object — no markdown fences, no commentary:\n\
     {\n  \"risk\": \"low|medium|high|critical\",\n  \"subtasks\": [\n    {\n      \"id\": \"s1\",\n      \"name\": \"short name\",\n      \"description\": \"what to do\",\n      \"priority\": 0,\n      \"dependencies\": [\"ids of subtasks that must finish first\"],\n      \"complexity\": 1-10,\n      \"tool\": \"tool name for the primary action\",\n      \"tool_args\": {},\n      \"validation\": [\"how to verify this step worked\"]\n    }\n  ]\n}\n\
     Rules:\n\
     - Keep plans small: only as many subtasks as the task truly needs.\n\
     - `dependencies` must reference ids of other subtasks in this plan; never invent ids.\n\
     - Prefer the available tools listed by the user for `tool`.\n\
     - If the task needs a capability that is NOT in the available tools, \
     add a subtask with \"tool\": \"install_capability\" that creates it \
     (code format: WAT, sandboxed, no fs/network), placed before its dependents.\n\
     - Include at least one validation criterion for risky steps."
}

fn build_user_message(task: &str, available_tools: &[String]) -> String {
    let tools = if available_tools.is_empty() {
        "(none listed)".to_string()
    } else {
        available_tools.join(", ")
    };
    format!(
        "Task: {}\n\nAvailable tools: {}\n\nProduce the plan JSON now.",
        task, tools
    )
}

// ---------- LLM 输出解析（纯函数，独立测试） ----------

#[derive(Debug, serde::Deserialize)]
struct LlmPlan {
    #[serde(default)]
    risk: Option<String>,
    #[serde(default)]
    subtasks: Vec<LlmSub>,
}

#[derive(Debug, serde::Deserialize)]
struct LlmSub {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    priority: u32,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    complexity: Option<u32>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    tool_args: Option<serde_json::Value>,
    #[serde(default)]
    validation: Vec<String>,
}

/// 解析 LLM 输出为执行计划。
///
/// 容错规则见模块文档；硬失败仅两种：提取不到 JSON / DAG 有环。
pub fn parse_llm_plan(raw: &str, task: &str, max_subtasks: usize) -> Layer2Result<ExecutionPlan> {
    let json_str = extract_json(raw)?;
    let llm: LlmPlan =
        serde_json::from_str(json_str).map_err(|e| anyhow::anyhow!("invalid plan JSON: {}", e))?;

    if llm.subtasks.is_empty() {
        return Err(anyhow::anyhow!("plan contains no subtasks"));
    }

    let mut plan = ExecutionPlan::new(task);
    plan.risk_level = parse_risk(llm.risk.as_deref());

    // 截断到上限（保留前 N 个 —— LLM 通常按重要性排序输出）
    let kept: Vec<&LlmSub> = llm.subtasks.iter().take(max_subtasks).collect();
    let valid_ids: Vec<String> = kept
        .iter()
        .enumerate()
        .map(|(i, s)| normalize_id(s.id.as_deref(), i))
        .collect();

    for (i, sub) in kept.iter().enumerate() {
        let id = valid_ids[i].clone();
        let name = sub
            .name
            .clone()
            .unwrap_or_else(|| format!("Step {}", i + 1));
        let description = sub.description.clone().unwrap_or_else(|| name.clone());

        let mut st = SubTask::new(id.clone(), name, description);
        st.priority = sub.priority;
        st.estimated_complexity = sub.complexity.unwrap_or(5).clamp(1, 10);

        // 依赖：只保留指向本计划内其他子任务的 id（剔除幻觉引用与自引用）
        for dep in &sub.dependencies {
            if valid_ids.contains(dep) && dep != &id {
                st.dependencies.push(dep.clone());
            }
        }

        if let Some(tool) = sub.tool.as_deref() {
            let tool = tool.trim();
            if !tool.is_empty() {
                st.tool = Some(tool.to_string());
                st.tool_args = Some(sub.tool_args.clone().unwrap_or(serde_json::json!({})));
            }
        }
        for v in &sub.validation {
            let v = v.trim();
            if !v.is_empty() {
                st.validation_criteria.push(v.to_string());
            }
        }
        plan.add_subtask(st);
    }

    // 拓扑排序 —— 循环依赖是唯一让解析硬失败的 DAG 问题
    plan.compute_execution_order()?;
    Ok(plan)
}

/// 从可能带 markdown 围栏或前后缀文本的输出中提取 JSON 主体。
fn extract_json(raw: &str) -> Layer2Result<&str> {
    let start = raw
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no JSON object found in LLM output"))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("unterminated JSON object in LLM output"))?;
    if end <= start {
        return Err(anyhow::anyhow!("malformed JSON bounds in LLM output"));
    }
    Ok(&raw[start..=end])
}

fn normalize_id(id: Option<&str>, index: usize) -> String {
    match id {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => format!("s{}", index + 1),
    }
}

fn parse_risk(risk: Option<&str>) -> RiskLevel {
    match risk.map(|r| r.trim().to_lowercase()).as_deref() {
        Some("low") => RiskLevel::Low,
        Some("high") => RiskLevel::High,
        Some("critical") => RiskLevel::Critical,
        // medium / 未知 / 缺失 → 默认
        _ => RiskLevel::Medium,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Layer2Result<ExecutionPlan> {
        parse_llm_plan(raw, "test task", MAX_SUBTASKS)
    }

    #[test]
    fn parses_clean_plan_with_dag() {
        let raw = r#"{
            "risk": "high",
            "subtasks": [
                {"id": "s1", "name": "Search", "description": "find files",
                 "tool": "grep", "tool_args": {"pattern": "TODO"},
                 "validation": ["results non-empty"]},
                {"id": "s2", "name": "Fix", "description": "edit files",
                 "dependencies": ["s1"], "tool": "edit_file", "complexity": 7},
                {"id": "s3", "name": "Verify", "description": "run tests",
                 "dependencies": ["s2", "s1"]}
            ]
        }"#;
        let plan = parse(raw).unwrap();
        assert_eq!(plan.risk_level, RiskLevel::High);
        assert_eq!(plan.subtasks.len(), 3);
        assert_eq!(plan.execution_order.first().map(String::as_str), Some("s1"));
        // s3 depends on both; s1 must come before s3
        let pos = |id: &str| plan.execution_order.iter().position(|s| s == id).unwrap();
        assert!(pos("s1") < pos("s2"));
        assert!(pos("s2") < pos("s3"));
        assert_eq!(plan.subtasks[0].tool.as_deref(), Some("grep"));
        assert_eq!(plan.subtasks[1].estimated_complexity, 7);
        assert!(!plan.subtasks[0].validation_criteria.is_empty());
    }

    #[test]
    fn strips_markdown_fences() {
        let raw = "Here is the plan:\n```json\n{\"risk\":\"low\",\"subtasks\":[{\"id\":\"a\",\"description\":\"do it\"}]}\n```\nDone.";
        let plan = parse(raw).unwrap();
        assert_eq!(plan.subtasks.len(), 1);
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let raw = r#"{"subtasks": [{"description": "only a description"}]}"#;
        let plan = parse(raw).unwrap();
        assert_eq!(plan.subtasks.len(), 1);
        assert_eq!(plan.subtasks[0].id, "s1"); // synthesized id
        assert_eq!(plan.subtasks[0].name, "Step 1");
        assert_eq!(plan.subtasks[0].estimated_complexity, 5); // default
        assert_eq!(plan.risk_level, RiskLevel::Medium); // default
    }

    #[test]
    fn unknown_dependencies_are_dropped_not_fatal() {
        let raw = r#"{"subtasks": [
            {"id": "a", "description": "first"},
            {"id": "b", "description": "second", "dependencies": ["a", "ghost_id", "b"]}
        ]}"#;
        let plan = parse(raw).unwrap();
        let b = &plan.subtasks[1];
        assert_eq!(
            b.dependencies,
            vec!["a".to_string()],
            "ghost + self deps dropped"
        );
    }

    #[test]
    fn cyclic_plan_errors() {
        let raw = r#"{"subtasks": [
            {"id": "a", "dependencies": ["b"]},
            {"id": "b", "dependencies": ["a"]}
        ]}"#;
        let err = parse(raw).unwrap_err();
        assert!(err.to_string().contains("Circular dependency"));
    }

    #[test]
    fn empty_subtasks_rejected() {
        assert!(parse(r#"{"subtasks": []}"#).is_err());
        assert!(parse("no json at all").is_err());
        assert!(parse("{broken json").is_err());
    }

    #[test]
    fn subtask_count_is_capped() {
        let subs: Vec<String> = (0..30)
            .map(|i| format!(r#"{{"id": "s{}", "description": "step {}"}}"#, i, i))
            .collect();
        let raw = format!(r#"{{"subtasks": [{}]}}"#, subs.join(","));
        let plan = parse_llm_plan(&raw, "big task", 5).unwrap();
        assert_eq!(plan.subtasks.len(), 5);
    }

    #[test]
    fn install_capability_subtask_survives_parsing() {
        // The self-evolution hook: a missing-tool gap becomes a subtask.
        let raw = r#"{"subtasks": [
            {"id": "s1", "name": "Create csv tool",
             "description": "No csv tool exists; install one",
             "tool": "install_capability",
             "tool_args": {"name": "csv_stats", "format": "wat", "code": "(module)"},
             "validation": ["smoke test passes"]},
            {"id": "s2", "description": "Use it", "dependencies": ["s1"],
             "tool": "csv_stats"}
        ]}"#;
        let plan = parse(raw).unwrap();
        assert_eq!(plan.subtasks[0].tool.as_deref(), Some("install_capability"));
        assert_eq!(
            plan.subtasks[0].tool_args.as_ref().unwrap()["format"],
            serde_json::json!("wat")
        );
        assert!(
            plan.execution_order.iter().position(|s| s == "s1")
                < plan.execution_order.iter().position(|s| s == "s2")
        );
    }

    #[test]
    fn blank_tool_is_ignored() {
        let raw = r#"{"subtasks": [{"id": "a", "description": "x", "tool": "  "}]}"#;
        let plan = parse(raw).unwrap();
        assert!(plan.subtasks[0].tool.is_none());
    }

    // ---- decomposer fallback behavior (no network) ----

    #[tokio::test]
    async fn without_llm_uses_heuristic_and_reports_source() {
        let d = LlmTaskDecomposer::without_llm();
        let result = d
            .decompose("Create a file and write content", &[])
            .await
            .unwrap();
        assert_eq!(result.source, PlanSource::Heuristic);
        assert!(!result.plan.subtasks.is_empty());
        assert!(result
            .suggestions
            .iter()
            .any(|s| s.contains("degraded to heuristics")));
    }

    #[test]
    fn plan_result_default_source_is_heuristic() {
        // Existing constructor callers keep working unchanged.
        let plan = TaskDecomposer::new().decompose("simple task").unwrap();
        let result = PlanResult::new(plan);
        assert_eq!(result.source, PlanSource::Heuristic);
    }

    #[test]
    fn user_message_lists_tools_and_task() {
        let msg = build_user_message("do the thing", &["grep".into(), "bash".into()]);
        assert!(msg.contains("do the thing"));
        assert!(msg.contains("grep, bash"));
        let msg_empty = build_user_message("t", &[]);
        assert!(msg_empty.contains("(none listed)"));
    }
}
