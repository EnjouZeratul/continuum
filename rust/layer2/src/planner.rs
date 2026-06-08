//! # Agent Planner
//!
//! 任务分解和执行计划生成。
//!
//! 支持将复杂任务分解为可执行的子任务序列。

use crate::types::{Layer2Result, TaskId};
use crate::workflow_engine::{Dag, Node};
use serde::{Deserialize, Serialize};

/// 任务分解策略
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecompositionStrategy {
    /// 顺序分解：按步骤顺序执行
    Sequential,
    /// 并行分解：独立子任务并行执行
    Parallel,
    /// 混合分解：根据依赖关系自动选择
    #[default]
    Hybrid,
    /// 层次分解：先粗粒度再细粒度
    Hierarchical,
}

/// 子任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    /// 子任务ID
    pub id: String,
    /// 子任务名称
    pub name: String,
    /// 子任务描述
    pub description: String,
    /// 执行优先级 (0最高)
    pub priority: u32,
    /// 依赖的任务ID列表
    pub dependencies: Vec<String>,
    /// 预估复杂度 (1-10)
    pub estimated_complexity: u32,
    /// 执行工具
    pub tool: Option<String>,
    /// 工具参数
    pub tool_args: Option<serde_json::Value>,
    /// 验证条件
    pub validation_criteria: Vec<String>,
    /// 失败时的替代方案
    pub fallback: Option<Box<SubTask>>,
}

impl SubTask {
    /// 创建新的子任务
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            priority: 0,
            dependencies: Vec::new(),
            estimated_complexity: 5,
            tool: None,
            tool_args: None,
            validation_criteria: Vec::new(),
            fallback: None,
        }
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// 添加依赖
    pub fn with_dependency(mut self, dep_id: impl Into<String>) -> Self {
        self.dependencies.push(dep_id.into());
        self
    }

    /// 设置执行工具
    pub fn with_tool(mut self, tool: impl Into<String>, args: serde_json::Value) -> Self {
        self.tool = Some(tool.into());
        self.tool_args = Some(args);
        self
    }

    /// 设置验证条件
    pub fn with_validation(mut self, criteria: impl Into<String>) -> Self {
        self.validation_criteria.push(criteria.into());
        self
    }

    /// 设置失败替代方案
    pub fn with_fallback(mut self, fallback: SubTask) -> Self {
        self.fallback = Some(Box::new(fallback));
        self
    }
}

/// 执行计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// 计划ID
    pub id: String,
    /// 原始任务描述
    pub original_task: String,
    /// 分解策略
    pub strategy: DecompositionStrategy,
    /// 子任务列表
    pub subtasks: Vec<SubTask>,
    /// 执行顺序（拓扑排序后的ID列表）
    pub execution_order: Vec<String>,
    /// 估算总步数
    pub estimated_steps: u32,
    /// 风险评估
    pub risk_level: RiskLevel,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 风险等级
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// 低风险：简单的确定性任务
    Low,
    /// 中风险：需要多个工具协作
    #[default]
    Medium,
    /// 高风险：涉及复杂逻辑或外部系统
    High,
    /// 极高风险：可能需要用户介入
    Critical,
}

impl ExecutionPlan {
    /// 创建新的执行计划
    pub fn new(original_task: impl Into<String>) -> Self {
        Self {
            id: TaskId::new().to_string(),
            original_task: original_task.into(),
            strategy: DecompositionStrategy::default(),
            subtasks: Vec::new(),
            execution_order: Vec::new(),
            estimated_steps: 1,
            risk_level: RiskLevel::default(),
            created_at: chrono::Utc::now(),
        }
    }

    /// 添加子任务
    pub fn add_subtask(&mut self, subtask: SubTask) -> &mut Self {
        self.subtasks.push(subtask);
        self
    }

    /// 计算执行顺序（拓扑排序）
    pub fn compute_execution_order(&mut self) -> Layer2Result<()> {
        let mut dag = Dag::new();

        // 添加节点
        for subtask in &self.subtasks {
            let node = Node::new(&subtask.id, &subtask.name);
            dag.add_node(node)?;
        }

        // 添加边（依赖关系）
        for subtask in &self.subtasks {
            for dep in &subtask.dependencies {
                dag.add_edge(dep, &subtask.id)?;
            }
        }

        // 检查循环依赖
        if dag.has_cycle() {
            return Err(anyhow::anyhow!(
                "Circular dependency detected in execution plan"
            ));
        }

        // 拓扑排序
        self.execution_order = dag.topological_sort()?;

        // 估算步数
        self.estimated_steps = self.subtasks.len() as u32;

        Ok(())
    }

    /// 转换为 DAG 工作流
    pub fn to_dag(&self) -> Layer2Result<Dag> {
        let mut dag = Dag::new();

        for subtask in &self.subtasks {
            let mut node = Node::new(&subtask.id, &subtask.name);
            // 使用 config 字段存储子任务信息
            node.config = serde_json::json!({
                "description": subtask.description,
                "tool": subtask.tool,
                "tool_args": subtask.tool_args,
            });
            dag.add_node(node)?;
        }

        for subtask in &self.subtasks {
            for dep in &subtask.dependencies {
                dag.add_edge(dep, &subtask.id)?;
            }
        }

        Ok(dag)
    }
}

/// 任务分解器
pub struct TaskDecomposer {
    /// 分解策略
    strategy: DecompositionStrategy,
    /// 最大分解深度
    max_depth: u32,
    /// 最小子任务粒度
    #[allow(dead_code)]
    min_granularity: u32,
}

impl Default for TaskDecomposer {
    fn default() -> Self {
        Self {
            strategy: DecompositionStrategy::Hybrid,
            max_depth: 3,
            min_granularity: 1,
        }
    }
}

impl TaskDecomposer {
    /// 创建新的任务分解器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置分解策略
    pub fn with_strategy(mut self, strategy: DecompositionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// 设置最大分解深度
    pub fn with_max_depth(mut self, depth: u32) -> Self {
        self.max_depth = depth;
        self
    }

    /// 分解任务为执行计划
    pub fn decompose(&self, task: &str) -> Layer2Result<ExecutionPlan> {
        let mut plan = ExecutionPlan::new(task);
        plan.strategy = self.strategy;

        // 分析任务复杂度
        let complexity = self.analyze_complexity(task);
        plan.risk_level = self.estimate_risk(task, complexity);

        // 根据策略分解任务
        let subtasks = match self.strategy {
            DecompositionStrategy::Sequential => self.decompose_sequential(task),
            DecompositionStrategy::Parallel => self.decompose_parallel(task),
            DecompositionStrategy::Hierarchical => self.decompose_hierarchical(task, 0),
            DecompositionStrategy::Hybrid => self.decompose_hybrid(task),
        };

        for subtask in subtasks {
            plan.add_subtask(subtask);
        }

        plan.compute_execution_order()?;

        Ok(plan)
    }

    /// 分析任务复杂度
    fn analyze_complexity(&self, task: &str) -> u32 {
        let mut complexity = 1u32;

        // 关键词分析
        let task_lower = task.to_lowercase();

        // 复杂度增加因素
        if task_lower.contains("implement") || task_lower.contains("create") {
            complexity += 2;
        }
        if task_lower.contains("refactor") || task_lower.contains("rewrite") {
            complexity += 2;
        }
        if task_lower.contains("integrate") || task_lower.contains("connect") {
            complexity += 1;
        }
        if task_lower.contains("test") || task_lower.contains("verify") {
            complexity += 1;
        }
        if task_lower.contains("and") || task_lower.contains("then") {
            complexity += 1;
        }
        if task_lower.contains("multiple") || task_lower.contains("several") {
            complexity += 1;
        }

        // 长度因素
        let word_count = task.split_whitespace().count();
        if word_count > 20 {
            complexity += 1;
        }
        if word_count > 50 {
            complexity += 1;
        }

        complexity.min(10)
    }

    /// 估算风险等级
    fn estimate_risk(&self, task: &str, complexity: u32) -> RiskLevel {
        let task_lower = task.to_lowercase();

        // 检查高风险关键词
        if task_lower.contains("delete")
            || task_lower.contains("remove")
            || task_lower.contains("drop")
        {
            return RiskLevel::Critical;
        }
        if task_lower.contains("production")
            || task_lower.contains("live")
            || task_lower.contains("deploy")
        {
            return RiskLevel::High;
        }
        if task_lower.contains("database") || task_lower.contains("migration") {
            return RiskLevel::High;
        }

        // 基于复杂度
        match complexity {
            1..=3 => RiskLevel::Low,
            4..=6 => RiskLevel::Medium,
            7..=8 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    /// 顺序分解
    fn decompose_sequential(&self, task: &str) -> Vec<SubTask> {
        let steps = self.extract_steps(task);
        let mut subtasks = Vec::new();
        let mut prev_id: Option<String> = None;

        for (i, step) in steps.into_iter().enumerate() {
            let id = format!("step_{}", i + 1);
            let mut subtask = SubTask::new(&id, format!("Step {}", i + 1), step);
            subtask.priority = i as u32;

            if let Some(prev) = prev_id {
                subtask = subtask.with_dependency(prev);
            }

            prev_id = Some(id);
            subtasks.push(subtask);
        }

        if subtasks.is_empty() {
            subtasks.push(SubTask::new("step_1", "Execute task", task));
        }

        subtasks
    }

    /// 并行分解
    fn decompose_parallel(&self, task: &str) -> Vec<SubTask> {
        let parts = self.extract_parallel_parts(task);
        let mut subtasks = Vec::new();

        for (i, part) in parts.into_iter().enumerate() {
            let id = format!("parallel_{}", i + 1);
            let subtask = SubTask::new(&id, format!("Task {}", i + 1), part);
            subtasks.push(subtask);
        }

        if subtasks.is_empty() {
            subtasks.push(SubTask::new("parallel_1", "Execute task", task));
        }

        subtasks
    }

    /// 层次分解
    fn decompose_hierarchical(&self, task: &str, depth: u32) -> Vec<SubTask> {
        if depth >= self.max_depth {
            return vec![SubTask::new(format!("leaf_{}", depth), "Execute", task)];
        }

        let main_steps = self.extract_steps(task);
        let mut subtasks = Vec::new();

        for (i, step) in main_steps.into_iter().enumerate() {
            let id = format!("h{}_{}", depth, i + 1);
            let mut subtask = SubTask::new(&id, format!("Phase {}", i + 1), step.clone());
            subtask.estimated_complexity = self.analyze_complexity(&step);

            // 如果步骤仍然复杂，继续分解
            if subtask.estimated_complexity > 5 && depth < self.max_depth - 1 {
                let sub_subtasks = self.decompose_hierarchical(&step, depth + 1);
                for (j, sub_sub) in sub_subtasks.into_iter().enumerate() {
                    let mut sub_sub_id = sub_sub;
                    sub_sub_id.id = format!("{}_{}", id, j + 1);
                    sub_sub_id.dependencies.push(id.clone());
                    subtasks.push(sub_sub_id);
                }
            }

            subtasks.push(subtask);
        }

        subtasks
    }

    /// 混合分解
    fn decompose_hybrid(&self, task: &str) -> Vec<SubTask> {
        let complexity = self.analyze_complexity(task);

        if complexity <= 3 {
            // 简单任务，单步执行
            vec![SubTask::new("execute", "Execute task", task)]
        } else if complexity <= 6 {
            // 中等复杂度，顺序分解
            self.decompose_sequential(task)
        } else {
            // 高复杂度，层次分解
            self.decompose_hierarchical(task, 0)
        }
    }

    /// 提取步骤
    fn extract_steps(&self, task: &str) -> Vec<String> {
        let mut steps = Vec::new();

        // 按句号、分号、换行分割
        let sentences: Vec<&str> = task
            .split(&['.', ';', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.len() > 1 {
            steps = sentences.into_iter().map(|s| s.to_string()).collect();
        } else {
            // 尝试按 "and then" 或 "then" 分割
            let mut then_parts: Vec<&str> = task.split("and then").collect();
            if then_parts.len() == 1 {
                then_parts = task.split("then").collect();
            }
            if then_parts.len() == 1 {
                then_parts = task.split("after that").collect();
            }

            if then_parts.len() > 1 {
                steps = then_parts
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
            } else {
                // 单一任务
                steps.push(task.to_string());
            }
        }

        steps
    }

    /// 提取并行部分
    fn extract_parallel_parts(&self, task: &str) -> Vec<String> {
        // 按 "and" 或逗号分割
        let mut parts: Vec<&str> = task.split(", and ").collect();
        if parts.len() == 1 {
            parts = task.split(" and ").collect();
        }
        if parts.len() == 1 {
            parts = task.split(", ").collect();
        }

        let parts: Vec<&str> = parts
            .into_iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && s.len() > 3)
            .collect();

        if parts.len() > 1 {
            parts.into_iter().map(|s| s.to_string()).collect()
        } else {
            vec![task.to_string()]
        }
    }
}

/// 规划结果
#[derive(Debug, Clone)]
pub struct PlanResult {
    /// 执行计划
    pub plan: ExecutionPlan,
    /// 分解质量评分 (0-100)
    pub quality_score: u32,
    /// 分解建议
    pub suggestions: Vec<String>,
}

impl PlanResult {
    /// 创建新的规划结果
    pub fn new(plan: ExecutionPlan) -> Self {
        let quality_score = Self::calculate_quality(&plan);
        let suggestions = Self::generate_suggestions(&plan);

        Self {
            plan,
            quality_score,
            suggestions,
        }
    }

    /// 计算分解质量
    fn calculate_quality(plan: &ExecutionPlan) -> u32 {
        let mut score = 100u32;

        // 检查子任务数量
        if plan.subtasks.is_empty() {
            score = 0;
        } else if plan.subtasks.len() == 1 {
            score -= 20; // 未分解
        }

        // 检查执行顺序是否合理
        if plan.execution_order.len() != plan.subtasks.len() {
            score -= 30;
        }

        // 检查是否有验证条件
        let has_validation = plan
            .subtasks
            .iter()
            .any(|s| !s.validation_criteria.is_empty());
        if !has_validation {
            score -= 10;
        }

        // 检查是否有失败处理
        let has_fallback = plan.subtasks.iter().any(|s| s.fallback.is_some());
        if !has_fallback && plan.risk_level >= RiskLevel::High {
            score -= 15;
        }

        score
    }

    /// 生成建议
    fn generate_suggestions(plan: &ExecutionPlan) -> Vec<String> {
        let mut suggestions = Vec::new();

        if plan.subtasks.len() == 1 {
            suggestions.push("Consider breaking down the task into smaller subtasks".to_string());
        }

        if plan.risk_level >= RiskLevel::High {
            suggestions.push("High-risk task: consider adding validation steps".to_string());
        }

        let has_fallback = plan.subtasks.iter().any(|s| s.fallback.is_some());
        if !has_fallback && !plan.subtasks.is_empty() {
            suggestions
                .push("Consider adding fallback strategies for critical subtasks".to_string());
        }

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DecompositionStrategy, ExecutionPlan, PlanResult, RiskLevel, SubTask, TaskDecomposer,
    };

    #[test]
    fn test_subtask_creation() {
        let subtask = SubTask::new("test_1", "Test", "Test subtask");
        assert_eq!(subtask.id, "test_1");
        assert_eq!(subtask.name, "Test");
    }

    #[test]
    fn test_subtask_with_dependencies() {
        let subtask = SubTask::new("test_2", "Test", "Test").with_dependency("test_1");
        assert_eq!(subtask.dependencies.len(), 1);
    }

    #[test]
    fn test_execution_plan_creation() {
        let plan = ExecutionPlan::new("Test task");
        assert!(!plan.original_task.is_empty());
        assert!(plan.subtasks.is_empty());
    }

    #[test]
    fn test_task_decomposer() {
        let decomposer = TaskDecomposer::new();
        let plan = decomposer
            .decompose("Create a file and write some content")
            .unwrap();

        assert!(!plan.subtasks.is_empty());
        assert!(!plan.execution_order.is_empty());
    }

    #[test]
    fn test_complexity_analysis() {
        let decomposer = TaskDecomposer::new();

        let simple = decomposer.analyze_complexity("Read a file");
        assert!(simple <= 3);

        let complex = decomposer.analyze_complexity(
            "Implement a complete authentication system with OAuth2 integration",
        );
        assert!(complex > 3);
    }

    #[test]
    fn test_risk_estimation() {
        let decomposer = TaskDecomposer::new();

        let low = decomposer.estimate_risk("Read a file", 2);
        assert_eq!(low, RiskLevel::Low);

        let critical = decomposer.estimate_risk("Delete the production database", 5);
        assert_eq!(critical, RiskLevel::Critical);
    }

    #[test]
    fn test_sequential_decomposition() {
        let decomposer = TaskDecomposer::new().with_strategy(DecompositionStrategy::Sequential);

        let plan = decomposer
            .decompose("First step. Second step. Third step.")
            .unwrap();

        assert!(plan.subtasks.len() >= 3);
        // 验证依赖链
        for i in 1..plan.subtasks.len() {
            assert!(plan.subtasks[i]
                .dependencies
                .contains(&plan.subtasks[i - 1].id));
        }
    }

    #[test]
    fn test_parallel_decomposition() {
        let decomposer = TaskDecomposer::new().with_strategy(DecompositionStrategy::Parallel);

        let plan = decomposer
            .decompose("Task A and Task B and Task C")
            .unwrap();

        assert!(plan.subtasks.len() >= 2);
        // 并行任务不应该有依赖
        let has_deps: bool = plan.subtasks.iter().any(|s| !s.dependencies.is_empty());
        assert!(!has_deps);
    }

    #[test]
    fn test_plan_result_quality() {
        let mut plan = ExecutionPlan::new("Test task");
        plan.add_subtask(SubTask::new("s1", "Step 1", "First step"));
        plan.add_subtask(SubTask::new("s2", "Step 2", "Second step").with_dependency("s1"));
        plan.compute_execution_order().unwrap();

        let result = PlanResult::new(plan);
        assert!(result.quality_score > 0);
    }

    #[test]
    fn test_dag_conversion() {
        let mut plan = ExecutionPlan::new("Test task");
        plan.add_subtask(SubTask::new("s1", "Step 1", "First step"));
        plan.add_subtask(SubTask::new("s2", "Step 2", "Second step").with_dependency("s1"));
        plan.compute_execution_order().unwrap();

        let dag_result = plan.to_dag();
        assert!(dag_result.is_ok());
    }
}
