use crate::runtime::actor::command::{ActorCtrlCmd, CmdPublisher, CmdSubscriber};
use crate::runtime::actor::signal::ShutdownCmd;
use async_broadcast::broadcast;
use derive_getters::Getters;
use orion_error::ErrorWith;
use orion_error::{ContextRecord, ErrorOwe, WithContext};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use wp_error::run_error::RunResult;
use wp_log::{debug_ctrl, info_ctrl};

#[derive(Getters)]
pub struct TaskGroup {
    name: String,
    cmd: ActorCtrlCmd,
    handles: Vec<JoinHandle<()>>,
    cmd_pub: CmdPublisher,
    cmd_sub: CmdSubscriber,
}

impl TaskGroup {
    pub fn new<S: Into<String>>(name: S, cmd: ShutdownCmd) -> Self {
        let (cmd_pub, cmd_sub) = broadcast(1000);
        Self {
            name: name.into(),
            handles: Vec::new(),
            cmd_pub,
            cmd_sub,
            cmd: ActorCtrlCmd::Stop(cmd),
        }
    }
    pub fn publish(&self) -> CmdPublisher {
        self.cmd_pub.clone()
    }
    pub fn subscribe(&self) -> CmdSubscriber {
        self.cmd_sub.clone()
    }
    pub fn append(&mut self, handle: JoinHandle<()>) {
        self.handles.push(handle);
    }
    pub async fn cmd_alone(&self) -> RunResult<()> {
        let cmd = ActorCtrlCmd::Isolate;
        info_ctrl!("{} broadcast cmd :{:?}", self.name, cmd);
        self.cmd_pub.broadcast(cmd.clone()).await.owe_sys()?;
        Ok(())
    }
    pub async fn cmd_stop_now(&self) -> RunResult<()> {
        let cmd = ShutdownCmd::Immediate;
        info_ctrl!("{} broadcast cmd :{:?}", self.name, cmd);
        self.cmd_pub
            .broadcast(ActorCtrlCmd::Stop(cmd))
            .await
            .owe_sys()?;
        Ok(())
    }
    pub async fn cmd_stop(&self, stop: ShutdownCmd) -> RunResult<()> {
        info_ctrl!(
            "{} broadcast cmd :{:?} (subscribers={})",
            self.name,
            stop,
            self.cmd_pub.receiver_count()
        );
        self.cmd_pub
            .broadcast(ActorCtrlCmd::Stop(stop))
            .await
            .owe_sys()?;
        Ok(())
    }
    pub async fn broadcast_cmd(&mut self, cmd: ActorCtrlCmd) {
        info_ctrl!("{} broadcast cmd :{:?}", self.name, cmd);
        self.cmd_pub.broadcast(cmd).await.expect(" send cmd error");
    }

    pub async fn wait_grace_down(&mut self, out_cmd: Option<ActorCtrlCmd>) -> RunResult<()> {
        let cmd = out_cmd.unwrap_or(self.cmd.clone());
        let mut ctx = WithContext::want("grace down routine group");
        let msg = format!("{} broadcast cmd :{:?} ", self.name, cmd);
        info_ctrl!("{}", msg);
        ctx.record("cmd", msg);
        self.cmd_pub.broadcast(cmd).await.owe_sys().with(&ctx)?;
        let mut index = 0;
        while let Some(h) = self.handles.pop() {
            if !h.is_finished() {
                info_ctrl!("{} group routines [{}] wait... ", self.name, index);
                #[cfg(debug_assertions)]
                {
                    sleep(Duration::from_secs(1)).await;
                }
                h.await.owe_sys().with(&ctx)?;
            }
            debug_ctrl!("{} group routines[{}] finished end", self.name, index);
            index += 1;
        }
        info_ctrl!("{} group routines end", self.name);
        Ok(())
    }

    pub async fn signal_wait_grace_down_ex(&mut self) -> RunResult<ShutdownCmd> {
        let stop = ShutdownCmd::Immediate;
        self.do_stop(stop.clone()).await?;
        Ok(stop)
    }
    async fn do_stop(&self, stop: ShutdownCmd) -> RunResult<()> {
        if stop.eq(&ShutdownCmd::NoOp) {
            return Ok(());
        }
        info_ctrl!("will stop routin group({})", self.name);
        self.cmd_alone().await?;
        sleep(Duration::from_millis(100)).await;
        self.cmd_stop(stop).await?;
        while !self.routin_is_finished() {
            sleep(Duration::from_millis(100)).await;
            debug_ctrl!("wait routin({}) finish", self.name)
        }
        Ok(())
    }
    pub fn routin_is_finished(&self) -> bool {
        for h in &self.handles {
            if !h.is_finished() {
                return false;
            }
        }
        true
    }

    pub async fn grace_down(&mut self, out_cmd: ActorCtrlCmd) -> RunResult<()> {
        info_ctrl!("{} broadcast cmd :{:?}", self.name, out_cmd);
        self.cmd_pub.broadcast(out_cmd).await.owe_sys()?;
        Ok(())
    }
}
