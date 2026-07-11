use super::agent::InfraSinkAgent;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wp_conf::limits::sink_channel_cap;

use crate::resources::SinkResUnit;
use crate::sinks::SinkRuntime;
use crate::sinks::{ASinkSender, SinkDatYReceiver, SinkDatYSender, SinkPackage, SinkRecUnit};
use crate::stat::MonSend;
use crate::stat::metric_collect::MetricCollectors;
use derive_getters::Getters;
use orion_overload::append::Appendable;
use wp_conf::structure::SinkGroupConf;
use wp_connector_api::{SinkErrorOwe, SinkResult};
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::fmt_def::TextFmt;
use wp_model_core::model::{DataField, DataRecord, DataType, Value};
use wp_stat::StatReq;

// split internal helpers

mod io; // 直发/原始数据下发
mod oml; // OML/条件路由
#[cfg(any(test, feature = "perf-ci"))]
pub mod perf; // 性能基准工具
mod recovery; // 故障恢复与收尾
type GroupedRecords = HashMap<String, Vec<SinkRecUnit>>;

const DEFAULT_STREAM_TAG_FIELD: &str = "wp_stream_tag";
const WP_EVENT_ID_FIELD: &str = "wp_event_id";
const FIELD_WP_META_DISABLE: &str = "wp_meta_disable";

#[derive(Clone)]
struct OutputMetaConfig {
    emit_stream_tag: bool,
    emit_event_id: bool,
}

impl OutputMetaConfig {
    fn from_group(conf: &SinkGroupConf, disabled: &HashSet<String>) -> Self {
        let structured_text = conf.sinks().iter().any(|sink| {
            matches!(sink.fmt, TextFmt::Json | TextFmt::Csv) && !Self::is_arrow_framed(sink)
        });

        Self {
            emit_stream_tag: structured_text && !disabled.contains(DEFAULT_STREAM_TAG_FIELD),
            emit_event_id: structured_text && !disabled.contains(WP_EVENT_ID_FIELD),
        }
    }

    fn is_arrow_framed(sink: &wp_conf::structure::SinkInstanceConf) -> bool {
        sink.core
            .params
            .get("protocol")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("arrow"))
            && sink
                .core
                .params
                .get("data_format")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("arrow_framed"))
    }

    fn any_enabled(&self) -> bool {
        self.emit_stream_tag || self.emit_event_id
    }
}

struct SinkRecUnitPool {
    inner: Vec<Vec<SinkRecUnit>>,
}

impl SinkRecUnitPool {
    fn new() -> Self {
        Self { inner: Vec::new() }
    }

    fn take(&mut self) -> Vec<SinkRecUnit> {
        self.inner
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(wp_conf::limits::sink_pool_unit_init_cap()))
    }

    fn recycle(&mut self, mut vec: Vec<SinkRecUnit>) {
        let unit_max_cap = wp_conf::limits::sink_pool_unit_max_cap();
        if vec.capacity() > unit_max_cap {
            vec.shrink_to(unit_max_cap);
        }
        vec.clear();
        if self.inner.len() < wp_conf::limits::sink_pool_max() {
            self.inner.push(vec);
        }
    }
}

#[derive(Getters)]
pub struct SinkDispatcher {
    conf: SinkGroupConf,
    sinks: Vec<SinkRuntime>,
    dat_s: SinkDatYSender,
    dat_r: SinkDatYReceiver,
    res: SinkResUnit,
    unit_pool: SinkRecUnitPool,
    wp_meta_disable: HashSet<String>,
    output_meta: OutputMetaConfig,
    ingress_target: String,
    ingress_stat: MetricCollectors,
    ingress_pending: usize,
}

impl SinkDispatcher {
    pub fn new(conf: SinkGroupConf, res: SinkResUnit) -> Self {
        // 改用 tokio::mpsc 事件化通道，便于与 runtime 协作
        let (dat_s, dat_r) = tokio::sync::mpsc::channel(sink_channel_cap());
        let ingress_target = format!("{}@recv", conf.name());
        let wp_meta_disable = Self::collect_wp_meta_disable(&conf);
        let output_meta = OutputMetaConfig::from_group(&conf, &wp_meta_disable);
        Self {
            conf,
            sinks: Vec::new(),
            dat_s,
            dat_r,
            res,
            unit_pool: SinkRecUnitPool::new(),
            wp_meta_disable,
            output_meta,
            ingress_stat: MetricCollectors::new(ingress_target.clone(), Vec::new()),
            ingress_target,
            ingress_pending: 0,
        }
    }

    fn collect_wp_meta_disable(conf: &SinkGroupConf) -> HashSet<String> {
        let mut fields = HashSet::new();
        for sink in conf.sinks() {
            if let Some(items) = sink
                .core
                .params
                .get(FIELD_WP_META_DISABLE)
                .and_then(|value| value.as_array())
            {
                for item in items.iter().filter_map(|item| item.as_str()) {
                    if item.trim().is_empty() {
                        continue;
                    }
                    fields.insert(item.to_string());
                }
            }
        }
        fields
    }

    pub(super) fn apply_wp_meta_disable_to_record(&self, record: &mut DataRecord) {
        if self.wp_meta_disable.is_empty() {
            return;
        }
        for item in record.items.iter_mut() {
            if self.wp_meta_disable.contains(item.get_name()) {
                item.as_field_mut().meta = DataType::Ignore;
            }
        }
    }

    pub(super) fn apply_wp_meta_to_record(
        &self,
        event_id: u64,
        meta: &crate::sinks::ProcMeta,
        record: &mut DataRecord,
    ) {
        self.apply_wp_meta_disable_to_record(record);
        if !self.output_meta.any_enabled() {
            return;
        }
        if self.output_meta.emit_stream_tag
            && let crate::sinks::ProcMeta::Rule(rule) = meta
            && !rule.is_empty()
        {
            Self::upsert_payload_chars(record, DEFAULT_STREAM_TAG_FIELD, rule.clone());
        }
        if self.output_meta.emit_event_id {
            Self::upsert_payload_chars(record, WP_EVENT_ID_FIELD, event_id.to_string());
        }
    }

    pub(super) fn apply_wp_meta_to_arc(
        &self,
        event_id: u64,
        meta: &crate::sinks::ProcMeta,
        record: Arc<DataRecord>,
    ) -> Arc<DataRecord> {
        if self.wp_meta_disable.is_empty() && !self.output_meta.any_enabled() {
            return record;
        }
        let mut record = Arc::try_unwrap(record).unwrap_or_else(|arc| arc.as_ref().clone());
        self.apply_wp_meta_to_record(event_id, meta, &mut record);
        Arc::new(record)
    }

    pub(super) fn apply_wp_meta_to_unit(&self, unit: SinkRecUnit) -> SinkRecUnit {
        if self.wp_meta_disable.is_empty() && !self.output_meta.any_enabled() {
            return unit;
        }
        let (id, meta, record) = unit.into_parts();
        let record = self.apply_wp_meta_to_arc(id, &meta, record);
        SinkRecUnit::with_record(id, meta, record)
    }

    fn upsert_payload_chars(
        record: &mut DataRecord,
        name: &str,
        value: impl Into<wp_model_core::model::FValueStr>,
    ) {
        let value = value.into();
        if let Some(item) = record.items.iter_mut().find(|item| item.get_name() == name) {
            let field = item.as_field_mut();
            field.meta = DataType::Chars;
            field.value = Value::Chars(value);
        } else {
            record.append(DataField::from_chars(name, value));
        }
    }
    pub fn set_ingress_stat_target(
        &mut self,
        replica_idx: usize,
        replica_cnt: usize,
        stat_reqs: Vec<StatReq>,
    ) {
        self.ingress_target = if replica_cnt > 1 {
            format!("{}#{}@recv", self.conf.name(), replica_idx)
        } else {
            format!("{}@recv", self.conf.name())
        };
        self.ingress_stat = MetricCollectors::new(self.ingress_target.clone(), stat_reqs);
    }

    pub fn record_ingress_batch(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.ingress_pending = self.ingress_pending.saturating_add(count);
    }

    pub async fn send_ingress_stat(&mut self, mon_send: &MonSend) -> SinkResult<()> {
        if self.ingress_pending > 0 {
            self.ingress_stat
                .record_task_batch(self.ingress_target.as_str(), self.ingress_pending);
            self.ingress_pending = 0;
        }

        self.ingress_stat
            .send_stat(mon_send)
            .await
            .owe_sink("send ingress stat failed")
    }

    pub fn get_dat_r_mut(&mut self) -> &mut SinkDatYReceiver {
        &mut self.dat_r
    }
    pub fn get_sinks_mut(&mut self) -> &mut Vec<SinkRuntime> {
        &mut self.sinks
    }
    pub fn close_channel(&mut self) {
        self.dat_r.close();
    }
    pub fn get_name(&self) -> &str {
        self.conf.name().as_str()
    }
    pub fn freeze_all(&mut self) {
        info_data!("{} sink group freeze all", self.conf.name());
        for sink_rt in self.sinks.iter_mut() {
            sink_rt.freeze();
        }
    }
    pub fn active_all(&mut self) {
        for sink_rt in self.sinks.iter_mut() {
            sink_rt.ready();
        }
    }

    pub fn active_one(&mut self, name: &str) {
        for sink_rt in self.sinks.iter_mut() {
            if sink_rt.name == name {
                info_data!("{} sink group active one", self.conf.name());
                sink_rt.ready();
                break;
            }
        }
    }

    /// 批量处理数据包（支持批量优化）
    pub(crate) async fn group_sink_package(
        &mut self,
        package: SinkPackage,
        infra: &InfraSinkAgent,
        bad_s: &ASinkSender,
        mon: Option<&MonSend>,
        cache: &mut FieldQueryCache,
    ) -> SinkResult<usize> {
        let mut processed_count = 0;

        // 先按规则分组，同一规则共享一次 OML 批处理
        let mut records_by_rule: GroupedRecords = HashMap::new();
        for unit in package.into_iter() {
            let key = unit.meta().abstract_info();
            records_by_rule
                .entry(key)
                .or_insert_with(|| self.unit_pool.take())
                .push(unit);
            processed_count += 1;
        }

        // 批量处理同一规则下的记录
        for (_rule_str, units) in records_by_rule {
            if units.is_empty() {
                continue;
            }
            let Some(meta) = units.first().map(|unit| unit.meta().clone()) else {
                continue;
            };
            let mut per_sink_units = self
                .oml_proc_batch_async(units, infra, cache, &meta)
                .await?;
            for (idx, sink_rt) in self.sinks.iter_mut().enumerate() {
                let payload = {
                    if !sink_rt.is_ready() {
                        let unused = std::mem::take(&mut per_sink_units[idx]);
                        self.unit_pool.recycle(unused);
                        None
                    } else {
                        let units = std::mem::take(&mut per_sink_units[idx]);
                        if units.is_empty() {
                            self.unit_pool.recycle(units);
                            None
                        } else {
                            let pkg = SinkPackage::from_units(units);
                            let name_snapshot = sink_rt.name.clone();
                            sink_rt.send_package_to_sink(&pkg, Some(bad_s), mon).await?;
                            let vec_back = pkg.into_inner();
                            Some((name_snapshot, vec_back))
                        }
                    }
                };
                if let Some((name, vec_back)) = payload {
                    self.unit_pool.recycle(vec_back);
                    info_data!("sink {} send batch rec suc!", name);
                }
            }
            for leftover in per_sink_units.into_iter() {
                self.unit_pool.recycle(leftover);
            }
        }

        Ok(processed_count)
    }

    // heavy OML pipeline helpers are moved to dispatcher::oml

    // 直发与原始数据下发在 dispatcher::io

    // 恢复与收尾在 dispatcher::recovery

    pub fn get_data_sender(&self) -> SinkDatYSender {
        self.dat_s.clone()
    }
}

impl Appendable<SinkRuntime> for SinkDispatcher {
    fn append(&mut self, first: SinkRuntime) {
        self.sinks.push(first);
    }
}

// tests moved into a dedicated file for readability
#[cfg(test)]
mod tests;
