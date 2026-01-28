//! UDP Syslog source implementation
//!
//! This module provides the UDP-based syslog source that can receive syslog messages
//! over UDP protocol. Syslog parsing (header strip, tag extraction) is done in the
//! preprocessing hook, not in the decoder layer.

use crate::sources::event_id::next_event_id;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::net::UdpSocket;
use wp_connector_api::{DataSource, EventPreHook, SourceBatch, SourceEvent, Tags};
use wp_connector_api::{SourceError, SourceReason, SourceResult};
use wp_parse_api::RawData;

use super::normalize;

/// Build syslog preprocessing hook based on configuration
///
/// This is the unified syslog processing logic for both UDP and TCP sources.
/// The preprocessing hook is called on each SourceEvent before parsing.
///
/// # Arguments
/// * `strip` - Whether to strip syslog header (skip/tag mode)
/// * `attach` - Whether to attach metadata tags (tag mode)
///
/// # Header Mode Mapping
/// - `raw`  (strip=false, attach=false) => returns None, no preprocessing
/// - `skip` (strip=true,  attach=false) => strip header only
/// - `tag`  (strip=true,  attach=true)  => strip header + extract tags
pub fn build_preproc_hook(strip: bool, attach: bool) -> Option<EventPreHook> {
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

        // Full syslog normalization - parse header to find message body
        let ns = normalize::normalize_slice(s);

        // Attach metadata tags if requested (tag mode)
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

        // Strip header if requested (skip/tag mode)
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

/// UDP Syslog data source
///
/// Receives syslog messages over UDP protocol. Raw UDP datagrams are passed
/// directly to SourceEvent, and syslog header processing is done in the
/// preprocessing hook based on `header_mode` configuration.
pub struct UdpSyslogSource {
    key: String,
    tags: Tags,
    socket: UdpSocket,
    /// Receive buffer for UDP datagrams
    recv_buf: Vec<u8>,
    /// Cached preprocessing hook (created once, reused for all messages)
    preproc_hook: Option<EventPreHook>,
    /// Log first received packet once to help diagnose delivery
    first_seen_logged: bool,
}

impl UdpSyslogSource {
    /// Create a new UDP syslog source
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this source
    /// * `addr` - Address to bind to (e.g., "0.0.0.0:514")
    /// * `tags` - Tags to attach to received messages
    /// * `strip_header` - Whether to strip syslog header (skip/tag mode)
    /// * `attach_meta_tags` - Whether to attach syslog metadata as tags (tag mode)
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

        // Create preprocessing hook once, reuse for all messages
        // raw mode (strip=false, attach=false) => preproc_hook = None
        // skip mode (strip=true, attach=false) => preproc_hook strips header
        // tag mode (strip=true, attach=true) => preproc_hook strips header + extracts tags
        let preproc_hook = build_preproc_hook(strip_header, attach_meta_tags);

        let mode = match (strip_header, attach_meta_tags) {
            (false, false) => "raw",
            (true, false) => "skip",
            (true, true) => "tag",
            (false, true) => "tag-only", // unusual but possible
        };

        info_ctrl!(
            "UDP syslog source '{}': mode={}, preproc_hook={}",
            key,
            mode,
            if preproc_hook.is_some() {
                "enabled"
            } else {
                "disabled"
            }
        );

        // 64KB receive buffer for individual datagrams (max UDP payload)
        let recv_buf = vec![0u8; 65536];

        Ok(Self {
            key,
            socket,
            tags,
            recv_buf,
            preproc_hook,
            first_seen_logged: false,
        })
    }

    /// Receive a single UDP datagram and create a SourceEvent
    async fn recv_event(&mut self) -> SourceResult<SourceEvent> {
        loop {
            match self.socket.recv_from(&mut self.recv_buf).await {
                Ok((len, addr)) => {
                    // Log first seen packet (once) - only log metadata, not content
                    if !self.first_seen_logged {
                        info_data!(
                            "UDP syslog source '{}' received first packet from {} (len={})",
                            self.key,
                            addr,
                            len
                        );
                        self.first_seen_logged = true;
                    }

                    // Clone the received bytes (raw UDP datagram, including syslog header)
                    let payload = RawData::Bytes(Bytes::copy_from_slice(&self.recv_buf[..len]));

                    // Create tags with access_ip
                    let mut stags = self.tags.clone();
                    stags.set("access_ip", addr.ip().to_string());

                    // Create SourceEvent with raw payload
                    let mut event =
                        SourceEvent::new(next_event_id(), &self.key, payload, Arc::new(stags));
                    event.ups_ip = Some(addr.ip());
                    // Attach preprocessing hook (will strip header / extract tags based on config)
                    event.preproc = self.preproc_hook.clone();

                    return Ok(event);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    // Interrupted by signal, retry
                    continue;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Should not happen in async recv_from, but handle gracefully
                    continue;
                }
                Err(e) => {
                    error_data!("UDP syslog '{}' recv_from error: {}", self.key, e);
                    return Err(SourceError::from(SourceReason::Disconnect(e.to_string())));
                }
            }
        }
    }

    /// Try to receive a UDP datagram without blocking
    fn try_recv_event(&mut self) -> Option<SourceEvent> {
        loop {
            match self.socket.try_recv_from(&mut self.recv_buf) {
                Ok((len, addr)) => {
                    let payload = RawData::Bytes(Bytes::copy_from_slice(&self.recv_buf[..len]));

                    let mut stags = self.tags.clone();
                    stags.set("access_ip", addr.ip().to_string());

                    let mut event =
                        SourceEvent::new(next_event_id(), &self.key, payload, Arc::new(stags));
                    event.ups_ip = Some(addr.ip());
                    event.preproc = self.preproc_hook.clone();

                    return Some(event);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, return None
                    return None;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    // Interrupted by signal, retry
                    continue;
                }
                Err(e) => {
                    error_data!("UDP syslog '{}' try_recv_from error: {}", self.key, e);
                    return None;
                }
            }
        }
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
            match self.try_recv_event() {
                Some(event) => batch.push(event),
                None => break,
            }
        }

        Ok(batch)
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        let event = self.try_recv_event()?;
        Some(vec![event])
    }

    fn can_try_receive(&mut self) -> bool {
        true
    }

    fn identifier(&self) -> String {
        self.key.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preproc_hook_raw_mode() {
        // raw mode: strip=false, attach=false => no hook
        let hook = build_preproc_hook(false, false);
        assert!(hook.is_none());
    }

    #[test]
    fn test_preproc_hook_skip_mode() {
        // skip mode: strip=true, attach=false
        let hook = build_preproc_hook(true, false);
        assert!(hook.is_some());

        let mut event = SourceEvent::new(
            1,
            "test",
            RawData::String("<13>Oct 11 22:14:15 host app: body".into()),
            Arc::new(Tags::new()),
        );
        hook.unwrap()(&mut event);
        assert_eq!(event.payload.to_string(), "body");
        // No tags should be attached in skip mode
        assert!(event.tags.get("syslog.pri").is_none());
    }

    #[test]
    fn test_preproc_hook_tag_mode() {
        // tag mode: strip=true, attach=true
        let hook = build_preproc_hook(true, true);
        assert!(hook.is_some());

        let mut event = SourceEvent::new(
            1,
            "test",
            RawData::String("<13>Oct 11 22:14:15 host app: body".into()),
            Arc::new(Tags::new()),
        );
        hook.unwrap()(&mut event);
        assert_eq!(event.payload.to_string(), "body");
        // Tags should be attached in tag mode
        assert_eq!(event.tags.get("syslog.pri"), Some("13"));
    }
}
