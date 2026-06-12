use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::Local;
use orion_overload::new::New1;
use tokio::sync::mpsc::error::TrySendError;
use wp_knowledge::telemetry::{
    CacheLayer, CacheOutcome, CacheTelemetryEvent, KnowledgeTelemetry, QueryTelemetryEvent,
    ReloadOutcome, ReloadTelemetryEvent,
};
use wp_stat::{
    DataDim, ReportVariant, StatCollector, StatRecorder, StatReq, StatStage, StatTarget,
};

use crate::stat::MonSend;
use crate::stat::runtime_counters;

const KNOWLEDGE_TARGET: &str = "__knowledge";
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

struct BridgeInner {
    mon_send: Option<MonSend>,
    reload: StatCollector,
    cache: StatCollector,
    query: StatCollector,
    query_latency: StatCollector,
    last_flush: Instant,
}

pub struct KnowledgeStatsTelemetry {
    flush_interval: Duration,
    inner: Mutex<BridgeInner>,
}

impl KnowledgeStatsTelemetry {
    pub fn new() -> Self {
        Self::with_flush_interval(DEFAULT_FLUSH_INTERVAL)
    }

    fn with_flush_interval(flush_interval: Duration) -> Self {
        Self {
            flush_interval,
            inner: Mutex::new(BridgeInner {
                mon_send: None,
                reload: StatCollector::new(
                    KNOWLEDGE_TARGET.to_string(),
                    stat_req("kdb_reload", vec!["provider", "outcome"], 16),
                ),
                cache: StatCollector::new(
                    KNOWLEDGE_TARGET.to_string(),
                    stat_req("kdb_cache", vec!["layer", "provider", "outcome"], 64),
                ),
                query: StatCollector::new(
                    KNOWLEDGE_TARGET.to_string(),
                    stat_req("kdb_query", vec!["provider", "mode", "status"], 32),
                ),
                query_latency: StatCollector::new(
                    KNOWLEDGE_TARGET.to_string(),
                    stat_req(
                        "kdb_query_latency_bucket",
                        vec!["provider", "mode", "status", "latency_bucket"],
                        128,
                    ),
                ),
                last_flush: Instant::now(),
            }),
        }
    }

    pub fn attach_monitor_sender(&self, mon_send: MonSend) {
        let mut inner = self
            .inner
            .lock()
            .expect("knowledge stats bridge lock poisoned");
        inner.mon_send = Some(mon_send);
        flush_locked(&mut inner, true, self.flush_interval);
    }

    #[cfg(test)]
    fn flush_now(&self) {
        let mut inner = self
            .inner
            .lock()
            .expect("knowledge stats bridge lock poisoned");
        flush_locked(&mut inner, true, self.flush_interval);
    }
}

impl Default for KnowledgeStatsTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeTelemetry for KnowledgeStatsTelemetry {
    fn on_cache(&self, event: &CacheTelemetryEvent) {
        let mut inner = self
            .inner
            .lock()
            .expect("knowledge stats bridge lock poisoned");
        inner.cache.record_task(
            KNOWLEDGE_TARGET,
            DataDim::new((
                cache_layer_name(event.layer),
                provider_name_opt(event.provider_kind.as_ref()),
                cache_outcome_name(event.outcome),
            )),
        );
        flush_locked(&mut inner, false, self.flush_interval);
    }

    fn on_reload(&self, event: &ReloadTelemetryEvent) {
        let mut inner = self
            .inner
            .lock()
            .expect("knowledge stats bridge lock poisoned");
        inner.reload.record_task(
            KNOWLEDGE_TARGET,
            DataDim::new((
                provider_name(&event.provider_kind),
                reload_outcome_name(event.outcome),
            )),
        );
        flush_locked(&mut inner, false, self.flush_interval);
    }

    fn on_query(&self, event: &QueryTelemetryEvent) {
        let mut inner = self
            .inner
            .lock()
            .expect("knowledge stats bridge lock poisoned");
        let provider = provider_name(&event.provider_kind);
        let mode = query_mode_name(event.mode);
        let status = query_status_name(event.success);
        inner
            .query
            .record_task(KNOWLEDGE_TARGET, DataDim::new((provider, mode, status)));
        inner.query_latency.record_task(
            KNOWLEDGE_TARGET,
            data_dim4(provider, mode, status, latency_bucket(event.elapsed)),
        );
        flush_locked(&mut inner, false, self.flush_interval);
    }
}

fn stat_req(name: &str, collect: Vec<&str>, max: usize) -> StatReq {
    StatReq {
        stage: StatStage::Parse,
        name: name.to_string(),
        target: StatTarget::Item(KNOWLEDGE_TARGET.to_string()),
        collect: collect.into_iter().map(str::to_string).collect(),
        max,
    }
}

fn provider_name(kind: &wp_knowledge::loader::ProviderKind) -> &str {
    match kind {
        wp_knowledge::loader::ProviderKind::SqliteAuthority => "sqlite",
        wp_knowledge::loader::ProviderKind::Postgres => "postgres",
        wp_knowledge::loader::ProviderKind::Mysql => "mysql",
        wp_knowledge::loader::ProviderKind::Redis => "redis",
    }
}

fn provider_name_opt(kind: Option<&wp_knowledge::loader::ProviderKind>) -> &str {
    kind.map(provider_name).unwrap_or("unknown")
}

fn cache_layer_name(layer: CacheLayer) -> &'static str {
    match layer {
        CacheLayer::Local => "local",
        CacheLayer::Result => "result",
        CacheLayer::Metadata => "metadata",
    }
}

fn cache_outcome_name(outcome: CacheOutcome) -> &'static str {
    match outcome {
        CacheOutcome::Hit => "hit",
        CacheOutcome::Miss => "miss",
    }
}

fn reload_outcome_name(outcome: ReloadOutcome) -> &'static str {
    match outcome {
        ReloadOutcome::Success => "success",
        ReloadOutcome::Failure => "failure",
    }
}

fn query_mode_name(mode: wp_knowledge::runtime::QueryModeTag) -> &'static str {
    match mode {
        wp_knowledge::runtime::QueryModeTag::Many => "many",
        wp_knowledge::runtime::QueryModeTag::FirstRow => "first_row",
    }
}

fn query_status_name(success: bool) -> &'static str {
    if success { "success" } else { "failure" }
}

fn latency_bucket(elapsed: Duration) -> &'static str {
    let ms = elapsed.as_millis();
    match ms {
        0 => "lt_1ms",
        1..=4 => "1_5ms",
        5..=19 => "5_20ms",
        20..=99 => "20_100ms",
        100..=499 => "100_500ms",
        _ => "ge_500ms",
    }
}

fn data_dim4(a: &str, b: &str, c: &str, d: &str) -> DataDim {
    let mut dim = DataDim::with_capacity(4);
    dim.push(Some(a.to_string()));
    dim.push(Some(b.to_string()));
    dim.push(Some(c.to_string()));
    dim.push(Some(d.to_string()));
    dim
}

fn flush_locked(inner: &mut BridgeInner, force: bool, flush_interval: Duration) {
    if !force && inner.last_flush.elapsed() < flush_interval {
        return;
    }
    let Some(mon_send) = inner.mon_send.clone() else {
        return;
    };
    let now = Local::now().naive_local();
    for collector in [
        &mut inner.reload,
        &mut inner.cache,
        &mut inner.query,
        &mut inner.query_latency,
    ] {
        let report = collector.collect_stat_with_time(now);
        if report.get_data().is_empty() {
            continue;
        }
        match mon_send.try_send(ReportVariant::Stat(report)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => runtime_counters::rec_monitor_send_drop_full(),
            Err(TrySendError::Closed(_)) => {
                runtime_counters::rec_monitor_send_drop_closed();
                inner.mon_send = None;
                break;
            }
        }
    }
    inner.last_flush = Instant::now();
}

fn global_bridge() -> Arc<KnowledgeStatsTelemetry> {
    static BRIDGE: OnceLock<Arc<KnowledgeStatsTelemetry>> = OnceLock::new();
    BRIDGE
        .get_or_init(|| Arc::new(KnowledgeStatsTelemetry::new()))
        .clone()
}

pub fn ensure_stats_telemetry_bridge_installed() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let _ = wp_knowledge::facade::install_runtime_telemetry(global_bridge());
    });
}

pub fn attach_stats_monitor_sender(mon_send: MonSend) {
    global_bridge().attach_monitor_sender(mon_send);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_flushes_buffered_events_after_sender_attach() {
        let bridge = KnowledgeStatsTelemetry::with_flush_interval(Duration::from_secs(60));
        bridge.on_reload(&ReloadTelemetryEvent {
            outcome: ReloadOutcome::Success,
            provider_kind: wp_knowledge::loader::ProviderKind::SqliteAuthority,
        });
        bridge.on_cache(&CacheTelemetryEvent {
            layer: CacheLayer::Result,
            outcome: CacheOutcome::Miss,
            provider_kind: Some(wp_knowledge::loader::ProviderKind::SqliteAuthority),
        });

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        bridge.attach_monitor_sender(tx);

        let first = rx.try_recv().expect("reload report");
        let second = rx.try_recv().expect("cache report");
        let ReportVariant::Stat(first) = first;
        let ReportVariant::Stat(second) = second;
        assert_eq!(first.get_name(), "kdb_reload");
        assert_eq!(second.get_name(), "kdb_cache");
    }

    #[test]
    fn bridge_emits_query_and_latency_reports() {
        let bridge = KnowledgeStatsTelemetry::with_flush_interval(Duration::ZERO);
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        bridge.attach_monitor_sender(tx);

        bridge.on_query(&QueryTelemetryEvent {
            provider_kind: wp_knowledge::loader::ProviderKind::Mysql,
            mode: wp_knowledge::runtime::QueryModeTag::FirstRow,
            success: true,
            elapsed: Duration::from_millis(7),
        });
        bridge.flush_now();

        let mut names = Vec::new();
        while let Ok(ReportVariant::Stat(report)) = rx.try_recv() {
            names.push(report.get_name().to_string());
        }
        names.sort();
        assert_eq!(
            names,
            vec![
                "kdb_query".to_string(),
                "kdb_query_latency_bucket".to_string()
            ]
        );
    }
}
