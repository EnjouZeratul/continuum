//! # Execution Monitor
//!
//! 执行监控和自我纠错机制。
//!
//! 与 retry.rs 的 ErrorRecovery 集成，提供智能纠错能力。

use crate::checkpoint_system::{ErrorCategory, ErrorRecovery, RecoveryLayer};
use crate::planner::{ExecutionPlan, RiskLevel};
use crate::types::Layer2Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 执行状态
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// 未开始
    #[default]
    Pending,
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 步骤成功完成
    StepCompleted,
    /// 步骤失败
    StepFailed,
    /// 整体完成
    Completed,
    /// 整体失败
    Failed,
    /// 用户介入中
    AwaitingUserInput,
    /// 已取消
    Cancelled,
}

/// 步骤执行结果
#[derive(Debug, Clone)]
pub struct StepResult {
    /// 子任务ID
    pub subtask_id: String,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 输出内容
    pub output: Option<String>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时长
    pub duration: Duration,
    /// 重试次数
    pub retry_count: u32,
    /// 使用的恢复层
    pub recovery_layer: Option<RecoveryLayer>,
}

/// 执行监控器
#[allow(clippy::type_complexity)]
pub struct ExecutionMonitor {
    /// 执行计划
    plan: Arc<RwLock<ExecutionPlan>>,
    /// 当前状态
    status: Arc<RwLock<ExecutionStatus>>,
    /// 步骤结果
    step_results: Arc<RwLock<HashMap<String, StepResult>>>,
    /// 错误恢复器
    #[allow(dead_code)]
    error_recovery: Arc<ErrorRecovery>,
    /// 开始时间
    start_time: Arc<RwLock<Option<Instant>>>,
    /// 进度回调
    progress_callback: Arc<RwLock<Option<Box<dyn Fn(&str, ExecutionStatus) + Send + Sync>>>>,
    /// 纠错历史
    correction_history: Arc<RwLock<Vec<CorrectionRecord>>>,
}

/// 纠错记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionRecord {
    /// 记录ID
    pub id: String,
    /// 失败的子任务ID
    pub failed_subtask: String,
    /// 错误类型 (字符串表示)
    pub error_category: String,
    /// 原始错误消息
    pub original_error: String,
    /// 应用的纠错策略
    pub strategy: CorrectionStrategy,
    /// 纠错是否成功
    pub success: bool,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 纠错策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrectionStrategy {
    /// 重试执行
    Retry { max_attempts: u32 },
    /// 跳过步骤
    Skip,
    /// 替代方案
    Alternative { replacement_subtask: String },
    /// 分解子任务
    Decompose { new_subtasks: Vec<String> },
    /// 用户介入
    UserIntervention { action: String },
    /// 调整参数
    AdjustParameters { new_params: serde_json::Value },
}

impl CorrectionStrategy {
    /// 获取策略名称（用于调试输出）
    pub fn debug_name(&self) -> &'static str {
        match self {
            CorrectionStrategy::Retry { .. } => "Retry",
            CorrectionStrategy::Skip => "Skip",
            CorrectionStrategy::Alternative { .. } => "Alternative",
            CorrectionStrategy::Decompose { .. } => "Decompose",
            CorrectionStrategy::UserIntervention { .. } => "UserIntervention",
            CorrectionStrategy::AdjustParameters { .. } => "AdjustParameters",
        }
    }
}

impl ExecutionMonitor {
    /// 创建新的执行监控器
    pub fn new(plan: ExecutionPlan) -> Self {
        Self {
            plan: Arc::new(RwLock::new(plan)),
            status: Arc::new(RwLock::new(ExecutionStatus::Pending)),
            step_results: Arc::new(RwLock::new(HashMap::new())),
            error_recovery: Arc::new(ErrorRecovery::new()),
            start_time: Arc::new(RwLock::new(None)),
            progress_callback: Arc::new(RwLock::new(None)),
            correction_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 设置进度回调
    pub async fn set_progress_callback<F>(&self, callback: F)
    where
        F: Fn(&str, ExecutionStatus) + Send + Sync + 'static,
    {
        *self.progress_callback.write().await = Some(Box::new(callback));
    }

    /// 获取当前状态
    pub async fn get_status(&self) -> ExecutionStatus {
        *self.status.read().await
    }

    /// 获取执行进度 (0-100)
    pub async fn get_progress(&self) -> u32 {
        let plan = self.plan.read().await;
        let results = self.step_results.read().await;

        if plan.subtasks.is_empty() {
            return 0;
        }

        let completed = results
            .values()
            .filter(|r| matches!(r.status, ExecutionStatus::StepCompleted))
            .count();

        (completed as u32 * 100) / plan.subtasks.len() as u32
    }

    /// 开始执行
    pub async fn start(&self) -> Layer2Result<()> {
        let mut status = self.status.write().await;
        *status = ExecutionStatus::Running;
        drop(status);

        *self.start_time.write().await = Some(Instant::now());

        // 通知开始
        self.notify_progress("execution_started", ExecutionStatus::Running)
            .await;

        Ok(())
    }

    /// 报告步骤完成
    pub async fn report_step_completed(
        &self,
        subtask_id: &str,
        output: String,
    ) -> Layer2Result<()> {
        let result = StepResult {
            subtask_id: subtask_id.to_string(),
            status: ExecutionStatus::StepCompleted,
            output: Some(output),
            error: None,
            duration: Duration::from_secs(0),
            retry_count: 0,
            recovery_layer: None,
        };

        self.step_results
            .write()
            .await
            .insert(subtask_id.to_string(), result);
        self.notify_progress(subtask_id, ExecutionStatus::StepCompleted)
            .await;

        Ok(())
    }

    /// 报告步骤失败
    pub async fn report_step_failed(
        &self,
        subtask_id: &str,
        error: String,
    ) -> Layer2Result<CorrectionDecision> {
        // 分析错误类型
        let category = ErrorCategory::from_error_message(&error);

        // 记录失败
        let result = StepResult {
            subtask_id: subtask_id.to_string(),
            status: ExecutionStatus::StepFailed,
            output: None,
            error: Some(error.clone()),
            duration: Duration::from_secs(0),
            retry_count: 0,
            recovery_layer: None,
        };

        self.step_results
            .write()
            .await
            .insert(subtask_id.to_string(), result);

        // 决定纠错策略
        let decision = self.decide_correction(subtask_id, &category, &error).await;

        // 记录纠错决策
        self.record_correction(subtask_id, &category, &error, &decision)
            .await;

        self.notify_progress(subtask_id, ExecutionStatus::StepFailed)
            .await;

        Ok(decision)
    }

    /// 决定纠错策略
    async fn decide_correction(
        &self,
        subtask_id: &str,
        category: &ErrorCategory,
        error: &str,
    ) -> CorrectionDecision {
        let plan = self.plan.read().await;

        // 查找子任务
        let subtask = plan.subtasks.iter().find(|s| s.id == subtask_id);

        // 根据错误类型和风险等级决定策略
        match category {
            ErrorCategory::Transient => {
                // 临时错误：重试
                CorrectionDecision {
                    strategy: CorrectionStrategy::Retry { max_attempts: 3 },
                    should_continue: true,
                    user_message: Some("Temporary error, will retry automatically".to_string()),
                }
            }
            ErrorCategory::Resource => {
                // 资源错误：等待后重试
                CorrectionDecision {
                    strategy: CorrectionStrategy::Retry { max_attempts: 2 },
                    should_continue: true,
                    user_message: Some("Resource issue, waiting before retry".to_string()),
                }
            }
            ErrorCategory::Logic => {
                // 逻辑错误：检查是否有替代方案
                if let Some(subtask) = subtask {
                    if let Some(fallback) = &subtask.fallback {
                        CorrectionDecision {
                            strategy: CorrectionStrategy::Alternative {
                                replacement_subtask: fallback.name.clone(),
                            },
                            should_continue: true,
                            user_message: Some("Using fallback strategy".to_string()),
                        }
                    } else {
                        // 尝试分解
                        CorrectionDecision {
                            strategy: CorrectionStrategy::Decompose {
                                new_subtasks: vec!["simplified_step".to_string()],
                            },
                            should_continue: true,
                            user_message: Some("Breaking down the task".to_string()),
                        }
                    }
                } else {
                    CorrectionDecision {
                        strategy: CorrectionStrategy::Skip,
                        should_continue: true,
                        user_message: Some("Skipping failed step".to_string()),
                    }
                }
            }
            ErrorCategory::Configuration => {
                // 配置错误：需要用户介入
                CorrectionDecision {
                    strategy: CorrectionStrategy::UserIntervention {
                        action: "Please check your configuration".to_string(),
                    },
                    should_continue: false,
                    user_message: Some(format!("Configuration error: {}", error)),
                }
            }
            ErrorCategory::UserInterrupt => {
                // 用户中断：停止执行
                CorrectionDecision {
                    strategy: CorrectionStrategy::Skip,
                    should_continue: false,
                    user_message: Some("Execution cancelled by user".to_string()),
                }
            }
            ErrorCategory::System => {
                // 系统错误：根据风险等级决定
                if plan.risk_level == RiskLevel::Critical {
                    CorrectionDecision {
                        strategy: CorrectionStrategy::UserIntervention {
                            action: "Critical error requires manual intervention".to_string(),
                        },
                        should_continue: false,
                        user_message: Some(format!("Critical system error: {}", error)),
                    }
                } else {
                    CorrectionDecision {
                        strategy: CorrectionStrategy::Retry { max_attempts: 1 },
                        should_continue: true,
                        user_message: Some("System error, attempting recovery".to_string()),
                    }
                }
            }
        }
    }

    /// 记录纠错
    async fn record_correction(
        &self,
        subtask_id: &str,
        category: &ErrorCategory,
        error: &str,
        decision: &CorrectionDecision,
    ) {
        let category_str = match category {
            ErrorCategory::Transient => "Transient",
            ErrorCategory::Resource => "Resource",
            ErrorCategory::Configuration => "Configuration",
            ErrorCategory::Logic => "Logic",
            ErrorCategory::System => "System",
            ErrorCategory::UserInterrupt => "UserInterrupt",
        };
        let record = CorrectionRecord {
            id: format!("correction_{}", chrono::Utc::now().timestamp()),
            failed_subtask: subtask_id.to_string(),
            error_category: category_str.to_string(),
            original_error: error.to_string(),
            strategy: decision.strategy.clone(),
            success: false, // 开始时标记为失败，成功后更新
            timestamp: chrono::Utc::now(),
        };

        self.correction_history.write().await.push(record);
    }

    /// 应用纠错策略
    pub async fn apply_correction(
        &self,
        subtask_id: &str,
        decision: &CorrectionDecision,
    ) -> Layer2Result<bool> {
        match &decision.strategy {
            CorrectionStrategy::Retry { max_attempts: _ } => {
                // 使用 ErrorRecovery 进行重试
                // 简化实现：返回需要重试
                Ok(true)
            }
            CorrectionStrategy::Skip => {
                // 跳过步骤，标记为完成（带警告）
                self.report_step_completed(
                    subtask_id,
                    "[SKIPPED] Step skipped due to unrecoverable error".to_string(),
                )
                .await?;
                Ok(true)
            }
            CorrectionStrategy::Alternative {
                replacement_subtask,
            } => {
                // 应用替代方案
                self.report_step_completed(
                    subtask_id,
                    format!("[ALTERNATIVE] Used: {}", replacement_subtask),
                )
                .await?;
                Ok(true)
            }
            CorrectionStrategy::UserIntervention { action: _ } => {
                // 标记需要用户介入
                let mut status = self.status.write().await;
                *status = ExecutionStatus::AwaitingUserInput;
                Ok(false)
            }
            CorrectionStrategy::Decompose { new_subtasks } => {
                // 标记已分解
                self.report_step_completed(
                    subtask_id,
                    format!("[DECOMPOSED] Into: {}", new_subtasks.join(", ")),
                )
                .await?;
                Ok(true)
            }
            CorrectionStrategy::AdjustParameters { new_params: _ } => {
                // 标记参数已调整
                self.report_step_completed(
                    subtask_id,
                    "[ADJUSTED] Parameters modified".to_string(),
                )
                .await?;
                Ok(true)
            }
        }
    }

    /// 完成执行
    pub async fn complete(&self) -> Layer2Result<ExecutionSummary> {
        let mut status = self.status.write().await;
        *status = ExecutionStatus::Completed;
        drop(status);

        self.notify_progress("execution_completed", ExecutionStatus::Completed)
            .await;

        let plan = self.plan.read().await;
        let results = self.step_results.read().await;
        let corrections = self.correction_history.read().await;
        let start_time = self.start_time.read().await;

        let completed = results
            .values()
            .filter(|r| matches!(r.status, ExecutionStatus::StepCompleted))
            .count();
        let failed = results
            .values()
            .filter(|r| matches!(r.status, ExecutionStatus::StepFailed))
            .count();
        let skipped = results
            .values()
            .filter(|r| {
                r.output
                    .as_ref()
                    .map(|o| o.starts_with("[SKIPPED]"))
                    .unwrap_or(false)
            })
            .count();

        Ok(ExecutionSummary {
            plan_id: plan.id.clone(),
            total_steps: plan.subtasks.len(),
            completed_steps: completed,
            failed_steps: failed,
            skipped_steps: skipped,
            correction_count: corrections.len(),
            duration: start_time.map(|t| t.elapsed()).unwrap_or(Duration::ZERO),
            status: ExecutionStatus::Completed,
        })
    }

    /// 获取纠错历史
    pub async fn get_correction_history(&self) -> Vec<CorrectionRecord> {
        self.correction_history.read().await.clone()
    }

    /// 通知进度
    async fn notify_progress(&self, subtask_id: &str, status: ExecutionStatus) {
        if let Some(callback) = self.progress_callback.read().await.as_ref() {
            callback(subtask_id, status);
        }
    }
}

/// 纠错决策
#[derive(Debug, Clone)]
pub struct CorrectionDecision {
    /// 纠错策略
    pub strategy: CorrectionStrategy,
    /// 是否继续执行
    pub should_continue: bool,
    /// 用户消息
    pub user_message: Option<String>,
}

/// 执行摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// 计划ID
    pub plan_id: String,
    /// 总步骤数
    pub total_steps: usize,
    /// 完成步骤数
    pub completed_steps: usize,
    /// 失败步骤数
    pub failed_steps: usize,
    /// 跳过步骤数
    pub skipped_steps: usize,
    /// 纠错次数
    pub correction_count: usize,
    /// 执行时长
    pub duration: Duration,
    /// 最终状态
    pub status: ExecutionStatus,
}

/// 自我纠错器
pub struct SelfCorrector {
    /// 纠错历史
    history: RwLock<Vec<CorrectionRecord>>,
    /// 学习到的模式
    patterns: RwLock<HashMap<String, CorrectionStrategy>>,
}

impl Default for SelfCorrector {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfCorrector {
    /// 创建新的自我纠错器
    pub fn new() -> Self {
        Self {
            history: RwLock::new(Vec::new()),
            patterns: RwLock::new(HashMap::new()),
        }
    }

    /// 学习纠错模式
    pub async fn learn_pattern(&self, error_signature: &str, strategy: CorrectionStrategy) {
        self.patterns
            .write()
            .await
            .insert(error_signature.to_string(), strategy);
    }

    /// 获取推荐的纠错策略
    pub async fn get_recommended_strategy(&self, error: &str) -> Option<CorrectionStrategy> {
        let patterns = self.patterns.read().await;

        // 查找匹配的模式
        for (signature, strategy) in patterns.iter() {
            if error.contains(signature) {
                return Some(strategy.clone());
            }
        }

        None
    }

    /// 记录纠错结果
    pub async fn record_result(&self, record: CorrectionRecord) {
        // 如果纠错成功，学习这个模式
        if record.success {
            let signature = Self::extract_signature(&record.original_error);
            self.learn_pattern(&signature, record.strategy.clone())
                .await;
        }

        self.history.write().await.push(record);
    }

    /// 提取错误签名
    fn extract_signature(error: &str) -> String {
        // 简化实现：提取前50个字符作为签名
        let error_lower = error.to_lowercase();
        if error_lower.len() > 50 {
            error_lower[..50].to_string()
        } else {
            error_lower
        }
    }

    /// 获取纠错成功率
    pub async fn get_success_rate(&self) -> f32 {
        let history = self.history.read().await;
        if history.is_empty() {
            return 0.0;
        }

        let success_count = history.iter().filter(|r| r.success).count();
        success_count as f32 / history.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{ExecutionPlan, SubTask};

    #[tokio::test]
    async fn test_execution_monitor_creation() {
        let plan = ExecutionPlan::new("Test task");
        let monitor = ExecutionMonitor::new(plan);

        let status = monitor.get_status().await;
        assert_eq!(status, ExecutionStatus::Pending);
    }

    #[tokio::test]
    async fn test_progress_calculation() {
        let mut plan = ExecutionPlan::new("Test task");
        plan.add_subtask(SubTask::new("s1", "Step 1", "First"));
        plan.add_subtask(SubTask::new("s2", "Step 2", "Second"));
        plan.compute_execution_order().unwrap();

        let monitor = ExecutionMonitor::new(plan);
        monitor.start().await.unwrap();

        assert_eq!(monitor.get_progress().await, 0);

        monitor
            .report_step_completed("s1", "Done".to_string())
            .await
            .unwrap();
        assert_eq!(monitor.get_progress().await, 50);

        monitor
            .report_step_completed("s2", "Done".to_string())
            .await
            .unwrap();
        assert_eq!(monitor.get_progress().await, 100);
    }

    #[tokio::test]
    async fn test_correction_decision() {
        let plan = ExecutionPlan::new("Test task");
        let monitor = ExecutionMonitor::new(plan);

        let decision = monitor
            .decide_correction("test_subtask", &ErrorCategory::Transient, "Network timeout")
            .await;

        assert!(decision.should_continue);
        matches!(decision.strategy, CorrectionStrategy::Retry { .. });
    }

    #[tokio::test]
    async fn test_self_corrector_learning() {
        let corrector = SelfCorrector::new();

        corrector
            .learn_pattern("timeout", CorrectionStrategy::Retry { max_attempts: 3 })
            .await;

        let strategy = corrector
            .get_recommended_strategy("Connection timeout occurred")
            .await;
        assert!(strategy.is_some());
    }

    #[tokio::test]
    async fn test_correction_history() {
        let plan = ExecutionPlan::new("Test task");
        let monitor = ExecutionMonitor::new(plan);

        monitor
            .report_step_failed("s1", "Error occurred".to_string())
            .await
            .unwrap();

        let history = monitor.get_correction_history().await;
        assert!(!history.is_empty());
    }
}
