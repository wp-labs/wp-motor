use crate::orchestrator::engine::resource::EngineResource;
use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::supervisor::monitor::ActorMonitor;
use crate::stat::MonSend;
use wp_conf::RunArgs;
use wp_stat::StatRequires;

// 启动 sink/infra 的旧版启动器也复用以确保接收端生命周期正确

pub fn start_moni_tasks(
    args: &RunArgs,
    resource: &EngineResource,
    stat_reqs: &StatRequires,
) -> (MonSend, TaskGroup) {
    let mut moni_group = TaskGroup::new("monitor", ShutdownCmd::Immediate);
    // 为与旧版 start_warp_service 对齐，这里统计全阶段（Pick/Parse/Sink/…）
    let reqs = stat_reqs.get_all().clone();
    // 若存在 infra，则与旧版一致，将监控数据写入 infra 的 moni sink；否则为 None
    let moni_sink = resource.infra.as_ref().map(|i| i.moni_agent());
    let mut monitor = ActorMonitor::new(
        moni_group.subscribe(),
        moni_sink,
        args.stat_print,
        args.stat_sec,
    );
    // 使用 ActorMonitor 内部通道的发送端，确保接收端在 `monitor.stat_proc` 中被持续消费
    let mon_send = monitor.send_agent();

    moni_group.append(tokio::spawn(async move {
        if let Err(e) = monitor.stat_proc(reqs).await {
            error_ctrl!("monitor error:{}", e);
        }
    }));

    info_ctrl!(
        "数据源配置统计: V2源={}个, 接受器(来自V2)={}个",
        resource.source_count(),
        resource.acceptor_count()
    );

    (mon_send, moni_group)
}
