use crate::sinks::prelude::*;
use async_trait::async_trait;
use std::time::Instant;
//use wp_model_core::instant::Instant;

use wp_connector_api::{SinkError, SinkReason, SinkResult};

#[derive(Clone, Default)]
struct StgPoint {
    secs: usize,
    health: bool,
}
impl StgPoint {
    pub fn new(secs: usize, health: bool) -> Self {
        Self { secs, health }
    }
}
pub struct HealthController {
    cur_stg: StgPoint,
    items: Vec<StgPoint>,
    pot_beg: std::time::Instant,
}

impl Default for HealthController {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthController {
    pub fn new() -> Self {
        Self {
            items: vec![StgPoint::new(2, false), StgPoint::new(2, true)],
            cur_stg: StgPoint::new(2, true),
            pot_beg: Instant::now(),
        }
    }
    fn use_next_stg(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.cur_stg = self.items.remove(0);
        self.pot_beg = Instant::now();
    }
    pub fn decide_health(&mut self) -> bool {
        let pot_now = Instant::now();
        let pot_splice = pot_now - self.pot_beg;
        if pot_splice > time::Duration::seconds(self.cur_stg.secs as i64) {
            self.use_next_stg();
        }
        self.cur_stg.health
    }
}
pub struct ASinkTestProxy {
    ctrl_stg: HealthController,
    impl_ins: Box<dyn AsyncSink>,
}
impl ASinkTestProxy {
    pub fn new<T>(ins: T, ctrl_stg: HealthController) -> Self
    where
        T: AsyncSink + 'static,
    {
        Self {
            ctrl_stg,
            impl_ins: Box::new(ins),
        }
    }
}

#[async_trait]
impl AsyncCtrl for ASinkTestProxy {
    async fn stop(&mut self) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl).want("stop send to AsinkTestStub"))?;
        }
        self.impl_ins.stop().await?;
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.reconnect().await?;
        Ok(())
    }
}

#[async_trait]
impl AsyncRecordSink for ASinkTestProxy {
    async fn sink_record(&mut self, data: &DataRecord) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_record(data).await
    }

    async fn sink_records(&mut self, data: Vec<std::sync::Arc<DataRecord>>) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_records(data).await
    }
}

#[async_trait]
impl AsyncRawdatSink for ASinkTestProxy {
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_str(data).await
    }
    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_bytes(data).await
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_str_batch(data).await
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_bytes_batch(data).await
    }
}
// Blanket impl of AsyncSink is provided by wp-sink-api; no explicit impl needed here.
