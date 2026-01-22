use super::chunk_reader::ChunkedLineReader;
use crate::sources::event_id::next_event_id;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose;
use bytes::Bytes;
use orion_conf::{ErrorWith, UvsConfFrom};
use orion_error::ToStructError;
use std::sync::Arc;
use wp_connector_api::{
    DataSource, SourceBatch, SourceError, SourceEvent, SourceReason, SourceResult, Tags,
};
use wp_parse_api::RawData;

#[derive(Debug, Clone)]
pub enum FileEncoding {
    Text,
    Base64,
    Hex,
}

const DEFAULT_BATCH_LINES: usize = 128;
const DEFAULT_BATCH_BYTES: usize = 400 * 1024;
const DEFAULT_CHUNK_BYTES: usize = 64 * 1024;
const MIN_CHUNK_BYTES: usize = 4 * 1024;
const MAX_CHUNK_BYTES: usize = 128 * 1024;

pub struct FileSource {
    pub(super) key: String,
    pub(super) reader: ChunkedLineReader,
    pub(super) encode: FileEncoding,
    pub(super) base_tags: Tags,
    pub(super) batch_lines: usize,
    pub(super) batch_bytes_budget: usize,
}

impl FileSource {
    pub async fn new(
        key: String,
        path: &str,
        encode: FileEncoding,
        mut tags: Tags,
        range_start: u64,
        range_end: Option<u64>,
    ) -> SourceResult<Self> {
        use std::path::Path;
        let file_path = Path::new(path);
        if !file_path.exists() {
            return Err(
                SourceReason::from_conf(format!(" {} not exists", file_path.display())).to_err(),
            );
        }
        let mut file = tokio::fs::File::open(file_path)
            .await
            .map_err(|e| SourceError::from(SourceReason::Disconnect(e.to_string())))
            .with(file_path)
            .want("open source file")?;
        use std::io::SeekFrom;
        use tokio::io::AsyncSeekExt;
        file.seek(SeekFrom::Start(range_start))
            .await
            .map_err(|e| SourceError::from(SourceReason::Disconnect(e.to_string())))
            .with(file_path)
            .want("seek to posion")?;
        tags.set("access_source", path.to_string());
        let batch_lines = DEFAULT_BATCH_LINES;
        let batch_bytes_budget = DEFAULT_BATCH_BYTES;
        let chunk_bytes = DEFAULT_CHUNK_BYTES.clamp(MIN_CHUNK_BYTES, MAX_CHUNK_BYTES);
        let limit = range_end.map(|end| end.saturating_sub(range_start));
        let reader = ChunkedLineReader::new(file, chunk_bytes, limit);
        Ok(Self {
            key,
            reader,
            encode,
            base_tags: tags,
            batch_lines,
            batch_bytes_budget,
        })
    }

    fn payload_from_line(encode: &FileEncoding, line: Vec<u8>) -> SourceResult<RawData> {
        match encode {
            FileEncoding::Text => Ok(RawData::Bytes(Bytes::from(line))),
            FileEncoding::Base64 => {
                let s = std::str::from_utf8(&line).map_err(|_| {
                    SourceError::from(SourceReason::SupplierError(
                        "invalid utf8 in base64 text".to_string(),
                    ))
                })?;
                let val = general_purpose::STANDARD.decode(s.trim()).map_err(|_| {
                    SourceError::from(SourceReason::SupplierError(
                        "base64 decode error".to_string(),
                    ))
                })?;
                Ok(RawData::Bytes(Bytes::from(val)))
            }
            FileEncoding::Hex => {
                let s = std::str::from_utf8(&line).map_err(|_| {
                    SourceError::from(SourceReason::SupplierError(
                        "invalid utf8 in hex text".to_string(),
                    ))
                })?;
                let val = hex::decode(s.trim()).map_err(|_| {
                    SourceError::from(SourceReason::SupplierError("hex decode error".to_string()))
                })?;
                Ok(RawData::Bytes(Bytes::from(val)))
            }
        }
    }

    fn make_event(&self, payload: RawData) -> SourceEvent {
        SourceEvent::new(
            next_event_id(),
            &self.key,
            payload,
            Arc::new(self.base_tags.clone()),
        )
    }

    pub fn identifier(&self) -> String {
        self.key.clone()
    }
}

#[async_trait]
impl DataSource for FileSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        let mut batch = SourceBatch::with_capacity(self.batch_lines);
        let mut produced_rows = 0usize;
        let mut used_bytes = 0usize;
        loop {
            match self.reader.next_line().await? {
                Some(line) => {
                    used_bytes = used_bytes.saturating_add(line.len());
                    let payload = Self::payload_from_line(&self.encode, line)?;
                    batch.push(self.make_event(payload));
                    produced_rows += 1;
                    if produced_rows >= self.batch_lines
                        || (self.batch_bytes_budget > 0 && used_bytes >= self.batch_bytes_budget)
                    {
                        break;
                    }
                }
                None => {
                    if batch.is_empty() {
                        return Err(SourceError::from(SourceReason::EOF));
                    }
                    break;
                }
            }
        }
        Ok(batch)
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        None
    }

    fn can_try_receive(&mut self) -> bool {
        false
    }

    fn identifier(&self) -> String {
        self.key.clone()
    }
}
