use crate::runtime::{
    actor::command::TaskController,
    collector::realtime::{JMActPicker, picker::round::SrcStatus},
    errors::err4_dispatch_data,
};
use crate::sample_log_with_hits;
use async_broadcast::RecvError;
use orion_error::ConvStructError;
use std::time::{Duration, Instant};
// stride uses crate::LOG_SAMPLE_STRIDE globally; no per-site stride import here
use tokio::time::sleep;
use wp_connector_api::{DataSource, SourceResult};
use wp_error::{
    RunResult,
    error_handling::{ErrorHandlingStrategy, sys_robust_mode},
};

impl JMActPicker {
    pub(super) async fn fetch_into_pending(
        &mut self,
        source: &mut dyn DataSource,
        task_ctrl: &mut TaskController,
        batch_max: usize,
        timeout: Duration,
    ) -> RunResult<SrcStatus> {
        if batch_max == 0 {
            return Ok(SrcStatus::Ready);
        }

        // 新版 DataSource 支持非阻塞读取：尽量使用 try_receive 降低等待成本
        let try_mode = source.can_try_receive();
        let read_result = if try_mode {
            self.read_batch_nonblocking(source, batch_max)
        } else {
            self.read_batch_blocking(source, task_ctrl, batch_max, timeout)
                .await
        };

        match read_result {
            Ok(status) => {
                sample_log_with_hits!(
                    FETCH_PENDING_LOG_HITS,
                    info_mtrc,
                    "fetch_into_pending status={:?} pending_cnt={}",
                    status,
                    self.pending_count()
                );
                Ok(status)
            }
            Err(e) => {
                // 统一根据错误分类决定后续行为：
                // - FixRetry/Tolerant/Ignore：保持 Ready，由上游/数据源决定退让；
                // - Terminate：视为源终止，促使上层尽快退出；
                // - Throw：传递致命错误。
                match err4_dispatch_data(&e, &sys_robust_mode()) {
                    ErrorHandlingStrategy::FixRetry => {
                        // 无需内部 backoff（统一由数据源或上层控制退让）
                        debug_data!("retryable source error: {}", e);
                        Ok(SrcStatus::Ready)
                    }
                    ErrorHandlingStrategy::Tolerant => {
                        debug_data!("read data error, stg : tolerant:{}", e);
                        Ok(SrcStatus::Ready)
                    }
                    ErrorHandlingStrategy::Ignore => Ok(SrcStatus::Ready),
                    ErrorHandlingStrategy::Terminate => Ok(SrcStatus::Terminal),
                    ErrorHandlingStrategy::Throw => {
                        error_data!("read data error, stg : interrupt:{}", e);
                        Err(e.conv())
                    }
                }
            }
        }
    }
    /// 非阻塞批量读取数据
    pub(crate) fn read_batch_nonblocking(
        &mut self,
        source: &mut dyn DataSource,
        max_count: usize,
    ) -> SourceResult<SrcStatus> {
        let mut status = SrcStatus::Ready;
        let mut total = 0;
        loop {
            if total >= max_count {
                break;
            }
            total += 1;
            // 非阻塞读取：若返回 None 或空批，则视为 Miss（本轮到此为止）
            match source.try_receive() {
                Some(batch) if !batch.is_empty() => {
                    self.extend_pending(batch);
                }
                Some(_) | None => {
                    status = SrcStatus::Miss;
                    break;
                }
            }
        }
        Ok(status)
    }

    /// 阻塞批量读取（配合 select）
    pub(crate) async fn read_batch_blocking(
        &mut self,
        source: &mut dyn DataSource,
        task_ctrl: &mut TaskController,
        max_count: usize,
        timeout: Duration,
    ) -> SourceResult<SrcStatus> {
        if max_count == 0 {
            return Ok(SrcStatus::Ready);
        }

        let mut total = 0;
        let deadline = Instant::now() + timeout; // 单轮拉取的最长等待时间
        let mut status = SrcStatus::Ready;
        let mut cmd_closed = false; // 一旦控制通道关闭，不再监听，避免 select 永远阻塞在已关闭分支
        loop {
            if total >= max_count {
                break;
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| Duration::from_millis(0));
            if remaining.is_zero() {
                status = SrcStatus::Miss;
                break;
            }
            tokio::select! {
                biased;
                // 优先响应控制命令（biased），提高停止/限速等指令的生效速度
                cmd = task_ctrl.cmds_sub_mut().recv(), if !cmd_closed => {
                    match cmd {
                        Ok(cmd) => {
                            task_ctrl.update_cmd(cmd);
                            status = SrcStatus::Terminal;
                            info_ctrl!(
                                "{}-picker received control cmd during blocking fetch: {:?} (pending_cnt={})",
                                source.identifier(),
                                task_ctrl.work_cmd(),
                                self.pending_count()
                            );
                            break;
                        }
                        Err(RecvError::Closed) => {
                            cmd_closed = true;
                            continue;
                        }
                        Err(RecvError::Overflowed(_)) => {
                            status = SrcStatus::Terminal;
                            break;
                        }
                    }
                }
                // 阻塞读取一批事件；成功则写入 pending 并继续，失败直接返回错误
                res = source.receive() => {
                    match res {
                        Ok(batch) => {
                            for v in &batch {
                                trace_data!(
                                    "[{}] => received data : {}",source.identifier(),v.payload
                                );
                            }
                            self.extend_pending(batch);
                            total += 1;
                        }
                        Err(e) => {
                            warn_data!(
                                "blocking receive error from source '{}': {} (pending_cnt={})",
                                source.identifier(),
                                e,
                                self.pending_count()
                            );
                            return Err(e);
                        }
                    }
                }
                // 到达本轮超时时间，认为源侧暂时无数据（Miss）
                _ = sleep(remaining) => {
                    status = SrcStatus::Miss;
                    trace_ctrl!(
                        "{}-picker blocking fetch timed out after {:?} (total_round={}, pending_cnt={})",
                        source.identifier(),
                        timeout,
                        total,
                        self.pending_count()
                    );
                    break;
                }
            }
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{actor::command::TaskController, parser::workflow::ParseWorkerSender};
    use crate::sources::event_id::next_event_id;
    use async_broadcast::broadcast;
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use wp_connector_api::{
        DataSource, SourceBatch, SourceError, SourceEvent, SourceReason, SourceResult, Tags,
    };
    use wp_parse_api::RawData;

    const TEST_CMD_BUFFER_CAP: usize = 4;
    const TEST_TASK_UNIT: usize = 16;
    const TEST_FETCH_TIMEOUT_MS: u64 = 5;
    const TEST_SLOW_SOURCE_DELAY_MS: u64 = 10;
    const TEST_BLOCKING_TIMEOUT_MS: u64 = 50;
    const TEST_SOURCE_CHANNEL_CAP: usize = 4;
    const TEST_SMALL_SOURCE_CHANNEL_CAP: usize = 2;
    const TEST_SINGLE_SOURCE_CHANNEL_CAP: usize = 1;

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
        TaskController::from_speed_limit("fetch", cmd_rx, None, TEST_TASK_UNIT)
    }

    struct SpyNoopSource {
        polled: bool,
    }

    impl SpyNoopSource {
        fn new() -> Self {
            Self { polled: false }
        }

        fn polled(&self) -> bool {
            self.polled
        }
    }

    #[async_trait]
    impl DataSource for SpyNoopSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
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
            "spy".into()
        }
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
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
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

    #[allow(dead_code)]
    struct BlockingErrorSource {
        id: String,
        reason: SourceReason,
    }

    impl BlockingErrorSource {
        #[allow(dead_code)]
        fn new(id: impl Into<String>, reason: SourceReason) -> Self {
            Self {
                id: id.into(),
                reason,
            }
        }
    }

    #[async_trait]
    impl DataSource for BlockingErrorSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
            Err(SourceError::from(self.reason.clone()))
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

    struct SlowBlockingSource {
        delay: Duration,
        sent: bool,
    }

    impl SlowBlockingSource {
        fn new(delay: Duration) -> Self {
            Self { delay, sent: false }
        }
    }

    #[async_trait]
    impl DataSource for SlowBlockingSource {
        async fn receive(&mut self) -> SourceResult<SourceBatch> {
            if self.sent {
                return Err(SourceError::from(SourceReason::EOF));
            }
            self.sent = true;
            tokio::time::sleep(self.delay).await;
            Ok(vec![make_event("slow")])
        }

        fn try_receive(&mut self) -> Option<SourceBatch> {
            None
        }

        fn can_try_receive(&mut self) -> bool {
            false
        }

        fn identifier(&self) -> String {
            "slow".into()
        }
    }

    #[tokio::test]
    async fn fetch_into_pending_skips_when_no_budget() {
        let (tx, _rx) = mpsc::channel::<SourceBatch>(TEST_SINGLE_SOURCE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![ParseWorkerSender::new(tx)]);
        let mut ctrl = make_task_ctrl();
        let mut src = SpyNoopSource::new();

        picker
            .fetch_into_pending(
                &mut src,
                &mut ctrl,
                0,
                Duration::from_millis(TEST_FETCH_TIMEOUT_MS),
            )
            .await
            .expect("zero budget fetch should not fail");

        assert_eq!(picker.pending_count(), 0);
        assert!(!src.polled(), "batch_max = 0 时不应触碰数据源");
    }

    #[tokio::test]
    async fn fetch_into_pending_extends_pending_in_try_mode() {
        let (tx, _rx) = mpsc::channel::<SourceBatch>(TEST_SOURCE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![ParseWorkerSender::new(tx)]);
        let mut ctrl = make_task_ctrl();

        let batches = vec![
            vec![make_event("one")],
            vec![make_event("two"), make_event("three")],
        ];
        let mut src = TryBatchSource::new("try", batches);

        picker
            .fetch_into_pending(
                &mut src,
                &mut ctrl,
                4,
                Duration::from_millis(TEST_FETCH_TIMEOUT_MS),
            )
            .await
            .expect("try-mode fetch should succeed");

        assert_eq!(picker.pending_count(), 2, "应将批次写入 pending");

        let first = picker.take_pending().expect("应有首个批次");
        assert_eq!(first.len(), 1);
        let second = picker.take_pending().expect("应有第二个批次");
        assert_eq!(second.len(), 2);
    }

    #[tokio::test]
    async fn fetch_into_pending_records_wait_in_blocking_mode() {
        let (tx, _rx) = mpsc::channel::<SourceBatch>(TEST_SMALL_SOURCE_CHANNEL_CAP);
        let mut picker = JMActPicker::new(vec![ParseWorkerSender::new(tx)]);
        let mut ctrl = make_task_ctrl();
        let mut src = SlowBlockingSource::new(Duration::from_millis(TEST_SLOW_SOURCE_DELAY_MS));

        let status = picker
            .fetch_into_pending(
                &mut src,
                &mut ctrl,
                1,
                Duration::from_millis(TEST_BLOCKING_TIMEOUT_MS * 5),
            )
            .await
            .expect("blocking fetch should succeed");

        assert_eq!(status, SrcStatus::Ready, "应完成一次阻塞读取");
        assert_eq!(picker.pending_count(), 1);
    }
}
