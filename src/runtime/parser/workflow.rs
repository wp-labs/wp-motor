use crate::{runtime::prelude::*, types::EventBatchRecv, types::EventBatchSend};

use super::act_parser::ActParser;
use crate::runtime::actor::command::CmdSubscriber;
use wp_log::{error_ctrl, info_ctrl};

pub struct ActorWork {
    name: String,
    dat_r: EventBatchRecv,
    cmd_r: CmdSubscriber,
    mon_s: MonSend,
    actor: ActParser,
}

impl ActorWork {
    pub fn new<S: Into<String>>(
        name: S,
        dat_r: EventBatchRecv,
        cmd_r: CmdSubscriber,
        mon_s: MonSend,
        actor: ActParser,
    ) -> Self {
        ActorWork {
            name: name.into(),
            dat_r,
            cmd_r,
            mon_s,
            actor,
        }
    }
    pub async fn proc(&mut self, setting: ParseOption) -> WparseResult<()> {
        info_ctrl!("actor({}) work start", self.name);

        if let Err(e) = self
            .actor
            .parse_events(&self.cmd_r, &mut self.dat_r, &self.mon_s, setting)
            .await
        {
            error_ctrl!("actor({}) work error: {}", self.name, e);
            return Err(e);
        }
        info_ctrl!("actor({}) work end", self.name);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ParseWorkerSender {
    pub dat_s: EventBatchSend,
}

impl ParseWorkerSender {
    pub fn new(dat_s: EventBatchSend) -> Self {
        Self { dat_s }
    }
}
