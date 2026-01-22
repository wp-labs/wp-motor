use crate::sinks::prelude::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio_async_drop::tokio_async_drop;
use wp_connector_api::{SinkError, SinkReason, SinkResult};
use wp_model_core::model::DataRecord;

const RESCUE_FLUSH_INTERVAL: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RescuePayload {
    Record { record: DataRecord },
    Raw { raw: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescueEntry {
    #[serde(default = "RescueEntry::default_version")]
    version: u8,
    #[serde(flatten)]
    payload: RescuePayload,
}

impl RescueEntry {
    pub const CURRENT_VERSION: u8 = 1;

    const fn default_version() -> u8 {
        Self::CURRENT_VERSION
    }

    pub fn record(record: &DataRecord) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            payload: RescuePayload::Record {
                record: record.clone(),
            },
        }
    }

    pub fn raw_line(raw: String) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            payload: RescuePayload::Raw { raw },
        }
    }

    pub fn parse(line: &str) -> serde_json::Result<Self> {
        serde_json::from_str(line)
    }

    pub fn payload(&self) -> &RescuePayload {
        &self.payload
    }

    pub fn into_payload(self) -> RescuePayload {
        self.payload
    }
}

pub struct RescueFileSink {
    path: String,
    writer: BufWriter<tokio::fs::File>,
    proc_cnt: usize,
}

impl RescueFileSink {
    pub async fn new(out_path: &str) -> AnyResult<Self> {
        if let Some(parent) = Path::new(out_path).parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(out_path)
            .await?;
        Ok(Self {
            path: out_path.to_string(),
            writer: BufWriter::with_capacity(102_400, file),
            proc_cnt: 0,
        })
    }

    fn sink_err<E: std::fmt::Display>(err: E) -> SinkError {
        SinkError::from(SinkReason::Sink(err.to_string()))
    }

    async fn write_entry(&mut self, entry: &RescueEntry) -> SinkResult<()> {
        let mut line = serde_json::to_vec(entry).map_err(Self::sink_err)?;
        line.push(b'\n');
        self.writer.write_all(&line).await.map_err(Self::sink_err)?;
        self.proc_cnt += 1;
        if self.proc_cnt.is_multiple_of(RESCUE_FLUSH_INTERVAL) {
            self.writer.flush().await.map_err(Self::sink_err)?;
        }
        Ok(())
    }
}

impl Drop for RescueFileSink {
    fn drop(&mut self) {
        tokio_async_drop!({
            let _ = self.writer.flush().await;
        });
        if let Some(new_path) = self.path.strip_suffix(".lock")
            && let Err(e) = fs::rename(&self.path, new_path)
        {
            error_data!("unlock rescue file failed: {}", e);
        }
    }
}

#[async_trait]
impl AsyncCtrl for RescueFileSink {
    async fn stop(&mut self) -> SinkResult<()> {
        self.writer.flush().await.map_err(Self::sink_err)?;
        if let Some(new_path) = self.path.strip_suffix(".lock")
            && let Err(e) = fs::rename(&self.path, new_path)
        {
            error_data!("unlock rescue file on stop failed: {}", e);
        }
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncRecordSink for RescueFileSink {
    async fn sink_record(&mut self, data: &DataRecord) -> SinkResult<()> {
        let entry = RescueEntry::record(data);
        self.write_entry(&entry).await
    }

    async fn sink_records(&mut self, data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
        for record in data.iter() {
            self.sink_record(record).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncRawdatSink for RescueFileSink {
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        let entry = RescueEntry::raw_line(data.to_string());
        self.write_entry(&entry).await
    }

    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        let entry = RescueEntry::raw_line(String::from_utf8_lossy(data).into_owned());
        self.write_entry(&entry).await
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        for line in data {
            self.sink_str(line).await?;
        }
        Ok(())
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        for item in data {
            self.sink_bytes(item).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnyResult;
    use tempfile::tempdir;
    use wp_connector_api::{AsyncCtrl, AsyncRecordSink};
    use wp_model_core::model::DataField;

    #[test]
    fn rescue_entry_roundtrip_record() {
        let mut record = DataRecord::default();
        record.append(DataField::from_chars("field", "value"));
        let entry = RescueEntry::record(&record);
        let line = serde_json::to_string(&entry).expect("serialize entry");
        let parsed = RescueEntry::parse(&line).expect("parse entry");
        match parsed.into_payload() {
            RescuePayload::Record {
                record: parsed_record,
            } => {
                assert_eq!(parsed_record, record);
            }
            _ => panic!("expected record payload"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn rescue_sink_writes_structured_lines() -> AnyResult<()> {
        let temp = tempdir()?;
        let path = temp
            .path()
            .join("rescue/groupA/test_sink-2024-01-01_00:00:00.dat.lock");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut sink = RescueFileSink::new(path.to_str().unwrap()).await?;
        let mut record = DataRecord::default();
        record.append(DataField::from_chars("key", "value"));
        AsyncRecordSink::sink_record(&mut sink, &record).await?;
        AsyncCtrl::stop(&mut sink).await?;

        let dat_path = temp
            .path()
            .join("rescue/groupA/test_sink-2024-01-01_00:00:00.dat");
        assert!(dat_path.exists(), "rescue file should be unlocked");
        let content = std::fs::read_to_string(dat_path)?;
        let first_line = content.lines().next().unwrap_or("");
        let parsed = RescueEntry::parse(first_line).expect("parse stored entry");
        match parsed.into_payload() {
            RescuePayload::Record {
                record: parsed_record,
            } => {
                assert_eq!(parsed_record, record);
            }
            _ => panic!("expected record payload"),
        }
        Ok(())
    }
}
