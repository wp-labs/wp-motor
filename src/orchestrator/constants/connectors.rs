// src/applications/config/constants/connectors.rs
//! 连接器类型相关常量
#![allow(dead_code)]

/// 文件类型连接器标识
pub const CONNECTOR_TYPE_FILE: &str = "file";

/// 测试救援类型连接器标识
pub const CONNECTOR_TYPE_TEST_RESCUE: &str = "test_rescue";

/// Kafka类型连接器标识
pub const CONNECTOR_TYPE_KAFKA: &str = "kafka";

/// 支持的连接器类型列表
pub const SUPPORTED_CONNECTOR_TYPES: &[&str] = &[
    CONNECTOR_TYPE_FILE,
    CONNECTOR_TYPE_TEST_RESCUE,
    CONNECTOR_TYPE_KAFKA,
];

/// 文件类型连接器列表（用于格式决策）
pub const FILE_CONNECTOR_TYPES: &[&str] = &[CONNECTOR_TYPE_FILE, CONNECTOR_TYPE_TEST_RESCUE];
