use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use derive_getters::Getters;
use tokio::sync::Mutex;

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

#[derive(Debug)]
struct SourceRateLimiterState {
    next_deadline: SystemInstant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRateMode {
    Auto,
    Fixed,
}

/// Shared input-side rate limiter for all source workers in one runtime.
#[derive(Clone, Debug)]
pub struct SourceRateLimiter {
    mode: SourceRateMode,
    rate_per_sec: Arc<AtomicUsize>,
    lease_events: Arc<AtomicUsize>,
    state: Arc<Mutex<SourceRateLimiterState>>,
}

#[derive(Debug)]
pub struct SourceRateLease {
    limiter: SourceRateLimiter,
    remaining_events: usize,
}

impl SourceRateLimiter {
    pub fn new(rate_per_sec: usize) -> Option<Self> {
        if rate_per_sec == 0 && source_rate_limit_mode_off() {
            return None;
        }

        let mode = if rate_per_sec == 0 {
            SourceRateMode::Auto
        } else {
            SourceRateMode::Fixed
        };
        let initial_rate = if mode == SourceRateMode::Auto {
            auto_initial_rate_per_sec()
        } else {
            rate_per_sec
        };

        Some(Self::with_mode(mode, initial_rate))
    }

    fn with_mode(mode: SourceRateMode, rate_per_sec: usize) -> Self {
        Self {
            mode,
            rate_per_sec: Arc::new(AtomicUsize::new(rate_per_sec)),
            lease_events: Arc::new(AtomicUsize::new(Self::default_lease_events(rate_per_sec))),
            state: Arc::new(Mutex::new(SourceRateLimiterState {
                next_deadline: SystemInstant::now(),
            })),
        }
    }

    pub fn is_auto(&self) -> bool {
        self.mode == SourceRateMode::Auto
    }

    pub fn current_rate_per_sec(&self) -> usize {
        self.rate_per_sec.load(Ordering::Relaxed)
    }

    pub fn set_rate_per_sec(&self, rate_per_sec: usize) {
        self.rate_per_sec.store(rate_per_sec, Ordering::Relaxed);
        self.lease_events
            .store(Self::default_lease_events(rate_per_sec), Ordering::Relaxed);
    }

    pub fn new_lease(&self) -> SourceRateLease {
        SourceRateLease {
            limiter: self.clone(),
            remaining_events: 0,
        }
    }

    fn lease_events(&self) -> usize {
        self.lease_events.load(Ordering::Relaxed)
    }

    fn default_lease_events(rate_per_sec: usize) -> usize {
        if let Ok(raw) = std::env::var("WP_SOURCE_RATE_LEASE_EVENTS")
            && let Ok(value) = raw.parse::<usize>()
            && value > 0
        {
            return value;
        }

        // Around 200 global limiter hits/sec/source at high rates, while keeping
        // low-rate configs responsive and avoiding one lock per tiny batch.
        (rate_per_sec / 200).clamp(1, 4096)
    }

    async fn wait_for_events(&self, events: usize) {
        if events == 0 {
            return;
        }
        let Some(duration) = self.duration_for(events) else {
            return;
        };
        let wait = {
            let mut state = self.state.lock().await;
            let now = SystemInstant::now();
            let wait = state.next_deadline.saturating_duration_since(now);
            let base = if wait.is_zero() {
                now
            } else {
                state.next_deadline
            };
            state.next_deadline = base + duration;
            wait
        };

        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    #[inline]
    fn duration_for(&self, events: usize) -> Option<Duration> {
        let rate_per_sec = self.current_rate_per_sec();
        if rate_per_sec == 0 {
            return None;
        }
        let nanos = 1_000_000_000_u128.saturating_mul(events as u128) / rate_per_sec as u128;
        Some(Duration::from_nanos(nanos.clamp(1, u64::MAX as u128) as u64))
    }
}

impl SourceRateLease {
    pub fn is_fixed(&self) -> bool {
        self.limiter.mode == SourceRateMode::Fixed
    }

    pub async fn consume(&mut self, events: usize) {
        if events == 0 {
            return;
        }

        let mut required = events;
        while required > self.remaining_events {
            required -= self.remaining_events;
            let grant = self.limiter.lease_events().max(required);
            self.limiter.wait_for_events(grant).await;
            self.remaining_events = grant;
        }
        self.remaining_events -= required;
    }
}

fn source_rate_limit_mode_off() -> bool {
    std::env::var("WP_SOURCE_RATE_LIMIT_MODE")
        .map(|v| v.eq_ignore_ascii_case("off"))
        .unwrap_or(false)
}

fn auto_initial_rate_per_sec() -> usize {
    read_env_usize("WP_SOURCE_AUTO_INITIAL_RPS").unwrap_or(10_000)
}

pub(crate) fn source_auto_initial_rate_is_overridden() -> bool {
    read_env_usize("WP_SOURCE_AUTO_INITIAL_RPS").is_some()
}

fn read_env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok()?.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::runtime::actor::limit::{RateLimiter, SourceRateLease, SourceRateLimiter};

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

    #[test]
    fn source_rate_limiter_auto_when_rate_is_zero() {
        let limiter = SourceRateLimiter::new(0).expect("auto limiter");
        assert!(limiter.is_auto());
        assert!(limiter.current_rate_per_sec() > 0);
    }

    #[test]
    fn source_rate_limiter_fixed_when_rate_is_nonzero() {
        let limiter = SourceRateLimiter::new(100).expect("fixed limiter");
        assert!(!limiter.is_auto());
        assert_eq!(limiter.current_rate_per_sec(), 100);
    }

    #[test]
    fn source_rate_limiter_can_update_dynamic_rate() {
        let limiter = SourceRateLimiter::new(0).expect("auto limiter");
        limiter.set_rate_per_sec(1234);
        assert_eq!(limiter.current_rate_per_sec(), 1234);
    }

    #[test]
    fn source_rate_limiter_lease_size_scales_and_clamps() {
        if std::env::var("WP_SOURCE_RATE_LEASE_EVENTS").is_ok() {
            return;
        }

        let low = SourceRateLimiter::new(100).expect("limiter");
        assert_eq!(low.lease_events(), 1);

        let high = SourceRateLimiter::new(10_000_000).expect("limiter");
        assert_eq!(high.lease_events(), 4096);
    }

    #[tokio::test]
    async fn source_rate_limiter_shared_limiter_delays_following_events() {
        let limiter = SourceRateLimiter::new(1_000).expect("limiter");

        limiter.wait_for_events(1_000).await;
        let start = std::time::Instant::now();
        limiter.wait_for_events(1).await;

        assert!(
            start.elapsed() >= Duration::from_millis(900),
            "second lease should wait on the shared deadline"
        );
    }

    #[tokio::test]
    async fn source_rate_limiter_local_lease_reuses_remaining_events_without_waiting() {
        let limiter = SourceRateLimiter::new(1_000).expect("limiter");
        let mut lease = SourceRateLease {
            limiter,
            remaining_events: 10,
        };

        let start = std::time::Instant::now();
        lease.consume(3).await;

        assert!(
            start.elapsed() < Duration::from_millis(50),
            "local remaining lease should not touch the global clock"
        );
        assert_eq!(lease.remaining_events, 7);
    }
}
