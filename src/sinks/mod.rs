#![allow(unexpected_cfgs)]
mod backends;
mod decorators;
mod net;
mod pdm_outer;
mod prelude;
mod rescue;
mod routing;
mod runtime;
mod sink_build;
#[cfg(test)]
mod test_helpers;
mod testunit;
pub mod types;
mod utils;

// Keep public only the items required by external apps/tests; rest are crate-internal
pub(crate) use backends::file::FileSink;
pub use backends::file::create_watch_out; // tests rely on this helper
pub(crate) use decorators::test_proxy::ASinkTestProxy;
pub(crate) use decorators::test_proxy::HealthController;
pub(crate) use rescue::RescueFileSink;
pub use rescue::{RescueEntry, RescuePayload};
pub use routing::agent::InfraSinkAgent; // used by apps/tests
pub(crate) use routing::agent::SinkGroupAgent;
pub(crate) use routing::dispatcher::SinkDispatcher;
#[cfg(any(test, feature = "perf-ci"))]
pub use routing::dispatcher::perf::OmlBatchPerfCase;
pub use routing::registry::SinkRegistry; // used by apps/tests
pub use routing::registry::SinkRouteAgent; // used by tests
pub(crate) use runtime::manager::SinkRuntime;
pub use sink_build::build_file_sink;
pub use types::*; // SinkBackendType, SinkEndpoint (used by apps/tests)
pub use utils::buffer_monitor::BufferMonitor; // used by tests
pub use utils::formatter::FormatAdapter; // used by tests
pub use utils::view::DebugViewer; // used by apps
pub use utils::view::ViewOuter; // used by apps

// Note: registration of external sinks has been moved to apps/* to avoid
// feature-coupling the core library with extension crates.

// Built-in factories (null/file/test_rescue) are always available
pub(crate) mod builtin_factories;
pub use builtin_factories::register_builtin_factories;
// Backward-compat alias for older tests/tools
pub use builtin_factories::register_builtin_factories as register_builtin_sinks;
// Expose a simple null sink for benches and external tests
pub use backends::blackhole::BlackHoleSink;
pub use backends::blackhole_factory::BlackHoleFactory;
pub use backends::file_factory::FileFactory;
pub use backends::syslog::SyslogFactory;
pub use backends::syslog::register_factory_syslog;
pub use backends::test_rescue::TestRescueFactory;
pub use builtin_factories::make_blackhole_sink;
// Controlled network backoff (adaptive toggle) and build-time rate-limit hint APIs
pub use net::transport::{
    get_global_rate_limit_rps, get_tcp_build_rate_limit_hint, net_backoff_adaptive,
    set_global_rate_limit_rps, set_net_backoff_adaptive, set_tcp_build_rate_limit_hint,
};
