#[macro_use]
extern crate log as _;
#[macro_use]
extern crate serde;

extern crate winnow;
extern crate wp_knowledge as wp_know;

pub use winnow::*;

/// 开发期适配器统一注册入口（MySQL/Kafka）
/// 内置（file/syslog）配置聚合
mod buildin;
mod common;
mod cond;
pub mod connectors;
pub mod constants;
pub mod error;
pub mod limits;
pub mod loader;
//pub mod oml;
//pub mod pdm;
pub mod engine;
pub mod generator;
pub mod run_args;

// 向后兼容性：重新导出旧的类型名称
#[deprecated(note = "Use RuntimeMode instead")]
pub use run_args::RuntimeMode as RunMode;

#[deprecated(note = "Use RuntimeArgs instead")]
pub use run_args::RuntimeArgs as RunArgs;

#[deprecated(note = "Use RuntimeArgsFrom instead")]
pub use run_args::RuntimeArgsFrom as RunArgsFrom;

// 向后兼容性：重新导出旧的类型名称
#[deprecated(note = "Use StringOrArray instead")]
pub type StrOrVec = StringOrArray;

#[deprecated(note = "Use WarpConditionParser instead")]
pub use cond::WarpConditionParser as TCondParser;
pub mod sinks;
pub mod sources;
pub mod stat;
pub mod structure;
pub mod test_support;
mod types;
pub mod utils;

// 便于外部复用：核心配置结构快速重导出
//pub use buildin::{OutFile, Syslog};
pub use sinks::{ConnectorRec, DefaultsBody, RouteFile, RouteGroup, RouteSink, StringOrArray};

pub use common::io_locate::find_connectors_base_dir;
pub use common::paths;
pub use cond::WarpConditionParser;
pub use types::SafeDefault;

// 重新导出 WildArray 类型以供外部使用
pub use wp_specs::WildArray;

// 生成器速度模型配置
pub use generator::{CombineModeConfig, SpeedProfileConfig};
