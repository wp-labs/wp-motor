//! Centralized memory-related limits and queue capacities for runtime components.
//!
//! Defaults are selected by `WP_MEMORY_PROFILE`:
//! - `low`: smaller buffers for bounded memory use
//! - `standard`/unset: balanced production profile
//! - `throughput`: larger parser/sink channels for complex samples or fast sinks
//!
//! Individual values can be overridden with the `WP_*` environment variables below.

use std::{str::FromStr, sync::OnceLock};

const ENV_MEMORY_PROFILE: &str = "WP_MEMORY_PROFILE";
const ENV_PARSER_CHANNEL_CAP: &str = "WP_PARSER_CHANNEL_CAP";
const ENV_SINK_CHANNEL_CAP: &str = "WP_SINK_CHANNEL_CAP";
const ENV_SINK_BATCH_SIZE: &str = "WP_SINK_BATCH_SIZE";
const ENV_PARSE_WORKERS: &str = "WP_PARSE_WORKERS";
const ENV_PARSE_WORKERS_MAX: &str = "WP_PARSE_WORKERS_MAX";
const ENV_PICKER_BURST_MAX: &str = "WP_PICKER_BURST_MAX";
const ENV_PICKER_COALESCE_TRIGGER: &str = "WP_PICKER_COALESCE_TRIGGER";
const ENV_PICKER_COALESCE_MAX_EVENTS: &str = "WP_PICKER_COALESCE_MAX_EVENTS";
const ENV_TCP_RECV_BYTES: &str = "WP_TCP_RECV_BYTES";
const ENV_TCP_BATCH_CAPACITY: &str = "WP_TCP_BATCH_CAPACITY";
const ENV_TCP_BATCH_BYTES: &str = "WP_TCP_BATCH_BYTES";
const ENV_TCP_SHRINK_HIGH_WATER_BYTES: &str = "WP_TCP_SHRINK_HIGH_WATER_BYTES";
const ENV_TCP_SHRINK_TARGET_BYTES: &str = "WP_TCP_SHRINK_TARGET_BYTES";
const ENV_UDP_RECV_BUFFER_BYTES: &str = "WP_UDP_RECV_BUFFER_BYTES";
const ENV_UDP_BATCH_SIZE: &str = "WP_UDP_BATCH_SIZE";
const ENV_FILE_BATCH_LINES: &str = "WP_FILE_BATCH_LINES";
const ENV_FILE_BATCH_BYTES: &str = "WP_FILE_BATCH_BYTES";
const ENV_FILE_CHUNK_BYTES: &str = "WP_FILE_CHUNK_BYTES";
const ENV_SINK_POOL_MAX: &str = "WP_SINK_POOL_MAX";
const ENV_SINK_POOL_UNIT_MAX_CAP: &str = "WP_SINK_POOL_UNIT_MAX_CAP";
const ENV_SINK_POOL_UNIT_INIT_CAP: &str = "WP_SINK_POOL_UNIT_INIT_CAP";
const ENV_FIELD_QUERY_CACHE_CAP: &str = "WP_FIELD_QUERY_CACHE_CAP";
const ENV_PICKER_PENDING_MAX_BYTES: &str = "WP_PICKER_PENDING_MAX_BYTES";
const ENV_DEBUG_VIEW_CHANNEL_CAP: &str = "WP_DEBUG_VIEW_CHANNEL_CAP";
const ENV_DEBUG_VIEW_BATCH_LINES: &str = "WP_DEBUG_VIEW_BATCH_LINES";
const ENV_CMD_CHANNEL_CAP: &str = "WP_CMD_CHANNEL_CAP";

const KIB: usize = 1024;
const MIB: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProfileKind {
    Low,
    Standard,
    Throughput,
}

impl FromStr for MemoryProfileKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" | "low_mem" | "low-mem" | "small" | "tiny" | "xs" => Ok(Self::Low),
            "" | "standard" | "default" | "normal" | "balanced" => Ok(Self::Standard),
            "throughput" | "high" | "large" => Ok(Self::Throughput),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLimits {
    pub parser_channel_cap: usize,
    pub sink_channel_cap: usize,
    pub sink_batch_size: usize,
    pub parse_workers: usize,
    pub parse_workers_max: usize,
    pub picker_burst_max: usize,
    pub picker_coalesce_trigger: usize,
    pub picker_coalesce_max_events: usize,
    pub tcp_recv_bytes: usize,
    pub tcp_batch_capacity: usize,
    pub tcp_batch_bytes: usize,
    pub tcp_shrink_high_water_bytes: usize,
    pub tcp_shrink_target_bytes: usize,
    pub udp_recv_buffer_bytes: usize,
    pub udp_batch_size: usize,
    pub file_batch_lines: usize,
    pub file_batch_bytes: usize,
    pub file_chunk_bytes: usize,
    pub sink_pool_max: usize,
    pub sink_pool_unit_max_cap: usize,
    pub sink_pool_unit_init_cap: usize,
    pub field_query_cache_cap: usize,
    pub picker_pending_max_bytes: usize,
    pub debug_view_channel_cap: usize,
    pub debug_view_batch_lines: usize,
    pub cmd_channel_cap: usize,
}

impl MemoryLimits {
    pub const fn for_profile(kind: MemoryProfileKind) -> Self {
        match kind {
            MemoryProfileKind::Low => Self {
                parser_channel_cap: 32,
                sink_channel_cap: 16,
                sink_batch_size: 256,
                parse_workers: 2,
                parse_workers_max: usize::MAX,
                picker_burst_max: 4,
                picker_coalesce_trigger: 8,
                picker_coalesce_max_events: 32,
                tcp_recv_bytes: 512 * KIB,
                tcp_batch_capacity: 32,
                tcp_batch_bytes: 32 * KIB,
                tcp_shrink_high_water_bytes: 256 * KIB,
                tcp_shrink_target_bytes: 64 * KIB,
                udp_recv_buffer_bytes: MIB,
                udp_batch_size: 32,
                file_batch_lines: 32,
                file_batch_bytes: 64 * KIB,
                file_chunk_bytes: 16 * KIB,
                sink_pool_max: 16,
                sink_pool_unit_max_cap: 512,
                sink_pool_unit_init_cap: 16,
                field_query_cache_cap: 128,
                picker_pending_max_bytes: MIB,
                debug_view_channel_cap: 256,
                debug_view_batch_lines: 32,
                cmd_channel_cap: 1024,
            },
            MemoryProfileKind::Standard => Self {
                parser_channel_cap: 48,
                sink_channel_cap: 24,
                sink_batch_size: 512,
                parse_workers: 2,
                parse_workers_max: usize::MAX,
                picker_burst_max: 6,
                picker_coalesce_trigger: 24,
                picker_coalesce_max_events: 96,
                tcp_recv_bytes: 2 * MIB,
                tcp_batch_capacity: 256,
                tcp_batch_bytes: 256 * KIB,
                tcp_shrink_high_water_bytes: 512 * KIB,
                tcp_shrink_target_bytes: 128 * KIB,
                udp_recv_buffer_bytes: 2 * MIB,
                udp_batch_size: 64,
                file_batch_lines: 96,
                file_batch_bytes: 256 * KIB,
                file_chunk_bytes: 64 * KIB,
                sink_pool_max: 64,
                sink_pool_unit_max_cap: 2048,
                sink_pool_unit_init_cap: 32,
                field_query_cache_cap: 512,
                picker_pending_max_bytes: 2 * MIB,
                debug_view_channel_cap: 1024,
                debug_view_batch_lines: 100,
                cmd_channel_cap: 4096,
            },
            MemoryProfileKind::Throughput => Self {
                parser_channel_cap: 96,
                sink_channel_cap: 48,
                sink_batch_size: 512,
                parse_workers: 2,
                parse_workers_max: usize::MAX,
                picker_burst_max: 6,
                picker_coalesce_trigger: 48,
                picker_coalesce_max_events: 192,
                tcp_recv_bytes: 2 * MIB,
                tcp_batch_capacity: 128,
                tcp_batch_bytes: 64 * KIB,
                tcp_shrink_high_water_bytes: MIB,
                tcp_shrink_target_bytes: 256 * KIB,
                udp_recv_buffer_bytes: 8 * MIB,
                udp_batch_size: 128,
                file_batch_lines: 128,
                file_batch_bytes: 400 * KIB,
                file_chunk_bytes: 64 * KIB,
                sink_pool_max: 96,
                sink_pool_unit_max_cap: 4096,
                sink_pool_unit_init_cap: 64,
                field_query_cache_cap: 1000,
                picker_pending_max_bytes: 2 * MIB,
                debug_view_channel_cap: 2048,
                debug_view_batch_lines: 100,
                cmd_channel_cap: 8192,
            },
        }
    }

    pub fn from_env() -> Self {
        let kind = std::env::var(ENV_MEMORY_PROFILE)
            .ok()
            .and_then(|value| value.parse::<MemoryProfileKind>().ok())
            .unwrap_or(MemoryProfileKind::Standard);
        let mut limits = Self::for_profile(kind);
        limits.parser_channel_cap =
            env_usize(ENV_PARSER_CHANNEL_CAP).unwrap_or(limits.parser_channel_cap);
        limits.sink_channel_cap =
            env_usize(ENV_SINK_CHANNEL_CAP).unwrap_or(limits.sink_channel_cap);
        limits.sink_batch_size = env_usize(ENV_SINK_BATCH_SIZE).unwrap_or(limits.sink_batch_size);
        limits.parse_workers = env_usize(ENV_PARSE_WORKERS).unwrap_or(limits.parse_workers);
        limits.parse_workers_max =
            env_usize(ENV_PARSE_WORKERS_MAX).unwrap_or(limits.parse_workers_max);
        limits.picker_burst_max =
            env_usize(ENV_PICKER_BURST_MAX).unwrap_or(limits.picker_burst_max);
        limits.picker_coalesce_trigger =
            env_usize(ENV_PICKER_COALESCE_TRIGGER).unwrap_or(limits.picker_coalesce_trigger);
        limits.picker_coalesce_max_events =
            env_usize(ENV_PICKER_COALESCE_MAX_EVENTS).unwrap_or(limits.picker_coalesce_max_events);
        limits.tcp_recv_bytes = env_usize(ENV_TCP_RECV_BYTES).unwrap_or(limits.tcp_recv_bytes);
        limits.tcp_batch_capacity =
            env_usize(ENV_TCP_BATCH_CAPACITY).unwrap_or(limits.tcp_batch_capacity);
        limits.tcp_batch_bytes = env_usize(ENV_TCP_BATCH_BYTES).unwrap_or(limits.tcp_batch_bytes);
        limits.tcp_shrink_high_water_bytes = env_usize(ENV_TCP_SHRINK_HIGH_WATER_BYTES)
            .unwrap_or(limits.tcp_shrink_high_water_bytes);
        limits.tcp_shrink_target_bytes =
            env_usize(ENV_TCP_SHRINK_TARGET_BYTES).unwrap_or(limits.tcp_shrink_target_bytes);
        limits.udp_recv_buffer_bytes =
            env_usize(ENV_UDP_RECV_BUFFER_BYTES).unwrap_or(limits.udp_recv_buffer_bytes);
        limits.udp_batch_size = env_usize(ENV_UDP_BATCH_SIZE).unwrap_or(limits.udp_batch_size);
        limits.file_batch_lines =
            env_usize(ENV_FILE_BATCH_LINES).unwrap_or(limits.file_batch_lines);
        limits.file_batch_bytes =
            env_usize(ENV_FILE_BATCH_BYTES).unwrap_or(limits.file_batch_bytes);
        limits.file_chunk_bytes =
            env_usize(ENV_FILE_CHUNK_BYTES).unwrap_or(limits.file_chunk_bytes);
        limits.sink_pool_max = env_usize(ENV_SINK_POOL_MAX).unwrap_or(limits.sink_pool_max);
        limits.sink_pool_unit_max_cap =
            env_usize(ENV_SINK_POOL_UNIT_MAX_CAP).unwrap_or(limits.sink_pool_unit_max_cap);
        limits.sink_pool_unit_init_cap =
            env_usize(ENV_SINK_POOL_UNIT_INIT_CAP).unwrap_or(limits.sink_pool_unit_init_cap);
        limits.field_query_cache_cap =
            env_usize(ENV_FIELD_QUERY_CACHE_CAP).unwrap_or(limits.field_query_cache_cap);
        limits.picker_pending_max_bytes =
            env_usize(ENV_PICKER_PENDING_MAX_BYTES).unwrap_or(limits.picker_pending_max_bytes);
        limits.debug_view_channel_cap =
            env_usize(ENV_DEBUG_VIEW_CHANNEL_CAP).unwrap_or(limits.debug_view_channel_cap);
        limits.debug_view_batch_lines =
            env_usize(ENV_DEBUG_VIEW_BATCH_LINES).unwrap_or(limits.debug_view_batch_lines);
        limits.cmd_channel_cap = env_usize(ENV_CMD_CHANNEL_CAP).unwrap_or(limits.cmd_channel_cap);
        limits.normalize()
    }

    fn normalize(mut self) -> Self {
        self.parser_channel_cap = self.parser_channel_cap.max(1);
        self.sink_channel_cap = self.sink_channel_cap.max(1);
        self.sink_batch_size = self.sink_batch_size.max(1);
        self.parse_workers = self.parse_workers.max(1);
        self.parse_workers_max = self.parse_workers_max.max(1);
        self.picker_burst_max = self.picker_burst_max.max(1);
        self.picker_coalesce_trigger = self.picker_coalesce_trigger.max(1);
        self.picker_coalesce_max_events = self.picker_coalesce_max_events.max(1);
        self.tcp_recv_bytes = self.tcp_recv_bytes.clamp(KIB, 64 * MIB);
        self.tcp_batch_capacity = self.tcp_batch_capacity.max(1);
        self.tcp_batch_bytes = self.tcp_batch_bytes.max(KIB);
        self.tcp_shrink_target_bytes = self.tcp_shrink_target_bytes.clamp(KIB, self.tcp_recv_bytes);
        self.tcp_shrink_high_water_bytes = self
            .tcp_shrink_high_water_bytes
            .max(self.tcp_shrink_target_bytes);
        self.udp_recv_buffer_bytes = self.udp_recv_buffer_bytes.max(KIB);
        self.udp_batch_size = self.udp_batch_size.max(1);
        self.file_batch_lines = self.file_batch_lines.max(1);
        self.file_batch_bytes = self.file_batch_bytes.max(KIB);
        self.file_chunk_bytes = self.file_chunk_bytes.clamp(4 * KIB, 4 * MIB);
        self.sink_pool_unit_init_cap = self
            .sink_pool_unit_init_cap
            .max(1)
            .min(self.sink_pool_unit_max_cap.max(1));
        self.sink_pool_unit_max_cap = self
            .sink_pool_unit_max_cap
            .max(self.sink_pool_unit_init_cap);
        self.field_query_cache_cap = self.field_query_cache_cap.max(1);
        self.picker_pending_max_bytes = self.picker_pending_max_bytes.max(KIB);
        self.debug_view_channel_cap = self.debug_view_channel_cap.max(1);
        self.debug_view_batch_lines = self.debug_view_batch_lines.max(1);
        self.cmd_channel_cap = self.cmd_channel_cap.max(1);
        self
    }
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

pub fn memory_limits() -> MemoryLimits {
    static LIMITS: OnceLock<MemoryLimits> = OnceLock::new();
    *LIMITS.get_or_init(MemoryLimits::from_env)
}

/// Parser input channel capacity (per parser worker)
/// Lower values 减少峰值内存并更早施加背压。
pub const PARSER_CHANNEL_CAP_DEFAULT: usize = 48;

/// 获取当前 parser 通道容量。
pub fn parser_channel_cap() -> usize {
    memory_limits().parser_channel_cap
}

/// Sink sync channel capacity (per sink group dispatcher)。
pub const SINK_CHANNEL_CAP_DEFAULT: usize = 24;

/// 获取当前 sink 通道容量。
pub fn sink_channel_cap() -> usize {
    memory_limits().sink_channel_cap
}

pub fn sink_batch_size() -> usize {
    memory_limits().sink_batch_size
}

pub fn parse_workers() -> usize {
    memory_limits().parse_workers
}

pub fn parse_workers_max() -> usize {
    memory_limits().parse_workers_max
}

pub fn clamp_parse_workers(workers: usize) -> usize {
    workers.max(1).min(parse_workers_max())
}

pub fn picker_burst_max() -> usize {
    memory_limits().picker_burst_max
}

pub fn picker_coalesce_trigger() -> usize {
    memory_limits().picker_coalesce_trigger
}

pub fn picker_coalesce_max_events() -> usize {
    memory_limits().picker_coalesce_max_events
}

pub fn tcp_recv_bytes() -> usize {
    memory_limits().tcp_recv_bytes
}

pub fn tcp_batch_capacity() -> usize {
    memory_limits().tcp_batch_capacity
}

pub fn tcp_batch_bytes() -> usize {
    memory_limits().tcp_batch_bytes
}

pub fn tcp_shrink_high_water_bytes() -> usize {
    memory_limits().tcp_shrink_high_water_bytes
}

pub fn tcp_shrink_target_bytes() -> usize {
    memory_limits().tcp_shrink_target_bytes
}

pub fn udp_recv_buffer_bytes() -> usize {
    memory_limits().udp_recv_buffer_bytes
}

pub fn udp_batch_size() -> usize {
    memory_limits().udp_batch_size
}

pub fn file_batch_lines() -> usize {
    memory_limits().file_batch_lines
}

pub fn file_batch_bytes() -> usize {
    memory_limits().file_batch_bytes
}

pub fn file_chunk_bytes() -> usize {
    memory_limits().file_chunk_bytes
}

pub fn sink_pool_max() -> usize {
    memory_limits().sink_pool_max
}

pub fn sink_pool_unit_max_cap() -> usize {
    memory_limits().sink_pool_unit_max_cap
}

pub fn sink_pool_unit_init_cap() -> usize {
    memory_limits().sink_pool_unit_init_cap
}

pub fn field_query_cache_cap() -> usize {
    memory_limits().field_query_cache_cap
}

pub fn picker_pending_max_bytes() -> usize {
    memory_limits().picker_pending_max_bytes
}

pub fn debug_view_channel_cap() -> usize {
    memory_limits().debug_view_channel_cap
}

pub fn debug_view_batch_lines() -> usize {
    memory_limits().debug_view_batch_lines
}

pub fn cmd_channel_cap() -> usize {
    memory_limits().cmd_channel_cap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_profile_presets_are_ordered() {
        let low = MemoryLimits::for_profile(MemoryProfileKind::Low);
        let standard = MemoryLimits::for_profile(MemoryProfileKind::Standard);
        let throughput = MemoryLimits::for_profile(MemoryProfileKind::Throughput);
        assert!(low.parser_channel_cap < standard.parser_channel_cap);
        assert!(standard.parser_channel_cap < throughput.parser_channel_cap);
        assert!(low.sink_channel_cap < standard.sink_channel_cap);
        assert!(standard.sink_channel_cap < throughput.sink_channel_cap);
        assert!(low.tcp_recv_bytes < standard.tcp_recv_bytes);
        assert_eq!(standard.tcp_recv_bytes, throughput.tcp_recv_bytes);
        assert_eq!(standard.tcp_batch_bytes, 256 * KIB);
        assert_eq!(standard.tcp_batch_capacity, 256);
        assert_eq!(standard.sink_batch_size, 512);
        assert_eq!(standard.picker_burst_max, 6);
        assert_eq!(standard.picker_pending_max_bytes, 2 * MIB);
    }

    #[test]
    fn memory_profile_unset_defaults_to_standard() {
        let standard = MemoryLimits::for_profile(MemoryProfileKind::Standard);
        let parsed = std::env::var(ENV_MEMORY_PROFILE)
            .ok()
            .and_then(|value| value.parse::<MemoryProfileKind>().ok())
            .unwrap_or(MemoryProfileKind::Standard);

        if std::env::var(ENV_MEMORY_PROFILE).is_ok() {
            return;
        }

        assert_eq!(parsed, MemoryProfileKind::Standard);
        assert_eq!(MemoryLimits::for_profile(parsed), standard);
    }

    #[test]
    fn memory_profile_names_parse() {
        assert_eq!(
            "low".parse::<MemoryProfileKind>(),
            Ok(MemoryProfileKind::Low)
        );
        assert_eq!(
            "small".parse::<MemoryProfileKind>(),
            Ok(MemoryProfileKind::Low)
        );
        assert_eq!(
            "balanced".parse::<MemoryProfileKind>(),
            Ok(MemoryProfileKind::Standard)
        );
        assert_eq!(
            "throughput".parse::<MemoryProfileKind>(),
            Ok(MemoryProfileKind::Throughput)
        );
        assert!("unknown".parse::<MemoryProfileKind>().is_err());
    }
}
