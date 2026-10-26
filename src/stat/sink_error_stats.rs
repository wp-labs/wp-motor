//! 按 sink 汇总因发送失败而被丢弃的数据（`drop_when_error = true`）。
//!
//! 计数放在进程级注册表里，原因与 `runtime_counters` 一致：sink 运行时分布在多个 actor
//! 中，而监控侧只持有接收端，无法直接读取各个 `SinkRuntime`。监控每轮导出时取走窗口增量，
//! 这样统计表里既有窗口值，也能通过 `sum stat` 看到累计值；累计值与最近错误则通过
//! [`SinkErrorSnapshot`] 暴露给监控日志。

use chrono::{DateTime, Local};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// 上报的最近错误最多保留的字符数，避免把完整 HTTP 响应体写进统计/日志。
pub const MAX_ERROR_CHARS: usize = 200;

#[derive(Debug, Clone, Default)]
struct SinkErrorEntry {
    dropped_total: u64,
    dropped_window: u64,
    last_error_at: Option<DateTime<Local>>,
    last_error: Option<String>,
}

/// 单个 sink 的丢弃快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkErrorSnapshot {
    pub sink: String,
    /// 自上次 [`take_window`] 以来新增的丢弃条数
    pub dropped_window: u64,
    /// 进程启动以来该 sink 的累计丢弃条数
    pub dropped_total: u64,
    pub last_error_at: Option<DateTime<Local>>,
    pub last_error: Option<String>,
}

fn registry() -> &'static Mutex<BTreeMap<String, SinkErrorEntry>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, SinkErrorEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// 测试专用串行锁：窗口是全进程共享的，`take_window` 会取走所有 sink 的增量，
/// 任何断言窗口语义的测试都需要互斥，否则会被并行测试抽走。
#[cfg(test)]
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_registry() -> MutexGuard<'static, BTreeMap<String, SinkErrorEntry>> {
    registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn snapshot(sink: &str, entry: &SinkErrorEntry) -> SinkErrorSnapshot {
    SinkErrorSnapshot {
        sink: sink.to_string(),
        dropped_window: entry.dropped_window,
        dropped_total: entry.dropped_total,
        last_error_at: entry.last_error_at,
        last_error: entry.last_error.clone(),
    }
}

/// 记录一次真实发送失败后的丢弃，返回该 sink 的累计丢弃条数。
pub fn record_failure(sink: &str, count: u64, error: &str) -> u64 {
    let mut guard = lock_registry();
    let entry = guard.entry(sink.to_string()).or_default();
    entry.dropped_total = entry.dropped_total.saturating_add(count);
    entry.dropped_window = entry.dropped_window.saturating_add(count);
    entry.last_error_at = Some(Local::now());
    entry.last_error = Some(sanitize_error_text(error));
    entry.dropped_total
}

/// 记录一次熔断期间被跳过的丢弃（不更新最近错误），返回累计丢弃条数。
pub fn record_drop(sink: &str, count: u64) -> u64 {
    let mut guard = lock_registry();
    let entry = guard.entry(sink.to_string()).or_default();
    entry.dropped_total = entry.dropped_total.saturating_add(count);
    entry.dropped_window = entry.dropped_window.saturating_add(count);
    entry.dropped_total
}

/// 取走所有 sink 的窗口增量（累计值与最近错误保留）。
pub fn take_window() -> Vec<SinkErrorSnapshot> {
    let mut guard = lock_registry();
    let mut out = Vec::new();
    for (sink, entry) in guard.iter_mut() {
        if entry.dropped_window == 0 {
            continue;
        }
        out.push(snapshot(sink, entry));
        entry.dropped_window = 0;
    }
    out
}

/// 读取单个 sink 的累计状态，不重置窗口。
pub fn snapshot_for(sink: &str) -> Option<SinkErrorSnapshot> {
    let guard = lock_registry();
    guard.get(sink).map(|entry| snapshot(sink, entry))
}

/// 把错误文本压缩成单行并截断，便于写入日志与统计。
pub fn sanitize_error_text(raw: &str) -> String {
    let mut out = String::with_capacity(MAX_ERROR_CHARS + 3);
    let mut truncated = false;
    for (kept, ch) in raw.chars().enumerate() {
        if kept >= MAX_ERROR_CHARS {
            truncated = true;
            break;
        }
        out.push(if ch.is_control() { ' ' } else { ch });
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return "sink error".to_string();
    }
    if truncated {
        format!("{}...", trimmed)
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_take_window_keeps_total() {
        let _guard = test_lock();
        let sink = "/sink/sink_error_stats/take_window";
        let total = record_failure(sink, 3, "connection refused");
        assert_eq!(total, 3);

        let window = take_window();
        let item = window
            .iter()
            .find(|x| x.sink == sink)
            .expect("snapshot present");
        assert_eq!(item.dropped_window, 3);
        assert_eq!(item.dropped_total, 3);
        assert_eq!(item.last_error.as_deref(), Some("connection refused"));
        assert!(item.last_error_at.is_some());

        // 窗口被取走后清零，但累计值保留。
        let again = take_window();
        assert!(again.iter().all(|x| x.sink != sink));

        record_failure(sink, 1, "timeout");
        let snap = snapshot_for(sink).expect("snapshot");
        assert_eq!(snap.dropped_total, 4);
        assert_eq!(snap.last_error.as_deref(), Some("timeout"));
    }

    #[test]
    fn record_drop_counts_without_touching_last_error() {
        let _guard = test_lock();
        let sink = "/sink/sink_error_stats/skip";
        record_failure(sink, 1, "connection refused");
        let _ = take_window();

        record_drop(sink, 5);
        let snap = snapshot_for(sink).expect("snapshot");
        assert_eq!(snap.dropped_total, 6);
        assert_eq!(snap.last_error.as_deref(), Some("connection refused"));

        let window = take_window();
        let item = window
            .iter()
            .find(|x| x.sink == sink)
            .expect("snapshot present");
        assert_eq!(item.dropped_window, 5);
    }

    #[test]
    fn snapshot_for_unknown_sink_is_none() {
        assert!(snapshot_for("/sink/sink_error_stats/never_recorded").is_none());
    }

    #[test]
    fn error_text_is_single_line_and_truncated() {
        let raw = format!(
            "first line\nsecond line\t{}",
            "x".repeat(MAX_ERROR_CHARS * 2)
        );
        let text = sanitize_error_text(&raw);
        assert!(!text.contains('\n'));
        assert!(!text.contains('\t'));
        assert!(text.ends_with("..."));
        assert!(text.chars().count() <= MAX_ERROR_CHARS + 3);
    }
}
