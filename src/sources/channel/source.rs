use super::chunk_reader::ChunkedLineReader;
use crate::sources::event_id::next_event_id;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose;
use bytes::Bytes;
use orion_conf::{ErrorWith, UvsConfFrom};
use orion_error::ToStructError;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
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

pub struct ChannelSource {
    pub(super) key: String,
    pub(super) base_tags: Tags,
    sender: Sender<String>,
    recevr: Receiver<String>,
}
