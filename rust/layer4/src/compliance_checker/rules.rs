//! # Compliance Rules
//!
//! 定义各种合规标准的检查规则。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 合规标准
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum ComplianceStandard {
    /// SOC 2 (Service Organization Control)
    SOC2,
    /// HIPAA (Health Insurance Portability and Accountability Act)
    HIPAA,
    /// GDPR (General Data Protection Regulation)
    GDPR,
    /// PCI-DSS (Payment Card Industry Data Security Standard)
    PCIDSS,
    /// ISO 27001
    ISO27001,
    /// 自定义标准
    Custom,
}

impl ComplianceStandard {
    /// 获取标准名称
    pub fn name(&self) -> &'static str {
        match self {
            ComplianceStandard::SOC2 => "SOC 2",
            ComplianceStandard::HIPAA => "HIPAA",
            ComplianceStandard::GDPR => "GDPR",
            ComplianceStandard::PCIDSS => "PCI-DSS",
            ComplianceStandard::ISO27001 => "ISO 27001",
            ComplianceStandard::Custom => "Custom",
        }
    }

    /// 获取标准描述
    pub fn description(&self) -> &'static str {
        match self {
            ComplianceStandard::SOC2 => "Service Organization Control 2 - Security, Availability, Processing Integrity, Confidentiality, Privacy",
            ComplianceStandard::HIPAA => "Health Insurance Portability and Accountability Act - Healthcare data protection",
            ComplianceStandard::GDPR => "General Data Protection Regulation - EU data privacy",
            ComplianceStandard::PCIDSS => "Payment Card Industry Data Security Standard - Payment card data protection",
            ComplianceStandard::ISO27001 => "ISO/IEC 27001 - Information security management",
            ComplianceStandard::Custom => "Custom compliance standard",
        }
    }

    /// 获取默认规则
    pub fn default_rules(&self) -> Vec<ComplianceRule> {
        match self {
            ComplianceStandard::SOC2 => vec![
                ComplianceRule::new("SOC2-CC6.1", "Access Control", RuleCategory::Security)
                    .with_description("Logical and physical access controls must be implemented")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("SOC2-CC6.2", "Authentication", RuleCategory::Security)
                    .with_description("Authentication mechanisms must be implemented")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("SOC2-CC6.3", "Authorization", RuleCategory::Security)
                    .with_description("Authorization controls must be implemented")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("SOC2-CC7.1", "Monitoring", RuleCategory::Operations)
                    .with_description("System monitoring must be implemented")
                    .with_severity(RuleSeverity::Medium),
                ComplianceRule::new("SOC2-CC7.2", "Logging", RuleCategory::Operations)
                    .with_description("Audit logging must be implemented and retained")
                    .with_severity(RuleSeverity::High),
            ],
            ComplianceStandard::HIPAA => vec![
                ComplianceRule::new("HIPAA-164.312.a", "Access Control", RuleCategory::Security)
                    .with_description("Implement technical policies and procedures for electronic information systems")
                    .with_severity(RuleSeverity::Critical),
                ComplianceRule::new("HIPAA-164.312.b", "Audit Controls", RuleCategory::Operations)
                    .with_description("Implement hardware, software, and/or procedural mechanisms that record and examine activity")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("HIPAA-164.312.c", "Integrity", RuleCategory::Security)
                    .with_description("Implement policies and procedures to protect electronic health information")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("HIPAA-164.312.d", "Authentication", RuleCategory::Security)
                    .with_description("Implement procedures to verify entity identity")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("HIPAA-164.312.e", "Transmission Security", RuleCategory::Security)
                    .with_description("Implement technical security measures to guard against unauthorized access")
                    .with_severity(RuleSeverity::High),
            ],
            ComplianceStandard::GDPR => vec![
                ComplianceRule::new("GDPR-Art.5", "Data Processing Principles", RuleCategory::Privacy)
                    .with_description("Personal data must be processed lawfully, fairly and transparently")
                    .with_severity(RuleSeverity::Critical),
                ComplianceRule::new("GDPR-Art.6", "Lawfulness of Processing", RuleCategory::Privacy)
                    .with_description("Processing must have a valid legal basis")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("GDPR-Art.17", "Right to Erasure", RuleCategory::Privacy)
                    .with_description("Data subjects have the right to have their data erased")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("GDPR-Art.25", "Data Protection by Design", RuleCategory::Privacy)
                    .with_description("Implement appropriate technical and organizational measures")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("GDPR-Art.32", "Security of Processing", RuleCategory::Security)
                    .with_description("Implement appropriate security measures")
                    .with_severity(RuleSeverity::High),
            ],
            ComplianceStandard::PCIDSS => vec![
                ComplianceRule::new("PCI-1", "Firewall Configuration", RuleCategory::Security)
                    .with_description("Install and maintain a firewall configuration")
                    .with_severity(RuleSeverity::Critical),
                ComplianceRule::new("PCI-2", "Default Passwords", RuleCategory::Security)
                    .with_description("Do not use vendor-supplied defaults for system passwords")
                    .with_severity(RuleSeverity::Critical),
                ComplianceRule::new("PCI-3", "Stored Data Protection", RuleCategory::Security)
                    .with_description("Protect stored cardholder data")
                    .with_severity(RuleSeverity::Critical),
                ComplianceRule::new("PCI-4", "Encryption", RuleCategory::Security)
                    .with_description("Encrypt transmission of cardholder data")
                    .with_severity(RuleSeverity::Critical),
                ComplianceRule::new("PCI-10", "Access Monitoring", RuleCategory::Operations)
                    .with_description("Track and monitor all access to network resources and cardholder data")
                    .with_severity(RuleSeverity::High),
            ],
            ComplianceStandard::ISO27001 => vec![
                ComplianceRule::new("ISO-A.9", "Access Control", RuleCategory::Security)
                    .with_description("Implement access control policies and procedures")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("ISO-A.10", "Cryptography", RuleCategory::Security)
                    .with_description("Implement cryptographic controls")
                    .with_severity(RuleSeverity::High),
                ComplianceRule::new("ISO-A.12", "Operations Security", RuleCategory::Operations)
                    .with_description("Implement operational procedures and responsibilities")
                    .with_severity(RuleSeverity::Medium),
                ComplianceRule::new("ISO-A.16", "Incident Management", RuleCategory::Operations)
                    .with_description("Implement information security incident management")
                    .with_severity(RuleSeverity::High),
            ],
            ComplianceStandard::Custom => vec![],
        }
    }
}

/// 规则类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleCategory {
    /// 安全控制
    Security,
    /// 隐私保护
    Privacy,
    /// 运营流程
    Operations,
    /// 数据保护
    DataProtection,
    /// 访问控制
    AccessControl,
    /// 审计监控
    Audit,
}

/// 规则严重性
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RuleSeverity {
    /// 低
    Low,
    /// 中
    #[default]
    Medium,
    /// 高
    High,
    /// 严重
    Critical,
}

/// 合规规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRule {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 规则类别
    pub category: RuleCategory,
    /// 规则描述
    pub description: Option<String>,
    /// 规则严重性
    pub severity: RuleSeverity,
    /// 检查函数名称
    pub check_function: Option<String>,
    /// 规则参数
    pub parameters: HashMap<String, serde_json::Value>,
    /// 是否启用
    pub enabled: bool,
}

impl ComplianceRule {
    /// 创建新的合规规则
    pub fn new(id: impl Into<String>, name: impl Into<String>, category: RuleCategory) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category,
            description: None,
            severity: RuleSeverity::default(),
            check_function: None,
            parameters: HashMap::new(),
            enabled: true,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置严重性
    pub fn with_severity(mut self, severity: RuleSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// 设置检查函数
    pub fn with_check_function(mut self, function: impl Into<String>) -> Self {
        self.check_function = Some(function.into());
        self
    }

    /// 添加参数
    pub fn with_parameter(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.parameters.insert(key.into(), value);
        self
    }

    /// 禁用规则
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_name() {
        assert_eq!(ComplianceStandard::SOC2.name(), "SOC 2");
        assert_eq!(ComplianceStandard::HIPAA.name(), "HIPAA");
    }

    #[test]
    fn test_standard_default_rules() {
        let rules = ComplianceStandard::SOC2.default_rules();
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.id == "SOC2-CC6.1"));
    }

    #[test]
    fn test_rule_creation() {
        let rule = ComplianceRule::new("TEST-1", "Test Rule", RuleCategory::Security)
            .with_description("Test description")
            .with_severity(RuleSeverity::High);

        assert_eq!(rule.id, "TEST-1");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.category, RuleCategory::Security);
        assert_eq!(rule.severity, RuleSeverity::High);
        assert!(rule.enabled);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(RuleSeverity::Critical > RuleSeverity::High);
        assert!(RuleSeverity::High > RuleSeverity::Medium);
        assert!(RuleSeverity::Medium > RuleSeverity::Low);
    }
}
