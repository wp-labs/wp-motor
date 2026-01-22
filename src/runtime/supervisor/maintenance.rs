use wp_connector_api::AsyncCtrl;

use std::time::Duration;

use tokio::time::{sleep, timeout};

use crate::runtime::actor::command::{CmdSubscriber, TaskController};
use crate::sinks::{ASinkHandle, ASinkReceiver, ASinkSender};

pub struct ActMaintainer {
    cmd_r: CmdSubscriber,
    keep_item: Vec<(ASinkReceiver, ASinkSender, ASinkSender)>,
}
impl ActMaintainer {
    pub fn new(cmd_r: CmdSubscriber) -> Self {
        Self {
            cmd_r,
            keep_item: Vec::new(),
        }
    }
    pub fn fix_channel(&mut self) -> (ASinkSender, ASinkReceiver) {
        let (bad_sink_s, bad_sink_r) = tokio::sync::mpsc::channel::<ASinkHandle>(100);
        let (fix_sink_s, fix_sink_r) = tokio::sync::mpsc::channel::<ASinkHandle>(100);
        self.keep_item
            .push((bad_sink_r, fix_sink_s, bad_sink_s.clone()));
        (bad_sink_s, fix_sink_r)
    }
    pub async fn proc(&mut self) {
        let mut run_ctrl = TaskController::new("maintainer", self.cmd_r.clone(), None);
        loop {
            for (bad_sink_r, fix_sink_s, bad_sink_s) in self.keep_item.iter_mut() {
                if let Ok(Some(mut data)) =
                    timeout(Duration::from_millis(100), bad_sink_r.recv()).await
                {
                    let result = data.sink.reconnect().await;
                    match result {
                        Ok(_) => {
                            let sink_name = data.name.clone();
                            warn_ctrl!("reconnect success ,send {} to fix_sink_q", sink_name);
                            match fix_sink_s.send(data).await {
                                Ok(()) => {
                                    debug_ctrl!("Successfully sent {} to fix_sink_q", sink_name);
                                }
                                Err(_) => {
                                    warn_ctrl!(
                                        "Failed to send {} to fix_sink_q, channel may be closed",
                                        sink_name
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            let sink_name = data.name.clone();
                            warn_ctrl!("{} reconnect fail! {}", sink_name, e);
                            match bad_sink_s.send(data).await {
                                Ok(()) => {
                                    debug_ctrl!(
                                        "Successfully sent {} back to bad_sink_q",
                                        sink_name
                                    );
                                }
                                Err(_) => {
                                    warn_ctrl!(
                                        "Failed to send {} back to bad_sink_q, channel may be closed",
                                        sink_name
                                    );
                                }
                            }
                            sleep(tokio::time::Duration::from_secs(5)).await;
                        }
                    }
                    run_ctrl.rec_task_suc();
                } else {
                    run_ctrl.rec_task_idle();
                }
            }
            //run_ctrl.run_breathe_slow();
            if run_ctrl.is_down().await {
                break;
            }
        }
    }
}
