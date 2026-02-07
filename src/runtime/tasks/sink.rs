use orion_error::OperationContext;

use crate::knowledge::KnowdbHandler;
use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;

use crate::runtime::sink::act_sink::{InfraGroups, SinkService, SinkWork};
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::runtime::supervisor::maintenance::ActMaintainer;
use crate::sinks::InfraSinkAgent;
use crate::stat::MonSend;
use std::sync::Arc;

pub fn start_data_sinks(
    infra: InfraSinkAgent,
    act_sink: SinkService,
    mon_send: MonSend,
    act_mt_sink: &mut ActMaintainer,
    knowdb: Option<Arc<KnowdbHandler>>,
) -> TaskGroup {
    let mut ctx = OperationContext::want("start-data-sinks").with_auto_log();
    let mut routine_group = TaskGroup::new("oml-sink", ShutdownCmd::Timeout(200));
    let sink_groups = act_sink.items;
    let knowdb_handler = knowdb;
    for x in sink_groups {
        let (bad_sink_s, fix_sink_r) = act_mt_sink.fix_channel();
        let sink_cmd_sub = routine_group.subscribe();
        let sink_mon = mon_send.clone();

        let cur_infra = infra.clone();
        let sink_name = x.get_name().to_string();
        let knowdb_for_task = knowdb_handler.clone();
        let handle = tokio::spawn(async move {
            if let Some(handler) = knowdb_for_task.as_ref() {
                handler.ensure_thread_ready();
            } else {
                warn_ctrl!("no knowdb handler for {} ", sink_name);
            }
            info_ctrl!("spawn tokio Sink Group {}", x.conf().name());
            if let Err(e) =
                SinkWork::async_proc(x, cur_infra, sink_cmd_sub, sink_mon, bad_sink_s, fix_sink_r)
                    .await
            {
                error_ctrl! { "{}  sink error: {}", sink_name,e}
            }
        });
        routine_group.append(handle);
    }
    ctx.mark_suc();
    routine_group
}
pub fn start_infra_working(
    infra_sink: InfraSinkService,
    mon_send: MonSend,
    infra_group: &TaskGroup,
    act_mt_sink: &mut ActMaintainer,
) {
    let groups = InfraGroups {
        default: infra_sink.default_sink,
        miss: infra_sink.miss_sink,
        residue: infra_sink.residue_sink,
        // intercept removed
        monitor: infra_sink.moni_sink,
        error: infra_sink.err_sink,
    };

    let (bad_sink_s, fix_sink_r) = act_mt_sink.fix_channel();
    let sink_cmd_sub = infra_group.subscribe();
    let sink_mon = mon_send.clone();

    tokio::spawn(async move {
        info_data!("spawn tokio Sink infra Group ");
        if let Err(e) =
            SinkWork::async_proc_infra(groups, sink_cmd_sub, sink_mon, bad_sink_s, fix_sink_r).await
        {
            error_ctrl! { "sink error: {}", e}
        }
    });
}
