//! Facade: 配置/模型对外入口（稳定 re-export）。

pub use crate::orchestrator::config::loader::WarpConf;
pub use crate::orchestrator::config::models::{load_warp_engine_confs, stat_reqs_from};
pub use wp_conf::constants::{
    ENGINE_CONF_FILE, KNOWDB_TOML, SINK_FWK_TOML, SOURCE_WPL, WPARSE_CONF, WPARSE_LOG_PATH,
    WPARSE_OML_FILE, WPARSE_RULE_FILE, WPGEN_TOML, WPSRC_TOML,
};
pub use wp_conf::engine::EngineConfig;
pub use wp_conf::generator::wpgen::{WpGenConfig, WpGenResolved};
pub use wp_conf::loader::ConfDelegate;
// Sink helpers exposed for apps (wpgen/wproj), masking orchestrator internals
pub use crate::orchestrator::config::build_sinks::{
    CMD_CHANNEL_MAX, SinkRouteTable, build_sink_target, dat_channel_max, fmt_file,
};
