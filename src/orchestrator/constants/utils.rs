// src/applications/config/constants/utils.rs
//! 常量相关工具函数
#![allow(dead_code)]

use crate::orchestrator::constants as modules;

/// 检查字符串是否为支持的连接器类型
pub fn is_supported_connector_type(conn_type: &str) -> bool {
    modules::connectors::SUPPORTED_CONNECTOR_TYPES.contains(&conn_type)
}

/// 检查字符串是否为基础设施组名
pub fn is_infra_group_name(group_name: &str) -> bool {
    // 统一调用配置层实现
    wp_conf::sinks::is_infra_group_name(group_name)
}

/// 检查字符串是否为业务组名（排除基础设施组）
pub fn is_business_group_name(group_name: &str) -> bool {
    !is_infra_group_name(group_name)
}

/// 检查字段名是否在嵌套黑名单中
pub fn is_nested_field_blacklisted(field_name: &str) -> bool {
    modules::fields::NESTED_FIELD_BLACKLIST.contains(&field_name)
}

/// 获取连接字段的主名称（优先级：connect > use > connector）
pub fn get_primary_connect_field() -> &'static str {
    modules::fields::FIELD_CONNECT
}

/// 检查格式是否受支持
pub fn is_supported_format(format_str: &str) -> bool {
    modules::formats::SUPPORTED_FORMATS.contains(&format_str)
}

/// 获取组名的显示友好名称
pub fn get_group_display_name(group_name: &str) -> &'static str {
    match group_name {
        modules::groups::GROUP_MONITOR => "Monitor",
        modules::groups::GROUP_DEFAULT => "Default",
        modules::groups::GROUP_MISS => "Miss",
        modules::groups::GROUP_RESIDUE => "Residue",

        modules::groups::GROUP_ERROR => "Error",
        _ => "Unknown",
    }
}

/// 获取连接器类型的显示友好名称
pub fn get_connector_type_display_name(conn_type: &str) -> &'static str {
    match conn_type {
        modules::connectors::CONNECTOR_TYPE_FILE => "File",
        modules::connectors::CONNECTOR_TYPE_TEST_RESCUE => "Test Rescue",
        modules::connectors::CONNECTOR_TYPE_KAFKA => "Kafka",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::constants::{
        connectors::*, fields::*, formats::*, groups::*, paths::*, status::*,
    };

    #[test]
    fn test_supported_connector_types() {
        assert!(is_supported_connector_type(CONNECTOR_TYPE_FILE));
        assert!(is_supported_connector_type(CONNECTOR_TYPE_TEST_RESCUE));
        assert!(is_supported_connector_type(CONNECTOR_TYPE_KAFKA));
        assert!(!is_supported_connector_type("unknown_type"));
    }

    #[test]
    fn test_infra_group_names() {
        assert!(is_infra_group_name(GROUP_MONITOR));
        assert!(is_infra_group_name(GROUP_DEFAULT));
        assert!(is_infra_group_name(GROUP_MISS));
        assert!(is_infra_group_name(GROUP_RESIDUE));

        assert!(is_infra_group_name(GROUP_ERROR));
        assert!(!is_infra_group_name("business_group"));
    }

    #[test]
    fn test_business_group_names() {
        assert!(is_business_group_name("business_group"));
        assert!(!is_business_group_name(GROUP_MONITOR));
    }

    #[test]
    fn test_nested_field_blacklist() {
        assert!(is_nested_field_blacklisted(FIELD_PARAMS));
        assert!(is_nested_field_blacklisted(FIELD_PARAMS_OVERRIDE));
        assert!(!is_nested_field_blacklisted(FIELD_TAGS));
        assert!(!is_nested_field_blacklisted(FIELD_NAME));
    }

    #[test]
    fn test_supported_formats() {
        assert!(is_supported_format(FORMAT_JSON));
        assert!(is_supported_format(FORMAT_KV));
        assert!(is_supported_format(FORMAT_RAW));
        assert!(!is_supported_format("unknown_format"));
    }

    #[test]
    fn test_display_names() {
        assert_eq!(get_group_display_name(GROUP_MONITOR), "Monitor");
        assert_eq!(get_group_display_name(GROUP_DEFAULT), "Default");
        assert_eq!(get_group_display_name(GROUP_MISS), "Miss");
        assert_eq!(get_group_display_name(GROUP_RESIDUE), "Residue");

        assert_eq!(get_group_display_name(GROUP_ERROR), "Error");
        assert_eq!(get_group_display_name("unknown"), "Unknown");

        assert_eq!(get_connector_type_display_name(CONNECTOR_TYPE_FILE), "File");
        assert_eq!(
            get_connector_type_display_name(CONNECTOR_TYPE_TEST_RESCUE),
            "Test Rescue"
        );
        assert_eq!(
            get_connector_type_display_name(CONNECTOR_TYPE_KAFKA),
            "Kafka"
        );
        assert_eq!(get_connector_type_display_name("unknown"), "Unknown");
    }

    #[test]
    fn test_primary_connect_field() {
        assert_eq!(get_primary_connect_field(), FIELD_CONNECT);
    }
}
