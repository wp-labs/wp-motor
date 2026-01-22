use std::sync::{Arc, RwLock};

use tokio::sync::mpsc::{Receiver, Sender};
use wp_connector_api::{SourceBatch, SourceEvent};

pub type AnyResult<T> = anyhow::Result<T>;

pub type EventBatchRecv = Receiver<SourceBatch>;
pub type EventBatchSend = Sender<SourceBatch>;

pub type RawDatRecv = Receiver<SourceEvent>;
pub type RawDatSend = Sender<SourceEvent>;

pub trait Abstract {
    fn abstract_info(&self) -> String;
}

pub trait Build1<T> {
    fn build(args: T) -> Self;
}

pub type SafeH<T> = Arc<RwLock<T>>;
impl<T> Build1<T> for SafeH<T> {
    fn build(args: T) -> Self {
        Arc::new(RwLock::new(args))
    }
}
