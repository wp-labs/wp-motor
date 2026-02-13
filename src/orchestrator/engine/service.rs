use std::time::Duration;

use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::actor::{TaskGroup, TaskManager};
use crate::runtime::supervisor::maintenance::ActMaintainer;
use crate::runtime::tasks::{
    add_acceptor_tasks, start_data_sinks, start_infra_working, start_moni_tasks,
    start_parser_tasks_frames, start_picker_tasks,
};
use tokio::time::sleep;
use wp_conf::{RunArgs, RunMode};
use wp_error::run_error::RunResult;
use wp_stat::StatRequires;
// logging macros
use wp_log::info_ctrl;

use super::resource::EngineResource;
// 启动 sink/infra 的旧版启动器也复用以确保接收端生命周期正确

/// 重要：任务组的追加顺序需与旧版 `start_warp_service` 保持一致，原因如下：
/// - TaskManager 在优雅退出时采用“追加的逆序”进行下线（后进先出）；
/// - 为确保下游（sink/infra/monitor）在上游（parser/picker）之后关闭、且能完整消费残留数据，
///   需要按既定顺序 append：
///   1) monitor → 2) infra → 3) sink → 4) maint → 5) parser；主组设为 pickers；
/// - 接受器（acceptors）属于采集链路的一部分，在 daemon 模式下被加入主组（pickers 组内），
///   以复用旧版“主流程完成→全局退出”的一致语义。
pub async fn start_warp_service(
    mut resource: EngineResource,
    run_mode: RunMode,
    args: RunArgs,
    stat_reqs: StatRequires,
) -> RunResult<TaskManager> {
    let mode_s = match run_mode {
        RunMode::Daemon => "daemon",
        RunMode::Batch => "batch",
    };
    info_ctrl!(
        "start warp-service: run_mode={}, parallel={}, line_max={:?}",
        mode_s,
        args.parallel,
        args.line_max
    );
    let mut task_manager = TaskManager::default();

    // 阶段开关（全局）
    crate::engine_flags::set_skip_parse(args.skip_parse);
    crate::engine_flags::set_skip_sink(args.skip_sink);

    // 语义分析开关（控制 jieba 分词器和语义词典的加载）
    oml::set_semantic_enabled(args.semantic_enabled);

    // 提前设置全局构建期限速提示（发送单元构建期将读取该目标决定背压策略）。
    crate::sinks::set_global_rate_limit_rps(args.speed_limit);

    // 启动监控任务
    let (moni_send, moni_group) = start_moni_tasks(&args, &resource, &stat_reqs);

    // 启动解析任务：即使 skip_parse=true 也保持解析服务运行，仅在解析逻辑内无害化（不做实际解析）。
    let (subsc_channel, parser_group) =
        start_parser_tasks_frames(&args, &resource, moni_send.clone(), &stat_reqs).await?;

    // 启动 sink/infra 任务（确保解析线程的下游接收端已就位、且生命周期覆盖解析期）
    let mut maint_group = TaskGroup::new("amt", ShutdownCmd::Timeout(200));
    let infra_group = TaskGroup::new("infra", ShutdownCmd::Timeout(200));
    let mut sink_amt = ActMaintainer::new(maint_group.subscribe());

    // 提前获取 infra 的 agent，供 sink 组启动使用
    let infra_agent_opt = resource.infra.as_ref().map(|i| i.agent());
    let knowdb_handler = resource.knowdb_handler.clone();

    // 启动基础设施 sink（默认/残留/拦截/监控/错误等），保持其接收端活跃
    if let Some(infra_svc) = resource.infra.take() {
        start_infra_working(infra_svc, moni_send.clone(), &infra_group, &mut sink_amt);
    }

    // 准备业务 sink 组（需要 infra agent），稍后按既定顺序 append
    let mut sink_group_opt = None;
    if let (Some(sinks), Some(infra_agent)) = (resource.sinks.take(), infra_agent_opt) {
        let sink_group = start_data_sinks(
            infra_agent,
            sinks,
            moni_send.clone(),
            &mut sink_amt,
            knowdb_handler.clone(),
        );
        sink_group_opt = Some(sink_group);
    }

    // 维护协程（修复通道/巡检等）
    maint_group.append(tokio::spawn(async move {
        sink_amt.proc().await;
    }));

    // 准备收集与接受器清单（先取接受器，避免被 `get_all_sources` 消费源结构）
    let acceptors_prepared = if matches!(run_mode, RunMode::Daemon) {
        Some(resource.get_all_acceptors())
    } else {
        None
    };
    let all_sources = resource.get_all_sources();

    sleep(Duration::from_millis(100)).await;
    // 启动采集器（pickers）
    let mut picker_group = start_picker_tasks(
        &args,
        all_sources,
        moni_send.clone(),
        subsc_channel,
        &stat_reqs,
    );

    // 启动接受器（acceptors）并纳入主组
    if let Some(all_acceptors) = acceptors_prepared {
        add_acceptor_tasks(&mut picker_group, all_acceptors);
    } else {
        info_ctrl!("run-mode=batch: 跳过启动接受器任务，以避免阻塞主组完成");
    }
    task_manager.append_group(moni_group);
    task_manager.append_group(infra_group);
    if let Some(sg) = sink_group_opt {
        task_manager.append_group(sg);
    }
    task_manager.append_group(maint_group);
    task_manager.append_group(parser_group);
    // 将采集任务组设为主组（主流程），用于整体生命周期与退出判断
    task_manager.set_main(picker_group);

    Ok(task_manager)
}

// 旧的兼容包装与混合模式已移除；统一使用当前架构启动。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_creation() {
        let resource = EngineResource::new();
        assert!(!resource.has_sources());
        assert!(!resource.has_acceptors());
    }

    #[tokio::test]
    async fn test_mixed_service_preparation() {
        // 这个测试需要实际的服务组件才能完成
        // 在实际应用中，你需要准备有效的资源管理器、sink等
        let _resource = EngineResource::new();
        // 这里只测试函数存在性，实际测试需要有效的组件
    }

    #[test]
    fn no_global_backoff_gate_anymore() {
        // 全局 backoff gate 已移除，改为在 NetWriter 构建期与实时水位自决。
    }
}
