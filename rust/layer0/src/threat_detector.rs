//! # Threat Detector
//!
//! 威胁检测器：检测和响应潜在安全威胁。
//!
//! ## 功能
//! - 异常行为检测
//! - 攻击模式识别
//! - 威胁评分
//! - 自动响应

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;

/// 威胁检测错误
#[derive(Debug, Error)]
pub enum ThreatError {
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),

    #[error("Detection failed: {0}")]
    DetectionFailed(String),

    #[error("Rule not found: {0}")]
    RuleNotFound(String),
}

/// 威胁级别
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ThreatLevel {
    /// 信息级别 - 无威胁
    #[default]
    Info,
    /// 低危 - 轻微异常
    Low,
    /// 中危 - 需要关注
    Medium,
    /// 高危 - 需要立即处理
    High,
    /// 严重 - 紧急响应
    Critical,
}

impl ThreatLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// Parse from string (non-standard name to avoid confusion with FromStr trait)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(Self::Info),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// 威胁类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreatType {
    /// 暴力破解
    BruteForce,
    /// DDoS 攻击
    DDoS,
    /// SQL 注入
    SqlInjection,
    /// XSS 攻击
    #[allow(clippy::upper_case_acronyms)]
    XSS,
    /// 路径遍历
    PathTraversal,
    /// 权限提升
    PrivilegeEscalation,
    /// 数据泄露
    DataExfiltration,
    /// 异常访问
    AnomalousAccess,
    /// 速率异常
    RateAnomaly,
    /// 自定义威胁
    Custom,
}

impl ThreatType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BruteForce => "brute_force",
            Self::DDoS => "ddos",
            Self::SqlInjection => "sql_injection",
            Self::XSS => "xss",
            Self::PathTraversal => "path_traversal",
            Self::PrivilegeEscalation => "privilege_escalation",
            Self::DataExfiltration => "data_exfiltration",
            Self::AnomalousAccess => "anomalous_access",
            Self::RateAnomaly => "rate_anomaly",
            Self::Custom => "custom",
        }
    }
}

/// 检测到的威胁
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threat {
    /// 威胁 ID
    pub id: String,
    /// 威胁类型
    pub threat_type: ThreatType,
    /// 威胁级别
    pub level: ThreatLevel,
    /// 描述
    pub description: String,
    /// 来源 IP 或标识
    pub source: String,
    /// 目标资源
    pub target: Option<String>,
    /// 检测时间
    pub detected_at: String,
    /// 证据数据
    pub evidence: serde_json::Value,
    /// 置信度 (0.0-1.0)
    pub confidence: f32,
    /// 是否已处理
    pub handled: bool,
}

impl Threat {
    pub fn new(
        threat_type: ThreatType,
        level: ThreatLevel,
        source: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("THR-{}", uuid::Uuid::new_v4()),
            threat_type,
            level,
            description: description.into(),
            source: source.into(),
            target: None,
            detected_at: chrono::Utc::now().to_rfc3339(),
            evidence: serde_json::json!({}),
            confidence: 0.5,
            handled: false,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn with_evidence(mut self, evidence: serde_json::Value) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn mark_handled(&mut self) {
        self.handled = true;
    }
}

/// 检测规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 威胁类型
    pub threat_type: ThreatType,
    /// 基础威胁级别
    pub base_level: ThreatLevel,
    /// 触发阈值
    pub threshold: f32,
    /// 时间窗口（秒）
    pub time_window_secs: u64,
    /// 是否启用
    pub enabled: bool,
    /// 描述
    pub description: String,
}

impl DetectionRule {
    pub fn new(name: impl Into<String>, threat_type: ThreatType, base_level: ThreatLevel) -> Self {
        Self {
            id: format!("RULE-{}", uuid::Uuid::new_v4()),
            name: name.into(),
            threat_type,
            base_level,
            threshold: 0.5,
            time_window_secs: 300,
            enabled: true,
            description: String::new(),
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn with_time_window(mut self, secs: u64) -> Self {
        self.time_window_secs = secs;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

/// 响应动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseAction {
    /// 仅记录
    Log,
    /// 发送告警
    Alert,
    /// 临时封禁
    TempBan { duration_secs: u64 },
    /// 永久封禁
    PermanentBan,
    /// 限流
    RateLimit { requests_per_sec: u32 },
    /// 自定义动作
    Custom { action: String },
}

/// 响应规则
#[derive(Debug, Clone)]
pub struct ResponseRule {
    /// 触发的威胁级别
    pub min_level: ThreatLevel,
    /// 响应动作
    pub action: ResponseAction,
    /// 是否启用
    pub enabled: bool,
}

impl ResponseRule {
    pub fn new(min_level: ThreatLevel, action: ResponseAction) -> Self {
        Self {
            min_level,
            action,
            enabled: true,
        }
    }
}

/// 威胁统计
#[derive(Debug, Clone, Default)]
pub struct ThreatStats {
    /// 总检测次数
    pub total_detections: u64,
    /// 各级别威胁数量
    pub by_level: HashMap<ThreatLevel, u64>,
    /// 各类型威胁数量
    pub by_type: HashMap<ThreatType, u64>,
    /// 活跃威胁数
    pub active_threats: u64,
    /// 已处理威胁数
    pub handled_threats: u64,
}

/// 活动记录
#[derive(Debug, Clone)]
struct ActivityRecord {
    /// 活动类型
    activity_type: String,
    /// 来源
    source: String,
    /// 时间戳
    timestamp: Instant,
    /// 相关数据
    data: serde_json::Value,
}

/// 威胁检测器配置
#[derive(Debug, Clone)]
pub struct ThreatDetectorConfig {
    /// 是否启用检测
    pub enabled: bool,
    /// 检测间隔（秒）
    pub detection_interval_secs: u64,
    /// 历史记录保留时间（秒）
    pub history_retention_secs: u64,
    /// 自动响应
    pub auto_response: bool,
    /// 告警阈值
    pub alert_threshold: ThreatLevel,
}

impl Default for ThreatDetectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            detection_interval_secs: 60,
            history_retention_secs: 3600 * 24,
            auto_response: true,
            alert_threshold: ThreatLevel::High,
        }
    }
}

/// 威胁检测器
pub struct ThreatDetector {
    /// 配置
    config: ThreatDetectorConfig,
    /// 检测规则
    rules: RwLock<Vec<DetectionRule>>,
    /// 响应规则
    response_rules: RwLock<Vec<ResponseRule>>,
    /// 活动历史
    activity_history: RwLock<Vec<ActivityRecord>>,
    /// 检测到的威胁
    threats: RwLock<Vec<Threat>>,
    /// 统计信息
    stats: RwLock<ThreatStats>,
    /// 封禁列表
    ban_list: RwLock<HashMap<String, Instant>>,
}

impl ThreatDetector {
    /// 创建新的威胁检测器
    pub fn new() -> Self {
        Self::with_config(ThreatDetectorConfig::default())
    }

    /// 使用自定义配置创建
    pub fn with_config(config: ThreatDetectorConfig) -> Self {
        let detector = Self {
            config,
            rules: RwLock::new(Vec::new()),
            response_rules: RwLock::new(Vec::new()),
            activity_history: RwLock::new(Vec::new()),
            threats: RwLock::new(Vec::new()),
            stats: RwLock::new(ThreatStats::default()),
            ban_list: RwLock::new(HashMap::new()),
        };

        // 添加默认规则
        detector.add_default_rules();
        detector.add_default_response_rules();
        detector
    }

    fn add_default_rules(&self) {
        let default_rules = vec![
            DetectionRule::new(
                "Brute Force Detection",
                ThreatType::BruteForce,
                ThreatLevel::High,
            )
            .with_threshold(0.3)
            .with_time_window(300)
            .with_description("检测短时间内多次失败登录尝试"),
            DetectionRule::new(
                "Rate Anomaly Detection",
                ThreatType::RateAnomaly,
                ThreatLevel::Medium,
            )
            .with_threshold(0.5)
            .with_time_window(60)
            .with_description("检测异常请求速率"),
            DetectionRule::new(
                "SQL Injection Detection",
                ThreatType::SqlInjection,
                ThreatLevel::Critical,
            )
            .with_threshold(0.8)
            .with_time_window(1)
            .with_description("检测 SQL 注入模式"),
            DetectionRule::new(
                "Path Traversal Detection",
                ThreatType::PathTraversal,
                ThreatLevel::High,
            )
            .with_threshold(0.7)
            .with_time_window(1)
            .with_description("检测路径遍历攻击"),
            DetectionRule::new("XSS Detection", ThreatType::XSS, ThreatLevel::High)
                .with_threshold(0.7)
                .with_time_window(1)
                .with_description("检测跨站脚本攻击"),
        ];

        let mut rules = self.rules.write();
        for rule in default_rules {
            rules.push(rule);
        }
    }

    fn add_default_response_rules(&self) {
        let default_responses = vec![
            ResponseRule::new(ThreatLevel::Critical, ResponseAction::PermanentBan),
            ResponseRule::new(
                ThreatLevel::High,
                ResponseAction::TempBan {
                    duration_secs: 3600,
                },
            ),
            ResponseRule::new(
                ThreatLevel::Medium,
                ResponseAction::RateLimit {
                    requests_per_sec: 10,
                },
            ),
            ResponseRule::new(ThreatLevel::Low, ResponseAction::Alert),
            ResponseRule::new(ThreatLevel::Info, ResponseAction::Log),
        ];

        let mut response_rules = self.response_rules.write();
        for rule in default_responses {
            response_rules.push(rule);
        }
    }

    /// 记录活动
    pub fn record_activity(
        &self,
        activity_type: impl Into<String>,
        source: impl Into<String>,
        data: serde_json::Value,
    ) {
        let record = ActivityRecord {
            activity_type: activity_type.into(),
            source: source.into(),
            timestamp: Instant::now(),
            data,
        };

        self.activity_history.write().push(record);
        self.cleanup_old_activities();
    }

    /// 检测威胁
    pub fn detect(&self) -> Vec<Threat> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut detected_threats = Vec::new();
        let rules = self.rules.read();
        let activity = self.activity_history.read();

        for rule in rules.iter().filter(|r| r.enabled) {
            let threats = self.detect_with_rule(rule, &activity);
            detected_threats.extend(threats);
        }

        // 更新统计
        self.update_stats(&detected_threats);

        // 存储威胁
        let mut threats = self.threats.write();
        for threat in &detected_threats {
            threats.push(threat.clone());
        }

        detected_threats
    }

    fn detect_with_rule(&self, rule: &DetectionRule, activity: &[ActivityRecord]) -> Vec<Threat> {
        let window = Duration::from_secs(rule.time_window_secs);

        let relevant_activities: Vec<_> = activity
            .iter()
            .filter(|a| {
                a.timestamp.elapsed() < window && self.activity_matches_rule(&a.activity_type, rule)
            })
            .collect();

        if relevant_activities.is_empty() {
            return Vec::new();
        }

        // 计算威胁分数
        let score = self.calculate_threat_score(rule, &relevant_activities);

        if score >= rule.threshold {
            // 按来源分组
            let mut by_source: HashMap<String, Vec<&ActivityRecord>> = HashMap::new();
            for act in relevant_activities {
                by_source.entry(act.source.clone()).or_default().push(act);
            }

            by_source
                .into_iter()
                .map(|(source, activities)| {
                    let level = self.calculate_level(rule.base_level, score);
                    Threat::new(rule.threat_type, level, source, &rule.description)
                        .with_confidence(score)
                        .with_evidence(serde_json::json!({
                            "rule_id": rule.id,
                            "rule_name": rule.name,
                            "activity_count": activities.len(),
                            "score": score,
                        }))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn activity_matches_rule(&self, activity_type: &str, rule: &DetectionRule) -> bool {
        match rule.threat_type {
            ThreatType::BruteForce => {
                activity_type.contains("login") || activity_type.contains("auth")
            }
            ThreatType::RateAnomaly => activity_type.contains("request"),
            ThreatType::SqlInjection => {
                activity_type.contains("query") || activity_type.contains("sql")
            }
            ThreatType::XSS => activity_type.contains("input") || activity_type.contains("html"),
            ThreatType::PathTraversal => {
                activity_type.contains("file") || activity_type.contains("path")
            }
            ThreatType::DDoS => {
                activity_type.contains("request") || activity_type.contains("connection")
            }
            ThreatType::PrivilegeEscalation => {
                activity_type.contains("permission") || activity_type.contains("admin")
            }
            ThreatType::DataExfiltration => {
                activity_type.contains("download") || activity_type.contains("export")
            }
            ThreatType::AnomalousAccess => activity_type.contains("access"),
            ThreatType::Custom => true,
        }
    }

    fn calculate_threat_score(&self, rule: &DetectionRule, activities: &[&ActivityRecord]) -> f32 {
        if activities.is_empty() {
            return 0.0;
        }

        let count = activities.len() as f32;
        let window = rule.time_window_secs as f32;
        let rate = count / window.max(1.0);

        // 基于活动频率计算分数
        match rule.threat_type {
            ThreatType::BruteForce => (rate * 60.0).min(1.0),
            ThreatType::RateAnomaly => (rate / 10.0).min(1.0),
            ThreatType::SqlInjection | ThreatType::XSS | ThreatType::PathTraversal => {
                // 检测恶意模式
                activities.iter().any(|a| {
                    let data = a.data.to_string().to_lowercase();
                    data.contains("select") || data.contains("script") || data.contains("../")
                }) as usize as f32
            }
            _ => (count / 5.0).min(1.0),
        }
    }

    fn calculate_level(&self, base_level: ThreatLevel, score: f32) -> ThreatLevel {
        if score >= 0.9 {
            ThreatLevel::Critical
        } else if score >= 0.7 {
            ThreatLevel::High
        } else if score >= 0.5 {
            base_level
        } else if score >= 0.3 {
            ThreatLevel::Low
        } else {
            ThreatLevel::Info
        }
    }

    fn update_stats(&self, new_threats: &[Threat]) {
        let mut stats = self.stats.write();
        stats.total_detections += new_threats.len() as u64;

        for threat in new_threats {
            *stats.by_level.entry(threat.level).or_default() += 1;
            *stats.by_type.entry(threat.threat_type).or_default() += 1;
            if threat.handled {
                stats.handled_threats += 1;
            } else {
                stats.active_threats += 1;
            }
        }
    }

    /// 响应威胁
    pub fn respond(&self, threat: &Threat) -> Option<ResponseAction> {
        if !self.config.auto_response {
            return None;
        }

        let response_rules = self.response_rules.read();
        response_rules
            .iter()
            .filter(|r| r.enabled && threat.level >= r.min_level)
            .max_by_key(|r| r.min_level as i32)
            .map(|r| r.action.clone())
    }

    /// 添加检测规则
    pub fn add_rule(&self, rule: DetectionRule) {
        self.rules.write().push(rule);
    }

    /// 添加响应规则
    pub fn add_response_rule(&self, rule: ResponseRule) {
        self.response_rules.write().push(rule);
    }

    /// 获取威胁列表
    pub fn get_threats(&self) -> Vec<Threat> {
        self.threats.read().clone()
    }

    /// 获取活跃威胁
    pub fn get_active_threats(&self) -> Vec<Threat> {
        self.threats
            .read()
            .iter()
            .filter(|t| !t.handled)
            .cloned()
            .collect()
    }

    /// 标记威胁已处理
    pub fn handle_threat(&self, threat_id: &str) -> Result<(), ThreatError> {
        let mut threats = self.threats.write();
        if let Some(threat) = threats.iter_mut().find(|t| t.id == threat_id) {
            threat.mark_handled();
            let mut stats = self.stats.write();
            stats.active_threats = stats.active_threats.saturating_sub(1);
            stats.handled_threats += 1;
            Ok(())
        } else {
            Err(ThreatError::RuleNotFound(threat_id.to_string()))
        }
    }

    /// 检查是否被封禁
    pub fn is_banned(&self, source: &str) -> bool {
        let ban_list = self.ban_list.read();
        if let Some(&ban_time) = ban_list.get(source) {
            if ban_time.elapsed() < Duration::from_secs(3600) {
                return true;
            }
        }
        false
    }

    /// 封禁来源
    pub fn ban(&self, source: &str, duration_secs: u64) {
        let expiry = Instant::now() + Duration::from_secs(duration_secs);
        self.ban_list.write().insert(source.to_string(), expiry);
    }

    /// 解封来源
    pub fn unban(&self, source: &str) {
        self.ban_list.write().remove(source);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> ThreatStats {
        self.stats.read().clone()
    }

    /// 清理旧活动记录
    fn cleanup_old_activities(&self) {
        let retention = Duration::from_secs(self.config.history_retention_secs);
        self.activity_history
            .write()
            .retain(|a| a.timestamp.elapsed() < retention);
    }

    /// 重置统计
    pub fn reset_stats(&self) {
        *self.stats.write() = ThreatStats::default();
    }

    /// 清空威胁记录
    pub fn clear_threats(&self) {
        self.threats.write().clear();
        self.stats.write().active_threats = 0;
    }
}

impl Default for ThreatDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_creation() {
        let threat = Threat::new(
            ThreatType::BruteForce,
            ThreatLevel::High,
            "192.168.1.1",
            "Multiple failed login attempts",
        );

        assert_eq!(threat.threat_type, ThreatType::BruteForce);
        assert_eq!(threat.level, ThreatLevel::High);
        assert!(!threat.handled);
    }

#[test]
    fn test_threat_level_conversion() {
        assert_eq!(ThreatLevel::parse("high"), Some(ThreatLevel::High));
        assert_eq!(
            ThreatLevel::parse("critical"),
            Some(ThreatLevel::Critical)
        );
        assert_eq!(ThreatLevel::parse("invalid"), None);
    }

    #[test]
    fn test_detection_rule() {
        let rule = DetectionRule::new("Test Rule", ThreatType::BruteForce, ThreatLevel::Medium)
            .with_threshold(0.8)
            .with_time_window(60);

        assert_eq!(rule.threshold, 0.8);
        assert_eq!(rule.time_window_secs, 60);
        assert!(rule.enabled);
    }

    #[test]
    fn test_threat_detector_creation() {
        let detector = ThreatDetector::new();
        assert!(detector.config.enabled);
        assert!(!detector.rules.read().is_empty());
    }

    #[test]
    fn test_record_activity() {
        let detector = ThreatDetector::new();
        detector.record_activity(
            "login_attempt",
            "192.168.1.1",
            serde_json::json!({"success": false}),
        );

        let history = detector.activity_history.read();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_detect_no_threats() {
        let detector = ThreatDetector::new();
        let threats = detector.detect();
        // 无活动，无威胁
        assert!(threats.is_empty());
    }

    #[test]
    fn test_detect_brute_force() {
        let detector = ThreatDetector::new();

        // 模拟多次失败登录
        for _ in 0..15 {
            detector.record_activity(
                "login_attempt",
                "192.168.1.100",
                serde_json::json!({"success": false}),
            );
        }

        let threats = detector.detect();
        assert!(!threats.is_empty());

        let bf_threats: Vec<_> = threats
            .iter()
            .filter(|t| t.threat_type == ThreatType::BruteForce)
            .collect();
        assert!(!bf_threats.is_empty());
    }

    #[test]
    fn test_respond_to_threat() {
        let detector = ThreatDetector::new();

        let critical_threat = Threat::new(
            ThreatType::SqlInjection,
            ThreatLevel::Critical,
            "192.168.1.1",
            "SQL injection detected",
        );

        let response = detector.respond(&critical_threat);
        assert!(matches!(response, Some(ResponseAction::PermanentBan)));

        let low_threat = Threat::new(
            ThreatType::RateAnomaly,
            ThreatLevel::Low,
            "192.168.1.2",
            "Slightly elevated rate",
        );

        let response = detector.respond(&low_threat);
        assert!(matches!(response, Some(ResponseAction::Alert)));
    }

    #[test]
    fn test_ban_functionality() {
        let detector = ThreatDetector::new();

        detector.ban("192.168.1.1", 3600);
        assert!(detector.is_banned("192.168.1.1"));

        detector.unban("192.168.1.1");
        assert!(!detector.is_banned("192.168.1.1"));
    }

    #[test]
    fn test_handle_threat() {
        let detector = ThreatDetector::new();

        // 创建并添加威胁
        let mut threat = Threat::new(
            ThreatType::BruteForce,
            ThreatLevel::High,
            "192.168.1.1",
            "Test threat",
        );
        threat.id = "THR-TEST-1".to_string();
        detector.threats.write().push(threat);

        let result = detector.handle_threat("THR-TEST-1");
        assert!(result.is_ok());

        let threats = detector.threats.read();
        assert!(
            threats
                .iter()
                .find(|t| t.id == "THR-TEST-1")
                .unwrap()
                .handled
        );
    }

    #[test]
    fn test_get_stats() {
        let detector = ThreatDetector::new();

        detector.record_activity("login", "192.168.1.1", serde_json::json!({}));
        detector.record_activity("login", "192.168.1.1", serde_json::json!({}));

        let _threats = detector.detect();
        let _stats = detector.get_stats();
    }

    #[test]
    fn test_add_custom_rule() {
        let detector = ThreatDetector::new();
        let initial_count = detector.rules.read().len();

        let custom_rule =
            DetectionRule::new("Custom Rule", ThreatType::Custom, ThreatLevel::Medium);
        detector.add_rule(custom_rule);

        assert_eq!(detector.rules.read().len(), initial_count + 1);
    }

    #[test]
    fn test_clear_threats() {
        let detector = ThreatDetector::new();

        // 添加一些威胁
        for _ in 0..5 {
            detector.record_activity("login", "192.168.1.1", serde_json::json!({}));
        }
        detector.detect();

        detector.clear_threats();
        assert!(detector.threats.read().is_empty());
    }

    #[test]
    fn test_disabled_detector() {
        let config = ThreatDetectorConfig {
            enabled: false,
            ..Default::default()
        };
        let detector = ThreatDetector::with_config(config);

        detector.record_activity("login", "192.168.1.1", serde_json::json!({}));
        let threats = detector.detect();

        // 禁用检测器不应检测到威胁
        assert!(threats.is_empty());
    }

    #[test]
    fn test_auto_response_disabled() {
        let config = ThreatDetectorConfig {
            auto_response: false,
            ..Default::default()
        };
        let detector = ThreatDetector::with_config(config);

        let threat = Threat::new(
            ThreatType::BruteForce,
            ThreatLevel::High,
            "192.168.1.1",
            "Test",
        );
        let response = detector.respond(&threat);

        assert!(response.is_none());
    }

    #[test]
    fn test_threat_with_target() {
        let threat = Threat::new(
            ThreatType::SqlInjection,
            ThreatLevel::High,
            "attacker",
            "Attack",
        )
        .with_target("users_table");

        assert_eq!(threat.target, Some("users_table".to_string()));
    }

    #[test]
    fn test_threat_confidence_clamping() {
        let threat = Threat::new(
            ThreatType::BruteForce,
            ThreatLevel::Medium,
            "source",
            "desc",
        )
        .with_confidence(1.5);
        assert_eq!(threat.confidence, 1.0);

        let threat = Threat::new(
            ThreatType::BruteForce,
            ThreatLevel::Medium,
            "source",
            "desc",
        )
        .with_confidence(-0.5);
        assert_eq!(threat.confidence, 0.0);
    }

    #[test]
    fn test_multiple_sources() {
        let detector = ThreatDetector::new();

        // 不同来源的活动
        detector.record_activity("login", "192.168.1.1", serde_json::json!({}));
        detector.record_activity("login", "192.168.1.2", serde_json::json!({}));
        detector.record_activity("login", "192.168.1.3", serde_json::json!({}));

        let threats = detector.detect();
        // 可能不会触发威胁（取决于阈值）
        assert!(threats.len() <= 3);
    }
}
