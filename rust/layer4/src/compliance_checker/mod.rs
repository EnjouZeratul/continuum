//! # Compliance Checker
//!
//! 合规检查器：检查系统是否符合合规标准。
//!
//! ## 功能
//!
//! - 多标准支持（SOC2, HIPAA, GDPR, PCI-DSS）
//! - 自动化合规检查
//! - 合规报告生成
//! - 违规检测和警报
//!
//! ## 用法
//!
//! ```rust,ignore
//! use sh_layer4::compliance_checker::{ComplianceChecker, ComplianceStandard, ComplianceConfig};
//!
//! let checker = ComplianceChecker::new(ComplianceConfig {
//!     standards: vec![ComplianceStandard::SOC2, ComplianceStandard::HIPAA],
//!     ..Default::default()
//! });
//!
//! // 运行合规检查
//! let report = checker.run_check().await?;
//!
//! // 生成合规报告
//! let export = checker.generate_report(ReportFormat::JSON).await?;
//! ```

pub mod checker;
pub mod report;
pub mod rules;

pub use checker::{
    CheckContext, ComplianceChecker, ComplianceConfig, ComplianceSummary, QuickCheckResult,
    ResourceInfo, RuleChecker,
};
pub use report::{CheckResult, ComplianceReport, ComplianceStatus, ReportFormat, Violation};
pub use rules::{ComplianceRule, ComplianceStandard, RuleCategory, RuleSeverity};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let _config = ComplianceConfig::default();
        let _standard = ComplianceStandard::SOC2;
    }
}
