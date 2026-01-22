use crate::runtime::collector::realtime::JMActPicker;
use crate::runtime::collector::realtime::constants::{
    PICKER_COALESCE_MAX_EVENTS, PICKER_COALESCE_TRIGGER,
};
use crate::runtime::collector::realtime::picker::round::RoundStat;

use crate::runtime::actor::command::TaskController;
use crate::sample_log_with_hits;
use crate::stat::metric_collect::MetricCollectors;
// stride uses crate::LOG_SAMPLE_STRIDE globally; no per-site stride import here
use tokio::sync::mpsc::error::TrySendError;
use wp_connector_api::SourceBatch;

// 轻量背压观测：按固定步长抽样打印（宏内部维护静态计数器）
/// Picker state and constructor.
impl JMActPicker {
    /// 选取下一批要发送的 payload：
    /// - 当 pending 堆积较多时，优先将队头多个小批合并为一批（降低“批数”压力）；
    /// - 否则直接取队头；
    /// - 若队列为空，返回 None。
    #[inline]
    fn pop_next_payload_for_post(&mut self) -> Option<SourceBatch> {
        if self.pending_count() >= PICKER_COALESCE_TRIGGER
            && let Some(b) = self.coalesce_pending_front(PICKER_COALESCE_MAX_EVENTS)
        {
            return Some(b);
        }
        self.take_pending()
    }

    /// 批量处理 pending 队列中的事件：一次性取出至多 `batch_size` 条，
    /// 尝试投递（必要时轮转一次）；要么整批成功，要么整批失败并回滚。
    pub(crate) fn handle_pending_batch(
        &mut self,
        src_key: &str,
        task_ctrl: &mut TaskController,
        stat_ext: &mut MetricCollectors,
        batch_size: usize,
    ) -> RoundStat {
        let mut rs = RoundStat::new();

        let mut delivered = 0;
        while delivered < batch_size {
            // 无数据可投递则直接返回（不计为错误）
            let payload = match self.pop_next_payload_for_post() {
                Some(b) => b,
                None => return rs,
            };

            let event_cnt = payload.len();
            let mut pending_payload = Some(payload);
            let max_tries = self.parse_roller().len().max(1); // 最多轮转一圈，保持公平

            'roll: for _ in 0..max_tries {
                if let Some(parser) = self.parse_roller().cur() {
                    let Some(batch_to_send) = pending_payload.take() else {
                        break 'roll;
                    };
                    match parser.dat_s.try_send(batch_to_send) {
                        Ok(()) => {
                            // 成功：统计+轮转，下一个批次
                            stat_ext.record_task_batch(src_key, event_cnt);
                            rs.add_proc(1);
                            task_ctrl.rec_task_suc_cnt(event_cnt);
                            self.parse_roller_mut().roll();
                            delivered += 1;
                            break 'roll;
                        }
                        Err(TrySendError::Full(batch)) => {
                            // 解析通道满：回填批次并轮转后重试
                            pending_payload = Some(batch);
                            self.parse_roller_mut().roll();
                            let pend = self.pending_count();
                            sample_log_with_hits!(
                                PARSE_CH_FULL_HITS,
                                warn_mtrc,
                                "backpressure: parse channel full, pending_batches={}, last_batch_events={}",
                                pend,
                                event_cnt
                            );
                        }
                        Err(TrySendError::Closed(_batch)) => {
                            rs.to_dist_terminal();
                            // 理论上 parse 端不应关闭；若关闭，终止本轮并由上层决定退出。
                            // TODO: 如需具备容错，可在外层替换/剔除关闭的订阅者。
                            unimplemented!()
                        }
                    }
                }
            }

            // 一圈尝试仍失败：将批次放回队头，标记 Pending，交由上层在下一轮再试
            if let Some(batch) = pending_payload {
                self.set_pending_front(batch);
                rs.to_dist_pending();
                break;
            }
        }
        rs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::parser::workflow::ParseWorkerSender;
    use crate::sources::event_id::next_event_id;
    use async_broadcast::broadcast;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::sync::mpsc::error::TryRecvError;
    use wp_connector_api::{SourceBatch, SourceEvent, Tags};
    use wp_parse_api::RawData;

    const TEST_CMD_BUFFER_CAP: usize = 4;
    const TEST_PARSE_CHANNEL_CAP: usize = 4;
    const TEST_SINGLE_CHANNEL_CAP: usize = 1;
    const TEST_TASK_UNIT: usize = 16;

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

    fn make_task_ctrl() -> TaskController {
        let (_cmd_tx, cmd_rx) = broadcast(TEST_CMD_BUFFER_CAP);
        TaskController::from_speed_limit("handle", cmd_rx, None, TEST_TASK_UNIT)
    }

    fn make_metrics() -> MetricCollectors {
        MetricCollectors::new("src".to_string(), vec![])
    }

    #[test]
    fn handle_pending_batch_sends_up_to_batch_size() {
        let (parse_a, mut recv_a) = mpsc::channel::<SourceBatch>(TEST_PARSE_CHANNEL_CAP);
        let (parse_b, mut recv_b) = mpsc::channel::<SourceBatch>(TEST_PARSE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![
            ParseWorkerSender::new(parse_a),
            ParseWorkerSender::new(parse_b),
        ]);

        picker.extend_pending(vec![make_event("b1")]);
        picker.extend_pending(vec![make_event("b2")]);
        picker.extend_pending(vec![make_event("b3")]);

        let mut ctrl = make_task_ctrl();
        let mut metrics = make_metrics();
        let rs = picker.handle_pending_batch("src", &mut ctrl, &mut metrics, 2);

        assert_eq!(rs.send_cnt(), 2, "应仅处理 batch_size 个批次");
        assert_eq!(picker.pending_count(), 1, "剩余 pending 应被保留");
        assert_eq!(ctrl.total_count(), 2, "TaskController 应累加事件数");
        assert!(recv_a.try_recv().is_ok(), "第一个订阅者应接收到批次");
        assert!(recv_b.try_recv().is_ok(), "第二个订阅者应接收到批次");
        assert!(
            matches!(recv_a.try_recv(), Err(TryRecvError::Empty)),
            "不应再分发额外批次"
        );
        assert!(
            matches!(recv_b.try_recv(), Err(TryRecvError::Empty)),
            "不应再分发额外批次"
        );
    }

    #[test]
    fn handle_pending_batch_requeues_on_backpressure() {
        let (parse_tx, mut recv) = mpsc::channel::<SourceBatch>(TEST_SINGLE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![ParseWorkerSender::new(parse_tx.clone())]);

        let pending = vec![make_event("retry")];
        picker.extend_pending(pending);

        parse_tx
            .try_send(vec![make_event("occupy")])
            .expect("填充 channel 以制造 backpressure");

        let mut ctrl = make_task_ctrl();
        let mut metrics = make_metrics();
        let rs = picker.handle_pending_batch("src", &mut ctrl, &mut metrics, 1);

        assert_eq!(rs.send_cnt(), 0, "投递失败时 proc_cnt 应为 0");
        assert_eq!(picker.pending_count(), 1, "失败批次应被重新放回 pending");
        assert_eq!(ctrl.total_count(), 0, "失败批次不应计入成功计数");
        let restored = picker.take_pending().expect("pending 应保留原批次");
        assert_eq!(restored.len(), 1);

        // 清理占位消息，避免 channel 继续保持满状态
        assert!(recv.try_recv().is_ok());
    }
}
