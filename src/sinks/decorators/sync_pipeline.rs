use crate::{
    core::sinks::sync_sink::{RecSyncSink, TrySendStatus, traits::SyncCtrl},
    sinks::{SinkDataEnum, SinkRecUnit},
};
use wp_connector_api::SinkResult;

#[derive(Clone)]
pub struct SyncFrame<I, T>
where
    T: SyncCtrl,
    I: SyncCtrl,
{
    ins: I,
    next_proc: Option<T>,
}

impl<T, I> SyncCtrl for SyncFrame<T, I>
where
    T: SyncCtrl,
    I: SyncCtrl,
{
    fn stop(&mut self) -> SinkResult<()> {
        self.ins.stop()?;
        if let Some(ref mut next_proc) = self.next_proc {
            next_proc.stop()?;
        }
        Ok(())
    }
}

impl<T, I> RecSyncSink for SyncFrame<T, I>
where
    T: SyncCtrl + RecSyncSink,
    I: SyncCtrl + RecSyncSink,
{
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        self.ins.send_to_sink(data.clone())?;
        if let Some(ref next_proc) = self.next_proc {
            next_proc.send_to_sink(data)?;
        }
        Ok(())
    }
    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        let original = (
            *data.id(),
            SinkDataEnum::Rec(data.meta().clone(), data.data().clone()),
        );
        match self.ins.try_send_to_sink(data.clone()) {
            TrySendStatus::Sended => {
                if let Some(ref next_proc) = self.next_proc {
                    match next_proc.try_send_to_sink(data) {
                        TrySendStatus::Sended => TrySendStatus::Sended,
                        TrySendStatus::Fulfilled(_, _) => {
                            TrySendStatus::Fulfilled(original.0, original.1)
                        }
                        TrySendStatus::Err(e) => TrySendStatus::Err(e),
                    }
                } else {
                    TrySendStatus::Sended
                }
            }
            TrySendStatus::Fulfilled(_, _) => TrySendStatus::Fulfilled(original.0, original.1),
            TrySendStatus::Err(e) => TrySendStatus::Err(e),
        }
    }
}

impl<I, T> SyncFrame<I, T>
where
    T: SyncCtrl,
    I: SyncCtrl,
{
    pub fn new(in_ins: I) -> Self {
        SyncFrame {
            ins: in_ins,
            next_proc: None,
        }
    }
    pub fn next_pipe(&mut self, assembler: T) {
        self.next_proc = Some(assembler);
    }
}
