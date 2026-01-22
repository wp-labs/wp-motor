//! ActPicker 的分发与突发读取逻辑：从数据源拉取、处理 pending、按订阅者轮转分发。

use super::actor::JMActPicker;
use crate::runtime::actor::command::TaskController;
use crate::runtime::collector::realtime::constants::PICKER_BURST_MAX;
use crate::runtime::collector::realtime::picker::round::{RoundStat, SrcStatus};
use crate::runtime::prelude::*;
use crate::stat::metric_collect::MetricCollectors;
use std::time::Duration;
use wp_connector_api::DataSource;

impl JMActPicker {
    pub(super) fn burst_max() -> usize {
        PICKER_BURST_MAX
    }

    /// - 先尝试处理 pending；
    /// - 不足则从数据源拉取，填充 pending；
    /// - 返回 RoundStat（终止标志/本轮已等待总时长/是否有投递进展）。
    pub(super) async fn round_pick(
        &mut self,
        source: &mut dyn DataSource,
        task_ctrl: &mut TaskController,
        stat_ext: &mut MetricCollectors,
        timeout: Duration,
    ) -> RunResult<RoundStat> {
        let mut rs = RoundStat::new();
        // 以当前 pending 水位制定“是否拉取、拉取配额”的计划
        let pending_before_pull = self.pending_count();
        let pull_plan = self.pull_policy().plan_pull(pending_before_pull);
        if pull_plan.allow() {
            let status = self
                .fetch_into_pending(source, task_ctrl, pull_plan.fetch_budget(), timeout)
                .await?;
            trace_ctrl!(
                "{}-picker fetch status={:?} pending_after={}",
                source.identifier(),
                status,
                self.pending_count()
            );
            rs.up_src_status(status);
        }

        // 若源侧终止，则倾向“清空 pending”后尽快退出（full_post=true）
        let pending_total = self.pending_count();
        let full_post = matches!(rs.src_status(), SrcStatus::Terminal);
        if pending_total > 0 && !self.post_policy_mut().in_cooldown() {
            // 非 cooldown 期：按 pending 水位与 burst 决定本轮要发送多少批
            let post_plan = self.post_policy().plan_post(pending_total, full_post);
            if post_plan.allow() {
                let batch_rs = self.handle_pending_batch(
                    source.identifier().as_str(),
                    task_ctrl,
                    stat_ext,
                    post_plan.batch_size(),
                );
                trace_ctrl!(
                    "{}-picker posted {} events to parsers (pending_rem={})",
                    source.identifier(),
                    batch_rs.send_cnt(),
                    self.pending_count()
                );
                let progressed = batch_rs.send_cnt() > 0;
                // 若本轮没有任何投递进展，进入 post 退避（跨若干轮跳过 post）
                self.post_policy_mut().on_post_result(progressed);
                rs = rs.merge(batch_rs);
            }
        }

        Ok(rs)
    }

    /// 每轮 burst 完结后的收尾：仅轮转一次，保持块状分发的公平性
    pub(super) fn finish_burst_round(&mut self) {
        // 仅轮转一次，维持“块状发送 + 公平轮转”策略，避免单订阅者长期饥饿
        self.parse_roller_mut().roll();
    }

    /// 非阻塞拉取一次控制命令；返回是否应停止
    pub(super) fn poll_cmd_now(&self, run_ctrl: &mut TaskController) -> bool {
        // 非阻塞检查控制命令，降低对数据路径的干扰
        if let Ok(cmd) = run_ctrl.cmds_sub_mut().try_recv() {
            run_ctrl.update_cmd(cmd);
        }
        run_ctrl.is_stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::actor::command::ActorCtrlCmd;
    use crate::runtime::parser::workflow::ParseWorkerSender;
    use crate::sources::event_id::next_event_id;
    use async_broadcast::broadcast;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::sync::mpsc::error::TryRecvError;
    use wp_connector_api::{DataSource, SourceBatch, SourceEvent, Tags};
    use wp_parse_api::RawData;

    const TEST_CMD_BUFFER_CAP: usize = 4;
    const TEST_TASK_UNIT: usize = 16;
    const TEST_ROUND_TIMEOUT_MS: u64 = 5;
    const TEST_PARSE_CHANNEL_CAP: usize = 4;
    #[allow(dead_code)]
    const TEST_SMALL_PARSE_CHANNEL_CAP: usize = 2;

    fn make_event(tag: &str) -> SourceEvent {
        let mut tags = Tags::new();
        tags.set("tag", tag.to_string());
        SourceEvent::new(
            next_event_id(),
            tag,
            RawData::from_string(tag.to_string()),
            Arc::new(tags),
        )
    }

    fn make_metrics() -> MetricCollectors {
        MetricCollectors::new("src".to_string(), vec![])
    }

    fn make_task_ctrl() -> (TaskController, async_broadcast::Sender<ActorCtrlCmd>) {
        let (cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);
        (
            TaskController::from_speed_limit("round", cmd_rx, None, TEST_TASK_UNIT),
            cmd_tx,
        )
    }

    struct TryBatchSource {
        id: String,
        batches: Vec<SourceBatch>,
        idx: usize,
    }

    impl TryBatchSource {
        fn new(id: impl Into<String>, batches: Vec<SourceBatch>) -> Self {
            Self {
                id: id.into(),
                batches,
                idx: 0,
            }
        }
    }

    #[async_trait]
    impl DataSource for TryBatchSource {
        async fn receive(&mut self) -> wp_connector_api::SourceResult<SourceBatch> {
            panic!("blocking receive should not be called in try-mode test");
        }

        fn try_receive(&mut self) -> Option<SourceBatch> {
            if self.idx < self.batches.len() {
                let out = self.batches[self.idx].clone();
                self.idx += 1;
                Some(out)
            } else {
                None
            }
        }

        fn can_try_receive(&mut self) -> bool {
            true
        }

        fn identifier(&self) -> String {
            self.id.clone()
        }
    }
    /*

    struct NoopSource {
        id: String,
        polled: bool,
    }

    impl NoopSource {
        fn new(id: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                polled: false,
            }
        }
        fn polled(&self) -> bool {
            self.polled
        }
    }

    #[async_trait]
    impl DataSource for NoopSource {
        async fn receive(&mut self) -> wp_connector_api::SourceResult<SourceBatch> {
            self.polled = true;
            Ok(vec![])
        }

        fn try_receive(&mut self) -> Option<SourceBatch> {
            self.polled = true;
            None
        }

        fn can_try_receive(&mut self) -> bool {
            self.polled = true;
            true
        }

        fn identifier(&self) -> String {
            self.id.clone()
        }
    }
    */

    #[tokio::test]
    async fn round_pick_processes_pending_and_fetches_new_batches() {
        let (parse_tx, mut parse_rx) = mpsc::channel::<SourceBatch>(TEST_PARSE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![ParseWorkerSender::new(parse_tx)]);

        // 预先放入一个 pending 批次，确保本轮需要发送历史 backlog
        picker.extend_pending(vec![make_event("pending")]);

        // Source 提供一个新的批次（包含两条事件），检查本轮既能取新批次也能消费旧 pending
        let mut source = TryBatchSource::new(
            "src",
            vec![vec![make_event("fresh-1"), make_event("fresh-2")]],
        );

        let (mut ctrl, _cmd_tx) = make_task_ctrl();
        let mut metrics = make_metrics();

        let rs = picker
            .round_pick(
                &mut source,
                &mut ctrl,
                &mut metrics,
                Duration::from_millis(TEST_ROUND_TIMEOUT_MS),
            )
            .await
            .expect("round pick should succeed");

        assert_eq!(rs.send_cnt(), 2, "pending+新批次都应被投递");
        assert_eq!(picker.pending_count(), 0, "所有批次都应在本轮被消费");
        assert_eq!(ctrl.total_count(), 3, "共 3 条事件累计到 task controller");
        assert_eq!(source.idx, 1, "source 应只消费一个非空批次");

        let first = parse_rx.try_recv().expect("pending 批次应被发送");
        assert_eq!(first.len(), 1, "pending 批次只包含 1 条");
        let second = parse_rx.try_recv().expect("拉取的新批次应被发送");
        assert_eq!(second.len(), 2, "新批次包含 2 条事件");
        assert!(
            matches!(parse_rx.try_recv(), Err(TryRecvError::Empty)),
            "本轮不应再额外发送批次"
        );
    }

    #[tokio::test]
    async fn round_pick_drains_pending_on_terminal_status() {
        let (parse_tx, mut parse_rx) = mpsc::channel::<SourceBatch>(TEST_PARSE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![ParseWorkerSender::new(parse_tx)]);

        struct TerminalSource;
        #[async_trait]
        impl DataSource for TerminalSource {
            async fn receive(&mut self) -> wp_connector_api::SourceResult<SourceBatch> {
                Err(wp_connector_api::SourceError::from(
                    wp_connector_api::SourceReason::EOF,
                ))
            }
            fn try_receive(&mut self) -> Option<SourceBatch> {
                None
            }
            fn can_try_receive(&mut self) -> bool {
                false
            }
            fn identifier(&self) -> String {
                "terminal".into()
            }
        }

        let (mut ctrl, _cmd_tx) = make_task_ctrl();
        let mut metrics = make_metrics();
        let mut source = TerminalSource;

        let rs = picker
            .round_pick(
                &mut source,
                &mut ctrl,
                &mut metrics,
                Duration::from_millis(TEST_ROUND_TIMEOUT_MS),
            )
            .await
            .expect("round pick should succeed even on EOF");

        assert!(matches!(rs.src_status(), SrcStatus::Terminal));
        assert_eq!(rs.send_cnt(), 0, "EOF 时无数据应立即返回");
        assert!(matches!(parse_rx.try_recv(), Err(TryRecvError::Empty)));
        assert_eq!(picker.pending_count(), 0);
    }
}
