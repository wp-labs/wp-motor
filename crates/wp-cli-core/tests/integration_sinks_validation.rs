// Integration test for sink routes validation
//
// This test ensures that the sink route validation logic works correctly
// across the full call chain from CLI to config layer.

use orion_variate::EnvDict;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use wp_cli_core::business::connectors::sinks;
use wp_conf::test_support::ForTest;

/// Helper to create a test environment with valid sink configuration
fn create_valid_sink_env() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();

    // Create directory structure
    fs::create_dir_all(root.join("connectors/sink.d")).unwrap();
    fs::create_dir_all(root.join("models/sinks/business.d")).unwrap();

    // Create connector definition
    fs::write(
        root.join("connectors/sink.d/test.toml"),
        r#"
[[connectors]]
id = "test_sink"
type = "file"
allow_override = ["file", "path"]
"#,
    )
    .unwrap();

    // Create valid route configuration with RULE only
    fs::write(
        root.join("models/sinks/business.d/valid.toml"),
        r#"
version = "2.0"

[sink_group]
name = "test_group"
rule = ["/test/*", "/api/*"]
tags = ["test"]

[[sink_group.sinks]]
name = "sink1"
connect = "test_sink"
params = { file = "output.txt" }
"#,
    )
    .unwrap();

    (temp, root)
}

#[test]
fn test_validate_routes_with_valid_config() {
    let (_temp, root) = create_valid_sink_env();

    let result = sinks::validate_routes(root.to_str().unwrap(), &EnvDict::test_default());

    assert!(result.is_ok(), "Valid configuration should pass validation");
}

#[test]
fn test_validate_routes_detects_oml_rule_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join("connectors/sink.d")).unwrap();
    fs::create_dir_all(root.join("models/sinks/business.d")).unwrap();

    // Create connector
    fs::write(
        root.join("connectors/sink.d/test.toml"),
        r#"
[[connectors]]
id = "test_sink"
type = "file"
allow_override = ["file"]
"#,
    )
    .unwrap();

    // Create INVALID configuration with both OML and RULE
    fs::write(
        root.join("models/sinks/business.d/invalid.toml"),
        r#"
version = "2.0"

[sink_group]
name = "conflict_group"
oml = ["model1", "model2"]
rule = ["/test/*", "/api/*"]

[[sink_group.sinks]]
name = "sink1"
connect = "test_sink"
params = { file = "output.txt" }
"#,
    )
    .unwrap();

    let result = sinks::validate_routes(root.to_str().unwrap(), &EnvDict::test_default());

    assert!(
        result.is_err(),
        "Configuration with both OML and RULE should fail"
    );
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("OML and RULE cannot be used together"),
        "Error message should mention OML/RULE conflict, got: {}",
        error_msg
    );
}

#[test]
fn test_validate_routes_detects_invalid_rule_pattern() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join("connectors/sink.d")).unwrap();
    fs::create_dir_all(root.join("models/sinks/business.d")).unwrap();

    fs::write(
        root.join("connectors/sink.d/test.toml"),
        r#"
[[connectors]]
id = "test_sink"
type = "file"
allow_override = ["file"]
"#,
    )
    .unwrap();

    // Create configuration with invalid RULE pattern (missing leading '/')
    fs::write(
        root.join("models/sinks/business.d/invalid_rule.toml"),
        r#"
version = "2.0"

[sink_group]
name = "invalid_rule_group"
rule = ["test/*", "api/*"]

[[sink_group.sinks]]
name = "sink1"
connect = "test_sink"
params = { file = "output.txt" }
"#,
    )
    .unwrap();

    let result = sinks::validate_routes(root.to_str().unwrap(), &EnvDict::test_default());

    assert!(
        result.is_err(),
        "Invalid rule pattern should fail validation"
    );
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("should start with '/'"),
        "Error should mention missing '/' prefix, got: {}",
        error_msg
    );
}

#[test]
fn test_validate_routes_detects_empty_patterns() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join("connectors/sink.d")).unwrap();
    fs::create_dir_all(root.join("models/sinks/business.d")).unwrap();

    fs::write(
        root.join("connectors/sink.d/test.toml"),
        r#"
[[connectors]]
id = "test_sink"
type = "file"
allow_override = ["file"]
"#,
    )
    .unwrap();

    // Create configuration with empty rule pattern
    fs::write(
        root.join("models/sinks/business.d/empty_pattern.toml"),
        r#"
version = "2.0"

[sink_group]
name = "empty_pattern_group"
rule = ["", "/valid/*"]

[[sink_group.sinks]]
name = "sink1"
connect = "test_sink"
params = { file = "output.txt" }
"#,
    )
    .unwrap();

    let result = sinks::validate_routes(root.to_str().unwrap(), &EnvDict::test_default());

    assert!(result.is_err(), "Empty pattern should fail validation");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Empty rule pattern found"),
        "Error should mention empty pattern, got: {}",
        error_msg
    );
}

#[test]
fn test_route_table_generation() {
    let (_temp, root) = create_valid_sink_env();

    let result = sinks::route_table(root.to_str().unwrap(), &[], &[], &EnvDict::test_default());

    assert!(result.is_ok(), "Route table generation should succeed");
    let routes = result.unwrap();
    assert!(!routes.is_empty(), "Should generate at least one route");

    let route = &routes[0];
    assert_eq!(route.group, "test_group");
    assert_eq!(route.name, "sink1");
    assert_eq!(route.connector, "test_sink");
}

#[test]
fn test_route_table_with_filters() {
    let (_temp, root) = create_valid_sink_env();

    // Test group filter
    let result = sinks::route_table(
        root.to_str().unwrap(),
        &["test_group".to_string()],
        &[],
        &EnvDict::test_default(),
    );
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());

    // Test non-matching filter
    let result = sinks::route_table(
        root.to_str().unwrap(),
        &["nonexistent".to_string()],
        &[],
        &EnvDict::test_default(),
    );
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "Non-matching filter should return empty"
    );

    // Test sink filter
    let result = sinks::route_table(
        root.to_str().unwrap(),
        &[],
        &["sink1".to_string()],
        &EnvDict::test_default(),
    );
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}
