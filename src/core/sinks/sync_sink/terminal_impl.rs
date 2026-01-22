//! SinkTerminal 的实现
use super::traits::{RecSyncSink, SyncCtrl, TrySendStatus};
use super::types::SinkTerminal;
use crate::sinks::{ProcMeta, SinkEndpoint, SinkRecUnit};
use std::sync::Arc;
use wp_connector_api::SinkResult;
use wp_model_core::model::DataField;
use wp_model_core::model::DataRecord;
use wpl::PkgID;

impl SinkTerminal {
    /// 发送记录数据到 sink
    pub fn send_record(
        &self,
        id: PkgID,
        meta: ProcMeta,
        record: Arc<DataRecord>,
    ) -> SinkResult<()> {
        let unit = SinkRecUnit::new(id, meta, record);
        self.send_to_sink(unit)
    }

    /// 尝试发送记录数据到 sink
    pub fn try_send_record(
        &self,
        id: PkgID,
        meta: ProcMeta,
        record: Arc<DataRecord>,
    ) -> TrySendStatus {
        let unit = SinkRecUnit::new(id, meta, record);
        self.try_send_to_sink(unit)
    }

    /// 发送原始数据到 sink
    pub fn send_raw(&self, id: PkgID, raw: String) -> SinkResult<()> {
        match self {
            SinkTerminal::BlackHole(_n) => Ok(()),
            SinkTerminal::Debug(_v) => {
                println!("{}", raw);
                Ok(())
            }
            _ => {
                let mut record = DataRecord::default();
                record.append(DataField::from_chars("raw", &raw));
                let unit =
                    SinkRecUnit::new(id, ProcMeta::Rule("raw".to_string()), Arc::new(record));
                self.send_to_sink(unit)
            }
        }
    }

    /// 尝试发送原始数据到 sink
    pub fn try_send_raw(&self, id: PkgID, raw: String) -> TrySendStatus {
        match self {
            SinkTerminal::BlackHole(_n) => TrySendStatus::Sended,
            SinkTerminal::Debug(_v) => {
                println!("{}", raw);
                TrySendStatus::Sended
            }
            _ => {
                let mut record = DataRecord::default();
                record.append(DataField::from_chars("raw", &raw));
                let unit =
                    SinkRecUnit::new(id, ProcMeta::Rule("raw".to_string()), Arc::new(record));
                self.try_send_to_sink(unit)
            }
        }
    }
}

impl RecSyncSink for SinkTerminal {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        match self {
            SinkTerminal::Channel(s) => s.send_to_sink(data),
            SinkTerminal::BlackHole(n) => n.send_to_sink(data),
            SinkTerminal::Debug(v) => v.send_to_sink(data),
            SinkTerminal::Storage(s) => {
                // Storage 类型：数据应该发送到存储端点
                println!("SinkTerminal::Storage: called");
                match s {
                    SinkEndpoint::Buffer(adapter) => {
                        // 对于 Buffer adapter，需要调用 RecSyncSink trait
                        println!("SinkTerminal::Storage: Buffer endpoint");
                        adapter.send_to_sink(data)
                    }
                    SinkEndpoint::File(file_sink) => {
                        println!("SinkTerminal::Storage: File endpoint");
                        file_sink.send_to_sink(data)
                    }
                    SinkEndpoint::WFile(wfile_sink) => {
                        println!("SinkTerminal::Storage: WFile endpoint");
                        wfile_sink.send_to_sink(data)
                    }
                    SinkEndpoint::View(view) => {
                        println!("SinkTerminal::Storage: View endpoint");
                        view.send_to_sink(data)
                    }
                    SinkEndpoint::Null => {
                        println!("SinkTerminal::Storage: Null endpoint");
                        Ok(())
                    }
                }
            }
        }
    }

    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        match self {
            SinkTerminal::Channel(s) => s.try_send_to_sink(data),
            SinkTerminal::BlackHole(n) => n.try_send_to_sink(data),
            SinkTerminal::Debug(v) => v.try_send_to_sink(data),
            SinkTerminal::Storage(s) => {
                // Storage 类型：数据应该发送到存储端点
                match s {
                    SinkEndpoint::Buffer(adapter) => {
                        // 对于 Buffer adapter，需要调用 RecSyncSink trait
                        adapter.try_send_to_sink(data)
                    }
                    SinkEndpoint::File(file_sink) => file_sink.try_send_to_sink(data),
                    SinkEndpoint::WFile(wfile_sink) => wfile_sink.try_send_to_sink(data),
                    SinkEndpoint::View(view) => view.try_send_to_sink(data),
                    SinkEndpoint::Null => TrySendStatus::Sended,
                }
            }
        }
    }
}

impl SyncCtrl for SinkTerminal {
    fn stop(&mut self) -> SinkResult<()> {
        match self {
            SinkTerminal::Channel(_s) => {
                // Channel 类型：无法直接停止，可能需要关闭 sender
                Ok(())
            }
            SinkTerminal::BlackHole(_n) => {
                // BlackHole 不需要停止
                Ok(())
            }
            SinkTerminal::Debug(_v) => {
                // Debug 不需要停止
                Ok(())
            }
            SinkTerminal::Storage(_s) => {
                // Storage 类型：根据具体实现处理
                Ok(())
            }
        }
    }
}
