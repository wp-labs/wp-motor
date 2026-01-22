use crate::{
    core::sinks::sync_sink::{RecSyncSink, TrySendStatus, traits::SyncCtrl},
    sinks::prelude::*,
};
use async_trait::async_trait;

use crate::sinks::SinkRecUnit;
use wp_connector_api::SinkResult;

#[derive(Clone)]
pub struct StubOuter {}

impl SyncCtrl for StubOuter {
    fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncCtrl for StubOuter {
    async fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl RecSyncSink for StubOuter {
    fn send_to_sink(&self, _data: SinkRecUnit) -> SinkResult<()> {
        Ok(())
    }
    fn try_send_to_sink(&self, _data: SinkRecUnit) -> TrySendStatus {
        TrySendStatus::Sended
    }
}

#[async_trait]
impl AsyncRawdatSink for StubOuter {
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
