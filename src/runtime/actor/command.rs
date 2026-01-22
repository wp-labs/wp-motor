use super::limit::{RateLimiter, SystemInstant};
use crate::runtime::actor::constants::ACTOR_CMD_POLL_TIMEOUT_MS;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::types::Abstract;
use derive_getters::Getters;
#[cfg(any(test, feature = "dev-tools"))]
use orion_error::ErrorOwe;

use std::fmt::Display;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use wp_connector_api::ControlEvent;
#[cfg(any(test, feature = "dev-tools"))]
use wp_error::run_error::RunResult;
use wp_log::info_ctrl;

pub enum TaskEndReason {
    SucEnded,
    Interrupt,
}

impl Display for TaskEndReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskEndReason::SucEnded => write!(f, "SucEnded"),
            TaskEndReason::Interrupt => write!(f, "Interrupt"),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TaskScope {
    All,
    One(String),
}

impl Display for TaskScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.clone() {
            TaskScope::All => write!(f, "All"),
            TaskScope::One(name) => write!(f, "One({})", name),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ActorCtrlCmd {
    Stop(ShutdownCmd),
    Execute(TaskScope),
    Suspend(TaskScope),
    NoOp,
    Isolate,
}

impl Display for ActorCtrlCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.clone() {
            ActorCtrlCmd::Stop(x) => write!(f, "CtrlCmd::Stop({})", x),
            ActorCtrlCmd::Execute(x) => write!(f, "CtrlCmd::Work({})", x),
            ActorCtrlCmd::Suspend(x) => write!(f, "CtrlCmd::Freeze({})", x),
            ActorCtrlCmd::NoOp => write!(f, "CtrlCmd::Null"),
            ActorCtrlCmd::Isolate => write!(f, "CtrlCmd::Alone"),
        }
    }
}

impl Abstract for ActorCtrlCmd {
    fn abstract_info(&self) -> String {
        format!("ctrl_cmd::{}", self)
    }
}

impl ActorCtrlCmd {
    pub fn is_end(&self, total_cnt: usize, last_proc_time: &Instant) -> bool {
        match self.clone() {
            ActorCtrlCmd::Stop(x) => match x {
                ShutdownCmd::Immediate => {
                    info_ctrl!("routine cmd stop ");
                    return true;
                }
                ShutdownCmd::CountLimit(limit) => {
                    if total_cnt >= limit {
                        info_ctrl!("routine limit end");
                        return true;
                    }
                }
                ShutdownCmd::Timeout(millis) => {
                    if std::time::Instant::now() - *last_proc_time
                        > std::time::Duration::from_millis(millis as u64)
                    {
                        info_ctrl!("routine wait {} millis sec end ", millis);
                        return true;
                    }
                }
                ShutdownCmd::NoOp => {}
            },
            ActorCtrlCmd::Execute(_scope) => {}
            _ => {}
        }
        false
    }
}

pub type CmdSubscriber = async_broadcast::Receiver<ActorCtrlCmd>;
pub type CmdPublisher = async_broadcast::Sender<ActorCtrlCmd>;

/// 将 `ActorCtrlCmd` 转译为对数据源可见的 `ControlEvent`，并返回给调用方订阅。
pub fn spawn_ctrl_event_bridge(
    mut cmd_sub: CmdSubscriber,
    capacity: usize,
) -> async_broadcast::Receiver<ControlEvent> {
    let (ctrl_tx, ctrl_rx) = async_broadcast::broadcast::<ControlEvent>(capacity);
    tokio::spawn(async move {
        while let Ok(cmd) = cmd_sub.recv().await {
            if let Some(evt) = map_ctrl_cmd(cmd) {
                let _ = ctrl_tx.broadcast(evt).await;
            }
        }
    });
    ctrl_rx
}

fn map_ctrl_cmd(cmd: ActorCtrlCmd) -> Option<ControlEvent> {
    match cmd {
        ActorCtrlCmd::Stop(_) => Some(ControlEvent::Stop),
        ActorCtrlCmd::Isolate => Some(ControlEvent::Isolate(true)),
        _ => None,
    }
}

#[derive(Getters, Debug)]
pub struct TaskController {
    //cmd_s: CmdPublisher,
    act_name: String,
    work_cmd: ActorCtrlCmd,
    cmds_sub: CmdSubscriber,
    total_suc_cnt: usize,
    unit_suc_cnt: usize,
    unit_size: usize,
    limit: RateLimiter,
    last_miss: Option<SystemInstant>,
    unit_waited: Duration,
}

impl TaskController {
    pub fn cmds_sub_mut(&mut self) -> &mut CmdSubscriber {
        &mut self.cmds_sub
    }
    pub fn new(act_name: &str, cmds_sub: CmdSubscriber, sec_count: Option<usize>) -> Self {
        let speed_limit = RateLimiter::new_or_default(sec_count, 1, act_name);
        Self {
            act_name: act_name.to_string(),
            cmds_sub,
            total_suc_cnt: 0,
            work_cmd: ActorCtrlCmd::Execute(TaskScope::All),
            limit: speed_limit,
            unit_size: 100,
            unit_suc_cnt: 0,
            last_miss: None,
            unit_waited: Duration::ZERO,
        }
    }
    pub fn from_speed_limit(
        act_name: &str,
        cmds_sub: CmdSubscriber,
        sec_limit: Option<usize>,
        unit_size: usize,
    ) -> Self {
        let speed_limit = sec_limit
            .map(|x| RateLimiter::new(x, unit_size, act_name))
            //.unwrap_or(SpeedLimit::default());
            .unwrap_or_default();
        warn_ctrl!("actor:{} {}", act_name, sec_limit.unwrap_or(0));
        Self {
            //cmd_s,
            act_name: act_name.to_string(),
            cmds_sub,
            total_suc_cnt: 0,
            work_cmd: ActorCtrlCmd::Execute(TaskScope::All),
            limit: speed_limit,
            unit_size,
            unit_suc_cnt: 0,
            last_miss: None,
            unit_waited: Duration::ZERO,
        }
    }

    pub fn is_unit_end(&self) -> bool {
        self.unit_suc_cnt >= self.unit_size
    }
    pub async fn is_down(&mut self) -> bool {
        let result = self
            .work_cmd
            .is_end(self.total_suc_cnt, &self.get_miss_time());
        if let Ok(Ok(cmd)) = timeout(
            Duration::from_millis(ACTOR_CMD_POLL_TIMEOUT_MS),
            self.cmds_sub.recv(),
        )
        .await
        {
            info_ctrl!("{} Recv cmd : {:?}", self.act_name, cmd);
            self.work_cmd = cmd;
        }
        result
    }
    pub fn update_cmd(&mut self, cmd: ActorCtrlCmd) {
        info_ctrl!("{} update cmd: {} ", self.act_name, cmd);
        self.work_cmd = cmd;
    }
    pub fn is_stop(&self) -> bool {
        self.work_cmd
            .is_end(self.total_suc_cnt, &(self.get_miss_time()))
    }
    #[cfg(any(test, feature = "dev-tools"))]
    #[allow(dead_code)]
    pub async fn recv_update_cmd(&mut self) -> RunResult<()> {
        let cmd = self.cmds_sub.recv().await.owe_sys()?;
        self.update_cmd(cmd);
        Ok(())
    }

    pub fn not_alone(&self) -> bool {
        !matches!(self.work_cmd, ActorCtrlCmd::Isolate | ActorCtrlCmd::Stop(_))
    }
    #[cfg(any(test, feature = "dev-tools"))]
    #[allow(dead_code)]
    pub async fn not_alone_else_upcmd(&mut self) -> RunResult<bool> {
        if self.is_alone() {
            self.recv_update_cmd().await?;
            return Ok(false);
        }
        Ok(true)
    }

    #[cfg(any(test, feature = "dev-tools"))]
    #[allow(dead_code)]
    pub fn is_alone(&self) -> bool {
        self.work_cmd == ActorCtrlCmd::Isolate
    }
    #[inline]
    pub fn total_count(&self) -> usize {
        self.total_suc_cnt
    }
    #[inline]
    pub fn rec_task_unit_reset(&mut self) -> bool {
        self.limit.rec_beg();
        self.unit_suc_cnt = 0;
        self.unit_waited = Duration::ZERO;
        false
    }
    #[inline]
    pub fn rec_task_suc(&mut self) {
        self.total_suc_cnt += 1;
        self.unit_suc_cnt += 1;
        self.last_miss = None;
    }
    #[inline]
    pub fn rec_task_suc_cnt(&mut self, count: usize) {
        self.total_suc_cnt += count;
        self.unit_suc_cnt += count;
        self.last_miss = None;
    }
    #[inline]
    #[allow(dead_code)]
    pub fn rec_task_fail(&mut self, count: usize) {
        if count > 0 {
            self.total_suc_cnt = self.total_suc_cnt.saturating_sub(count);
            self.unit_suc_cnt = self.unit_suc_cnt.saturating_sub(count);
        }
    }
    #[inline]
    pub fn rec_task_idle(&mut self) {
        if self.last_miss().is_none() {
            self.last_miss = Some(Instant::now());
            //debug_ctrl!("{} rec miss; total:{}", self.act_name(), self.total_count())
        }
    }

    #[inline]
    pub fn get_miss_time(&self) -> Instant {
        self.last_miss.unwrap_or(Instant::now())
    }

    #[cfg(any(test, feature = "dev-tools"))]
    #[allow(dead_code)]
    pub fn rec_time_now(&mut self) -> bool {
        self.total_suc_cnt += 1;
        self.limit.rec_beg();
        true
    }
    /// Async-friendly version; prefer this inside async tasks.
    #[cfg(any(test, feature = "dev-tools"))]
    #[allow(dead_code)]
    pub async fn unit_speed_limit_wait_async(&self) {
        self.limit.limit_speed_wait_async().await;
    }
    pub fn unit_speed_limit_left(&self) -> Duration {
        self.limit.limit_speed_time()
    }
}
