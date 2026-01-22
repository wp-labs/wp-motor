use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ---- Constants (backoff, probing, drain) ----
pub const NET_SENDQ_BACKOFF_HIGH_PCT: u8 = 60; // 高水位(%)
pub const NET_SENDQ_BACKOFF_SLEEP_MS: u64 = 2; // 固定退让时长(ms)
pub const NET_SENDQ_PROBE_STRIDE: u64 = 32; // 回退：写计数退避步长
pub const NET_TCP_DRAIN_POLL_MS: u64 = 10; // 关闭前排空轮询间隔(ms)

// 对极小包（<=1KiB），默认绕过绝大多数探测与 sleep，交由内核流控
pub const NET_BACKOFF_SMALL_BYPASS_BYTES: usize = 1024;
/// 小包的最小时间门控（毫秒）
pub const NET_BACKOFF_SMALL_PROBE_MS: u64 = 1;
/// 大包的最小时间门控（毫秒）
pub const NET_BACKOFF_LARGE_PROBE_MS: u64 = 1;

// --- Probe stride tunables ---
/// 小包探测：cap/16 作为基础步长；随后 clamp 到 [64KiB, 256KiB]
pub const NET_SMALL_STRIDE_BASE_DIV: usize = 16;
pub const NET_SMALL_STRIDE_MIN_BYTES: usize = 64 * 1024;
pub const NET_SMALL_STRIDE_MAX_BYTES: usize = 256 * 1024;
/// 大包探测的最小步长下限
pub const NET_LARGE_STRIDE_MIN_BYTES: usize = 16 * 1024;

/// 计算“小包探测”的字节步长：clamp(cap/NET_SMALL_STRIDE_BASE_DIV, [NET_SMALL_STRIDE_MIN_BYTES, NET_SMALL_STRIDE_MAX_BYTES])
#[inline]
pub fn small_probe_stride(sndbuf_cap: usize) -> usize {
    let mut base = sndbuf_cap / NET_SMALL_STRIDE_BASE_DIV;
    if base > NET_SMALL_STRIDE_MAX_BYTES {
        base = NET_SMALL_STRIDE_MAX_BYTES;
    }
    NET_SMALL_STRIDE_MIN_BYTES.max(base)
}

/// 计算“中/大包探测”的字节步长：max(cap/NET_SMALL_STRIDE_BASE_DIV, avg*NET_SMALL_STRIDE_BASE_DIV, NET_LARGE_STRIDE_MIN_BYTES)
#[inline]
pub fn large_probe_stride(sndbuf_cap: usize, avg: usize) -> usize {
    let cap_stride = sndbuf_cap / NET_SMALL_STRIDE_BASE_DIV;
    let avg_stride = avg.saturating_mul(NET_SMALL_STRIDE_BASE_DIV);
    cap_stride.max(avg_stride).max(NET_LARGE_STRIDE_MIN_BYTES)
}

/// 判断“写计数触发点”是否到期：默认等价于 sent_cnt % NET_SENDQ_PROBE_STRIDE == 0。
/// 若步长为 2 的幂，使用位运算优化；否则回退到取模。
#[inline]
pub fn is_probe_tick_due(sent_cnt: u64) -> bool {
    if NET_SENDQ_PROBE_STRIDE.is_power_of_two() {
        let mask = NET_SENDQ_PROBE_STRIDE - 1;
        (sent_cnt & mask) == 0
    } else if NET_SENDQ_PROBE_STRIDE > 0 {
        sent_cnt.is_multiple_of(NET_SENDQ_PROBE_STRIDE)
    } else {
        false
    }
}
/// 紧急保护阈值（按平均包长分档，单位 %）与微睡眠时长（毫秒）。
/// 建议默认：
/// - <=400B: 30%
/// - <=1KiB: 50%
/// - <=2KiB: 60%
/// -  >2KiB: 80%
pub const NET_EMERG_PCT_400B: u8 = 30;
pub const NET_EMERG_PCT_1K: u8 = 50;
pub const NET_EMERG_PCT_2K: u8 = 60;
pub const NET_EMERG_PCT_DEFAULT: u8 = 80;
pub const NET_EMERGENCY_SLEEP_MS: u64 = 2;

/// 根据当前平均包长选择“紧急水位”阈值（百分比）。
#[inline]
pub fn emergency_pct_for(avg_len: usize) -> u8 {
    if avg_len <= 400 {
        NET_EMERG_PCT_400B
    } else if avg_len <= 1024 {
        NET_EMERG_PCT_1K
    } else if avg_len <= 2048 {
        NET_EMERG_PCT_2K
    } else {
        NET_EMERG_PCT_DEFAULT
    }
}

/// 自适应退让的内部配置
#[derive(Clone, Copy, Debug)]
pub struct AdaptiveCfg {
    pub(crate) target_pct: u8,
    pub(crate) hysteresis_pct: u8,
    pub(crate) min_ms: u64,
    pub(crate) max_ms: u64,
    pub(crate) step_ms: u64,
    pub(crate) cur_ms: u64,
}
impl Default for AdaptiveCfg {
    fn default() -> Self {
        Self {
            target_pct: 30,
            hysteresis_pct: 5,
            min_ms: 0,
            max_ms: 8,
            step_ms: 1,
            cur_ms: 0,
        }
    }
}

/// 可选的发送队列背压策略配置
#[derive(Clone, Copy, Debug)]
pub struct BackpressureCfg {
    pub(crate) high_water_pct: u8,
    pub(crate) sleep_ms: u64,
    pub(crate) adaptive: Option<AdaptiveCfg>,
}
impl Default for BackpressureCfg {
    fn default() -> Self {
        Self {
            high_water_pct: NET_SENDQ_BACKOFF_HIGH_PCT,
            sleep_ms: NET_SENDQ_BACKOFF_SLEEP_MS,
            adaptive: None,
        }
    }
}
impl BackpressureCfg {
    pub fn adaptive_default() -> Self {
        Self {
            high_water_pct: NET_SENDQ_BACKOFF_HIGH_PCT,
            sleep_ms: 0,
            adaptive: Some(AdaptiveCfg::default()),
        }
    }
    pub(crate) fn auto_sleep_ms(&mut self, pct: u8) -> u64 {
        if let Some(mut ad) = self.adaptive {
            let hi = ad.target_pct.saturating_add(ad.hysteresis_pct);
            let lo = ad.target_pct.saturating_sub(ad.hysteresis_pct);
            if pct > hi {
                ad.cur_ms = (ad.cur_ms + ad.step_ms).min(ad.max_ms);
            } else if pct < lo {
                ad.cur_ms = ad.cur_ms.saturating_sub(ad.step_ms);
                if ad.cur_ms < ad.min_ms {
                    ad.cur_ms = ad.min_ms;
                }
            }
            self.sleep_ms = ad.cur_ms;
            self.adaptive = Some(ad);
            ad.cur_ms
        } else if pct >= self.high_water_pct && self.sleep_ms > 0 {
            self.sleep_ms
        } else {
            0
        }
    }
}

/// Backoff 模式
#[derive(Clone, Copy, Debug)]
pub enum BackoffMode {
    Auto,
    ForceOn,
    ForceOff,
}

/// 发送策略（构建期确定）
#[derive(Clone, Copy, Debug)]
pub struct NetSendPolicy {
    pub rate_limit_rps: usize,
    pub backoff_mode: BackoffMode,
    pub adaptive: bool,
}
impl Default for NetSendPolicy {
    fn default() -> Self {
        Self {
            rate_limit_rps: 0,
            backoff_mode: BackoffMode::Auto,
            adaptive: true,
        }
    }
}

// ---- Global toggles/hints ----
static NET_BACKOFF_ADAPTIVE_ENABLE: AtomicBool = AtomicBool::new(true);
pub fn set_net_backoff_adaptive(v: bool) {
    NET_BACKOFF_ADAPTIVE_ENABLE.store(v, Ordering::Relaxed);
}
pub fn net_backoff_adaptive() -> bool {
    NET_BACKOFF_ADAPTIVE_ENABLE.load(Ordering::Relaxed)
}

thread_local! {
    static TCP_BUILD_RATE_LIMIT_HINT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
pub fn set_tcp_build_rate_limit_hint(rps: Option<usize>) {
    TCP_BUILD_RATE_LIMIT_HINT.with(|c| c.set(rps));
}
pub fn get_tcp_build_rate_limit_hint() -> Option<usize> {
    TCP_BUILD_RATE_LIMIT_HINT.with(|c| c.get())
}

static GLOBAL_RATE_LIMIT_RPS: AtomicUsize = AtomicUsize::new(0);
pub fn set_global_rate_limit_rps(v: usize) {
    GLOBAL_RATE_LIMIT_RPS.store(v, Ordering::Relaxed);
}
pub fn get_global_rate_limit_rps() -> usize {
    GLOBAL_RATE_LIMIT_RPS.load(Ordering::Relaxed)
}
