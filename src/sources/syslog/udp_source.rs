//! UDP Syslog source implementation
//!
//! This module provides the UDP-based syslog source that can receive syslog messages
//! over UDP protocol. Syslog parsing (header strip, tag extraction) is done in the
//! preprocessing hook, not in the decoder layer.
//!
//! ## Performance Optimizations
//!
//! - **Batch size**: Up to 128 packets per `receive()` call (matching TCP)
//! - **fast_strip**: Skip full syslog parsing when only stripping header (no tags)
//! - **Bytes payload**: Convert recv buffers directly into `RawData::Bytes` for zero-copy sharing downstream
//! - **recvmmsg()**: On Linux, use `recvmmsg()` to receive multiple datagrams in one syscall

use crate::sources::event_id::next_event_id;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use tokio::net::UdpSocket;
use wp_connector_api::{DataSource, EventPreHook, SourceBatch, SourceEvent, Tags};
use wp_connector_api::{SourceError, SourceReason, SourceResult};
use wp_parse_api::RawData;

use super::normalize;

/// Maximum batch size for UDP receive (matches TCP for fairness)
const UDP_BATCH_SIZE: usize = 128;
/// Maximum datagram size (syslog payload over UDP cannot exceed this)
const UDP_MAX_DATAGRAM: usize = 65536;

/// Allocate a packet buffer with pre-set length so it can be handed directly to recv APIs.
fn allocate_packet_buffer() -> BytesMut {
    let mut buf = BytesMut::with_capacity(UDP_MAX_DATAGRAM);
    unsafe {
        buf.set_len(UDP_MAX_DATAGRAM);
    }
    buf
}

/// Reset an existing packet buffer, ensuring it is ready for the next recv call.
#[cfg(target_os = "linux")]
fn reset_packet_buffer(buf: &mut BytesMut) {
    buf.clear();
    if buf.capacity() < UDP_MAX_DATAGRAM {
        buf.reserve(UDP_MAX_DATAGRAM - buf.capacity());
    }
    unsafe {
        buf.set_len(UDP_MAX_DATAGRAM);
    }
}

/// Freeze the current packet (taking ownership) and replace the working buffer with a fresh one.
fn freeze_packet_buffer(buf: &mut BytesMut, len: usize) -> Bytes {
    let mut owned = allocate_packet_buffer();
    std::mem::swap(&mut owned, buf);
    owned.truncate(len);
    owned.freeze()
}

/// Build syslog preprocessing hook based on configuration
///
/// This is the unified syslog processing logic for both UDP and TCP sources.
/// The preprocessing hook is called on each SourceEvent before parsing.
///
/// # Arguments
/// * `strip` - Whether to strip syslog header (skip/tag mode)
/// * `attach` - Whether to attach metadata tags (tag mode)
/// * `fast` - Enable fast_strip optimization (skip full parsing when strip=true, attach=false)
///
/// # Header Mode Mapping
/// - `raw`  (strip=false, attach=false) => returns None, no preprocessing
/// - `skip` (strip=true,  attach=false) => strip header only (fast path if fast=true)
/// - `tag`  (strip=true,  attach=true)  => strip header + extract tags (full parse required)
///
/// # Fast Strip Behavior
/// When fast=true, the hook first attempts strict RFC5424/RFC3164 validation.
/// Only if the format is strictly valid will the fast path be used.
/// Any non-conforming format immediately falls back to normalize::normalize_slice.
pub fn build_preproc_hook(strip: bool, attach: bool, fast: bool) -> Option<EventPreHook> {
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

        // Fast strip path: only for strictly valid RFC3164/5424 formats
        // Uses the same validation logic as normalize module to ensure consistency
        if fast && strip && !attach {
            // Try strict RFC5424 fast path
            if let Some((msg_start, _meta)) = try_fast_rfc5424(s)
                && let Some(new_payload) = slice_payload(&f.payload, msg_start, s.len())
            {
                f.payload = new_payload;
                return;
            }

            // Try strict RFC3164 fast path
            if let Some((msg_start, _meta)) = try_fast_rfc3164(s)
                && let Some(new_payload) = slice_payload(&f.payload, msg_start, s.len())
            {
                f.payload = new_payload;
                return;
            }

            // Format doesn't match strict RFC3164/5424 - fall through to full parsing
            // This ensures non-syslog text is not incorrectly stripped
        }

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
            let start = ns.msg_start.min(s.len());
            let end = ns.msg_end.min(s.len());
            if let Some(new_payload) = slice_payload(&f.payload, start, end) {
                f.payload = new_payload;
            }
        }
    }))
}

/// Strict RFC5424 fast path validation
/// Returns (msg_start, pri) if format is valid, None otherwise
///
/// RFC5424: <PRI>VERSION SP TIMESTAMP SP HOSTNAME SP APP SP PROCID SP MSGID SP SD [SP MSG]
#[inline]
fn try_fast_rfc5424(s: &str) -> Option<(usize, Option<u8>)> {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }

    // Parse PRI
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' || i == 1 {
        return None;
    }
    let pri: Option<u8> = s[1..i].parse::<u16>().ok().map(|p| (p & 0xFF) as u8);
    i += 1; // after '>'

    // VERSION: must be digit(s) followed by space
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return None;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    i += 1;

    // Skip 5 space-separated tokens (TIMESTAMP HOSTNAME APP PROCID MSGID)
    let mut tok = 0;
    while i < bytes.len() && tok < 5 {
        // Token must have at least one character
        let tok_start = i;
        while i < bytes.len() && bytes[i] != b' ' {
            i += 1;
        }
        if i == tok_start || i >= bytes.len() {
            return None;
        }
        i += 1; // skip space
        tok += 1;
    }
    if tok != 5 {
        return None;
    }

    // Structured-data: '-' or '[' ... ']'
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
        if i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        return Some((i, pri));
    }
    if i < bytes.len() && bytes[i] == b'[' {
        // Find matching ']'
        if let Some(close_rel) = s[i + 1..].find(']') {
            i = i + 1 + close_rel + 1; // after ']'
            if i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            return Some((i, pri));
        }
    }
    None
}

/// Strict RFC3164 fast path validation
/// Returns (msg_start, pri) if format is valid, None otherwise
///
/// RFC3164: <PRI>MMM DD HH:MM:SS HOSTNAME TAG: MSG
#[inline]
fn try_fast_rfc3164(s: &str) -> Option<(usize, Option<u8>)> {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }

    // Parse PRI
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' || i == 1 {
        return None;
    }
    let pri: Option<u8> = s[1..i].parse::<u16>().ok().map(|p| (p & 0xFF) as u8);
    i += 1; // after '>'

    // Parse month (must be valid 3-letter abbreviation)
    if i + 3 > bytes.len() {
        return None;
    }
    let month = &s[i..i + 3];
    if !is_valid_month(month) {
        return None;
    }
    i += 3;

    // Must have space after month
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    i += 1;

    // RFC3164 allows leading space for single-digit day
    if i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }

    // Parse day (1 or 2 digits)
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return None;
    }
    i += 1;
    if i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }

    // Must have space after day
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    i += 1;

    // Parse time HH:MM:SS (exactly 8 characters with colons at positions 2 and 5)
    if i + 8 > bytes.len() {
        return None;
    }
    if !bytes[i].is_ascii_digit()
        || !bytes[i + 1].is_ascii_digit()
        || bytes[i + 2] != b':'
        || !bytes[i + 3].is_ascii_digit()
        || !bytes[i + 4].is_ascii_digit()
        || bytes[i + 5] != b':'
        || !bytes[i + 6].is_ascii_digit()
        || !bytes[i + 7].is_ascii_digit()
    {
        return None;
    }
    i += 8;

    // Must have space after time
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    i += 1;

    // Parse hostname (at least one character until space)
    let hostname_start = i;
    while i < bytes.len() && bytes[i] != b' ' {
        i += 1;
    }
    if i == hostname_start || i >= bytes.len() {
        return None;
    }
    i += 1; // skip space after hostname

    // Find ": " after tag - this is required for RFC3164
    if let Some(col) = s[i..].find(": ") {
        let msg_start = i + col + 2;
        return Some((msg_start, pri));
    }
    None
}

/// Check if a 3-character string is a valid month abbreviation
#[inline]
fn is_valid_month(month: &str) -> bool {
    matches!(
        month,
        "Jan"
            | "Feb"
            | "Mar"
            | "Apr"
            | "May"
            | "Jun"
            | "Jul"
            | "Aug"
            | "Sep"
            | "Oct"
            | "Nov"
            | "Dec"
    )
}

/// Helper to slice payload without copying when possible
#[inline]
fn slice_payload(payload: &RawData, start: usize, end: usize) -> Option<RawData> {
    if start > end {
        return None;
    }
    match payload {
        RawData::Bytes(b) => {
            let start = start.min(b.len());
            let end = end.min(b.len());
            Some(RawData::Bytes(b.slice(start..end)))
        }
        RawData::String(st) => {
            let start = start.min(st.len());
            let end = end.min(st.len());
            Some(RawData::String(st[start..end].to_string()))
        }
        RawData::ArcBytes(arc_b) => {
            let start = start.min(arc_b.len());
            let end = end.min(arc_b.len());
            // Use Bytes::copy_from_slice for ArcBytes since we can't slice it directly
            Some(RawData::Bytes(Bytes::copy_from_slice(&arc_b[start..end])))
        }
    }
}

/// UDP Syslog data source
///
/// Receives syslog messages over UDP protocol. Raw UDP datagrams are passed
/// directly to SourceEvent, and syslog header processing is done in the
/// preprocessing hook based on `header_mode` configuration.
///
/// ## Performance Features
/// - Batch receiving up to 128 packets per call
/// - fast_strip optimization for skip mode
/// - Bytes-based payload for zero-copy downstream
/// - recvmmsg() on Linux for batch syscalls
pub struct UdpSyslogSource {
    key: String,
    tags: Tags,
    socket: UdpSocket,
    /// Receive buffer for UDP datagrams (copy-on-write to hand ownership to payload)
    recv_buf: BytesMut,
    /// Cached preprocessing hook (created once, reused for all messages)
    preproc_hook: Option<EventPreHook>,
    /// Log first received packet once to help diagnose delivery
    first_seen_logged: bool,
    /// Linux-specific: batch receive buffers for recvmmsg()
    #[cfg(target_os = "linux")]
    batch_buffers: Vec<BytesMut>,
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
    /// * `fast_strip` - Enable fast_strip optimization (skip full parsing in skip mode)
    /// * `recv_buffer` - UDP socket receive buffer size (bytes)
    pub async fn new(
        key: String,
        addr: String,
        tags: Tags,
        strip_header: bool,
        attach_meta_tags: bool,
        fast_strip: bool,
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

        // Reuse addr/port so multiple instances can bind the same endpoint
        socket2.set_reuse_address(true)?;
        if let Err(e) = socket2.set_reuse_port(true) {
            debug_ctrl!(
                "UDP syslog '{}' failed to enable SO_REUSEPORT ({}); continuing without it",
                key,
                e
            );
        }

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
        // skip mode (strip=true, attach=false) => preproc_hook strips header (fast path if fast_strip=true)
        // tag mode (strip=true, attach=true) => preproc_hook strips header + extracts tags
        let preproc_hook = build_preproc_hook(strip_header, attach_meta_tags, fast_strip);

        let mode = match (strip_header, attach_meta_tags) {
            (false, false) => "raw",
            (true, false) => {
                if fast_strip {
                    "skip+fast"
                } else {
                    "skip"
                }
            }
            (true, true) => "tag",
            (false, true) => "tag-only", // unusual but possible
        };

        info_ctrl!(
            "UDP syslog source '{}': mode={}, fast_strip={}, preproc_hook={}",
            key,
            mode,
            fast_strip,
            if preproc_hook.is_some() {
                "enabled"
            } else {
                "disabled"
            }
        );

        // 64KB receive buffer for individual datagrams (max UDP payload)
        let recv_buf = allocate_packet_buffer();

        // Linux: pre-allocate batch buffers for recvmmsg()
        #[cfg(target_os = "linux")]
        let batch_buffers = (0..UDP_BATCH_SIZE)
            .map(|_| allocate_packet_buffer())
            .collect::<Vec<_>>();

        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                key,
                socket,
                tags,
                recv_buf,
                preproc_hook,
                first_seen_logged: false,
                batch_buffers,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {
                key,
                socket,
                tags,
                recv_buf,
                preproc_hook,
                first_seen_logged: false,
            })
        }
    }

    /// Receive a single UDP datagram and create a SourceEvent
    /// Detects and discards truncated packets (len == buffer size)
    async fn recv_event(&mut self) -> SourceResult<SourceEvent> {
        loop {
            match self.socket.recv_from(&mut self.recv_buf).await {
                Ok((len, addr)) => {
                    // Check for potential truncation: if len equals buffer size,
                    // the datagram was likely truncated
                    if len == UDP_MAX_DATAGRAM {
                        warn_data!(
                            "UDP syslog '{}' discarding truncated packet from {} (len={}, likely exceeded 64KB)",
                            self.key,
                            addr,
                            len
                        );
                        continue; // discard and try next packet
                    }

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

                    // Convert the mutable buffer into an owned Bytes payload (no extra copy)
                    let payload = RawData::Bytes(freeze_packet_buffer(&mut self.recv_buf, len));

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
    /// Detects and discards truncated packets (len == buffer size)
    fn try_recv_event(&mut self) -> Option<SourceEvent> {
        loop {
            match self.socket.try_recv_from(&mut self.recv_buf) {
                Ok((len, addr)) => {
                    // Check for potential truncation
                    if len == UDP_MAX_DATAGRAM {
                        warn_data!(
                            "UDP syslog '{}' discarding truncated packet from {} (len={}, likely exceeded 64KB)",
                            self.key,
                            addr,
                            len
                        );
                        continue; // discard and try next packet
                    }

                    // Use Bytes payload for zero-copy sharing
                    let payload = RawData::Bytes(freeze_packet_buffer(&mut self.recv_buf, len));

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

    /// Linux-specific: batch receive using recvmmsg() syscall
    /// Returns (len, addr, truncated) tuples for each received datagram
    #[cfg(target_os = "linux")]
    fn try_recv_batch_linux(&mut self) -> Vec<(usize, std::net::SocketAddr, bool)> {
        use std::mem::MaybeUninit;
        use std::os::unix::io::AsRawFd;

        const MAX_MSGS: usize = 64; // recvmmsg batch size (smaller than UDP_BATCH_SIZE for memory efficiency)

        let fd = self.socket.as_raw_fd();
        let mut results = Vec::with_capacity(MAX_MSGS);

        // Prepare mmsghdr array
        let mut msgvec: [MaybeUninit<libc::mmsghdr>; MAX_MSGS] =
            unsafe { MaybeUninit::uninit().assume_init() };
        let mut iovecs: [MaybeUninit<libc::iovec>; MAX_MSGS] =
            unsafe { MaybeUninit::uninit().assume_init() };
        let mut addrs: [MaybeUninit<libc::sockaddr_storage>; MAX_MSGS] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let batch_count = MAX_MSGS.min(self.batch_buffers.len());

        for i in 0..batch_count {
            let buf = &mut self.batch_buffers[i];
            reset_packet_buffer(buf);
            iovecs[i].write(libc::iovec {
                iov_base: buf.as_mut_ptr() as *mut libc::c_void,
                iov_len: UDP_MAX_DATAGRAM,
            });

            addrs[i] = MaybeUninit::zeroed();

            let mut hdr: libc::mmsghdr = unsafe { std::mem::zeroed() };
            hdr.msg_hdr.msg_name = addrs[i].as_mut_ptr() as *mut libc::c_void;
            hdr.msg_hdr.msg_namelen = std::mem::size_of::<libc::sockaddr_storage>() as u32;
            hdr.msg_hdr.msg_iov = iovecs[i].as_mut_ptr();
            hdr.msg_hdr.msg_iovlen = 1;
            hdr.msg_hdr.msg_flags = 0; // will be filled by recvmmsg
            msgvec[i].write(hdr);
        }

        // Call recvmmsg with MSG_DONTWAIT for non-blocking
        let ret = unsafe {
            libc::recvmmsg(
                fd,
                msgvec[0].as_mut_ptr(),
                batch_count as libc::c_uint,
                libc::MSG_DONTWAIT,
                std::ptr::null_mut(), // no timeout
            )
        };

        if ret <= 0 {
            return results;
        }

        let received = ret as usize;

        for i in 0..received {
            let msg = unsafe { msgvec[i].assume_init_ref() };
            let len = msg.msg_len as usize;
            let flags = msg.msg_hdr.msg_flags;

            // Check MSG_TRUNC flag - indicates datagram was truncated
            let truncated = (flags & libc::MSG_TRUNC) != 0;

            // Parse sockaddr
            let addr_ptr = unsafe { addrs[i].assume_init_ref() };
            let socket_addr = unsafe {
                match addr_ptr.ss_family as libc::c_int {
                    libc::AF_INET => {
                        let addr4 = &*(addr_ptr as *const _ as *const libc::sockaddr_in);
                        std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
                            std::net::Ipv4Addr::from(u32::from_be(addr4.sin_addr.s_addr)),
                            u16::from_be(addr4.sin_port),
                        ))
                    }
                    libc::AF_INET6 => {
                        let addr6 = &*(addr_ptr as *const _ as *const libc::sockaddr_in6);
                        std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
                            std::net::Ipv6Addr::from(addr6.sin6_addr.s6_addr),
                            u16::from_be(addr6.sin6_port),
                            addr6.sin6_flowinfo,
                            addr6.sin6_scope_id,
                        ))
                    }
                    _ => continue, // skip unknown address family
                }
            };

            results.push((len, socket_addr, truncated));
        }

        results
    }

    /// Linux-specific: convert batch receive results to SourceEvents
    /// Skips truncated packets with warning
    #[cfg(target_os = "linux")]
    fn batch_to_events(
        &mut self,
        batch_results: Vec<(usize, std::net::SocketAddr, bool)>,
    ) -> Vec<SourceEvent> {
        let mut events = Vec::with_capacity(batch_results.len());

        for (i, (len, addr, truncated)) in batch_results.into_iter().enumerate() {
            if i >= self.batch_buffers.len() {
                break;
            }

            // Skip truncated packets
            if truncated {
                warn_data!(
                    "UDP syslog '{}' discarding truncated packet from {} (len={}, MSG_TRUNC) [recvmmsg]",
                    self.key,
                    addr,
                    len
                );
                continue;
            }

            // Log first seen packet (once)
            if !self.first_seen_logged {
                info_data!(
                    "UDP syslog source '{}' received first packet from {} (len={}) [recvmmsg]",
                    self.key,
                    addr,
                    len
                );
                self.first_seen_logged = true;
            }

            let payload = RawData::Bytes(freeze_packet_buffer(&mut self.batch_buffers[i], len));

            let mut stags = self.tags.clone();
            stags.set("access_ip", addr.ip().to_string());

            let mut event = SourceEvent::new(next_event_id(), &self.key, payload, Arc::new(stags));
            event.ups_ip = Some(addr.ip());
            event.preproc = self.preproc_hook.clone();

            events.push(event);
        }

        events
    }
}

#[async_trait::async_trait]
impl DataSource for UdpSyslogSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        // Linux: use recvmmsg() for batch syscalls
        #[cfg(target_os = "linux")]
        {
            // First packet (blocking)
            let event = self.recv_event().await?;
            let mut batch = vec![event];

            // Try batch receive using recvmmsg (non-blocking)
            let batch_results = self.try_recv_batch_linux();
            let mut events = self.batch_to_events(batch_results);
            batch.append(&mut events);

            // If recvmmsg didn't get much, try individual receives
            while batch.len() < UDP_BATCH_SIZE {
                match self.try_recv_event() {
                    Some(event) => batch.push(event),
                    None => break,
                }
            }

            Ok(batch)
        }

        // Non-Linux: fallback to try_recv_from loop
        #[cfg(not(target_os = "linux"))]
        {
            let mut batch = Vec::with_capacity(UDP_BATCH_SIZE);

            // First packet (blocking)
            let event = self.recv_event().await?;
            batch.push(event);

            // Try to collect more packets without blocking
            while batch.len() < UDP_BATCH_SIZE {
                match self.try_recv_event() {
                    Some(event) => batch.push(event),
                    None => break,
                }
            }

            Ok(batch)
        }
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
    use tokio::net::UdpSocket;

    #[test]
    fn test_preproc_hook_raw_mode() {
        // raw mode: strip=false, attach=false => no hook
        let hook = build_preproc_hook(false, false, false);
        assert!(hook.is_none());
    }

    #[test]
    fn packet_buffer_helpers_roundtrip() {
        let mut buf = allocate_packet_buffer();
        // Write predictable payload at the beginning of the buffer
        let payload = b"hello-bytes";
        buf[..payload.len()].copy_from_slice(payload);

        let bytes = freeze_packet_buffer(&mut buf, payload.len());
        assert_eq!(bytes.as_ref(), payload);
        assert_eq!(buf.len(), UDP_MAX_DATAGRAM);
        assert!(buf.capacity() >= UDP_MAX_DATAGRAM);
    }

    #[tokio::test]
    async fn recv_event_uses_bytes_payload() {
        if UdpSocket::bind("127.0.0.1:0").await.is_err() {
            return;
        }
        let mut source = UdpSyslogSource::new(
            "test-src".to_string(),
            "127.0.0.1:0".to_string(),
            Tags::new(),
            false,
            false,
            false,
            1024,
        )
        .await
        .unwrap();

        let listen_addr = source.socket.local_addr().unwrap();

        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender
            .send_to(b"<13>Aug 11 22:14:15 host app: body", listen_addr)
            .await
            .unwrap();

        let event = source.recv_event().await.unwrap();
        match event.payload {
            RawData::Bytes(bytes) => {
                assert_eq!(bytes.as_ref(), b"<13>Aug 11 22:14:15 host app: body");
            }
            other => panic!("expected RawData::Bytes, got {:?}", other),
        }
    }

    #[test]
    fn test_preproc_hook_skip_mode() {
        // skip mode: strip=true, attach=false, fast=false (full parsing)
        let hook = build_preproc_hook(true, false, false);
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
    fn test_preproc_hook_skip_fast_mode() {
        // skip+fast mode: strip=true, attach=false, fast=true (fast path)
        let hook = build_preproc_hook(true, false, true);
        assert!(hook.is_some());

        // Test RFC3164 fast path
        let mut event = SourceEvent::new(
            1,
            "test",
            RawData::String("<34>Oct 11 22:14:15 mymachine app: hello world".into()),
            Arc::new(Tags::new()),
        );
        hook.as_ref().unwrap()(&mut event);
        assert_eq!(event.payload.to_string(), "hello world");

        // Test RFC5424 fast path
        let mut event5424 = SourceEvent::new(
            2,
            "test",
            RawData::String("<14>1 2024-10-05T12:34:56Z host app 123 - - hello world".into()),
            Arc::new(Tags::new()),
        );
        hook.as_ref().unwrap()(&mut event5424);
        assert_eq!(event5424.payload.to_string(), "hello world");
    }

    #[test]
    fn test_preproc_hook_tag_mode() {
        // tag mode: strip=true, attach=true (fast_strip has no effect when attaching tags)
        let hook = build_preproc_hook(true, true, false);
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
