use std::time::Duration;

use crate::core::parser::WplEngine;
use crate::runtime::prelude::*;

use crate::orchestrator::engine::definition::WplCodePKG;
use crate::runtime::actor::command::{CmdSubscriber, TaskController};
use crate::sinks::InfraSinkAgent;
use crate::sinks::SinkRouteAgent;
use crate::stat::{MonSend, STAT_INTERVAL_MS};
use crate::types::EventBatchRecv;
use tokio::time::{MissedTickBehavior, interval, sleep};
use wp_error::run_error::RunResult;
use wp_log::info_ctrl;
use wp_stat::StatReq;
use wpl::OPTIMIZE_TIMES;
use wpl::WparseResult;

//clone will error;
//#[derive(Clone)]
pub struct ActParser {
    pub engine: WplEngine,
    pub sinks: SinkRouteAgent,
}

impl ActParser {
    pub fn optimized(&mut self, count: usize) {
        self.engine.optimized(count);
    }
    pub async fn from_all_model(
        pipelines: Vec<WplPipeline>,
        sinks: SinkRouteAgent,
        infra: InfraSinkAgent,
    ) -> RunResult<Self> {
        trace_ctrl!("setting depend");
        let pipe_lines = WplEngine::from(pipelines, infra).owe_conf()?;
        //let pipe_lines = ParseEngine::from(pipelines, infra).to_uvs::<ConfErrReader>()?;
        Ok(ActParser {
            engine: pipe_lines,
            sinks,
        })
    }

    pub fn from_normal(
        wpl_code: WplCodePKG,
        sinks: SinkRouteAgent,
        infra: InfraSinkAgent,
        _stat_reqs: Vec<StatReq>,
    ) -> RunResult<Self> {
        trace_ctrl!("setting depend");
        let wpl_pkgs = WplRepository::from_wpl_tolerant(wpl_code, infra.error.end()).owe_rule()?;
        let pipe_lines = WplEngine::from_code(&wpl_pkgs, infra).owe_conf()?;
        Ok(ActParser {
            engine: pipe_lines,
            sinks,
        })
    }
}

impl ActParser {
    pub async fn parse_events(
        &mut self,
        cmd_recv: &CmdSubscriber,
        dat_recv: &mut EventBatchRecv,
        mon_send: &MonSend,
        setting: ParseOption,
    ) -> WparseResult<()> {
        trace_ctrl!("proc frames begin");
        let mut run_ctrl = TaskController::new("lang", cmd_recv.clone(), None);
        warn_ctrl!(
            "parse engine pipelin cnt: {}",
            self.engine.pipelines.pipelines().len()
        );
        let mut stat_tick = interval(Duration::from_millis(STAT_INTERVAL_MS as u64));
        stat_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
               Some(mut batch)  = dat_recv.recv() => {
                   // 批量处理 SourceEvent（原地修改 batch，避免额外 Vec 分配）
                   for event in batch.iter_mut() {
                       trace_data!("recv frame ");
                       run_ctrl.rec_task_suc();
                       if let Some(hook) = event.preproc.clone() { (hook)(event); }
                       if run_ctrl.total_count().is_multiple_of(OPTIMIZE_TIMES) {
                           self.optimized(OPTIMIZE_TIMES);
                       }
                   }
                   // 若开启 skip-parse：不执行解析逻辑，直接进入下一轮（保持解析服务结构与速率控制不变）。
                   if crate::engine_flags::skip_parse() {
                       continue;
                   }
                   // 正常执行解析+下发
                   self.engine.proc_batch(batch, &setting).await?;
               }
               Ok(cmd) =  run_ctrl.cmds_sub_mut().recv() => { run_ctrl.update_cmd(cmd); }
              _ = sleep(Duration::from_millis(50)) => {
                  // 记录一次“空等”，使 ShutdownCmd::Timeout 能正确感知最后处理时间
                  run_ctrl.rec_task_idle();
                  if run_ctrl.is_stop() { break; }
              }
              _ = stat_tick.tick() => {
                  self.engine.send_stat(mon_send).await?;
              }
            }
        }
        info_ctrl!("engine proc frames end: total {}", run_ctrl.total_count());
        self.engine.send_stat(mon_send).await?;
        Ok(())
    }
}

impl ActParser {}
