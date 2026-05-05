use super::TaskGroup;
use super::exit_policy::{
    ExitAction, ExitPhase, ExitPolicyKind, ExitTrigger, RuntimeSnapshot, TaskRole,
    build_exit_policy,
};
use super::signal::stop_signals;
use crate::runtime::actor::command::ActorCtrlCmd;
use crate::runtime::actor::signal::ShutdownCmd;
use futures_lite::prelude::*;
use orion_error::conversion::ToStructError;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use wp_error::{RunReason, run_error::RunResult};
use wp_log::info_ctrl;

#[cfg(test)]
const DAEMON_FORCE_STOP_TIMEOUT: Duration = Duration::from_millis(300);
#[cfg(not(test))]
const DAEMON_FORCE_STOP_TIMEOUT: Duration = Duration::from_secs(5);

struct RoleTaskGroup {
    role: TaskRole,
    group: TaskGroup,
}

#[derive(Default)]
pub struct TaskManager {
    role_groups: Vec<RoleTaskGroup>,
}

impl TaskManager {
    pub fn append_group_with_role(&mut self, role: TaskRole, group: TaskGroup) {
        self.role_groups.push(RoleTaskGroup { role, group });
    }

    pub fn is_empty(&self) -> bool {
        self.role_groups.is_empty()
    }

    pub fn detach_role(&mut self, role: TaskRole) -> Vec<TaskGroup> {
        let mut detached = Vec::new();
        let mut kept = Vec::with_capacity(self.role_groups.len());
        for rg in self.role_groups.drain(..) {
            if rg.role == role {
                detached.push(rg.group);
            } else {
                kept.push(rg);
            }
        }
        self.role_groups = kept;
        detached
    }

    pub async fn prune_finished_groups(&mut self) -> RunResult<()> {
        let mut kept = Vec::with_capacity(self.role_groups.len());
        for mut rg in self.role_groups.drain(..) {
            if rg.group.routin_is_finished() {
                rg.group.wait_finished().await?;
            } else {
                kept.push(rg);
            }
        }
        self.role_groups = kept;
        Ok(())
    }

    pub async fn isolate_role(&self, role: TaskRole) -> RunResult<()> {
        for rg in &self.role_groups {
            if rg.role == role && !rg.group.routin_is_finished() {
                rg.group.cmd_alone().await?;
            }
        }
        Ok(())
    }

    pub async fn execute_role_all(&self, role: TaskRole) -> RunResult<()> {
        for rg in &self.role_groups {
            if rg.role == role && !rg.group.routin_is_finished() {
                rg.group.cmd_execute_all().await?;
            }
        }
        Ok(())
    }

    pub async fn stop_role(&mut self, role: TaskRole, stop: ShutdownCmd) -> RunResult<()> {
        let mut kept = Vec::with_capacity(self.role_groups.len());
        for mut rg in self.role_groups.drain(..) {
            if rg.role == role {
                rg.group
                    .wait_grace_down(Some(ActorCtrlCmd::Stop(stop.clone())))
                    .await?;
                info_ctrl!(
                    "role group {:?}({}) stopped and removed",
                    rg.role,
                    rg.group.name()
                );
            } else {
                kept.push(rg);
            }
        }
        self.role_groups = kept;
        Ok(())
    }

    pub async fn wait_role_groups_finished(
        &mut self,
        role: TaskRole,
        wait_timeout: Duration,
    ) -> RunResult<()> {
        let started_at = Instant::now();
        loop {
            let mut all_finished = true;
            let mut has_role = false;
            for rg in &self.role_groups {
                if rg.role != role {
                    continue;
                }
                has_role = true;
                if !rg.group.routin_is_finished() {
                    all_finished = false;
                    break;
                }
            }
            if !has_role || all_finished {
                break;
            }
            if started_at.elapsed() >= wait_timeout {
                return Err(RunReason::validation_error().to_err());
            }
            sleep(Duration::from_millis(100)).await;
        }

        let mut kept = Vec::with_capacity(self.role_groups.len());
        for mut rg in self.role_groups.drain(..) {
            if rg.role == role {
                rg.group.wait_finished().await?;
                info_ctrl!(
                    "role group {:?}({}) finished and removed",
                    rg.role,
                    rg.group.name()
                );
            } else {
                kept.push(rg);
            }
        }
        self.role_groups = kept;
        Ok(())
    }

    pub async fn all_down_wait_policy(&mut self, policy_kind: ExitPolicyKind) -> RunResult<()> {
        self.all_down_wait_policy_with_signal(policy_kind, false)
            .await
    }

    pub async fn all_down_wait_policy_with_signal(
        &mut self,
        policy_kind: ExitPolicyKind,
        initial_signal_received: bool,
    ) -> RunResult<()> {
        if self.role_groups.is_empty() {
            return Err(RunReason::validation_error().to_err());
        }

        let mut policy = build_exit_policy(policy_kind);
        let mut phase = ExitPhase::Running;
        let mut phase_started_at = Instant::now();
        let mut signals = stop_signals()?;
        let mut pending_signal = initial_signal_received;

        loop {
            let signal_received = if pending_signal {
                true
            } else {
                matches!(
                    timeout(Duration::from_millis(100), signals.next()).await,
                    Ok(Some(_))
                )
            };
            pending_signal = false;
            let action = self.next_exit_policy_action(
                policy_kind,
                phase,
                phase_started_at,
                signal_received,
            )?;

            if self
                .apply_exit_policy_action(
                    policy_kind,
                    &mut phase,
                    &mut phase_started_at,
                    action,
                    Some(&mut *policy),
                )
                .await?
            {
                return Ok(());
            }

            sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn poll_exit_policy(
        &mut self,
        policy_kind: ExitPolicyKind,
        phase: &mut ExitPhase,
        phase_started_at: &mut Instant,
        signal_received: bool,
    ) -> RunResult<bool> {
        let action =
            self.next_exit_policy_action(policy_kind, *phase, *phase_started_at, signal_received)?;
        self.apply_exit_policy_action(policy_kind, phase, phase_started_at, action, None)
            .await
    }

    pub fn next_exit_policy_action(
        &self,
        policy_kind: ExitPolicyKind,
        phase: ExitPhase,
        phase_started_at: Instant,
        signal_received: bool,
    ) -> RunResult<ExitAction> {
        if self.role_groups.is_empty() {
            return Err(RunReason::validation_error().to_err());
        }
        let mut policy = build_exit_policy(policy_kind);
        let snapshot =
            self.build_runtime_snapshot(phase, phase_started_at.elapsed(), signal_received);
        Ok(policy.decide(&snapshot))
    }

    pub async fn all_down_force_policy(&mut self, policy_kind: ExitPolicyKind) -> RunResult<()> {
        if self.role_groups.is_empty() {
            return Err(RunReason::validation_error().to_err());
        }
        let policy = build_exit_policy(policy_kind);
        info_ctrl!("exit-policy {:?}: force stopping", policy_kind);
        self.stop_role_groups(policy.stop_cmd(), None).await
    }

    fn build_runtime_snapshot(
        &self,
        phase: ExitPhase,
        phase_elapsed: Duration,
        signal_received: bool,
    ) -> RuntimeSnapshot {
        let mut counters: BTreeMap<TaskRole, (usize, usize)> = BTreeMap::new();
        for rg in &self.role_groups {
            let entry = counters.entry(rg.role).or_insert((0, 0));
            entry.0 += 1;
            if rg.group.routin_is_finished() {
                entry.1 += 1;
            }
        }
        let role_totals = counters
            .iter()
            .map(|(role, (total, _))| (*role, *total))
            .collect::<BTreeMap<_, _>>();
        let role_finished = counters
            .into_iter()
            .map(|(role, (total, finished))| (role, total > 0 && total == finished))
            .collect::<BTreeMap<_, _>>();
        RuntimeSnapshot::new(
            phase,
            phase_elapsed,
            signal_received,
            role_totals,
            role_finished,
        )
    }

    async fn quiesce_by_policy(&mut self, policy_kind: ExitPolicyKind) -> RunResult<()> {
        match policy_kind {
            // daemon 进入 quiescing 时先停止 acceptor，避免 picker 已结束但 acceptor 仍持续监听导致无法收敛
            ExitPolicyKind::Daemon => self.request_role_stop(TaskRole::Acceptor).await,
            ExitPolicyKind::Batch | ExitPolicyKind::Generator => Ok(()),
        }
    }

    pub async fn apply_exit_policy_action(
        &mut self,
        policy_kind: ExitPolicyKind,
        phase: &mut ExitPhase,
        phase_started_at: &mut Instant,
        action: ExitAction,
        policy: Option<&mut dyn super::exit_policy::ExitPolicy>,
    ) -> RunResult<bool> {
        match action {
            ExitAction::Stay => Ok(false),
            ExitAction::EnterQuiescing(trigger) => {
                info_ctrl!(
                    "exit-policy {:?}: enter quiescing (trigger={:?})",
                    policy_kind,
                    trigger
                );
                self.quiesce_by_policy(policy_kind).await?;
                *phase = ExitPhase::Quiescing;
                *phase_started_at = Instant::now();
                Ok(false)
            }
            ExitAction::EnterStopping(trigger) => {
                info_ctrl!(
                    "exit-policy {:?}: enter stopping (trigger={:?})",
                    policy_kind,
                    trigger
                );
                let stop = policy
                    .map(|p| p.stop_cmd())
                    .unwrap_or(ShutdownCmd::Immediate);
                let force_timeout = match trigger {
                    ExitTrigger::QuiescingTimeout => Some(DAEMON_FORCE_STOP_TIMEOUT),
                    ExitTrigger::Signal | ExitTrigger::RoleFinished(_) => None,
                };
                self.stop_role_groups(stop, force_timeout).await?;
                Ok(true)
            }
        }
    }

    async fn request_role_stop(&mut self, role: TaskRole) -> RunResult<()> {
        for rg in &mut self.role_groups {
            if rg.role == role && !rg.group.routin_is_finished() {
                rg.group.cmd_stop(ShutdownCmd::Immediate).await?;
            }
        }
        Ok(())
    }

    async fn stop_role_groups(
        &mut self,
        stop: ShutdownCmd,
        acceptor_force_timeout: Option<Duration>,
    ) -> RunResult<()> {
        self.role_groups.reverse();
        for rg in &mut self.role_groups {
            rg.group.cmd_alone().await?;
        }
        for rg in &mut self.role_groups {
            let role_timeout = acceptor_force_timeout.filter(|_| rg.role == TaskRole::Acceptor);
            if let Some(wait_timeout) = role_timeout {
                rg.group
                    .wait_grace_down_with_timeout(
                        Some(ActorCtrlCmd::Stop(stop.clone())),
                        wait_timeout,
                    )
                    .await?;
            } else {
                rg.group
                    .wait_grace_down(Some(ActorCtrlCmd::Stop(stop.clone())))
                    .await?;
            }
            info_ctrl!("role group {:?}({}) await end!", rg.role, rg.group.name());
        }
        info_ctrl!("all role groups await end!");
        self.role_groups.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn make_natural_group(name: &str, delay_ms: u64, finished: Arc<AtomicBool>) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        group.append(tokio::spawn(async move {
            sleep(Duration::from_millis(delay_ms)).await;
            finished.store(true, Ordering::SeqCst);
        }));
        group
    }

    fn make_stop_aware_group(
        name: &str,
        stop_seen: Arc<AtomicBool>,
        observed_before_stop: Option<Arc<AtomicBool>>,
        probe_flag: Option<Arc<AtomicBool>>,
    ) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let mut cmd_sub = group.subscribe();
        group.append(tokio::spawn(async move {
            loop {
                match cmd_sub.recv().await {
                    Ok(ActorCtrlCmd::Stop(_)) => {
                        if let (Some(observed), Some(flag)) =
                            (observed_before_stop.as_ref(), probe_flag.as_ref())
                        {
                            observed.store(flag.load(Ordering::SeqCst), Ordering::SeqCst);
                        }
                        stop_seen.store(true, Ordering::SeqCst);
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }));
        group
    }

    fn make_delayed_stop_group(
        name: &str,
        stop_seen: Arc<AtomicBool>,
        finished: Arc<AtomicBool>,
        delay_ms: u64,
    ) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let mut cmd_sub = group.subscribe();
        group.append(tokio::spawn(async move {
            loop {
                match cmd_sub.recv().await {
                    Ok(ActorCtrlCmd::Stop(_)) => {
                        stop_seen.store(true, Ordering::SeqCst);
                        sleep(Duration::from_millis(delay_ms)).await;
                        finished.store(true, Ordering::SeqCst);
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }));
        group
    }

    fn make_stuck_after_stop_group(name: &str, stop_seen: Arc<AtomicBool>) -> TaskGroup {
        let mut group = TaskGroup::new(name, ShutdownCmd::Immediate);
        let mut cmd_sub = group.subscribe();
        group.append(tokio::spawn(async move {
            loop {
                match cmd_sub.recv().await {
                    Ok(ActorCtrlCmd::Stop(_)) => {
                        stop_seen.store(true, Ordering::SeqCst);
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => return,
                }
            }
            loop {
                sleep(Duration::from_millis(20)).await;
            }
        }));
        group
    }

    #[tokio::test]
    async fn batch_policy_waits_parser_drain_before_stopping_downstream() {
        let picker_done = Arc::new(AtomicBool::new(false));
        let parser_done = Arc::new(AtomicBool::new(false));
        let sink_stop_seen = Arc::new(AtomicBool::new(false));
        let parser_done_when_sink_stop = Arc::new(AtomicBool::new(false));

        let mut tm = TaskManager::default();
        tm.append_group_with_role(
            TaskRole::Picker,
            make_natural_group("picker", 20, picker_done.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Parser,
            make_natural_group("parser", 120, parser_done.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Sink,
            make_stop_aware_group(
                "sink",
                sink_stop_seen.clone(),
                Some(parser_done_when_sink_stop.clone()),
                Some(parser_done.clone()),
            ),
        );

        tm.all_down_wait_policy(ExitPolicyKind::Batch)
            .await
            .expect("batch policy should exit cleanly");

        assert!(picker_done.load(Ordering::SeqCst));
        assert!(parser_done.load(Ordering::SeqCst));
        assert!(sink_stop_seen.load(Ordering::SeqCst));
        assert!(
            parser_done_when_sink_stop.load(Ordering::SeqCst),
            "sink stop should happen after parser reports finished in batch mode"
        );
    }

    #[tokio::test]
    async fn daemon_policy_stops_parser_when_picker_finishes() {
        let picker_done = Arc::new(AtomicBool::new(false));
        let parser_stop_seen = Arc::new(AtomicBool::new(false));

        let mut tm = TaskManager::default();
        tm.append_group_with_role(
            TaskRole::Picker,
            make_natural_group("picker", 20, picker_done.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Parser,
            make_stop_aware_group("parser", parser_stop_seen.clone(), None, None),
        );
        tm.append_group_with_role(
            TaskRole::Sink,
            make_stop_aware_group("sink", Arc::new(AtomicBool::new(false)), None, None),
        );

        tm.all_down_wait_policy(ExitPolicyKind::Daemon)
            .await
            .expect("daemon policy should exit cleanly when picker finishes");

        assert!(picker_done.load(Ordering::SeqCst));
        assert!(parser_stop_seen.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn daemon_policy_quiesces_acceptor_before_global_stop() {
        let picker_done = Arc::new(AtomicBool::new(false));
        let acceptor_stop_seen = Arc::new(AtomicBool::new(false));
        let acceptor_finished = Arc::new(AtomicBool::new(false));
        let parser_stop_seen = Arc::new(AtomicBool::new(false));
        let parser_stop_after_acceptor = Arc::new(AtomicBool::new(false));

        let mut tm = TaskManager::default();
        tm.append_group_with_role(
            TaskRole::Picker,
            make_natural_group("picker", 20, picker_done.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Acceptor,
            make_delayed_stop_group(
                "acceptor",
                acceptor_stop_seen.clone(),
                acceptor_finished.clone(),
                60,
            ),
        );
        tm.append_group_with_role(
            TaskRole::Parser,
            make_stop_aware_group(
                "parser",
                parser_stop_seen.clone(),
                Some(parser_stop_after_acceptor.clone()),
                Some(acceptor_finished.clone()),
            ),
        );

        tm.all_down_wait_policy(ExitPolicyKind::Daemon)
            .await
            .expect("daemon policy should quiesce acceptor and then stop");

        assert!(picker_done.load(Ordering::SeqCst));
        assert!(acceptor_stop_seen.load(Ordering::SeqCst));
        assert!(acceptor_finished.load(Ordering::SeqCst));
        assert!(parser_stop_seen.load(Ordering::SeqCst));
        assert!(
            parser_stop_after_acceptor.load(Ordering::SeqCst),
            "parser should receive global stop only after acceptor quiesce finished"
        );
    }

    #[tokio::test]
    async fn daemon_policy_timeout_forces_abort_stuck_acceptor() {
        let picker_done = Arc::new(AtomicBool::new(false));
        let acceptor_stop_seen = Arc::new(AtomicBool::new(false));
        let parser_stop_seen = Arc::new(AtomicBool::new(false));
        let started = Instant::now();

        let mut tm = TaskManager::default();
        tm.append_group_with_role(
            TaskRole::Picker,
            make_natural_group("picker", 20, picker_done.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Acceptor,
            make_stuck_after_stop_group("acceptor", acceptor_stop_seen.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Parser,
            make_stop_aware_group("parser", parser_stop_seen.clone(), None, None),
        );

        tm.all_down_wait_policy(ExitPolicyKind::Daemon)
            .await
            .expect("daemon policy should force stop when acceptor never exits");

        assert!(picker_done.load(Ordering::SeqCst));
        assert!(acceptor_stop_seen.load(Ordering::SeqCst));
        assert!(parser_stop_seen.load(Ordering::SeqCst));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "daemon timeout fallback should converge quickly in tests"
        );
    }

    #[tokio::test]
    async fn daemon_policy_timeout_does_not_abort_non_acceptor_groups() {
        let picker_done = Arc::new(AtomicBool::new(false));
        let acceptor_stop_seen = Arc::new(AtomicBool::new(false));
        let parser_stop_seen = Arc::new(AtomicBool::new(false));
        let parser_finished = Arc::new(AtomicBool::new(false));
        let started = Instant::now();

        let mut tm = TaskManager::default();
        tm.append_group_with_role(
            TaskRole::Picker,
            make_natural_group("picker", 20, picker_done.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Acceptor,
            make_stuck_after_stop_group("acceptor", acceptor_stop_seen.clone()),
        );
        tm.append_group_with_role(
            TaskRole::Parser,
            make_delayed_stop_group(
                "parser",
                parser_stop_seen.clone(),
                parser_finished.clone(),
                700,
            ),
        );

        tm.all_down_wait_policy(ExitPolicyKind::Daemon)
            .await
            .expect("daemon policy should keep non-acceptor graceful shutdown");

        assert!(picker_done.load(Ordering::SeqCst));
        assert!(acceptor_stop_seen.load(Ordering::SeqCst));
        assert!(parser_stop_seen.load(Ordering::SeqCst));
        assert!(parser_finished.load(Ordering::SeqCst));
        assert!(
            started.elapsed() >= Duration::from_millis(700),
            "non-acceptor groups should not be force-aborted by acceptor timeout"
        );
    }

    #[tokio::test]
    async fn initial_signal_can_drive_policy_without_waiting_second_signal() {
        let picker_stop_seen = Arc::new(AtomicBool::new(false));
        let parser_stop_seen = Arc::new(AtomicBool::new(false));

        let mut tm = TaskManager::default();
        tm.append_group_with_role(
            TaskRole::Picker,
            make_stop_aware_group("picker", picker_stop_seen.clone(), None, None),
        );
        tm.append_group_with_role(
            TaskRole::Parser,
            make_stop_aware_group("parser", parser_stop_seen.clone(), None, None),
        );

        timeout(
            Duration::from_secs(3),
            tm.all_down_wait_policy_with_signal(ExitPolicyKind::Daemon, true),
        )
        .await
        .expect("initial signal path should not block waiting another signal")
        .expect("initial signal should drive policy transition");

        assert!(picker_stop_seen.load(Ordering::SeqCst));
        assert!(parser_stop_seen.load(Ordering::SeqCst));
    }
}
