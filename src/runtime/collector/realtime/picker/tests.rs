use crate::sources::event_id::next_event_id;
use async_trait::async_trait;
use std::sync::Arc;
use wp_connector_api::{DataSource, SourceBatch, SourceEvent, Tags};
use wp_connector_api::{SourceError, SourceReason, SourceResult};
use wp_parse_api::RawData;

// Basic smoke-tests to ensure signatures and lifecycle keep working after refactor.

#[allow(dead_code)]
struct MockDataSource {
    id: String,
    data_count: usize,
}

#[async_trait]
impl DataSource for MockDataSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        if self.data_count > 0 {
            self.data_count -= 1;
            let st = Tags::new();
            Ok(vec![SourceEvent::new(
                next_event_id(),
                &self.id,
                RawData::from_string(format!("test data {}", self.data_count)),
                Arc::new(st),
            )])
        } else {
            Err(SourceError::from(SourceReason::EOF))
        }
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        None
    }

    fn can_try_receive(&mut self) -> bool {
        false
    }
    fn identifier(&self) -> String {
        self.id.clone()
    }
}
