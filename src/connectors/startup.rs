//! Centralized initialization for engine-side connector registries.
//! - Registers built-in sinks
//! - Registers built-in sources (syslog, tcp, file)
//! - Imports any factories that were (still) registered via API registries
//! - Logs the final registered kinds for diagnostics

use crate::connectors::registry as reg;

pub fn init_runtime_registries() {
    // 1) register built-in sinks (file/syslog/tcp/test_rescue/blackhole)
    crate::sinks::register_builtin_factories();

    // 2) register built-in sources
    // syslog factory
    crate::sources::syslog::register_syslog_factory();
    // tcp factory
    crate::sources::tcp::register_tcp_factory();
    // file factory explicit path
    crate::sources::file::register_factory_only();

    // 3) log final kinds
    log_registered_kinds();
}

pub fn log_registered_kinds() {
    let sinks = reg::sink_diagnostics();
    if sinks.is_empty() {
        log::warn!("no sinks registered");
    } else {
        for (k, loc) in sinks {
            log::info!("sink kind='{}' at {}:{}", k, loc.file(), loc.line());
        }
    }
    let srcs = reg::source_diagnostics();
    if srcs.is_empty() {
        log::warn!("no sources registered");
    } else {
        for (k, loc) in srcs {
            log::info!("source kind='{}' at {}:{}", k, loc.file(), loc.line());
        }
    }
}
