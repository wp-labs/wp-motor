use super::TaskGroup;
use super::signal::stop_signals;
use crate::runtime::actor::command::ActorCtrlCmd;
use crate::runtime::actor::signal::ShutdownCmd;
use futures_lite::prelude::*;
use orion_error::{ToStructError, UvsLogicFrom};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use wp_error::{RunReason, run_error::RunResult};
use wp_log::info_ctrl;

#[derive(Default)]
pub struct TaskManager {
    main: Option<TaskGroup>,
    h_groups: Vec<TaskGroup>,
}

impl TaskManager {
    pub fn append_group(&mut self, h_group: TaskGroup) {
        self.h_groups.push(h_group);
    }
    pub fn set_main(&mut self, h_group: TaskGroup) {
        self.main = Some(h_group);
    }
    pub fn push_group(&mut self, h_group: TaskGroup) -> &mut TaskGroup {
        self.h_groups.push(h_group);
        self.h_groups.last_mut().unwrap()
    }

    pub async fn all_down_wait_signal(&mut self) -> RunResult<()> {
        let mut signals = stop_signals()?;
        if let Some(main) = &mut self.main {
            loop {
                if main.routin_is_finished() {
                    self.all_down_wait_signal_ex().await?;
                    return Ok(());
                } else if let Ok(Some(_)) =
                    timeout(Duration::from_millis(100), signals.next()).await
                {
                    info_ctrl!("receive exit signal!");
                    self.all_down_wait_signal_ex().await?;
                    return Ok(());
                }

                sleep(Duration::from_millis(100)).await;
            }
        } else {
            Err(RunReason::from_logic("not main routine".to_string()).to_err())
        }
    }
    pub async fn all_down_wait_signal_ex(&mut self) -> RunResult<()> {
        self.h_groups.reverse();
        if let Some(main) = &mut self.main {
            let stop = main.signal_wait_grace_down_ex().await?;
            if stop.eq(&ShutdownCmd::NoOp) {
                return Ok(());
            }
            for group in &mut self.h_groups {
                group.cmd_alone().await?;
            }
            for group in &mut self.h_groups {
                group
                    .wait_grace_down(Some(ActorCtrlCmd::Stop(stop.clone())))
                    .await?;
                info_ctrl!("routine group {} await end!", group.name());
            }
            info_ctrl!("all routine group await end!");
            self.h_groups.clear();
            Ok(())
        } else {
            Err(RunReason::from_logic("not main routine".to_string()).to_err())
        }
    }
}
