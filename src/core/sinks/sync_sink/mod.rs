//! SyncSink 模块 - 处理同步数据发送的 trait 和实现

pub mod channel_impl;
pub mod impls;
pub mod terminal_impl;
pub mod traits;
pub mod types;

// 重新导出主要类型
pub use crate::sinks::BlackHoleSink;
pub use traits::{RecSyncSink, TrySendStatus};
pub use types::SinkTerminal;

// 为外部兼容性，重新导出需要的类型
pub use crate::sinks::SinkDatYSender;
