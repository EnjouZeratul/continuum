//! 速率限制模块
//!
//! Token Bucket 和滑动窗口算法。
//! 使用 DashMap 实现高性能并发访问。

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// 每秒允许的请求数
    pub requests_per_second: u32,
    /// 每分钟允许的请求数
    pub requests_per_minute: u32,
    /// 每小时允许的请求数
    pub requests_per_hour: u32,
    /// Burst 大小（突发流量）
    pub burst_size: u32,
    /// 分钟窗口容量上限（防止内存无限增长）
    #[serde(default = "default_minute_capacity")]
    pub minute_window_capacity: usize,
    /// 小时窗口容量上限
    #[serde(default = "default_hour_capacity")]
    pub hour_window_capacity: usize,
}

fn default_minute_capacity() -> usize {
    1000
}
fn default_hour_capacity() -> usize {
    10000
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 10,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 20,
            minute_window_capacity: 1000,
            hour_window_capacity: 10000,
        }
    }
}

/// Token Bucket 状态
#[derive(Debug)]
struct TokenBucket {
    /// 当前 token 数
    tokens: f64,
    /// 最大 token 数
    max_tokens: f64,
    /// 每秒补充的 token 数
    refill_rate: f64,
    /// 上次更新时间
    last_update: Instant,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_update: Instant::now(),
        }
    }

    fn try_take(&mut self, tokens: f64) -> bool {
        // 先补充 token
        let elapsed = self.last_update.elapsed().as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_update = Instant::now();

        // 检查是否有足够的 token
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }
}

/// 速率限制器（高性能版本，使用 DashMap）
pub struct RateLimiter {
    /// 配置
    config: RateLimitConfig,
    /// 每个 key 的 token bucket（并发安全）
    buckets: DashMap<String, TokenBucket>,
    /// 每个 key 的请求计数（滑动窗口，并发安全）
    counters: DashMap<String, SlidingWindowCounter>,
}

/// 滑动窗口计数器
#[derive(Debug)]
struct SlidingWindowCounter {
    /// 最近一分钟的请求时间戳
    minute_requests: Vec<Instant>,
    /// 最近一小时的请求时间戳
    hour_requests: Vec<Instant>,
    /// 分钟窗口容量上限
    minute_capacity: usize,
    /// 小时窗口容量上限
    hour_capacity: usize,
}

impl SlidingWindowCounter {
    fn with_capacity(minute_capacity: usize, hour_capacity: usize) -> Self {
        Self {
            minute_requests: Vec::new(),
            hour_requests: Vec::new(),
            minute_capacity,
            hour_capacity,
        }
    }

    fn add_request(&mut self) {
        let now = Instant::now();
        self.minute_requests.push(now);
        self.hour_requests.push(now);

        // 清理过期记录
        self.minute_requests
            .retain(|t| t.elapsed() < Duration::from_secs(60));
        self.hour_requests
            .retain(|t| t.elapsed() < Duration::from_secs(3600));

        // 如果超过容量上限，强制截断（保留最新的）
        if self.minute_requests.len() > self.minute_capacity {
            let excess = self.minute_requests.len() - self.minute_capacity;
            self.minute_requests.drain(0..excess);
        }
        if self.hour_requests.len() > self.hour_capacity {
            let excess = self.hour_requests.len() - self.hour_capacity;
            self.hour_requests.drain(0..excess);
        }
    }

    fn minute_count(&self) -> usize {
        self.minute_requests.len()
    }

    fn hour_count(&self) -> usize {
        self.hour_requests.len()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::with_config(RateLimitConfig::default())
    }

    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            config: config.clone(),
            buckets: DashMap::new(),
            counters: DashMap::new(),
        }
    }

    /// 检查是否允许请求
    pub async fn check(&self, key: &str) -> anyhow::Result<bool> {
        // 检查 Token Bucket（秒级限制）
        let bucket_result = {
            let mut bucket = self.buckets.entry(key.to_string()).or_insert_with(|| {
                TokenBucket::new(
                    self.config.burst_size as f64,
                    self.config.requests_per_second as f64,
                )
            });
            bucket.try_take(1.0)
        };

        if !bucket_result {
            return Ok(false);
        }

        // 检查滑动窗口（分钟和小时限制）
        let window_result = {
            let minute_cap = self.config.minute_window_capacity;
            let hour_cap = self.config.hour_window_capacity;
            let mut counter = self
                .counters
                .entry(key.to_string())
                .or_insert_with(|| SlidingWindowCounter::with_capacity(minute_cap, hour_cap));

            let minute_exceeded =
                counter.minute_count() >= self.config.requests_per_minute as usize;
            let hour_exceeded = counter.hour_count() >= self.config.requests_per_hour as usize;

            if minute_exceeded || hour_exceeded {
                false
            } else {
                counter.add_request();
                true
            }
        };

        Ok(window_result)
    }

    /// 重置指定 key 的限制
    pub fn reset(&self, key: &str) {
        self.buckets.remove(key);
        self.counters.remove(key);
    }

    /// 获取指定 key 的状态
    pub fn get_status(&self, key: &str) -> RateLimitStatus {
        let tokens_remaining = self
            .buckets
            .get(key)
            .map(|b| b.tokens as u32)
            .unwrap_or(self.config.burst_size);

        let minute_remaining = self.config.requests_per_minute
            - self
                .counters
                .get(key)
                .map(|c| c.minute_count() as u32)
                .unwrap_or(0);

        let hour_remaining = self.config.requests_per_hour
            - self
                .counters
                .get(key)
                .map(|c| c.hour_count() as u32)
                .unwrap_or(0);

        RateLimitStatus {
            tokens_remaining,
            minute_remaining,
            hour_remaining,
        }
    }

    /// 清理过期的条目（定期维护）
    pub fn cleanup_expired(&self, max_age: Duration) {
        let now = Instant::now();

        // 清理过期的 buckets
        self.buckets
            .retain(|_, bucket| now.duration_since(bucket.last_update) < max_age);

        // 清理空的 counters
        self.counters.retain(|_, counter| {
            !counter.minute_requests.is_empty() || !counter.hour_requests.is_empty()
        });
    }

    /// 获取当前活跃的 key 数量
    pub fn active_keys(&self) -> usize {
        self.buckets.len()
    }
}

/// 速率限制状态
#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub tokens_remaining: u32,
    pub minute_remaining: u32,
    pub hour_remaining: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_basic_rate_limit() {
        let limiter = RateLimiter::new();

        // 前10次请求应该成功
        for _ in 0..10 {
            assert!(limiter.check("test_key").await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            requests_per_minute: 2,
            requests_per_hour: 3,
            burst_size: 2,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // Burst 应该允许
        assert!(limiter.check("test_key").await.unwrap());
        assert!(limiter.check("test_key").await.unwrap());

        // 超过 burst 应该被限制
        assert!(!limiter.check("test_key").await.unwrap());
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let config = RateLimitConfig {
            requests_per_second: 100,
            requests_per_minute: 1000,
            requests_per_hour: 10000,
            burst_size: 50,
            ..Default::default()
        };
        let limiter = Arc::new(RateLimiter::with_config(config));

        let mut tasks = vec![];

        for _ in 0..100 {
            let limiter_clone = Arc::clone(&limiter);
            tasks.push(tokio::spawn(async move {
                limiter_clone.check("concurrent_key").await.unwrap()
            }));
        }

        let results: Vec<bool> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // 统计成功和失败的请求数
        let success_count = results.iter().filter(|&&r| r).count();
        let fail_count = results.iter().filter(|&&r| !r).count();

        // 由于 burst_size 是 50，应该有一些成功，一些失败
        assert!(success_count > 0, "At least some requests should succeed");
        println!("Success: {}, Fail: {}", success_count, fail_count);
    }

    #[tokio::test]
    async fn test_burst_handling() {
        let config = RateLimitConfig {
            requests_per_second: 5,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 10,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // 连续快速请求应该消耗 burst
        let mut success_count = 0;
        for _ in 0..20 {
            if limiter.check("burst_key").await.unwrap() {
                success_count += 1;
            }
        }

        // burst_size 是 10，应该允许约 10 个请求通过
        assert!(
            success_count <= 11,
            "Burst should be limited, but got {} successes",
            success_count
        );
        assert!(
            success_count >= 8,
            "At least burst_size requests should succeed, but got {}",
            success_count
        );
    }

    #[tokio::test]
    async fn test_token_refill_accuracy() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 5,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // 消耗所有 token
        for _ in 0..5 {
            assert!(limiter.check("refill_key").await.unwrap());
        }

        // 应该被限制
        assert!(!limiter.check("refill_key").await.unwrap());

        // 等待 token 补充 (100ms 应该补充约 1 个 token)
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        // 现在应该至少有一个 token 可用
        assert!(
            limiter.check("refill_key").await.unwrap(),
            "Token should be refilled after waiting"
        );
    }

    #[tokio::test]
    async fn test_different_keys_isolated() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            requests_per_minute: 1,
            requests_per_hour: 1,
            burst_size: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // key1 应该被限制在 burst_size
        assert!(limiter.check("key1").await.unwrap());
        assert!(!limiter.check("key1").await.unwrap());

        // key2 应该独立计数
        assert!(limiter.check("key2").await.unwrap());
        assert!(!limiter.check("key2").await.unwrap());
    }

    #[test]
    fn test_reset_functionality() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            requests_per_minute: 1,
            requests_per_hour: 1,
            burst_size: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // 在同步上下文中测试 reset
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert!(limiter.check("reset_key").await.unwrap());
            assert!(!limiter.check("reset_key").await.unwrap());
        });

        // 重置
        limiter.reset("reset_key");

        rt.block_on(async {
            // 重置后应该可以再次请求
            assert!(limiter.check("reset_key").await.unwrap());
        });
    }

    #[test]
    fn test_status_reporting() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 20,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 消耗一些 token
            for _ in 0..5 {
                limiter.check("status_key").await.unwrap();
            }
        });

        let status = limiter.get_status("status_key");
        assert!(status.tokens_remaining < 20, "Tokens should be consumed");
        assert!(
            status.minute_remaining < 100,
            "Minute count should increase"
        );
    }

    #[test]
    fn test_cleanup_expired() {
        let limiter = RateLimiter::new();

        // 创建一些条目
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            limiter.check("key1").await.unwrap();
            limiter.check("key2").await.unwrap();
        });

        assert!(limiter.active_keys() >= 2);

        // 清理（设置为 0 表示立即过期）
        limiter.cleanup_expired(Duration::from_secs(0));

        // 应该被清理
        assert_eq!(limiter.active_keys(), 0);
    }

    #[test]
    fn test_active_keys_count() {
        let limiter = RateLimiter::new();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            limiter.check("key1").await.unwrap();
            limiter.check("key2").await.unwrap();
            limiter.check("key3").await.unwrap();
        });

        assert_eq!(limiter.active_keys(), 3);
    }

    // ========== 边界条件测试 ==========

    #[test]
    fn test_zero_rate_limit() {
        // 零速率限制 - requests_per_minute/hour 为 0 会阻止所有请求
        // 我们测试 requests_per_second=0 但其他值足够大的情况
        let config = RateLimitConfig {
            requests_per_second: 0,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 2,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // burst_size 为 2，前两次请求应该成功（token bucket 初始有 burst_size 个 token）
            let first = limiter.check("key").await.unwrap();
            assert!(first, "First request with burst_size=2 should succeed");

            let second = limiter.check("key").await.unwrap();
            assert!(second, "Second request with burst_size=2 should succeed");

            // 第三次请求应该失败（没有 refill，因为 requests_per_second=0）
            let third = limiter.check("key").await.unwrap();
            assert!(!third, "Third request should be rate limited (no refill)");
        });
    }

    #[test]
    fn test_very_small_burst_size() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert!(limiter.check("key").await.unwrap());
            assert!(!limiter.check("key").await.unwrap());
        });
    }

    #[test]
    fn test_large_burst_size() {
        let config = RateLimitConfig {
            requests_per_second: 1000,
            requests_per_minute: 100000,
            requests_per_hour: 1000000,
            burst_size: 1000,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut success_count = 0;
            for _ in 0..500 {
                if limiter.check("key").await.unwrap() {
                    success_count += 1;
                }
            }
            assert!(
                success_count >= 400,
                "Should allow most requests with large burst"
            );
        });
    }

    #[test]
    fn test_empty_key() {
        let limiter = RateLimiter::new();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 空键应该被正常处理
            assert!(limiter.check("").await.unwrap());
        });
    }

    #[test]
    fn test_special_characters_in_key() {
        let limiter = RateLimiter::new();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 特殊字符键
            let special_keys = vec![
                "key:with:colons",
                "key-with-dashes",
                "key_with_underscores",
                "key.with.dots",
                "key/with/slashes",
            ];
            for key in special_keys {
                assert!(
                    limiter.check(key).await.unwrap(),
                    "Key '{}' should work",
                    key
                );
            }
        });
    }

    #[test]
    fn test_unicode_key() {
        let limiter = RateLimiter::new();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Unicode 键
            assert!(limiter.check("用户_123").await.unwrap());
            assert!(limiter.check("🔑_key").await.unwrap());
        });
    }

    #[test]
    fn test_very_long_key() {
        let limiter = RateLimiter::new();
        let long_key = "a".repeat(10000);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert!(limiter.check(&long_key).await.unwrap());
        });
    }

    #[test]
    fn test_reset_nonexistent_key() {
        let limiter = RateLimiter::new();

        // 重置不存在的键不应该 panic
        limiter.reset("nonexistent_key");
        assert_eq!(limiter.active_keys(), 0);
    }

    #[test]
    fn test_status_nonexistent_key() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::default();

        let status = limiter.get_status("nonexistent");
        // 对于不存在的键，应该返回配置的最大值
        assert_eq!(status.tokens_remaining, config.burst_size);
        assert_eq!(status.minute_remaining, config.requests_per_minute);
        assert_eq!(status.hour_remaining, config.requests_per_hour);
    }

    #[tokio::test]
    async fn test_rapid_requests() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 5,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // 快速连续请求
        let mut success_count = 0;
        for _ in 0..20 {
            if limiter.check("rapid").await.unwrap() {
                success_count += 1;
            }
        }

        // 应该受到 burst_size 限制
        assert!(
            success_count <= 7,
            "Expected ~5 successful requests, got {}",
            success_count
        );
    }

    #[test]
    fn test_cleanup_with_negative_duration() {
        let limiter = RateLimiter::new();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            limiter.check("key").await.unwrap();
        });

        // 使用非常大的 duration（相当于负数时间）
        // 这不应该 panic
        limiter.cleanup_expired(Duration::from_secs(u64::MAX));

        // 键应该仍然存在
        assert!(limiter.active_keys() >= 1);
    }

    #[tokio::test]
    async fn test_status_accuracy() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            requests_per_minute: 100,
            requests_per_hour: 1000,
            burst_size: 10,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // 消耗 3 个 token
        for _ in 0..3 {
            limiter.check("status_test").await.unwrap();
        }

        let status = limiter.get_status("status_test");
        // tokens_remaining 应该少于初始值
        assert!(status.tokens_remaining < 10);
        // 但不能是负数
        assert!(status.tokens_remaining > 0);
    }

    #[test]
    fn test_config_default_values() {
        let config = RateLimitConfig::default();
        assert_eq!(config.requests_per_second, 10);
        assert_eq!(config.requests_per_minute, 100);
        assert_eq!(config.requests_per_hour, 1000);
        assert_eq!(config.burst_size, 20);
    }

    #[test]
    fn test_config_serialization() {
        let config = RateLimitConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: RateLimitConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.requests_per_second, config.requests_per_second);
    }

    #[tokio::test]
    async fn test_token_refill_boundary() {
        let config = RateLimitConfig {
            requests_per_second: 100, // 100 tokens/sec
            requests_per_minute: 10000,
            requests_per_hour: 100000,
            burst_size: 10,
            ..Default::default()
        };
        let limiter = RateLimiter::with_config(config);

        // 消耗所有 tokens
        for _ in 0..10 {
            limiter.check("refill_boundary").await.unwrap();
        }

        // 应该被限制
        assert!(!limiter.check("refill_boundary").await.unwrap());

        // 等待 10ms，应该补充约 1 个 token
        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;

        // 现在应该有至少一个 token
        assert!(limiter.check("refill_boundary").await.unwrap());
    }
}
