use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
/// Local mirror of `wp_ctrl_api::CommandType` to avoid optional dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandType {
    LoadModel,
}

#[derive(Clone)]
pub struct RuntimeControlHandle {
    sender: RuntimeCommandSender,
    state: Arc<Mutex<RuntimeControlState>>,
}

impl RuntimeControlHandle {
    pub async fn request(
        &self,
        request_id: impl Into<String>,
        command: CommandType,
    ) -> Result<oneshot::Receiver<RuntimeCommandResp>, RuntimeCommandSendError> {
        self.sender.request(request_id, command).await
    }

    pub async fn request_load_model(
        &self,
        request_id: impl Into<String>,
    ) -> Result<oneshot::Receiver<RuntimeCommandResp>, RuntimeCommandSendError> {
        self.request(request_id, CommandType::LoadModel).await
    }

    pub fn command_sender(&self) -> RuntimeCommandSender {
        self.sender.clone()
    }

    pub fn status_snapshot(&self) -> RuntimeStatusSnapshot {
        self.state
            .lock()
            .expect("runtime control state poisoned on snapshot")
            .snapshot()
    }

    pub(crate) fn activate(&self) {
        self.state
            .lock()
            .expect("runtime control state poisoned on activate")
            .accepting_commands = true;
    }

    pub(crate) fn deactivate(&self) {
        self.state
            .lock()
            .expect("runtime control state poisoned on deactivate")
            .accepting_commands = false;
    }

    pub(crate) fn mark_running(&self, request_id: &str, command: &CommandType) {
        if matches!(command, CommandType::LoadModel) {
            self.state
                .lock()
                .expect("runtime control state poisoned on mark_running")
                .mark_reload_running(request_id);
        }
    }

    pub(crate) fn finish(&self, request_id: &str, result: RuntimeCommandResult) {
        self.state
            .lock()
            .expect("runtime control state poisoned on finish")
            .finish_reload(request_id, result);
    }
}

#[derive(Clone)]
pub struct RuntimeCommandSender {
    tx: mpsc::Sender<RuntimeCommandReq>,
    state: Arc<Mutex<RuntimeControlState>>,
    reload_gate: Arc<Semaphore>,
}

impl RuntimeCommandSender {
    pub async fn request(
        &self,
        request_id: impl Into<String>,
        command: CommandType,
    ) -> Result<oneshot::Receiver<RuntimeCommandResp>, RuntimeCommandSendError> {
        let request_id = request_id.into();
        if !self
            .state
            .lock()
            .expect("runtime control state poisoned on readiness check")
            .accepting_commands
        {
            return Err(RuntimeCommandSendError::RuntimeNotReady);
        }
        let is_reload = matches!(command, CommandType::LoadModel);
        let reload_permit = if is_reload {
            let permit = self
                .reload_gate
                .clone()
                .try_acquire_owned()
                .map_err(|_| RuntimeCommandSendError::ReloadBusy)?;
            self.state
                .lock()
                .expect("runtime control state poisoned on request")
                .mark_reload_submitted(&request_id);
            Some(permit)
        } else {
            None
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        let req = RuntimeCommandReq::new(request_id.clone(), command, reply_tx, reload_permit);
        if let Err(err) = self.tx.send(req).await {
            if is_reload {
                self.state
                    .lock()
                    .expect("runtime control state poisoned on send rollback")
                    .rollback_reload_submission(&request_id);
            }
            let _ = err;
            return Err(RuntimeCommandSendError::ChannelClosed);
        }

        Ok(reply_rx)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCommandSendError {
    RuntimeNotReady,
    ReloadBusy,
    ChannelClosed,
}

impl Display for RuntimeCommandSendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeCommandSendError::RuntimeNotReady => {
                write!(f, "runtime command receiver not ready")
            }
            RuntimeCommandSendError::ReloadBusy => write!(f, "reload already in progress"),
            RuntimeCommandSendError::ChannelClosed => {
                write!(f, "runtime command channel closed")
            }
        }
    }
}

impl std::error::Error for RuntimeCommandSendError {}

pub(crate) struct RuntimeCommandReq {
    request_id: String,
    command: CommandType,
    reply: oneshot::Sender<RuntimeCommandResp>,
    _reload_permit: Option<OwnedSemaphorePermit>,
}

impl RuntimeCommandReq {
    fn new(
        request_id: String,
        command: CommandType,
        reply: oneshot::Sender<RuntimeCommandResp>,
        reload_permit: Option<OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            request_id,
            command,
            reply,
            _reload_permit: reload_permit,
        }
    }

    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }

    pub(crate) fn command(&self) -> &CommandType {
        &self.command
    }

    pub(crate) fn respond(self, resp: RuntimeCommandResp) {
        let _ = self.reply.send(resp);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandResp {
    pub request_id: String,
    pub accepted: bool,
    pub result: RuntimeCommandResult,
}

impl RuntimeCommandResp {
    pub fn accepted(request_id: impl Into<String>, result: RuntimeCommandResult) -> Self {
        Self {
            request_id: request_id.into(),
            accepted: true,
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCommandResult {
    ReloadDone,
    ReloadDoneWithForceReplace,
    ReloadFailed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatusSnapshot {
    pub accepting_commands: bool,
    pub reloading: bool,
    pub current_request_id: Option<String>,
    pub last_reload_request_id: Option<String>,
    pub last_reload_result: Option<RuntimeCommandResult>,
    pub last_reload_started_at: Option<SystemTime>,
    pub last_reload_finished_at: Option<SystemTime>,
}

#[derive(Default)]
struct RuntimeControlState {
    accepting_commands: bool,
    current_request_id: Option<String>,
    last_reload_request_id: Option<String>,
    last_reload_result: Option<RuntimeCommandResult>,
    last_reload_started_at: Option<SystemTime>,
    last_reload_finished_at: Option<SystemTime>,
}

impl RuntimeControlState {
    fn snapshot(&self) -> RuntimeStatusSnapshot {
        RuntimeStatusSnapshot {
            accepting_commands: self.accepting_commands,
            reloading: self.current_request_id.is_some(),
            current_request_id: self.current_request_id.clone(),
            last_reload_request_id: self.last_reload_request_id.clone(),
            last_reload_result: self.last_reload_result.clone(),
            last_reload_started_at: self.last_reload_started_at,
            last_reload_finished_at: self.last_reload_finished_at,
        }
    }

    fn mark_reload_submitted(&mut self, request_id: &str) {
        self.current_request_id = Some(request_id.to_string());
        self.last_reload_started_at = Some(SystemTime::now());
        self.last_reload_finished_at = None;
    }

    fn mark_reload_running(&mut self, request_id: &str) {
        if self.current_request_id.as_deref() != Some(request_id) {
            self.current_request_id = Some(request_id.to_string());
            self.last_reload_started_at = Some(SystemTime::now());
            self.last_reload_finished_at = None;
        }
    }

    fn finish_reload(&mut self, request_id: &str, result: RuntimeCommandResult) {
        self.current_request_id = None;
        self.last_reload_request_id = Some(request_id.to_string());
        self.last_reload_result = Some(result);
        self.last_reload_finished_at = Some(SystemTime::now());
    }

    fn rollback_reload_submission(&mut self, request_id: &str) {
        if self.current_request_id.as_deref() == Some(request_id) {
            self.current_request_id = None;
            self.last_reload_started_at = None;
            self.last_reload_finished_at = None;
        }
    }
}

pub(crate) fn runtime_command_bus(
    buffer: usize,
) -> (RuntimeControlHandle, mpsc::Receiver<RuntimeCommandReq>) {
    let state = Arc::new(Mutex::new(RuntimeControlState::default()));
    let reload_gate = Arc::new(Semaphore::new(1));
    let (tx, rx) = mpsc::channel(buffer.max(1));
    let sender = RuntimeCommandSender {
        tx,
        state: state.clone(),
        reload_gate,
    };
    (RuntimeControlHandle { sender, state }, rx)
}

pub(crate) fn default_runtime_command_bus()
-> (RuntimeControlHandle, mpsc::Receiver<RuntimeCommandReq>) {
    runtime_command_bus(wp_conf::limits::cmd_channel_cap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_model_request_marks_snapshot_and_rejects_parallel_reload() {
        let (handle, mut rx) = runtime_command_bus(8);
        let not_ready = handle
            .request_load_model("req-0")
            .await
            .expect_err("reload before activation should be rejected");
        assert_eq!(not_ready, RuntimeCommandSendError::RuntimeNotReady);

        handle.activate();
        let first = handle
            .request_load_model("req-1")
            .await
            .expect("first reload should be accepted");
        let snap = handle.status_snapshot();
        assert!(
            snap.accepting_commands,
            "activation should mark command bus ready"
        );
        assert!(
            snap.reloading,
            "accepted reload should mark snapshot as reloading"
        );
        assert_eq!(snap.current_request_id.as_deref(), Some("req-1"));

        let err = handle
            .request_load_model("req-2")
            .await
            .expect_err("parallel reload should be rejected");
        assert_eq!(err, RuntimeCommandSendError::ReloadBusy);

        let req = rx.recv().await.expect("queued request");
        handle.mark_running(req.request_id(), req.command());
        let resp = RuntimeCommandResp::accepted("req-1", RuntimeCommandResult::ReloadDone);
        handle.finish("req-1", resp.result.clone());
        req.respond(resp);

        let snap = handle.status_snapshot();
        assert!(
            !snap.reloading,
            "finished reload should clear reloading snapshot"
        );
        assert_eq!(snap.last_reload_request_id.as_deref(), Some("req-1"));
        assert_eq!(
            snap.last_reload_result,
            Some(RuntimeCommandResult::ReloadDone)
        );

        drop(first);
    }

    #[tokio::test]
    async fn send_failure_rolls_back_reload_snapshot() {
        let (handle, rx) = runtime_command_bus(8);
        drop(rx);

        handle.activate();
        let err = handle
            .request_load_model("req-closed")
            .await
            .expect_err("closed channel should fail");
        assert_eq!(err, RuntimeCommandSendError::ChannelClosed);

        let snap = handle.status_snapshot();
        assert!(
            !snap.reloading,
            "send failure should not leave reload marked active"
        );
        assert!(snap.current_request_id.is_none());
        assert!(snap.last_reload_result.is_none());
    }

    #[tokio::test]
    async fn deactivate_rejects_new_requests_after_runtime_was_ready() {
        let (handle, _rx) = runtime_command_bus(8);
        handle.activate();
        handle.deactivate();

        let err = handle
            .request_load_model("req-after-deactivate")
            .await
            .expect_err("deactivated runtime should reject new requests");
        assert_eq!(err, RuntimeCommandSendError::RuntimeNotReady);

        let snap = handle.status_snapshot();
        assert!(!snap.accepting_commands);
        assert!(!snap.reloading);
    }
}
