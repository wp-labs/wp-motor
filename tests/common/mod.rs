#![allow(dead_code)]
// Common helpers for integration tests under `tests/`
// Keep this module minimal and dependency‑free so any test file can `mod common;` safely.

use std::net::{TcpListener, UdpSocket};

// Re-export types used by builders to avoid long paths at call sites.
use toml::map::Map as TomlMap;
use wp_connector_api::SourceSpec as ResolvedSourceSpec;

/// Return true if we can bind a UDP socket on localhost (ephemeral port).
/// Some CI sandboxes disallow network binds; affected tests should early‑return.
pub fn is_udp_available() -> bool {
    UdpSocket::bind("127.0.0.1:0").is_ok()
}

/// Return true if we can bind a TCP listener on localhost (ephemeral port).
/// Useful for conditionally running TCP dependent tests.
pub fn is_tcp_available() -> bool {
    TcpListener::bind("127.0.0.1:0").is_ok()
}

/// Small assertion helper: assert that `haystack` contains `needle`,
/// with a shorter panic message that prints a compact diff context.
pub fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expect substring not found. needle=\n---\n{}\n---\nin:\n{}",
        needle,
        preview(haystack, 240)
    );
}

/// Return a truncated preview of a potentially long string.
fn preview(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut out = s[..max].to_string();
        out.push_str("...<truncated>");
        out
    }
}

/// Builder for file source specifications (unified across tests)
pub struct FileSourceBuilder {
    name: String,
    path: String,
    tags: Vec<String>,
    instances: Option<i64>,
}

impl FileSourceBuilder {
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            tags: Vec::new(),
            instances: None,
        }
    }

    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_instances(mut self, instances: i64) -> Self {
        self.instances = Some(instances);
        self
    }

    pub fn build(self) -> ResolvedSourceSpec {
        let mut params = TomlMap::new();
        params.insert("path".to_string(), toml::Value::String(self.path));
        params.insert(
            "encode".to_string(),
            toml::Value::String("text".to_string()),
        );
        if let Some(inst) = self.instances {
            params.insert("instances".to_string(), toml::Value::Integer(inst));
        }
        ResolvedSourceSpec {
            name: self.name,
            kind: "file".to_string(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params),
            tags: self.tags,
        }
    }
}

/// Builder for syslog source specifications (UDP/TCP)
pub struct SyslogSourceBuilder {
    name: String,
    addr: String,
    port: i64,
    protocol: String,
    tags: Vec<String>,
    tcp_recv_bytes: Option<i64>,
    strip_header: bool,
    attach_meta_tags: bool,
}

impl SyslogSourceBuilder {
    pub fn new(name: &str, protocol: &str) -> Self {
        Self {
            name: name.to_string(),
            addr: "127.0.0.1".to_string(),
            port: 0,
            protocol: protocol.to_string(),
            tags: Vec::new(),
            tcp_recv_bytes: None,
            strip_header: true,
            attach_meta_tags: true,
        }
    }

    pub fn with_port(mut self, port: i64) -> Self {
        self.port = port;
        self
    }

    pub fn with_addr(mut self, addr: &str) -> Self {
        self.addr = addr.to_string();
        self
    }

    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_tcp_buffer(mut self, size: i64) -> Self {
        self.tcp_recv_bytes = Some(size);
        self
    }

    pub fn without_header_stripping(mut self) -> Self {
        self.strip_header = false;
        self
    }

    pub fn without_meta_tags(mut self) -> Self {
        self.attach_meta_tags = false;
        self
    }

    pub fn build(self) -> ResolvedSourceSpec {
        let mut params = TomlMap::new();
        params.insert("addr".to_string(), toml::Value::String(self.addr));
        params.insert("port".to_string(), toml::Value::Integer(self.port));
        params.insert("protocol".to_string(), toml::Value::String(self.protocol));
        params.insert(
            "strip_header".to_string(),
            toml::Value::Boolean(self.strip_header),
        );
        params.insert(
            "attach_meta_tags".to_string(),
            toml::Value::Boolean(self.attach_meta_tags),
        );
        if let Some(tcp_bytes) = self.tcp_recv_bytes {
            params.insert(
                "tcp_recv_bytes".to_string(),
                toml::Value::Integer(tcp_bytes),
            );
        }
        ResolvedSourceSpec {
            name: self.name,
            kind: "syslog".to_string(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params),
            tags: self.tags,
        }
    }
}
