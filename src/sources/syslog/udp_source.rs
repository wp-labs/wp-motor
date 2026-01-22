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
    strip_header: bool,
    attach_meta_tags: bool,
    fast_strip: bool,
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
    pub async fn new(
        key: String,
        addr: String,
        tags: Tags,
        strip_header: bool,
        attach_meta_tags: bool,
        fast_strip: bool,
    ) -> anyhow::Result<Self> {
        // Parse address and create socket
        let target: SocketAddr = addr.parse()?;
        let socket = UdpSocket::bind(&target).await?;
        let local = socket
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| addr.clone());

        // 控制面与数据面双日志，记录监听地址与实际本地地址（包含端口）
        info_ctrl!("UDP syslog listen '{}' addr={} local={}", key, addr, local);

        // Create compatible decoder for UdpFramed
        let decoder = DatagramDecoder::default();

        let frame = UdpFramed::new(socket, decoder);

        Ok(Self {
            key,
            frame,
            tags,
            strip_header,
            attach_meta_tags,
            fast_strip,
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

                    // 预处理闭包：在 parse 侧决定是否 strip/注入 syslog 元信息
                    let strip = self.strip_header;
                    let attach = self.attach_meta_tags;
                    let fast = self.fast_strip;
                    let pre: Option<EventPreHook> = if strip || attach {
                        Some(std::sync::Arc::new(move |f: &mut SourceEvent| {
                            // 仅对可视文本做 syslog 规范化
                            let s_opt = match &f.payload {
                                RawData::String(s) => Some(s.as_str()),
                                RawData::Bytes(b) => std::str::from_utf8(b).ok(),
                                RawData::ArcBytes(b) => std::str::from_utf8(b).ok(),
                            };
                            if let Some(s) = s_opt {
                                if fast
                                    && strip
                                    && !attach
                                    && let Some(pos) = s.find(" wpgen: ")
                                {
                                    let start = pos + 8;
                                    match &mut f.payload {
                                        RawData::Bytes(b) => {
                                            let len = b.len();
                                            if start <= len {
                                                *b = b.slice(start..len);
                                            }
                                        }
                                        RawData::String(st) => {
                                            if start <= st.len() {
                                                *st = st[start..].to_string();
                                            }
                                        }
                                        RawData::ArcBytes(_) => {
                                            // ArcBytes 是不可变的，不能进行 in-place 修改
                                            // 暂时跳过 fast strip，保持原数据
                                        }
                                    }
                                    return;
                                }
                                let ns = normalize::normalize_slice(s);
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
                                        RawData::ArcBytes(_) => {
                                            // ArcBytes 是不可变的，不能进行 in-place 修改
                                            // 保持原数据不变
                                        }
                                    }
                                }
                            }
                        }) as EventPreHook)
                    } else {
                        None
                    };

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
        let event = self.recv_event().await?;
        Ok(vec![event])
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        use futures_util::FutureExt;
        use futures_util::StreamExt;
        let out = self.frame.next().now_or_never()?;
        match out {
            Some(Ok((event, addr))) => {
                let mut stags = self.tags.clone();
                stags.set("access_ip", addr.ip().to_string());

                let strip = self.strip_header;
                let attach = self.attach_meta_tags;
                let fast = self.fast_strip;
                let pre: Option<EventPreHook> = if strip || attach {
                    Some(std::sync::Arc::new(move |f: &mut SourceEvent| {
                        let s_opt = match &f.payload {
                            RawData::String(s) => Some(s.as_str()),
                            RawData::Bytes(b) => std::str::from_utf8(b).ok(),
                            RawData::ArcBytes(b) => std::str::from_utf8(b).ok(),
                        };
                        if let Some(s) = s_opt {
                            if fast
                                && strip
                                && !attach
                                && let Some(pos) = s.find(" wpgen: ")
                            {
                                let start = pos + 8;
                                match &mut f.payload {
                                    RawData::Bytes(b) => {
                                        let len = b.len();
                                        if start <= len {
                                            *b = b.slice(start..len);
                                        }
                                    }
                                    RawData::String(st) => {
                                        if start <= st.len() {
                                            *st = st[start..].to_string();
                                        }
                                    }
                                    RawData::ArcBytes(_) => {}
                                }
                                return;
                            }
                            let ns = normalize::normalize_slice(s);
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
                                    RawData::ArcBytes(_) => {}
                                }
                            }
                        }
                    }) as EventPreHook)
                } else {
                    None
                };

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
