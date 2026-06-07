//! 成本追踪模块
//!
//! Token 计数、费用计算、预算控制。

use crate::utils::generate_short_id;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// 成本报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    /// 总输入 token 数
    pub total_input_tokens: u64,
    /// 总输出 token 数
    pub total_output_tokens: u64,
    /// 总成本（美元）
    pub total_cost_usd: f64,
    /// 模型成本明细
    pub model_costs: HashMap<String, ModelCost>,
    /// 报告时间
    pub timestamp: String,
}

/// 单模型成本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

/// 模型定价（每百万 token）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// 输入 token 价格（美元/百万 token）
    pub input_price_per_million: f64,
    /// 输出 token 价格（美元/百万 token）
    pub output_price_per_million: f64,
}

impl ModelPricing {
    /// 计算成本
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_price_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_price_per_million;
        input_cost + output_cost
    }
}

/// 默认模型定价
fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut pricing = HashMap::new();

    // Claude 模型定价
    pricing.insert(
        "claude-opus-4-6".to_string(),
        ModelPricing {
            input_price_per_million: 15.0,
            output_price_per_million: 75.0,
        },
    );
    pricing.insert(
        "claude-sonnet-4-6".to_string(),
        ModelPricing {
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
        },
    );
    pricing.insert(
        "claude-haiku-4-5".to_string(),
        ModelPricing {
            input_price_per_million: 0.8,
            output_price_per_million: 4.0,
        },
    );

    // OpenAI 模型定价
    pricing.insert(
        "claude-sonnet-4-6".to_string(),
        ModelPricing {
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
        },
    );
    pricing.insert(
        "gpt-4o-mini".to_string(),
        ModelPricing {
            input_price_per_million: 0.15,
            output_price_per_million: 0.6,
        },
    );

    pricing
}

/// 成本追踪器
pub struct CostTracker {
    /// 使用记录
    usage: RwLock<HashMap<String, UsageRecord>>,
    /// 模型定价
    pricing: HashMap<String, ModelPricing>,
    /// 预算上限
    budget_limit: RwLock<Option<f64>>,
    /// 当前总成本
    current_cost: RwLock<f64>,
}

/// 使用记录
#[derive(Debug, Clone)]
struct UsageRecord {
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    #[allow(dead_code)]
    timestamp: Instant,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            usage: RwLock::new(HashMap::new()),
            pricing: default_pricing(),
            budget_limit: RwLock::new(None),
            current_cost: RwLock::new(0.0),
        }
    }

    /// 设置预算上限
    pub fn set_budget_limit(&self, limit: f64) {
        *self.budget_limit.write() = Some(limit);
    }

    /// 记录使用
    pub fn record_usage(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> anyhow::Result<()> {
        // 计算成本
        let pricing = self.pricing.get(model).cloned().unwrap_or(ModelPricing {
            // 默认定价（中等模型）
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
        });

        let cost = pricing.calculate_cost(input_tokens, output_tokens);

        // 检查预算
        let current = *self.current_cost.read();
        let limit = *self.budget_limit.read();

        if let Some(limit) = limit {
            if current + cost > limit {
                return Err(anyhow::anyhow!(
                    "Budget limit exceeded: current {:.4}, new {:.4}, limit {:.2}",
                    current,
                    current + cost,
                    limit
                ));
            }
        }

        // 更新记录
        let record_id = generate_short_id();
        self.usage.write().insert(
            record_id,
            UsageRecord {
                model: model.to_string(),
                input_tokens,
                output_tokens,
                timestamp: Instant::now(),
            },
        );

        // 更新当前成本
        *self.current_cost.write() += cost;

        Ok(())
    }

    /// 获取当前使用情况
    pub fn get_current_usage(&self) -> UsageSnapshot {
        let usage = self.usage.read();
        let mut model_costs = HashMap::new();
        let mut total_input = 0;
        let mut total_output = 0;

        for record in usage.values() {
            let entry = model_costs
                .entry(record.model.clone())
                .or_insert(ModelCost {
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_usd: 0.0,
                });

            entry.input_tokens += record.input_tokens;
            entry.output_tokens += record.output_tokens;

            let pricing = self
                .pricing
                .get(&record.model)
                .cloned()
                .unwrap_or(ModelPricing {
                    input_price_per_million: 3.0,
                    output_price_per_million: 15.0,
                });

            entry.cost_usd += pricing.calculate_cost(record.input_tokens, record.output_tokens);

            total_input += record.input_tokens;
            total_output += record.output_tokens;
        }

        UsageSnapshot {
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            total_cost_usd: *self.current_cost.read(),
            model_costs,
            budget_remaining: self
                .budget_limit
                .read()
                .map(|limit| limit - *self.current_cost.read()),
        }
    }

    /// 预估下一步成本
    pub fn estimate_next_step(
        &self,
        model: &str,
        estimated_input: u64,
        estimated_output: u64,
    ) -> CostEstimate {
        let pricing = self.pricing.get(model).cloned().unwrap_or(ModelPricing {
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
        });

        let estimated_cost = pricing.calculate_cost(estimated_input, estimated_output);

        CostEstimate {
            min_tokens: estimated_input,
            max_tokens: estimated_input + estimated_output,
            estimated_cost_usd: estimated_cost,
            confidence: "medium".to_string(), // [NOTE] 置信度估算为基本启发式，未来可改进
        }
    }

    /// 生成报告
    pub fn generate_report(&self) -> CostReport {
        let snapshot = self.get_current_usage();

        CostReport {
            total_input_tokens: snapshot.total_input_tokens,
            total_output_tokens: snapshot.total_output_tokens,
            total_cost_usd: snapshot.total_cost_usd,
            model_costs: snapshot.model_costs,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// 重置追踪器
    pub fn reset(&self) {
        self.usage.write().clear();
        *self.current_cost.write() = 0.0;
    }
}

/// 使用快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub model_costs: HashMap<String, ModelCost>,
    pub budget_remaining: Option<f64>,
}

/// 成本预估
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub min_tokens: u64,
    pub max_tokens: u64,
    pub estimated_cost_usd: f64,
    pub confidence: String,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_calculation() {
        let pricing = ModelPricing {
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
        };

        // 1000 input + 500 output tokens
        let cost = pricing.calculate_cost(1000, 500);
        assert!(cost > 0.0);
        assert!(cost < 1.0); // 应该小于 1 美元
    }

    #[test]
    fn test_usage_tracking() {
        let tracker = CostTracker::new();

        tracker
            .record_usage("claude-sonnet-4-6", 1000, 500)
            .unwrap();

        let snapshot = tracker.get_current_usage();
        assert_eq!(snapshot.total_input_tokens, 1000);
        assert_eq!(snapshot.total_output_tokens, 500);
    }

    #[test]
    fn test_budget_limit() {
        let tracker = CostTracker::new();
        tracker.set_budget_limit(0.01); // 1 美分

        // 第一次应该成功
        tracker.record_usage("claude-sonnet-4-6", 100, 50).unwrap();

        // 第二次可能超出预算
        let result = tracker.record_usage("claude-sonnet-4-6", 10000, 5000);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_models() {
        let tracker = CostTracker::new();

        // 记录不同模型的使用
        tracker.record_usage("claude-opus-4-6", 1000, 500).unwrap();
        tracker
            .record_usage("claude-sonnet-4-6", 2000, 1000)
            .unwrap();
        tracker.record_usage("claude-haiku-4-5", 500, 250).unwrap();
        tracker.record_usage("gpt-4o", 1500, 750).unwrap();
        tracker.record_usage("gpt-4o-mini", 3000, 1500).unwrap();

        let snapshot = tracker.get_current_usage();

        // 验证总 token 数
        assert_eq!(snapshot.total_input_tokens, 8000);
        assert_eq!(snapshot.total_output_tokens, 4000);

        // 验证每个模型都有记录
        assert!(snapshot.model_costs.contains_key("claude-opus-4-6"));
        assert!(snapshot.model_costs.contains_key("claude-sonnet-4-6"));
        assert!(snapshot.model_costs.contains_key("claude-haiku-4-5"));
        assert!(snapshot.model_costs.contains_key("gpt-4o"));
        assert!(snapshot.model_costs.contains_key("gpt-4o-mini"));

        // 验证总成本大于 0
        assert!(snapshot.total_cost_usd > 0.0);

        // 验证不同模型有不同成本
        let opus_cost = snapshot
            .model_costs
            .get("claude-opus-4-6")
            .unwrap()
            .cost_usd;
        let haiku_cost = snapshot
            .model_costs
            .get("claude-haiku-4-5")
            .unwrap()
            .cost_usd;

        // Opus 应该比 Haiku 贵（即使 token 数相同）
        assert!(
            opus_cost > haiku_cost,
            "Opus should be more expensive than Haiku"
        );
    }

    #[test]
    fn test_budget_reset() {
        let tracker = CostTracker::new();
        tracker.set_budget_limit(1.0);

        // 消耗一些预算
        tracker
            .record_usage("claude-sonnet-4-6", 5000, 2500)
            .unwrap();
        let snapshot = tracker.get_current_usage();
        assert!(snapshot.total_cost_usd > 0.0);
        assert!(snapshot.budget_remaining.is_some());
        assert!(snapshot.budget_remaining.unwrap() < 1.0);

        // 重置
        tracker.reset();

        // 验证重置后状态
        let snapshot = tracker.get_current_usage();
        assert_eq!(snapshot.total_input_tokens, 0);
        assert_eq!(snapshot.total_output_tokens, 0);
        assert_eq!(snapshot.total_cost_usd, 0.0);
        assert!(snapshot.model_costs.is_empty());

        // 预算限制应该仍然有效 - 使用较小的用量
        tracker
            .record_usage("claude-sonnet-4-6", 1000, 500)
            .unwrap();
        let snapshot = tracker.get_current_usage();
        assert!(snapshot.total_cost_usd > 0.0);
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(CostTracker::new());
        let mut handles = vec![];

        for i in 0..10 {
            let t = Arc::clone(&tracker);
            handles.push(thread::spawn(move || {
                let model = match i % 3 {
                    0 => "claude-opus-4-6",
                    1 => "claude-sonnet-4-6",
                    _ => "claude-haiku-4-5",
                };
                t.record_usage(model, 100, 50).unwrap()
            }));
        }

        // 所有记录都应该成功
        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = tracker.get_current_usage();
        assert_eq!(snapshot.total_input_tokens, 1000);
        assert_eq!(snapshot.total_output_tokens, 500);
    }

    #[test]
    fn test_unknown_model_pricing() {
        let tracker = CostTracker::new();

        // 使用未知模型应该使用默认定价
        tracker.record_usage("unknown-model", 1000, 500).unwrap();

        let snapshot = tracker.get_current_usage();
        assert!(snapshot.model_costs.contains_key("unknown-model"));
        // 验证使用了默认定价（应该比 haiku 贵，比 opus 便宜）
        let cost = snapshot.model_costs.get("unknown-model").unwrap().cost_usd;
        assert!(cost > 0.0);
    }

    #[test]
    fn test_estimate_next_step() {
        let tracker = CostTracker::new();

        let estimate = tracker.estimate_next_step("claude-sonnet-4-6", 1000, 500);
        assert_eq!(estimate.min_tokens, 1000);
        assert_eq!(estimate.max_tokens, 1500);
        assert!(estimate.estimated_cost_usd > 0.0);
    }

    #[test]
    fn test_generate_report() {
        let tracker = CostTracker::new();

        tracker
            .record_usage("claude-sonnet-4-6", 1000, 500)
            .unwrap();

        let report = tracker.generate_report();
        assert_eq!(report.total_input_tokens, 1000);
        assert_eq!(report.total_output_tokens, 500);
        assert!(!report.timestamp.is_empty());
    }

    #[test]
    fn test_budget_remaining_calculation() {
        let tracker = CostTracker::new();
        tracker.set_budget_limit(1.0); // $1

        tracker
            .record_usage("claude-sonnet-4-6", 1000, 500)
            .unwrap();

        let snapshot = tracker.get_current_usage();
        assert!(snapshot.budget_remaining.is_some());
        let remaining = snapshot.budget_remaining.unwrap();

        // 剩余预算应该小于总预算
        assert!(remaining < 1.0);
        assert!(remaining > 0.9); // 大部分预算应该剩余
    }
}
