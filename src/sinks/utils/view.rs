use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::core::sinks::sync_sink::{RecSyncSink, TrySendStatus};
use crate::sinks::backends::file::FileSink;
use crate::sinks::decorators::sync_pipeline::SyncFrame;
use std::sync::Arc;
use wp_connector_api::SinkResult;
use wp_model_core::model::format::MetaFmt;

use crate::sinks::SinkRecUnit;
use async_trait::async_trait;

use super::formatter::FormatAdapter;

#[derive(Clone, Default)]
pub struct DebugViewer {}

#[async_trait]
impl RecSyncSink for DebugViewer {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        println!("\n{:=^120}", "");
        // 直接处理 DataRecord，因为 SinkRecUnit 包含的是 Arc<DataRecord>
        let record = data.data();
        println!("{}", record);
        print!("{}", MetaFmt(record.as_ref()));
        Ok(())
    }

    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        // 与 send_to_sink 语义一致；try 版不阻塞
        match self.send_to_sink(data) {
            Ok(()) => TrySendStatus::Sended,
            Err(e) => TrySendStatus::Err(Arc::new(e)),
        }
    }

    fn send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> SinkResult<()> {
        for unit in data {
            self.send_to_sink(unit)?;
        }
        Ok(())
    }

    fn try_send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> Vec<TrySendStatus> {
        data.into_iter()
            .map(|unit| self.try_send_to_sink(unit))
            .collect()
    }
}

impl SyncCtrl for DebugViewer {}

pub type ViewOuter = SyncFrame<DebugViewer, FormatAdapter<FileSink>>;
