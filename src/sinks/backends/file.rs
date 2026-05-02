use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::core::sinks::sync_sink::{RecSyncSink, TrySendStatus};
use crate::sinks::utils::buffer_monitor::BufferMonitor;
use crate::sinks::utils::formatter::FormatAdapter;
use crate::sinks::{SinkEndpoint, SinkRecUnit};
use crate::types::{Build1, SafeH};
use orion_error::compat_prelude::ErrorOweBase;
use std::fs;
use std::fs::File;
use std::io::{Cursor, ErrorKind, Write};
use std::sync::Arc;
use wp_connector_api::{SinkReason, SinkResult};
use wp_data_fmt::{FormatType, RecordFormatter};
use wp_model_core::model::fmt_def::TextFmt;

pub fn create_watch_out(fmt: TextFmt) -> (SafeH<Cursor<Vec<u8>>>, SinkEndpoint) {
    let buffer_out = BufferMonitor::new();
    let buffer = buffer_out.buffer.clone();
    let mut out: FormatAdapter<BufferMonitor> = FormatAdapter::new(fmt);
    out.next_pipe(buffer_out);
    let out = SinkEndpoint::Buffer(out);
    (buffer, out)
}

#[derive(Clone)]
pub struct FileSink {
    path: String,
    out_io: SafeH<std::fs::File>,
    buffer: Cursor<Vec<u8>>,
    lock_released: bool,
}

impl FileSink {
    pub fn new(out_path: &str) -> SinkResult<Self> {
        let out_io = File::create(out_path).owe(SinkReason::sink(format!(
            "create output file '{}'",
            out_path
        )))?;
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

impl RecSyncSink for FileSink {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        if let Ok(mut out_io) = self.out_io.write() {
            let formatted = FormatType::from(&wp_model_core::model::fmt_def::TextFmt::Raw)
                .fmt_record(data.data());
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
pub use wp_core_connectors::sinks::file::{AsyncFileSink, FileSinkSpec};
