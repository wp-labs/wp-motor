//! TCP server configuration and tuning parameters
//!
//! This module contains all configuration options and environmental
//! tunables for the TCP server implementation.

/// Environment variable names for TCP tuning
pub const ENV_TCP_READ_BURST: &str = "WF_TCP_READ_BURST";
pub const ENV_TCP_READ_CHUNK: &str = "WF_TCP_READ_CHUNK";

/// Default TCP read burst size
pub const TCP_READ_BURST_DEFAULT: usize = 64;
pub const TCP_READ_BURST_MIN: usize = 1;
pub const TCP_READ_BURST_MAX: usize = 1024;

/// Default TCP read chunk size
pub const TCP_READ_CHUNK_DEFAULT: usize = 64 * 1024;
pub const TCP_READ_CHUNK_MIN: usize = 1024;
pub const TCP_READ_CHUNK_MAX: usize = 4 * 1024 * 1024;

/// TCP server runtime tunables
#[derive(Clone, Copy, Debug)]
pub struct TcpTunables {
    pub read_burst: usize,
    pub read_chunk: usize,
}

impl TcpTunables {
    /// Create new tunables with custom values
    pub fn new(read_burst: usize, read_chunk: usize) -> Self {
        Self {
            read_burst: read_burst.clamp(TCP_READ_BURST_MIN, TCP_READ_BURST_MAX),
            read_chunk: read_chunk.clamp(TCP_READ_CHUNK_MIN, TCP_READ_CHUNK_MAX),
        }
    }
}

impl Default for TcpTunables {
    fn default() -> Self {
        Self {
            read_burst: TCP_READ_BURST_DEFAULT,
            read_chunk: TCP_READ_CHUNK_DEFAULT,
        }
    }
}
