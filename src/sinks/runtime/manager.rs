use crate::compat::LegacyOwe;
use crate::sinks::pdm_outer::TDMDataAble;
use crate::sinks::prelude::*;
use chrono::Utc;
use derive_getters::Getters;
use orion_exp::{Expression, RustSymbol};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use wp_conf::structure::default_batch_size;
use wp_model_core::model::{DataField, fmt_def::TextFmt};

// 全局计数器，用于生成唯一的救援文件序号
static RESCUE_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

use crate::runtime::errors::err4_send_to_sink;
use crate::sinks::RescueFileSink;
use crate::sinks::{
    ASinkHandle, ASinkSender, ProcMeta, SinkBackendType, SinkDataEnum, SinkFFVPackage, SinkPackage,
    SinkStrPackage,
};
use crate::stat::MonSend;
use crate::stat::metric_collect::MetricCollectors;
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
        info_ctrl!("create sink:{} batch_size={}", conf.full_name(), batch_size);
        let pre_tags = Self::compile_tags(&conf);

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
    pub async fn swap_backsink(&mut self) -> SinkResult<Option<SinkBackendType>> {
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
        let old_primary =
            std::mem::replace(&mut self.primary, SinkBackendType::Proxy(Box::new(back)));
        Ok(Some(old_primary))
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
                match err4_send_to_sink(&e, &sys_robust_mode()) {
                    ErrorHandlingStrategy::FixRetry => {
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
            } else {
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
                    return Ok(());
                }
                Err(e) => {
                    for e_id in 0..records.len() as u64 {
                        error_edata!(e_id, "flush sink data failed: {}", e);
                    }
                    match self.handle_send_error(&e, bad_s, mon).await? {
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
                    return Ok(());
                }
                Err(e) => match self.handle_send_error(&e, bad_s, mon).await? {
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

        self.record_package_stats_begin_str(&package);
        loop {
            let raw_strings: Vec<&str> = package.iter().map(|unit| unit.data().as_str()).collect();
            let result = self.primary.sink_str_batch(raw_strings).await;

            match result {
                Ok(()) => {
                    self.record_package_stats_end_str(&package);
                    return Ok(());
                }
                Err(e) => match self.handle_send_error(&e, bad_s, mon).await? {
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

    /// 处理发送错误
    async fn handle_send_error(
        &mut self,
        error: &SinkError,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<BatchErrHandle> {
        match err4_send_to_sink(error, &sys_robust_mode()) {
            ErrorHandlingStrategy::FixRetry => {
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
                    .send(ASinkHandle::new(self.name.clone(), old_primary))
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
        if self.name == sink_h.name {
            let mut old_primary = std::mem::replace(&mut self.primary, sink_h.sink);
            old_primary.stop().await?;
            self.send_stat(mon).await?;
            self.backup_used = false;
            return Ok(true);
        }
        Ok(false)
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
