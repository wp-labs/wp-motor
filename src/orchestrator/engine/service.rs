use std::time::{Duration, Instant};

use crate::runtime::actor::TaskRole;
use crate::runtime::actor::command::ActorCtrlCmd;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::actor::{TaskGroup, TaskManager};
use crate::runtime::parser::workflow::{ParseDispatchRouter, ParseWorkerSender};
use crate::runtime::reload_drain::{ReloadDrainBus, ReloadDrainTracker};
use crate::runtime::supervisor::maintenance::ActMaintainer;
use crate::runtime::supervisor::monitor::MonitorSinkHandle;
use crate::runtime::tasks::{
    start_acceptor_tasks, start_data_sinks, start_infra_working, start_moni_tasks,
    start_parser_tasks_frames, start_picker_tasks,
};
use crate::stat::MonSend;
use orion_error::{ToStructError, UvsFrom};
use tokio::time::sleep;
use wp_conf::{RunArgs, RunMode};
use wp_error::{RunReason, run_error::RunResult};
use wp_stat::StatRequires;
// logging macros
use wp_log::info_ctrl;

use super::resource::EngineResource;
// 启动 sink/infra 的旧版启动器也复用以确保接收端生命周期正确

pub struct ProcessingTaskSet {
    parser_senders: Vec<ParseWorkerSender>,
    parser_group: TaskGroup,
    infra_group: TaskGroup,
    sink_group: Option<TaskGroup>,
    maint_group: TaskGroup,
    drain_tracker: ReloadDrainTracker,
}

struct DetachedProcessingGroup {
    group: TaskGroup,
    stop_deadline: Instant,
}

impl ProcessingTaskSet {
    pub fn parser_senders(&self) -> Vec<ParseWorkerSender> {
        self.parser_senders.clone()
    }

    pub fn install(self, task_manager: &mut TaskManager) -> ReloadDrainTracker {
        task_manager.append_group_with_role(TaskRole::Infra, self.infra_group);
        if let Some(sg) = self.sink_group {
            task_manager.append_group_with_role(TaskRole::Sink, sg);
        }
        task_manager.append_group_with_role(TaskRole::Maintainer, self.maint_group);
        task_manager.append_group_with_role(TaskRole::Parser, self.parser_group);
        self.drain_tracker
    }

    pub async fn shutdown_unused(mut self) -> RunResult<()> {
        self.parser_group
            .wait_grace_down(Some(ActorCtrlCmd::Stop(ShutdownCmd::Immediate)))
            .await?;
        if let Some(ref mut sink_group) = self.sink_group {
            sink_group
                .wait_grace_down(Some(ActorCtrlCmd::Stop(ShutdownCmd::Immediate)))
                .await?;
        }
        self.infra_group
            .wait_grace_down(Some(ActorCtrlCmd::Stop(ShutdownCmd::Immediate)))
            .await?;
        self.maint_group
            .wait_grace_down(Some(ActorCtrlCmd::Stop(ShutdownCmd::Immediate)))
            .await?;
        Ok(())
    }
}

pub struct EngineRuntime {
    task_manager: TaskManager,
    parse_router: ParseDispatchRouter,
    moni_send: MonSend,
    monitor_sink: MonitorSinkHandle,
    reload_timeout: Duration,
    active_processing: ReloadDrainTracker,
    detached_processing_groups: Vec<DetachedProcessingGroup>,
}

impl EngineRuntime {
    pub fn new(
        task_manager: TaskManager,
        parse_router: ParseDispatchRouter,
        moni_send: MonSend,
        monitor_sink: MonitorSinkHandle,
        reload_timeout: Duration,
        active_processing: ReloadDrainTracker,
    ) -> Self {
        Self {
            task_manager,
            parse_router,
            moni_send,
            monitor_sink,
            reload_timeout,
            active_processing,
            detached_processing_groups: Vec::new(),
        }
    }

    pub fn task_manager_mut(&mut self) -> &mut TaskManager {
        &mut self.task_manager
    }

    pub fn monitor_sender(&self) -> MonSend {
        self.moni_send.clone()
    }

    pub fn reload_timeout(&self) -> Duration {
        self.reload_timeout
    }

    pub fn replace_monitor_sink(&self, sink: Option<crate::facade::test_helpers::SinkTerminal>) {
        self.monitor_sink.replace(sink);
    }

    pub fn parse_router_snapshot(&self) -> Vec<ParseWorkerSender> {
        self.parse_router.snapshot()
    }

    pub fn disconnect_parse_router(&self) {
        self.parse_router.begin_reload();
    }

    pub fn replace_parse_router(&self, parser_senders: Vec<ParseWorkerSender>) {
        self.parse_router.replace(parser_senders);
    }

    pub async fn isolate_picker(&mut self) -> RunResult<()> {
        self.prune_finished_background_processing().await?;
        self.task_manager.isolate_role(TaskRole::Picker).await
    }

    pub async fn resume_picker(&mut self) -> RunResult<()> {
        self.prune_finished_background_processing().await?;
        self.task_manager.execute_role_all(TaskRole::Picker).await
    }

    pub fn install_processing_tasks(&mut self, processing: ProcessingTaskSet) {
        self.replace_parse_router(processing.parser_senders());
        self.active_processing = processing.install(&mut self.task_manager);
    }

    pub fn next_processing_epoch(&self) -> u64 {
        self.active_processing.next_epoch()
    }

    pub async fn wait_processing_drained(&mut self, deadline: Instant) -> RunResult<()> {
        while !self.active_processing.is_fully_quiesced() {
            let wait_timeout = remaining_timeout(deadline)?;
            let event = tokio::time::timeout(wait_timeout, self.active_processing.recv())
                .await
                .map_err(|_| RunReason::from_logic().to_err())?
                .ok_or_else(|| RunReason::from_logic().to_err())?;
            self.active_processing
                .observe(&event)
                .map_err(|detail| RunReason::from_logic().to_err().with_detail(detail))?;
        }
        self.task_manager
            .stop_role(TaskRole::Maintainer, ShutdownCmd::Immediate)
            .await?;
        self.detach_processing_roles(Instant::now() + self.reload_timeout);
        self.prune_finished_background_processing().await?;
        Ok(())
    }

    pub async fn force_stop_processing(&mut self) -> RunResult<()> {
        self.task_manager
            .stop_role(TaskRole::Parser, ShutdownCmd::Immediate)
            .await?;
        self.task_manager
            .stop_role(TaskRole::Sink, ShutdownCmd::Immediate)
            .await?;
        self.task_manager
            .stop_role(TaskRole::Infra, ShutdownCmd::Immediate)
            .await?;
        self.task_manager
            .stop_role(TaskRole::Maintainer, ShutdownCmd::Immediate)
            .await?;
        self.prune_finished_background_processing().await?;
        Ok(())
    }

    pub async fn shutdown(
        mut self,
        policy_kind: crate::runtime::actor::ExitPolicyKind,
    ) -> RunResult<()> {
        if !self.task_manager.is_empty() {
            self.task_manager.all_down_wait_policy(policy_kind).await?;
        }
        self.shutdown_detached_processing_groups().await
    }

    pub async fn shutdown_with_signal(
        mut self,
        policy_kind: crate::runtime::actor::ExitPolicyKind,
        initial_signal_received: bool,
    ) -> RunResult<()> {
        if !self.task_manager.is_empty() {
            self.task_manager
                .all_down_wait_policy_with_signal(policy_kind, initial_signal_received)
                .await?;
        }
        self.shutdown_detached_processing_groups().await
    }

    fn detach_processing_roles(&mut self, stop_deadline: Instant) {
        for role in [TaskRole::Parser, TaskRole::Sink, TaskRole::Infra] {
            self.detached_processing_groups.extend(
                self.task_manager
                    .detach_role(role)
                    .into_iter()
                    .map(|group| DetachedProcessingGroup {
                        group,
                        stop_deadline,
                    }),
            );
        }
    }

    async fn prune_finished_background_processing(&mut self) -> RunResult<()> {
        self.task_manager.prune_finished_groups().await?;
        let mut running = Vec::with_capacity(self.detached_processing_groups.len());
        let now = Instant::now();
        for mut detached in self.detached_processing_groups.drain(..) {
            if detached.group.routin_is_finished() {
                detached.group.wait_finished().await?;
            } else if now >= detached.stop_deadline {
                detached
                    .group
                    .wait_grace_down_with_timeout(
                        Some(ActorCtrlCmd::Stop(ShutdownCmd::Immediate)),
                        detached_force_stop_timeout(self.reload_timeout),
                    )
                    .await?;
            } else {
                running.push(detached);
            }
        }
        self.detached_processing_groups = running;
        Ok(())
    }

    async fn shutdown_detached_processing_groups(&mut self) -> RunResult<()> {
        for mut detached in self.detached_processing_groups.drain(..) {
            detached
                .group
                .wait_grace_down_with_timeout(
                    Some(ActorCtrlCmd::Stop(ShutdownCmd::Immediate)),
                    detached_force_stop_timeout(self.reload_timeout),
                )
                .await?;
        }
        Ok(())
    }
}

fn detached_force_stop_timeout(reload_timeout: Duration) -> Duration {
    reload_timeout
        .min(Duration::from_secs(1))
        .max(Duration::from_millis(50))
}

fn remaining_timeout(deadline: Instant) -> RunResult<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| RunReason::from_logic().to_err())
}

pub async fn start_processing_tasks(
    args: &RunArgs,
    mut resource: EngineResource,
    moni_send: MonSend,
    stat_reqs: &StatRequires,
    reload_epoch: u64,
) -> RunResult<ProcessingTaskSet> {
    let (drain_bus, drain_rx) = ReloadDrainBus::new(reload_epoch);
    let (subsc_channel, parser_group) =
        start_parser_tasks_frames(args, &resource, moni_send.clone(), stat_reqs, &drain_bus)
            .await?;

    let mut maint_group = TaskGroup::new("amt", ShutdownCmd::Timeout(200));
    let mut infra_group = TaskGroup::new("infra", ShutdownCmd::Timeout(200));
    let mut sink_amt = ActMaintainer::new(maint_group.subscribe());

    let infra_total = usize::from(resource.infra.is_some());
    let infra_agent_opt = resource.infra.as_ref().map(|i| i.agent());
    let knowdb_handler = resource.knowdb_handler.clone();

    if let Some(infra_svc) = resource.infra.take() {
        start_infra_working(
            infra_svc,
            moni_send.clone(),
            &mut infra_group,
            &mut sink_amt,
            &drain_bus,
        );
    }

    let parser_total = subsc_channel.len();
    let sink_total = if infra_agent_opt.is_some() {
        resource
            .sinks
            .as_ref()
            .map(|svc| svc.items.len())
            .unwrap_or(0)
    } else {
        0
    };
    let sink_group_opt =
        if let (Some(sinks), Some(infra_agent)) = (resource.sinks.take(), infra_agent_opt) {
            Some(start_data_sinks(
                infra_agent,
                sinks,
                moni_send.clone(),
                &mut sink_amt,
                knowdb_handler.clone(),
                &drain_bus,
            ))
        } else {
            None
        };

    maint_group.append(tokio::spawn(async move {
        sink_amt.proc().await;
    }));

    Ok(ProcessingTaskSet {
        parser_senders: subsc_channel,
        parser_group,
        infra_group,
        sink_group: sink_group_opt,
        maint_group,
        drain_tracker: ReloadDrainTracker::new(
            reload_epoch,
            parser_total,
            sink_total,
            infra_total,
            drain_rx,
        ),
    })
}

/// 重要：任务组统一按角色注册，由 ExitPolicy 决定退出时机（不再通过“主组”切换语义）。
pub async fn start_warp_service(
    mut resource: EngineResource,
    run_mode: RunMode,
    args: RunArgs,
    stat_reqs: StatRequires,
) -> RunResult<EngineRuntime> {
    let mode_s = match run_mode {
        RunMode::Daemon => "daemon",
        RunMode::Batch => "batch",
    };
    info_ctrl!(
        "start warp-service: run_mode={}, parallel={}, line_max={:?}, reload_timeout_ms={}",
        mode_s,
        args.parallel,
        args.line_max,
        args.reload_timeout_ms
    );
    let mut task_manager = TaskManager::default();

    // 阶段开关（全局）
    crate::engine_flags::set_skip_parse(args.skip_parse);
    crate::engine_flags::set_skip_sink(args.skip_sink);

    // 语义分析开关（控制 jieba 分词器和语义词典的加载）
    oml::set_semantic_enabled(args.semantic_enabled);
    info_ctrl!(
        "semantic analysis: {}",
        if args.semantic_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );

    // 提前设置全局构建期限速提示（发送单元构建期将读取该目标决定背压策略）。
    crate::sinks::set_global_rate_limit_rps(args.speed_limit);

    // 启动监控任务
    let (moni_send, moni_group, monitor_sink) = start_moni_tasks(&args, &resource, &stat_reqs);
    crate::knowledge::attach_stats_monitor_sender(moni_send.clone());

    // 准备收集与接受器清单（先取接受器，避免被 `get_all_sources` 消费源结构）
    let mut acceptor_group_opt = if matches!(run_mode, RunMode::Daemon) {
        let acceptors = resource.get_all_acceptors();
        Some(start_acceptor_tasks(acceptors))
    } else {
        None
    };
    let all_sources = resource.get_all_sources();

    // daemon 模式下 acceptor 作为独立角色纳入策略机
    if let Some(acceptor_group) = acceptor_group_opt.take() {
        task_manager.append_group_with_role(TaskRole::Acceptor, acceptor_group);
    } else {
        info_ctrl!("run-mode=batch: 跳过启动接受器任务");
    }
    task_manager.append_group_with_role(TaskRole::Monitor, moni_group);
    let processing =
        start_processing_tasks(&args, resource, moni_send.clone(), &stat_reqs, 1).await?;
    let parse_router = ParseDispatchRouter::new(processing.parser_senders());
    let active_processing = processing.install(&mut task_manager);

    sleep(Duration::from_millis(100)).await;
    let picker_group = start_picker_tasks(
        &args,
        all_sources,
        moni_send.clone(),
        parse_router.clone(),
        &stat_reqs,
    );
    task_manager.append_group_with_role(TaskRole::Picker, picker_group);

    Ok(EngineRuntime::new(
        task_manager,
        parse_router,
        moni_send,
        monitor_sink,
        Duration::from_millis(args.reload_timeout_ms),
        active_processing,
    ))
}

// 旧的兼容包装与混合模式已移除；统一使用当前架构启动。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::actor::ExitPolicyKind;
    use crate::runtime::actor::command::{ActorCtrlCmd, TaskScope};
    use crate::runtime::actor::exit_policy::ExitAction;
    use crate::runtime::reload_drain::{ReloadDrainBus, ReloadDrainReporter, ReloadDrainTracker};
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;
    use wp_connector_api::{SourceBatch, SourceEvent, Tags};
    use wp_model_core::raw::RawData;

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

    fn make_stop_only_group(name: &str) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let mut cmd_sub = group.subscribe();
        group.append(tokio::spawn(async move {
            loop {
                match cmd_sub.recv().await {
                    Ok(ActorCtrlCmd::Stop(_)) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        }));
        group
    }

    fn make_auto_finish_group(name: &str, delay_ms: u64) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        group.append(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }));
        group
    }

    fn make_reload_drain_tracker(
        epoch: u64,
        parser_total: usize,
        sink_total: usize,
        infra_total: usize,
    ) -> (ReloadDrainBus, ReloadDrainTracker) {
        let (bus, rx) = ReloadDrainBus::new(epoch);
        (
            bus,
            ReloadDrainTracker::new(epoch, parser_total, sink_total, infra_total, rx),
        )
    }

    const TEST_RELOAD_TIMEOUT: Duration = Duration::from_millis(120);

    fn make_parser_group(
        name: &str,
        observe: Option<mpsc::Sender<usize>>,
        mut drain_reporter: Option<ReloadDrainReporter>,
    ) -> (TaskGroup, ParseWorkerSender) {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let mut cmd_sub = group.subscribe();
        let (tx, mut rx) = mpsc::channel::<SourceBatch>(8);
        group.append(tokio::spawn(async move {
            loop {
                tokio::select! {
                    recv = rx.recv() => {
                        match recv {
                            Some(batch) => {
                                if let Some(obs) = observe.as_ref() {
                                    let _ = obs.send(batch.len()).await;
                                }
                            }
                            None => {
                                if let Some(reporter) = drain_reporter.as_mut() {
                                    reporter.notify();
                                }
                                break;
                            }
                        }
                    }
                    cmd = cmd_sub.recv() => {
                        match cmd {
                            Ok(ActorCtrlCmd::Stop(_)) | Err(_) => {
                                if let Some(reporter) = drain_reporter.as_mut() {
                                    reporter.notify();
                                }
                                break;
                            }
                            Ok(_) => {}
                        }
                    }
                }
            }
        }));
        (group, ParseWorkerSender::new(tx))
    }

    fn make_drain_then_tail_group(
        name: &str,
        drain_delay_ms: u64,
        tail_delay_ms: u64,
        mut reporter: ReloadDrainReporter,
    ) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        group.append(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(drain_delay_ms)).await;
            reporter.notify();
            tokio::time::sleep(Duration::from_millis(tail_delay_ms)).await;
        }));
        group
    }

    fn make_drain_then_tail_parser_group(
        name: &str,
        drain_delay_ms: u64,
        tail_delay_ms: u64,
        mut reporter: ReloadDrainReporter,
    ) -> (TaskGroup, ParseWorkerSender) {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let (tx, _rx) = mpsc::channel::<SourceBatch>(8);
        group.append(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(drain_delay_ms)).await;
            reporter.notify();
            tokio::time::sleep(Duration::from_millis(tail_delay_ms)).await;
        }));
        (group, ParseWorkerSender::new(tx))
    }

    fn make_abort_group(
        name: &str,
        abort_delay_ms: u64,
        reporter: ReloadDrainReporter,
    ) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        group.append(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(abort_delay_ms)).await;
            drop(reporter);
        }));
        group
    }

    fn make_abort_parser_group(
        name: &str,
        abort_delay_ms: u64,
        reporter: ReloadDrainReporter,
    ) -> (TaskGroup, ParseWorkerSender) {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let (tx, _rx) = mpsc::channel::<SourceBatch>(8);
        group.append(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(abort_delay_ms)).await;
            drop(reporter);
        }));
        (group, ParseWorkerSender::new(tx))
    }

    fn make_picker_group(events: Arc<Mutex<Vec<&'static str>>>) -> TaskGroup {
        let mut group = TaskGroup::new("picker", ShutdownCmd::Immediate);
        let mut cmd_sub = group.subscribe();
        group.append(tokio::spawn(async move {
            loop {
                match cmd_sub.recv().await {
                    Ok(ActorCtrlCmd::Isolate) => {
                        events.lock().expect("lock picker events").push("isolate");
                    }
                    Ok(ActorCtrlCmd::Execute(TaskScope::All)) => {
                        events.lock().expect("lock picker events").push("resume");
                    }
                    Ok(ActorCtrlCmd::Stop(_)) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        }));
        group
    }

    fn make_batch(tag: &str) -> SourceBatch {
        let mut tags = Tags::new();
        tags.set("tag", tag.to_string());
        vec![SourceEvent::new(
            1,
            tag,
            RawData::from_string(tag.to_string()),
            Arc::new(tags),
        )]
    }

    #[tokio::test]
    async fn runtime_reload_flow_forces_replace_after_graceful_timeout() {
        let (mon_tx, _mon_rx) = mpsc::channel(8);
        let picker_events = Arc::new(Mutex::new(Vec::new()));
        let (old_drain_bus, old_drain_tracker) = make_reload_drain_tracker(1, 1, 0, 0);

        let (old_parser_group, old_sender) = make_parser_group(
            "old-parser",
            None,
            Some(old_drain_bus.reporter(TaskRole::Parser, "old-parser")),
        );
        let old_processing = ProcessingTaskSet {
            parser_senders: vec![old_sender],
            parser_group: old_parser_group,
            infra_group: make_auto_finish_group("old-infra", 20),
            sink_group: Some(make_auto_finish_group("old-sink", 20)),
            maint_group: make_stop_only_group("old-maint"),
            drain_tracker: old_drain_tracker,
        };

        let mut task_manager = TaskManager::default();
        let parse_router = ParseDispatchRouter::new(old_processing.parser_senders());
        let active_processing = old_processing.install(&mut task_manager);
        task_manager
            .append_group_with_role(TaskRole::Picker, make_picker_group(picker_events.clone()));

        let mut runtime = EngineRuntime::new(
            task_manager,
            parse_router,
            mon_tx,
            MonitorSinkHandle::default(),
            TEST_RELOAD_TIMEOUT,
            active_processing,
        );
        let held_old_senders = runtime.parse_router_snapshot();

        let (observed_tx, mut observed_rx) = mpsc::channel(4);
        let (new_drain_bus, new_drain_tracker) = make_reload_drain_tracker(2, 1, 0, 0);
        let (new_parser_group, new_sender) = make_parser_group(
            "new-parser",
            Some(observed_tx),
            Some(new_drain_bus.reporter(TaskRole::Parser, "new-parser")),
        );
        let new_processing = ProcessingTaskSet {
            parser_senders: vec![new_sender],
            parser_group: new_parser_group,
            infra_group: make_auto_finish_group("new-infra", 20),
            sink_group: Some(make_auto_finish_group("new-sink", 20)),
            maint_group: make_stop_only_group("new-maint"),
            drain_tracker: new_drain_tracker,
        };

        runtime.isolate_picker().await.expect("isolate picker");
        runtime.disconnect_parse_router();
        let drain_res = runtime
            .wait_processing_drained(Instant::now() + Duration::from_millis(80))
            .await;
        assert!(
            drain_res.is_err(),
            "held parser senders should keep old parser alive and force timeout"
        );

        runtime
            .force_stop_processing()
            .await
            .expect("force stop old processing");
        runtime.install_processing_tasks(new_processing);
        runtime.resume_picker().await.expect("resume picker");
        drop(held_old_senders);

        let sender = runtime
            .parse_router_snapshot()
            .into_iter()
            .next()
            .expect("new parser sender");
        sender
            .dat_s
            .try_send(make_batch("reloaded"))
            .expect("route batch to new parser");
        let observed = tokio::time::timeout(Duration::from_secs(1), observed_rx.recv())
            .await
            .expect("observe parser batch")
            .expect("parser observation");
        assert_eq!(observed, 1, "new parser should receive re-routed batch");

        let events = picker_events.lock().expect("lock picker events").clone();
        assert_eq!(events, vec!["isolate", "resume"]);

        runtime
            .task_manager_mut()
            .all_down_force_policy(ExitPolicyKind::Batch)
            .await
            .expect("shutdown runtime");
    }

    #[tokio::test]
    async fn runtime_reload_returns_after_processing_quiesced_before_tasks_finish() {
        let (mon_tx, _mon_rx) = mpsc::channel(8);
        let (drain_bus, drain_tracker) = make_reload_drain_tracker(1, 1, 1, 1);
        let (parser_group, parser_sender) = make_drain_then_tail_parser_group(
            "parser-tail",
            20,
            180,
            drain_bus.reporter(TaskRole::Parser, "parser-tail"),
        );
        let processing = ProcessingTaskSet {
            parser_senders: vec![parser_sender],
            parser_group,
            infra_group: make_drain_then_tail_group(
                "infra-tail",
                25,
                180,
                drain_bus.reporter(TaskRole::Infra, "infra-tail"),
            ),
            sink_group: Some(make_drain_then_tail_group(
                "sink-tail",
                30,
                180,
                drain_bus.reporter(TaskRole::Sink, "sink-tail"),
            )),
            maint_group: make_stop_only_group("maint-tail"),
            drain_tracker,
        };

        let mut task_manager = TaskManager::default();
        let parse_router = ParseDispatchRouter::new(processing.parser_senders());
        let active_processing = processing.install(&mut task_manager);
        let mut runtime = EngineRuntime::new(
            task_manager,
            parse_router,
            mon_tx,
            MonitorSinkHandle::default(),
            TEST_RELOAD_TIMEOUT,
            active_processing,
        );

        let started_at = Instant::now();
        runtime
            .wait_processing_drained(Instant::now() + Duration::from_millis(120))
            .await
            .expect("processing should quiesce before task tail finishes");
        assert!(
            started_at.elapsed() < Duration::from_millis(120),
            "wait_processing_drained should return on quiesced events, not wait for full task finish"
        );

        tokio::time::sleep(Duration::from_millis(220)).await;
    }

    #[tokio::test]
    async fn runtime_prunes_detached_processing_after_tail_deadline() {
        let (mon_tx, _mon_rx) = mpsc::channel(8);
        let (drain_bus, drain_tracker) = make_reload_drain_tracker(1, 1, 1, 1);
        let (parser_group, parser_sender) = make_drain_then_tail_parser_group(
            "parser-stuck-tail",
            20,
            1_000,
            drain_bus.reporter(TaskRole::Parser, "parser-stuck-tail"),
        );
        let processing = ProcessingTaskSet {
            parser_senders: vec![parser_sender],
            parser_group,
            infra_group: make_drain_then_tail_group(
                "infra-stuck-tail",
                25,
                1_000,
                drain_bus.reporter(TaskRole::Infra, "infra-stuck-tail"),
            ),
            sink_group: Some(make_drain_then_tail_group(
                "sink-stuck-tail",
                30,
                1_000,
                drain_bus.reporter(TaskRole::Sink, "sink-stuck-tail"),
            )),
            maint_group: make_stop_only_group("maint-stuck-tail"),
            drain_tracker,
        };

        let mut task_manager = TaskManager::default();
        let parse_router = ParseDispatchRouter::new(processing.parser_senders());
        let active_processing = processing.install(&mut task_manager);
        let mut runtime = EngineRuntime::new(
            task_manager,
            parse_router,
            mon_tx,
            MonitorSinkHandle::default(),
            TEST_RELOAD_TIMEOUT,
            active_processing,
        );

        runtime
            .wait_processing_drained(Instant::now() + Duration::from_millis(120))
            .await
            .expect("processing should quiesce before tail deadline");
        assert!(
            !runtime.detached_processing_groups.is_empty(),
            "detached processing should still be tracked before tail deadline"
        );

        tokio::time::sleep(TEST_RELOAD_TIMEOUT + Duration::from_millis(40)).await;
        runtime
            .isolate_picker()
            .await
            .expect("pruning overdue detached processing should not fail");
        assert!(
            runtime.detached_processing_groups.is_empty(),
            "overdue detached processing should be force-pruned"
        );
    }

    #[tokio::test]
    async fn runtime_reload_aborted_worker_forces_error_path() {
        let (mon_tx, _mon_rx) = mpsc::channel(8);
        let (drain_bus, drain_tracker) = make_reload_drain_tracker(1, 1, 1, 1);
        let (parser_group, parser_sender) = make_abort_parser_group(
            "parser-abort",
            20,
            drain_bus.reporter(TaskRole::Parser, "parser-abort"),
        );
        let processing = ProcessingTaskSet {
            parser_senders: vec![parser_sender],
            parser_group,
            infra_group: make_abort_group(
                "infra-abort",
                25,
                drain_bus.reporter(TaskRole::Infra, "infra-abort"),
            ),
            sink_group: Some(make_abort_group(
                "sink-abort",
                30,
                drain_bus.reporter(TaskRole::Sink, "sink-abort"),
            )),
            maint_group: make_stop_only_group("maint-abort"),
            drain_tracker,
        };

        let mut task_manager = TaskManager::default();
        let parse_router = ParseDispatchRouter::new(processing.parser_senders());
        let active_processing = processing.install(&mut task_manager);
        let mut runtime = EngineRuntime::new(
            task_manager,
            parse_router,
            mon_tx,
            MonitorSinkHandle::default(),
            TEST_RELOAD_TIMEOUT,
            active_processing,
        );

        let err = runtime
            .wait_processing_drained(Instant::now() + Duration::from_millis(120))
            .await
            .expect_err("aborted workers should fail drain wait");
        assert!(
            err.to_string().contains("reload drain aborted"),
            "aborted drain should surface explicit reason"
        );
    }

    #[tokio::test]
    async fn batch_disconnects_parse_router_only_after_entering_quiescing() {
        let (mon_tx, _mon_rx) = mpsc::channel(8);
        let (drain_bus, drain_tracker) = make_reload_drain_tracker(1, 1, 0, 0);

        let (parser_group, parser_sender) = make_parser_group(
            "batch-parser",
            None,
            Some(drain_bus.reporter(TaskRole::Parser, "batch-parser")),
        );
        let processing = ProcessingTaskSet {
            parser_senders: vec![parser_sender],
            parser_group,
            infra_group: make_auto_finish_group("batch-infra", 20),
            sink_group: Some(make_auto_finish_group("batch-sink", 20)),
            maint_group: make_stop_only_group("batch-maint"),
            drain_tracker,
        };

        let mut task_manager = TaskManager::default();
        let parse_router = ParseDispatchRouter::new(processing.parser_senders());
        let active_processing = processing.install(&mut task_manager);
        task_manager
            .append_group_with_role(TaskRole::Picker, make_auto_finish_group("batch-picker", 20));

        let mut runtime = EngineRuntime::new(
            task_manager,
            parse_router,
            mon_tx,
            MonitorSinkHandle::default(),
            TEST_RELOAD_TIMEOUT,
            active_processing,
        );
        tokio::time::sleep(Duration::from_millis(200)).await;

        let running_action = runtime
            .task_manager_mut()
            .next_exit_policy_action(
                ExitPolicyKind::Batch,
                crate::runtime::actor::exit_policy::ExitPhase::Running,
                Instant::now(),
                false,
            )
            .expect("running phase action");
        assert!(
            matches!(running_action, ExitAction::EnterQuiescing(_)),
            "batch policy should enter quiescing once picker finishes, got {running_action:?}"
        );

        let quiescing_before_disconnect = runtime
            .task_manager_mut()
            .next_exit_policy_action(
                ExitPolicyKind::Batch,
                crate::runtime::actor::exit_policy::ExitPhase::Quiescing,
                Instant::now(),
                false,
            )
            .expect("quiescing phase action");
        assert!(
            matches!(quiescing_before_disconnect, ExitAction::Stay),
            "before disconnecting router, parser should still be kept alive by runtime-held senders; got {quiescing_before_disconnect:?}"
        );

        runtime.disconnect_parse_router();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let quiescing_after_disconnect = runtime
            .task_manager_mut()
            .next_exit_policy_action(
                ExitPolicyKind::Batch,
                crate::runtime::actor::exit_policy::ExitPhase::Quiescing,
                Instant::now(),
                false,
            )
            .expect("quiescing phase action after disconnect");
        assert!(
            matches!(quiescing_after_disconnect, ExitAction::EnterStopping(_)),
            "after disconnecting router, parser should finish and batch policy should stop; got {quiescing_after_disconnect:?}"
        );
    }
}
