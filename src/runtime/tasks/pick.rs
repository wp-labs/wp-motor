use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::limit::SourceRateLimiter;
use crate::runtime::actor::limit::source_auto_initial_rate_is_overridden;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::collector::realtime::SourceWorker;
use crate::runtime::collector::realtime::picker::auto_limit::AutoRateController;
use crate::runtime::parser::workflow::ParseDispatchRouter;
use crate::stat::MonSend;
use wp_conf::RunArgs;
use wp_connector_api::SourceHandle;
use wp_stat::StatRequires;
use wp_stat::StatStage;

/// 启动采集任务（pickers）
/// 使用 Frame 订阅通道启动采集任务（将 SourceFrame 分发到解析线程）
pub fn start_picker_tasks(
    run_args: &RunArgs,
    all_sources: Vec<SourceHandle>,
    mon_send: MonSend,
    parse_router: ParseDispatchRouter,
    stat_reqs: &StatRequires,
) -> TaskGroup {
    let mut picker_group = TaskGroup::new("picker", ShutdownCmd::Immediate);
    info_ctrl!("启动数据收集(Frame)： {}个数据源", all_sources.len());
    let source_rate_limiter = SourceRateLimiter::new(run_args.speed_limit);
    let auto_rate_controller = source_rate_limiter
        .as_ref()
        .filter(|limiter| limiter.is_auto())
        .map(|limiter| {
            let workers = run_args.parallel.max(1);
            if !source_auto_initial_rate_is_overridden() {
                limiter.set_rate_per_sec(default_auto_initial_rate(workers));
            }
            AutoRateController::shared_for_workers(workers)
        });
    for source_h in all_sources {
        let worker = SourceWorker::new(
            run_args.speed_limit,
            run_args.line_max,
            run_args.fetch_timeout_ms,
            mon_send.clone(),
            parse_router.clone(),
            source_rate_limiter.clone(),
            auto_rate_controller.clone(),
        );
        let cmd_sub = picker_group.subscribe();
        let c_args = run_args.clone();
        let reqs = stat_reqs.get_requ_items(StatStage::Pick);
        info_ctrl!(
            "spawning picker for source '{}' (line_max={:?}, speed_limit={})",
            source_h.source.identifier(),
            c_args.line_max,
            c_args.speed_limit
        );
        picker_group.append(tokio::spawn(async move {
            let max_line = c_args.line_max;
            let source_id = source_h.source.identifier();
            info_ctrl!("启动数据源 picker(Frame): {}", source_id);
            if let Err(e) = worker.run(source_h.source, cmd_sub, max_line, reqs).await {
                error_ctrl!("数据源 '{}' picker 错误: {}", source_id, e);
            } else {
                info_ctrl!("数据源 '{}' picker 正常结束", source_id);
            }
        }));
    }
    picker_group
}

fn default_auto_initial_rate(workers: usize) -> usize {
    let _ = workers;
    10_000
}

#[cfg(test)]
mod tests {
    use super::default_auto_initial_rate;

    #[test]
    fn auto_initial_rate_is_conservative_and_worker_independent() {
        assert_eq!(default_auto_initial_rate(0), 10_000);
        assert_eq!(default_auto_initial_rate(4), 10_000);
        assert_eq!(default_auto_initial_rate(10), 10_000);
    }
}
