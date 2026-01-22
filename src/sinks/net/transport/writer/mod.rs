use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, UdpSocket};
use wp_connector_api::{SinkError, SinkReason, SinkResult};

use super::config::*; // reuse constants/policy/adaptive toggles

// further split for readability: platform ops, probe, backoff, logging, nodelay
mod backoff;
mod logging;
mod nodelay;
mod os;
mod probe;

/// 统一的网络写入器（UDP/TCP）
pub enum Transport {
    Udp(UdpSocket),
    Tcp(TcpStream),
    #[cfg(test)]
    Null,
}

pub struct NetWriter {
    pub transport: Transport,
    pub sent_cnt: u64,
    /// 可选：启用发送队列感知的退让。
    pub backpressure: Option<BackpressureCfg>,
    /// 最近一段时间单次写入大小（指数滑动平均，字节）。仅用于探测节流，不改变水位/睡眠策略。
    pub(crate) avg_write_len: f64,
    /// 自上次探测以来累计写入的字节数（用于按字节节流探测频率）。
    pub(crate) bytes_since_probe: usize,
    #[cfg(test)]
    pub(crate) sndbuf_override: Option<usize>,
    #[cfg(test)]
    pub(crate) probe_count: u64,
    #[cfg(test)]
    pub(crate) pending_override: Option<usize>,
    #[cfg(test)]
    pub(crate) last_slept_ms: u64,
    // NODELAY 动态切换状态（仅 TCP 有效）
    pub(crate) nodelay_on: Option<bool>,
    pub(crate) nodelay_last_change: Option<std::time::Instant>,
    // 降频计算平均写入长度：只在采样点计算
    pub(crate) avg_bytes_acc: usize,
    pub(crate) avg_writes_acc: u64,
    // 缓存 SO_SNDBUF，避免每次探测都进行 getsockopt 调用
    pub(crate) cached_sndbuf: Option<usize>,
    // 限制探测的最小时间间隔，避免在极端高 EPS 时过于频繁（单位：Instant）
    pub(crate) last_probe_at: Option<std::time::Instant>,
    // 缓存端点，便于错误日志稳定输出
    pub(crate) peer_addr: Option<String>,
    pub(crate) local_addr: Option<String>,
}

impl NetWriter {
    /// 建立 UDP 连接（复用 syslog sink 的缓冲设置与本地绑定策略）
    pub async fn connect_udp(addr: &str) -> anyhow::Result<Self> {
        let target: SocketAddr = addr.parse()?;
        let domain = match target {
            SocketAddr::V4(_) => Domain::IPV4,
            SocketAddr::V6(_) => Domain::IPV6,
        };
        let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        let _ = sock.set_send_buffer_size(4 * 1024 * 1024);
        let local: SocketAddr = match target {
            SocketAddr::V4(_) => "0.0.0.0:0".parse()?,
            SocketAddr::V6(_) => "[::]:0".parse()?,
        };
        sock.bind(&local.into())?;
        sock.connect(&target.into())?;
        sock.set_nonblocking(true)?;
        let std_sock = std::net::UdpSocket::from(sock);
        let socket = UdpSocket::from_std(std_sock)?;
        let peer = socket.peer_addr().ok().map(|a| a.to_string());
        let local = socket.local_addr().ok().map(|a| a.to_string());
        Ok(Self {
            transport: Transport::Udp(socket),
            sent_cnt: 0,
            backpressure: None,
            avg_write_len: 0.0,
            bytes_since_probe: 0,
            #[cfg(test)]
            sndbuf_override: None,
            #[cfg(test)]
            probe_count: 0,
            #[cfg(test)]
            pending_override: None,
            #[cfg(test)]
            last_slept_ms: 0,
            nodelay_on: None,
            nodelay_last_change: None,
            avg_bytes_acc: 0,
            avg_writes_acc: 0,
            cached_sndbuf: None,
            last_probe_at: None,
            peer_addr: peer,
            local_addr: local,
        })
    }

    /// 建立 TCP 连接
    pub async fn connect_tcp(addr: &str) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        // 默认保留 Nagle，以提升小包高 EPS 的吞吐（降低 PPS 与系统调用次数）。
        // 同时尽力扩大发送缓冲区，减小对端拥塞造成的写错误风险。
        let peer = stream.peer_addr().ok().map(|a| a.to_string());
        let local = stream.local_addr().ok().map(|a| a.to_string());
        Ok(Self {
            transport: Transport::Tcp(stream),
            sent_cnt: 0,
            backpressure: None,
            avg_write_len: 0.0,
            bytes_since_probe: 0,
            #[cfg(test)]
            sndbuf_override: None,
            #[cfg(test)]
            probe_count: 0,
            #[cfg(test)]
            pending_override: None,
            #[cfg(test)]
            last_slept_ms: 0,
            nodelay_on: None,
            nodelay_last_change: None,
            avg_bytes_acc: 0,
            avg_writes_acc: 0,
            cached_sndbuf: None,
            last_probe_at: None,
            peer_addr: peer,
            local_addr: local,
        })
    }

    /// 基于发送策略构建 TCP 写入器（建议在构建期确定是否启用 backoff）。
    pub async fn connect_tcp_with_policy(
        addr: &str,
        policy: NetSendPolicy,
    ) -> anyhow::Result<Self> {
        let mut w = Self::connect_tcp(addr).await?;
        let enable = match policy.backoff_mode {
            BackoffMode::ForceOn => true,
            BackoffMode::ForceOff => false,
            BackoffMode::Auto => policy.rate_limit_rps == 0,
        };
        if enable {
            w.backpressure = Some(if policy.adaptive {
                BackpressureCfg::adaptive_default()
            } else {
                BackpressureCfg::default()
            });
        } else {
            w.backpressure = None;
        }
        Ok(w)
    }

    /// 写入原始字节（UDP 发送单报文；TCP write_all）
    pub async fn write(&mut self, bytes: &[u8]) -> SinkResult<()> {
        if matches!(self.transport, Transport::Tcp(_)) && self.backpressure.is_some() {
            // 仅统计累加，真正计算与退让在观测点执行
            let len = bytes.len();
            self.avg_bytes_acc = self.avg_bytes_acc.saturating_add(len);
            self.avg_writes_acc = self.avg_writes_acc.saturating_add(1);
            self.bytes_since_probe = self.bytes_since_probe.saturating_add(len);
            // 依据估算 avg 选择小/中大路径进行一次可能的观测
            if self.estimate_avg_len() <= NET_BACKOFF_SMALL_BYPASS_BYTES {
                self.handle_small_probe().await;
            } else {
                self.handle_large_probe().await;
            }
        }
        match &mut self.transport {
            Transport::Udp(sock) => {
                sock.send(bytes).await.map_err(|e| {
                    SinkError::from(SinkReason::Sink(format!("udp send error: {}", e)))
                })?;
                self.sent_cnt = self.sent_cnt.saturating_add(1);
                Ok(())
            }
            Transport::Tcp(stream) => {
                if let Err(e) = stream.write_all(bytes).await {
                    // 发送失败时，记录策略与水位的快照，便于定位“发送过快”或对端复位等问题
                    self.log_tcp_send_error(&e, bytes.len());
                    return Err(SinkError::from(SinkReason::Sink(format!(
                        "tcp send error: {}",
                        e
                    ))));
                }
                self.sent_cnt = self.sent_cnt.saturating_add(1);
                Ok(())
            }
            #[cfg(test)]
            Transport::Null => {
                self.sent_cnt = self.sent_cnt.saturating_add(1);
                Ok(())
            }
        }
    }

    // probe/backoff helpers are implemented in submodules

    /// 尝试优雅关闭 TCP 写端，促使对端尽快读取完所有已提交数据并收到 FIN。
    pub async fn shutdown(&mut self) -> SinkResult<()> {
        if let Transport::Tcp(stream) = &mut self.transport {
            stream.shutdown().await.map_err(|e| {
                SinkError::from(SinkReason::Sink(format!("tcp shutdown error: {}", e)))
            })?;
        }
        Ok(())
    }

    /// Best-effort wait until TCP send queue drains or timeout.
    pub async fn drain_until_empty(&self, max_wait: std::time::Duration) {
        let start = std::time::Instant::now();
        let poll = std::time::Duration::from_millis(NET_TCP_DRAIN_POLL_MS);
        loop {
            if start.elapsed() >= max_wait {
                break;
            }
            let pending = self.os_sendq_len().unwrap_or(None);
            match pending {
                Some(0) => break,
                Some(_) => {}
                None => break,
            }
            tokio::time::sleep(poll).await;
        }
    }
}

#[cfg(test)]
impl NetWriter {
    pub fn test_stub() -> Self {
        Self {
            transport: Transport::Null,
            sent_cnt: 0,
            backpressure: None,
            avg_write_len: 0.0,
            bytes_since_probe: 0,
            sndbuf_override: None,
            probe_count: 0,
            pending_override: None,
            last_slept_ms: 0,
            nodelay_on: None,
            nodelay_last_change: None,
            avg_bytes_acc: 0,
            avg_writes_acc: 0,
            cached_sndbuf: None,
            last_probe_at: None,
            peer_addr: None,
            local_addr: None,
        }
    }
    pub fn test_set_backpressure_enabled(&mut self, enabled: bool) {
        self.backpressure = if enabled {
            Some(BackpressureCfg::default())
        } else {
            None
        };
    }
    pub fn test_override_sndbuf(&mut self, cap: Option<usize>) {
        self.sndbuf_override = cap;
    }
    pub fn test_override_pending(&mut self, pending: Option<usize>) {
        self.pending_override = pending;
    }
    pub fn test_get_last_slept_ms(&self) -> u64 {
        self.last_slept_ms
    }
    pub fn test_reset_last_slept(&mut self) {
        self.last_slept_ms = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn fixed_backoff_threshold() {
        let mut cfg = BackpressureCfg::default();
        assert_eq!(cfg.auto_sleep_ms(80), NET_SENDQ_BACKOFF_SLEEP_MS);
        assert_eq!(cfg.auto_sleep_ms(20), 0);
    }

    #[test]
    fn adaptive_increase_and_decrease() {
        let mut cfg = BackpressureCfg::adaptive_default();
        assert_eq!(cfg.auto_sleep_ms(60), 1);
        assert_eq!(cfg.auto_sleep_ms(60), 2);
        assert_eq!(cfg.auto_sleep_ms(10), 1);
        assert_eq!(cfg.auto_sleep_ms(10), 0);
        assert_eq!(cfg.auto_sleep_ms(30), 0);
    }

    #[test]
    fn adaptive_clamps_to_bounds() {
        let mut cfg = BackpressureCfg::adaptive_default();
        let mut last = 0;
        for _ in 0..32 {
            last = cfg.auto_sleep_ms(99);
        }
        assert_eq!(last, 8);
        for _ in 0..32 {
            last = cfg.auto_sleep_ms(0);
        }
        assert_eq!(last, 0);
    }

    #[tokio::test]
    async fn probe_triggers_by_bytes_small_packets() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            loop {
                let _ = sock.readable().await;
                match sock.try_read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        });
        let mut writer = match NetWriter::connect_tcp(&addr.to_string()).await {
            Ok(w) => w,
            Err(_) => return,
        };
        writer.test_set_backpressure_enabled(true);
        writer.test_override_sndbuf(Some(64 * 1024));
        let msg = vec![0u8; 512];
        let initial = writer.probe_count;
        for _ in 0..128 {
            writer.write(&msg).await.unwrap();
        }
        assert!(
            writer.probe_count > initial,
            "probe should have triggered at least once"
        );
    }

    #[tokio::test]
    async fn probe_respects_large_stride_for_large_packets() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let _ = sock.readable().await;
                match sock.try_read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        });
        let mut writer = match NetWriter::connect_tcp(&addr.to_string()).await {
            Ok(w) => w,
            Err(_) => return,
        };
        writer.test_set_backpressure_enabled(true);
        writer.test_override_sndbuf(Some(64 * 1024));
        let big = vec![0u8; 8192];
        let initial = writer.probe_count;
        for _ in 0..8 {
            writer.write(&big).await.unwrap();
        }
        assert_eq!(
            writer.probe_count, initial,
            "probe should not trigger before stride reached"
        );
        for _ in 0..8 {
            writer.write(&big).await.unwrap();
        }
        assert!(
            writer.probe_count > initial,
            "probe should trigger after stride reached"
        );
    }

    #[test]
    fn dyn_high_pct_behaves_expected() {
        assert_eq!(NetWriter::compute_dynamic_high_pct(512, 64 * 1024), 88);
        assert_eq!(
            NetWriter::compute_dynamic_high_pct(32 * 1024, 64 * 1024),
            75
        );
    }

    #[test]
    fn dyn_scale_sleep_ms() {
        assert_eq!(NetWriter::scale_sleep_ms(2, 512, 64 * 1024), 1);
        assert_eq!(NetWriter::scale_sleep_ms(2, 8 * 1024, 64 * 1024), 6);
    }

    #[tokio::test]
    async fn backoff_start_condition_respects_policy_and_backpressure() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });
        let mut writer = match NetWriter::connect_tcp(&addr.to_string()).await {
            Ok(w) => w,
            Err(_) => return,
        };
        writer.test_set_backpressure_enabled(true);
        writer.test_override_sndbuf(Some(64 * 1024));
        writer.avg_write_len = (6 * 1024) as f64;
        writer.test_override_pending(Some(50 * 1024));
        writer.test_set_backpressure_enabled(false);
        writer.test_reset_last_slept();
        writer.maybe_backoff().await;
        assert_eq!(
            writer.test_get_last_slept_ms(),
            0,
            "rate-limited: no backoff"
        );
        writer.test_set_backpressure_enabled(true);
        set_net_backoff_adaptive(false);
        writer.test_reset_last_slept();
        writer.maybe_backoff().await;
        assert!(
            writer.test_get_last_slept_ms() > 0,
            "unlimited: backoff should be active and sleep > 0ms"
        );
        set_net_backoff_adaptive(true);
    }
}

// compute/scale helpers defined in backoff.rs
