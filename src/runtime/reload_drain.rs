use crate::runtime::actor::TaskRole;
use orion_error::{ToStructError, UvsFrom};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use wp_error::run_error::{RunReason, RunResult};
use wp_log::warn_ctrl;

#[derive(Debug, Clone)]
pub struct ReloadDrainEvent {
    epoch: u64,
    role: TaskRole,
    worker_name: String,
    outcome: ReloadDrainOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadDrainOutcome {
    Drained,
    Aborted,
}

impl ReloadDrainEvent {
    fn new(epoch: u64, role: TaskRole, worker_name: String, outcome: ReloadDrainOutcome) -> Self {
        Self {
            epoch,
            role,
            worker_name,
            outcome,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn role(&self) -> TaskRole {
        self.role
    }

    pub fn worker_name(&self) -> &str {
        self.worker_name.as_str()
    }

    pub fn outcome(&self) -> ReloadDrainOutcome {
        self.outcome
    }
}

pub struct ReloadDrainReporter {
    tx: UnboundedSender<ReloadDrainEvent>,
    pending: Option<ReloadDrainEvent>,
}

impl ReloadDrainReporter {
    fn send_pending(&mut self, outcome: ReloadDrainOutcome) {
        if let Some(mut event) = self.pending.take() {
            event.outcome = outcome;
            let _ = self.tx.send(event);
        }
    }

    pub fn notify(&mut self) {
        self.send_pending(ReloadDrainOutcome::Drained);
    }
}

impl Drop for ReloadDrainReporter {
    fn drop(&mut self) {
        self.send_pending(ReloadDrainOutcome::Aborted);
    }
}

#[derive(Clone)]
pub struct ReloadDrainBus {
    epoch: u64,
    tx: UnboundedSender<ReloadDrainEvent>,
}

impl ReloadDrainBus {
    pub fn new(epoch: u64) -> (Self, UnboundedReceiver<ReloadDrainEvent>) {
        let (tx, rx) = unbounded_channel();
        (Self { epoch, tx }, rx)
    }

    pub fn reporter<S: Into<String>>(&self, role: TaskRole, worker_name: S) -> ReloadDrainReporter {
        ReloadDrainReporter {
            tx: self.tx.clone(),
            pending: Some(ReloadDrainEvent::new(
                self.epoch,
                role,
                worker_name.into(),
                ReloadDrainOutcome::Aborted,
            )),
        }
    }
}

pub struct ReloadDrainTracker {
    epoch: u64,
    parser_pending: usize,
    sink_pending: usize,
    infra_pending: usize,
    rx: UnboundedReceiver<ReloadDrainEvent>,
}

impl ReloadDrainTracker {
    pub fn new(
        epoch: u64,
        parser_pending: usize,
        sink_pending: usize,
        infra_pending: usize,
        rx: UnboundedReceiver<ReloadDrainEvent>,
    ) -> Self {
        Self {
            epoch,
            parser_pending,
            sink_pending,
            infra_pending,
            rx,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn next_epoch(&self) -> u64 {
        self.epoch + 1
    }

    pub fn is_fully_quiesced(&self) -> bool {
        self.parser_pending == 0 && self.sink_pending == 0 && self.infra_pending == 0
    }

    pub async fn recv(&mut self) -> Option<ReloadDrainEvent> {
        self.rx.recv().await
    }

    pub fn observe(&mut self, event: &ReloadDrainEvent) -> RunResult<()> {
        if event.epoch() != self.epoch {
            warn_ctrl!(
                "ignore stale reload drain event epoch={} current_epoch={} role={:?} worker={}",
                event.epoch(),
                self.epoch,
                event.role(),
                event.worker_name()
            );
            return Ok(());
        }

        let pending = match event.role() {
            TaskRole::Parser => &mut self.parser_pending,
            TaskRole::Sink => &mut self.sink_pending,
            TaskRole::Infra => &mut self.infra_pending,
            other => {
                warn_ctrl!(
                    "ignore unsupported reload drain role {:?} epoch={} worker={}",
                    other,
                    event.epoch(),
                    event.worker_name()
                );
                return Ok(());
            }
        };

        if *pending == 0 {
            warn_ctrl!(
                "duplicate reload drain event ignored epoch={} role={:?} worker={}",
                event.epoch(),
                event.role(),
                event.worker_name()
            );
            return Ok(());
        }

        if event.outcome() == ReloadDrainOutcome::Aborted {
            return Err(RunReason::from_logic().to_err().with_detail(format!(
                "reload drain aborted epoch={} role={:?} worker={}",
                event.epoch(),
                event.role(),
                event.worker_name()
            )));
        }

        *pending -= 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn reporter_drop_emits_pending_event() {
        let (bus, mut rx) = ReloadDrainBus::new(7);
        let reporter = bus.reporter(TaskRole::Parser, "parser-drop");
        drop(reporter);

        let event = timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("drop should emit drain event")
            .expect("event should be present");
        assert_eq!(event.epoch(), 7);
        assert_eq!(event.role(), TaskRole::Parser);
        assert_eq!(event.worker_name(), "parser-drop");
        assert_eq!(event.outcome(), ReloadDrainOutcome::Aborted);
    }

    #[test]
    fn tracker_rejects_aborted_event() {
        let (bus, rx) = ReloadDrainBus::new(3);
        let mut tracker = ReloadDrainTracker::new(3, 1, 0, 0, rx);
        let reporter = bus.reporter(TaskRole::Parser, "parser-abort");
        drop(reporter);
        let event = tracker
            .rx
            .blocking_recv()
            .expect("aborted event should be present");
        let err = tracker
            .observe(&event)
            .expect_err("aborted event must fail");
        assert!(err.to_string().contains("parser-abort"));
    }
}
