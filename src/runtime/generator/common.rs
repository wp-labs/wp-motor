use crate::orchestrator::config::build_sinks::build_sink_target;
use crate::sinks::SinkBackendType;
use wp_conf::structure::SinkInstanceConf;
use wp_error::run_error::RunResult;

#[inline]
pub async fn build_sink_instance(
    out_conf: &SinkInstanceConf,
    replica_idx: usize,
    replica_cnt: usize,
    rate_limit_rps: usize,
) -> RunResult<SinkBackendType> {
    build_sink_target(out_conf, replica_idx, replica_cnt, rate_limit_rps).await
}

// Defaults (no env toggles)
pub const DEFAULT_BATCH: usize = 128;
pub const DEFAULT_UNIT_SIZE: usize = 50_000;

/// 单个子批的行数兼底上限：防止单行特别大时一个子批只有 1 行、
/// 也防止极端情况下子批过大。每个子批达到行数或字节预算任一即下发。
pub const BATCH_FLUSH_LINES: usize = 4_096;

// ===== 动态批量大小策略：rate × 动态行长(EMA) × 时间窗 =====
//
// 子批字节预算不再固定，而是根据：
//   budget = base_rate(EPS) × avg_line_bytes(EMA) × time_window
// 自适应计算。混合日志下 EMA 跟踪行长分布的漂移。
// - 不限速(base_rate=0) → 封顶 MAX，极致吞吐
// - 高字节速率 → 大批（数据累积快，延迟仍被时间窗兜底）
// - 低字节速率 → 小批（低延迟），但用 MIN 兜底避免退化成逐行 syscall
// - 首次观察前用种子值，避免 avg=0 导致预算异常

/// 预算下限：避免极低速时退化成逐行 flush（8KiB）
pub const BATCH_BUDGET_MIN: usize = 8 * 1024;
/// 预算上限：不限速/极高速率封顶（1MiB）
pub const BATCH_BUDGET_MAX: usize = 1024 * 1024;
/// 首次观察前的种子预算（对齐历史 256KiB 默认）
pub const BATCH_SEED_BUDGET: usize = 256 * 1024;
/// EMA 平滑系数：越小越平滑、跟踪越慢；越大越敏感、拓动越大
pub const BATCH_EMA_ALPHA: f64 = 0.02;
/// 时间窗（秒）：单批最大可接受延迟，预算 = 字节速率 × 时间窗
pub const BATCH_TIME_WINDOW_SECS: f64 = 0.1;

/// 批量发送大小策略：根据速率 × 动态行长(EMA) × 时间窗自适应计算子批字节预算。
///
/// 混合日志下，`avg_line_bytes` 用 EMA 跟踪行长分布的变化；每发一行调用 `observe_line`
/// 更新，`budget_bytes` 返回当前应使用的预算。
pub struct BatchSizePolicy {
    base_rate: usize,
    avg_line_bytes: f64,
    initialized: bool,
}

impl BatchSizePolicy {
    pub fn new(base_rate: usize) -> Self {
        Self {
            base_rate,
            avg_line_bytes: 0.0,
            initialized: false,
        }
    }

    /// 观察一行，用 EMA 更新平均行长。首行直接初始化。
    pub fn observe_line(&mut self, len: usize) {
        if !self.initialized {
            self.avg_line_bytes = len as f64;
            self.initialized = true;
        } else {
            self.avg_line_bytes =
                BATCH_EMA_ALPHA * (len as f64) + (1.0 - BATCH_EMA_ALPHA) * self.avg_line_bytes;
        }
    }

    /// 当前子批字节预算。
    pub fn budget_bytes(&self) -> usize {
        if self.base_rate == 0 {
            return BATCH_BUDGET_MAX; // 不限速：1MiB 封顶
        }
        if !self.initialized {
            return BATCH_SEED_BUDGET; // 首次观察前：种子值
        }
        // 字节速率(bytes/sec) = EPS × 平均行长
        let byte_rate = self.base_rate as f64 * self.avg_line_bytes;
        let budget = byte_rate * BATCH_TIME_WINDOW_SECS;
        (budget as usize).clamp(BATCH_BUDGET_MIN, BATCH_BUDGET_MAX)
    }

    /// 当前 EMA 平均行长（测试/观测用）
    #[allow(dead_code)]
    pub fn avg_line_bytes(&self) -> f64 {
        self.avg_line_bytes
    }
}

#[inline]
pub fn default_batch() -> usize {
    DEFAULT_BATCH
}

/// 平均切分总量到并行流水线（余数前置）。
pub fn split_total_among_parallel(parallel: usize, total: Option<usize>) -> Vec<Option<usize>> {
    let p = parallel.max(1);
    let mut per = Vec::with_capacity(p);
    if let Some(t) = total {
        let base = t / p;
        let rem = t % p;
        for i in 0..p {
            per.push(Some(base + if i < rem { 1 } else { 0 }));
        }
    } else {
        per.resize(p, None);
    }
    per
}

/// 从总速率推导每流水线速率；为 0 则表示不限速。
pub fn per_pipeline_speed(global_speed: usize, parallel: usize) -> Option<usize> {
    if global_speed > 0 {
        Some(std::cmp::max(1, global_speed / parallel.max(1)))
    } else {
        None
    }
}
