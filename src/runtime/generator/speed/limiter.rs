//! 动态速率限制器
//!
//! 基于令牌桶算法实现动态速率限制，支持速率随时间变化

use super::controller::DynamicSpeedController;
use super::profile::SpeedProfile;
use std::time::{Duration, Instant};

/// 动态速率限制器
///
/// 结合 DynamicSpeedController 和令牌桶算法，实现随时间变化的速率限制。
///
/// # 工作原理
/// 1. 定期更新目标速率（每 100ms）
/// 2. 使用令牌桶算法平滑速率
/// 3. 返回需要等待的时间以达到目标速率
pub struct DynamicRateLimiter {
    controller: DynamicSpeedController,
    #[allow(dead_code)] // 用于日志/调试
    name: String,
    // 令牌桶状态
    tokens: f64,
    max_tokens: f64,
    last_refill: Instant,
    // 速率更新控制
    current_rate: usize,
    last_rate_update: Instant,
    rate_update_interval_ms: u64,
    // 统计
    total_consumed: usize,
    total_waited_ms: u64,
}

impl DynamicRateLimiter {
    /// 创建新的动态速率限制器
    ///
    /// # 参数
    /// - `profile` - 速度变化模型
    /// - `name` - 限制器名称（用于日志）
    pub fn new(profile: SpeedProfile, name: &str) -> Self {
        let mut controller = DynamicSpeedController::new(profile.clone());
        let initial_rate = controller.current_speed();
        let max_tokens = Self::calc_max_tokens(initial_rate);

        Self {
            controller,
            name: name.to_string(),
            tokens: max_tokens,
            max_tokens,
            last_refill: Instant::now(),
            current_rate: initial_rate,
            last_rate_update: Instant::now(),
            rate_update_interval_ms: 100, // 每 100ms 更新速率
            total_consumed: 0,
            total_waited_ms: 0,
        }
    }

    /// 从恒定速率创建（兼容旧 API）
    pub fn from_constant(rate: usize, name: &str) -> Self {
        Self::new(SpeedProfile::Constant(rate), name)
    }

    /// 计算最大令牌数（基于速率的 0.2 秒容量）
    fn calc_max_tokens(rate: usize) -> f64 {
        (rate as f64 * 0.2).max(10.0)
    }

    /// 获取当前目标速率
    pub fn current_rate(&self) -> usize {
        self.current_rate
    }

    /// 获取已消耗的总量
    pub fn total_consumed(&self) -> usize {
        self.total_consumed
    }

    /// 获取总等待时间（毫秒）
    pub fn total_waited_ms(&self) -> u64 {
        self.total_waited_ms
    }

    /// 更新速率（如果需要）
    fn maybe_update_rate(&mut self) {
        let _ = self.maybe_update_rate_at(Instant::now());
    }

    fn maybe_update_rate_at(&mut self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.last_rate_update).as_millis() as u64;

        if elapsed >= self.rate_update_interval_ms {
            let new_rate = self.controller.current_speed();
            let rate_changed = new_rate != self.current_rate;

            if rate_changed {
                // 在更新前按照旧速率补齐令牌
                self.refill_tokens_at(now);
                self.max_tokens = Self::calc_max_tokens(new_rate);
                self.tokens = self.tokens.min(self.max_tokens);
                self.current_rate = new_rate;
            }
            self.last_rate_update = now;
            return rate_changed;
        }

        false
    }

    fn refill_tokens_at(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        if self.current_rate > 0 {
            let refill = elapsed * self.current_rate as f64;
            self.tokens = (self.tokens + refill).min(self.max_tokens);
        }
    }

    /// 消耗令牌并返回需要等待的时间
    ///
    /// # 参数
    /// - `count` - 要消耗的数量
    ///
    /// # 返回
    /// 需要等待的时间。如果返回 Duration::ZERO，表示可以立即继续。
    pub fn consume(&mut self, count: usize) -> Duration {
        // 速率为 0 表示不限速
        if self.current_rate == 0 {
            self.total_consumed += count;
            return Duration::ZERO;
        }

        let now = Instant::now();
        let rate_changed = self.maybe_update_rate_at(now);

        if !rate_changed {
            self.refill_tokens_at(now);
        }

        // 消耗令牌
        self.tokens -= count as f64;
        self.total_consumed += count;

        if self.tokens < 0.0 {
            // 计算需要等待的时间
            let wait_secs = (-self.tokens) / self.current_rate as f64;
            let wait_duration = Duration::from_secs_f64(wait_secs);
            self.total_waited_ms += wait_duration.as_millis() as u64;
            wait_duration
        } else {
            Duration::ZERO
        }
    }

    /// 记录开始（兼容旧 API）
    pub fn rec_beg(&mut self) {
        // 仅更新速率，不消耗令牌
        self.maybe_update_rate();
    }

    /// 计算限速等待时间（兼容旧 API）
    ///
    /// 此方法假设在 rec_beg 和 limit_speed_time 之间消耗了 unit_size 个单位
    pub fn limit_speed_time(&mut self) -> Duration {
        self.consume(0) // 实际消耗在 consume 中处理
    }

    /// 消耗并等待（组合操作）
    pub async fn consume_and_wait(&mut self, count: usize) {
        let wait = self.consume(count);
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.total_consumed = 0;
        self.total_waited_ms = 0;
    }

    /// 重置所有状态
    pub fn reset(&mut self) {
        self.controller.reset();
        self.current_rate = self.controller.current_speed();
        self.max_tokens = Self::calc_max_tokens(self.current_rate);
        self.tokens = self.max_tokens;
        self.last_refill = Instant::now();
        self.last_rate_update = Instant::now();
        self.reset_stats();
    }
}

impl Default for DynamicRateLimiter {
    fn default() -> Self {
        Self::new(SpeedProfile::default(), "default")
    }
}

/// 创建速率限制器的便捷方法
impl From<SpeedProfile> for DynamicRateLimiter {
    fn from(profile: SpeedProfile) -> Self {
        Self::new(profile, "unnamed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limiter() {
        let limiter = DynamicRateLimiter::default();
        // 默认是 Constant(1000)
        assert_eq!(limiter.current_rate(), 1000);
    }

    #[test]
    fn test_from_profile() {
        let limiter: DynamicRateLimiter = SpeedProfile::Constant(5000).into();
        assert_eq!(limiter.current_rate(), 5000);
    }

    #[test]
    fn test_from_constant() {
        let limiter = DynamicRateLimiter::from_constant(3000, "test");
        assert_eq!(limiter.current_rate(), 3000);
    }

    #[test]
    fn test_constant_rate() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(1000), "test");

        // 消耗少量不应等待太久
        let wait = limiter.consume(10);
        assert!(wait.as_millis() < 100);
    }

    #[test]
    fn test_zero_rate_no_limit() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(0), "test");

        // 速率为 0 应该不限速
        let wait = limiter.consume(1000000);
        assert_eq!(wait, Duration::ZERO);
    }

    #[test]
    fn test_rate_update() {
        let mut limiter = DynamicRateLimiter::new(
            SpeedProfile::Ramp {
                start: 100,
                end: 10000,
                duration_secs: 1.0,
            },
            "test",
        );

        let initial_rate = limiter.current_rate();
        assert!((100..=200).contains(&initial_rate));

        // 等待并更新
        std::thread::sleep(Duration::from_millis(150));
        limiter.maybe_update_rate();

        // 速率应该有所增加
        let new_rate = limiter.current_rate();
        assert!(new_rate > initial_rate);
    }

    #[test]
    fn test_rate_change_does_not_overfill_tokens() {
        let mut limiter = DynamicRateLimiter::new(
            SpeedProfile::Stepped {
                steps: vec![(0.1, 100), (0.1, 10_000)],
                loop_forever: false,
            },
            "test",
        );

        // 进入第二阶段，速率大幅上升
        std::thread::sleep(Duration::from_millis(150));
        limiter.maybe_update_rate();

        // 消耗大量令牌，预期需要接近 0.2s 的等待（如果未被错误补偿）
        let wait = limiter.consume(2000);
        assert!(wait.as_millis() >= 150, "unexpected wait: {:?}", wait);
    }

    #[test]
    fn test_consume_tracking() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(10000), "test");

        limiter.consume(100);
        limiter.consume(200);
        limiter.consume(300);

        assert_eq!(limiter.total_consumed(), 600);
    }

    #[test]
    fn test_reset_stats() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(10000), "test");

        limiter.consume(100);
        limiter.consume(200);
        assert_eq!(limiter.total_consumed(), 300);

        limiter.reset_stats();
        assert_eq!(limiter.total_consumed(), 0);
        assert_eq!(limiter.total_waited_ms(), 0);
    }

    #[test]
    fn test_reset() {
        let mut limiter = DynamicRateLimiter::new(
            SpeedProfile::Ramp {
                start: 100,
                end: 10000,
                duration_secs: 1.0,
            },
            "test",
        );

        // 消耗一些并等待
        limiter.consume(50);
        std::thread::sleep(Duration::from_millis(200));
        limiter.maybe_update_rate();

        let rate_before = limiter.current_rate();
        let consumed_before = limiter.total_consumed();
        assert!(rate_before > 100);
        assert!(consumed_before > 0);

        // 重置后应回到初始状态
        limiter.reset();

        let rate_after = limiter.current_rate();
        assert!((100..=200).contains(&rate_after));
        assert_eq!(limiter.total_consumed(), 0);
    }

    #[test]
    fn test_rec_beg_updates_rate() {
        let mut limiter = DynamicRateLimiter::new(
            SpeedProfile::Ramp {
                start: 100,
                end: 10000,
                duration_secs: 1.0,
            },
            "test",
        );

        let initial_rate = limiter.current_rate();

        // 等待后调用 rec_beg 应该更新速率
        std::thread::sleep(Duration::from_millis(150));
        limiter.rec_beg();

        let new_rate = limiter.current_rate();
        assert!(new_rate > initial_rate);
    }

    #[test]
    fn test_high_rate_low_wait() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(100000), "test");

        // 高速率下，消耗少量几乎不需要等待
        for _ in 0..10 {
            let wait = limiter.consume(100);
            assert!(wait.as_millis() < 50);
        }
    }

    #[test]
    fn test_low_rate_needs_wait() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(10), "test");

        // 低速率下，消耗超过令牌桶容量需要等待
        // 最大令牌 = 10 * 0.2 = 2，但最小为 10
        // 消耗 20 应该需要等待
        let wait = limiter.consume(20);
        assert!(wait.as_millis() > 0);
    }

    #[test]
    fn test_wait_time_accumulation() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(100), "test");

        // 消耗大量导致等待
        limiter.consume(200);

        // 应该累计了等待时间
        let waited = limiter.total_waited_ms();
        assert!(waited > 0);
    }

    #[test]
    fn test_different_profiles() {
        // 测试各种 profile 都能正常创建和使用
        let profiles = vec![
            SpeedProfile::Constant(1000),
            SpeedProfile::sinusoidal(1000, 500, 60.0),
            SpeedProfile::stepped(vec![(10.0, 1000), (10.0, 2000)], true),
            SpeedProfile::burst(1000, 5000, 100, 0.1),
            SpeedProfile::ramp(100, 1000, 60.0),
            SpeedProfile::random_walk(1000, 0.3),
        ];

        for profile in profiles {
            let mut limiter = DynamicRateLimiter::new(profile, "test");
            let _rate = limiter.current_rate(); // 验证能正常获取速率
            let _ = limiter.consume(10);
        }
    }

    #[tokio::test]
    async fn test_consume_and_wait() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(100), "test");

        let start = Instant::now();
        // 消耗超过令牌桶容量，应该等待
        limiter.consume_and_wait(50).await;
        let elapsed = start.elapsed();

        // 应该有一些等待时间（但不会太长）
        assert!(elapsed.as_millis() < 1000);
    }

    #[tokio::test]
    async fn test_consume_and_wait_no_wait() {
        let mut limiter = DynamicRateLimiter::new(SpeedProfile::Constant(100000), "test");

        let start = Instant::now();
        // 高速率下消耗少量几乎不需要等待
        limiter.consume_and_wait(10).await;
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() < 50);
    }

    #[test]
    fn test_sinusoidal_limiter() {
        let mut limiter = DynamicRateLimiter::new(
            SpeedProfile::Sinusoidal {
                base: 1000,
                amplitude: 500,
                period_secs: 1.0,
            },
            "test",
        );

        let rate = limiter.current_rate();
        assert!((500..=1500).contains(&rate));

        let _ = limiter.consume(10);
        assert_eq!(limiter.total_consumed(), 10);
    }

    #[test]
    fn test_stepped_limiter() {
        let mut limiter = DynamicRateLimiter::new(
            SpeedProfile::Stepped {
                steps: vec![(0.5, 1000), (0.5, 5000)],
                loop_forever: false,
            },
            "test",
        );

        // 第一阶段
        assert_eq!(limiter.current_rate(), 1000);

        // 等待进入第二阶段
        std::thread::sleep(Duration::from_millis(600));
        limiter.maybe_update_rate();

        assert_eq!(limiter.current_rate(), 5000);
    }
}
