use std::cell::Cell;
use std::time::Duration;

use derive_getters::Getters;

pub type SystemInstant = std::time::Instant;
#[derive(Getters, Debug)]
pub struct RateLimiter {
    limit_ns: Duration,
    /// 下一次出发（允许继续发送）的绝对时间点；基于周期累加，具备补偿能力。
    next_deadline: Cell<SystemInstant>,
    run_cnt: usize,
    unit_cnt: usize,
}

impl Default for RateLimiter {
    fn default() -> Self {
        // 无限制：零等待
        let limit_ns = Duration::from_nanos(0);
        info_ctrl!("speed: unlimited (no wait)");
        Self {
            limit_ns,
            next_deadline: Cell::new(SystemInstant::now()),
            run_cnt: 0,
            unit_cnt: 1,
        }
    }
}
impl RateLimiter {
    pub fn new(sec_count: usize, unit_cnt: usize, msg: &str) -> Self {
        // Treat sec_count == 0 as unlimited (no rate limiting)
        let limit_ns = if sec_count == 0 {
            Duration::from_nanos(0)
        } else {
            let nanos = 1_000_000_000_u128.saturating_mul(unit_cnt as u128) / (sec_count as u128);
            Duration::from_nanos(nanos as u64)
        };
        info_ctrl!(
            "{} speed init {}  limit ns times:{},",
            msg,
            sec_count,
            limit_ns.as_nanos()
        );
        let now = SystemInstant::now();
        // deadline/tick：下一次出发时间为 now + period（无限制时等价于 now）。
        let next_deadline = if limit_ns.is_zero() {
            now
        } else {
            now + limit_ns
        };
        Self {
            limit_ns,
            next_deadline: Cell::new(next_deadline),
            run_cnt: 0,
            unit_cnt,
        }
    }
    pub fn new_or_default(sec_count: Option<usize>, unit_cnt: usize, msg: &str) -> Self {
        match sec_count {
            Some(limit) => Self::new(limit, unit_cnt, msg),
            None => Self::default(),
        }
    }
    #[inline]
    pub fn rec_beg(&mut self) {
        self.run_cnt += self.unit_cnt;
    }
    /// Async-friendly sleep helper for rate limiting; prefer this inside async tasks.
    #[cfg(any(test, feature = "dev-tools"))]
    #[allow(dead_code)]
    pub async fn limit_speed_wait_async(&self) {
        let wait_time = self.limit_speed_time();
        if wait_time.as_nanos() > 0 {
            tokio::time::sleep(wait_time).await;
        }
    }
    pub fn limit_speed_time(&self) -> Duration {
        // deadline/tick 限速：按下一次出发时间进行补偿，避免累积误差。
        if self.limit_ns.is_zero() {
            return Duration::from_nanos(0);
        }
        let now = SystemInstant::now();
        let deadline = self.next_deadline.get();
        if now < deadline {
            let wait = deadline - now;
            // 下一轮出发时间按固定周期累加（非 now 基准），具备补偿能力。
            self.next_deadline.set(deadline + self.limit_ns);
            wait
        } else {
            // 已经超过出发时间：按周期向前追齐，不额外等待。
            let behind = now.duration_since(deadline);
            if !self.limit_ns.is_zero() {
                // 计算错过了多少个周期（至少推进 1 个周期）
                let period_ns = self.limit_ns.as_nanos();
                let missed = (behind.as_nanos() / period_ns) + 1;
                let advance = self
                    .limit_ns
                    .saturating_mul(missed as u32 /* n<=u32::MAX in practice */);
                self.next_deadline.set(deadline + advance);
            } else {
                self.next_deadline.set(now);
            }
            Duration::from_nanos(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::runtime::actor::limit::RateLimiter;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_limit() {
        let mut sp_limit = RateLimiter::new(1000, 1, "test");
        let now = tokio::time::Instant::now();
        for _ in 0..2000 {
            sp_limit.rec_beg();
            sp_limit.limit_speed_wait_async().await;
        }
        let end = tokio::time::Instant::now();
        let elapsed = end - now;
        println!("cost : {:?}", elapsed.as_millis());
        // 期望值为 2000 * 1ms = 2s；异步定时器及调度存在开销，给出宽松上界
        assert!(elapsed > Duration::from_secs(2));
        // Allow generous upper bound to account for timer granularity and CI scheduler jitter
        assert!(elapsed < Duration::from_millis(7000));
    }
}
