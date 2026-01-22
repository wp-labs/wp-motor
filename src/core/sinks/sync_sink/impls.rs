//! BlackHole 和 DebugView 等实现

use super::BlackHoleSink;
use super::traits::{RecSyncSink, SyncCtrl, TrySendStatus};
use super::types::DebugView;
use crate::sinks::SinkRecUnit;
use async_trait::async_trait;
use wp_connector_api::SinkResult;

#[async_trait]
impl RecSyncSink for BlackHoleSink {
    fn send_to_sink(&self, _data: SinkRecUnit) -> SinkResult<()> {
        // BlackHole 丢弃所有数据，直接返回成功
        Ok(())
    }

    fn try_send_to_sink(&self, _data: SinkRecUnit) -> TrySendStatus {
        TrySendStatus::Sended
    }

    fn send_to_sink_batch(&self, _data: Vec<SinkRecUnit>) -> SinkResult<()> {
        // BlackHole 丢弃所有数据，直接返回成功
        Ok(())
    }

    fn try_send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> Vec<TrySendStatus> {
        // 返回全部成功
        vec![TrySendStatus::Sended; data.len()]
    }
}

impl SyncCtrl for BlackHoleSink {
    fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

/*
#[async_trait]
impl FFVSyncSink for BlackHoleSink {
    fn send_ffv_to_sink(&self, _data: SinkFFVUnit) -> SinkResult<()> {
        Ok(())
    }

    fn try_send_ffv_to_sink(&self, _data: SinkFFVUnit) -> TrySendStatus {
        TrySendStatus::Sended
    }

    fn send_ffv_batch_to_sink(&self, _data: Vec<SinkFFVUnit>) -> SinkResult<()> {
        Ok(())
    }

    fn try_send_ffv_batch_to_sink(&self, data: Vec<SinkFFVUnit>) -> Vec<TrySendStatus> {
        vec![TrySendStatus::Sended; data.len()]
    }
}
*/

#[async_trait]
impl RecSyncSink for DebugView {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        let msg = format!(
            "Record[id={}, meta={:?}]: {:?}",
            data.id(),
            data.meta(),
            data.data()
        );
        self.send(msg);
        Ok(())
    }

    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        let msg = format!(
            "Record[id={}, meta={:?}]: {:?}",
            data.id(),
            data.meta(),
            data.data()
        );
        self.send(msg);
        TrySendStatus::Sended
    }
}

/*
#[async_trait]
impl FFVSyncSink for DebugView {
    fn send_ffv_to_sink(&self, data: SinkFFVUnit) -> SinkResult<()> {
        let msg = format!("FFV[id={}]: {:?}", data.id(), data.data());
        self.send(msg);
        Ok(())
    }

    fn try_send_ffv_to_sink(&self, data: SinkFFVUnit) -> TrySendStatus {
        let msg = format!("FFV[id={}]: {:?}", data.id(), data.data());
        self.send(msg);
        TrySendStatus::Sended
    }
}
*/
