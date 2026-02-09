use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::core::sinks::sync_sink::{RecSyncSink, TrySendStatus};
use crate::sinks::decorators::stub::StubOuter;
use crate::sinks::prelude::*;
use crate::types::{Build1, SafeH};
use std::io::{Cursor, Write};
use std::sync::Arc;

use async_trait::async_trait;
use orion_error::ErrorOwe;
use wp_connector_api::{SinkError, SinkReason, SinkResult};
use wp_model_core::model::Value;

use crate::sinks::SinkRecUnit;
// SinkResult comes from connector API now

pub type BufferMonitor = WatchOuterImpl<StubOuter>;

/// WatchOuterImpl is designed for real-time monitoring and observation.
///
/// # Design Philosophy
/// - **Real-time visibility**: Each record is written immediately to ensure external
///   readers can observe data as it arrives
/// - **Shared buffer**: The buffer is typically shared via `SafeH<Cursor<Vec<u8>>>` clone,
///   allowing external monitoring programs to read in real-time
/// - **No batching optimization**: Batch methods intentionally write records one-by-one
///   to maintain real-time visibility, rather than accumulating for bulk writes
///
/// # Use Cases
/// - Test fixtures that need to capture output
/// - Debug monitoring of data pipelines
/// - Real-time data observation tools
#[derive(Clone)]
pub struct WatchOuterImpl<T>
where
    T: SyncCtrl + Clone,
{
    pub buffer: SafeH<Cursor<Vec<u8>>>,
    next_proc: Option<T>,
}

impl<T> WatchOuterImpl<T>
where
    T: SyncCtrl + Clone,
{
    pub fn new() -> Self {
        WatchOuterImpl {
            buffer: SafeH::build(Cursor::new(Vec::with_capacity(10240))),
            next_proc: None,
        }
    }
    pub fn next_pipe(&mut self, assembler: T) {
        self.next_proc = Some(assembler);
    }
}

impl<T> Default for WatchOuterImpl<T>
where
    T: SyncCtrl + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SyncCtrl for WatchOuterImpl<T>
where
    T: SyncCtrl + Clone,
{
    fn stop(&mut self) -> SinkResult<()> {
        if let Some(ref mut next_proc) = self.next_proc {
            next_proc.stop()?;
        }
        Ok(())
    }
}

impl<T> RecSyncSink for WatchOuterImpl<T>
where
    T: SyncCtrl + RecSyncSink + Clone,
{
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        if let Ok(mut buffer) = self.buffer.write() {
            if buffer.get_ref().len() >= 10240 {
                error_data!("buffer full");
            }
            let formatted = extract_formatted(data.data());
            buffer.write_all(formatted.as_bytes()).owe_data()?;
            buffer.write_all(b"\n").owe_data()?;
        }
        if let Some(ref next_proc) = self.next_proc {
            next_proc.send_to_sink(data)?;
        }
        Ok(())
    }
    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        if let Ok(mut buffer) = self.buffer.write() {
            if buffer.get_ref().len() >= 10240 {
                error_data!("buffer full");
            }
            let formatted = extract_formatted(data.data());
            if let Err(e) = buffer.write_all(formatted.as_bytes()) {
                return TrySendStatus::Err(Arc::new(SinkError::from(SinkReason::Sink(format!(
                    "buffer write error: {}",
                    e
                )))));
            }
            if let Err(e) = buffer.write_all(b"\n") {
                return TrySendStatus::Err(Arc::new(SinkError::from(SinkReason::Sink(format!(
                    "buffer write error: {}",
                    e
                )))));
            }
        }
        if let Some(ref next_proc) = self.next_proc {
            return next_proc.try_send_to_sink(data);
        }
        TrySendStatus::Sended
    }
}

fn extract_formatted(record: &DataRecord) -> String {
    if let Some(field) = record.field("formatted") {
        if let Value::Chars(val) = field.get_value() {
            return val.to_string();
        }
        return field.get_value().to_string();
    }
    format!("{:?}", record)
}

#[async_trait]
impl AsyncCtrl for WatchOuterImpl<StubOuter>
where
    Self: Clone,
{
    async fn stop(&mut self) -> SinkResult<()> {
        if let Some(ref mut _next_proc) = self.next_proc {
            // 由于 T: SyncCtrl + Clone，但没有 AsyncCtrl，我们不能调用 stop
            // 这里需要特殊处理
        }
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        if let Some(ref mut _next_proc) = self.next_proc {
            // 由于 T: SyncCtrl + Clone，但没有 AsyncCtrl，我们不能调用 reconnect
            // 这里需要特殊处理
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncRawdatSink for WatchOuterImpl<StubOuter>
where
    Self: Clone,
{
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        if let Ok(mut buffer) = self.buffer.write() {
            if buffer.get_ref().len() >= 10240 {
                error_data!("buffer full");
            }
            buffer.write_fmt(format_args!("{}", data)).owe_data()?;
        }
        // StubOuter 没有实现 AsyncRawdatSink，所以我们不能调用
        Ok(())
    }
    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        if let Ok(mut buffer) = self.buffer.write() {
            if buffer.get_ref().len() >= 10240 {
                error_data!("buffer full");
            }
            buffer.write_all(data).owe_data()?;
        }
        // StubOuter 没有实现 AsyncRawdatSink，所以我们不能调用
        Ok(())
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        // For real-time monitoring, write each record immediately
        // to ensure data is visible to external readers without delay
        for str_data in data {
            self.sink_str(str_data).await?;
        }
        Ok(())
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        // For real-time monitoring, write each record immediately
        // to ensure data is visible to external readers without delay
        for bytes_data in data {
            self.sink_bytes(bytes_data).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnyResult;
    use wp_connector_api::AsyncRawDataSink;

    #[tokio::test]
    async fn test_realtime_monitoring() -> AnyResult<()> {
        let mut monitor = BufferMonitor::new();
        let buffer_ref = monitor.buffer.clone();

        // Write data in batch
        let data: Vec<&str> = vec!["line1", "line2", "line3"];
        monitor.sink_str_batch(data).await?;

        // External reader should see all data immediately
        let buffer = buffer_ref.read().unwrap();
        let content = String::from_utf8_lossy(buffer.get_ref());
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
        assert!(content.contains("line3"));

        Ok(())
    }

    #[tokio::test]
    async fn test_incremental_visibility() -> AnyResult<()> {
        let mut monitor = BufferMonitor::new();
        let buffer_ref = monitor.buffer.clone();

        // Write first record
        monitor.sink_str("first").await?;

        // Should be immediately visible
        {
            let buffer = buffer_ref.read().unwrap();
            let content = String::from_utf8_lossy(buffer.get_ref());
            assert!(content.contains("first"));
            assert!(!content.contains("second"));
        }

        // Write second record
        monitor.sink_str("second").await?;

        // Both should be visible
        {
            let buffer = buffer_ref.read().unwrap();
            let content = String::from_utf8_lossy(buffer.get_ref());
            assert!(content.contains("first"));
            assert!(content.contains("second"));
        }

        Ok(())
    }
}
