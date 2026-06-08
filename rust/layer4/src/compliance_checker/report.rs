//! # Compliance Report
//!
//! 合规检查报告生成。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::rules::{ComplianceRule, ComplianceStandard, RuleCategory, RuleSeverity};

/// 合规状态
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceStatus {
    /// 合规
    Compliant,
    /// 不合规
    NonCompliant,
    /// 部分合规
    PartiallyCompliant,
    /// 未检查
    #[default]
    NotChecked,
    /// 不适用
    NotApplicable,
}

/// 报告格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// JSON 格式
    Json,
    /// CSV 格式
    Csv,
    /// HTML 格式
    Html,
    /// Markdown 格式
    Markdown,
    /// PDF 格式
    Pdf,
}

/// 违规项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// 违规 ID
    pub id: String,
    /// 规则 ID
    pub rule_id: String,
    /// 规则名称
    pub rule_name: String,
    /// 严重性
    pub severity: RuleSeverity,
    /// 类别
    pub category: RuleCategory,
    /// 违规描述
    pub description: String,
    /// 发现时间
    pub detected_at: DateTime<Utc>,
    /// 资源类型
    pub resource_type: Option<String>,
    /// 资源 ID
    pub resource_id: Option<String>,
    /// 建议修复措施
    pub remediation: Option<String>,
    /// 相关证据
    pub evidence: Option<serde_json::Value>,
}

impl Violation {
    pub fn new(rule: &ComplianceRule, description: impl Into<String>) -> Self {
        Self {
            id: format!("VIO-{}", uuid::Uuid::new_v4()),
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            severity: rule.severity,
            category: rule.category,
            description: description.into(),
            detected_at: Utc::now(),
            resource_type: None,
            resource_id: None,
            remediation: rule.description.clone(),
            evidence: None,
        }
    }

    pub fn with_resource(
        mut self,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> Self {
        self.resource_type = Some(resource_type.into());
        self.resource_id = Some(resource_id.into());
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn with_evidence(mut self, evidence: serde_json::Value) -> Self {
        self.evidence = Some(evidence);
        self
    }
}

/// 检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// 规则 ID
    pub rule_id: String,
    /// 规则名称
    pub rule_name: String,
    /// 检查状态
    pub status: ComplianceStatus,
    /// 检查时间
    pub checked_at: DateTime<Utc>,
    /// 违规项（如果有）
    pub violations: Vec<Violation>,
    /// 检查详情
    pub details: Option<String>,
}

impl CheckResult {
    pub fn compliant(rule: &ComplianceRule) -> Self {
        Self {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            status: ComplianceStatus::Compliant,
            checked_at: Utc::now(),
            violations: Vec::new(),
            details: None,
        }
    }

    pub fn non_compliant(rule: &ComplianceRule, violations: Vec<Violation>) -> Self {
        Self {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            status: if violations.is_empty() {
                ComplianceStatus::Compliant
            } else {
                ComplianceStatus::NonCompliant
            },
            checked_at: Utc::now(),
            violations,
            details: None,
        }
    }

    pub fn not_applicable(rule: &ComplianceRule) -> Self {
        Self {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            status: ComplianceStatus::NotApplicable,
            checked_at: Utc::now(),
            violations: Vec::new(),
            details: Some("Rule not applicable to this system".to_string()),
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// 合规报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// 报告 ID
    pub id: String,
    /// 报告名称
    pub name: String,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
    /// 检查的标准
    pub standards: Vec<ComplianceStandard>,
    /// 总体状态
    pub overall_status: ComplianceStatus,
    /// 合规分数 (0-100)
    pub compliance_score: f32,
    /// 检查结果
    pub results: Vec<CheckResult>,
    /// 所有违规
    pub violations: Vec<Violation>,
    /// 摘要统计
    pub summary: ReportSummary,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 报告摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ReportSummary {
    /// 检查的规则总数
    pub total_rules: usize,
    /// 合规规则数
    pub compliant_rules: usize,
    /// 不合规规则数
    pub non_compliant_rules: usize,
    /// 不适用规则数
    pub not_applicable_rules: usize,
    /// 总违规数
    pub total_violations: usize,
    /// 严重违规数
    pub critical_violations: usize,
    /// 高危违规数
    pub high_violations: usize,
    /// 中危违规数
    pub medium_violations: usize,
    /// 低危违规数
    pub low_violations: usize,
}

impl ComplianceReport {
    /// 创建新的合规报告
    pub fn new(name: impl Into<String>, standards: Vec<ComplianceStandard>) -> Self {
        Self {
            id: format!("RPT-{}", uuid::Uuid::new_v4()),
            name: name.into(),
            generated_at: Utc::now(),
            standards,
            overall_status: ComplianceStatus::NotChecked,
            compliance_score: 0.0,
            results: Vec::new(),
            violations: Vec::new(),
            summary: ReportSummary::default(),
            metadata: HashMap::new(),
        }
    }

    /// 添加检查结果
    pub fn add_result(&mut self, result: CheckResult) {
        // 更新摘要统计
        self.summary.total_rules += 1;

        match result.status {
            ComplianceStatus::Compliant => self.summary.compliant_rules += 1,
            ComplianceStatus::NonCompliant => self.summary.non_compliant_rules += 1,
            ComplianceStatus::PartiallyCompliant => self.summary.non_compliant_rules += 1,
            ComplianceStatus::NotApplicable => self.summary.not_applicable_rules += 1,
            ComplianceStatus::NotChecked => {}
        }

        // 统计违规
        for violation in &result.violations {
            self.summary.total_violations += 1;
            match violation.severity {
                RuleSeverity::Critical => self.summary.critical_violations += 1,
                RuleSeverity::High => self.summary.high_violations += 1,
                RuleSeverity::Medium => self.summary.medium_violations += 1,
                RuleSeverity::Low => self.summary.low_violations += 1,
            }
        }

        self.violations.extend(result.violations.clone());
        self.results.push(result);
    }

    /// 计算合规分数
    pub fn calculate_score(&mut self) {
        if self.summary.total_rules == 0 {
            self.compliance_score = 100.0;
            self.overall_status = ComplianceStatus::NotChecked;
            return;
        }

        let applicable_rules = self.summary.total_rules - self.summary.not_applicable_rules;
        if applicable_rules == 0 {
            self.compliance_score = 100.0;
            self.overall_status = ComplianceStatus::NotApplicable;
            return;
        }

        // 基础分数 = 合规规则数 / 适用规则数 * 100
        let base_score = (self.summary.compliant_rules as f32 / applicable_rules as f32) * 100.0;

        // 根据违规严重性扣分
        let penalty = (self.summary.critical_violations as f32 * 10.0)
            + (self.summary.high_violations as f32 * 5.0)
            + (self.summary.medium_violations as f32 * 2.0)
            + (self.summary.low_violations as f32 * 0.5);

        self.compliance_score = (base_score - penalty).clamp(0.0, 100.0);

        // 确定总体状态
        self.overall_status = if self.compliance_score >= 90.0 {
            ComplianceStatus::Compliant
        } else if self.compliance_score >= 60.0 {
            ComplianceStatus::PartiallyCompliant
        } else {
            ComplianceStatus::NonCompliant
        };
    }

    /// 导出报告
    pub fn export(&self, format: ReportFormat) -> Vec<u8> {
        match format {
            ReportFormat::Json => serde_json::to_string_pretty(self)
                .unwrap_or_default()
                .into_bytes(),
            ReportFormat::Csv => self.export_csv(),
            ReportFormat::Html => self.export_html().into_bytes(),
            ReportFormat::Markdown => self.export_markdown().into_bytes(),
            ReportFormat::Pdf => {
                // PDF 需要额外的依赖库，这里返回 HTML 作为占位
                self.export_html().into_bytes()
            }
        }
    }

    fn export_csv(&self) -> Vec<u8> {
        let mut csv = String::from("Rule ID,Rule Name,Status,Violations Count,Severity\n");
        for result in &self.results {
            let status = format!("{:?}", result.status);
            let violation_count = result.violations.len();
            let severity = result
                .violations
                .first()
                .map(|v| format!("{:?}", v.severity))
                .unwrap_or_else(|| "N/A".to_string());
            csv.push_str(&format!(
                "{},{},{},{},{}\n",
                result.rule_id, result.rule_name, status, violation_count, severity
            ));
        }
        csv.into_bytes()
    }

    fn export_html(&self) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Compliance Report - {}</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        h1 {{ color: #333; }}
        .score {{ font-size: 24px; font-weight: bold; margin: 20px 0; }}
        .compliant {{ color: green; }}
        .non-compliant {{ color: red; }}
        .partial {{ color: orange; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f2f2f2; }}
    </style>
</head>
<body>
    <h1>Compliance Report</h1>
    <p>Generated: {}</p>
    <p>Standards: {}</p>
    <div class="score">Compliance Score: {:.1}%</div>
    <h2>Summary</h2>
    <ul>
        <li>Total Rules: {}</li>
        <li>Compliant: {}</li>
        <li>Non-Compliant: {}</li>
        <li>Total Violations: {}</li>
    </ul>
    <h2>Results</h2>
    <table>
        <tr><th>Rule ID</th><th>Rule Name</th><th>Status</th><th>Violations</th></tr>
        {}
    </table>
</body>
</html>"#,
            self.name,
            self.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            self.standards
                .iter()
                .map(|s| s.name())
                .collect::<Vec<_>>()
                .join(", "),
            self.compliance_score,
            self.summary.total_rules,
            self.summary.compliant_rules,
            self.summary.non_compliant_rules,
            self.summary.total_violations,
            self.results
                .iter()
                .map(|r| format!(
                    "<tr><td>{}</td><td>{}</td><td>{:?}</td><td>{}</td></tr>",
                    r.rule_id,
                    r.rule_name,
                    r.status,
                    r.violations.len()
                ))
                .collect::<Vec<_>>()
                .join("\n        ")
        )
    }

    fn export_markdown(&self) -> String {
        format!(
            r#"# Compliance Report: {}

**Generated:** {}
**Standards:** {}
**Score:** {:.1}%

## Summary

| Metric | Count |
|--------|-------|
| Total Rules | {} |
| Compliant | {} |
| Non-Compliant | {} |
| Total Violations | {} |
| Critical Violations | {} |

## Results

| Rule ID | Rule Name | Status | Violations |
|---------|-----------|--------|------------|
{}

## Violations

{}
"#,
            self.name,
            self.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            self.standards
                .iter()
                .map(|s| s.name())
                .collect::<Vec<_>>()
                .join(", "),
            self.compliance_score,
            self.summary.total_rules,
            self.summary.compliant_rules,
            self.summary.non_compliant_rules,
            self.summary.total_violations,
            self.summary.critical_violations,
            self.results
                .iter()
                .map(|r| format!(
                    "| {} | {} | {:?} | {} |",
                    r.rule_id,
                    r.rule_name,
                    r.status,
                    r.violations.len()
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            self.violations
                .iter()
                .map(|v| format!(
                    "- **{}**: {} (Severity: {:?})",
                    v.rule_id, v.description, v.severity
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_creation() {
        let rule = ComplianceRule::new("TEST-1", "Test", RuleCategory::Security);
        let violation = Violation::new(&rule, "Test violation");

        assert_eq!(violation.rule_id, "TEST-1");
        assert_eq!(violation.description, "Test violation");
    }

    #[test]
    fn test_check_result_compliant() {
        let rule = ComplianceRule::new("TEST-1", "Test", RuleCategory::Security);
        let result = CheckResult::compliant(&rule);

        assert_eq!(result.status, ComplianceStatus::Compliant);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_compliance_report() {
        let mut report = ComplianceReport::new("Test Report", vec![ComplianceStandard::SOC2]);

        let rule = ComplianceRule::new("TEST-1", "Test", RuleCategory::Security);
        let result = CheckResult::compliant(&rule);

        report.add_result(result);
        report.calculate_score();

        assert_eq!(report.summary.total_rules, 1);
        assert_eq!(report.summary.compliant_rules, 1);
        assert!(report.compliance_score > 0.0);
    }

    #[test]
    fn test_export_json() {
        let report = ComplianceReport::new("Test", vec![ComplianceStandard::SOC2]);
        let json = report.export(ReportFormat::Json);
        assert!(!json.is_empty());
    }

    #[test]
    fn test_export_markdown() {
        let mut report = ComplianceReport::new("Test Report", vec![ComplianceStandard::SOC2]);
        let rule = ComplianceRule::new("TEST-1", "Test Rule", RuleCategory::Security);
        report.add_result(CheckResult::compliant(&rule));
        report.calculate_score();

        let md = report.export(ReportFormat::Markdown);
        let md_str = String::from_utf8(md).unwrap();
        assert!(md_str.contains("# Compliance Report"));
        assert!(md_str.contains("TEST-1"));
    }
}
