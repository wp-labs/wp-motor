use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::core::sinks::sync_sink::{RecSyncSink, TrySendStatus};
use crate::sinks::prelude::*;
use crate::sinks::utils::buffer_monitor::BufferMonitor;
use crate::sinks::utils::formatter::FormatAdapter;
use crate::sinks::{SinkEndpoint, SinkRecUnit};
use crate::types::{AnyResult, Build1, SafeH};
use anyhow::Context;
use async_trait::async_trait;
use orion_error::ErrorOwe;
use std::fs;
use std::fs::File;
use std::io::{Cursor, ErrorKind, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio_async_drop::tokio_async_drop;
use wp_connector_api::{SinkBuildCtx, SinkReason, SinkResult, SinkSpec as ResolvedSinkSpec};
use wp_data_fmt::{DataFormat, FormatType};
use wp_model_core::model::fmt_def::TextFmt;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static SYNC_ALL_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub(super) fn take_sync_all_count() -> usize {
    SYNC_ALL_COUNTER.swap(0, Ordering::Relaxed)
}

#[cfg(test)]
fn record_sync_all_call() {
    SYNC_ALL_COUNTER.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(test))]
fn record_sync_all_call() {}

pub fn create_watch_out(fmt: TextFmt) -> (SafeH<Cursor<Vec<u8>>>, SinkEndpoint) {
    let buffer_out = BufferMonitor::new();
    let buffer = buffer_out.buffer.clone();
    let mut out: FormatAdapter<BufferMonitor> = FormatAdapter::new(fmt);
    out.next_pipe(buffer_out);
    let out = SinkEndpoint::Buffer(out);
    (buffer, out)
}

#[derive(Clone, Debug)]
pub(crate) struct FileSinkSpec {
    fmt: TextFmt,
    base: String,
    file_name: String,
    sync: bool,
}

impl FileSinkSpec {
    pub(crate) fn from_resolved(_kind: &str, spec: &ResolvedSinkSpec) -> AnyResult<Self> {
        if let Some(s) = spec.params.get("fmt").and_then(|v| v.as_str()) {
            let ok = matches!(s, "json" | "csv" | "show" | "kv" | "raw" | "proto-text");
            if !ok {
                anyhow::bail!(
                    "invalid fmt: '{}'; allowed: json,csv,show,kv,raw,proto-text",
                    s
                );
            }
        }
        let fmt = spec
            .params
            .get("fmt")
            .and_then(|v| v.as_str())
            .map(TextFmt::from)
            .unwrap_or(TextFmt::Json);
        let base = spec
            .params
            .get("base")
            .and_then(|v| v.as_str())
            .unwrap_or("./data/out_dat")
            .to_string();
        let file_name = spec
            .params
            .get("file")
            .and_then(|v| v.as_str())
            .unwrap_or("out.dat")
            .to_string();
        let sync = spec
            .params
            .get("sync")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self {
            fmt,
            base,
            file_name,
            sync,
        })
    }

    pub(crate) fn text_fmt(&self) -> TextFmt {
        self.fmt
    }

    pub(crate) fn sync(&self) -> bool {
        self.sync
    }

    pub(crate) fn resolve_path(&self, _ctx: &SinkBuildCtx) -> String {
        Path::new(&self.base)
            .join(&self.file_name)
            .display()
            .to_string()
    }
}

#[derive(Clone)]
pub struct FileSink {
    path: String,
    out_io: SafeH<std::fs::File>,
    buffer: Cursor<Vec<u8>>,
    lock_released: bool,
}

impl FileSink {
    pub fn new(out_path: &str) -> AnyResult<Self> {
        let out_io =
            File::create(out_path).with_context(|| format!("output file fail :{}", out_path))?;
        Ok(Self {
            path: out_path.to_string(),
            out_io: SafeH::build(out_io),
            buffer: Cursor::new(Vec::with_capacity(10240)),
            lock_released: !out_path.ends_with(".lock"),
        })
    }

    fn unlock_lockfile(&mut self) -> std::io::Result<()> {
        if self.lock_released || !self.path.ends_with(".lock") {
            self.lock_released = true;
            return Ok(());
        }
        if let Some(new_path) = self.path.strip_suffix(".lock") {
            match fs::rename(&self.path, new_path) {
                Ok(()) => {
                    self.lock_released = true;
                    Ok(())
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    self.lock_released = true;
                    Ok(())
                }
                Err(err) => Err(err),
            }
        } else {
            self.lock_released = true;
            Ok(())
        }
    }
}

impl Drop for FileSink {
    fn drop(&mut self) {
        if let Err(e) = self.unlock_lockfile() {
            error_data!("解锁备份文件失败,{}", e);
        }
    }
}

impl SyncCtrl for FileSink {
    fn stop(&mut self) -> SinkResult<()> {
        if let Ok(mut out_io) = self.out_io.write() {
            out_io
                .write_all(&self.buffer.clone().into_inner())
                .owe(SinkReason::Sink("file stop fail".into()))?;
        }
        if let Err(e) = self.unlock_lockfile() {
            error_data!("unlock rescue file on stop failed: {}", e);
        }
        Ok(())
    }
}

#[async_trait]
impl RecSyncSink for FileSink {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        if let Ok(mut out_io) = self.out_io.write() {
            let formatted = FormatType::from(&wp_model_core::model::fmt_def::TextFmt::Raw)
                .format_record(data.data());
            out_io
                .write_all(format!("{}\n", formatted).as_bytes())
                .owe(SinkReason::sink("file out fail"))?;
        }
        Ok(())
    }

    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        match self.send_to_sink(data) {
            Ok(()) => TrySendStatus::Sended,
            Err(e) => TrySendStatus::Err(Arc::new(e)),
        }
    }
}

// Default buffer for classic file sink (kept for compatibility)
const FILE_BUF_SIZE: usize = 102_400; // 100 KiB

// Classic async file sink (original behavior preserved):
// - Direct BufWriter writes
// - Periodic flush by count (every 100 writes) OR immediate flush based on config
pub struct AsyncFileSink {
    path: String,
    out_io: BufWriter<tokio::fs::File>,
    proc_cnt: usize,
    sync: bool,
    lock_released: bool,
}

impl Drop for AsyncFileSink {
    fn drop(&mut self) {
        if let Err(e) = self.unlock_lockfile() {
            error_data!("解锁备份文件失败,{}", e);
        }
        tokio_async_drop!({
            let _ = self.out_io.flush().await;
        });
    }
}

impl AsyncFileSink {
    pub async fn new(out_path: &str) -> AnyResult<Self> {
        Self::with_sync(out_path, false).await
    }

    pub async fn with_sync(out_path: &str, sync: bool) -> AnyResult<Self> {
        if let Some(parent) = std::path::Path::new(out_path).parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }
        let out_io = OpenOptions::new()
            .append(true)
            .create(true)
            .open(out_path)
            .await
            .with_context(|| format!("output file fail :{}", out_path))?;
        Ok(Self {
            path: out_path.to_string(),
            out_io: BufWriter::with_capacity(FILE_BUF_SIZE, out_io),
            proc_cnt: 0,
            sync,
            lock_released: !out_path.ends_with(".lock"),
        })
    }

    fn unlock_lockfile(&mut self) -> std::io::Result<()> {
        if self.lock_released || !self.path.ends_with(".lock") {
            self.lock_released = true;
            return Ok(());
        }
        if let Some(new_path) = self.path.strip_suffix(".lock") {
            match fs::rename(&self.path, new_path) {
                Ok(()) => {
                    self.lock_released = true;
                    Ok(())
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    self.lock_released = true;
                    Ok(())
                }
                Err(err) => Err(err),
            }
        } else {
            self.lock_released = true;
            Ok(())
        }
    }
}

#[async_trait]
impl AsyncCtrl for AsyncFileSink {
    async fn stop(&mut self) -> SinkResult<()> {
        self.out_io
            .flush()
            .await
            .owe(SinkReason::sink("file out fail"))?;
        if let Err(e) = self.unlock_lockfile() {
            error_data!("unlock rescue file on stop failed: {}", e);
        }
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncRawdatSink for AsyncFileSink {
    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        self.out_io
            .write_all(data)
            .await
            .owe(SinkReason::sink("file out fail"))?;
        self.proc_cnt += 1;

        // Flush immediately if sync mode enabled, otherwise flush every 100 writes
        if self.sync || self.proc_cnt.is_multiple_of(100) {
            self.out_io
                .flush()
                .await
                .owe(SinkReason::sink("file out fail"))?;

            // If sync mode enabled, force data to disk
            if self.sync {
                self.out_io
                    .get_ref()
                    .sync_all()
                    .await
                    .owe(SinkReason::sink("file sync fail"))?;
                record_sync_all_call();
            }
        }
        Ok(())
    }
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        if data.as_bytes().last() == Some(&b'\n') {
            self.sink_bytes(data.as_bytes()).await
        } else {
            // Need to add newline and trigger flush logic
            let mut buffer = Vec::with_capacity(data.len() + 1);
            buffer.extend_from_slice(data.as_bytes());
            buffer.push(b'\n');
            self.sink_bytes(&buffer).await
        }
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        if data.is_empty() {
            return Ok(());
        }

        let mut total_len = 0;
        for str_data in &data {
            total_len += str_data.len();
            if str_data.as_bytes().last().is_none_or(|&b| b != b'\n') {
                total_len += 1;
            }
        }

        let mut buffer = Vec::with_capacity(total_len);
        for str_data in &data {
            buffer.extend_from_slice(str_data.as_bytes());
            if str_data.as_bytes().last().is_none_or(|&b| b != b'\n') {
                buffer.push(b'\n');
            }
        }

        self.out_io
            .write_all(&buffer)
            .await
            .owe(SinkReason::sink("file out fail"))?;

        self.proc_cnt += data.len();

        // Flush immediately if sync mode enabled, otherwise flush every 100 writes
        if self.sync || self.proc_cnt.is_multiple_of(100) {
            self.out_io
                .flush()
                .await
                .owe(SinkReason::sink("file out fail"))?;

            // If sync mode enabled, force data to disk
            if self.sync {
                self.out_io
                    .get_ref()
                    .sync_all()
                    .await
                    .owe(SinkReason::sink("file sync fail"))?;
                record_sync_all_call();
            }
        }

        Ok(())
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        if data.is_empty() {
            return Ok(());
        }

        let mut total_len = 0;
        for bytes_data in &data {
            total_len += bytes_data.len();
            if bytes_data.last().is_none_or(|&b| b != b'\n') {
                total_len += 1;
            }
        }

        let mut buffer = Vec::with_capacity(total_len);
        for bytes_data in &data {
            buffer.extend_from_slice(bytes_data);
            if bytes_data.last().is_none_or(|&b| b != b'\n') {
                buffer.push(b'\n');
            }
        }

        self.out_io
            .write_all(&buffer)
            .await
            .owe(SinkReason::sink("file out fail"))?;

        self.proc_cnt += data.len();

        // Flush immediately if sync mode enabled, otherwise flush every 100 writes
        if self.sync || self.proc_cnt.is_multiple_of(100) {
            self.out_io
                .flush()
                .await
                .owe(SinkReason::sink("file out fail"))?;

            // If sync mode enabled, force data to disk
            if self.sync {
                self.out_io
                    .get_ref()
                    .sync_all()
                    .await
                    .owe(SinkReason::sink("file sync fail"))?;
                record_sync_all_call();
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write as _;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wp_connector_api::AsyncRecordSink;
    use wp_model_core::model::DataRecord;
    use wp_model_core::model::fmt_def::TextFmt;

    use crate::sinks::backends::file::AsyncFileSink;
    use crate::sinks::utils::formatter::AsyncFormatter;
    use crate::types::AnyResult;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_processor() -> AnyResult<()> {
        let mut pipe: AsyncFormatter<AsyncFileSink> = AsyncFormatter::new(TextFmt::Json);
        pipe.next_pipe(AsyncFileSink::new("./temp/test.pid").await?);
        let test_data = DataRecord::default();
        pipe.sink_record(&test_data).await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stop_unlocks_only_own_lock() -> AnyResult<()> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("wp_rescue_unlock_{}", ts));
        let own_lock = base.join("group1/sinkA-001.dat.lock");
        let other_lock = base.join("group1/sinkB-001.dat.lock");
        if let Some(p) = own_lock.parent() {
            fs::create_dir_all(p)?;
        }
        if let Some(p) = other_lock.parent() {
            fs::create_dir_all(p)?;
        }

        fs::File::create(&other_lock)?.write_all(b"test")?;

        let mut sink = AsyncFileSink::new(own_lock.to_string_lossy().as_ref()).await?;
        wp_connector_api::AsyncRawDataSink::sink_str(&mut sink, "line1").await?;
        wp_connector_api::AsyncCtrl::stop(&mut sink).await?;

        assert!(!Path::new(own_lock.to_string_lossy().as_ref()).exists());
        assert!(Path::new(base.join("group1/sinkA-001.dat").to_string_lossy().as_ref()).exists());
        assert!(Path::new(other_lock.to_string_lossy().as_ref()).exists());

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sync_parameter() -> AnyResult<()> {
        use wp_connector_api::{AsyncCtrl, AsyncRawDataSink};

        // Reset sync counter before exercising the sink
        super::take_sync_all_count();

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("wp_sync_test_{}", ts));
        fs::create_dir_all(&base)?;

        let sync_file = base.join("sync_true.dat.lock");
        let mut sink_sync =
            AsyncFileSink::with_sync(sync_file.to_string_lossy().as_ref(), true).await?;
        AsyncRawDataSink::sink_str(&mut sink_sync, "line1").await?;
        AsyncRawDataSink::sink_str(&mut sink_sync, "line2").await?;
        let sync_calls = super::take_sync_all_count();
        assert_eq!(
            sync_calls, 2,
            "sync=true 应在每次写入后触发 fsync 确保落盘"
        );
        AsyncCtrl::stop(&mut sink_sync).await?;

        let no_sync_file = base.join("sync_false.dat.lock");
        let mut sink_no_sync =
            AsyncFileSink::with_sync(no_sync_file.to_string_lossy().as_ref(), false).await?;
        AsyncRawDataSink::sink_str(&mut sink_no_sync, "line1").await?;
        AsyncRawDataSink::sink_str(&mut sink_no_sync, "line2").await?;
        let sync_calls = super::take_sync_all_count();
        assert_eq!(
            sync_calls, 0,
            "sync=false 模式下不应调用 fsync"
        );
        AsyncCtrl::stop(&mut sink_no_sync).await?;

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }
}
