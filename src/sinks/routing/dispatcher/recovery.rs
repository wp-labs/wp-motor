use super::SinkDispatcher;
use crate::sinks::ASinkHandle;
use crate::stat::MonSend;
use wp_connector_api::AsyncCtrl;
use wp_connector_api::SinkResult; // trait for stop()

impl SinkDispatcher {
    pub async fn proc_fix(
        &mut self,
        sink_h: ASinkHandle,
        mon: &MonSend,
    ) -> SinkResult<Option<ASinkHandle>> {
        for j in self.sinks.iter_mut() {
            if j.name == sink_h.name {
                j.recover_sink(sink_h, mon).await?;
                return Ok(None);
            }
        }
        Ok(Some(sink_h))
    }

    pub async fn proc_end(&mut self) -> SinkResult<()> {
        for sink_rt in self.sinks.iter_mut() {
            sink_rt.primary.stop().await?;
        }
        Ok(())
    }
}
