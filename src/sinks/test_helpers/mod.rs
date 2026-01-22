//! Test helpers for sinks module

use async_trait::async_trait;
use std::sync::Arc;
use wp_connector_api::SinkResult;
use wp_connector_api::{AsyncCtrl, AsyncRawDataSink, AsyncRecordSink};
use wp_model_core::model::DataRecord;

/// Simple MockSink implementation for testing
/// This is a lightweight mock that doesn't require mockall
#[derive(Default)]
pub struct MockSink {
    /// Track number of calls for testing
    #[cfg(test)]
    pub call_count: std::sync::atomic::AtomicUsize,
}

impl Clone for MockSink {
    fn clone(&self) -> Self {
        MockSink {
            #[cfg(test)]
            call_count: std::sync::atomic::AtomicUsize::new(
                self.call_count.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl MockSink {
    pub fn new() -> Self {
        MockSink {
            #[cfg(test)]
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl AsyncCtrl for MockSink {
    async fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncRecordSink for MockSink {
    async fn sink_record(&mut self, _data: &DataRecord) -> SinkResult<()> {
        Ok(())
    }

    async fn sink_records(&mut self, _data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncRawDataSink for MockSink {
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
