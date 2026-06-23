//! Shared constants for collector realtime components.

/// Control channel buffer size when bridging actor commands to data sources.
pub(crate) const PICKER_CTRL_EVENT_BUFFER: usize = 1024;

/// Default rounds per dispatch loop iteration.
pub(crate) const PICKER_DEFAULT_ROUND_BATCH: usize = 10;

/// Estimated events processed per round batch (used for throttle window sizing).
pub(crate) const PICKER_EVENT_CNT_OF_BATCH: usize = 100;

/// Default pending queue capacity for `ActPicker`.
pub(crate) const PICKER_PENDING_CAPACITY: usize = 64;

/// Soft byte budget for picker pending backlog.
/// Once pending bytes reach this watermark, picker stops pulling more source batches
/// and waits for parser-side draining to catch up.
pub(crate) fn picker_pending_max_bytes() -> usize {
    wp_conf::limits::picker_pending_max_bytes()
}

pub(crate) fn picker_burst_max() -> usize {
    wp_conf::limits::picker_burst_max()
}

/// Post-policy initial backoff rounds.
pub(crate) const PICKER_POST_BACKOFF_INITIAL_ROUNDS: u32 = 1;

/// Post-policy maximum backoff rounds.
pub(crate) const PICKER_POST_BACKOFF_MAX_ROUNDS: u32 = 8;

/// Growth factor for post-policy exponential backoff.
pub(crate) const PICKER_POST_BACKOFF_GROWTH_FACTOR: u32 = 2;

/// Pull-policy low watermark multiplier relative to burst size.
pub(crate) const PICKER_PULL_LO_MULTIPLIER: usize = 2;

/// Pull-policy high watermark multiplier relative to burst size.
/// Balanced: 3（原 4）— 更早停止拉取，降低 pending 高水位时长
pub(crate) const PICKER_PULL_HI_MULTIPLIER: usize = 3;
pub(crate) fn picker_coalesce_trigger() -> usize {
    wp_conf::limits::picker_coalesce_trigger()
}

pub(crate) fn picker_coalesce_max_events() -> usize {
    wp_conf::limits::picker_coalesce_max_events()
}

// ---- Logging sample strides (to avoid log storms on hot paths) ----
// 抽样打印步长：解析通道满（parse channel full）
//pub(crate) const LOG_SAMPLE_STRIDE_PARSE_CH_FULL: u64 = 128;
// 抽样打印步长：sink 分发通道满（sink dispatcher channel full）
//pub(crate) const LOG_SAMPLE_STRIDE_SINK_CH_FULL: u64 = 64;
// 抽样打印步长：采集 fetch 完成（高频路径）
//pub(crate) const LOG_SAMPLE_STRIDE_FETCH_PENDING: u64 = 1024;
// 抽样打印步长：pending 高水位提示
//pub(crate) const LOG_SAMPLE_STRIDE_PENDING_HI: u64 = 256;
