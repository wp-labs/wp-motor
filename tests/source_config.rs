//! Integration tests for unified source configuration system
//!
//! This test suite validates the unified configuration system that allows
//! defining sources with connectors and parameter overrides. It covers:
//! - File source configuration and building
//! - Configuration validation scenarios
//! - Connector dependency management
//! - Parameter override enforcement
//! - Base+file path handling

use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use std::sync::Once;
use wp_conf::test_support::ForTest;
use wp_engine::facade::test_helpers as fth;
static INIT: Once = Once::new();
fn init_runtime() {
    INIT.call_once(|| {
        wp_engine::connectors::startup::init_runtime_registries();
    });
}

//=============================================================================
// Test Constants and Utilities
//=============================================================================

/// Base directory for all test configurations
const TEST_BASE_DIR: &str = "tmp/unified_config_tests";

/// Common connector ID for file sources
const FILE_CONNECTOR_ID: &str = "file_main";

/// Standard file source key for testing
const FILE_SOURCE_KEY: &str = "file_unified";

/// Default file content for test files
const DEFAULT_FILE_CONTENT: &str = "hello";

/// Test file names
const TEST_LOG_FILE: &str = "wparse_unified_sources_test.log";
const TEST_VALIDATE_FILE: &str = "wparse_unified_sources_validate.log";
const TEST_BASE_FILE: &str = "wparse_unified_sources_base_file.log";
const TEST_WHITELIST_FILE: &str = "wparse_unified_sources_wl.log";

//=============================================================================
// Test Directory Management
//=============================================================================

/// Create a clean test directory structure
fn create_test_dir(test_name: &str) -> PathBuf {
    let test_dir = PathBuf::from(TEST_BASE_DIR).join(test_name);

    // Clean up any existing directory
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).expect("Failed to clean up existing test directory");
    }

    // Create directory structure for connectors
    // Use facade test helper to avoid path guesswork and internal coupling.
    let _connector_dir = fth::create_connectors_dir(&test_dir);

    test_dir
}

/// Clean up a test directory
fn cleanup_test_dir(test_name: &str) {
    let test_dir = PathBuf::from(TEST_BASE_DIR).join(test_name);
    std::fs::remove_dir_all(test_dir).ok();
}

/// Get the connector directory path for a test
fn get_connector_dir(test_dir: &Path) -> PathBuf {
    fth::create_connectors_dir(test_dir)
}

//=============================================================================
// File Management Utilities
//=============================================================================

/// Create a temporary test file with specified content
fn create_test_file(filename: &str, content: &str) -> PathBuf {
    let file_path = std::env::temp_dir().join(filename);
    std::fs::write(&file_path, content).expect("Failed to create test file");
    file_path
}

/// Create a temporary test file in the temp directory
fn create_temp_file_in_path(path: &Path, filename: &str, content: &str) -> PathBuf {
    let file_path = path.join(filename);
    std::fs::write(&file_path, content).expect("Failed to create test file");
    file_path
}

//=============================================================================
// Configuration Builders
//=============================================================================

/// Builder for file connector configurations
struct FileConnectorBuilder {
    id: String,
    path: Option<String>,
    base: Option<String>,
    file: Option<String>,
    encode: String,
    allow_override: Vec<String>,
}

impl FileConnectorBuilder {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            path: None,
            base: None,
            file: None,
            encode: "text".to_string(),
            allow_override: vec!["path".to_string(), "encode".to_string()],
        }
    }

    fn with_path(mut self, path: &str) -> Self {
        self.path = Some(path.to_string());
        self
    }

    fn with_base_file(mut self, base: &str, file: &str) -> Self {
        self.base = Some(base.to_string());
        self.file = Some(file.to_string());
        self
    }

    #[allow(dead_code)]
    fn with_encoding(mut self, encoding: &str) -> Self {
        self.encode = encoding.to_string();
        self
    }

    fn with_allowed_overrides(mut self, overrides: Vec<&str>) -> Self {
        self.allow_override = overrides.into_iter().map(|s| s.to_string()).collect();
        self
    }

    fn build(self) -> String {
        let mut config = String::new();
        config.push_str(&format!(
            r#"[[connectors]]
id = "{}"
type = "file"
allow_override = ["{}"]
"#,
            self.id,
            self.allow_override.join("\", \"")
        ));

        config.push_str("[connectors.params]\n");

        if let Some(path) = self.path {
            config.push_str(&format!("path = \"{}\"\n", path));
        }

        if let Some(base) = self.base {
            config.push_str(&format!("base = \"{}\"\n", base));
        }

        if let Some(file) = self.file {
            config.push_str(&format!("file = \"{}\"\n", file));
        }

        config.push_str(&format!("encode = \"{}\"\n", self.encode));
        config
    }
}

/// Builder for source configurations
struct SourceConfigBuilder {
    key: String,
    enable: bool,
    connect: String,
    params_override: String,
    tags: Vec<String>,
}

impl SourceConfigBuilder {
    fn new(key: &str, connector: &str) -> Self {
        Self {
            key: key.to_string(),
            enable: true,
            connect: connector.to_string(),
            params_override: "{ }".to_string(),
            tags: Vec::new(),
        }
    }

    fn disabled(mut self) -> Self {
        self.enable = false;
        self
    }

    fn with_params(mut self, params: &str) -> Self {
        self.params_override = params.to_string();
        self
    }

    fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(|s| s.to_string()).collect();
        self
    }

    fn build(self) -> String {
        let mut config = String::new();
        config.push_str(&format!(
            r#"
[[sources]]
key = "{}"
enable = {}
connect = "{}"
"#,
            self.key, self.enable, self.connect
        ));

        if !self.tags.is_empty() {
            config.push_str(&format!(
                "tags = [{}]\n",
                self.tags
                    .iter()
                    .map(|tag| format!("\"{}\"", tag))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        config.push_str(&format!("params_override = {}\n", self.params_override));
        config
    }
}

//=============================================================================
// Common Test Patterns
//=============================================================================

/// Setup a complete test environment with file connector and source config
fn setup_file_source_test(
    test_name: &str,
    connector_builder: FileConnectorBuilder,
    source_builder: SourceConfigBuilder,
    test_file: Option<PathBuf>,
) -> (PathBuf, PathBuf) {
    let test_dir = create_test_dir(test_name);
    let connector_dir = get_connector_dir(&test_dir);

    // Write connector configuration
    let connector_config = connector_builder.build();
    std::fs::write(connector_dir.join("c1.toml"), connector_config)
        .expect("Failed to write connector config");

    // Create test file if provided
    if let Some(file_path) = test_file {
        std::fs::write(&file_path, DEFAULT_FILE_CONTENT)
            .expect("Failed to write test file content");
    }

    // Write source configuration
    let source_config = source_builder.build();
    let config_path = test_dir.join("sources.toml");
    std::fs::write(&config_path, source_config).expect("Failed to write source config");

    (test_dir, config_path)
}

/// Register the file source factory
fn register_file_source_factory() {
    // 统一通过集中初始化完成注册
    init_runtime();
}

/// Create a source configuration parser
fn create_config_parser(work_dir: PathBuf) -> wp_engine::sources::SourceConfigParser {
    wp_engine::sources::SourceConfigParser::new(work_dir)
}

//=============================================================================
// Basic Configuration Tests
//=============================================================================

#[tokio::test]
async fn unified_config_builds_file_source_successfully() -> anyhow::Result<()> {
    register_file_source_factory();

    // Create test file
    let test_file = create_test_file(TEST_LOG_FILE, DEFAULT_FILE_CONTENT);

    // Build connector and source configurations
    let connector_builder =
        FileConnectorBuilder::new(FILE_CONNECTOR_ID).with_path(&test_file.display().to_string());

    let source_builder = SourceConfigBuilder::new(FILE_SOURCE_KEY, FILE_CONNECTOR_ID);

    // Setup test environment
    let (test_dir, _config_path) = setup_file_source_test(
        "build_file_source",
        connector_builder,
        source_builder,
        Some(test_file),
    );

    // Parse and build configuration
    let parser = create_config_parser(test_dir);
    let env_dict = EnvDict::new();
    let (sources, _) = parser
        .parse_and_build_from(
            &format!(
                r#"
[[sources]]
key = "{}"
enable = true
connect = "{}"
params_override = {{ }}
"#,
                FILE_SOURCE_KEY, FILE_CONNECTOR_ID
            ),
            &env_dict,
        )
        .await
        .expect("Failed to parse and build configuration");

    // Verify results
    assert_eq!(sources.len(), 1, "Expected exactly 1 source to be built");
    assert_eq!(
        sources[0].source.identifier(),
        FILE_SOURCE_KEY,
        "Source identifier should match configuration key"
    );

    println!("✅ File source built successfully with unified configuration");

    cleanup_test_dir("build_file_source");
    Ok(())
}

#[test]
fn unified_config_validates_file_source_configuration() {
    register_file_source_factory();

    // Build connector configuration
    let test_file = std::env::temp_dir().join(TEST_VALIDATE_FILE);
    let connector_builder =
        FileConnectorBuilder::new(FILE_CONNECTOR_ID).with_path(&test_file.display().to_string());

    // Setup test environment (no source config needed for validation-only)
    let test_dir = create_test_dir("validate_file_source");
    let connector_dir = get_connector_dir(&test_dir);

    let connector_config = connector_builder.build();
    std::fs::write(connector_dir.join("c1.toml"), connector_config)
        .expect("Failed to write connector config");

    // Parse and validate configuration
    let parser = create_config_parser(test_dir);
    let specs = parser
        .parse_and_validate_only(
            &format!(
                r#"
[[sources]]
key = "{}"
enable = true
connect = "{}"
params_override = {{ }}
"#,
                FILE_SOURCE_KEY, FILE_CONNECTOR_ID
            ),
            &EnvDict::test_default(),
        )
        .expect("Failed to validate configuration");

    // Validate specifications
    assert_eq!(specs.len(), 1, "Expected exactly 1 specification");
    assert_eq!(
        specs[0].name, FILE_SOURCE_KEY,
        "Specification name should match source key"
    );
    assert!(
        specs[0].kind.is_empty(),
        "Kind should be empty in validate-only mode"
    );

    println!("✅ File source configuration validated successfully");

    cleanup_test_dir("validate_file_source");
}

#[test]
fn validate_only_succeeds_without_connectors() {
    let test_dir = create_test_dir("validate_without_connectors");

    let source_config = SourceConfigBuilder::new("s1", "missing_conn")
        .with_tags(vec!["env:test"])
        .build();

    let parser = create_config_parser(test_dir);
    let specs = parser
        .parse_and_validate_only(&source_config, &EnvDict::test_default())
        .expect("Validate-only should succeed without connectors");

    assert_eq!(specs.len(), 1, "Expected exactly 1 specification");
    assert_eq!(specs[0].name, "s1", "Specification name should match");
    assert!(
        specs[0].kind.is_empty(),
        "Kind should be empty without connectors"
    );
    assert!(
        specs[0].params.is_empty(),
        "Params should be empty without connectors"
    );

    println!("✅ Validate-only works correctly without connectors");

    cleanup_test_dir("validate_without_connectors");
}

#[tokio::test]
async fn build_fails_without_connectors() {
    let test_dir = create_test_dir("build_without_connectors");

    let source_config = SourceConfigBuilder::new("s1", FILE_CONNECTOR_ID).build();

    let parser = create_config_parser(test_dir);
    let env_dict = EnvDict::new();
    let result = parser.parse_and_build_from(&source_config, &env_dict).await;

    assert!(result.is_err(), "Build should fail without connectors");

    let error_message = format!("{}", result.unwrap_err());
    assert!(
        error_message.contains("connector not found"),
        "Error should mention missing connector"
    );

    println!(
        "✅ Build correctly failed without connectors: {}",
        error_message
    );

    cleanup_test_dir("build_without_connectors");
}

//=============================================================================
// Advanced Configuration Tests
//=============================================================================

#[tokio::test]
async fn unified_config_handles_base_file_parameters() -> anyhow::Result<()> {
    register_file_source_factory();

    // Create test file in temp directory
    let tmpdir = std::env::temp_dir();
    let test_file = create_temp_file_in_path(&tmpdir, TEST_BASE_FILE, DEFAULT_FILE_CONTENT);

    // Build connector with base+file configuration
    let connector_builder = FileConnectorBuilder::new(FILE_CONNECTOR_ID)
        .with_base_file(
            &tmpdir.display().to_string(),
            &test_file.file_name().unwrap().to_string_lossy(),
        )
        .with_allowed_overrides(vec!["base", "file", "encode"]);

    let source_builder = SourceConfigBuilder::new(FILE_SOURCE_KEY, FILE_CONNECTOR_ID);

    // Setup test environment
    let (test_dir, _config_path) = setup_file_source_test(
        "base_file_config",
        connector_builder,
        source_builder,
        Some(test_file),
    );

    // Parse and build configuration
    let parser = create_config_parser(test_dir);
    let env_dict = EnvDict::new();
    let (sources, _) = parser
        .parse_and_build_from(
            &format!(
                r#"
[[sources]]
key = "{}"
enable = true
connect = "{}"
params_override = {{ }}
"#,
                FILE_SOURCE_KEY, FILE_CONNECTOR_ID
            ),
            &env_dict,
        )
        .await
        .expect("Failed to parse and build base+file configuration");

    // Verify results
    assert_eq!(sources.len(), 1, "Expected exactly 1 source to be built");
    assert_eq!(
        sources[0].source.identifier(),
        FILE_SOURCE_KEY,
        "Source identifier should match configuration key"
    );

    println!("✅ Base+file configuration handled successfully");

    cleanup_test_dir("base_file_config");
    Ok(())
}

#[tokio::test]
async fn unified_config_enforces_parameter_override_whitelist() {
    register_file_source_factory();

    // Create test file
    let test_file = create_test_file(TEST_WHITELIST_FILE, DEFAULT_FILE_CONTENT);

    // Build connector that only allows "path" override
    let connector_builder = FileConnectorBuilder::new(FILE_CONNECTOR_ID)
        .with_path(&test_file.display().to_string())
        .with_allowed_overrides(vec!["path"]); // Note: "encode" is NOT in whitelist

    // Build source that tries to override "encode" parameter
    let source_builder =
        SourceConfigBuilder::new("s1", FILE_CONNECTOR_ID).with_params(r#"{ encode = "hex" }"#);

    // Setup test environment
    let (test_dir, _config_path) = setup_file_source_test(
        "whitelist_enforcement",
        connector_builder,
        source_builder,
        Some(test_file),
    );

    // Attempt to parse and build configuration
    let parser = create_config_parser(test_dir);
    let env_dict = EnvDict::new();
    let result = parser
        .parse_and_build_from(
            &format!(
                r#"
[[sources]]
key = "{}"
enable = true
connect = "{}"
params_override = {{ encode = "hex" }}
"#,
                "s1", FILE_CONNECTOR_ID
            ),
            &env_dict,
        )
        .await;

    assert!(
        result.is_err(),
        "Build should fail when trying to override non-whitelisted parameter"
    );

    println!("✅ Parameter override whitelist correctly enforced");

    cleanup_test_dir("whitelist_enforcement");
}

//=============================================================================
// Error Handling and Edge Cases
//=============================================================================

#[test]
fn configuration_handles_missing_test_files_gracefully() {
    register_file_source_factory();

    // Build connector with non-existent file path
    let connector_builder =
        FileConnectorBuilder::new(FILE_CONNECTOR_ID).with_path("/non/existent/test.log");

    let source_builder = SourceConfigBuilder::new("test_missing_file", FILE_CONNECTOR_ID);

    // Setup test environment (no actual file creation)
    let (test_dir, _config_path) = setup_file_source_test(
        "missing_file_handling",
        connector_builder,
        source_builder,
        None, // No file creation
    );

    // Validation should succeed (file existence checked during build, not validation)
    let parser = create_config_parser(test_dir);
    let specs = parser
        .parse_and_validate_only(
            &format!(
                r#"
[[sources]]
key = "{}"
enable = true
connect = "{}"
params_override = {{ }}
"#,
                "test_missing_file", FILE_CONNECTOR_ID
            ),
            &EnvDict::test_default(),
        )
        .expect("Validation should succeed even with missing files");

    assert_eq!(specs.len(), 1, "Expected exactly 1 specification");

    println!("✅ Missing file handling works correctly in validation mode");

    cleanup_test_dir("missing_file_handling");
}

#[test]
fn configuration_handles_empty_parameters_correctly() {
    register_file_source_factory();

    let test_dir = create_test_dir("empty_parameters");
    let connector_dir = get_connector_dir(&test_dir);

    // Create minimal connector configuration
    let connector_config = FileConnectorBuilder::new("minimal_connector")
        .with_allowed_overrides(vec![])
        .build();

    std::fs::write(connector_dir.join("c1.toml"), connector_config)
        .expect("Failed to write connector config");

    // Create source with empty parameters
    let source_config = SourceConfigBuilder::new("empty_params", "minimal_connector")
        .with_params("{}")
        .build();

    let parser = create_config_parser(test_dir);
    let specs = parser
        .parse_and_validate_only(&source_config, &EnvDict::test_default())
        .expect("Validation should succeed with empty parameters");

    assert_eq!(specs.len(), 1, "Expected exactly 1 specification");
    assert_eq!(
        specs[0].name, "empty_params",
        "Specification name should match"
    );

    println!("✅ Empty parameters handled correctly");

    cleanup_test_dir("empty_parameters");
}

#[test]
fn configuration_handles_disabled_sources() {
    // Test that disabled sources are filtered out during validation
    let test_dir = create_test_dir("disabled_sources");

    let source_config = SourceConfigBuilder::new("disabled_source", FILE_CONNECTOR_ID)
        .disabled()
        .build();

    let parser = create_config_parser(test_dir);
    let specs = parser
        .parse_and_validate_only(&source_config, &EnvDict::test_default())
        .expect("Validation should succeed for disabled sources");

    // Disabled sources should be filtered out (0 specs expected)
    assert_eq!(
        specs.len(),
        0,
        "Disabled sources should be filtered out during validation"
    );

    println!("✅ Disabled sources correctly filtered out during validation");

    cleanup_test_dir("disabled_sources");
}
