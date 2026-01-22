// src/applications/config/constants/formats.rs
//! 数据格式相关常量
#![allow(dead_code)]

/// JSON 格式标识
pub const FORMAT_JSON: &str = "json";

/// 键值对格式标识
pub const FORMAT_KV: &str = "kv";

/// 原始文本格式标识
pub const FORMAT_RAW: &str = "raw";

/// 默认输出格式（用于文件类连接器）
pub const DEFAULT_OUTPUT_FORMAT: &str = FORMAT_JSON;

/// 支持的格式列表
pub const SUPPORTED_FORMATS: &[&str] = &[FORMAT_JSON, FORMAT_KV, FORMAT_RAW];
