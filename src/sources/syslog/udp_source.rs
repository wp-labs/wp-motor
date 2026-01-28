//! UDP Syslog source implementation
//!
//! This module provides the UDP-based syslog source that can receive syslog messages
//! over UDP protocol with automatic framing and normalization.

use crate::sources::event_id::next_event_id;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use bytes::{Bytes, BytesMut};
use tokio::net::UdpSocket;
use tokio_util::codec::Decoder as TokioDecoder;
use tokio_util::udp::UdpFramed;
use wp_connector_api::{DataSource, EventPreHook, SourceBatch, SourceEvent, Tags};
use wp_connector_api::{SourceError, SourceReason, SourceResult};
use wp_parse_api::RawData;

use super::normalize;

/// Build syslog preprocessing hook based on configuration
///
/// # Arguments
/// * `strip` - Whether to strip syslog header
/// * `attach` - Whether to attach metadata tags
fn build_preproc_hook(strip: bool, attach: bool) -> Option<EventPreHook> {
    if !strip && !attach {
        return None;
    }

    Some(Arc::new(move |f: &mut SourceEvent| {
        // Get text representation from payload
        let s_opt = match &f.payload {
            RawData::String(s) => Some(s.as_str()),
            RawData::Bytes(b) => std::str::from_utf8(b).ok(),
            RawData::ArcBytes(b) => std::str::from_utf8(b).ok(),
        };

        let Some(s) = s_opt else { return };

        // Full syslog normalization
        let ns = normalize::normalize_slice(s);

        // Attach metadata tags if requested
        if attach {
            let tags = Arc::make_mut(&mut f.tags);
            if let Some(pri) = ns.meta.pri {
                tags.set("syslog.pri", pri.to_string());
            }
            if let Some(ref fac) = ns.meta.facility {
                tags.set("syslog.facility", fac.clone());
            }
            if let Some(ref sev) = ns.meta.severity {
                tags.set("syslog.severity", sev.clone());
            }
        }

        // Strip header if requested
        if strip {
            match &mut f.payload {
                RawData::Bytes(b) => {
                    let start = ns.msg_start.min(b.len());
                    let end = ns.msg_end.min(b.len());
                    if start <= end {
                        *b = b.slice(start..end);
                    }
                }
                RawData::String(st) => {
                    let start = ns.msg_start.min(st.len());
                    let end = ns.msg_end.min(st.len());
                    *st = st[start..end].to_string();
                }
                RawData::ArcBytes(arc_b) => {
                    // Convert ArcBytes to Bytes for modification
                    let start = ns.msg_start.min(arc_b.len());
                    let end = ns.msg_end.min(arc_b.len());
                    if start <= end {
                        let new_bytes = Bytes::copy_from_slice(&arc_b[start..end]);
                        f.payload = RawData::Bytes(new_bytes);
                    }
                }
            }
        }
    }))
}

#[derive(Debug, Default, Clone)]
struct DatagramDecoder {
    inner: crate::protocol::syslog::SyslogDecoder,
}

impl TokioDecoder for DatagramDecoder {
    type Item = crate::protocol::syslog::SyslogFrame;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }
        let raw = src.split().freeze();
        let frame = self
            .inner
            .decode_bytes(raw)
            .context("decode syslog datagram")?;
        Ok(Some(frame))
    }
}

/// UDP Syslog data source
///
/// Receives syslog messages over UDP protocol
pub struct UdpSyslogSource {
    key: String,
    tags: Tags,
    frame: UdpFramed<DatagramDecoder>,
    /// Cached preprocessing hook (created once, reused for all messages)
    preproc_hook: Option<EventPreHook>,
    // Log first received packet once to help diagnose delivery
    first_seen_logged: bool,
}

impl UdpSyslogSource {
    /// Create a new UDP syslog source
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this source
    /// * `addr` - Address to bind to (e.g., "0.0.0.0:514")
    /// * `tags` - Tags to attach to received messages
    /// * `strip_header` - Whether to strip syslog header
    /// * `attach_meta_tags` - Whether to attach syslog metadata as tags
    /// * `recv_buffer` - UDP socket receive buffer size (bytes)
    pub async fn new(
        key: String,
        addr: String,
        tags: Tags,
        strip_header: bool,
        attach_meta_tags: bool,
        recv_buffer: usize,
    ) -> anyhow::Result<Self> {
        use socket2::{Domain, Protocol, Socket, Type};

        // Parse address
        let target: SocketAddr = addr.parse()?;

        // Create socket with socket2 to set buffer size before binding
        let domain = if target.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };
        let socket2 = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

        // Set receive buffer size before binding
        if recv_buffer > 0 {
            socket2.set_recv_buffer_size(recv_buffer)?;
        }

        // Bind the socket
        socket2.bind(&target.into())?;
        socket2.set_nonblocking(true)?;

        let actual_size = socket2.recv_buffer_size().unwrap_or(0);

        // Convert to tokio UdpSocket
        let std_socket: std::net::UdpSocket = socket2.into();
        let socket = UdpSocket::from_std(std_socket)?;

        let local = socket
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| addr.clone());

        info_ctrl!(
            "UDP syslog listen '{}' addr={} local={} recv_buffer={}->{}",
            key,
            addr,
            local,
            recv_buffer,
            actual_size
        );

        let decoder = DatagramDecoder::default();
        let frame = UdpFramed::new(socket, decoder);

        // Create preprocessing hook once, reuse for all messages
        let preproc_hook = build_preproc_hook(strip_header, attach_meta_tags);

        Ok(Self {
            key,
            frame,
            tags,
            preproc_hook,
            first_seen_logged: false,
        })
    }

    async fn recv_event(&mut self) -> SourceResult<SourceEvent> {
        use futures_util::StreamExt;

        if let Some(data) = self.frame.next().await {
            match data {
                Ok((event, addr)) => {
                    // Log first seen packet (once)
                    if !self.first_seen_logged {
                        info_data!(
                            "UDP syslog source '{}' received first packet from {}",
                            self.key,
                            addr
                        );
                        self.first_seen_logged = true;
                    }
                    // 基础标签：克隆并附加 access_ip
                    let mut stags = self.tags.clone();
                    stags.set("access_ip", addr.ip().to_string());

                    // 使用缓存的预处理逻辑（避免每次创建新闭包）
                    let pre = self.preproc_hook.clone();

                    let mut frame = SourceEvent::new(
                        next_event_id(),
                        &self.key,
                        RawData::Bytes(Bytes::copy_from_slice(event.message_bytes())),
                        Arc::new(stags),
                    );
                    frame.ups_ip = Some(addr.ip());
                    frame.preproc = pre;
                    return Ok(frame);
                }
                Err(e) => {
                    error_data!("UDP syslog '{}' failed to read frame: {}", self.key, e);
                }
            }
        }
        Err(SourceError::from(SourceReason::NotData))
    }
}

#[async_trait::async_trait]
impl DataSource for UdpSyslogSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        // Batch receive: collect multiple packets at once for better throughput
        const BATCH_SIZE: usize = 32;
        let mut batch = Vec::with_capacity(BATCH_SIZE);

        // First packet (blocking)
        let event = self.recv_event().await?;
        batch.push(event);

        // Try to collect more packets without blocking
        while batch.len() < BATCH_SIZE {
            match self.try_receive() {
                Some(mut events) => batch.append(&mut events),
                None => break,
            }
        }

        Ok(batch)
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        use futures_util::FutureExt;
        use futures_util::StreamExt;
        let out = self.frame.next().now_or_never()?;
        match out {
            Some(Ok((event, addr))) => {
                let mut stags = self.tags.clone();
                stags.set("access_ip", addr.ip().to_string());

                // 使用缓存的预处理逻辑（避免每次创建新闭包）
                let pre = self.preproc_hook.clone();

                let mut frame = SourceEvent::new(
                    next_event_id(),
                    &self.key,
                    RawData::Bytes(Bytes::copy_from_slice(event.message_bytes())),
                    Arc::new(stags),
                );
                frame.ups_ip = Some(addr.ip());
                frame.preproc = pre;
                Some(vec![frame])
            }
            Some(Err(e)) => {
                error_data!("UDP syslog '{}' try_receive error: {}", self.key, e);
                None
            }
            None => None,
        }
    }

    fn can_try_receive(&mut self) -> bool {
        true
    }

    fn identifier(&self) -> String {
        self.key.clone()
    }
}
