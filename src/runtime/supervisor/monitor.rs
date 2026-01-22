use crate::facade::test_helpers::SinkTerminal;
use crate::sinks::ProcMeta;

use std::time::Duration;

use crate::runtime::actor::constants::ACTOR_IDLE_TICK_MS;
use crate::types::AnyResult;
use tokio::time::sleep;

use wp_stat::ReportVariant;
use wp_stat::StatReq;
use wp_stat::TimedStat;

use crate::runtime::actor::command::{CmdSubscriber, TaskController};
use crate::stat::runtime_metric::RuntimeMetrics;
use crate::stat::{MonRecv, MonSend};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use wp_log::info_ctrl;

pub struct ActorMonitor {
    mon_r: MonRecv,
    mon_s: MonSend,
    cmd_r: CmdSubscriber,
    stat_sec: usize,
    stat_print: bool,
    sink: Option<SinkTerminal>,
    //actions: Vec<Action>,
}

impl ActorMonitor {
    pub fn new(
        cmd_r: CmdSubscriber,
        sink: Option<SinkTerminal>,
        stat_print: bool,
        stat_sec: usize,
        //actions: Vec<Action>,
    ) -> Self {
        let (mon_s, mon_r) = tokio::sync::mpsc::channel::<ReportVariant>(100000);
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

    pub async fn stat_proc(&mut self, reqs: Vec<StatReq>) -> AnyResult<()> {
        info_ctrl!(
            "monitor proc start: stat_sec={}, stat_print={}",
            self.stat_sec,
            self.stat_print
        );
        let mut wparse_stat = RuntimeMetrics::default();
        wparse_stat.registry(reqs);
        let mut time_stat = TimedStat::new();
        let mut run_ctrl = TaskController::new("monitor", self.cmd_r.clone(), None);
        info_ctrl!(
            "monitor loop started (stat_sec={}, stat_print={})",
            self.stat_sec,
            self.stat_print
        );
        let mut stat_check_ticks = 0u8;
        // 进程内存观测器（轻量）：每个统计窗口打印一次当前 RSS
        let cur_pid = sysinfo::get_current_pid().ok();
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        loop {
            tokio::select! {
                res = self.mon_r.recv() => {
                    match res {
                        Some(x) => { wparse_stat.slice.merge_slice(x); run_ctrl.rec_task_suc(); }
                        None => { info_ctrl!("monitor channel closed; exit"); break; }
                    }
                }
                Ok(cmd) = run_ctrl.cmds_sub_mut().recv() => { info_ctrl!("monitor recv cmd: {}", cmd); run_ctrl.update_cmd(cmd); }
                _ = sleep(Duration::from_millis(ACTOR_IDLE_TICK_MS)) => {
                    run_ctrl.rec_task_idle();
                    if run_ctrl.is_stop() { break; }
                }
            }

            stat_check_ticks = stat_check_ticks.wrapping_add(1);
            if stat_check_ticks < 10 {
                continue;
            }
            stat_check_ticks = 0;

            if time_stat.over_reset_timed(self.stat_sec) {
                // 打印一次进程内存（RSS）快照，辅助定位“背压导致的堆积”
                if let Some(pid) = cur_pid {
                    // 刷新进程快照，然后读取当前进程 RSS
                    let _ = sys.refresh_processes(ProcessesToUpdate::All, true);
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
                    let mut tdc_vec = wparse_stat.slice.conv_to_tdc();
                    while let Some(tdc) = tdc_vec.pop() {
                        if let Some(sink) = &mut self.sink
                            && let Err(e) = sink.send_record(0, ProcMeta::Null, tdc.into())
                        {
                            error_data!("sink error:{}", e);
                        }
                    }
                }
                wparse_stat.sum_up();
            }
        }
        // 退出前进行一次快速“尾部排空”：尽可能合并缓冲区中剩余的统计片段，避免出现“最后一单元未完成”的不完整统计。
        while let Ok(x) = self.mon_r.try_recv() {
            wparse_stat.slice.merge_slice(x);
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
