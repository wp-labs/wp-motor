// src/applications/config/constants/tests.rs
//! 常量模块测试用例

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_constants() {
        assert_eq!(PATH_CONNECTORS_DIR, "connectors");
        assert_eq!(PATH_SINK_SUBDIR, "sink.d");
        assert_eq!(PATH_BUSINESS_SUBDIR, "business.d");
        assert_eq!(PATH_INFRA_SUBDIR, "infra.d");
        assert_eq!(PATH_DEFAULTS_FILE, "defaults.toml");
        assert_eq!(FILE_EXT_TOML, "toml");
    }

    #[test]
    fn test_connector_type_constants() {
        assert_eq!(CONNECTOR_TYPE_FILE, "file");
        assert_eq!(CONNECTOR_TYPE_TEST_RESCUE, "test_rescue");
        assert_eq!(CONNECTOR_TYPE_KAFKA, "kafka");

        assert!(is_supported_connector_type(CONNECTOR_TYPE_FILE));
        assert!(is_supported_connector_type(CONNECTOR_TYPE_TEST_RESCUE));
        assert!(is_supported_connector_type(CONNECTOR_TYPE_KAFKA));
        assert!(!is_supported_connector_type("unknown_type"));
    }

    #[test]
    fn test_group_name_constants() {
        assert_eq!(GROUP_MONITOR, "monitor");
        assert_eq!(GROUP_DEFAULT, "default");
        assert_eq!(GROUP_MISS, "miss");
        assert_eq!(GROUP_RESIDUE, "residue");
        assert_eq!(GROUP_ERROR, "error");

        assert!(is_infra_group_name(GROUP_MONITOR));
        assert!(is_infra_group_name(GROUP_DEFAULT));
        assert!(is_infra_group_name(GROUP_MISS));
        assert!(is_infra_group_name(GROUP_RESIDUE));
        // intercept removed
        assert!(is_infra_group_name(GROUP_ERROR));

        assert!(!is_business_group_name(GROUP_MONITOR));
        assert!(is_business_group_name("business_group"));
    }

    #[test]
    fn test_field_constants() {
        assert_eq!(FIELD_PARAMS, "params");
        assert_eq!(FIELD_PARAMS_OVERRIDE, "params_override");
        assert_eq!(FIELD_TAGS, "tags");
        assert_eq!(FIELD_FMT, "fmt");
        assert_eq!(FIELD_OML, "oml");
        assert_eq!(FIELD_RULE, "rule");
        assert_eq!(FIELD_NAME, "name");
        assert_eq!(FIELD_CONNECT, "connect");
        assert_eq!(FIELD_TYPE, "type");
        assert_eq!(FIELD_ALLOW_OVERRIDE, "allow_override");
        assert_eq!(FIELD_PARALLEL, "parallel");
        assert_eq!(FIELD_EXPECT, "expect");
        assert_eq!(FIELD_FILTER, "filter");
        assert_eq!(FIELD_VERSION, "version");
        assert_eq!(FIELD_BASE, "base");
        assert_eq!(FIELD_FILE, "file");

        assert!(is_nested_field_blacklisted(FIELD_PARAMS));
        assert!(is_nested_field_blacklisted(FIELD_PARAMS_OVERRIDE));
        assert!(!is_nested_field_blacklisted(FIELD_TAGS));
    }

    #[test]
    fn test_format_constants() {
        assert_eq!(FORMAT_JSON, "json");
        assert_eq!(FORMAT_KV, "kv");
        assert_eq!(FORMAT_RAW, "raw");
        assert_eq!(DEFAULT_OUTPUT_FORMAT, FORMAT_JSON);

        assert!(is_supported_format(FORMAT_JSON));
        assert!(is_supported_format(FORMAT_KV));
        assert!(is_supported_format(FORMAT_RAW));
        assert!(!is_supported_format("unknown_format"));
    }

    #[test]
    fn test_status_constants() {
        assert_eq!(STATUS_OK, "OK");
        assert_eq!(STATUS_EXCEEDED, "Exceeded");
        assert_eq!(STATUS_OUT_OF_RANGE, "Out of range");
        assert_eq!(STATUS_IGNORED, "Ignored");
        assert_eq!(STATUS_COUNTED, "counted");
        assert_eq!(STATUS_STATS, "stats");
        assert_eq!(STATUS_ERROR_PREFIX, "[100] validation error <<");
    }

    #[test]
    fn test_display_names() {
        assert_eq!(get_group_display_name(GROUP_MONITOR), "Monitor");
        assert_eq!(get_group_display_name(GROUP_DEFAULT), "Default");
        assert_eq!(get_group_display_name(GROUP_MISS), "Miss");
        assert_eq!(get_group_display_name(GROUP_RESIDUE), "Residue");
        // intercept removed
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
