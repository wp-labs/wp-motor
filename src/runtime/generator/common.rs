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

/// 单次批量发送的字节预算上限：攒到该字节量就发一次，
/// 使每次 write 的大小稳定可控，不依赖行长。
/// 默认 64KiB（保守值，平衡 syscall 频次与 TCP send buffer 压力）。
pub const BATCH_FLUSH_BYTES: usize = 64 * 1024;

/// 单个子批的行数兼底上限：防止单行特别大时一个子批只有 1 行、
/// 也防止极端情况下子批过大。每个子批达到行数或字节预算任一即下发。
pub const BATCH_FLUSH_LINES: usize = 4_096;

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
