use crate::sinks::pdm_outer::TDMDataAble;
use crate::sinks::prelude::*;
use chrono::Utc;
use derive_getters::Getters;
use orion_exp::{Expression, RustSymbol};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use wp_model_core::model::{DataField, fmt_def::TextFmt};

use crate::runtime::errors::err4_send_to_sink;
use crate::sinks::RescueFileSink;
use crate::sinks::{
    ASinkHandle, ASinkSender, SinkBackendType, SinkDataEnum, SinkFFVPackage, SinkPackage,
    SinkStrPackage,
};
use crate::stat::MonSend;
use crate::stat::metric_collect::MetricCollectors;
use wp_conf::structure::SinkInstanceConf;
use wp_connector_api::{SinkReason, SinkResult};
use wp_error::error_handling::{ErrorHandlingStrategy, sys_robust_mode};
use wp_parse_api::RawData;

use crate::types::AnyResult;
use orion_error::{ErrorOwe, ErrorWith};
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
    status: RuntimeStautus,
    normal_stat: MetricCollectors,
    backup_stat: MetricCollectors,
    timer: TimedStat,
    backup_used: bool,
    timer_poll_ticks: u8,
    last_stat_sent_at: Instant,
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
        let backup_name = format!("{}_bak", name.clone().into());
        let normal_stat = MetricCollectors::new(name.clone().into(), stat_reqs.clone());
        let backup_stat = MetricCollectors::new(backup_name.clone(), stat_reqs);
        info_ctrl!("create sink:{} ", conf.full_name());
        let pre_tags = Self::compile_tags(&conf);
        Self {
            rescue,
            //backup_name,
            name: name.into(),
            conf,
            pre_tags,
            primary: sink,
            cond,
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
    pub fn ready(&mut self) {
        self.status.ready();
    }

    pub fn get_cond(&self) -> Option<&Expression<DataField, RustSymbol>> {
        self.cond.as_ref()
    }
    pub async fn swap_backsink(&mut self) -> AnyResult<Option<SinkBackendType>> {
        let now = Utc::now();
        let fmt_time = now.format("%Y-%m-%d_%H:%M:%S").to_string();
        // 正在写的的rescue文件加上.lock后缀，当sink被drop时去掉.lock后缀
        let file_path = format!("{}/{}-{}.dat.lock", self.rescue, self.name, fmt_time);
        let out_path = Path::new(&file_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| SinkError::from(SinkReason::Sink(e.to_string())))?;
        }
        info_ctrl!("crate out file use async mode {}", file_path);
        let back = RescueFileSink::new(&file_path).await?;
        let old_primary =
            std::mem::replace(&mut self.primary, SinkBackendType::Proxy(Box::new(back)));
        Ok(Some(old_primary))
    }

    pub async fn send_stat(&mut self, mon_send: &MonSend) -> SinkResult<()> {
        self.normal_stat
            .send_stat(mon_send)
            .await
            .owe_sys()
            .want("sink stat")?;
        if self.backup_used {
            self.backup_stat
                .send_stat(mon_send)
                .await
                .owe_sys()
                .want("back sink stat")?;
        }
        Ok(())
    }
}
impl SinkRuntime {
    /// 发送单个数据项到 Sink（保持向后兼容）
    pub async fn send_to_sink(
        &mut self,
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
                        .map_err(|e| SinkError::from(SinkReason::Sink(e.to_string())))?;
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
                        debug_data!("sink error and tolerant : {}", e);
                        //pass;
                    }
                    ErrorHandlingStrategy::Ignore => {
                        debug_data!("sink error and ignore: {}", e);
                    }
                    ErrorHandlingStrategy::Terminate => {
                        info_data!("sink error and end: {}", e);
                        break;
                    }
                }
            } else {
                self.stat_end(&data);
                debug_data!("sink {} send suc!", self.name);
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

        self.record_package_stats_begin_rec(package);
        loop {
            let records: Vec<Arc<DataRecord>> =
                package.iter().map(|unit| unit.data().clone()).collect();
            if records.is_empty() {
                self.record_package_stats_end_rec(package);
                return Ok(());
            }
            match self.primary.sink_records(records).await {
                Ok(()) => {
                    self.record_package_stats_end_rec(package);
                    return Ok(());
                }
                Err(e) => {
                    if self.handle_send_error(&e, bad_s, mon).await? {
                        continue;
                    } else {
                        self.record_package_stats_end_rec(package);
                        return Err(e);
                    }
                }
            }
        }
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
                    .map_err(|e| SinkError::from(SinkReason::Sink(e.to_string())))
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
                Err(e) => {
                    if self.handle_send_error(&e, bad_s, mon).await? {
                        continue;
                    } else {
                        self.record_package_stats_end_ffv(&package);
                        return Err(e);
                    }
                }
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
                Err(e) => {
                    if self.handle_send_error(&e, bad_s, mon).await? {
                        continue;
                    } else {
                        self.record_package_stats_end_str(&package);
                        return Err(e);
                    }
                }
            }
        }
    }

    /// 记录包的统计开始信息
    fn record_package_stats_begin_rec(&mut self, package: &SinkPackage) {
        for unit in package {
            self.stat_beg(&SinkDataEnum::Rec(unit.meta().clone(), unit.data().clone()));
        }
    }

    /// 记录 FFV 包的统计开始信息
    fn record_package_stats_begin_ffv(&mut self, package: &SinkFFVPackage) {
        for unit in package {
            self.stat_beg(&SinkDataEnum::FFV(unit.data().clone()));
        }
    }

    /// 记录字符串包的统计开始信息
    fn record_package_stats_begin_str(&mut self, package: &SinkStrPackage) {
        for unit in package {
            self.stat_beg(&SinkDataEnum::Raw(unit.data().clone()));
        }
    }

    /// 记录包的统计结束信息
    fn record_package_stats_end_rec(&mut self, package: &SinkPackage) {
        for unit in package {
            self.stat_end(&SinkDataEnum::Rec(unit.meta().clone(), unit.data().clone()));
        }
    }

    /// 记录 FFV 包的统计结束信息
    fn record_package_stats_end_ffv(&mut self, package: &SinkFFVPackage) {
        for unit in package {
            self.stat_end(&SinkDataEnum::FFV(unit.data().clone()));
        }
    }

    /// 记录字符串包的统计结束信息
    fn record_package_stats_end_str(&mut self, package: &SinkStrPackage) {
        for unit in package {
            self.stat_end(&SinkDataEnum::Raw(unit.data().clone()));
        }
    }

    /// 处理发送错误
    async fn handle_send_error(
        &mut self,
        error: &SinkError,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<bool> {
        match err4_send_to_sink(error, &sys_robust_mode()) {
            ErrorHandlingStrategy::FixRetry => {
                if let Some(bad_sink_send) = bad_s {
                    self.use_back_sink(bad_sink_send, mon).await?;
                    return Ok(true);
                }
                Ok(false)
            }
            _ => Ok(false), // 表示未处理，需要返回错误
        }
    }

    fn stat_end(&mut self, data: &SinkDataEnum) {
        match &data {
            SinkDataEnum::Rec(_, dat) => {
                if self.backup_used {
                    self.backup_stat
                        .record_end(self.name.as_str(), Some(dat.as_ref()));
                } else {
                    self.normal_stat
                        .record_end(self.name.as_str(), Some(dat.as_ref()));
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
                self.normal_stat
                    .record_begin(self.name.as_str(), Some(dat.as_ref()));
                if self.backup_used {
                    self.backup_stat
                        .record_begin(self.name.as_str(), Some(dat.as_ref()));
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
            Err(err) => {
                return Err(SinkError::from(SinkReason::Sink(err.to_string())));
            }
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
    use async_trait::async_trait;
    use tempfile::tempdir;
    use wp_model_core::model::{DataField, DataRecord};

    struct FailingSink;

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn swap_back_routes_records_to_rescue_file() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let rescue_root = temp.path().join("rescue_root");
        std::fs::create_dir_all(&rescue_root)?;

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
            let packet =
                SinkDataEnum::Rec(ProcMeta::Rule("/shh/test_rule16".into()), Arc::new(record));

            runtime
                .send_to_sink(packet, Some(&bad_tx), None)
                .await
                .expect("send_to_sink should succeed after swap");

            let handle = bad_rx.recv().await.expect("bad sink handle");
            assert_eq!(handle.name, sink_name);
        }

        let benchmark_rescue = rescue_root.join("sink").join("benchmark");
        let entries = std::fs::read_dir(&benchmark_rescue)?.collect::<Result<Vec<_>, _>>()?;
        assert!(!entries.is_empty(), "expect rescue file created");
        let meta = std::fs::metadata(entries[0].path())?;
        assert!(meta.len() > 0, "rescue file should contain payload");
        Ok(())
    }
}
