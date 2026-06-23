//! Syslog 源常量与类型别名（统一复用 tcp::framing 定义）

use wp_conf_base::ConfParser;
use wp_connector_api::Tags;

pub use crate::sources::tcp::framing::{
    DEFAULT_TCP_RECV_BYTES, Message, STOP_CHANNEL_CAPACITY, default_tcp_recv_bytes,
};

/// Default UDP receive buffer size (8 MB)
pub const DEFAULT_UDP_RECV_BUFFER: usize = 8 * 1024 * 1024;

pub fn default_udp_recv_buffer() -> usize {
    wp_conf::limits::udp_recv_buffer_bytes()
}

/// Extract tags from `Vec<String>` items (k:v / k=v / flag)
#[allow(dead_code)]
pub fn tags_from_vec(items: &[String]) -> Tags {
    Tags::from_parse(items)
}
