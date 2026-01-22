//! Centralized limits and queue capacities for runtime components.
//! Adjust these carefully based on throughput and backpressure behavior.

/// Parser input channel capacity (per parser worker)
/// Lower values 减少峰值内存并更早施加背压。
pub const PARSER_CHANNEL_CAP_DEFAULT: usize = 512; // increased to ease parser backpressure

/// 获取当前 parser 通道容量。
pub fn parser_channel_cap() -> usize {
    PARSER_CHANNEL_CAP_DEFAULT
}

/// Sink sync channel capacity (per sink group dispatcher)。
pub const SINK_CHANNEL_CAP_DEFAULT: usize = 128;

/// 获取当前 sink 通道容量。
pub fn sink_channel_cap() -> usize {
    SINK_CHANNEL_CAP_DEFAULT
}

/// 单连接 reader 的批次队列容量。
pub const TCP_READER_BATCH_CAP_DEFAULT: usize = 64;

pub fn tcp_reader_batch_channel_cap() -> usize {
    TCP_READER_BATCH_CAP_DEFAULT
}
