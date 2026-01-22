//! Zero-copy core types and data structures
//!
//! This module contains the fundamental zero-copy types used throughout
//! the zero-copy TCP processing system.

use std::{net::IpAddr, sync::Arc};

/// Configuration for zero-copy buffer pool
#[derive(Debug, Clone, Copy)]
pub struct ZcpConfig {
    pub buffer_size: usize,
    pub pool_capacity: usize,
    pub clear_on_return: bool,
}

impl Default for ZcpConfig {
    fn default() -> Self {
        Self {
            buffer_size: 8192,     // 8KB per buffer
            pool_capacity: 1024,   // 1024 buffers total
            clear_on_return: true, // Security: clear sensitive data
        }
    }
}

impl ZcpConfig {
    /// Set buffer size in bytes
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Set pool capacity
    pub fn with_pool_capacity(mut self, capacity: usize) -> Self {
        self.pool_capacity = capacity;
        self
    }

    /// Set clear on return behavior
    pub fn with_clear_on_return(mut self, clear: bool) -> Self {
        self.clear_on_return = clear;
        self
    }
}

/// Zero-copy error types
#[derive(Debug, thiserror::Error)]
pub enum ZeroCopyError {
    #[error("Buffer pool exhausted: {available} free buffers")]
    PoolExhausted { available: usize },

    #[error("Invalid buffer index: {index}")]
    InvalidIndex { index: usize },

    #[error("Buffer overflow: attempted {attempted}, capacity {capacity}")]
    BufferOverflow { attempted: usize, capacity: usize },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type ZcpResult<T> = Result<T, ZeroCopyError>;

/// Zero-copy message with Arc-based data sharing
#[derive(Debug, Clone)]
pub struct ZcpMessage {
    client_ip: IpAddr,
    is_valid_ip: bool,
    payload: Arc<Vec<u8>>,
    timestamp_ns: u64,
    payload_len: usize,
}

impl ZcpMessage {
    pub fn new(client_ip: &[u8], payload: Vec<u8>) -> Self {
        let (ip_addr, is_valid_ip) = std::str::from_utf8(client_ip)
            .ok()
            .and_then(|s| s.parse().ok())
            .map(|ip| (ip, true))
            .unwrap_or_else(|| {
                (
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                    false,
                )
            });

        let payload_len = payload.len();
        let payload_arc: Arc<Vec<u8>> = Arc::from(payload);
        Self {
            client_ip: ip_addr,
            is_valid_ip,
            payload: payload_arc,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            payload_len,
        }
    }

    pub fn from_ip_addr(client_ip: IpAddr, payload: Vec<u8>) -> Self {
        let payload_len = payload.len();
        let payload_arc: Arc<Vec<u8>> = Arc::from(payload);
        Self {
            client_ip,
            is_valid_ip: true,
            payload: payload_arc,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            payload_len,
        }
    }

    pub fn from_arc_payload(client_ip: IpAddr, payload: Arc<Vec<u8>>) -> Self {
        let payload_len = payload.len();
        Self {
            client_ip,
            is_valid_ip: true,
            payload,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            payload_len,
        }
    }

    pub fn client_ip(&self) -> IpAddr {
        self.client_ip
    }
    pub fn client_ip_bytes(&self) -> Vec<u8> {
        match self.client_ip {
            std::net::IpAddr::V4(a) => a.octets().to_vec(),
            std::net::IpAddr::V6(a) => a.octets().to_vec(),
        }
    }
    pub fn client_ip_str(&self) -> String {
        if self.is_valid_ip {
            self.client_ip.to_string()
        } else {
            "unknown".to_string()
        }
    }
    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }
    pub fn payload_len(&self) -> usize {
        self.payload_len
    }
    pub fn timestamp_ns(&self) -> u64 {
        self.timestamp_ns
    }
    pub fn is_empty(&self) -> bool {
        self.payload_len == 0
    }
    pub fn into_payload_arc(self) -> std::sync::Arc<Vec<u8>> {
        self.payload
    }
    pub fn clone_payload_arc(&self) -> std::sync::Arc<Vec<u8>> {
        std::sync::Arc::clone(&self.payload)
    }
}

/// Message batch type for efficient channel operations
pub type MessageBatch = Vec<ZcpMessage>;

/// Batch configuration for TCP message processing
#[derive(Debug, Clone, Copy)]
pub struct BatchConfig {
    /// Maximum batch size before forced sending
    pub max_batch_size: usize,
    /// Maximum time to wait before sending incomplete batch
    pub batch_timeout_ms: u64,
    /// Initial batch capacity (pre-allocated)
    pub batch_capacity: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 16,
            batch_timeout_ms: 1,
            batch_capacity: 32,
        }
    }
}

impl BatchConfig {
    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.max_batch_size = size;
        self
    }
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.batch_timeout_ms = timeout_ms;
        self
    }
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.batch_capacity = capacity;
        self
    }
}

/// Buffer pool metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct BufferPoolMetrics {
    pub total_buffers: usize,
    pub free_buffers: usize,
    pub used_buffers: usize,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub allocation_failures: u64,
    pub peak_usage: usize,
}

impl BufferPoolMetrics {
    pub fn usage_ratio(&self) -> f64 {
        if self.total_buffers == 0 {
            0.0
        } else {
            self.used_buffers as f64 / self.total_buffers as f64
        }
    }
    pub fn is_under_pressure(&self, threshold: f64) -> bool {
        self.usage_ratio() > threshold
    }
}

/// Buffer statistics for monitoring
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub read_position: usize,
    pub write_position: usize,
    pub buffer_in_use: bool,
    pub buffer_capacity: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zcp_message_creation() {
        let client_ip = b"192.168.1.100";
        let payload = b"hello world".to_vec();
        let expected: &[u8] = b"hello world";
        let msg = ZcpMessage::new(client_ip, payload);
        assert_eq!(msg.payload(), expected);
        assert_eq!(msg.payload_len(), expected.len());
        assert!(msg.timestamp_ns() > 0);
    }
}
