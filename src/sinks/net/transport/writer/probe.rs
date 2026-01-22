use super::super::config::*;
use super::NetWriter;

impl NetWriter {
    /// 小包路径探测触发：字节步长 + 写计数触发点（先检查紧急阈值，其次时间门控）。
    ///
    /// 探测策略：
    /// 1. 优先按字节步长触发（基于 sndbuf 容量）
    /// 2. 退化到写计数触发点（每 NET_SENDQ_PROBE_STRIDE 次写操作）
    /// 3. 在写计数触发点时，先检查紧急阈值，再检查时间门控
    pub(super) async fn handle_small_probe(&mut self) {
        // 写计数触发点：每 NET_SENDQ_PROBE_STRIDE 次写触发一次到期检查
        let probe_tick_due = is_probe_tick_due(self.sent_cnt);

        // 路径 A：按字节步长触发（优先）；若 sndbuf 不可得，则退化为仅在写计数触发点触发
        let mut need_probe = if let Some(sndbuf_cap) = self.get_cached_sndbuf() {
            self.bytes_since_probe >= small_probe_stride(sndbuf_cap)
        } else {
            probe_tick_due
        };

        // 路径 B：写计数触发点（紧急阈值优先，其次时间门控）
        if !need_probe && probe_tick_due {
            if let (Some(p), Some(sndbuf_cap)) = self.peek_kernel_queues()
                && sndbuf_cap > 0
            {
                let pct = (p.saturating_mul(100) / sndbuf_cap) as u8;
                let avg = self.estimate_avg_len();
                if pct >= emergency_pct_for(avg) {
                    need_probe = true;
                }
            }
            if !need_probe && self.time_gate_due(NET_BACKOFF_SMALL_PROBE_MS) {
                need_probe = true;
            }
        }

        if need_probe {
            let avg = self.recalc_avg_now();
            if let Some(cap) = self.get_cached_sndbuf() {
                self.maybe_toggle_nodelay(cap, avg);
            }
            self.maybe_backoff().await;
            self.last_probe_at = Some(std::time::Instant::now());
            self.bytes_since_probe = 0;
        }
    }

    /// 中/大包路径：max(cap/16, 16*avg, 16KiB) + 最小 1ms 时间门限，触发一次观测。
    pub(super) async fn handle_large_probe(&mut self) {
        let mut need_probe = false;
        if let Some(sndbuf_cap) = self.get_cached_sndbuf() {
            let avg = if self.avg_write_len > 0.0 {
                self.avg_write_len as usize
            } else {
                self.estimate_avg_len()
            };
            let stride = large_probe_stride(sndbuf_cap, avg);
            if self.bytes_since_probe >= stride && self.time_gate_due(NET_BACKOFF_LARGE_PROBE_MS) {
                need_probe = true;
            }
            if self.bytes_since_probe >= stride {
                self.bytes_since_probe = 0;
            }
        } else if is_probe_tick_due(self.sent_cnt) {
            need_probe = true;
        }
        if need_probe {
            let avg = self.recalc_avg_now();
            if let Some(cap) = self.get_cached_sndbuf() {
                self.maybe_toggle_nodelay(cap, avg);
            }
            self.maybe_backoff().await;
        }
    }

    // ------- avg/time helpers -------
    pub(super) fn time_gate_due(&self, min_ms: u64) -> bool {
        let now = std::time::Instant::now();
        let min_dur = std::time::Duration::from_millis(min_ms);
        self.last_probe_at
            .map(|t| now.saturating_duration_since(t) >= min_dur)
            .unwrap_or(true)
    }
    pub(super) fn current_avg_len(&self) -> usize {
        self.avg_write_len.max(1.0) as usize
    }
    pub(super) fn recalc_avg_now(&mut self) -> usize {
        let avg = if self.avg_writes_acc > 0 {
            let denom = (self.avg_writes_acc as usize).max(1);
            (self.avg_bytes_acc / denom).max(1)
        } else {
            self.current_avg_len()
        };
        self.avg_write_len = avg as f64;
        self.avg_bytes_acc = 0;
        self.avg_writes_acc = 0;
        avg
    }
    pub(super) fn estimate_avg_len(&self) -> usize {
        if self.avg_writes_acc > 0 {
            (self.avg_bytes_acc / (self.avg_writes_acc as usize).max(1)).max(1)
        } else {
            self.current_avg_len()
        }
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;
    use std::time::Instant;

    /// 创建一个用于测试的 NetWriter 实例
    fn create_test_writer() -> NetWriter {
        NetWriter::test_stub()
    }

    #[tokio::test]
    async fn test_time_gate_due_first_call() {
        let writer = create_test_writer();

        // 第一次调用应该返回 true（没有上次探测时间）
        assert!(writer.time_gate_due(100));
    }

    #[tokio::test]
    async fn test_time_gate_due_within_interval() {
        let mut writer = create_test_writer();

        // 设置上次探测时间为现在
        writer.last_probe_at = Some(Instant::now());

        // 在时间间隔内调用应该返回 false
        assert!(!writer.time_gate_due(100));
    }

    #[tokio::test]
    async fn test_time_gate_due_after_interval() {
        let mut writer = create_test_writer();

        // 设置上次探测时间为过去
        let past_time = Instant::now() - std::time::Duration::from_millis(150);
        writer.last_probe_at = Some(past_time);

        // 超过时间间隔后应该返回 true
        assert!(writer.time_gate_due(100));
    }

    /// 创建一个用于测试的 NetWriter 实例
    fn create_test_writer_with_config() -> NetWriter {
        NetWriter::test_stub()
    }

    #[test]
    fn test_current_avg_len_minimum_value() {
        let writer = create_test_writer_with_config();

        // 平均长度应该至少为 1
        assert!(writer.current_avg_len() >= 1);
    }

    #[test]
    fn test_current_avg_len_with_positive_average() {
        let mut writer = create_test_writer_with_config();
        writer.avg_write_len = 1500.5;

        assert_eq!(writer.current_avg_len(), 1500);
    }

    #[test]
    fn test_recalc_avg_now_with_accumulators() {
        let mut writer = create_test_writer_with_config();

        // 设置累积数据
        writer.avg_bytes_acc = 3000;
        writer.avg_writes_acc = 2;

        let average = writer.recalc_avg_now();

        assert_eq!(average, 1500);
        assert_eq!(writer.avg_write_len, 1500.0);
        assert_eq!(writer.avg_bytes_acc, 0);
        assert_eq!(writer.avg_writes_acc, 0);
    }

    #[test]
    fn test_recalc_avg_now_without_accumulators() {
        let mut writer = create_test_writer_with_config();
        writer.avg_write_len = 800.0;

        let average = writer.recalc_avg_now();

        assert_eq!(average, 800);
    }

    #[test]
    fn test_estimate_avg_len_with_accumulators() {
        let mut writer = create_test_writer_with_config();

        writer.avg_bytes_acc = 4500;
        writer.avg_writes_acc = 3;

        let estimated = writer.estimate_avg_len();

        assert_eq!(estimated, 1500);
        // 不会重置累积计数器
        assert_eq!(writer.avg_bytes_acc, 4500);
        assert_eq!(writer.avg_writes_acc, 3);
    }

    #[test]
    fn test_estimate_avg_len_without_accumulators() {
        let mut writer = create_test_writer_with_config();
        writer.avg_write_len = 1200.0;

        let estimated = writer.estimate_avg_len();

        assert_eq!(estimated, 1200);
    }

    // 集成测试：测试完整的小包探测流程
    #[tokio::test]
    async fn test_handle_small_probe_integration() {
        let mut writer = create_test_writer();

        // 设置测试条件
        writer.test_override_sndbuf(Some(1024 * 1024));
        writer.bytes_since_probe = 65 * 1024; // 触发字节步长
        writer.test_set_backpressure_enabled(true);

        // 执行探测
        writer.handle_small_probe().await;

        // 验证状态更新
        assert!(writer.last_probe_at.is_some());
        assert_eq!(writer.bytes_since_probe, 0);
    }

    // 集成测试：测试完整的大包探测流程
    #[tokio::test]
    async fn test_handle_large_probe_integration() {
        let mut writer = create_test_writer();

        // 设置测试条件
        writer.test_override_sndbuf(Some(1024 * 1024));
        writer.avg_write_len = 1500.0;
        writer.bytes_since_probe = 200 * 1024;
        writer.last_probe_at = Some(Instant::now() - std::time::Duration::from_millis(10));
        writer.test_set_backpressure_enabled(true);

        // 执行探测
        writer.handle_large_probe().await;

        // 验证状态更新
        assert_eq!(writer.bytes_since_probe, 0); // 应该被重置
    }

    // 边界测试：测试零值处理
    #[test]
    fn test_zero_value_handling() {
        let mut writer = create_test_writer_with_config();

        // 测试零平均值
        writer.avg_write_len = 0.0;
        writer.avg_bytes_acc = 0;
        writer.avg_writes_acc = 0;

        assert_eq!(writer.current_avg_len(), 1);
        assert_eq!(writer.estimate_avg_len(), 1);

        // 测试零值处理
    }

    // 性能测试：验证时间复杂度
    #[test]
    fn test_performance_characteristics() {
        let writer = create_test_writer_with_config();

        let start = Instant::now();

        // 所有操作都应该是 O(1) 时间复杂度
        for _ in 0..10000 {
            let _ = writer.time_gate_due(100);
            let _ = writer.current_avg_len();
            let _ = writer.estimate_avg_len();
        }

        let duration = start.elapsed();
        assert!(duration.as_millis() < 100); // 应该在 100ms 内完成
    }
}
