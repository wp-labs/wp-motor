// Integration test for source file statistics
//
// This test ensures that the source file statistics collection works
// correctly across the full call chain.

use orion_variate::EnvDict;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use wp_cli_core::Ctx;
use wp_cli_core::list_file_sources_with_lines;
use wp_conf::engine::EngineConfig;

/// Helper to create a test environment with source configuration
fn create_source_env() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();

    // Create directory structure
    fs::create_dir_all(root.join("connectors/source.d")).unwrap();
    fs::create_dir_all(root.join("topology/sources")).unwrap();

    // Create connector definition
    fs::write(
        root.join("connectors/source.d/file.toml"),
        r#"
[[connectors]]
id = "test_file"
type = "file"
allow_override = ["path", "base", "file"]

[connectors.default_params]
fmt = "json"
"#,
    )
    .unwrap();

    // Create wpsrc.toml
    fs::write(
        root.join("topology/sources/wpsrc.toml"),
        r#"
[[sources]]
key = "test_source_1"
connect = "test_file"
enable = true
params_override = { path = "test_data1.log" }

[[sources]]
key = "test_source_2"
connect = "test_file"
enable = true
params_override = { path = "test_data2.log" }

[[sources]]
key = "disabled_source"
connect = "test_file"
enable = false
params_override = { path = "disabled.log" }
"#,
    )
    .unwrap();

    // Create test data files
    fs::write(root.join("test_data1.log"), "line1\nline2\nline3\n").unwrap();
    fs::write(
        root.join("test_data2.log"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();
    fs::write(root.join("disabled.log"), "should_not_count\n").unwrap();

    (temp, root)
}

#[test]
fn test_stat_src_file_counts_all_enabled_sources() {
    let (_temp, root) = create_source_env();
    let eng_conf = EngineConfig::init(root.to_str().unwrap());
    let dict = EnvDict::new();
    let ctx = Ctx::new(root.to_string_lossy().to_string());

    let report =
        list_file_sources_with_lines(Path::new(root.to_str().unwrap()), &eng_conf, &ctx, &dict);

    assert!(report.is_some(), "Should return a report");

    let report = report.unwrap();
    assert_eq!(
        report.items.len(),
        3,
        "Should have 3 items (2 enabled + 1 disabled)"
    );

    // Find enabled sources
    let enabled_items: Vec<_> = report.items.iter().filter(|i| i.enabled).collect();
    assert_eq!(enabled_items.len(), 2, "Should have 2 enabled sources");

    // Check total lines (only from enabled sources)
    assert_eq!(
        report.total_enabled_lines, 8,
        "Total should be 3 + 5 = 8 lines from enabled sources"
    );
}

#[test]
fn test_stat_src_file_individual_source_counts() {
    let (_temp, root) = create_source_env();
    let eng_conf = EngineConfig::init(root.to_str().unwrap());
    let dict = EnvDict::new();
    let ctx = Ctx::new(root.to_string_lossy().to_string());

    let report =
        list_file_sources_with_lines(Path::new(root.to_str().unwrap()), &eng_conf, &ctx, &dict)
            .unwrap();

    // Check individual source counts
    let source1 = report
        .items
        .iter()
        .find(|i| i.key == "test_source_1")
        .unwrap();
    assert_eq!(source1.lines, Some(3), "test_source_1 should have 3 lines");
    assert!(source1.enabled);
    assert!(source1.error.is_none());

    let source2 = report
        .items
        .iter()
        .find(|i| i.key == "test_source_2")
        .unwrap();
    assert_eq!(source2.lines, Some(5), "test_source_2 should have 5 lines");
    assert!(source2.enabled);

    let disabled = report
        .items
        .iter()
        .find(|i| i.key == "disabled_source")
        .unwrap();
    assert!(!disabled.enabled, "disabled_source should be disabled");
    assert_eq!(disabled.lines, None, "Disabled source lines should be None");
}

#[test]
fn test_stat_src_file_handles_missing_file() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join("connectors/source.d")).unwrap();
    fs::create_dir_all(root.join("topology/sources")).unwrap();

    fs::write(
        root.join("connectors/source.d/file.toml"),
        r#"
[[connectors]]
id = "test_file"
type = "file"
allow_override = ["path"]
"#,
    )
    .unwrap();

    fs::write(
        root.join("topology/sources/wpsrc.toml"),
        r#"
[[sources]]
key = "missing_file"
connect = "test_file"
enable = true
params_override = { path = "nonexistent.log" }
"#,
    )
    .unwrap();

    let eng_conf = EngineConfig::init(root.to_str().unwrap());
    let dict = EnvDict::new();
    let ctx = Ctx::new(root.to_string_lossy().to_string());

    let report =
        list_file_sources_with_lines(Path::new(root.to_str().unwrap()), &eng_conf, &ctx, &dict);

    assert!(
        report.is_some(),
        "Should return a report even when file is missing"
    );
    let report = report.unwrap();

    assert_eq!(report.items.len(), 1);
    assert_eq!(
        report.items[0].lines, None,
        "Missing file should have None lines"
    );
    assert!(report.items[0].error.is_some(), "Should have error message");
}

#[test]
fn test_stat_src_file_with_base_file_params() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join("connectors/source.d")).unwrap();
    fs::create_dir_all(root.join("topology/sources")).unwrap();
    fs::create_dir_all(root.join("data")).unwrap();

    fs::write(
        root.join("connectors/source.d/file.toml"),
        r#"
[[connectors]]
id = "test_file"
type = "file"
allow_override = ["base", "file"]
"#,
    )
    .unwrap();

    fs::write(
        root.join("topology/sources/wpsrc.toml"),
        r#"
[[sources]]
key = "base_file_source"
connect = "test_file"
enable = true
params_override = { base = "data", file = "test.log" }
"#,
    )
    .unwrap();

    // Create data file using base + file path
    fs::write(root.join("data/test.log"), "line1\nline2\n").unwrap();

    let eng_conf = EngineConfig::init(root.to_str().unwrap());
    let dict = EnvDict::new();
    let ctx = Ctx::new(root.to_string_lossy().to_string());

    let report =
        list_file_sources_with_lines(Path::new(root.to_str().unwrap()), &eng_conf, &ctx, &dict);

    assert!(report.is_some());
    let report = report.unwrap();

    assert_eq!(
        report.items[0].lines,
        Some(2),
        "Should count lines from base+file path"
    );
    assert!(report.items[0].path.contains("data"));
    assert!(report.items[0].path.contains("test.log"));
}

#[test]
fn test_stat_src_file_with_empty_wpsrc() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join("connectors/source.d")).unwrap();
    fs::create_dir_all(root.join("topology/sources")).unwrap();

    // Create empty wpsrc.toml
    fs::write(root.join("topology/sources/wpsrc.toml"), "").unwrap();

    let eng_conf = EngineConfig::init(root.to_str().unwrap());
    let dict = EnvDict::new();
    let ctx = Ctx::new(root.to_string_lossy().to_string());

    let report =
        list_file_sources_with_lines(Path::new(root.to_str().unwrap()), &eng_conf, &ctx, &dict);

    // Should either return None or empty report
    if let Some(report) = report {
        assert_eq!(report.items.len(), 0, "Empty wpsrc should have no items");
        assert_eq!(report.total_enabled_lines, 0);
    }
}
