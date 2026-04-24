use crate::facade::test_helpers::SinkTerminal;
use crate::sinks::ProcMeta;
use crate::sinks::SinkRecUnit;

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::core::sinks::sync_sink::RecSyncSink;
use crate::runtime::actor::constants::ACTOR_IDLE_TICK_MS;
use tokio::time::{MissedTickBehavior, interval, sleep};

use wp_stat::ReportVariant;
use wp_stat::StatReq;

use crate::runtime::actor::command::{CmdSubscriber, TaskController};
use crate::stat::metric_collect::MetricCollectors;
use crate::stat::runtime_counters;
use crate::stat::runtime_metric::RuntimeMetrics;
use crate::stat::{MonRecv, MonSend};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use wp_error::run_error::RunResult;
use wp_log::info_ctrl;
use wp_stat::StatStage;

const MONITOR_CHANNEL_CAP: usize = 4096;
const ENABLE_BACKLOG_JUDGEMENT: bool = false;

#[derive(Clone, Default)]
pub struct MonitorSinkHandle {
    sink: Arc<RwLock<Option<SinkTerminal>>>,
}

impl MonitorSinkHandle {
    pub fn new(sink: Option<SinkTerminal>) -> Self {
        Self {
            sink: Arc::new(RwLock::new(sink)),
        }
    }

    pub fn replace(&self, sink: Option<SinkTerminal>) {
        let mut guard = self
            .sink
            .write()
            .expect("monitor sink handle poisoned on replace");
        *guard = sink;
    }

    pub fn current(&self) -> Option<SinkTerminal> {
        self.sink
            .read()
            .expect("monitor sink handle poisoned on read")
            .clone()
    }
}

pub struct ActorMonitor {
    mon_r: MonRecv,
    mon_s: MonSend,
    cmd_r: CmdSubscriber,
    stat_sec: usize,
    stat_print: bool,
    sink: MonitorSinkHandle,
    //actions: Vec<Action>,
}

impl ActorMonitor {
    pub fn new(
        cmd_r: CmdSubscriber,
        sink: MonitorSinkHandle,
        stat_print: bool,
        stat_sec: usize,
        //actions: Vec<Action>,
    ) -> Self {
        let (mon_s, mon_r) = tokio::sync::mpsc::channel::<ReportVariant>(MONITOR_CHANNEL_CAP);
        Self {
            mon_r,
            mon_s,
            cmd_r,
            sink,
            stat_print,
            stat_sec,
            //actions,
        }
    }
    pub fn send_agent(&self) -> MonSend {
        self.mon_s.clone()
    }

    pub async fn stat_proc(&mut self, reqs: Vec<StatReq>) -> RunResult<()> {
        info_ctrl!(
            "monitor proc start: stat_sec={}, stat_print={}",
            self.stat_sec,
            self.stat_print
        );
        let mut wparse_stat = RuntimeMetrics::default();
        let sink_reqs: Vec<StatReq> = reqs
            .iter()
            .filter(|r| r.stage == StatStage::Sink)
            .cloned()
            .collect();
        wparse_stat.registry(reqs);
        let mut runtime_stat = MetricCollectors::new("__runtime".to_string(), sink_reqs);
        let mut run_ctrl = TaskController::new("monitor", self.cmd_r.clone(), None);
        info_ctrl!(
            "monitor loop started (stat_sec={}, stat_print={})",
            self.stat_sec,
            self.stat_print
        );
        // 进程内存观测器（轻量）：每个统计窗口打印一次当前 RSS
        let cur_pid = sysinfo::get_current_pid().ok();
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        // 用专用 interval 驱动导出，避免原 stat_check_ticks 在不同流量下引起的窗口抖动：
        // 原方案每 10 次循环迭代才检查一次时间，低流量时 10×sleep(50ms)=500ms 延迟，
        // 高流量时迭代无 sleep 则几乎立即检查——两种模式下实际导出窗口相差 ±500ms。
        let mut export_tick = interval(Duration::from_secs(self.stat_sec as u64));
        export_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        export_tick.tick().await; // 消耗立即触发的第一次 tick，保持窗口语义一致
        loop {
            tokio::select! {
                res = self.mon_r.recv() => {
                    match res {
                        Some(x) => { wparse_stat.slice.merge_slice(x); run_ctrl.rec_task_suc(); }
                        None => { info_ctrl!("monitor channel closed; exit"); break; }
                    }
                }
                Ok(cmd) = run_ctrl.cmds_sub_mut().recv() => {
                    info_ctrl!("monitor recv cmd: {}", cmd);
                    run_ctrl.update_cmd(cmd);
                    if run_ctrl.is_stop() {
                        break;
                    }
                }
                _ = sleep(Duration::from_millis(ACTOR_IDLE_TICK_MS)) => {
                    run_ctrl.rec_task_idle();
                    if run_ctrl.is_stop() { break; }
                }
                _ = export_tick.tick() => {
                    let snapshot = runtime_counters::take_snapshot();
                    if !snapshot.is_empty() {
                        runtime_stat.record_task_batch(
                            "__runtime/monitor_send_drop_full",
                            u64_to_usize(snapshot.monitor_send_drop_full),
                        );
                        runtime_stat.record_task_batch(
                            "__runtime/monitor_send_drop_closed",
                            u64_to_usize(snapshot.monitor_send_drop_closed),
                        );
                        runtime_stat.record_task_batch(
                            "__runtime/sink_channel_full",
                            u64_to_usize(snapshot.sink_channel_full),
                        );
                        runtime_stat.record_task_batch(
                            "__runtime/sink_channel_closed",
                            u64_to_usize(snapshot.sink_channel_closed),
                        );
                        merge_collectors_to_slice(&mut runtime_stat, &mut wparse_stat.slice);
                    }
                    if ENABLE_BACKLOG_JUDGEMENT {
                        emit_backlog_judgement(&wparse_stat.slice, snapshot, self.stat_print);
                    }
                    // 打印一次进程内存（RSS）快照，辅助定位"背压导致的堆积"
                    if let Some(pid) = cur_pid {
                        // 只刷新当前进程，避免 ProcessesToUpdate::All 全量扫描引入不固定的 syscall 开销
                        let _ = sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                        if let Some(p) = sys.process(pid) {
                            let rss_mb = (p.memory() as f64) / (1024.0 * 1024.0);
                            info_mtrc!("mem: rss={:.1} MiB", rss_mb);
                        }
                    }
                    if self.stat_print {
                        println!("interval stat:");
                        wparse_stat.slice.show_table();
                        println!("sum stat:");
                        wparse_stat.total.show_table();
                    }
                    if run_ctrl.not_alone() {
                        let tdc_vec = wparse_stat.slice.conv_to_tdc();
                        if let Some(sink) = self.sink.current() {
                            let units: Vec<SinkRecUnit> = tdc_vec
                                .into_iter()
                                .map(|tdc| SinkRecUnit::new(0, ProcMeta::Null, tdc.into()))
                                .collect();
                            if let Err(e) = sink.send_to_sink_batch(units) {
                                error_data!("sink error:{}", e);
                            }
                        }
                    }
                    wparse_stat.sum_up();
                }
            }
        }
        // 退出前进行一次快速"尾部排空"：尽可能合并缓冲区中剩余的统计片段，避免出现"最后一单元未完成"的不完整统计。
        while let Ok(x) = self.mon_r.try_recv() {
            wparse_stat.slice.merge_slice(x);
        }
        let snapshot = runtime_counters::take_snapshot();
        if !snapshot.is_empty() {
            runtime_stat.record_task_batch(
                "__runtime/monitor_send_drop_full",
                u64_to_usize(snapshot.monitor_send_drop_full),
            );
            runtime_stat.record_task_batch(
                "__runtime/monitor_send_drop_closed",
                u64_to_usize(snapshot.monitor_send_drop_closed),
            );
            runtime_stat.record_task_batch(
                "__runtime/sink_channel_full",
                u64_to_usize(snapshot.sink_channel_full),
            );
            runtime_stat.record_task_batch(
                "__runtime/sink_channel_closed",
                u64_to_usize(snapshot.sink_channel_closed),
            );
            merge_collectors_to_slice(&mut runtime_stat, &mut wparse_stat.slice);
        }
        if ENABLE_BACKLOG_JUDGEMENT {
            emit_backlog_judgement(&wparse_stat.slice, snapshot, self.stat_print);
        }
        // 将尾部切片并入总计后再打印
        wparse_stat.sum_up();
        if self.stat_print {
            println!("\n\n============================ total stat ==============================");
            wparse_stat.total.show_table();
            /*
            let tdc_vec = wparse_stat.total.conv_to_tdc();
            for mut tdc in tdc_vec {
                self.action_proc(UseCase::ParsEndSum, &mut tdc);
            }
             */
            sleep(std::time::Duration::from_millis(100)).await;
        }
        info_ctrl!("monitor proc end (total_events={})", run_ctrl.total_count());
        Ok(())
    }
}

fn u64_to_usize(v: u64) -> usize {
    v.min(usize::MAX as u64) as usize
}

fn merge_collectors_to_slice(
    collectors: &mut MetricCollectors,
    slice: &mut crate::stat::metric_set::MetricSet,
) {
    for collector in collectors.items.iter_mut() {
        slice.merge_slice(ReportVariant::Stat(collector.collect_stat()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::actor::TaskGroup;
    use crate::runtime::actor::signal::ShutdownCmd;
    use tokio::sync::mpsc;
    use tokio::time::Duration;
    use wp_stat::{StatRecorder, StatReq, StatTarget};

    #[tokio::test]
    async fn monitor_sink_handle_can_be_replaced() {
        let (old_tx, mut old_rx) = mpsc::channel(4);
        let (new_tx, mut new_rx) = mpsc::channel(4);
        let sink = MonitorSinkHandle::new(Some(SinkTerminal::Channel(old_tx)));
        let group = TaskGroup::new("monitor-test", ShutdownCmd::Immediate);
        let mut monitor = ActorMonitor::new(group.subscribe(), sink.clone(), false, 1);

        let mon_send = monitor.send_agent();
        let task = tokio::spawn(async move {
            monitor
                .stat_proc(vec![StatReq {
                    stage: wp_stat::StatStage::Sink,
                    name: "monitor-test".to_string(),
                    target: StatTarget::All,
                    collect: Vec::new(),
                    max: 8,
                }])
                .await
                .expect("monitor stat proc");
        });

        mon_send
            .try_send(wp_stat::ReportVariant::Stat({
                let mut collector = wp_stat::StatCollector::new(
                    "monitor/test".to_string(),
                    StatReq::simple_test(StatTarget::All, Vec::new(), 4),
                );
                collector.record_task("monitor/test", ());
                collector.collect_stat()
            }))
            .expect("send first stat");

        let first = tokio::time::timeout(Duration::from_secs(2), old_rx.recv())
            .await
            .expect("old sink should receive stat")
            .expect("old sink package");
        assert!(
            !first.is_empty(),
            "old sink should receive non-empty package"
        );

        sink.replace(Some(SinkTerminal::Channel(new_tx)));

        mon_send
            .try_send(wp_stat::ReportVariant::Stat({
                let mut collector = wp_stat::StatCollector::new(
                    "monitor/test".to_string(),
                    StatReq::simple_test(StatTarget::All, Vec::new(), 4),
                );
                collector.record_task("monitor/test", ());
                collector.collect_stat()
            }))
            .expect("send second stat");

        let second = tokio::time::timeout(Duration::from_secs(2), new_rx.recv())
            .await
            .expect("new sink should receive stat")
            .expect("new sink package");
        assert!(
            !second.is_empty(),
            "new sink should receive non-empty package after replacement"
        );

        group.cmd_stop_now().await.expect("stop monitor");
        task.await.expect("monitor task join");
    }
}

#[derive(Debug)]
struct BacklogRow {
    group: String,
    recv: u64,
    out: u64,
    backlog: i64,
}

fn emit_backlog_judgement(
    set: &crate::stat::metric_set::MetricSet,
    rt: runtime_counters::RuntimeCounterSnapshot,
    verbose: bool,
) {
    let mut recv_by_group: BTreeMap<String, u64> = BTreeMap::new();
    let mut out_by_group: BTreeMap<String, u64> = BTreeMap::new();

    for report in set.units() {
        if report.get_req().stage != StatStage::Sink {
            continue;
        }
        let target = report.target_display();
        if target.starts_with("__runtime") {
            continue;
        }
        let amount: u64 = report
            .get_data()
            .iter()
            .map(|r| r.stat.success as u64)
            .sum();
        if amount == 0 {
            continue;
        }

        if target.ends_with("@recv") {
            let group = if let Some((group, _)) = target.split_once('#') {
                group
            } else {
                target.trim_end_matches("@recv")
            };
            *recv_by_group.entry(group.to_string()).or_insert(0) += amount;
            continue;
        }
        if let Some((group, _)) = target.split_once('/') {
            *out_by_group.entry(group.to_string()).or_insert(0) += amount;
        }
    }

    let mut result = Vec::new();
    for (group, recv) in &recv_by_group {
        let out = out_by_group.get(group).copied().unwrap_or(0);
        result.push(BacklogRow {
            group: group.clone(),
            recv: *recv,
            out,
            backlog: (*recv as i64) - (out as i64),
        });
    }
    if result.is_empty() {
        return;
    }
    result.sort_by(|a, b| {
        b.backlog
            .cmp(&a.backlog)
            .then_with(|| a.group.cmp(&b.group))
    });

    let has_runtime_bp = rt.monitor_send_drop_full > 0
        || rt.monitor_send_drop_closed > 0
        || rt.sink_channel_full > 0;
    let has_backlog = result.iter().any(|row| row.backlog > 0);
    if !verbose && !has_runtime_bp && !has_backlog {
        return;
    }
    info_ctrl!(
        "backlog/runtime: monitor_drop_full={} monitor_drop_closed={} sink_ch_full={} sink_ch_closed={}",
        rt.monitor_send_drop_full,
        rt.monitor_send_drop_closed,
        rt.sink_channel_full,
        rt.sink_channel_closed
    );
    for row in result {
        let status = if row.backlog <= 0 {
            "OK"
        } else if rt.sink_channel_full > 0 || rt.monitor_send_drop_full > 0 {
            "BACKLOG+BP"
        } else {
            "BACKLOG"
        };
        if status == "OK" {
            info_ctrl!(
                "backlog: group={} recv={} out={} delta={} status={}",
                row.group,
                row.recv,
                row.out,
                row.backlog,
                status
            );
        } else {
            warn_ctrl!(
                "backlog: group={} recv={} out={} delta={} status={}",
                row.group,
                row.recv,
                row.out,
                row.backlog,
                status
            );
        }
    }
}
