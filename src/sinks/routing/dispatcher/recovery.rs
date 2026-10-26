use super::SinkDispatcher;
use crate::sinks::ASinkHandle;
use crate::stat::MonSend;
use wp_connector_api::SinkResult;

impl SinkDispatcher {
    pub async fn proc_fix(
        &mut self,
        sink_h: ASinkHandle,
        mon: &MonSend,
    ) -> SinkResult<Option<ASinkHandle>> {
        for j in self.sinks.iter_mut() {
            if j.is_backup_active() && j.accepts_handle(&sink_h) {
                j.recover_sink(sink_h, mon).await?;
                return Ok(None);
            }
        }
        Ok(Some(sink_h))
    }

    pub async fn proc_end(&mut self) -> SinkResult<()> {
        for sink_rt in self.sinks.iter_mut() {
            sink_rt.stop_backends().await?;
        }
        Ok(())
    }
}
