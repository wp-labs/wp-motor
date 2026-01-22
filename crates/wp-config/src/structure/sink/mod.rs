pub mod expect;
pub mod instance;
pub mod route;
// tags 校验改为统一使用 wp_model_core::tags::validate_tags；移除本地 utils

pub use expect::SinkExpectOverride;
pub use instance::SinkInstanceConf;
pub use route::SinkRouteConf;
