use crate::runtime::actor::limit::SourceRateLimiter;
use crate::runtime::collector::realtime::constants::picker_pending_max_bytes;
use crate::runtime::collector::realtime::picker::round::RoundStat;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

const DEFAULT_MIN_RPS: usize = 10_000;
const DEFAULT_MAX_RPS: usize = 10_000_000;
const DEFAULT_INCREASE_PCT: usize = 5;
const DEFAULT_STARTUP_INCREASE_PCT: usize = 50;
const DEFAULT_DECREASE_PCT: usize = 85;
const DEFAULT_MIN_INCREASE_STEP_PER_WORKER: usize = 5_000;
const DEFAULT_STARTUP_MIN_INCREASE_STEP_PER_WORKER: usize = 1_000;
const DEFAULT_STARTUP_WINDOW_MS: u64 = 5_000;
const DEFAULT_SAMPLE_WINDOW_MS: u64 = 1_000;
const DEFAULT_LOW_PENDING_BYTES_RATIO: usize = 30;
const DEFAULT_HIGH_PENDING_BYTES_RATIO: usize = 70;
const DEFAULT_RSS_GROWTH_MB: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRateDecision {
    Keep,
    Increase,
    Decrease,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AutoRateSample {
    pub pending_count: usize,
    pub pending_bytes: usize,
    pub round: RoundStat,
}

pub type SharedAutoRateController = Arc<Mutex<AutoRateController>>;

#[derive(Debug)]
pub struct AutoRateController {
    min_rps: usize,
    max_rps: usize,
    increase_pct: usize,
    startup_increase_pct: usize,
    decrease_pct: usize,
    min_increase_step: usize,
    startup_min_increase_step: usize,
    startup_window: Duration,
    sample_window: Duration,
    low_pending_bytes: usize,
    high_pending_bytes: usize,
    rss_growth_bytes: u64,
    rss_max_bytes: Option<u64>,
    cur_pid: Option<sysinfo::Pid>,
    sys: System,
    last_rss_bytes: Option<u64>,
    start_tick: Instant,
    last_tick: Instant,
    last_pending_bytes: usize,
    window_pending_count: usize,
    window_pending_bytes: usize,
    window_dist_pending: bool,
    window_sent_batches: usize,
}

impl AutoRateController {
    pub fn new() -> Self {
        Self::new_for_workers(1)
    }

    pub fn new_for_workers(workers: usize) -> Self {
        let workers = workers.max(1);
        let max_pending_bytes = picker_pending_max_bytes().max(1);
        let cur_pid = sysinfo::get_current_pid().ok();
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        Self {
            min_rps: read_env_usize("WP_SOURCE_AUTO_MIN_RPS").unwrap_or(DEFAULT_MIN_RPS),
            max_rps: read_env_usize("WP_SOURCE_AUTO_MAX_RPS").unwrap_or(DEFAULT_MAX_RPS),
            increase_pct: read_env_usize("WP_SOURCE_AUTO_INCREASE_PCT")
                .unwrap_or(DEFAULT_INCREASE_PCT),
            startup_increase_pct: read_env_usize("WP_SOURCE_AUTO_STARTUP_INCREASE_PCT")
                .unwrap_or(DEFAULT_STARTUP_INCREASE_PCT),
            decrease_pct: read_env_usize("WP_SOURCE_AUTO_DECREASE_PCT")
                .unwrap_or(DEFAULT_DECREASE_PCT),
            min_increase_step: read_env_usize("WP_SOURCE_AUTO_MIN_INCREASE_STEP")
                .unwrap_or(DEFAULT_MIN_INCREASE_STEP_PER_WORKER.saturating_mul(workers)),
            startup_min_increase_step: read_env_usize("WP_SOURCE_AUTO_STARTUP_MIN_INCREASE_STEP")
                .unwrap_or(DEFAULT_STARTUP_MIN_INCREASE_STEP_PER_WORKER.saturating_mul(workers)),
            startup_window: Duration::from_millis(
                read_env_usize("WP_SOURCE_AUTO_STARTUP_WINDOW_MS")
                    .unwrap_or(DEFAULT_STARTUP_WINDOW_MS as usize) as u64,
            ),
            sample_window: Duration::from_millis(
                read_env_usize("WP_SOURCE_AUTO_SAMPLE_WINDOW_MS")
                    .unwrap_or(DEFAULT_SAMPLE_WINDOW_MS as usize) as u64,
            ),
            low_pending_bytes: max_pending_bytes.saturating_mul(DEFAULT_LOW_PENDING_BYTES_RATIO)
                / 100,
            high_pending_bytes: max_pending_bytes.saturating_mul(DEFAULT_HIGH_PENDING_BYTES_RATIO)
                / 100,
            rss_growth_bytes: mb_to_bytes(
                read_env_usize("WP_SOURCE_AUTO_RSS_GROWTH_MB").unwrap_or(DEFAULT_RSS_GROWTH_MB),
            ),
            rss_max_bytes: read_env_usize("WP_SOURCE_AUTO_RSS_MAX_MB").map(mb_to_bytes),
            cur_pid,
            sys,
            last_rss_bytes: None,
            start_tick: Instant::now(),
            last_tick: Instant::now(),
            last_pending_bytes: 0,
            window_pending_count: 0,
            window_pending_bytes: 0,
            window_dist_pending: false,
            window_sent_batches: 0,
        }
    }

    pub fn shared() -> SharedAutoRateController {
        Self::shared_for_workers(1)
    }

    pub fn shared_for_workers(workers: usize) -> SharedAutoRateController {
        Arc::new(Mutex::new(Self::new_for_workers(workers)))
    }

    #[cfg(test)]
    fn for_test() -> Self {
        Self {
            min_rps: 100,
            max_rps: 10_000,
            increase_pct: 5,
            startup_increase_pct: 50,
            decrease_pct: 85,
            min_increase_step: 100,
            startup_min_increase_step: 1_000,
            startup_window: Duration::from_millis(0),
            sample_window: Duration::from_millis(0),
            low_pending_bytes: 300,
            high_pending_bytes: 700,
            rss_growth_bytes: mb_to_bytes(32),
            rss_max_bytes: None,
            cur_pid: None,
            sys: System::new_with_specifics(RefreshKind::nothing()),
            last_rss_bytes: None,
            start_tick: Instant::now(),
            last_tick: Instant::now(),
            last_pending_bytes: 0,
            window_pending_count: 0,
            window_pending_bytes: 0,
            window_dist_pending: false,
            window_sent_batches: 0,
        }
    }

    pub(crate) fn observe(
        &mut self,
        limiter: &SourceRateLimiter,
        sample: AutoRateSample,
    ) -> AutoRateDecision {
        if !limiter.is_auto() {
            return AutoRateDecision::Keep;
        }
        self.merge_sample(&sample);
        if self.last_tick.elapsed() < self.sample_window {
            return AutoRateDecision::Keep;
        }

        self.last_tick = Instant::now();
        let current = limiter
            .current_rate_per_sec()
            .clamp(self.min_rps, self.max_rps);
        let pending_count = self.window_pending_count;
        let pending_bytes = self.window_pending_bytes;
        let dist_pending = self.window_dist_pending;
        let sent_batches = self.window_sent_batches;
        let rss = self.sample_rss_bytes();
        let rss_growing_fast = self.rss_growing_fast(rss);
        let rss_over_max = self.rss_over_max_limit(rss);
        let decision = self.decide(
            pending_count,
            pending_bytes,
            dist_pending,
            sent_batches,
            rss_growing_fast,
            rss_over_max,
        );
        self.reset_window();
        let next = match decision {
            AutoRateDecision::Keep => current,
            AutoRateDecision::Increase => {
                let (increase_pct, min_increase_step) = self.increase_params();
                let step = current
                    .saturating_mul(increase_pct)
                    .checked_div(100)
                    .unwrap_or(0)
                    .max(min_increase_step);
                current
                    .saturating_add(step)
                    .clamp(self.min_rps, self.max_rps)
            }
            AutoRateDecision::Decrease => current
                .saturating_mul(self.decrease_pct)
                .checked_div(100)
                .unwrap_or(self.min_rps)
                .clamp(self.min_rps, self.max_rps),
        };

        if next != limiter.current_rate_per_sec() {
            limiter.set_rate_per_sec(next);
            info_ctrl!(
                "source auto rate limit {:?}: {} -> {} rps (pending_count={}, pending_bytes={}, rss_mib={:.1})",
                decision,
                current,
                next,
                pending_count,
                pending_bytes,
                rss.map(|v| v as f64 / 1024.0 / 1024.0).unwrap_or(0.0)
            );
        }
        self.last_pending_bytes = pending_bytes;
        self.record_rss_bytes(rss);
        decision
    }

    fn merge_sample(&mut self, sample: &AutoRateSample) {
        self.window_pending_count = self.window_pending_count.max(sample.pending_count);
        self.window_pending_bytes = self.window_pending_bytes.max(sample.pending_bytes);
        self.window_dist_pending |= sample.round.dist_status().is_pending();
        self.window_sent_batches = self
            .window_sent_batches
            .saturating_add(sample.round.send_cnt());
    }

    fn increase_params(&self) -> (usize, usize) {
        if self.start_tick.elapsed() < self.startup_window {
            (self.startup_increase_pct, self.startup_min_increase_step)
        } else {
            (self.increase_pct, self.min_increase_step)
        }
    }

    fn reset_window(&mut self) {
        self.window_pending_count = 0;
        self.window_pending_bytes = 0;
        self.window_dist_pending = false;
        self.window_sent_batches = 0;
    }

    fn decide(
        &self,
        pending_count: usize,
        pending_bytes: usize,
        dist_pending: bool,
        sent_batches: usize,
        rss_growing_fast: bool,
        rss_over_max: bool,
    ) -> AutoRateDecision {
        let pending_growing = pending_bytes > self.last_pending_bytes;
        if dist_pending
            || pending_bytes >= self.high_pending_bytes
            || (pending_growing && pending_bytes > self.low_pending_bytes)
            || rss_growing_fast
            || rss_over_max
        {
            return AutoRateDecision::Decrease;
        }

        if pending_bytes <= self.low_pending_bytes && pending_count == 0 && sent_batches > 0 {
            return AutoRateDecision::Increase;
        }

        AutoRateDecision::Keep
    }

    fn sample_rss_bytes(&mut self) -> Option<u64> {
        let pid = self.cur_pid?;
        let _ = self
            .sys
            .refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        self.sys.process(pid).map(|p| p.memory())
    }

    fn rss_over_max_limit(&self, rss: Option<u64>) -> bool {
        let Some(rss) = rss else {
            return false;
        };
        let Some(rss_max) = self.rss_max_bytes else {
            return false;
        };
        rss > rss_max
    }

    fn rss_growing_fast(&self, rss: Option<u64>) -> bool {
        let Some(rss) = rss else {
            return false;
        };
        let Some(last_rss) = self.last_rss_bytes else {
            return false;
        };
        rss > last_rss.saturating_add(self.rss_growth_bytes)
    }

    fn record_rss_bytes(&mut self, rss: Option<u64>) {
        let Some(rss) = rss else {
            return;
        };
        self.last_rss_bytes = Some(rss);
    }
}

fn read_env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok()?.parse::<usize>().ok()
}

fn mb_to_bytes(mb: usize) -> u64 {
    mb.saturating_mul(1024).saturating_mul(1024) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::collector::realtime::picker::round::RoundStat;

    fn limiter() -> SourceRateLimiter {
        SourceRateLimiter::new(0).expect("auto limiter")
    }

    fn sample(pending_count: usize, pending_bytes: usize, round: RoundStat) -> AutoRateSample {
        AutoRateSample {
            pending_count,
            pending_bytes,
            round,
        }
    }

    #[test]
    fn low_watermark_increases_auto_rate() {
        let limiter = limiter();
        limiter.set_rate_per_sec(1_000);
        let mut ctl = AutoRateController::for_test();
        let mut round = RoundStat::new();
        round.add_proc(1);

        let decision = ctl.observe(&limiter, sample(0, 0, round));

        assert_eq!(decision, AutoRateDecision::Increase);
        assert_eq!(limiter.current_rate_per_sec(), 1_100);
    }

    #[test]
    fn startup_window_increases_auto_rate_faster() {
        let limiter = limiter();
        limiter.set_rate_per_sec(1_000);
        let mut ctl = AutoRateController::for_test();
        ctl.startup_window = Duration::from_secs(60);
        let mut round = RoundStat::new();
        round.add_proc(1);

        let decision = ctl.observe(&limiter, sample(0, 0, round));

        assert_eq!(decision, AutoRateDecision::Increase);
        assert_eq!(limiter.current_rate_per_sec(), 2_000);
    }

    #[test]
    fn high_watermark_decreases_auto_rate() {
        let limiter = limiter();
        limiter.set_rate_per_sec(1_000);
        let mut ctl = AutoRateController::for_test();
        let mut round = RoundStat::new();
        round.add_proc(1);

        let decision = ctl.observe(&limiter, sample(3, 900, round));

        assert_eq!(decision, AutoRateDecision::Decrease);
        assert_eq!(limiter.current_rate_per_sec(), 850);
    }

    #[test]
    fn parse_backpressure_decreases_auto_rate() {
        let limiter = limiter();
        limiter.set_rate_per_sec(1_000);
        let mut ctl = AutoRateController::for_test();
        let mut round = RoundStat::new();
        round.to_dist_pending();

        let decision = ctl.observe(&limiter, sample(1, 1, round));

        assert_eq!(decision, AutoRateDecision::Decrease);
        assert_eq!(limiter.current_rate_per_sec(), 850);
    }

    #[test]
    fn fixed_limiter_is_not_changed_by_auto_controller() {
        let limiter = SourceRateLimiter::new(1_000).expect("fixed limiter");
        let mut ctl = AutoRateController::for_test();
        let mut round = RoundStat::new();
        round.to_dist_pending();

        let decision = ctl.observe(&limiter, sample(1, 900, round));

        assert_eq!(decision, AutoRateDecision::Keep);
        assert_eq!(limiter.current_rate_per_sec(), 1_000);
    }

    #[test]
    fn rss_without_max_limit_does_not_block_increase() {
        let ctl = AutoRateController::for_test();

        let decision = ctl.decide(0, 0, false, 1, false, false);

        assert_eq!(decision, AutoRateDecision::Increase);
    }

    #[test]
    fn rss_fast_growth_decreases_auto_rate() {
        let ctl = AutoRateController::for_test();

        let decision = ctl.decide(0, 0, false, 1, true, false);

        assert_eq!(decision, AutoRateDecision::Decrease);
    }

    #[test]
    fn rss_max_limit_decreases_auto_rate() {
        let ctl = AutoRateController::for_test();

        let decision = ctl.decide(0, 0, false, 1, false, true);

        assert_eq!(decision, AutoRateDecision::Decrease);
    }
}
