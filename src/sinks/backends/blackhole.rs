use async_trait::async_trait;
use wp_connector_api::SinkResult;

#[derive(Clone, Default)]
pub struct BlackHoleSink {
    sink_sleep_ms: u64,
}

impl BlackHoleSink {
    pub fn new(sink_sleep_ms: u64) -> Self {
        Self { sink_sleep_ms }
    }

    async fn maybe_sleep(&self) {
        if self.sink_sleep_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.sink_sleep_ms)).await;
        }
    }
}

#[async_trait]
impl wp_connector_api::AsyncCtrl for BlackHoleSink {
    async fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }
    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl wp_connector_api::AsyncRecordSink for BlackHoleSink {
    async fn sink_record(&mut self, _data: &wp_model_core::model::DataRecord) -> SinkResult<()> {
        self.maybe_sleep().await;
        Ok(())
    }

    async fn sink_records(
        &mut self,
        _data: Vec<std::sync::Arc<wp_model_core::model::DataRecord>>,
    ) -> SinkResult<()> {
        self.maybe_sleep().await;
        Ok(())
    }
}

#[async_trait]
impl wp_connector_api::AsyncRawDataSink for BlackHoleSink {
    async fn sink_str(&mut self, _data: &str) -> SinkResult<()> {
        self.maybe_sleep().await;
        Ok(())
    }
    async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
        self.maybe_sleep().await;
        Ok(())
    }

    async fn sink_str_batch(&mut self, _data: Vec<&str>) -> SinkResult<()> {
        self.maybe_sleep().await;
        Ok(())
    }

    async fn sink_bytes_batch(&mut self, _data: Vec<&[u8]>) -> SinkResult<()> {
        self.maybe_sleep().await;
        Ok(())
    }
}
