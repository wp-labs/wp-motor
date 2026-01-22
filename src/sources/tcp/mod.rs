//! TCP 源实现 - 高性能分离式架构
//!
//! 模块结构：
//! - framing.rs：TCP消息分帧处理（行/长度前缀/自动检测）
//! - source.rs：TcpSource 实现，直接管理监听 loop 交付的 sockets
//! - factory.rs：参数解析与 TcpSourceFactory
//! - zc_types.rs：零拷贝数据结构

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub mod acceptor;
mod config;
pub mod conn;
pub mod factory;
pub mod framing;
pub mod source;
pub mod worker;
pub mod zc;
pub use acceptor::TcpAcceptor;
pub use factory::{TcpSourceFactory, register_tcp_factory};
pub use framing::FramingMode;
pub use source::TcpSource;

// 零拷贝类型导出
pub use zc::types::{
    BatchConfig, BufferPoolMetrics, BufferStats, MessageBatch, ZcpConfig, ZcpMessage, ZcpResult,
    ZeroCopyError,
};

pub(crate) type ConnectionRegistry = Arc<Mutex<HashSet<u64>>>;
