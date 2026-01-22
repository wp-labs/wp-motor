use super::super::config::*;
use super::NetWriter;

impl NetWriter {
    /// 若开启背压：当内核发送缓冲区占用超过阈值时短暂休眠。
    pub(super) async fn maybe_backoff(&mut self) {
        #[cfg(test)]
        {
            self.probe_count = self.probe_count.saturating_add(1);
        }
        let Some(mut cfg) = self.backpressure else {
            return;
        };
        if !matches!(self.transport, super::Transport::Tcp(_)) {
            self.backpressure = Some(cfg);
            return;
        }
        self.do_backoff(&mut cfg).await;
        self.backpressure = Some(cfg);
    }

    /// 执行一次退让决策：读取内核水位，处理 fast-path 或执行固定/自适应退让。
    pub(super) async fn do_backoff(&mut self, cfg: &mut BackpressureCfg) {
        let (pending, sndbuf) = self.peek_kernel_queues();
        let (p, sndbuf_cap) = match (pending, sndbuf) {
            (Some(p), Some(c)) if c > 0 => (p, c),
            _ => return,
        };
        let pct = (p.saturating_mul(100) / sndbuf_cap) as u8;
        let avg = self.current_avg_len();
        if self.handle_fast_paths(avg, sndbuf_cap, pct).await {
            return;
        }
        if net_backoff_adaptive() {
            self.backoff_adaptive_sleep(cfg, pct, avg, sndbuf_cap).await;
        } else {
            self.backoff_fixed_sleep(cfg, pct).await;
        }
    }

    /// 小/大包快速通道；小包在拥塞极高时进行一次微退让。
    pub(super) async fn handle_fast_paths(
        &mut self,
        avg: usize,
        sndbuf_cap: usize,
        pct: u8,
    ) -> bool {
        let small_threshold = (sndbuf_cap / 16).max(1);
        if avg <= small_threshold && avg <= NET_BACKOFF_SMALL_BYPASS_BYTES {
            self.emergency_sleep_if_needed(pct, avg, sndbuf_cap).await;
            return true;
        }
        if avg >= (sndbuf_cap / 8) {
            return true;
        }
        false
    }

    /// 小包紧急微退让：仅在明显拥塞时触发，避免对端复位/中止。
    pub(super) async fn emergency_sleep_if_needed(
        &mut self,
        pct: u8,
        avg: usize,
        sndbuf_cap: usize,
    ) {
        // 动态紧急水位：根据平均包长 + sndbuf 调整阈值，达到则执行 2ms 微退让。
        let mut emerg_pct = emergency_pct_for(avg);
        if avg > NET_BACKOFF_SMALL_BYPASS_BYTES && sndbuf_cap > 0 {
            let dyn_high = Self::compute_dynamic_high_pct(avg, sndbuf_cap);
            let dyn_floor = dyn_high.saturating_sub(5);
            emerg_pct = emerg_pct.max(dyn_floor);
        }
        if pct >= emerg_pct {
            #[cfg(test)]
            {
                self.last_slept_ms = NET_EMERGENCY_SLEEP_MS;
            }
            tokio::time::sleep(std::time::Duration::from_millis(NET_EMERGENCY_SLEEP_MS)).await;
        }
    }

    /// 自适应退让：动态水位 + 随包长缩放休眠时长。
    pub(super) async fn backoff_adaptive_sleep(
        &mut self,
        cfg: &mut BackpressureCfg,
        pct: u8,
        avg: usize,
        cap: usize,
    ) {
        let dyn_high = Self::compute_dynamic_high_pct(avg, cap);
        cfg.high_water_pct = dyn_high;
        if let Some(mut ad) = cfg.adaptive {
            let safe_target = dyn_high.saturating_sub(ad.hysteresis_pct);
            ad.target_pct = safe_target.max(ad.hysteresis_pct);
            cfg.adaptive = Some(ad);
        }
        let base = cfg.auto_sleep_ms(pct);
        let ms = Self::scale_sleep_ms(base, avg, cap);
        if ms > 0 {
            #[cfg(test)]
            {
                self.last_slept_ms = ms;
            }
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }
    }

    /// 固定退让：基于高水位固定阈值。
    pub(super) async fn backoff_fixed_sleep(&mut self, cfg: &mut BackpressureCfg, pct: u8) {
        let ms = cfg.auto_sleep_ms(pct);
        if ms > 0 {
            #[cfg(test)]
            {
                self.last_slept_ms = ms;
            }
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }
    }
}

impl NetWriter {
    /// 计算动态高水位百分比：保证至少留出 k*avg 的空余，范围 [60,95]。
    pub(super) fn compute_dynamic_high_pct(avg_write: usize, sndbuf: usize) -> u8 {
        if sndbuf == 0 {
            return NET_SENDQ_BACKOFF_HIGH_PCT;
        }
        let safety_mult = if avg_write >= 4 * 1024 {
            1.0f64
        } else if avg_write >= 1024 {
            1.5f64
        } else {
            2.0f64
        };
        let min_margin = if avg_write >= 2 * 1024 {
            4 * 1024usize
        } else {
            8 * 1024usize
        };
        let max_margin = sndbuf / 4;
        let mut margin = (avg_write as f64 * safety_mult) as usize;
        margin = margin.clamp(min_margin, max_margin.max(min_margin));
        let used_pct = (margin.saturating_mul(100) / sndbuf) as u8;
        let mut high = 100u8.saturating_sub(used_pct);
        let high_cap = if avg_write > NET_BACKOFF_SMALL_BYPASS_BYTES {
            98
        } else {
            95
        };
        high = high.clamp(60, high_cap);
        high
    }

    /// 按包长缩放睡眠时间：基于 SO_SNDBUF/32 的归一化，范围 [0.5x, 3x]。
    pub(super) fn scale_sleep_ms(base_ms: u64, avg_write: usize, sndbuf: usize) -> u64 {
        if base_ms == 0 || sndbuf == 0 {
            return base_ms;
        }
        let unit = sndbuf / 32;
        if unit == 0 {
            return base_ms;
        }
        let ratio = (avg_write as f64) / (unit as f64);
        let factor = ratio.clamp(0.5, 3.0);
        ((base_ms as f64) * factor).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn adaptive_dynamic_high_respects_high_water_pct() {
        let mut writer = NetWriter::test_stub();
        let mut cfg = BackpressureCfg::adaptive_default();
        let avg = 3 * 1024;
        let cap = 512 * 1024;
        let dyn_high = NetWriter::compute_dynamic_high_pct(avg, cap);
        writer.backoff_adaptive_sleep(&mut cfg, 35, avg, cap).await;
        assert_eq!(
            writer.test_get_last_slept_ms(),
            0,
            "should not sleep under dyn_high"
        );
        let ad = cfg.adaptive.unwrap();
        assert_eq!(ad.target_pct + ad.hysteresis_pct, dyn_high);
    }

    #[tokio::test]
    async fn adaptive_dynamic_high_triggers_sleep_when_pct_exceeds_high() {
        let mut writer = NetWriter::test_stub();
        let mut cfg = BackpressureCfg::adaptive_default();
        let avg = 3 * 1024;
        let cap = 512 * 1024;
        let dyn_high = NetWriter::compute_dynamic_high_pct(avg, cap);
        writer
            .backoff_adaptive_sleep(&mut cfg, dyn_high + 1, avg, cap)
            .await;
        assert!(
            writer.test_get_last_slept_ms() > 0,
            "should sleep past dyn_high"
        );
    }

    #[test]
    fn dynamic_high_caps_small_packets_at_95() {
        let cap = 512 * 1024;
        let high = NetWriter::compute_dynamic_high_pct(512, cap);
        assert!(high <= 95);
    }

    #[test]
    fn dynamic_high_allows_large_packets_to_reach_98() {
        let cap = 512 * 1024;
        let high = NetWriter::compute_dynamic_high_pct(4 * 1024, cap);
        assert_eq!(high, 98);
    }

    #[tokio::test]
    async fn emergency_sleep_uses_dynamic_threshold_for_large_packets() {
        let mut writer = NetWriter::test_stub();
        let avg = 3 * 1024;
        let cap = 512 * 1024;
        writer.emergency_sleep_if_needed(85, avg, cap).await; // below dyn threshold
        assert_eq!(writer.test_get_last_slept_ms(), 0);
        writer.test_reset_last_slept();
        writer.emergency_sleep_if_needed(95, avg, cap).await;
        assert_eq!(writer.test_get_last_slept_ms(), NET_EMERGENCY_SLEEP_MS);
    }

    #[tokio::test]
    async fn fast_path_skips_emergency_for_medium_packets() {
        let mut writer = NetWriter::test_stub();
        let avg = 2 * 1024;
        let cap = 512 * 1024;
        // simulate fast-path; should not sleep because avg > small bypass threshold
        let handled = writer
            .handle_fast_paths(avg, cap, NET_EMERG_PCT_2K - 1)
            .await;
        assert!(!handled, "medium packets should not exit fast path early");
        assert_eq!(writer.test_get_last_slept_ms(), 0);
    }

    #[tokio::test]
    async fn emergency_sleep_keeps_small_packet_threshold() {
        let mut writer = NetWriter::test_stub();
        let avg = 256;
        let cap = 512 * 1024;
        writer.emergency_sleep_if_needed(25, avg, cap).await;
        assert_eq!(writer.test_get_last_slept_ms(), 0);
        writer.test_reset_last_slept();
        writer.emergency_sleep_if_needed(40, avg, cap).await;
        assert_eq!(writer.test_get_last_slept_ms(), NET_EMERGENCY_SLEEP_MS);
    }
}
