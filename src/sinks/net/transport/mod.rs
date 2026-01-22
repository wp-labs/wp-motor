// transport module split for readability: config (constants/policy/toggles) + writer (NetWriter)

mod config;
mod writer;

// Re-exports to preserve the original public API surface used by sinks/backends
#[allow(unused_imports)]
pub use config::{
    BackoffMode, NET_SENDQ_BACKOFF_HIGH_PCT, NET_SENDQ_BACKOFF_SLEEP_MS, NetSendPolicy,
    get_global_rate_limit_rps, get_tcp_build_rate_limit_hint, net_backoff_adaptive,
    set_global_rate_limit_rps, set_net_backoff_adaptive, set_tcp_build_rate_limit_hint,
};
pub use writer::{NetWriter, Transport};
