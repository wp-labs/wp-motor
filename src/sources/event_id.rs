use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static EVENT_ID_SEED: Lazy<AtomicU64> = Lazy::new(|| {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    AtomicU64::new(seed)
});

/// 全局事件 ID 生成器：返回单调递增的 u64。
pub fn next_event_id() -> u64 {
    EVENT_ID_SEED.fetch_add(1, Ordering::Relaxed)
}
