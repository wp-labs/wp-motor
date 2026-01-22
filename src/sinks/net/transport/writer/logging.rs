use super::super::config::*;
use super::NetWriter;

impl NetWriter {
    /// 记录 TCP 发送失败时的关键信息：错误、backoff 策略、队列水位、包类与 NODELAY 状态。
    pub(super) fn log_tcp_send_error(&mut self, e: &std::io::Error, payload_len: usize) {
        let (pending, sndbuf) = self.peek_kernel_queues();
        let (p_s, c_s, pct_s) = match (pending, sndbuf) {
            (Some(p), Some(c)) if c > 0 => {
                let pct = (p.saturating_mul(100) / c) as u8;
                (p.to_string(), c.to_string(), pct.to_string())
            }
            (p, c) => (
                p.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                c.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                "-".into(),
            ),
        };
        let avg_est = self.estimate_avg_len();
        let avg_cur = self.current_avg_len();
        let (class, class_note) = match sndbuf {
            Some(sndbuf_cap) if sndbuf_cap > 0 => {
                if avg_cur <= (sndbuf_cap / 16) {
                    (
                        "small",
                        format!("avg<=cap/16 ({}<={})", avg_cur, sndbuf_cap / 16),
                    )
                } else if avg_cur >= (sndbuf_cap / 8) {
                    (
                        "large",
                        format!("avg>=cap/8 ({}>={})", avg_cur, sndbuf_cap / 8),
                    )
                } else {
                    ("medium", format!("cap={}, avg={}", sndbuf_cap, avg_cur))
                }
            }
            _ => {
                if avg_est <= NET_BACKOFF_SMALL_BYPASS_BYTES {
                    (
                        "small",
                        format!("avg_est<={}B", NET_BACKOFF_SMALL_BYPASS_BYTES),
                    )
                } else {
                    ("mid/large", format!("avg_est={}", avg_est))
                }
            }
        };
        let bp_on = self.backpressure.is_some();
        let adaptive = net_backoff_adaptive();
        let nodelay = self
            .nodelay_on
            .map(|v| if v { "on" } else { "off" })
            .unwrap_or("unknown");
        let last_probe_ms = self
            .last_probe_at
            .map(|t| {
                format!(
                    "{}",
                    std::time::Instant::now()
                        .saturating_duration_since(t)
                        .as_millis()
                )
            })
            .unwrap_or_else(|| "-".into());
        // backpressure 配置快照
        let (cfg_high, cfg_sleep, cfg_mode) = self
            .backpressure
            .map(|c| {
                let mode = if c.adaptive.is_some() {
                    "adaptive"
                } else {
                    "fixed"
                };
                (c.high_water_pct, c.sleep_ms, mode)
            })
            .unwrap_or((
                NET_SENDQ_BACKOFF_HIGH_PCT,
                NET_SENDQ_BACKOFF_SLEEP_MS,
                "fixed",
            ));
        // 步长快照（如可得）
        let (stride_small, stride_large) = match sndbuf {
            Some(sndbuf_cap) if sndbuf_cap > 0 => {
                let small = (64 * 1024).max(sndbuf_cap / 8);
                let base = if self.avg_write_len > 0.0 {
                    self.avg_write_len as usize
                } else {
                    avg_est
                };
                let cap_stride = sndbuf_cap / 16;
                let avg_stride = base.saturating_mul(16);
                let large = cap_stride.max(avg_stride).max(16 * 1024);
                (small, large)
            }
            _ => (0, 0),
        };
        // 端点与错误信息
        let peer = self.peer_addr.clone().unwrap_or_else(|| "-".into());
        let local = self.local_addr.clone().unwrap_or_else(|| "-".into());
        let os_code = e
            .raw_os_error()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into());
        let kind = format!("{:?}", e.kind());
        // 动态紧急阈值（基于当前 avg_est）
        let emerg_pct = emergency_pct_for(avg_est);
        log::error!(
            "tcp send error: err='{}' os_code={} kind={} peer={} local={} payload={}B bp={{on:{}, mode:{}}} cfg={{high:{}%, sleep:{}ms, mode:{}}} class={} ({}) avg_cur={}B avg_est={}B water={{pending:{}, sndbuf:{}, pct:{}}} nodelay={} probe={{bytes_since:{}, last_ms_ago:{}, stride_small:{}, stride_large:{}}} limits={{small_bypass:{}B, emerg_pct:{}%, emerg_sleep:{}ms}}",
            e,
            os_code,
            kind,
            peer,
            local,
            payload_len,
            bp_on,
            if adaptive { "adaptive" } else { "fixed" },
            cfg_high,
            cfg_sleep,
            cfg_mode,
            class,
            class_note,
            avg_cur,
            avg_est,
            p_s,
            c_s,
            pct_s,
            nodelay,
            self.bytes_since_probe,
            last_probe_ms,
            stride_small,
            stride_large,
            NET_BACKOFF_SMALL_BYPASS_BYTES,
            emerg_pct,
            NET_EMERGENCY_SLEEP_MS
        );
    }
}
