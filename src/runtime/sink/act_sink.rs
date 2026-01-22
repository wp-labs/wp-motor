use crate::facade::test_helpers::SinkTerminal;
use crate::resources::ResManager;
use crate::resources::SinkID;
use crate::runtime::prelude::*;
use wp_connector_api::AsyncCtrl;
use wp_data_model::cache::FieldQueryCache;

use crate::orchestrator::config::build_sinks::{SinkRouteTable, build_sink_target};
use crate::runtime::actor::command::{ActorCtrlCmd, TaskScope};
use crate::runtime::actor::command::{CmdSubscriber, TaskController};
use crate::runtime::actor::constants::ACTOR_IDLE_TICK_MS;
use crate::runtime::sink::drain::{DrainEvent, DrainState};
use crate::sinks::SinkDispatcher;
use crate::sinks::SinkRouteAgent;
use crate::sinks::SinkRuntime;
use crate::sinks::{
    ASinkHandle, ASinkReceiver, ASinkSender, SinkDatAReceiver, SinkDatYReceiver, SinkDataEnum,
    SinkPackage,
};
use crate::sinks::{InfraSinkAgent, SinkGroupAgent};
use crate::stat::MonSend;
use orion_error::ContextRecord;
use orion_error::OperationContext;
use orion_overload::append::Appendable;
use std::time::Duration;
use tokio::time::sleep;
use wp_conf::TCondParser;
use wp_conf::structure::SinkInstanceConf;
use wp_conf::structure::{FlexGroup, SinkGroupConf};
use wp_connector_api::SinkResult;
use wp_error::run_error::{RunError, RunResult};
use wp_log::{info_ctrl, warn_ctrl};
use wp_stat::StatReq;

#[derive(Default)]
pub struct SinkService {
    pub items: Vec<SinkDispatcher>,
}

pub struct SinkWork {}

// 显式的基础组打包，避免依赖顺序传参
pub struct InfraGroups {
    pub default: SinkDispatcher,
    pub miss: SinkDispatcher,
    pub residue: SinkDispatcher,
    pub monitor: SinkDispatcher,
    pub error: SinkDispatcher,
}

struct InfraChannel {
    dispatcher: SinkDispatcher,
    bad_sink_s: ASinkSender,
    mon_send: MonSend,
    closed: bool,
}

impl InfraChannel {
    fn new(dispatcher: SinkDispatcher, bad_sink_s: &ASinkSender, mon_send: &MonSend) -> Self {
        Self {
            dispatcher,
            bad_sink_s: bad_sink_s.clone(),
            mon_send: mon_send.clone(),
            closed: false,
        }
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn mark_closed(&mut self) {
        self.closed = true;
    }

    fn close_channel(&mut self) {
        self.dispatcher.close_channel();
    }

    async fn handle_pkg(
        &mut self,
        pkg_opt: Option<SinkPackage>,
        drain_state: &mut DrainState,
        run_ctrl: &mut TaskController,
    ) -> SinkResult<bool> {
        if let Some(pkg) = pkg_opt {
            let mut processed = 0usize;
            for unit in pkg.iter() {
                processed += self
                    .dispatcher
                    .group_sink_one(
                        *unit.id(),
                        SinkDataEnum::Rec(unit.meta().clone(), unit.data().clone()),
                        &self.bad_sink_s,
                        Some(&self.mon_send),
                    )
                    .await?;
            }
            if processed > 0 {
                run_ctrl.rec_task_suc_cnt(processed);
            } else {
                run_ctrl.rec_task_idle();
            }
            return Ok(false);
        }
        self.mark_closed();
        Ok(match drain_state.channel_closed_is_drained() {
            DrainEvent::Drained => {
                info_ctrl!("infra sinks drain complete");
                true
            }
            DrainEvent::AllClosed => true,
            DrainEvent::Pending => false,
        })
    }

    fn freeze_all(&mut self) {
        self.dispatcher.freeze_all();
    }

    fn active_one(&mut self, name: &str) {
        self.dispatcher.active_one(name);
    }

    async fn proc_end(&mut self) -> SinkResult<String> {
        self.dispatcher.proc_end().await?;
        Ok(self.dispatcher.get_name().to_string())
    }

    fn get_dat_r_mut(&mut self) -> &mut SinkDatYReceiver {
        self.dispatcher.get_dat_r_mut()
    }
}

impl SinkWork {
    pub async fn async_proc(
        mut sink: SinkDispatcher,
        infra: InfraSinkAgent,
        mut cmd_r: CmdSubscriber,
        mon_send: MonSend,
        bad_sink_s: ASinkSender,
        mut fix_sink_r: ASinkReceiver,
    ) -> SinkResult<()> {
        let mut ctx = OperationContext::want("sink start proc");
        let name = format!("work-sink:{:20}", sink.conf().name());
        let mut run_ctrl = TaskController::new(name.as_str(), cmd_r.clone(), None);
        let mut cache = FieldQueryCache::with_capacity(1000);
        let sink_name = sink.get_name().to_string();
        ctx.record("name", name);
        let mut drain_state = DrainState::new(1);
        loop {
            tokio::select! {
                pkg_opt = sink.get_dat_r_mut().recv() => {
                    match pkg_opt {
                        Some(pkg) => {
                            let processed = sink
                                .group_sink_package(pkg, &infra, &bad_sink_s, Some(&mon_send), &mut cache)
                                .await?;
                            if processed > 0 {
                                run_ctrl.rec_task_suc_cnt(processed);
                            } else {
                                run_ctrl.rec_task_idle();
                            }
                        }
                        None => {
                            match drain_state.channel_closed_is_drained() {
                                DrainEvent::Drained => {
                                    info_ctrl!("{} drain complete", sink_name);
                                    break;
                                }
                                DrainEvent::AllClosed => break,
                                DrainEvent::Pending => {}
                            }
                        }
                    }
                }
                cmd_res = cmd_r.recv(), if !drain_state.is_draining() => {
                    match cmd_res {
                        Ok(cmd) => {
                            if let ActorCtrlCmd::Execute(TaskScope::One(target)) = cmd.clone() {
                                sink.freeze_all();
                                sink.active_one(target.as_str());
                            }
                            run_ctrl.update_cmd(cmd);
                            if run_ctrl.is_stop() {
                                if !drain_state.is_draining() {
                                    info_ctrl!("{} enter draining state", sink_name);
                                }
                                drain_state.start_draining();
                                sink.close_channel();
                            }
                        }
                        Err(err) => {
                            warn_ctrl!("sink cmd channel closed: {}", err);
                            if !drain_state.is_draining() {
                                info_ctrl!("{} enter draining state", sink_name);
                            }
                            drain_state.start_draining();
                            sink.close_channel();
                        }
                    }
                }
                Some(h) = fix_sink_r.recv(), if !drain_state.is_draining() => {
                    Self::proc_fix_ex(h,&mut sink,  &mon_send).await?;
                }
            }
        }
        sink.proc_end().await?;
        info_ctrl!("{} async sinks proc end", sink_name);
        Ok(())
    }
    #[allow(dead_code)]
    pub async fn sink_group_fix(
        sinks: &mut [SinkDispatcher],
        sink_h: ASinkHandle,
        mon_send: &MonSend,
    ) -> SinkResult<()> {
        let mut sink_hold = Some(sink_h);
        for sink in sinks.iter_mut() {
            if let Some(handle) = sink_hold {
                if let Some(unmatch) = Self::proc_fix_ex(handle, sink, mon_send).await? {
                    sink_hold = Some(unmatch);
                } else {
                    break;
                }
            }
        }
        Ok(())
    }
    pub async fn async_proc_infra(
        groups: InfraGroups,
        mut cmd_r: CmdSubscriber,
        mon_send: MonSend,
        bad_sink_s: ASinkSender,
        mut fix_sink_r: ASinkReceiver,
    ) -> SinkResult<()> {
        // 基础组固定 5 个：default/miss/residue/monitor/error
        let mut c0 = InfraChannel::new(groups.default, &bad_sink_s, &mon_send);
        let mut c1 = InfraChannel::new(groups.miss, &bad_sink_s, &mon_send);
        let mut c2 = InfraChannel::new(groups.residue, &bad_sink_s, &mon_send);
        let mut c3 = InfraChannel::new(groups.monitor, &bad_sink_s, &mon_send);
        let mut c4 = InfraChannel::new(groups.error, &bad_sink_s, &mon_send);

        let mut run_ctrl = TaskController::new("infra sinks ", cmd_r.clone(), None);
        let mut drain_state = DrainState::new(5);
        loop {
            tokio::select! {
                pkg_opt = c0.get_dat_r_mut().recv(), if !c0.is_closed() => {
                    if c0.handle_pkg(pkg_opt, &mut drain_state, &mut run_ctrl).await? {
                        break;
                    }
                }
                pkg_opt = c1.get_dat_r_mut().recv(), if !c1.is_closed() => {
                    if c1.handle_pkg(pkg_opt, &mut drain_state, &mut run_ctrl).await? {
                        break;
                    }
                }
                pkg_opt = c2.get_dat_r_mut().recv(), if !c2.is_closed() => {
                    if c2.handle_pkg(pkg_opt, &mut drain_state, &mut run_ctrl).await? {
                        break;
                    }
                }
                pkg_opt = c3.get_dat_r_mut().recv(), if !c3.is_closed() => {
                    if c3.handle_pkg(pkg_opt, &mut drain_state, &mut run_ctrl).await? {
                        break;
                    }
                }
                pkg_opt = c4.get_dat_r_mut().recv(), if !c4.is_closed() => {
                    if c4.handle_pkg(pkg_opt, &mut drain_state, &mut run_ctrl).await? {
                        break;
                    }
                }
                cmd_res = cmd_r.recv(), if !drain_state.is_draining() => {
                    match cmd_res {
                        Ok(cmd) => {
                            if let ActorCtrlCmd::Execute(TaskScope::One(sink_name)) = cmd.clone() {
                                for ch in [&mut c0, &mut c1, &mut c2, &mut c3, &mut c4] { ch.freeze_all(); }
                                for ch in [&mut c0, &mut c1, &mut c2, &mut c3, &mut c4] { ch.active_one(sink_name.as_str()); }
                            }
                            run_ctrl.update_cmd(cmd);
                            if run_ctrl.is_stop() {
                                if !drain_state.is_draining() {
                                    info_ctrl!("infra sinks enter draining state");
                                }
                                drain_state.start_draining();
                                for ch in [&mut c0, &mut c1, &mut c2, &mut c3, &mut c4] { ch.close_channel(); }
                            }
                        }
                        Err(err) => {
                            warn_ctrl!("infra cmd channel closed: {}", err);
                            if !drain_state.is_draining() {
                                info_ctrl!("infra sinks enter draining state");
                            }
                            drain_state.start_draining();
                            for ch in [&mut c0, &mut c1, &mut c2, &mut c3, &mut c4] { ch.close_channel(); }
                        }
                    }
                }
                Some(h) = fix_sink_r.recv(), if !drain_state.is_draining() => {
                    let mut hold = Some(h);
                    for ch in [&mut c0, &mut c1, &mut c2, &mut c3, &mut c4] {
                        let Some(handle) = hold.take() else { break; };
                        if let Some(unmatch) = Self::proc_fix_ex(handle, &mut ch.dispatcher, &mon_send).await? {
                            hold = Some(unmatch);
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        for ch in [&mut c0, &mut c1, &mut c2, &mut c3, &mut c4] {
            let sink_name = ch.proc_end().await?;
            info_ctrl!("infra:{} async sinks proc end", sink_name);
        }
        Ok(())
    }

    pub async fn proc_fix_ex(
        sink_h: ASinkHandle,
        sink: &mut SinkDispatcher,
        mon: &MonSend,
    ) -> SinkResult<Option<ASinkHandle>> {
        sink.proc_fix(sink_h, mon).await
    }
}

// Note: group-level freeze/ready helpers were unused; individual SinkDispatcher methods cover the case.

impl Appendable<SinkDispatcher> for SinkService {
    fn append(&mut self, ins: SinkDispatcher) {
        self.items.push(ins);
    }
}

impl SinkService {
    pub fn agent(&self) -> SinkRouteAgent {
        let mut items = Vec::new();
        for item in &self.items {
            items.push(SinkGroupAgent::new(
                item.conf().clone(),
                SinkTerminal::Channel(item.get_data_sender()),
            ));
        }
        SinkRouteAgent { items }
    }
    pub(crate) async fn async_sinks_spawn(
        rescue: String,
        table_conf: &SinkRouteTable,
        res_center: &ResManager,
        stat_reqs: Vec<StatReq>,
        rate_limit_rps: usize,
    ) -> RunResult<SinkService> {
        let mut sink_table = SinkService::default();
        for group_conf in &table_conf.group {
            info_ctrl!("init SinkGroup: {}", group_conf.name());
            let p_cnt = group_conf.parallel_cnt();
            for i in 0..p_cnt {
                let sink_group = Self::build_sink_group(
                    rescue.clone(),
                    res_center,
                    &stat_reqs,
                    group_conf,
                    i,
                    p_cnt,
                    rate_limit_rps,
                )
                .await?;
                sink_table.append(sink_group);
            }
        }
        Ok(sink_table)
    }

    async fn build_sink_group(
        rescue: String,
        res_center: &ResManager,
        stat_reqs: &Vec<StatReq>,
        group_conf: &FlexGroup,
        replica_idx: usize,
        replica_cnt: usize,
        rate_limit_rps: usize,
    ) -> Result<SinkDispatcher, RunError> {
        let mut sink_group = SinkDispatcher::new(
            SinkGroupConf::Flexi(group_conf.clone()),
            res_center
                .alloc_sink_res(&SinkID::from(group_conf.name()))
                .await?,
        );
        for conf in group_conf.sinks() {
            Self::init_sink_group(
                rescue.clone(),
                stat_reqs.to_owned(),
                &mut sink_group,
                conf.clone(),
                replica_idx,
                replica_cnt,
                rate_limit_rps,
            )
            .await?;
        }
        Ok(sink_group)
    }

    async fn init_sink_group(
        rescue: String,
        stat_reqs: Vec<StatReq>,
        sink_group: &mut SinkDispatcher,
        conf: SinkInstanceConf,
        replica_idx: usize,
        replica_cnt: usize,
        rate_limit_rps: usize,
    ) -> Result<(), RunError> {
        let sink = build_sink_target(&conf, replica_idx, replica_cnt, rate_limit_rps).await?;

        let mut filter = None;
        if let Some(code) = conf.read_filter_content() {
            let parsed = TCondParser::exp(&mut code.as_str()).owe_rule()?;
            filter = Some(parsed);
            info_data!("sink load filter: {}", conf.name())
        }

        // 运行态名称使用 full_name = group/inner_name（配置装配阶段已注入 group_name）
        let full_name = conf.full_name();
        sink_group.append(SinkRuntime::new(
            rescue.clone(),
            full_name,
            conf.clone(),
            sink,
            filter,
            stat_reqs,
        ));
        Ok(())
    }
}

pub struct ActSink {
    mon_s: MonSend,
    cmd_r: CmdSubscriber,
    bad_s: Option<ASinkSender>,
}

impl ActSink {
    pub fn new(mon_s: MonSend, cmd_r: CmdSubscriber, bad_s: Option<ASinkSender>) -> Self {
        Self {
            mon_s,
            cmd_r,
            bad_s,
        }
    }
}

impl ActSink {
    pub async fn post_to_sink(
        &mut self,
        mut sink_rt: SinkRuntime,
        mut dat_r: SinkDatAReceiver,
    ) -> anyhow::Result<()> {
        info_data!("async sinks proc start");
        let mut run_ctrl = TaskController::new("sink", self.cmd_r.clone(), None);
        loop {
            tokio::select! {
                res = dat_r.recv() => {
                    match res {
                        Some(package) => {
                            // Handle SinkPackage
                            for unit in package.iter() {
                                sink_rt
                                    .send_to_sink(SinkDataEnum::Rec(unit.meta().clone(), unit.data().clone()), Option::from(&self.bad_s), Some(&self.mon_s))
                                    .await?;
                                run_ctrl.rec_task_suc();
                            }
                        }
                        None => {
                            info_ctrl!("sink dat channel closed; exit");
                            break;
                        }
                    }
                }
                Ok(cmd) = run_ctrl.cmds_sub_mut().recv() => {
                    info_ctrl!("sink recv cmd: {}", cmd);
                    run_ctrl.update_cmd(cmd)
                }
                _ = sleep(Duration::from_millis(ACTOR_IDLE_TICK_MS)) => {
                    run_ctrl.rec_task_idle();
                    if run_ctrl.is_stop(){
                        info_ctrl!("async sinks proc stop");
                        break;
                    }
                }
            }
        }
        info_data!(
            "async sinks proc end , total cnt:{}",
            run_ctrl.total_count()
        );
        sink_rt.primary.stop().await?;
        //let snap = sink_rt.stat.swap_snap();
        //self.mon_s.send(StatSlices::Sink(snap)).await?;

        info_ctrl!("async sinks proc end");
        Ok(())
    }
}
