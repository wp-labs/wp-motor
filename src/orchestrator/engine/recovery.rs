use crate::knowledge::KnowdbHandler;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::actor::{TaskGroup, TaskManager};
use crate::runtime::collector::recovery::ActCovPicker;
use crate::runtime::sink::act_sink::SinkService;
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::runtime::supervisor::maintenance::ActMaintainer;
use crate::runtime::supervisor::monitor::ActorMonitor;
use crate::runtime::tasks::{start_data_sinks, start_infra_working};
use std::sync::Arc;
use wp_conf::RunArgs;
use wp_error::run_error::RunResult;
use wp_stat::StatRequires;
use wp_stat::StatStage;

pub async fn recover_main(
    infra_sink: InfraSinkService,
    args: RunArgs,
    source: &str,
    act_sink: SinkService,
    stat_reqs: StatRequires,
    knowdb_handler: Option<Arc<KnowdbHandler>>,
) -> RunResult<()> {
    // 在恢复模式下，业务 sink 组需要保持可写（Ready）以接收从 rescue 读取的恢复数据。

    let mut mon_group = TaskGroup::new("moni", ShutdownCmd::Timeout(200));
    let infra_group = TaskGroup::new("infra", ShutdownCmd::Timeout(200));
    let mut actor_mon = ActorMonitor::new(
        mon_group.subscribe(),
        Some(infra_sink.moni_agent()),
        true,
        args.stat_sec,
    );
    let mon_send = actor_mon.send_agent();
    // 传递所有统计需求给监控器，以便正确处理和显示统计信息
    let monitor_reqs = stat_reqs.get_all().clone();
    mon_group.append(tokio::spawn(async move {
        let _ = actor_mon.stat_proc(monitor_reqs).await;
    }));

    let mut picker_group = TaskGroup::new("pick", ShutdownCmd::Immediate);
    // 默认空闲 3 秒自动退出；未来可加 CLI 开关覆盖
    let actor_picker = ActCovPicker::new(
        picker_group.subscribe(),
        source,
        args.speed_limit,
        mon_send.clone(),
        Some(std::time::Duration::from_secs(3)),
    );
    let mut mt_group = TaskGroup::new("maintainer", ShutdownCmd::Timeout(200));

    let agent = act_sink.agent();
    let mut sink_amt = ActMaintainer::new(mt_group.subscribe());
    let infra_agent = infra_sink.agent();
    start_infra_working(infra_sink, mon_send.clone(), &infra_group, &mut sink_amt);

    let sink_group = start_data_sinks(
        infra_agent,
        act_sink,
        mon_send,
        &mut sink_amt,
        knowdb_handler,
    );
    //sink_group.broadcast_cmd(CtrlCmd::Work(DoScope::One(sink_name.clone())));

    mt_group.append(tokio::spawn(async move {
        sink_amt.proc().await;
    }));

    let pick_reqs = stat_reqs.get_requ_items(StatStage::Pick);
    picker_group.append(tokio::spawn(async move {
        let _ = actor_picker.pick_data(agent, pick_reqs).await;
    }));
    let mut rt_admin = TaskManager::default();
    rt_admin.append_group(mon_group);
    rt_admin.append_group(sink_group);
    rt_admin.append_group(mt_group);
    rt_admin.append_group(infra_group);
    rt_admin.set_main(picker_group);
    rt_admin.all_down_wait_signal().await?;

    Ok(())
}
