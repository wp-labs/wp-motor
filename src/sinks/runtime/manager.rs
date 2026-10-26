use crate::compat::LegacyOwe;
use crate::sinks::pdm_outer::TDMDataAble;
use crate::sinks::prelude::*;
use chrono::{DateTime, Local, Utc};
use derive_getters::Getters;
use orion_exp::{Expression, RustSymbol};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use wp_conf::structure::default_batch_size;
use wp_model_core::model::{DataField, fmt_def::TextFmt};

// 全局计数器，用于生成唯一的救援文件序号
static RESCUE_FILE_SEQ: AtomicU64 = AtomicU64::new(0);
// 运行期 sink 实例 id：同名 sink 的多副本需要区分，用于把恢复后的 sink 换回正确副本
static SINK_INSTANCE_SEQ: AtomicU64 = AtomicU64::new(0);

use crate::runtime::errors::err4_send_to_sink;
use crate::sinks::RescueFileSink;
use crate::sinks::{
    ASinkHandle, ASinkSender, ProcMeta, SinkBackendType, SinkDataEnum, SinkFFVPackage, SinkPackage,
    SinkStrPackage,
};
use crate::stat::MonSend;
use crate::stat::metric_collect::MetricCollectors;
use crate::stat::sink_error_stats;
use wp_conf::structure::SinkInstanceConf;
use wp_connector_api::{BatchMeta, SinkReason, SinkResult};
use wp_error::error_handling::{ErrorHandlingStrategy, sys_robust_mode};
use wp_model_core::raw::RawData;

use orion_error::{UnifiedReason, conversion::ErrorWith};
use wp_connector_api::SinkError;
use wp_stat::StatRecorder;
use wp_stat::StatReq;
use wp_stat::TimedStat;

use super::stat::RuntimeStautus;

/// `drop_when_error` 下两条 WARN 之间的最小间隔（避免长故障期被单个 sink 刷屏）
const DROP_WARN_INTERVAL: Duration = Duration::from_secs(60);
/// rescue 模式下救援文件状态日志的最小间隔
const RESCUE_LOG_INTERVAL: Duration = Duration::from_secs(60);
/// 熔断后的冷却起点；连续失败按 2 倍增长，直到上限。
const HEALTH_BACKOFF_BASE: Duration = Duration::from_secs(5);
const HEALTH_BACKOFF_MAX: Duration = Duration::from_secs(60);

/// sink 级健康闸门。
///
/// `drop_when_error = true` 时启用：一次发送失败后暂停实际下发（冷却 5s，连续失败翻倍，
/// 上限 60s），冷却到期只放行一个探测批次。
/// 这样坏链路期间不会每批都去撞连接/重试，好链路恢复后下一个探测批次会自动重新投递。
#[derive(Debug)]
struct SinkHealth {
    failures: u32,
    retry_at: Option<Instant>,
    backoff: Duration,
}

impl SinkHealth {
    fn new() -> Self {
        Self {
            failures: 0,
            retry_at: None,
            backoff: HEALTH_BACKOFF_BASE,
        }
    }

    fn is_open(&self, now: Instant) -> bool {
        matches!(self.retry_at, Some(at) if now < at)
    }

    fn on_failure(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        if self.failures > 1 {
            self.backoff = (self.backoff * 2).min(HEALTH_BACKOFF_MAX);
        }
        self.retry_at = Some(now + self.backoff);
    }

    /// 健康闸门与故障期记账一起复位。
    fn on_success(&mut self) {
        self.failures = 0;
        self.retry_at = None;
        self.backoff = HEALTH_BACKOFF_BASE;
    }

    #[cfg(test)]
    fn set_backoff_for_test(&mut self, base: Duration) {
        self.backoff = base;
    }
}

/// `drop_when_error` 的故障期状态：只用于限流 WARN 与恢复汇总。
#[derive(Default)]
struct OutageTracker {
    outage: bool,
    outage_started_at: Option<DateTime<Local>>,
    dropped_in_outage: u64,
    dropped_since_warn: u64,
    last_warn_at: Option<Instant>,
}

impl OutageTracker {
    /// 记录一次丢弃；返回 `Some(n)` 表示应立即输出一条 WARN，`n` 为距上次 WARN 的丢弃数。
    fn record_drop(&mut self, count: u64) -> Option<u64> {
        if !self.outage {
            self.outage = true;
            self.outage_started_at = Some(Local::now());
            self.dropped_in_outage = 0;
            self.dropped_since_warn = 0;
            self.last_warn_at = None;
        }
        self.dropped_in_outage = self.dropped_in_outage.saturating_add(count);
        self.dropped_since_warn = self.dropped_since_warn.saturating_add(count);

        let should_warn = match self.last_warn_at {
            None => true,
            Some(at) => at.elapsed() >= DROP_WARN_INTERVAL,
        };
        if should_warn {
            self.last_warn_at = Some(Instant::now());
            let since_warn = self.dropped_since_warn;
            self.dropped_since_warn = 0;
            return Some(since_warn);
        }
        None
    }

    /// 记录一次成功发送；若刚结束一段故障期，返回 `(丢弃数, 故障开始时间)`。
    fn record_success(&mut self) -> Option<(u64, DateTime<Local>)> {
        if !self.outage {
            return None;
        }
        self.outage = false;
        self.last_warn_at = None;
        self.dropped_since_warn = 0;
        let dropped = self.dropped_in_outage;
        self.dropped_in_outage = 0;
        let started_at = self.outage_started_at.take()?;
        Some((dropped, started_at))
    }
}

#[derive(Getters)]
pub struct SinkRuntime {
    pub(crate) name: String,
    //backup_name: String,
    conf: SinkInstanceConf,
    // 预编译的 tags（去重：后写覆盖），避免每条记录构造 TagSet
    pre_tags: Vec<DataField>,
    pub primary: SinkBackendType,
    rescue: String,
    cond: Option<Expression<DataField, RustSymbol>>,
    batch_size: usize,
    pending_records: Vec<Arc<DataRecord>>,
    pending_meta: BatchMeta,
    output_disabled: Vec<String>,
    status: RuntimeStautus,
    normal_stat: MetricCollectors,
    backup_stat: MetricCollectors,
    timer: TimedStat,
    backup_used: bool,
    timer_poll_ticks: u8,
    last_stat_sent_at: Instant,
    drop_when_error: bool,
    /// 运行期实例 id（同一进程内唯一，用于同名副本定位）
    #[getter(skip)]
    instance_id: u64,
    /// 故障期间暂时退役的救援 sink：切回真实 sink 后仍保持打开，下次失败直接复用同一个文件
    #[getter(skip)]
    retired_rescue: Option<SinkBackendType>,
    /// 当前救援文件路径与本次故障期间已写入的记录数（用于可观测性）
    #[getter(skip)]
    rescue_file: Option<String>,
    #[getter(skip)]
    rescue_records: u64,
    #[getter(skip)]
    last_rescue_log_at: Option<Instant>,
    #[getter(skip)]
    health: SinkHealth,
    #[getter(skip)]
    outage: OutageTracker,
}

/// 批量发送错误处理结果
enum BatchErrHandle {
    Retry,
    Consume,
    Throw,
}

impl SinkRuntime {
    pub fn new<I: Into<String> + Clone>(
        rescue: String,
        name: I,
        conf: SinkInstanceConf,
        sink: SinkBackendType,
        cond: Option<Expression<DataField, RustSymbol>>,
        stat_reqs: Vec<StatReq>,
    ) -> Self {
        Self::with_batch_size(
            rescue,
            name,
            conf,
            sink,
            cond,
            stat_reqs,
            default_batch_size(),
        )
    }

    pub fn with_batch_size<I: Into<String> + Clone>(
        rescue: String,
        name: I,
        conf: SinkInstanceConf,
        sink: SinkBackendType,
        cond: Option<Expression<DataField, RustSymbol>>,
        stat_reqs: Vec<StatReq>,
        batch_size: usize,
    ) -> Self {
        let batch_size = batch_size.max(1);
        let backup_name = format!("{}_bak", name.clone().into());
        let normal_stat = MetricCollectors::new(name.clone().into(), stat_reqs.clone());
        let backup_stat = MetricCollectors::new(backup_name.clone(), stat_reqs);
        info_ctrl!(
            "create sink:{} batch_size={} drop_when_error={}",
            conf.full_name(),
            batch_size,
            conf.drop_when_error()
        );
        let pre_tags = Self::compile_tags(&conf);
        let drop_when_error = conf.drop_when_error();

        Self {
            rescue,
            name: name.into(),
            conf,
            pre_tags,
            primary: sink,
            cond,
            batch_size,
            pending_records: Vec::with_capacity(batch_size),
            pending_meta: BatchMeta::default(),
            output_disabled: Vec::new(),
            normal_stat,
            backup_stat,
            status: RuntimeStautus::Ready,
            timer: TimedStat::new(),
            backup_used: false,
            timer_poll_ticks: 0,
            last_stat_sent_at: Instant::now(),
            drop_when_error,
            instance_id: SINK_INSTANCE_SEQ.fetch_add(1, Ordering::Relaxed),
            retired_rescue: None,
            rescue_file: None,
            rescue_records: 0,
            last_rescue_log_at: None,
            health: SinkHealth::new(),
            outage: OutageTracker::default(),
        }
    }
    // 将配置中的 tags 解析为去重后的字段列表（后写覆盖），以降低运行期构造开销
    fn compile_tags(conf: &SinkInstanceConf) -> Vec<DataField> {
        use std::collections::BTreeMap;
        let tags = conf.tags();
        if tags.is_empty() {
            return Vec::new();
        }
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for item in tags {
            if let Some((k, v)) = item.split_once(':').or_else(|| item.split_once('=')) {
                map.insert(k.trim().to_string(), v.trim().to_string());
            } else {
                map.insert(item.trim().to_string(), "true".to_string());
            }
        }
        let mut out = Vec::with_capacity(map.len());
        for (k, v) in map.into_iter() {
            out.push(DataField::from_chars(k, v));
        }
        out
    }
    pub fn freeze(&mut self) {
        self.status.freeze();
    }
    pub fn set_meta_output_disabled(&mut self, fields: &[String]) {
        self.output_disabled = fields.to_vec();
    }
    pub fn ready(&mut self) {
        self.status.ready();
    }

    pub fn get_cond(&self) -> Option<&Expression<DataField, RustSymbol>> {
        self.cond.as_ref()
    }

    /// 当前运行期实例 id，用于同名 sink 多副本的精确定位。
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    /// `drop_when_error = true` 且熔断处于打开状态时，跳过本次实际下发。
    fn should_skip_send(&self) -> bool {
        self.drop_when_error && self.health.is_open(Instant::now())
    }

    /// 熔断打开时记录整批丢弃并返回 `true`，调用方据此直接结束本次发送。
    fn drop_batch_if_open(&mut self, count: u64) -> bool {
        if !self.should_skip_send() {
            return false;
        }
        self.record_skipped_drop(count);
        true
    }

    /// 记录一次真实发送失败后的丢弃：写进程级计数（供监控导出），并按间隔限流 WARN。
    fn record_failed_drop(&mut self, count: u64, error: &SinkError) {
        if count == 0 {
            return;
        }
        let raw = error.to_string();
        let text = sink_error_stats::sanitize_error_text(&raw);
        let total = sink_error_stats::record_failure(&self.name, count, &raw);
        if let Some(since_warn) = self.outage.record_drop(count) {
            warn_data!(
                "sink '{}' drop_when_error: dropped {} record(s), total {}, last_error={}",
                self.name,
                since_warn,
                total,
                text
            );
        }
    }

    /// 记录熔断期间的跳过：不计新错误，只累计丢弃，并区分于真实失败。
    fn record_skipped_drop(&mut self, count: u64) {
        if count == 0 {
            return;
        }
        let total = sink_error_stats::record_drop(&self.name, count);
        if let Some(since_warn) = self.outage.record_drop(count) {
            let last_error = sink_error_stats::snapshot_for(&self.name)
                .and_then(|snap| snap.last_error)
                .unwrap_or_default();
            warn_data!(
                "sink '{}' drop_when_error: breaker open, skipped {} record(s), total {}, last_error={}",
                self.name,
                since_warn,
                total,
                last_error
            );
        }
    }

    /// 发送成功时调用：健康闸门复位；若刚结束一段故障期，回收救援文件并输出汇总。
    async fn record_send_success(&mut self) {
        self.health.on_success();
        if let Some(mut retired) = self.retired_rescue.take() {
            let file = self.rescue_file.take().unwrap_or_default();
            let records = self.rescue_records;
            self.rescue_records = 0;
            self.last_rescue_log_at = None;
            match retired.stop().await {
                Ok(()) => info_data!(
                    "sink '{}' rescue file finalized: path={} records={}",
                    self.name,
                    file,
                    records
                ),
                Err(e) => error_data!(
                    "sink '{}' finalize rescue file {} failed: {}",
                    self.name,
                    file,
                    e
                ),
            }
        }
        if let Some((dropped, started_at)) = self.outage.record_success() {
            info_data!(
                "sink '{}' recovered from send failures (drop_when_error): dropped {} record(s) since {}",
                self.name,
                dropped,
                started_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
    }
    pub async fn swap_backsink(&mut self) -> SinkResult<Option<SinkBackendType>> {
        // 本次故障的救援文件仍然打开：直接复用，避免每个失败批次生成一个新文件。
        if let Some(rescue) = self.retired_rescue.take() {
            debug_ctrl!("reuse rescue sink for {}", self.name);
            let old_primary = std::mem::replace(&mut self.primary, rescue);
            return Ok(Some(old_primary));
        }
        let now = Utc::now();
        let fmt_time = now.format("%Y-%m-%d_%H:%M:%S").to_string();
        // 使用全局序号确保文件名唯一性，避免同一秒内重复创建相同文件名
        let seq = RESCUE_FILE_SEQ.fetch_add(1, Ordering::SeqCst);
        let file_path = format!(
            "{}/{}-{}-{}.dat.lock",
            self.rescue, self.name, fmt_time, seq
        );
        let out_path = Path::new(&file_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SinkReason::sink("create rescue sink directory failed").with_source(e)
            })?;
        }
        info_ctrl!("crate out file use async mode {}", file_path);
        let back = RescueFileSink::new(&file_path).await?;
        self.rescue_file = Some(file_path.clone());
        self.rescue_records = 0;
        let old_primary =
            std::mem::replace(&mut self.primary, SinkBackendType::Proxy(Box::new(back)));
        Ok(Some(old_primary))
    }

    /// 救援模式下每 60s 最多一条状态日志：文件路径、已写记录数、当前大小。
    ///
    /// 救援文件在回收前是 `.lock`，`wp rescue stat` 不会统计它，长故障期间需要这条日志兜底。
    fn log_rescue_state(&mut self) {
        let should_log = match self.last_rescue_log_at {
            None => true,
            Some(at) => at.elapsed() >= RESCUE_LOG_INTERVAL,
        };
        if !should_log {
            return;
        }
        self.last_rescue_log_at = Some(Instant::now());
        let file = self.rescue_file.as_deref().unwrap_or("<unknown>");
        let size = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
        warn_data!(
            "sink '{}' on rescue: file={} records={} size={}B",
            self.name,
            file,
            self.rescue_records,
            size
        );
    }

    pub async fn send_stat(&mut self, mon_send: &MonSend) -> SinkResult<()> {
        if !self.normal_stat.has_pending_data() {
            if let Some(rec) = self.build_sink_dim_record() {
                self.normal_stat.touch_task_record(self.name.as_str(), &rec);
            } else {
                self.normal_stat.touch_task_unit(self.name.as_str());
            }
        }
        self.normal_stat
            .send_stat(mon_send)
            .await
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))
            .doing("sink stat")?;
        if self.backup_used {
            let backup_name = format!("{}_bak", self.name);
            if !self.backup_stat.has_pending_data() {
                if let Some(rec) = self.build_sink_dim_record() {
                    self.backup_stat
                        .touch_task_record(backup_name.as_str(), &rec);
                } else {
                    self.backup_stat.touch_task_unit(backup_name.as_str());
                }
            }
            self.backup_stat
                .send_stat(mon_send)
                .await
                .owe(SinkReason::Uvs(UnifiedReason::system_error()))
                .doing("back sink stat")?;
        }
        Ok(())
    }

    fn build_sink_dim_record(&self) -> Option<DataRecord> {
        let mut fields = Vec::new();
        if let Some(group) = self.conf.group_name.as_deref()
            && !group.is_empty()
        {
            fields.push(DataField::from_chars("wp_sink_group", group));
        }
        let sink_name = self.conf.name().clone();
        if !sink_name.is_empty() {
            fields.push(DataField::from_chars("wp_sink_name", sink_name));
        }
        if fields.is_empty() {
            None
        } else {
            Some(DataRecord::from(fields))
        }
    }
}
impl SinkRuntime {
    /// 发送单个数据项到 Sink（保持向后兼容）
    pub async fn send_to_sink(
        &mut self,
        event_id: u64,
        data: SinkDataEnum,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        loop {
            let mut redo = false;
            if self.should_skip_send() {
                // 坏链路期间不再调用 connector：只做记账，同组其它 sink 继续。
                debug_edata!(event_id, "sink breaker open, drop record");
                self.record_skipped_drop(1);
                self.stat_beg(&data);
                self.stat_end(&data);
                break;
            }
            self.stat_beg(&data);
            // 避免不必要的数据克隆，改为按引用下发
            let result = match &data {
                SinkDataEnum::Rec(_rule, dat) => self.primary.sink_record(dat).await,
                SinkDataEnum::FFV(dat) => {
                    let raw = TextFmt::Raw
                        .gen_data(dat.clone())
                        .with_context("ffv")
                        .doing("render raw payload")?;
                    match raw {
                        RawData::String(line) => self.primary.sink_str(&line).await,
                        RawData::Bytes(bytes) => self.primary.sink_bytes(&bytes).await,
                        RawData::ArcBytes(bytes) => self.primary.sink_bytes(&bytes).await,
                    }
                }
                SinkDataEnum::Raw(dat) => self.primary.sink_str(dat).await,
            };

            //写入数据出错, 原因: sink 断连. 或 sink 失效. 处理的方案,只有重连.
            if let Err(e) = result {
                if self.drop_when_error {
                    // 丢弃模式径直绕过 wp-motor 层的救援机制：不写救援文件、不切换坏 sink；
                    // connector 自身的重试（params.max_retries）仍然生效。
                    debug_edata!(event_id, "sink error dropped: {}", e);
                    self.health.on_failure(Instant::now());
                    self.record_failed_drop(1, &e);
                    // 丢弃也视为已消费，避免在统计里留下永远不结束的 backlog。
                    self.stat_end(&data);
                } else {
                    match err4_send_to_sink(&e, &sys_robust_mode()) {
                        ErrorHandlingStrategy::FixRetry => {
                            if self.backup_used {
                                // 已经在救援态（primary 就是救援文件）仍写失败，通常是磁盘问题；
                                // 再换一次只会把救援 sink 当成坏 sink 吐回去，直接报错。
                                error_data!(
                                    "sink '{}' is already on rescue file but still failed: {}",
                                    self.name,
                                    e
                                );
                                return Err(e);
                            }
                            if let Some(bad_sink_send) = bad_s {
                                self.use_back_sink(bad_sink_send, mon).await?;
                                if !redo {
                                    redo = true;
                                }
                            }
                        }
                        ErrorHandlingStrategy::Throw => {
                            warn_data!("sink error and interrupt");
                            return Err(e);
                        }
                        ErrorHandlingStrategy::Tolerant => {
                            debug_edata!(event_id, "sink error and tolerant: {}", e);
                            //pass;
                        }
                        ErrorHandlingStrategy::Ignore => {
                            debug_edata!(event_id, "sink error and ignore: {}", e);
                        }
                        ErrorHandlingStrategy::Terminate => {
                            info_edata!(event_id, "sink error and end: {}", e);
                            break;
                        }
                    }
                }
            } else {
                if self.backup_used {
                    self.rescue_records = self.rescue_records.saturating_add(1);
                    self.log_rescue_state();
                }
                self.record_send_success().await;
                self.stat_end(&data);
                debug_edata!(event_id, "sink {} send suc!", self.name);
            }
            if !redo {
                break;
            }
        }
        if let Some(mon_send) = mon {
            self.send_stat(mon_send).await?;
        }
        Ok(())
    }

    /// 刷新 pending 缓冲中的记录并发送到 Sink
    async fn flush_pending_buffer(
        &mut self,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        if self.pending_records.is_empty() {
            return Ok(());
        }

        // 提取 buffer 内容，并为下一轮写入保留容量，避免频繁扩容
        let records = std::mem::replace(
            &mut self.pending_records,
            Vec::with_capacity(self.batch_size),
        );
        let meta = std::mem::take(&mut self.pending_meta);
        self.send_records_batch(records, meta, bad_s, mon, true)
            .await
    }

    /// 发送一批 records；`requeue_on_throw=true` 时在 Throw 分支回填 pending 缓冲
    async fn send_records_batch(
        &mut self,
        records: Vec<Arc<DataRecord>>,
        meta: BatchMeta,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
        requeue_on_throw: bool,
    ) -> SinkResult<()> {
        if records.is_empty() {
            return Ok(());
        }

        if self.drop_batch_if_open(records.len() as u64) {
            self.stat_beg_records_batch(&records);
            self.stat_end_records_batch(&records);
            return Ok(());
        }

        // 统计开始
        self.stat_beg_records_batch(&records);

        loop {
            let result = if meta.is_empty() {
                self.primary.sink_records(records.clone()).await
            } else {
                self.primary
                    .sink_records_with_meta(meta.clone(), records.clone())
                    .await
            };
            match result {
                Ok(()) => {
                    // 统计结束
                    self.stat_end_records_batch(&records);
                    if self.backup_used {
                        self.rescue_records =
                            self.rescue_records.saturating_add(records.len() as u64);
                        self.log_rescue_state();
                    }
                    self.record_send_success().await;
                    return Ok(());
                }
                Err(e) => {
                    if !self.drop_when_error {
                        for e_id in 0..records.len() as u64 {
                            error_edata!(e_id, "flush sink data failed: {}", e);
                        }
                    }
                    match self
                        .handle_send_error(&e, records.len(), bad_s, mon)
                        .await?
                    {
                        BatchErrHandle::Retry => continue,
                        BatchErrHandle::Consume => {
                            self.stat_end_records_batch(&records);
                            return Ok(());
                        }
                        BatchErrHandle::Throw => {
                            if requeue_on_throw {
                                // 失败时将数据放回 buffer
                                let pending_copy = records.clone();
                                self.pending_records = records;
                                self.pending_meta = meta;
                                self.stat_end_records_batch(&pending_copy);
                            } else {
                                self.stat_end_records_batch(&records);
                            }
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    async fn send_record_segment(
        &mut self,
        records: Vec<Arc<DataRecord>>,
        meta: BatchMeta,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        if records.is_empty() {
            return Ok(());
        }

        if !self.pending_records.is_empty() && self.pending_meta != meta {
            self.flush_pending_buffer(bad_s, mon).await?;
        }

        // 自动策略：当 pending 为空且入站段已达到阈值，直接下发可减少无效缓冲开销。
        if self.pending_records.is_empty() && records.len() >= self.batch_size {
            return self
                .send_records_batch(records, meta, bad_s, mon, false)
                .await;
        }

        if self.pending_records.is_empty() {
            self.pending_meta = meta;
        }

        self.pending_records.extend(records);
        if self.pending_records.len() >= self.batch_size {
            self.flush_pending_buffer(bad_s, mon).await?;
        }
        Ok(())
    }

    /// 批量发送记录数据包到 Sink
    pub async fn send_package_to_sink(
        &mut self,
        package: &SinkPackage,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        if package.is_empty() {
            return Ok(());
        }

        let mut records = Vec::with_capacity(package.len());
        let mut segment_meta: Option<BatchMeta> = None;
        for (idx, unit) in package.iter().enumerate() {
            let unit_meta = self.unit_batch_meta(unit.meta());
            if let Some(meta) = segment_meta.as_ref() {
                if meta != &unit_meta {
                    let meta = segment_meta.unwrap_or_default();
                    return self
                        .send_mixed_meta_package_by_segment_from(
                            package, idx, records, meta, bad_s, mon,
                        )
                        .await;
                }
            } else {
                segment_meta = Some(unit_meta);
            }
            records.push(unit.data().clone());
        }

        let meta = segment_meta.unwrap_or_default();
        self.send_record_segment(records, meta, bad_s, mon).await
    }

    async fn send_mixed_meta_package_by_segment_from(
        &mut self,
        package: &SinkPackage,
        start_idx: usize,
        mut segment_records: Vec<Arc<DataRecord>>,
        mut segment_meta: BatchMeta,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        for unit in package.iter().skip(start_idx) {
            let unit_meta = self.unit_batch_meta(unit.meta());
            if segment_meta != unit_meta {
                let records = std::mem::take(&mut segment_records);
                self.send_record_segment(records, segment_meta, bad_s, mon)
                    .await?;
                segment_meta = unit_meta;
            }
            segment_records.push(unit.data().clone());
        }

        self.send_record_segment(segment_records, segment_meta, bad_s, mon)
            .await
    }

    fn unit_batch_meta(&self, meta: &ProcMeta) -> BatchMeta {
        let mut batch_meta = BatchMeta::default();
        if let ProcMeta::OmlName(name) = meta
            && !name.is_empty()
        {
            batch_meta.oml_name = Some(name.clone());
        }
        batch_meta.set_output_disabled(self.output_disabled.iter().cloned());
        batch_meta
    }

    /// 公开的 flush 方法，用于手动触发 buffer 刷新
    pub async fn flush(
        &mut self,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        self.flush_pending_buffer(bad_s, mon).await
    }

    /// 批量发送 FFV 数据包到 Sink
    pub async fn send_ffv_package_to_sink(
        &mut self,
        package: SinkFFVPackage,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        if package.is_empty() {
            return Ok(());
        }

        if self.drop_batch_if_open(package.len() as u64) {
            self.record_package_stats_begin_ffv(&package);
            self.record_package_stats_end_ffv(&package);
            return Ok(());
        }

        self.record_package_stats_begin_ffv(&package);
        loop {
            let mut raw_strings = Vec::new();
            let mut raw_bytes = Vec::new();

            for unit in package.iter() {
                let raw = TextFmt::Raw
                    .gen_data(unit.data().clone())
                    .with_context("ffv_batch")
                    .doing("render raw payload")
                    .unwrap_or_else(|_| RawData::String("".to_string()));
                match raw {
                    RawData::String(s) => raw_strings.push(s),
                    RawData::Bytes(b) => raw_bytes.push(b.to_vec()),
                    RawData::ArcBytes(b) => raw_bytes.push(b.to_vec()),
                }
            }

            let result = if !raw_strings.is_empty() {
                let refs: Vec<&str> = raw_strings.iter().map(|s| s.as_str()).collect();
                self.primary.sink_str_batch(refs).await
            } else if !raw_bytes.is_empty() {
                let refs: Vec<&[u8]> = raw_bytes.iter().map(|b| b.as_ref()).collect();
                self.primary.sink_bytes_batch(refs).await
            } else {
                Ok(())
            };

            match result {
                Ok(()) => {
                    self.record_package_stats_end_ffv(&package);
                    if self.backup_used {
                        self.rescue_records =
                            self.rescue_records.saturating_add(package.len() as u64);
                        self.log_rescue_state();
                    }
                    self.record_send_success().await;
                    return Ok(());
                }
                Err(e) => match self
                    .handle_send_error(&e, package.len(), bad_s, mon)
                    .await?
                {
                    BatchErrHandle::Retry => continue,
                    BatchErrHandle::Consume => {
                        self.record_package_stats_end_ffv(&package);
                        return Ok(());
                    }
                    BatchErrHandle::Throw => {
                        self.record_package_stats_end_ffv(&package);
                        return Err(e);
                    }
                },
            }
        }
    }

    /// 批量发送字符串数据包到 Sink
    pub async fn send_str_package_to_sink(
        &mut self,
        package: SinkStrPackage,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        if package.is_empty() {
            return Ok(());
        }

        if self.drop_batch_if_open(package.len() as u64) {
            self.record_package_stats_begin_str(&package);
            self.record_package_stats_end_str(&package);
            return Ok(());
        }

        self.record_package_stats_begin_str(&package);
        loop {
            let raw_strings: Vec<&str> = package.iter().map(|unit| unit.data().as_str()).collect();
            let result = self.primary.sink_str_batch(raw_strings).await;

            match result {
                Ok(()) => {
                    self.record_package_stats_end_str(&package);
                    if self.backup_used {
                        self.rescue_records =
                            self.rescue_records.saturating_add(package.len() as u64);
                        self.log_rescue_state();
                    }
                    self.record_send_success().await;
                    return Ok(());
                }
                Err(e) => match self
                    .handle_send_error(&e, package.len(), bad_s, mon)
                    .await?
                {
                    BatchErrHandle::Retry => continue,
                    BatchErrHandle::Consume => {
                        self.record_package_stats_end_str(&package);
                        return Ok(());
                    }
                    BatchErrHandle::Throw => {
                        self.record_package_stats_end_str(&package);
                        return Err(e);
                    }
                },
            }
        }
    }

    /// 记录 FFV 包的统计开始信息
    fn record_package_stats_begin_ffv(&mut self, package: &SinkFFVPackage) {
        if self.normal_stat.supports_unit_batch()
            && (!self.backup_used || self.backup_stat.supports_unit_batch())
        {
            self.stat_beg_unit_batch(package.len());
            return;
        }
        for unit in package {
            self.stat_beg(&SinkDataEnum::FFV(unit.data().clone()));
        }
    }

    /// 记录字符串包的统计开始信息
    fn record_package_stats_begin_str(&mut self, package: &SinkStrPackage) {
        if self.normal_stat.supports_unit_batch()
            && (!self.backup_used || self.backup_stat.supports_unit_batch())
        {
            self.stat_beg_unit_batch(package.len());
            return;
        }
        for unit in package {
            self.stat_beg(&SinkDataEnum::Raw(unit.data().clone()));
        }
    }

    /// 记录 FFV 包的统计结束信息
    fn record_package_stats_end_ffv(&mut self, package: &SinkFFVPackage) {
        if self.normal_stat.supports_unit_batch()
            && (!self.backup_used || self.backup_stat.supports_unit_batch())
        {
            self.stat_end_unit_batch(package.len());
            return;
        }
        for unit in package {
            self.stat_end(&SinkDataEnum::FFV(unit.data().clone()));
        }
    }

    /// 记录字符串包的统计结束信息
    fn record_package_stats_end_str(&mut self, package: &SinkStrPackage) {
        if self.normal_stat.supports_unit_batch()
            && (!self.backup_used || self.backup_stat.supports_unit_batch())
        {
            self.stat_end_unit_batch(package.len());
            return;
        }
        for unit in package {
            self.stat_end(&SinkDataEnum::Raw(unit.data().clone()));
        }
    }

    fn stat_beg_unit_batch(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.normal_stat
            .record_begin_batch_unit(self.name.as_str(), count);
        if self.backup_used {
            self.backup_stat
                .record_begin_batch_unit(self.name.as_str(), count);
        }
    }

    fn stat_end_unit_batch(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        if self.backup_used {
            self.backup_stat
                .record_end_batch_unit(self.name.as_str(), count);
        } else {
            self.normal_stat
                .record_end_batch_unit(self.name.as_str(), count);
        }
    }

    fn stat_beg_records_batch(&mut self, records: &[Arc<DataRecord>]) {
        if self.normal_stat.supports_unit_batch()
            && (!self.backup_used || self.backup_stat.supports_unit_batch())
        {
            self.stat_beg_unit_batch(records.len());
            return;
        }
        let sink_dim_record = self.build_sink_dim_record();
        for record in records {
            let stat_record = sink_dim_record.as_ref().unwrap_or(record.as_ref());
            self.normal_stat
                .record_begin(self.name.as_str(), Some(stat_record));
            if self.backup_used {
                self.backup_stat
                    .record_begin(self.name.as_str(), Some(stat_record));
            }
        }
    }

    fn stat_end_records_batch(&mut self, records: &[Arc<DataRecord>]) {
        if self.normal_stat.supports_unit_batch()
            && (!self.backup_used || self.backup_stat.supports_unit_batch())
        {
            self.stat_end_unit_batch(records.len());
            return;
        }
        let sink_dim_record = self.build_sink_dim_record();
        if self.backup_used {
            for record in records {
                let stat_record = sink_dim_record.as_ref().unwrap_or(record.as_ref());
                self.backup_stat
                    .record_end(self.name.as_str(), Some(stat_record));
            }
        } else {
            for record in records {
                let stat_record = sink_dim_record.as_ref().unwrap_or(record.as_ref());
                self.normal_stat
                    .record_end(self.name.as_str(), Some(stat_record));
            }
        }
    }

    /// 处理发送错误；`dropped` 为本批被丢弃的记录数（`drop_when_error` 下写入计数）。
    async fn handle_send_error(
        &mut self,
        error: &SinkError,
        dropped: usize,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<BatchErrHandle> {
        if self.drop_when_error {
            // 丢弃整批：不写救援文件、不切换坏 sink，也不回填 pending 缓冲。
            debug_data!(
                "sink '{}' drop_when_error: drop {} record(s) in failed batch",
                self.name,
                dropped
            );
            self.health.on_failure(Instant::now());
            self.record_failed_drop(dropped as u64, error);
            return Ok(BatchErrHandle::Consume);
        }
        match err4_send_to_sink(error, &sys_robust_mode()) {
            ErrorHandlingStrategy::FixRetry => {
                if self.backup_used {
                    error_data!(
                        "sink '{}' is already on rescue file but still failed: {}",
                        self.name,
                        error
                    );
                    return Ok(BatchErrHandle::Throw);
                }
                if let Some(bad_sink_send) = bad_s {
                    self.use_back_sink(bad_sink_send, mon).await?;
                    return Ok(BatchErrHandle::Retry);
                }
                Ok(BatchErrHandle::Throw)
            }
            ErrorHandlingStrategy::Throw => Ok(BatchErrHandle::Throw),
            ErrorHandlingStrategy::Tolerant
            | ErrorHandlingStrategy::Ignore
            | ErrorHandlingStrategy::Terminate => Ok(BatchErrHandle::Consume),
        }
    }

    fn stat_end(&mut self, data: &SinkDataEnum) {
        match &data {
            SinkDataEnum::Rec(_, dat) => {
                let sink_dim_record = self.build_sink_dim_record();
                let stat_record = sink_dim_record.as_ref().unwrap_or(dat.as_ref());
                if self.backup_used {
                    self.backup_stat
                        .record_end(self.name.as_str(), Some(stat_record));
                } else {
                    self.normal_stat
                        .record_end(self.name.as_str(), Some(stat_record));
                }
            }
            SinkDataEnum::FFV(_) => {
                if self.backup_used {
                    self.backup_stat.record_end(self.name.as_str(), ());
                } else {
                    self.normal_stat.record_end(self.name.as_str(), ());
                }
            }
            SinkDataEnum::Raw(_) => {
                if self.backup_used {
                    self.backup_stat.record_end(self.name.as_str(), ());
                } else {
                    self.normal_stat.record_end(self.name.as_str(), ());
                }
            }
        };
    }

    fn stat_beg(&mut self, data: &SinkDataEnum) {
        match &data {
            SinkDataEnum::Rec(_, dat) => {
                let sink_dim_record = self.build_sink_dim_record();
                let stat_record = sink_dim_record.as_ref().unwrap_or(dat.as_ref());
                self.normal_stat
                    .record_begin(self.name.as_str(), Some(stat_record));
                if self.backup_used {
                    self.backup_stat
                        .record_begin(self.name.as_str(), Some(stat_record));
                }
            }
            SinkDataEnum::FFV(_) => {
                self.normal_stat.record_begin(self.name.as_str(), ());
                if self.backup_used {
                    self.backup_stat.record_begin(self.name.as_str(), ());
                }
            }
            SinkDataEnum::Raw(_) => {
                self.normal_stat.record_begin(self.name.as_str(), ());
                if self.backup_used {
                    self.backup_stat.record_begin(self.name.as_str(), ());
                }
            }
        };
    }

    pub fn is_ready(&self) -> bool {
        self.status.is_ready()
    }

    /// 当前运行期是否对应这个待恢复 handle（同名副本也能准确定位）。
    pub fn accepts_handle(&self, sink_h: &ASinkHandle) -> bool {
        self.name == sink_h.name && self.instance_id == sink_h.instance_id
    }

    /// 是否处于救援态（primary 当前是救援 sink）。
    pub fn is_backup_active(&self) -> bool {
        self.backup_used
    }

    async fn use_back_sink(
        &mut self,
        bad_sink_send: &ASinkSender,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        match self.swap_backsink().await {
            Ok(Some(old_primary)) => {
                self.backup_used = true;
                if let Some(mon) = mon {
                    self.send_stat(mon).await?;
                }
                if let Err(e) = bad_sink_send
                    .send(ASinkHandle::new(
                        self.name.clone(),
                        self.instance_id,
                        old_primary,
                    ))
                    .await
                {
                    warn_data!("Failed to enqueue bad sink for {}: {}", self.name, e);
                }
            }
            Ok(None) => {
                warn_data!("swap_back returned None for sink {}", self.name);
            }
            Err(err) => return Err(err),
        }
        Ok(())
    }
    pub async fn recover_sink(&mut self, sink_h: ASinkHandle, mon: &MonSend) -> SinkResult<bool> {
        // 只在救援态接受恢复：已切回的陈旧 handle 不能把当前真实 sink 换出去。
        if !self.is_backup_active() || !self.accepts_handle(&sink_h) {
            return Ok(false);
        }
        let retired = std::mem::replace(&mut self.primary, sink_h.sink);
        // 救援 sink 不急着关闭：本次故障内再次失败时直接换回来，保证一个 outage 只有一个救援文件。
        if let Some(mut previous) = self.retired_rescue.replace(retired) {
            previous.stop().await?;
        }
        self.send_stat(mon).await?;
        self.backup_used = false;
        self.health.on_success();
        debug_ctrl!(
            "sink '{}' recovered; rescue sink kept open for reuse",
            self.name
        );
        Ok(true)
    }

    /// 关闭所有后端（包括已退役的救援 sink），用进程退出时收尾。
    pub async fn stop_backends(&mut self) -> SinkResult<()> {
        let mut first_err = None;
        if let Some(mut retired) = self.retired_rescue.take() {
            self.rescue_file = None;
            self.rescue_records = 0;
            if let Err(e) = retired.stop().await {
                first_err = Some(e);
            }
        }
        if let Err(e) = self.primary.stop().await
            && first_err.is_none()
        {
            first_err = Some(e);
        }
        // primary 可能本身就是本次故障中的救援 sink，收尾后同样清空状态。
        self.rescue_file = None;
        self.rescue_records = 0;
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sinks::ProcMeta;
    use crate::sinks::SinkRecUnit;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;
    use wp_model_core::model::{DataField, DataRecord};

    struct FailingSink;

    /// 前 `fail_first` 次发送失败，之后成功；`attempts` 记录真实的 sink 调用次数。
    struct FlakySink {
        attempts: Arc<AtomicUsize>,
        fail_first: usize,
    }

    impl FlakySink {
        fn new(attempts: Arc<AtomicUsize>, fail_first: usize) -> Self {
            Self {
                attempts,
                fail_first,
            }
        }

        fn attempt(&self) -> SinkResult<()> {
            let idx = self.attempts.fetch_add(1, Ordering::SeqCst);
            if idx < self.fail_first {
                Err(SinkError::from(SinkReason::StgCtrl))
            } else {
                Ok(())
            }
        }
    }

    fn rec_packet(rule: &str) -> SinkDataEnum {
        let mut record = DataRecord::default();
        record.append(DataField::from_chars("k", "v"));
        SinkDataEnum::Rec(ProcMeta::WplName(rule.to_string()), Arc::new(record))
    }

    struct CountingSink {
        sink_records_calls: Arc<AtomicUsize>,
    }

    struct CapturingMetaSink {
        sink_records_calls: Arc<AtomicUsize>,
        metas: Arc<Mutex<Vec<BatchMeta>>>,
        batch_lens: Arc<Mutex<Vec<usize>>>,
    }

    impl CountingSink {
        fn new(sink_records_calls: Arc<AtomicUsize>) -> Self {
            Self { sink_records_calls }
        }
    }

    impl CapturingMetaSink {
        fn new(
            sink_records_calls: Arc<AtomicUsize>,
            metas: Arc<Mutex<Vec<BatchMeta>>>,
            batch_lens: Arc<Mutex<Vec<usize>>>,
        ) -> Self {
            Self {
                sink_records_calls,
                metas,
                batch_lens,
            }
        }
    }

    #[async_trait]
    impl AsyncCtrl for FailingSink {
        async fn stop(&mut self) -> SinkResult<()> {
            Ok(())
        }

        async fn reconnect(&mut self) -> SinkResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncCtrl for FlakySink {
        async fn stop(&mut self) -> SinkResult<()> {
            Ok(())
        }

        async fn reconnect(&mut self) -> SinkResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncRecordSink for FlakySink {
        async fn sink_record(&mut self, _data: &DataRecord) -> SinkResult<()> {
            self.attempt()
        }

        async fn sink_records(&mut self, _data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
            self.attempt()
        }
    }

    #[async_trait]
    impl AsyncRawdatSink for FlakySink {
        async fn sink_str(&mut self, _data: &str) -> SinkResult<()> {
            self.attempt()
        }

        async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
            self.attempt()
        }

        async fn sink_str_batch(&mut self, _data: Vec<&str>) -> SinkResult<()> {
            self.attempt()
        }

        async fn sink_bytes_batch(&mut self, _data: Vec<&[u8]>) -> SinkResult<()> {
            self.attempt()
        }
    }

    #[async_trait]
    impl AsyncRecordSink for FailingSink {
        async fn sink_record(&mut self, _data: &DataRecord) -> SinkResult<()> {
            Err(SinkError::from(SinkReason::StgCtrl))
        }

        async fn sink_records(&mut self, _data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
            Err(SinkError::from(SinkReason::StgCtrl))
        }
    }

    #[async_trait]
    impl AsyncRawdatSink for FailingSink {
        async fn sink_str(&mut self, _data: &str) -> SinkResult<()> {
            Err(SinkError::from(SinkReason::StgCtrl))
        }

        async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
            Err(SinkError::from(SinkReason::StgCtrl))
        }

        async fn sink_str_batch(&mut self, _data: Vec<&str>) -> SinkResult<()> {
            Err(SinkError::from(SinkReason::StgCtrl))
        }

        async fn sink_bytes_batch(&mut self, _data: Vec<&[u8]>) -> SinkResult<()> {
            Err(SinkError::from(SinkReason::StgCtrl))
        }
    }

    #[async_trait]
    impl AsyncCtrl for CountingSink {
        async fn stop(&mut self) -> SinkResult<()> {
            Ok(())
        }

        async fn reconnect(&mut self) -> SinkResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncRecordSink for CountingSink {
        async fn sink_record(&mut self, _data: &DataRecord) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_records(&mut self, _data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
            self.sink_records_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncRawdatSink for CountingSink {
        async fn sink_str(&mut self, _data: &str) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_str_batch(&mut self, _data: Vec<&str>) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_bytes_batch(&mut self, _data: Vec<&[u8]>) -> SinkResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncCtrl for CapturingMetaSink {
        async fn stop(&mut self) -> SinkResult<()> {
            Ok(())
        }

        async fn reconnect(&mut self) -> SinkResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncRecordSink for CapturingMetaSink {
        async fn sink_record(&mut self, _data: &DataRecord) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_records(&mut self, data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
            self.sink_records_calls.fetch_add(1, Ordering::SeqCst);
            self.batch_lens
                .lock()
                .expect("batch_lens lock")
                .push(data.len());
            Ok(())
        }

        async fn sink_records_with_meta(
            &mut self,
            meta: BatchMeta,
            data: Vec<Arc<DataRecord>>,
        ) -> SinkResult<()> {
            self.metas.lock().expect("metas lock").push(meta);
            self.batch_lens
                .lock()
                .expect("batch_lens lock")
                .push(data.len());
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncRawdatSink for CapturingMetaSink {
        async fn sink_str(&mut self, _data: &str) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_str_batch(&mut self, _data: Vec<&str>) -> SinkResult<()> {
            Ok(())
        }

        async fn sink_bytes_batch(&mut self, _data: Vec<&[u8]>) -> SinkResult<()> {
            Ok(())
        }
    }

    fn build_package(count: usize) -> SinkPackage {
        let units = (0..count).map(|idx| {
            let mut record = DataRecord::default();
            record.append(DataField::from_chars("k", format!("v{}", idx)));
            SinkRecUnit::new(
                idx as u64,
                ProcMeta::WplName("/bench/rule".to_string()),
                Arc::new(record),
            )
        });
        SinkPackage::from_units(units)
    }

    fn build_oml_package(count: usize, name: &str) -> SinkPackage {
        let units = (0..count).map(|idx| {
            let mut record = DataRecord::default();
            record.append(DataField::from_chars("k", format!("v{}", idx)));
            SinkRecUnit::new(
                idx as u64,
                ProcMeta::OmlName(name.to_string()),
                Arc::new(record),
            )
        });
        SinkPackage::from_units(units)
    }

    fn build_mixed_oml_package(names: &[&str]) -> SinkPackage {
        let units = names.iter().enumerate().map(|(idx, name)| {
            let mut record = DataRecord::default();
            record.append(DataField::from_chars("k", format!("v{}", idx)));
            SinkRecUnit::new(
                idx as u64,
                ProcMeta::OmlName((*name).to_string()),
                Arc::new(record),
            )
        });
        SinkPackage::from_units(units)
    }

    struct CapturingRuntime {
        runtime: SinkRuntime,
        plain_calls: Arc<AtomicUsize>,
        metas: Arc<Mutex<Vec<BatchMeta>>>,
        batch_lens: Arc<Mutex<Vec<usize>>>,
    }

    fn capturing_runtime(batch_size: usize) -> CapturingRuntime {
        let plain_calls = Arc::new(AtomicUsize::new(0));
        let metas = Arc::new(Mutex::new(Vec::new()));
        let batch_lens = Arc::new(Mutex::new(Vec::new()));
        let primary = SinkBackendType::Proxy(Box::new(CapturingMetaSink::new(
            plain_calls.clone(),
            metas.clone(),
            batch_lens.clone(),
        )));
        let conf = SinkInstanceConf::new_type(
            "bench".into(),
            TextFmt::Json,
            "blackhole".into(),
            Default::default(),
            None,
        );
        let runtime = SinkRuntime::with_batch_size(
            "./rescue".to_string(),
            "/sink/bench/[0]",
            conf,
            primary,
            None,
            Vec::new(),
            batch_size,
        );
        CapturingRuntime {
            runtime,
            plain_calls,
            metas,
            batch_lens,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn swap_back_routes_records_to_rescue_file() -> SinkResult<()> {
        let temp = tempdir().owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;

        let mut params = wp_connector_api::ParamMap::new();
        params.insert(
            "path".into(),
            serde_json::Value::String(rescue_root.join("dummy.dat").display().to_string()),
        );

        let conf = SinkInstanceConf::new_type(
            "benchmark".into(),
            TextFmt::Json,
            "file".into(),
            params,
            None,
        );

        let sink_name = "/sink/benchmark/[0]";
        let rescue_dir = rescue_root.display().to_string();
        let primary = SinkBackendType::Proxy(Box::new(FailingSink));
        let (bad_tx, mut bad_rx) = tokio::sync::mpsc::channel(1);

        {
            let mut runtime =
                SinkRuntime::new(rescue_dir, sink_name, conf, primary, None, Vec::new());

            let mut record = DataRecord::default();
            record.append(DataField::from_chars("k", "v"));
            let packet = SinkDataEnum::Rec(
                ProcMeta::WplName("/shh/test_rule16".into()),
                Arc::new(record),
            );

            runtime
                .send_to_sink(1, packet, Some(&bad_tx), None)
                .await
                .expect("send_to_sink should succeed after swap");

            let handle = bad_rx.recv().await.expect("bad sink handle");
            assert_eq!(handle.name, sink_name);
        }

        let benchmark_rescue = rescue_root.join("sink").join("benchmark");
        let entries = std::fs::read_dir(&benchmark_rescue)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?
            .collect::<Result<Vec<_>, _>>()
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        assert!(!entries.is_empty(), "expect rescue file created");
        let meta = std::fs::metadata(entries[0].path())
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        assert!(meta.len() > 0, "rescue file should contain payload");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn drop_when_error_single_failure_drops_without_rescue_or_swap() -> SinkResult<()> {
        let temp = tempdir().owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let sink_name = "drop_single";

        let mut conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        conf.set_drop_when_error(true);
        let (bad_tx, mut bad_rx) = tokio::sync::mpsc::channel(1);

        let mut runtime = SinkRuntime::new(
            rescue_root.display().to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FailingSink)),
            None,
            Vec::new(),
        );

        let mut record = DataRecord::default();
        record.append(DataField::from_chars("k", "v"));
        let packet =
            SinkDataEnum::Rec(ProcMeta::WplName("/shh/drop_rule".into()), Arc::new(record));

        runtime
            .send_to_sink(1, packet, Some(&bad_tx), None)
            .await
            .expect("drop_when_error sink failure must be swallowed");

        assert!(
            bad_rx.try_recv().is_err(),
            "drop_when_error must not hand the sink to the bad-sink channel"
        );
        assert_eq!(
            std::fs::read_dir(&rescue_root)
                .owe(SinkReason::Uvs(UnifiedReason::system_error()))?
                .count(),
            0,
            "drop_when_error must not create rescue files"
        );

        let snap = sink_error_stats::snapshot_for(sink_name).expect("drop counter recorded");
        assert_eq!(snap.dropped_total, 1);
        assert!(snap.last_error.is_some());
        assert!(snap.last_error_at.is_some());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn drop_when_error_batch_failure_drops_the_whole_batch() -> SinkResult<()> {
        let temp = tempdir().owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let sink_name = "drop_batch";
        let attempts = Arc::new(AtomicUsize::new(0));

        let mut conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        conf.set_drop_when_error(true);
        let batch_size = 4;
        let mut runtime = SinkRuntime::with_batch_size(
            rescue_root.display().to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FlakySink::new(attempts.clone(), usize::MAX))),
            None,
            Vec::new(),
            batch_size,
        );

        runtime
            .send_package_to_sink(&build_package(batch_size), None, None)
            .await
            .expect("drop_when_error batch failure must be swallowed");
        // 熔断已打开：第二批不再调用 connector。
        runtime
            .send_package_to_sink(&build_package(batch_size), None, None)
            .await
            .expect("breaker open, batch skipped");

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read_dir(&rescue_root)
                .owe(SinkReason::Uvs(UnifiedReason::system_error()))?
                .count(),
            0,
            "drop_when_error must not create rescue files for batches"
        );
        let snap = sink_error_stats::snapshot_for(sink_name).expect("drop counter recorded");
        assert_eq!(
            snap.dropped_total,
            (batch_size * 2) as u64,
            "failed batch and breaker-skipped batch are both accounted"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn drop_when_error_breaker_skips_without_touching_sink() -> SinkResult<()> {
        let sink_name = "drop_breaker_skip";
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        conf.set_drop_when_error(true);
        let mut runtime = SinkRuntime::new(
            "./rescue".to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FlakySink::new(attempts.clone(), usize::MAX))),
            None,
            Vec::new(),
        );

        runtime
            .send_to_sink(1, rec_packet("/shh/breaker_rule"), None, None)
            .await
            .expect("first failure dropped");
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "first send hits the sink"
        );

        for id in 2..=6 {
            runtime
                .send_to_sink(id, rec_packet("/shh/breaker_rule"), None, None)
                .await
                .expect("breaker skips silently");
        }
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "breaker open must not call the connector again"
        );

        let snap = sink_error_stats::snapshot_for(sink_name).expect("drop counter recorded");
        assert_eq!(snap.dropped_total, 6);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn drop_when_error_breaker_probe_recovers_after_cooldown() -> SinkResult<()> {
        let sink_name = "drop_breaker_probe";
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        conf.set_drop_when_error(true);
        // 第一次尝试失败，之后的探测成功。
        let mut runtime = SinkRuntime::new(
            "./rescue".to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FlakySink::new(attempts.clone(), 1))),
            None,
            Vec::new(),
        );
        runtime
            .health
            .set_backoff_for_test(Duration::from_millis(20));

        runtime
            .send_to_sink(1, rec_packet("/shh/probe_rule"), None, None)
            .await
            .expect("first failure dropped");
        assert!(runtime.health.is_open(Instant::now()));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(30)).await;
        runtime
            .send_to_sink(2, rec_packet("/shh/probe_rule"), None, None)
            .await
            .expect("probe succeeds after recovery");
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "probe reaches the sink");
        assert!(!runtime.health.is_open(Instant::now()));

        runtime
            .send_to_sink(3, rec_packet("/shh/probe_rule"), None, None)
            .await
            .expect("normal delivery resumes");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn rescue_file_is_reused_across_swap_cycles() -> SinkResult<()> {
        let temp = tempdir().owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let sink_name = "rescue_reuse";
        let conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        let (bad_tx, mut bad_rx) = tokio::sync::mpsc::channel(4);
        let (mon_tx, _mon_rx) = tokio::sync::mpsc::channel(16);

        let mut runtime = SinkRuntime::new(
            rescue_root.display().to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FailingSink)),
            None,
            Vec::new(),
        );
        let instance_id = runtime.instance_id();

        // 第一次失败：切到救援文件，真实 sink 进入 bad-sink 队列。
        runtime
            .send_to_sink(1, rec_packet("/shh/reuse"), Some(&bad_tx), None)
            .await?;
        bad_rx.recv().await.expect("bad sink handle");
        assert_eq!(
            rescue_files(&rescue_root)?,
            1,
            "first failure creates a file"
        );

        // 维护线程重连后切回真实 sink（仍然会失败），救援文件保持打开。
        runtime
            .recover_sink(
                ASinkHandle::new(
                    sink_name.to_string(),
                    instance_id,
                    SinkBackendType::Proxy(Box::new(FailingSink)),
                ),
                &mon_tx,
            )
            .await?;
        assert_eq!(rescue_files(&rescue_root)?, 1);

        // 第二次失败：复用同一个救援文件，不再新建。
        runtime
            .send_to_sink(2, rec_packet("/shh/reuse"), Some(&bad_tx), None)
            .await?;
        bad_rx.recv().await.expect("bad sink handle");
        assert_eq!(
            rescue_files(&rescue_root)?,
            1,
            "one outage must produce exactly one rescue file"
        );

        // 真正恢复：成功投递后救援文件被回收成 .dat。
        runtime
            .recover_sink(
                ASinkHandle::new(
                    sink_name.to_string(),
                    instance_id,
                    SinkBackendType::Proxy(Box::new(CountingSink::new(Arc::new(
                        AtomicUsize::new(0),
                    )))),
                ),
                &mon_tx,
            )
            .await?;
        runtime
            .send_to_sink(3, rec_packet("/shh/reuse"), Some(&bad_tx), None)
            .await?;

        let files = rescue_file_names(&rescue_root)?;
        assert_eq!(files.len(), 1, "finalized rescue file: {files:?}");
        assert!(
            files[0].ends_with(".dat"),
            "finalized rescue file must drop the .lock suffix: {files:?}"
        );
        Ok(())
    }

    #[test]
    fn sink_health_backoff_grows_and_caps() {
        let mut health = SinkHealth::new();
        let t = Instant::now();

        health.on_failure(t);
        assert!(
            health.is_open(t + Duration::from_secs(4)),
            "first cooldown is 5s"
        );
        assert!(!health.is_open(t + Duration::from_secs(6)));

        health.on_failure(t + Duration::from_secs(6));
        assert!(
            health.is_open(t + Duration::from_secs(15)),
            "second failure doubles the cooldown to 10s"
        );
        assert!(!health.is_open(t + Duration::from_secs(17)));

        for _ in 0..10 {
            health.on_failure(t);
        }
        assert!(
            health.is_open(t + Duration::from_secs(59)),
            "cooldown is capped at 60s"
        );
        assert!(!health.is_open(t + Duration::from_secs(61)));

        health.on_success();
        assert!(!health.is_open(t), "success resets the breaker");
        health.on_failure(t);
        assert!(
            health.is_open(t + Duration::from_secs(4)),
            "backoff resets to 5s"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn recover_sink_requires_matching_instance_id() -> SinkResult<()> {
        let sink_name = "rescue_instance_id";
        let conf = || SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        let mut first = SinkRuntime::new(
            "./rescue".to_string(),
            sink_name,
            conf(),
            SinkBackendType::Proxy(Box::new(FailingSink)),
            None,
            Vec::new(),
        );
        let mut second = SinkRuntime::new(
            "./rescue".to_string(),
            sink_name,
            conf(),
            SinkBackendType::Proxy(Box::new(FailingSink)),
            None,
            Vec::new(),
        );
        let (mon_tx, _mon_rx) = tokio::sync::mpsc::channel(16);

        let handle_for_second = ASinkHandle::new(
            sink_name.to_string(),
            second.instance_id(),
            SinkBackendType::Proxy(Box::new(FailingSink)),
        );
        assert!(!first.accepts_handle(&handle_for_second));
        assert!(
            !first.recover_sink(handle_for_second, &mon_tx).await?,
            "a replica must not swallow another replica's recovered sink"
        );

        // 未处于救援态时，陈旧 handle 不能把当前真实 sink 换出去。
        second.backup_used = true;
        let handle_for_second = ASinkHandle::new(
            sink_name.to_string(),
            second.instance_id(),
            SinkBackendType::Proxy(Box::new(FailingSink)),
        );
        assert!(
            second.recover_sink(handle_for_second, &mon_tx).await?,
            "the matching replica accepts its handle while rescuing"
        );
        assert!(!second.is_backup_active());

        let stale = ASinkHandle::new(
            sink_name.to_string(),
            second.instance_id(),
            SinkBackendType::Proxy(Box::new(FailingSink)),
        );
        assert!(
            !second.recover_sink(stale, &mon_tx).await?,
            "a stale handle must not swap the live sink again"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn drop_when_error_skips_str_package_when_breaker_open() -> SinkResult<()> {
        let sink_name = "drop_str_pkg";
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        conf.set_drop_when_error(true);
        let mut runtime = SinkRuntime::new(
            "./rescue".to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FlakySink::new(attempts.clone(), usize::MAX))),
            None,
            Vec::new(),
        );

        runtime
            .send_to_sink(1, rec_packet("/shh/str_pkg"), None, None)
            .await?;
        assert_eq!(attempts.load(Ordering::SeqCst), 1);

        let pkg = SinkStrPackage::from_units(
            (0..3).map(|idx| crate::sinks::SinkStrUnit::new(idx as u64, format!("line-{idx}"))),
        );
        runtime.send_str_package_to_sink(pkg, None, None).await?;

        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "breaker open must skip str package sends"
        );
        let snap = sink_error_stats::snapshot_for(sink_name).expect("drop counter recorded");
        assert_eq!(snap.dropped_total, 1 + 3);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn rescue_batch_failure_retries_into_rescue_file() -> SinkResult<()> {
        let temp = tempdir().owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let sink_name = "rescue_batch";
        let conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        let (bad_tx, mut bad_rx) = tokio::sync::mpsc::channel(4);
        let batch_size = 4;
        let mut runtime = SinkRuntime::with_batch_size(
            rescue_root.display().to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FailingSink)),
            None,
            Vec::new(),
            batch_size,
        );

        runtime
            .send_package_to_sink(&build_package(batch_size), Some(&bad_tx), None)
            .await?;
        let handle = bad_rx.recv().await.expect("bad sink handle");
        assert_eq!(handle.name, sink_name);

        // 救援文件是 BufWriter，小于 100 条时不会立刻落盘；退出时 stop 会 flush + 改名。
        runtime.stop_backends().await?;

        let files = rescue_file_names(&rescue_root)?;
        assert_eq!(files.len(), 1, "batch failure creates one rescue file");
        assert!(
            files[0].ends_with(".dat"),
            "stopped rescue file must be unlocked: {files:?}"
        );
        let size = std::fs::metadata(rescue_root.join(&files[0]))
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?
            .len();
        assert!(
            size > 0,
            "failed batch must be written into the rescue file"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn stop_backends_finalizes_retired_rescue() -> SinkResult<()> {
        let temp = tempdir().owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)
            .owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        let sink_name = "rescue_stop";
        let conf = SinkInstanceConf::null_new(sink_name.to_string(), TextFmt::Json, None);
        let (bad_tx, mut bad_rx) = tokio::sync::mpsc::channel(4);
        let (mon_tx, _mon_rx) = tokio::sync::mpsc::channel(16);
        let mut runtime = SinkRuntime::new(
            rescue_root.display().to_string(),
            sink_name,
            conf,
            SinkBackendType::Proxy(Box::new(FailingSink)),
            None,
            Vec::new(),
        );
        let instance_id = runtime.instance_id();

        runtime
            .send_to_sink(1, rec_packet("/shh/stop"), Some(&bad_tx), None)
            .await?;
        bad_rx.recv().await.expect("bad sink handle");
        runtime
            .recover_sink(
                ASinkHandle::new(
                    sink_name.to_string(),
                    instance_id,
                    SinkBackendType::Proxy(Box::new(FailingSink)),
                ),
                &mon_tx,
            )
            .await?;

        // 退出时即使没有新的成功投递，也要把退役的救援文件回收成 .dat。
        runtime.stop_backends().await?;
        let files = rescue_file_names(&rescue_root)?;
        assert_eq!(files.len(), 1, "finalized rescue file: {files:?}");
        assert!(
            files[0].ends_with(".dat"),
            "stop_backends must unlock the rescue file: {files:?}"
        );
        Ok(())
    }

    fn rescue_file_names(dir: &Path) -> SinkResult<Vec<String>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).owe(SinkReason::Uvs(UnifiedReason::system_error()))? {
            let entry = entry.owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
            if entry.path().is_file() {
                out.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        out.sort();
        Ok(out)
    }

    fn rescue_files(dir: &Path) -> SinkResult<usize> {
        Ok(rescue_file_names(dir)?.len())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn small_package_stays_in_pending_buffer_until_flush() -> SinkResult<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let primary = SinkBackendType::Proxy(Box::new(CountingSink::new(calls.clone())));
        let conf = SinkInstanceConf::new_type(
            "bench".into(),
            TextFmt::Json,
            "blackhole".into(),
            Default::default(),
            None,
        );
        let mut runtime = SinkRuntime::with_batch_size(
            "./rescue".to_string(),
            "/sink/bench/[0]",
            conf,
            primary,
            None,
            Vec::new(),
            8,
        );

        let package = build_package(5);
        runtime.send_package_to_sink(&package, None, None).await?;
        // 小包未达到阈值时进入 pending，不会立即下发
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        runtime.flush(None, None).await?;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn large_package_bypasses_pending_buffer() -> SinkResult<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let primary = SinkBackendType::Proxy(Box::new(CountingSink::new(calls.clone())));
        let conf = SinkInstanceConf::new_type(
            "bench".into(),
            TextFmt::Json,
            "blackhole".into(),
            Default::default(),
            None,
        );
        let mut runtime = SinkRuntime::with_batch_size(
            "./rescue".to_string(),
            "/sink/bench/[0]",
            conf,
            primary,
            None,
            Vec::new(),
            2,
        );

        let package = build_package(5);
        runtime.send_package_to_sink(&package, None, None).await?;
        // 入站包达到阈值且 pending 为空时，直接按 package 一次下发
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        runtime.flush(None, None).await?;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn large_oml_package_bypasses_pending_buffer_with_batch_meta() -> SinkResult<()> {
        let mut capture = capturing_runtime(2);

        let package = build_oml_package(5, "nginx_access");
        capture
            .runtime
            .send_package_to_sink(&package, None, None)
            .await?;

        assert_eq!(capture.plain_calls.load(Ordering::SeqCst), 0);
        let metas = capture.metas.lock().expect("metas lock");
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].oml_name(), Some("nginx_access"));
        assert_eq!(
            capture
                .batch_lens
                .lock()
                .expect("batch_lens lock")
                .as_slice(),
            &[5]
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn pending_buffer_flushes_before_oml_name_changes() -> SinkResult<()> {
        let mut capture = capturing_runtime(8);

        let nginx = build_oml_package(2, "nginx_access");
        capture
            .runtime
            .send_package_to_sink(&nginx, None, None)
            .await?;
        assert_eq!(capture.metas.lock().expect("metas lock").len(), 0);

        let conn = build_oml_package(2, "conn_events");
        capture
            .runtime
            .send_package_to_sink(&conn, None, None)
            .await?;
        {
            let metas = capture.metas.lock().expect("metas lock");
            assert_eq!(metas.len(), 1);
            assert_eq!(metas[0].oml_name(), Some("nginx_access"));
        }

        capture.runtime.flush(None, None).await?;
        assert_eq!(capture.plain_calls.load(Ordering::SeqCst), 0);
        let metas = capture.metas.lock().expect("metas lock");
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].oml_name(), Some("nginx_access"));
        assert_eq!(metas[1].oml_name(), Some("conn_events"));
        assert_eq!(
            capture
                .batch_lens
                .lock()
                .expect("batch_lens lock")
                .as_slice(),
            &[2, 2]
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn mixed_oml_package_splits_batches_by_oml_name() -> SinkResult<()> {
        let mut capture = capturing_runtime(2);

        let package = build_mixed_oml_package(&[
            "nginx_access",
            "nginx_access",
            "conn_events",
            "conn_events",
        ]);
        capture
            .runtime
            .send_package_to_sink(&package, None, None)
            .await?;

        assert_eq!(capture.plain_calls.load(Ordering::SeqCst), 0);
        let metas = capture.metas.lock().expect("metas lock");
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].oml_name(), Some("nginx_access"));
        assert_eq!(metas[1].oml_name(), Some("conn_events"));
        assert_eq!(
            capture
                .batch_lens
                .lock()
                .expect("batch_lens lock")
                .as_slice(),
            &[2, 2]
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn wpl_rule_package_does_not_emit_oml_batch_meta() -> SinkResult<()> {
        let mut capture = capturing_runtime(2);

        let package = build_package(5);
        capture
            .runtime
            .send_package_to_sink(&package, None, None)
            .await?;

        assert_eq!(capture.plain_calls.load(Ordering::SeqCst), 1);
        assert!(capture.metas.lock().expect("metas lock").is_empty());
        assert_eq!(
            capture
                .batch_lens
                .lock()
                .expect("batch_lens lock")
                .as_slice(),
            &[5]
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn group_disabled_meta_fields_are_attached_to_batch_meta() -> SinkResult<()> {
        let mut capture = capturing_runtime(8);
        capture
            .runtime
            .set_meta_output_disabled(&["wp_oml_name".to_string()]);

        let package = build_oml_package(2, "nginx_access");
        capture
            .runtime
            .send_package_to_sink(&package, None, None)
            .await?;
        capture.runtime.flush(None, None).await?;

        assert_eq!(capture.plain_calls.load(Ordering::SeqCst), 0);
        let metas = capture.metas.lock().expect("metas lock");
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].oml_name(), Some("nginx_access"));
        assert!(metas[0].is_output_disabled("wp_oml_name"));
        assert_eq!(
            capture
                .batch_lens
                .lock()
                .expect("batch_lens lock")
                .as_slice(),
            &[2]
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn disabled_meta_fields_call_meta_path_without_oml_name() -> SinkResult<()> {
        let mut capture = capturing_runtime(2);
        capture
            .runtime
            .set_meta_output_disabled(&["wp_oml_name".to_string()]);

        let package = build_package(5);
        capture
            .runtime
            .send_package_to_sink(&package, None, None)
            .await?;

        assert_eq!(capture.plain_calls.load(Ordering::SeqCst), 0);
        let metas = capture.metas.lock().expect("metas lock");
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].oml_name(), None);
        assert!(metas[0].is_output_disabled("wp_oml_name"));
        assert_eq!(
            capture
                .batch_lens
                .lock()
                .expect("batch_lens lock")
                .as_slice(),
            &[5]
        );
        Ok(())
    }
}
