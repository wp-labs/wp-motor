use super::actor::JMActPicker;
use crate::runtime::actor::command::{CmdSubscriber, TaskController, spawn_ctrl_event_bridge};
use crate::runtime::actor::constants::ACTOR_IDLE_TICK_MS;
use crate::runtime::collector::realtime::constants::{
    PICKER_CTRL_EVENT_BUFFER, PICKER_DEFAULT_ROUND_BATCH, PICKER_EVENT_CNT_OF_BATCH,
    PICKER_FETCH_TIMEOUT_MS,
};
use crate::runtime::collector::realtime::picker::round::{RoundStat, SrcStatus};
// stop_routine_run/err4_dispatch_data 仅在 dispatch.rs 中使用
use crate::runtime::parser::workflow::ParseWorkerSender;
use crate::runtime::prelude::*;
use crate::stat::metric_collect::MetricCollectors;
use crate::stat::{MonSend, STAT_INTERVAL_MS};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use wp_connector_api::DataSource;

/// 独立的 Source worker：负责源生命周期与数据调度，内部复用 ActPicker 的 pending/分发逻辑。
pub struct SourceWorker {
    picker: JMActPicker,
    // 速率限制（事件/秒）换算为“单元大小”的上限；None 表示不限速
    speed_limit: Option<usize>,
    // 任务级最大处理事件数（达到后退出）；None 表示不限制
    max_count: Option<usize>,
    mon_s: MonSend,
}

impl SourceWorker {
    pub fn new(
        speed_limit: usize,
        max_count: Option<usize>,
        mon_s: MonSend,
        parse_senders: Vec<ParseWorkerSender>,
    ) -> Self {
        // 0 表示不限速；其余情况下由 TaskController 进行节流
        let limit = if speed_limit == 0 {
            None
        } else {
            Some(speed_limit)
        };
        let picker = JMActPicker::new(parse_senders);
        Self {
            picker,
            speed_limit: limit,
            max_count,
            mon_s,
        }
    }

    // subscribe_parse 接口移除：请在 new(...) 传入 parse_senders

    /// Idle tick in milliseconds. If speed_limit == 0 (unlimited), default to 0ms (no idle tick).
    /// Otherwise, default to 1ms. Can be overridden by env `WPARSE_PICKER_IDLE_MS`.
    #[allow(dead_code)]
    fn idle_tick_ms(&self) -> u64 {
        match std::env::var("WPARSE_PICKER_IDLE_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            Some(ms) => ms, // allow 0 explicitly
            None => {
                if self.speed_limit.is_none() {
                    0
                } else {
                    1
                }
            }
        }
    }

    pub async fn run(
        mut self,
        mut source: Box<dyn DataSource>,
        cmd_recv: CmdSubscriber,
        max_line: Option<usize>,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<()> {
        trace_ctrl!("read data begin");

        // 启动源的生命周期管理
        let source_id = source.identifier();
        info_ctrl!("Starting data source '{}'", source_id);

        // Bridge internal ActorCtrlCmd -> ControlEvent for data source lifecycle control
        let ctrl_rx = spawn_ctrl_event_bridge(cmd_recv.clone(), PICKER_CTRL_EVENT_BUFFER);
        if let Err(e) = source.start(ctrl_rx).await {
            error_data!("Failed to start data source '{}': {}", source_id, e);
            return Err(e.conv());
        }
        info_ctrl!(
            "Data source '{}' started (speed_limit={:?}, max_count={:?})",
            source_id,
            self.speed_limit,
            self.max_count
        );

        let result = self
            .run_dispatch_loop(source.as_mut(), cmd_recv, max_line, stat_reqs)
            .await;

        info_ctrl!("Closing data source '{}'", source_id);
        trace_ctrl!(
            "{}-picker closing with pending={}",
            source_id,
            self.picker.pending_count()
        );
        if let Err(e) = source.close().await {
            error_data!("Failed to close data source '{}': {}", source_id, e);
        }

        if let Err(ref e) = result {
            warn_ctrl!(
                "{}-picker dispatch loop exited with error: {}",
                source_id,
                e
            );
        } else {
            info_ctrl!(
                "{}-picker dispatch loop exited (max_line={:?}, total_pending={})",
                source_id,
                max_line,
                self.picker.pending_count()
            );
        }

        result
    }

    fn throttle_unit_size(&self, round_batch: usize, event_cnt_of_batch: usize) -> usize {
        // 一次“速率单元”在默认情况下等于：每轮最大突发批数 × 每批轮次 × 每批事件数
        // 为什么：将限速触发点与内部调度的粒度对齐，避免过于频繁的 sleep 抖动。
        let default_unit = JMActPicker::burst_max()
            .saturating_mul(round_batch)
            .saturating_mul(event_cnt_of_batch)
            .max(1);
        match self.speed_limit {
            Some(limit) if limit > 0 => limit.min(default_unit),
            _ => default_unit,
        }
    }

    fn calc_sleep_duration(&self, round: &RoundStat, task_ctrl: &TaskController) -> Duration {
        // 仅在“完成一个限速单元”时才执行限速休眠；
        // 如果本轮处理量低于目标（如下游偏慢），不再因为限速而额外休眠，避免越限越慢。
        let throttle = if task_ctrl.is_unit_end() {
            task_ctrl.unit_speed_limit_left()
        } else {
            Duration::from_millis(0)
        };

        if !throttle.is_zero() {
            return throttle;
        }
        // 否则仅在确实经历了等待且没有投递进展时做温和退让
        if round.need_wait(self.picker.pending_count()) {
            Duration::from_millis(ACTOR_IDLE_TICK_MS)
        } else {
            Duration::from_millis(0)
        }
    }

    // 简化且对齐 DataSource::can_try_receive/try_receive 的新版调度
    async fn run_dispatch_loop(
        &mut self,
        source: &mut dyn DataSource,
        cmd_recv: CmdSubscriber,
        max_line: Option<usize>,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<()> {
        // 初始化统计与任务控制器
        let mut stat_ext = MetricCollectors::new(source.identifier(), stat_reqs);
        let rt_name = format!("{}-picker", source.identifier());
        let round_batch = PICKER_DEFAULT_ROUND_BATCH;
        let event_cnt_of_batch = PICKER_EVENT_CNT_OF_BATCH;
        let unit_size = self.throttle_unit_size(round_batch, event_cnt_of_batch);
        let mut task_ctrl = TaskController::from_speed_limit(
            rt_name.as_str(),
            cmd_recv,
            self.speed_limit,
            unit_size,
        );
        let stat_interval = Duration::from_millis(STAT_INTERVAL_MS as u64);
        let mut last_stat_tick = Instant::now();
        let timeout = Duration::from_millis(PICKER_FETCH_TIMEOUT_MS);
        'main: loop {
            // 每进入一轮突发循环，重置“速率单元”计数器
            task_ctrl.rec_task_unit_reset();
            let mut total_round = RoundStat::new();
            while !total_round.terminal_by_round(round_batch) {
                // 快速响应控制命令/退出条件
                if self.picker.poll_cmd_now(&mut task_ctrl)
                    || self.reached_limits(&task_ctrl, max_line)
                {
                    info_ctrl!(
                        "{}-picker reached processing limit (total_cnt={:?}, line_max={:?}, max_count={:?})",
                        source.identifier(),
                        task_ctrl.total_count(),
                        max_line,
                        self.max_count
                    );
                    break 'main;
                }
                // 单轮流程：拉取→（可选）发送→记录统计
                let one_round = self
                    .picker
                    .round_pick(source, &mut task_ctrl, &mut stat_ext, timeout)
                    .await?;
                match one_round.src_status() {
                    SrcStatus::Ready => {
                        trace_ctrl!(
                            "{}-picker round status=Ready pending_cnt={}",
                            source.identifier(),
                            self.picker.pending_count()
                        );
                    }
                    other => {
                        info_ctrl!(
                            "{}-picker round status={:?} (pending_cnt={}, send_cnt={})",
                            source.identifier(),
                            other,
                            self.picker.pending_count(),
                            one_round.send_cnt()
                        );
                    }
                }
                self.picker.finish_burst_round();
                total_round = total_round.merge(one_round);
                if total_round.is_stop() {
                    break 'main;
                }
                if task_ctrl.is_unit_end() {
                    // 完成一个“限速单元”，跳出到外层以统一计算休眠时间
                    break;
                }
            }
            if last_stat_tick.elapsed() >= stat_interval {
                // 仅周期性打印 pending 水位，以降低日志噪声
                info_mtrc!(
                    "{} pick-pending cnt: {}",
                    rt_name,
                    self.picker.pending_count()
                );
                stat_ext
                    .send_stat(&self.mon_s)
                    .await
                    .owe_sys()
                    .want("mon-stat")?;
                last_stat_tick = Instant::now();
            }
            // 外层根据“限速/等待”计算应休眠的时间，避免在数据路径处直接 sleep
            let sleep_dur = self.calc_sleep_duration(&total_round, &task_ctrl);
            if !sleep_dur.is_zero() {
                sleep(sleep_dur).await;
            }
        }
        stat_ext
            .send_stat(&self.mon_s)
            .await
            .owe_sys()
            .want("mon-stat")?;
        Ok(())
    }

    /// 达到 CLI 传入的 max_line 或内部 max_count 时退出当前任务单元
    fn reached_limits(&self, run_ctrl: &TaskController, max_line: Option<usize>) -> bool {
        if let Some(m) = max_line
            && run_ctrl.total_count() >= m
        {
            info_ctrl!(
                "task controller hit max_line={} with total_count={}",
                m,
                run_ctrl.total_count()
            );
            return true;
        }
        if let Some(m) = self.max_count
            && run_ctrl.total_count() >= m
        {
            info_ctrl!(
                "task controller hit max_count={} with total_count={}",
                m,
                run_ctrl.total_count()
            );
            return true;
        }
        false
    }
}

// ActPicker 的分发/读取细节已抽到 dispatch.rs，run.rs 仅保留 SourceWorker 主循环

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::actor::command::ActorCtrlCmd;
    use crate::runtime::actor::signal::ShutdownCmd;
    use crate::runtime::parser::workflow::ParseWorkerSender;
    use crate::sources::event_id::next_event_id;
    use async_broadcast::broadcast;
    use async_trait::async_trait;
    use tokio::sync::mpsc;
    use tokio::time::sleep as tsleep;
    use wp_connector_api::{
        SourceBatch, SourceError, SourceEvent, SourceReason, SourceResult, Tags,
    };
    use wp_parse_api::RawData;
    use wp_stat::ReportVariant;

    const TEST_MONITOR_CHANNEL_CAP: usize = 4;
    const TEST_PARSE_CHANNEL_CAP: usize = 32;
    //const TEST_SINGLE_PARSE_CHANNEL_CAP: usize = 1;
    const TEST_CMD_BUFFER_CAP: usize = 4;
    const TEST_LOOPING_SOURCE_YIELD_MS: u64 = 1;
    const TEST_STOP_DELAY_MS: u64 = 30;
    #[allow(dead_code)]
    const TEST_ONCE_DELAY_MS: u64 = 12;
    const TEST_TIMEOUT_SECS: u64 = 1;
    const TEST_EOF_TIMEOUT_MS: u64 = 500;
    const TEST_RATE_LIMIT_UNIT: usize = 100;

    fn make_event(label: &str) -> SourceEvent {
        SourceEvent::new(
            next_event_id(),
            label,
            RawData::from_string(label.to_string()),
            std::sync::Arc::new(Tags::new()),
        )
    }

    async fn setup_worker() -> (
        SourceWorker,
        mpsc::Receiver<ReportVariant>,
        mpsc::Receiver<SourceBatch>,
    ) {
        let (mon_tx, mon_rx) = mpsc::channel::<ReportVariant>(TEST_MONITOR_CHANNEL_CAP);
        let (parse_tx, parse_rx) = mpsc::channel::<SourceBatch>(TEST_PARSE_CHANNEL_CAP);
        let worker = SourceWorker::new(0, None, mon_tx, vec![ParseWorkerSender::new(parse_tx)]);
        (worker, mon_rx, parse_rx)
    }

    fn worker_with_speed_limit(limit: usize) -> SourceWorker {
        let (mon_tx, _mon_rx) = mpsc::channel::<ReportVariant>(TEST_MONITOR_CHANNEL_CAP);
        let (parse_tx, _parse_rx) = mpsc::channel::<SourceBatch>(TEST_MONITOR_CHANNEL_CAP);
        SourceWorker::new(limit, None, mon_tx, vec![ParseWorkerSender::new(parse_tx)])
    }

    #[test]
    fn throttle_unit_size_clamps_to_default_upper_bound() {
        let worker = worker_with_speed_limit(500);
        // default_unit = ActPicker::burst_max() * 4 * 5 = 320
        assert_eq!(worker.throttle_unit_size(4, 5), 320);
    }

    #[test]
    fn throttle_unit_size_defaults_when_unlimited() {
        let worker = worker_with_speed_limit(0);
        let expected = JMActPicker::burst_max() * 3 * 7;
        assert_eq!(worker.throttle_unit_size(3, 7), expected);
    }

    #[test]
    fn calc_sleep_duration_waits_after_unit_completed() {
        let worker = worker_with_speed_limit(100);
        let (_cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);
        let mut task_ctrl = TaskController::from_speed_limit(
            "calc",
            cmd_rx,
            Some(TEST_RATE_LIMIT_UNIT),
            TEST_RATE_LIMIT_UNIT,
        );
        task_ctrl.rec_task_unit_reset();

        let round = RoundStat::new();
        assert!(worker.calc_sleep_duration(&round, &task_ctrl).is_zero());

        task_ctrl.rec_task_suc_cnt(100);
        let wait = worker.calc_sleep_duration(&round, &task_ctrl);
        assert!(
            !wait.is_zero(),
            "rate limiting should trigger once a full unit is processed"
        );
    }

    struct FiniteSource {
        id: String,
        left: usize,
    }
    #[async_trait]
    impl DataSource for FiniteSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
            if self.left == 0 {
                return Err(SourceError::from(SourceReason::EOF));
            }
            self.left -= 1;
            Ok(vec![make_event(&self.id)])
        }
        fn try_receive(&mut self) -> Option<SourceBatch> {
            None
        }
        fn can_try_receive(&mut self) -> bool {
            false
        }
        fn identifier(&self) -> String {
            self.id.clone()
        }
    }

    struct LoopingSource {
        id: String,
    }
    #[async_trait]
    impl DataSource for LoopingSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
            // 轻微让出调度，避免纯自旋导致控制命令迟迟无法被轮询
            tsleep(std::time::Duration::from_millis(
                TEST_LOOPING_SOURCE_YIELD_MS,
            ))
            .await;
            Ok(vec![make_event(&self.id)])
        }
        fn try_receive(&mut self) -> Option<SourceBatch> {
            None
        }
        fn can_try_receive(&mut self) -> bool {
            false
        }
        fn identifier(&self) -> String {
            self.id.clone()
        }
    }

    #[tokio::test]
    async fn run_dispatch_loop_stops_after_max_line_limit() {
        let (mut worker, _mon_rx, parse_rx) = setup_worker().await;
        let drain = tokio::spawn(async move {
            let mut rx = parse_rx;
            while rx.recv().await.is_some() {}
        });
        let mut source = FiniteSource {
            id: "max-line".into(),
            left: 50,
        };
        let (_cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(TEST_TIMEOUT_SECS),
            worker.run_dispatch_loop(&mut source, cmd_rx, Some(5), vec![]),
        )
        .await;
        assert!(res.is_ok(), "run loop should finish within timeout");
        res.unwrap().expect("should stop after reaching max_line");
        drop(worker);
        drain.await.unwrap();
    }

    #[tokio::test]
    async fn run_dispatch_loop_honors_stop_command() {
        let (mut worker, _mon_rx, parse_rx) = setup_worker().await;
        let drain = tokio::spawn(async move {
            let mut rx = parse_rx;
            while rx.recv().await.is_some() {}
        });

        let mut source = LoopingSource { id: "stop".into() };
        let (cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);
        tokio::spawn(async move {
            tsleep(std::time::Duration::from_millis(TEST_STOP_DELAY_MS)).await;
            let _ = cmd_tx
                .broadcast(ActorCtrlCmd::Stop(ShutdownCmd::Immediate))
                .await;
        });

        let res = tokio::time::timeout(
            std::time::Duration::from_secs(TEST_TIMEOUT_SECS),
            worker.run_dispatch_loop(&mut source, cmd_rx, None, vec![]),
        )
        .await;
        assert!(res.is_ok(), "run loop should finish within timeout");
        res.unwrap().expect("should exit after stop command");
        drop(worker);
        drain.await.unwrap();
    }

    #[allow(dead_code)]
    struct OnceDelayedSource {
        id: String,
        called: bool,
    }
    #[async_trait]
    impl DataSource for OnceDelayedSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
            if self.called {
                return Err(SourceError::from(SourceReason::NotData));
            }
            self.called = true;
            tsleep(std::time::Duration::from_millis(TEST_ONCE_DELAY_MS)).await;
            Ok(vec![make_event(&self.id)])
        }
        fn try_receive(&mut self) -> Option<SourceBatch> {
            None
        }
        fn can_try_receive(&mut self) -> bool {
            false
        }
        fn identifier(&self) -> String {
            self.id.clone()
        }
    }

    struct EofImmediateSource {
        id: String,
    }
    #[async_trait]
    impl DataSource for EofImmediateSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
            Err(SourceError::from(SourceReason::EOF))
        }
        fn try_receive(&mut self) -> Option<SourceBatch> {
            None
        }
        fn can_try_receive(&mut self) -> bool {
            false
        }
        fn identifier(&self) -> String {
            self.id.clone()
        }
    }

    #[tokio::test]
    async fn run_dispatch_loop_exits_on_immediate_eof() {
        let (mut worker, _mon_rx, parse_rx) = setup_worker().await;
        // drain parser channel to avoid blocking the picker while we only inspect exit status
        let drain = tokio::spawn(async move {
            let mut rx = parse_rx;
            while rx.recv().await.is_some() {}
        });
        let mut source = EofImmediateSource { id: "eof".into() };
        let (_cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);

        let res = tokio::time::timeout(
            std::time::Duration::from_millis(TEST_EOF_TIMEOUT_MS),
            worker.run_dispatch_loop(&mut source, cmd_rx, None, vec![]),
        )
        .await;
        assert!(res.is_ok(), "run loop should exit quickly on EOF");
        res.unwrap()
            .expect("EOF should allow run loop to terminate cleanly");
        drop(worker);
        drain.await.unwrap();
    }

    #[tokio::test]
    async fn run_dispatch_loop_handles_finite_sources() {
        let (mut worker, _mon_rx, parse_rx) = setup_worker().await;
        let delivered = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let delivered_clone = delivered.clone();
        let drain = tokio::spawn(async move {
            let mut rx = parse_rx;
            while let Some(batch) = rx.recv().await {
                delivered_clone.fetch_add(batch.len(), std::sync::atomic::Ordering::Relaxed);
            }
        });
        let mut source = FiniteSource {
            id: "finite".into(),
            left: 3,
        };
        let (_cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);

        let res = tokio::time::timeout(
            std::time::Duration::from_millis(TEST_EOF_TIMEOUT_MS),
            worker.run_dispatch_loop(&mut source, cmd_rx, None, vec![]),
        )
        .await;
        assert!(res.is_ok(), "run loop should finish after finite source");
        res.unwrap()
            .expect("finite source should not cause run loop errors");
        drop(worker);
        drain.await.unwrap();
    }
}
