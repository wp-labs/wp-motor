use crate::sources::event_id::next_event_id;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender, error::TryRecvError};
use tokio::task::JoinHandle;
use wp_connector_api::{
    ControlEvent, CtrlRx, DataSource, SourceBatch, SourceError, SourceEvent, SourceReason,
    SourceResult, Tags,
};
use wp_parse_api::RawData;

use super::factory::unregister_channel_sender;

pub(super) const DEFAULT_CHANNEL_BATCH: usize = 128;

/// 内存 channel 数据源，允许通过 Sender 注入行数据供解析链条消费。
pub struct ChannelSource {
    pub(super) key: String,
    pub(super) base_tags: Tags,
    sender: Option<Sender<String>>,
    recevr: Receiver<String>,
    batch_limit: usize,
    stop_task: Option<JoinHandle<()>>,
}

impl ChannelSource {
    /// 使用手动提供的 sender/receiver 构造 channel source。
    pub fn from_parts(
        key: impl Into<String>,
        tags: Tags,
        sender: Sender<String>,
        recevr: Receiver<String>,
    ) -> Self {
        Self {
            key: key.into(),
            base_tags: tags,
            sender: Some(sender),
            recevr,
            batch_limit: DEFAULT_CHANNEL_BATCH,
            stop_task: None,
        }
    }

    /// 创建具备内部 channel 的 source，自动根据容量生成 Sender/Receiver。
    pub fn with_capacity(key: impl Into<String>, tags: Tags, capacity: usize) -> Self {
        let (sender, recevr) = mpsc::channel(capacity.max(1));
        Self::from_parts(key, tags, sender, recevr)
    }

    /// 获取一个可写端副本，用于外部生产者推入原始字符串。
    pub fn sender(&self) -> Sender<String> {
        self.sender
            .as_ref()
            .expect("channel sender already closed")
            .clone()
    }

    /// 主动关闭输入端，确保缓冲耗尽后 `receive` 返回 EOF。
    pub fn close_input(&mut self) {
        self.sender.take();
        unregister_channel_sender(&self.key);
    }

    /// 调整单批次最大聚合行数，最小值 1。
    pub fn set_batch_limit(&mut self, limit: usize) {
        self.batch_limit = limit.max(1);
    }

    fn build_event(&self, payload: String) -> SourceEvent {
        SourceEvent::new(
            next_event_id(),
            &self.key,
            RawData::String(payload),
            Arc::new(self.base_tags.clone()),
        )
    }

    fn drain_ready(&mut self, batch: &mut SourceBatch) {
        while batch.len() < self.batch_limit {
            match self.recevr.try_recv() {
                Ok(line) => batch.push(self.build_event(line)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }
}

impl Drop for ChannelSource {
    fn drop(&mut self) {
        unregister_channel_sender(&self.key);
    }
}

#[async_trait]
impl DataSource for ChannelSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        let mut batch = SourceBatch::with_capacity(self.batch_limit);
        let Some(line) = self.recevr.recv().await else {
            return Err(SourceError::from(SourceReason::EOF));
        };
        batch.push(self.build_event(line));
        self.drain_ready(&mut batch);
        Ok(batch)
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        match self.recevr.try_recv() {
            Ok(line) => {
                let mut batch = SourceBatch::with_capacity(self.batch_limit);
                batch.push(self.build_event(line));
                self.drain_ready(&mut batch);
                Some(batch)
            }
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    fn can_try_receive(&mut self) -> bool {
        true
    }

    fn identifier(&self) -> String {
        self.key.clone()
    }

    async fn start(&mut self, mut ctrl_rx: CtrlRx) -> SourceResult<()> {
        if self.stop_task.is_some() {
            return Err(
                SourceReason::SupplierError("channel source already started".into()).into(),
            );
        }

        let key = self.key.clone();
        let handle = tokio::spawn(async move {
            while let Ok(event) = ctrl_rx.recv().await {
                if matches!(event, ControlEvent::Stop | ControlEvent::Isolate(true)) {
                    log::info!("channel source '{}' received stop signal", key);
                    break;
                }
            }
        });
        self.stop_task = Some(handle);
        Ok(())
    }

    async fn close(&mut self) -> SourceResult<()> {
        self.close_input();
        if let Some(handle) = self.stop_task.take() {
            handle.abort();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ChannelSource;
    use tokio::time::Duration;
    use wp_connector_api::{DataSource, SourceReason, Tags};

    #[tokio::test]
    async fn receive_batches_multiple_messages() {
        let mut source = ChannelSource::with_capacity("chan", Tags::default(), 8);
        let tx = source.sender();
        tx.send("first".into()).await.unwrap();
        tx.send("second".into()).await.unwrap();

        let batch = source.receive().await.expect("batch");
        assert_eq!(batch.len(), 2);
        let payloads: Vec<String> = batch
            .into_iter()
            .map(|evt| evt.payload.to_string())
            .collect();
        assert_eq!(payloads, vec!["first", "second"]);
    }

    #[tokio::test]
    async fn try_receive_is_non_blocking() {
        let mut source = ChannelSource::with_capacity("chan", Tags::default(), 4);
        let tx = source.sender();
        tx.send("only".into()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        let batch = source.try_receive().expect("batch");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].payload.to_string(), "only");
    }

    #[tokio::test]
    async fn receive_returns_eof_after_close() {
        let mut source = ChannelSource::with_capacity("chan", Tags::default(), 1);
        source.close_input();
        match source.receive().await {
            Err(err) => assert!(matches!(err.reason(), SourceReason::EOF)),
            Ok(_) => panic!("expected eof"),
        }
    }
}
