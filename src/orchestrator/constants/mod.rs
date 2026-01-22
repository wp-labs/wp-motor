// src/applications/config/sinks/constants/mod.rs (moved into orchestrator constants)
//! Sink 配置系统常量定义
//!
//! 精简策略：默认不将子模块常量提升到本命名空间，避免滥用与未使用告警；
//! 外部请显式引用子模块（如 `orchestrator::constants::paths::PATH_SINK_SUBDIR`）。

pub mod connectors; // 连接器类型相关常量
pub mod fields; // 配置字段名相关常量
pub mod formats; // 数据格式相关常量
pub mod groups; // 组名相关常量
pub mod paths; // 路径和目录相关常量
pub mod status; // 状态标识相关常量
pub mod utils; // 常量相关工具函数

// 测试场景保留便捷 re-export，避免大范围测试改动
#[cfg(test)]
pub use connectors::*;
#[cfg(test)]
pub use fields::*;
#[cfg(test)]
pub use formats::*;
#[cfg(test)]
pub use groups::*;
#[cfg(test)]
pub use paths::*;
#[cfg(test)]
pub use status::*;
#[cfg(test)]
pub use utils::*;
