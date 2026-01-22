// src/applications/config/constants/status.rs
//! 状态标识相关常量
#![allow(dead_code)]

/// 成功状态标识
pub const STATUS_OK: &str = "OK";

/// 超出范围状态标识
pub const STATUS_EXCEEDED: &str = "Exceeded";

/// 超出范围（另一种表述）
pub const STATUS_OUT_OF_RANGE: &str = "Out of range";

/// 忽略状态标识
pub const STATUS_IGNORED: &str = "Ignored";

/// 计数状态标识
pub const STATUS_COUNTED: &str = "counted";

/// 统计状态标识
pub const STATUS_STATS: &str = "stats";

/// 验证错误状态前缀
pub const STATUS_ERROR_PREFIX: &str = "[100] validation error <<";
