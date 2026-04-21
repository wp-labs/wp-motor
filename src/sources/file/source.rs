use super::chunk_reader::ChunkedLineReader;
use crate::sources::event_id::next_event_id;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose;
use bytes::Bytes;
use orion_conf::{ErrorWith, UvsFrom};
use orion_error::ToStructError;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::task::JoinHandle;
use wp_connector_api::{DataSource, SourceBatch, SourceEvent, SourceReason, SourceResult, Tags};
use wp_model_core::raw::RawData;

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
            return Err(SourceReason::from_conf()
                .to_err()
                .with_detail(format!("source file not found: {}", file_path.display()))
                .with(file_path));
        }
        let mut file = tokio::fs::File::open(file_path)
            .await
            .map_err(|e| {
                SourceReason::Disconnect("open source file failed".to_string())
                    .to_err()
                    .with_std_source(e)
            })
            .with(file_path)
            .doing("open source file")?;
        use std::io::SeekFrom;
        use tokio::io::AsyncSeekExt;
        file.seek(SeekFrom::Start(range_start))
            .await
            .map_err(|e| {
                SourceReason::Disconnect("seek source file failed".to_string())
                    .to_err()
                    .with_std_source(e)
            })
            .with(file_path)
            .doing("seek to position")?;
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
                let s = std::str::from_utf8(&line).map_err(|e| {
                    SourceReason::SupplierError("invalid utf8 in base64 text".to_string())
                        .err_source(e)
                })?;
                let val = general_purpose::STANDARD.decode(s.trim()).map_err(|e| {
                    SourceReason::SupplierError("base64 decode error".to_string()).err_source(e)
                })?;
                Ok(RawData::Bytes(Bytes::from(val)))
            }
            FileEncoding::Hex => {
                let s = std::str::from_utf8(&line).map_err(|e| {
                    SourceReason::SupplierError("invalid utf8 in hex text".to_string())
                        .err_source(e)
                })?;
                let val = hex::decode(s.trim()).map_err(|e| {
                    SourceReason::SupplierError("hex decode error".to_string()).err_source(e)
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

pub struct MultiFileSource {
    key: String,
    paths: VecDeque<String>,
    encode: FileEncoding,
    tags: Tags,
    instances: usize,
    current_rx: Option<UnboundedReceiver<ParallelSourceMsg>>,
    current_tasks: Vec<JoinHandle<()>>,
    active_tasks: usize,
}

impl MultiFileSource {
    pub fn new(
        key: String,
        paths: Vec<String>,
        encode: FileEncoding,
        tags: Tags,
        instances: usize,
    ) -> Self {
        Self {
            key,
            paths: paths.into(),
            encode,
            tags,
            instances,
            current_rx: None,
            current_tasks: Vec::new(),
            active_tasks: 0,
        }
    }

    async fn launch_next_file(&mut self) -> SourceResult<bool> {
        let Some(path) = self.paths.pop_front() else {
            return Ok(false);
        };
        let ranges = compute_file_ranges(Path::new(&path), self.instances)
            .map_err(|e| {
                SourceReason::Disconnect("compute file ranges failed".to_string())
                    .to_err()
                    .with_std_source(e)
            })
            .with(path.as_str())
            .doing("compute source file ranges")?;
        let (tx, rx) = unbounded_channel();
        let shard_total = ranges.len();
        let mut tasks = Vec::with_capacity(shard_total);
        for (idx, (start, end)) in ranges.into_iter().enumerate() {
            let shard_key = if shard_total > 1 {
                format!("{}-{}", self.key, idx + 1)
            } else {
                self.key.clone()
            };
            let mut source = FileSource::new(
                shard_key,
                &path,
                self.encode.clone(),
                self.tags.clone(),
                start,
                end,
            )
            .await?;
            let tx = tx.clone();
            tasks.push(tokio::spawn(async move {
                loop {
                    match source.receive().await {
                        Ok(batch) => {
                            if tx.send(ParallelSourceMsg::Batch(batch)).is_err() {
                                break;
                            }
                        }
                        Err(err) if matches!(err.reason(), SourceReason::EOF) => {
                            let _ = tx.send(ParallelSourceMsg::Done);
                            break;
                        }
                        Err(err) => {
                            let _ = tx.send(ParallelSourceMsg::Err(err));
                            break;
                        }
                    }
                }
            }));
        }
        drop(tx);
        self.current_rx = Some(rx);
        self.current_tasks = tasks;
        self.active_tasks = shard_total;
        Ok(true)
    }

    async fn clear_finished_group(&mut self) {
        for task in self.current_tasks.drain(..) {
            let _ = task.await;
        }
        self.current_rx = None;
        self.active_tasks = 0;
    }

    async fn abort_current_group(&mut self) {
        for task in &self.current_tasks {
            task.abort();
        }
        self.clear_finished_group().await;
    }
}

enum ParallelSourceMsg {
    Batch(SourceBatch),
    Done,
    Err(SourceError),
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

#[async_trait]
impl DataSource for MultiFileSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        loop {
            if self.current_rx.is_none() && !self.launch_next_file().await? {
                return Err(SourceError::from(SourceReason::EOF));
            }

            let msg = match self.current_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => continue,
            };
            match msg {
                Some(ParallelSourceMsg::Batch(batch)) => return Ok(batch),
                Some(ParallelSourceMsg::Done) => {
                    self.active_tasks = self.active_tasks.saturating_sub(1);
                    if self.active_tasks == 0 {
                        self.clear_finished_group().await;
                    }
                }
                Some(ParallelSourceMsg::Err(err)) => {
                    self.abort_current_group().await;
                    return Err(err);
                }
                None => {
                    if self.active_tasks == 0 {
                        self.clear_finished_group().await;
                        continue;
                    }
                    self.abort_current_group().await;
                    return Err(SourceError::from(SourceReason::Disconnect(
                        "file source worker channel closed unexpectedly".to_string(),
                    )));
                }
            }
        }
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

    async fn close(&mut self) -> SourceResult<()> {
        self.abort_current_group().await;
        Ok(())
    }
}

pub(super) fn compute_file_ranges(
    path: &Path,
    instances: usize,
) -> std::io::Result<Vec<(u64, Option<u64>)>> {
    let size = std::fs::metadata(path)?.len();
    if size == 0 || instances <= 1 {
        return Ok(vec![(0, None)]);
    }
    let chunk = size.div_ceil(instances as u64);
    let mut starts = vec![0u64];
    for i in 1..instances {
        let target = chunk.saturating_mul(i as u64);
        if target >= size {
            break;
        }
        let aligned = align_to_next_line(path, target, size)?;
        if aligned < size {
            starts.push(aligned);
        }
    }
    starts.sort_unstable();
    starts.dedup();
    let mut ranges = Vec::with_capacity(starts.len());
    for (idx, &start) in starts.iter().enumerate() {
        let end = if idx + 1 < starts.len() {
            Some(starts[idx + 1])
        } else {
            None
        };
        ranges.push((start, end));
    }
    Ok(ranges)
}

fn align_to_next_line(path: &Path, offset: u64, file_size: u64) -> std::io::Result<u64> {
    use std::io::{Read, Seek, SeekFrom};
    if offset == 0 {
        return Ok(0);
    }
    let mut file = std::fs::File::open(path)?;
    let seek_pos = offset.saturating_sub(1);
    file.seek(SeekFrom::Start(seek_pos))?;
    let mut pos = seek_pos;
    let mut buf = [0u8; 4096];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            return Ok(file_size);
        }
        for &b in &buf[..read] {
            pos += 1;
            if b == b'\n' {
                return Ok(pos);
            }
            if pos >= file_size {
                return Ok(file_size);
            }
        }
    }
}
