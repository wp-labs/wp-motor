use crate::core::sinks::sync_sink::{RecSyncSink, TrySendStatus};
use crate::sinks::prelude::*;
use crate::{core::sinks::sync_sink::traits::SyncCtrl, sinks::pdm_outer::TDMDataAble};

use async_trait::async_trait;
use orion_error::ErrorOwe;
use wp_data_fmt::{DataFormat, FormatType};
use wp_model_core::model::fmt_def::TextFmt;
use wp_parse_api::RawData;
use wpl::generator::{CSVGenFmt, JsonGenFmt, KVGenFmt, ProtoGenFmt, RAWGenFmt};

use crate::sinks::SinkRecUnit;
use crate::types::AnyResult;
use std::sync::Arc;
use wp_connector_api::SinkResult;
use wp_model_core::model::{DataField, DataRecord};

pub fn fds_fmt_proc(fmt: TextFmt, line: DataRecord) -> AnyResult<RawData> {
    let formatter = FormatType::from(&fmt);
    let res = RawData::String(format!("{}\n", formatter.format_record(&line)));

    Ok(res)
}

pub fn gen_fmt_dat(fmt: TextFmt, line: FmtFieldVec) -> AnyResult<RawData> {
    let data = match fmt {
        TextFmt::Json => RawData::String(format!("{}\n", JsonGenFmt(&line))),
        TextFmt::Kv => RawData::String(format!("{}\n", KVGenFmt(&line))),
        TextFmt::Show => RawData::String(format!("{:?}\n", line)),
        TextFmt::Csv => RawData::String(format!("{}\n", CSVGenFmt(&line))),
        TextFmt::Raw => RawData::String(format!("{}\n", RAWGenFmt(&line))),
        TextFmt::Proto => {
            unimplemented!("unsupport proto buf gen")
        }
        TextFmt::ProtoText => RawData::String(format!("{}\n", ProtoGenFmt(&line))),
    };
    Ok(data)
}

pub struct AsyncFormatter<T>
where
    T: AsyncCtrl + AsyncRawdatSink,
{
    fmt: TextFmt,
    next_proc: Option<T>,
}

impl<T> AsyncFormatter<T>
where
    T: AsyncCtrl + AsyncRawdatSink,
{
    pub fn next_pipe(&mut self, assembler: T) {
        self.next_proc = Some(assembler);
    }
}

#[async_trait]
impl<T> AsyncCtrl for AsyncFormatter<T>
where
    T: AsyncCtrl + AsyncRawdatSink + Send,
{
    async fn stop(&mut self) -> SinkResult<()> {
        if let Some(ref mut next_proc) = self.next_proc {
            next_proc.stop().await?;
        }
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        if let Some(ref mut next_proc) = self.next_proc {
            next_proc.reconnect().await?;
        }

        Ok(())
    }
}

#[async_trait]
impl<T> AsyncRecordSink for AsyncFormatter<T>
where
    T: AsyncCtrl + AsyncRawdatSink + Send,
{
    async fn sink_record(&mut self, data: &DataRecord) -> SinkResult<()> {
        if let Some(ref mut next_proc) = self.next_proc {
            let data: RawData = self.fmt.cov_data(data.clone()).owe_data()?;
            match data {
                RawData::String(data_str) => {
                    next_proc.sink_str(&data_str).await?;
                }
                RawData::Bytes(data_bytes) => {
                    next_proc.sink_bytes(&data_bytes).await?;
                }
                RawData::ArcBytes(data_bytes) => {
                    next_proc.sink_bytes(&data_bytes).await?;
                }
            }
        }
        Ok(())
    }

    async fn sink_records(&mut self, data: Vec<std::sync::Arc<DataRecord>>) -> SinkResult<()> {
        for record in data {
            self.sink_record(&record).await?;
        }
        Ok(())
    }
}
// Blanket impl of AsyncSink is provided by wp-sink-api; no explicit impl needed here.

#[async_trait]
impl<T> AsyncRawdatSink for AsyncFormatter<T>
where
    T: AsyncCtrl + AsyncRawdatSink + Send,
{
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        if let Some(ref mut next) = self.next_proc {
            match self.fmt {
                TextFmt::Proto => {
                    unimplemented!("unsupport protobuf format")
                }
                _ => {
                    return next.sink_str(data).await;
                }
            }
        }
        Ok(())
    }
    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        if let Some(ref mut next) = self.next_proc {
            return next.sink_bytes(data).await;
        }
        Ok(())
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        for str_data in data {
            self.sink_str(str_data).await?;
        }
        Ok(())
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        for bytes_data in data {
            self.sink_bytes(bytes_data).await?;
        }
        Ok(())
    }
}

impl<T> AsyncFormatter<T>
where
    T: AsyncCtrl + AsyncRawdatSink,
{
    pub fn new(fmt: TextFmt) -> Self {
        AsyncFormatter {
            fmt,
            next_proc: None,
        }
    }
}

#[derive(Clone)]
pub struct FormatAdapter<T>
where
    T: SyncCtrl + RecSyncSink,
{
    fmt: TextFmt,
    next_proc: Option<T>,
}

impl<T> SyncCtrl for FormatAdapter<T>
where
    T: SyncCtrl + RecSyncSink,
{
    fn stop(&mut self) -> SinkResult<()> {
        if let Some(ref mut next_proc) = self.next_proc {
            next_proc.stop()?;
        }
        Ok(())
    }
}

impl<T> RecSyncSink for FormatAdapter<T>
where
    T: SyncCtrl + RecSyncSink,
{
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        println!("FormatAdapter: send_to_sink called");
        // 直接格式化记录数据
        let formatted = FormatType::from(&self.fmt).format_record(data.data());
        println!("FormatAdapter: formatted data = {}", formatted);

        // 创建一个新的记录，包含格式化后的字符串
        let formatted_record =
            DataRecord::from(vec![DataField::from_chars("formatted", &formatted)]);

        // 传递给下一个处理器
        if let Some(ref next_proc) = self.next_proc {
            println!("FormatAdapter: passing to next_proc");
            let rec_unit =
                SinkRecUnit::new(*data.id(), data.meta().clone(), Arc::new(formatted_record));
            next_proc.send_to_sink(rec_unit)?;
        } else {
            println!("FormatAdapter: No next_proc!");
        }
        Ok(())
    }
    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        // 直接格式化记录数据
        let formatted = FormatType::from(&self.fmt).format_record(data.data());

        // 创建一个新的记录，包含格式化后的字符串
        let formatted_record =
            DataRecord::from(vec![DataField::from_chars("formatted", &formatted)]);

        // 传递给下一个处理器
        if let Some(ref next_proc) = self.next_proc {
            let rec_unit =
                SinkRecUnit::new(*data.id(), data.meta().clone(), Arc::new(formatted_record));
            return next_proc.try_send_to_sink(rec_unit);
        }
        TrySendStatus::Sended
    }
}

impl<T> FormatAdapter<T>
where
    T: SyncCtrl + RecSyncSink,
{
    pub fn new(fmt: TextFmt) -> Self {
        FormatAdapter {
            fmt,
            next_proc: None,
        }
    }
    pub fn next_pipe(&mut self, assembler: T) {
        self.next_proc = Some(assembler);
    }
}
