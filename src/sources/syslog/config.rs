//! Configuration structures for syslog sources

use super::constants::DEFAULT_TCP_RECV_BYTES;
use anyhow::ensure;

/// Configuration for syslog sources
#[derive(Debug, Clone)]
pub struct SyslogSourceSpec {
    pub addr: String,
    pub port: u16,
    pub protocol: Protocol,
    pub tcp_recv_bytes: usize,
    pub strip_header: bool,
    pub attach_meta_tags: bool,
    /// Fast strip mode (TCP only, ignored for UDP)
    pub fast_strip: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Udp,
    Tcp,
}

impl SyslogSourceSpec {
    /// Parse configuration directly from params table (Factory path)
    pub fn from_params(params: &wp_connector_api::ParamMap) -> anyhow::Result<Self> {
        if let Some(v) = params.get("protocol").and_then(|v| v.as_str()) {
            let p = v.to_ascii_lowercase();
            ensure!(
                p == "udp" || p == "tcp",
                "invalid protocol: {} (must be 'udp' or 'tcp')",
                v
            );
        }
        if let Some(v) = params.get("tcp_recv_bytes").and_then(|v| v.as_i64()) {
            ensure!(v > 0, "tcp_recv_bytes must be > 0 (got {})", v);
        }
        if let Some(v) = params.get("port").and_then(|v| v.as_i64()) {
            ensure!(
                (0..=65535).contains(&v),
                "port out of range: {} (allow 0 or 1..=65535)",
                v
            );
        }

        let addr = params
            .get("addr")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0.0")
            .to_string();
        let port = params.get("port").and_then(|v| v.as_i64()).unwrap_or(514) as u16;
        let protocol = params
            .get("protocol")
            .and_then(|v| v.as_str())
            .unwrap_or("UDP");
        let protocol = match protocol.to_uppercase().as_str() {
            "TCP" => Protocol::Tcp,
            _ => Protocol::Udp,
        };
        let tcp_recv_bytes = params
            .get("tcp_recv_bytes")
            .and_then(|v| v.as_i64())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_TCP_RECV_BYTES as i64) as usize;
        // header_mode: controls how syslog header is handled
        //   New names (preferred):
        //     raw  => keep original message untouched
        //     skip => strip header, keep message body only
        //     tag  => strip header + extract syslog.pri/facility/severity tags
        //   Legacy aliases (backward-compatible):
        //     keep  => raw
        //     strip => skip
        //     parse => tag
        let header_mode = params
            .get("header_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("skip")
            .to_ascii_lowercase();
        let (strip_header, attach_meta_tags) = match header_mode.as_str() {
            "raw" | "keep" => (false, false),
            "skip" | "strip" => (true, false),
            "tag" | "parse" => (true, true),
            other => {
                log::warn!(
                    "syslog.header_mode invalid: '{}', fallback to 'tag'",
                    other
                );
                (true, true)
            }
        };
        let fast_strip = params
            .get("fast_strip")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self {
            addr,
            port,
            protocol,
            tcp_recv_bytes,
            strip_header,
            attach_meta_tags,
            fast_strip,
        })
    }

    /// Get the full address string
    pub fn address(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }
}
