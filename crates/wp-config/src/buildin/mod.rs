//! 内置（builtin）连接器配置聚合：file、syslog。
//!
//! 目的：将通用内置 sink/source 的配置集中管理，便于演进；
//! 通过 `structure::io` 的 re-export 维持旧路径兼容。

pub mod file;
pub mod syslog;
pub use file::FileSinkConf;
pub use syslog::{SyslogSinkConf, SyslogSourceConf};
