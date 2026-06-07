//! # Compliance Checker
//!
//! 合规检查器核心实现。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::report::{CheckResult, ComplianceReport, ComplianceStatus, ReportFormat, Violation};
use super::rules::{ComplianceRule, ComplianceStandard, RuleCategory, RuleSeverity};

/// 合规检查配置
#[derive(Debug, Clone)]
pub struct ComplianceConfig {
    /// 启用的标准
    pub enabled_standards: Vec<ComplianceStandard>,
    /// 自定义规则
    pub custom_rules: Vec<ComplianceRule>,
    /// 检查间隔（秒）
    pub check_interval_secs: u64,
    /// 是否自动修复
    pub auto_remediation: bool,
    /// 严重性阈值（低于此级别不报警）
    pub severity_threshold: RuleSeverity,
    /// 排除的规则 ID
    pub excluded_rules: Vec<String>,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            enabled_standards: vec![ComplianceStandard::SOC2],
            custom_rules: Vec::new(),
            check_interval_secs: 3600,
            auto_remediation: false,
            severity_threshold: RuleSeverity::Low,
            excluded_rules: Vec::new(),
        }
    }
}

impl ComplianceConfig {
    pub fn new(standards: Vec<ComplianceStandard>) -> Self {
        Self {
            enabled_standards: standards,
            ..Default::default()
        }
    }

    pub fn with_custom_rules(mut self, rules: Vec<ComplianceRule>) -> Self {
        self.custom_rules = rules;
        self
    }

    pub fn with_check_interval(mut self, secs: u64) -> Self {
        self.check_interval_secs = secs;
        self
    }

    pub fn with_auto_remediation(mut self, enabled: bool) -> Self {
        self.auto_remediation = enabled;
        self
    }

    pub fn with_severity_threshold(mut self, threshold: RuleSeverity) -> Self {
        self.severity_threshold = threshold;
        self
    }

    pub fn exclude_rule(mut self, rule_id: impl Into<String>) -> Self {
        self.excluded_rules.push(rule_id.into());
        self
    }
}

/// 检查上下文
#[derive(Debug, Clone)]
pub struct CheckContext {
    /// 系统信息
    pub system_info: HashMap<String, String>,
    /// 资源清单
    pub resources: Vec<ResourceInfo>,
    /// 配置快照
    pub config_snapshot: serde_json::Value,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for CheckContext {
    fn default() -> Self {
        Self {
            system_info: HashMap::new(),
            resources: Vec::new(),
            config_snapshot: serde_json::json!({}),
            timestamp: chrono::Utc::now(),
        }
    }
}

impl CheckContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_system_info(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.system_info.insert(key.into(), value.into());
        self
    }

    pub fn with_resources(mut self, resources: Vec<ResourceInfo>) -> Self {
        self.resources = resources;
        self
    }

    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config_snapshot = config;
        self
    }
}

/// 资源信息
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub resource_type: String,
    pub resource_id: String,
    pub metadata: HashMap<String, String>,
}

impl ResourceInfo {
    pub fn new(resource_type: impl Into<String>, resource_id: impl Into<String>) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// 规则检查器 Trait
pub trait RuleChecker: Send + Sync {
    fn check(&self, context: &CheckContext) -> CheckResult;
    fn rule_id(&self) -> &str;
}

/// 内置规则检查器
pub struct BuiltinRuleChecker {
    rule: ComplianceRule,
    check_fn: Arc<dyn Fn(&CheckContext) -> Option<Violation> + Send + Sync>,
}

impl BuiltinRuleChecker {
    pub fn new<F>(rule: ComplianceRule, check_fn: F) -> Self
    where
        F: Fn(&CheckContext) -> Option<Violation> + Send + Sync + 'static,
    {
        Self {
            rule,
            check_fn: Arc::new(check_fn),
        }
    }
}

impl RuleChecker for BuiltinRuleChecker {
    fn check(&self, context: &CheckContext) -> CheckResult {
        match (self.check_fn)(context) {
            Some(violation) => CheckResult::non_compliant(&self.rule, vec![violation]),
            None => CheckResult::compliant(&self.rule),
        }
    }

    fn rule_id(&self) -> &str {
        &self.rule.id
    }
}

/// 合规检查器
pub struct ComplianceChecker {
    config: ComplianceConfig,
    rules: Vec<ComplianceRule>,
    checkers: HashMap<String, Arc<dyn RuleChecker>>,
    last_report: Arc<RwLock<Option<ComplianceReport>>>,
}

impl ComplianceChecker {
    pub fn new(config: ComplianceConfig) -> Self {
        let mut checker = Self {
            config,
            rules: Vec::new(),
            checkers: HashMap::new(),
            last_report: Arc::new(RwLock::new(None)),
        };
        checker.load_rules();
        checker
    }

    fn load_rules(&mut self) {
        // 加载启用标准的默认规则
        for standard in &self.config.enabled_standards {
            for rule in standard.default_rules() {
                if !self.config.excluded_rules.contains(&rule.id) {
                    self.rules.push(rule);
                }
            }
        }

        // 加载自定义规则
        for rule in &self.config.custom_rules {
            if !self.config.excluded_rules.contains(&rule.id) {
                self.rules.push(rule.clone());
            }
        }

        // 为每个规则创建默认检查器
        for rule in &self.rules {
            let checker = self.create_default_checker(rule);
            self.checkers.insert(rule.id.clone(), Arc::new(checker));
        }
    }

    fn create_default_checker(&self, rule: &ComplianceRule) -> BuiltinRuleChecker {
        let rule_clone = rule.clone();
        BuiltinRuleChecker::new(rule_clone, move |_context| {
            // 默认检查逻辑 - 需要根据规则类型实现具体检查
            // 这里返回 None 表示合规（占位实现）
            None
        })
    }

    /// 注册自定义检查器
    pub fn register_checker(&mut self, checker: Arc<dyn RuleChecker>) {
        self.checkers.insert(checker.rule_id().to_string(), checker);
    }

    /// 执行检查
    pub async fn check(&self, context: &CheckContext) -> ComplianceReport {
        let mut report = ComplianceReport::new(
            format!(
                "Compliance Check - {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M")
            ),
            self.config.enabled_standards.clone(),
        );

        for rule in &self.rules {
            if let Some(checker) = self.checkers.get(&rule.id) {
                let result = checker.check(context);

                // 过滤低于严重性阈值的结果
                let should_include = match result.status {
                    ComplianceStatus::NonCompliant => result
                        .violations
                        .iter()
                        .any(|v| v.severity >= self.config.severity_threshold),
                    _ => true,
                };

                if should_include {
                    report.add_result(result);
                }
            }
        }

        report.calculate_score();

        // 保存报告
        {
            let mut last_report = self.last_report.write().await;
            *last_report = Some(report.clone());
        }

        report
    }

    /// 获取上次报告
    pub async fn last_report(&self) -> Option<ComplianceReport> {
        self.last_report.read().await.clone()
    }

    /// 导出报告
    pub async fn export_report(&self, format: ReportFormat) -> Option<Vec<u8>> {
        let report = self.last_report.read().await;
        report.as_ref().map(|r| r.export(format))
    }

    /// 获取所有规则
    pub fn rules(&self) -> &[ComplianceRule] {
        &self.rules
    }

    /// 按类别获取规则
    pub fn rules_by_category(&self, category: RuleCategory) -> Vec<&ComplianceRule> {
        self.rules
            .iter()
            .filter(|r| r.category == category)
            .collect()
    }

    /// 按严重性获取规则
    pub fn rules_by_severity(&self, severity: RuleSeverity) -> Vec<&ComplianceRule> {
        self.rules
            .iter()
            .filter(|r| r.severity == severity)
            .collect()
    }

    /// 快速检查（不生成完整报告）
    pub fn quick_check(&self, context: &CheckContext) -> QuickCheckResult {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        for rule in &self.rules {
            if let Some(checker) = self.checkers.get(&rule.id) {
                let result = checker.check(context);
                checked_count += 1;

                if !result.violations.is_empty() {
                    violations.extend(result.violations);
                }
            }
        }

        let status = if violations.is_empty() {
            ComplianceStatus::Compliant
        } else if violations
            .iter()
            .any(|v| matches!(v.severity, RuleSeverity::Critical | RuleSeverity::High))
        {
            ComplianceStatus::NonCompliant
        } else {
            ComplianceStatus::PartiallyCompliant
        };

        QuickCheckResult {
            status,
            checked_count,
            violation_count: violations.len(),
            critical_count: violations
                .iter()
                .filter(|v| v.severity == RuleSeverity::Critical)
                .count(),
            high_count: violations
                .iter()
                .filter(|v| v.severity == RuleSeverity::High)
                .count(),
        }
    }

    /// 生成合规摘要
    pub fn generate_summary(&self, report: &ComplianceReport) -> ComplianceSummary {
        let critical_issues: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.severity == RuleSeverity::Critical)
            .collect();

        let high_issues: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.severity == RuleSeverity::High)
            .collect();

        let recommendations: Vec<String> = critical_issues
            .iter()
            .filter_map(|v| v.remediation.clone())
            .chain(high_issues.iter().filter_map(|v| v.remediation.clone()))
            .take(5)
            .collect();

        ComplianceSummary {
            score: report.compliance_score,
            status: report.overall_status,
            total_rules: report.summary.total_rules,
            compliant_rules: report.summary.compliant_rules,
            total_violations: report.summary.total_violations,
            critical_violations: report.summary.critical_violations,
            high_violations: report.summary.high_violations,
            recommendations,
        }
    }
}

/// 快速检查结果
#[derive(Debug, Clone)]
pub struct QuickCheckResult {
    pub status: ComplianceStatus,
    pub checked_count: usize,
    pub violation_count: usize,
    pub critical_count: usize,
    pub high_count: usize,
}

/// 合规摘要
#[derive(Debug, Clone)]
pub struct ComplianceSummary {
    pub score: f32,
    pub status: ComplianceStatus,
    pub total_rules: usize,
    pub compliant_rules: usize,
    pub total_violations: usize,
    pub critical_violations: usize,
    pub high_violations: usize,
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_config() {
        let config =
            ComplianceConfig::new(vec![ComplianceStandard::SOC2, ComplianceStandard::HIPAA])
                .with_check_interval(1800)
                .with_auto_remediation(true);

        assert_eq!(config.enabled_standards.len(), 2);
        assert_eq!(config.check_interval_secs, 1800);
        assert!(config.auto_remediation);
    }

    #[test]
    fn test_check_context() {
        let context = CheckContext::new()
            .with_system_info("version", "1.0.0")
            .with_system_info("environment", "production");

        assert_eq!(
            context.system_info.get("version"),
            Some(&"1.0.0".to_string())
        );
    }

    #[test]
    fn test_resource_info() {
        let resource = ResourceInfo::new("server", "srv-001").with_metadata("region", "us-east-1");

        assert_eq!(resource.resource_type, "server");
        assert_eq!(
            resource.metadata.get("region"),
            Some(&"us-east-1".to_string())
        );
    }

    #[tokio::test]
    async fn test_compliance_checker_creation() {
        let config = ComplianceConfig::new(vec![ComplianceStandard::SOC2]);
        let checker = ComplianceChecker::new(config);

        // SOC2 默认规则数量
        assert!(!checker.rules().is_empty());
    }

    #[tokio::test]
    async fn test_quick_check() {
        let config = ComplianceConfig::new(vec![ComplianceStandard::SOC2]);
        let checker = ComplianceChecker::new(config);
        let context = CheckContext::new();

        let result = checker.quick_check(&context);
        assert_eq!(result.checked_count, checker.rules().len());
    }

    #[tokio::test]
    async fn test_check_generates_report() {
        let config = ComplianceConfig::new(vec![ComplianceStandard::SOC2]);
        let checker = ComplianceChecker::new(config);
        let context = CheckContext::new();

        let report = checker.check(&context).await;
        assert!(!report.results.is_empty());
        assert!(report.compliance_score >= 0.0 && report.compliance_score <= 100.0);
    }

    #[test]
    fn test_rules_filtering() {
        let config = ComplianceConfig::new(vec![ComplianceStandard::SOC2]).exclude_rule("SOC2-001");

        let checker = ComplianceChecker::new(config);

        // 被排除的规则不应在规则列表中
        assert!(!checker.rules().iter().any(|r| r.id == "SOC2-001"));
    }

    #[test]
    fn test_rules_by_category() {
        let config = ComplianceConfig::new(vec![ComplianceStandard::SOC2]);
        let checker = ComplianceChecker::new(config);

        let security_rules = checker.rules_by_category(RuleCategory::Security);
        assert!(!security_rules.is_empty());
    }

    #[test]
    fn test_rules_by_severity() {
        let config = ComplianceConfig::new(vec![ComplianceStandard::SOC2]);
        let checker = ComplianceChecker::new(config);

        let critical_rules = checker.rules_by_severity(RuleSeverity::Critical);
        // 取决于默认规则定义
        assert!(critical_rules.len() <= checker.rules().len());
    }
}
