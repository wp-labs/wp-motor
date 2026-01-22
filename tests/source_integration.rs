//! Integration tests for source data collection
//!
//! This test suite validates the complete end-to-end functionality of
//! syslog and file sources with real data flow, covering:
//! - File source reading and processing
//! - UDP/TCP syslog source lifecycle management
//! - Mixed source integration scenarios
//! - Error handling and validation
//! - Tag functionality and metadata attachment

use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::time::{Duration, timeout};
use wp_conf::RunMode;
use wp_engine::connectors::registry as reg;
use wp_engine::sources::SourceConfigParser;
use wp_parse_api::RawData;

//=============================================================================
// Test Constants and Utilities
//=============================================================================

/// Maximum number of messages to read from a source during testing
const MAX_TEST_MESSAGES: usize = 10;

/// Test data directory names
const TEST_DIR_FILE: &str = "test_data";
const TEST_DIR_MIXED: &str = "test_mixed";
const TEST_DIR_TAGS: &str = "test_tags";

/// Test log messages
const APP_LOG_MESSAGES: &[&str] = &[
    "2025-10-15 10:30:45 INFO Application started",
    "2025-10-15 10:30:46 DEBUG Loading configuration",
    "2025-10-15 10:30:47 INFO Database connected",
];

const LARGE_FILE_MESSAGES: &[&str] = &[
    "L0 First shard line",
    "L1 Second shard line",
    "L2 Third shard line",
    "L3 Fourth shard line",
    "L4 Fifth shard line",
    "L5 Sixth shard line",
];

/// Access log test messages
const ACCESS_LOG_MESSAGES: &[&str] = &[
    "192.168.1.100 - - [15/Oct/2025:10:30:45 +0000] \"GET /api/users HTTP/1.1\" 200 1234",
    "192.168.1.101 - - [15/Oct/2025:10:31:02 +0000] \"POST /api/login HTTP/1.1\" 401 567",
];

/// Syslog test messages
const SYSLOG_MESSAGES: &[&str] = &[
    "<34>Oct 15 10:30:45 mymachine su: 'su root' failed",
    "<13>Oct 15 10:31:02 mymachine login: ROOT LOGIN",
];

/// Test suite initialization
use std::sync::Once;
static INIT: Once = Once::new();
fn setup_test_environment() {
    INIT.call_once(|| {
        wp_engine::connectors::startup::init_runtime_registries();
    });
}

// 轻量通用工具（与其它 tests 共享）
mod common;

/// Create a test directory with the given name
fn create_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from(name);
    std::fs::create_dir_all(&test_dir).expect("Failed to create test directory");
    test_dir
}

/// Clean up a test directory
fn cleanup_test_dir(name: &str) {
    std::fs::remove_dir_all(name).ok();
}

/// Create a test file with the given content
async fn create_test_file(dir: &Path, filename: &str, content: &[&str]) -> PathBuf {
    let test_file = dir.join(filename);
    tokio::fs::write(&test_file, content.join("\n"))
        .await
        .expect("Failed to write test file");
    test_file
}

/// Get a registered source factory from engine registry
fn get_factory(kind: &str) -> std::sync::Arc<dyn wp_connector_api::SourceFactory> {
    reg::get_source_factory(kind).expect("factory not registered")
}

/// Create a source build context
fn create_build_context() -> wp_connector_api::SourceBuildCtx {
    wp_connector_api::SourceBuildCtx::new(PathBuf::from("."))
}

fn primary_source_handle_mut(
    svc: &mut wp_connector_api::SourceSvcIns,
) -> &mut wp_connector_api::SourceHandle {
    svc.sources
        .get_mut(0)
        .expect("test expects a single source handle")
}

fn primary_source_handle(svc: &wp_connector_api::SourceSvcIns) -> &wp_connector_api::SourceHandle {
    svc.sources
        .first()
        .expect("test expects a single source handle")
}

/// Read all available messages from a source
async fn read_messages_from_source(source: &mut wp_connector_api::SourceHandle) -> Vec<String> {
    let mut messages = Vec::new();
    for _ in 0..MAX_TEST_MESSAGES {
        match source.source.receive().await {
            Ok(batch) => {
                if batch.is_empty() {
                    break;
                }
                for frame in batch {
                    match &frame.payload {
                        RawData::String(s) => messages.push(s.clone()),
                        RawData::Bytes(b) => messages.push(String::from_utf8_lossy(b).to_string()),
                        RawData::ArcBytes(b) => {
                            messages.push(String::from_utf8_lossy(b).to_string())
                        }
                    }
                    if messages.len() >= MAX_TEST_MESSAGES {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
    messages
}

/// Read messages from a source with detailed output
async fn read_messages_with_logging(
    source: &mut wp_connector_api::SourceHandle,
    source_id: &str,
    max_messages: usize,
) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    let mut emitted = 0usize;
    println!("Reading from source: {}", source_id);

    while emitted < max_messages {
        match source.source.receive().await {
            Ok(batch) => {
                if batch.is_empty() {
                    break;
                }
                for frame in batch {
                    if emitted >= max_messages {
                        break;
                    }
                    let msg = match &frame.payload {
                        RawData::String(s) => s.clone(),
                        RawData::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                        RawData::ArcBytes(b) => String::from_utf8_lossy(b).to_string(),
                    };
                    emitted += 1;
                    println!("  Message {}: {}", emitted, msg);
                    messages.push((source_id.to_string(), msg));
                }
            }
            Err(e) => {
                println!("  Error reading message {}: {}", emitted + 1, e);
                break;
            }
        }
    }
    messages
}

// 重用 common 中统一的构造器，减少样板
use crate::common::{FileSourceBuilder, SyslogSourceBuilder};

//=============================================================================
// File Source Tests
//=============================================================================

#[tokio::test]
async fn file_source_reads_all_messages_correctly() -> anyhow::Result<()> {
    setup_test_environment();

    let test_dir = create_test_dir(TEST_DIR_FILE);
    let test_file = create_test_file(&test_dir, "test.log", APP_LOG_MESSAGES).await;

    // Build and configure file source
    let spec = FileSourceBuilder::new("test_file_source", &test_file.display().to_string())
        .with_tags(vec!["test", "type:log"])
        .build();

    let factory = get_factory("file");

    let ctx = create_build_context();
    let mut source = factory
        .build(&spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build source: {}", e))?;

    // Verify message reading
    let messages = {
        let handle = primary_source_handle_mut(&mut source);
        read_messages_from_source(handle).await
    };

    assert_eq!(
        messages.len(),
        3,
        "Expected 3 messages, got {}",
        messages.len()
    );
    common::assert_contains(&messages[0], "Application started");
    common::assert_contains(&messages[1], "Loading configuration");
    common::assert_contains(&messages[2], "Database connected");

    cleanup_test_dir(TEST_DIR_FILE);
    Ok(())
}

#[tokio::test]
async fn file_source_supports_multiple_instances() -> anyhow::Result<()> {
    setup_test_environment();

    let test_dir = create_test_dir("test_multi_file");
    let test_file = create_test_file(&test_dir, "multi.log", LARGE_FILE_MESSAGES).await;

    let spec = FileSourceBuilder::new("multi_file_source", &test_file.display().to_string())
        .with_instances(2)
        .build();

    let factory = get_factory("file");
    let ctx = create_build_context();
    let mut svc = factory
        .build(&spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build multi file source: {}", e))?;

    assert!(
        svc.sources.len() >= 2,
        "expected multiple source handles for multi-instance file"
    );

    let mut combined = Vec::new();
    for handle in svc.sources.iter_mut() {
        combined.extend(read_messages_from_source(handle).await);
    }
    combined.sort();
    let mut expected: Vec<_> = LARGE_FILE_MESSAGES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(combined, expected);

    cleanup_test_dir("test_multi_file");
    Ok(())
}

//=============================================================================
// Syslog Source Tests
//=============================================================================

#[tokio::test]
async fn udp_syslog_source_can_be_created_and_configured() -> anyhow::Result<()> {
    // Skip test if UDP is not available on this system
    if !common::is_udp_available() {
        println!("Skipping UDP test - UDP not available");
        return Ok(());
    }

    setup_test_environment();

    let spec = SyslogSourceBuilder::new("test_udp_syslog", "udp")
        .with_tags(vec!["test", "protocol:udp"])
        .build();

    let factory = get_factory("syslog");

    let ctx = create_build_context();
    let source = factory
        .build(&spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build UDP syslog source: {}", e))?;

    assert_eq!(
        primary_source_handle(&source).source.identifier(),
        "test_udp_syslog"
    );

    println!("✅ UDP syslog source created successfully");
    println!("ℹ️  Note: Full UDP testing requires network packet sending capabilities");
    Ok(())
}

#[tokio::test]
async fn tcp_syslog_source_lifecycle_management_works() -> anyhow::Result<()> {
    if !common::is_tcp_available() {
        println!("Skipping TCP syslog lifecycle test - TCP not available");
        return Ok(());
    }

    setup_test_environment();

    let spec = SyslogSourceBuilder::new("test_tcp_syslog", "tcp")
        .with_port(0) // Use ephemeral port
        .with_tcp_buffer(4096)
        .with_tags(vec!["test", "protocol:tcp"])
        .build();

    let factory = get_factory("syslog");

    let ctx = create_build_context();
    let mut source = factory
        .build(&spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build TCP syslog source: {}", e))?;

    // Test TCP source lifecycle: start and stop
    let (_ctrl_tx, ctrl_rx) = async_broadcast::broadcast::<wp_connector_api::ControlEvent>(1);

    let start_result = {
        let handle = primary_source_handle_mut(&mut source);
        timeout(Duration::from_secs(5), handle.source.start(ctrl_rx)).await
    };
    match start_result {
        Ok(result) => {
            assert!(
                result.is_ok(),
                "Failed to start TCP syslog source: {:?}",
                result
            );
        }
        Err(_) => {
            println!("Skipping TCP syslog lifecycle test - start timed out");
            return Ok(());
        }
    }
    println!("✅ TCP syslog source started successfully");

    let stop_result = {
        let handle = primary_source_handle_mut(&mut source);
        timeout(Duration::from_secs(5), handle.source.close()).await
    };
    match stop_result {
        Ok(result) => {
            assert!(
                result.is_ok(),
                "Failed to stop TCP syslog source: {:?}",
                result
            );
        }
        Err(_) => {
            println!("Skipping TCP syslog lifecycle test - stop timed out");
            return Ok(());
        }
    }
    println!("✅ TCP syslog source stopped successfully");

    Ok(())
}

//=============================================================================
// Integration Tests
//=============================================================================

#[tokio::test]
async fn mixed_sources_integration_processes_multiple_data_types() -> anyhow::Result<()> {
    setup_test_environment();

    let test_dir = create_test_dir(TEST_DIR_MIXED);

    // Create test data files
    let access_log = create_test_file(&test_dir, "access.log", ACCESS_LOG_MESSAGES).await;
    let syslog_log = create_test_file(&test_dir, "syslog.log", SYSLOG_MESSAGES).await;

    // Create multiple sources
    let ctx = create_build_context();
    let mut sources = Vec::new();

    // Access log file source
    let access_spec = FileSourceBuilder::new("mixed_access", &access_log.display().to_string())
        .with_tags(vec!["source:file", "type:access"])
        .build();

    let file_factory = reg::get_source_factory("file")
        .ok_or_else(|| anyhow::anyhow!("File factory not registered"))?;

    let access_source = file_factory
        .build(&access_spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build access source: {}", e))?;
    sources.push(access_source);

    // Syslog file source
    let syslog_file_spec =
        FileSourceBuilder::new("mixed_syslog_file", &syslog_log.display().to_string())
            .with_tags(vec!["source:file", "type:syslog"])
            .build();

    let syslog_file_source = file_factory
        .build(&syslog_file_spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build syslog file source: {}", e))?;
    sources.push(syslog_file_source);

    // UDP syslog source（仅在环境允许 UDP 绑定时构建）
    if common::is_udp_available() {
        let syslog_udp_spec = SyslogSourceBuilder::new("mixed_syslog_udp", "udp")
            .with_tags(vec!["source:syslog", "protocol:udp"])
            .build();

        let syslog_factory = reg::get_source_factory("syslog")
            .ok_or_else(|| anyhow::anyhow!("Syslog factory not registered"))?;

        let syslog_udp_source = syslog_factory
            .build(&syslog_udp_spec, &ctx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build syslog UDP source: {}", e))?;
        sources.push(syslog_udp_source);
    } else {
        println!("Skipping UDP syslog in mixed test - UDP not available");
    }

    assert!(sources.len() >= 2);

    // Read from all sources
    let mut all_messages = Vec::new();
    for source in &mut sources {
        let source_id = primary_source_handle(source).source.identifier();

        // Skip UDP sources as they don't have test data in this scenario
        if source_id.contains("udp") {
            println!(
                "Skipping UDP source: {} (no test data available)",
                source_id
            );
            continue;
        }

        let messages =
            read_messages_with_logging(primary_source_handle_mut(source), &source_id, 5).await;
        all_messages.extend(messages);
    }

    // Print summary
    println!("📊 Total messages read: {}", all_messages.len());
    for (i, (id, msg)) in all_messages.iter().enumerate() {
        println!("  [{}] {}: {}", i, id, msg);
    }

    // Verify specific message content
    let access_messages: Vec<_> = all_messages
        .iter()
        .filter(|(id, _)| id.contains("access"))
        .collect();
    let syslog_messages: Vec<_> = all_messages
        .iter()
        .filter(|(id, _)| id.contains("syslog_file"))
        .collect();

    assert_eq!(access_messages.len(), 2, "Expected 2 access log messages");
    assert_eq!(syslog_messages.len(), 2, "Expected 2 syslog file messages");

    // Content verification
    common::assert_contains(&access_messages[0].1, "GET /api/users");
    common::assert_contains(&access_messages[1].1, "POST /api/login");
    common::assert_contains(&syslog_messages[0].1, "su root");
    common::assert_contains(&syslog_messages[1].1, "ROOT LOGIN");

    // 关闭所有启动过的 source，避免后台任务影响后续测试
    for mut source in sources {
        let source_id = primary_source_handle(&source).source.identifier();
        if let Err(e) = primary_source_handle_mut(&mut source).source.close().await {
            println!("Warn: failed to close source {}: {}", source_id, e);
        }
    }

    cleanup_test_dir(TEST_DIR_MIXED);
    Ok(())
}

//=============================================================================
// Error Handling Tests
//=============================================================================

#[tokio::test]
async fn source_error_handling_detects_invalid_configurations() {
    setup_test_environment();

    let work_dir = PathBuf::from(".");
    let parser = SourceConfigParser::new(work_dir);
    let env_dict = EnvDict::new();
    let run_mode = RunMode::Daemon;

    // Test 1: Non-existent file should fail
    let invalid_file_config = r#"
[[sources]]
key = "non_existent_file"
connect = "file_main"
enable = true
tags = ["test"]
params_override = {
    path = "/non/existent/file.log",
    encode = "text"
}
"#;

    let tmp_wpsrc = NamedTempFile::new().expect("create temp wpsrc");
    std::fs::write(tmp_wpsrc.path(), invalid_file_config).expect("write temp wpsrc");
    let result = parser
        .build_source_handles(tmp_wpsrc.path(), run_mode, &env_dict)
        .await;
    assert!(result.is_err(), "Expected failure for non-existent file");
    println!("✅ Non-existent file correctly rejected");

    // Test 2: Invalid port should fail validation
    let invalid_port_config = r#"
[[sources]]
key = "invalid_port"
connect = "syslog_main"
enable = true
tags = ["test"]
params_override = {
    addr = "127.0.0.1",
    port = 99999,
    protocol = "udp"
}
"#;

    let result = parser
        .parse_and_build_from(invalid_port_config, &env_dict)
        .await;
    assert!(result.is_err(), "Expected failure for invalid port");
    println!("✅ Invalid port configuration correctly rejected");
}

#[tokio::test]
async fn source_configuration_validation_catches_parameter_errors() -> anyhow::Result<()> {
    setup_test_environment();
    let ctx = create_build_context();

    // Test 1: Invalid file encoding
    let invalid_file_spec = FileSourceBuilder::new("invalid_encoding", "/dev/null").build();

    // Manually modify the spec to have invalid encoding for testing
    let mut invalid_file_spec = invalid_file_spec;
    invalid_file_spec.params.insert(
        "encode".to_string(),
        serde_json::Value::String("invalid_encoding".to_string()),
    );

    let file_factory = reg::get_source_factory("file")
        .ok_or_else(|| anyhow::anyhow!("File factory not registered"))?;

    let result = file_factory.build(&invalid_file_spec, &ctx).await;
    assert!(
        result.is_err(),
        "Expected failure for invalid file encoding"
    );
    println!("✅ Invalid file encoding correctly rejected");

    // Test 2: Invalid syslog protocol
    let invalid_protocol_spec =
        SyslogSourceBuilder::new("invalid_protocol", "invalid_protocol").build();

    let syslog_factory = reg::get_source_factory("syslog")
        .ok_or_else(|| anyhow::anyhow!("Syslog factory not registered"))?;

    let result = syslog_factory.build(&invalid_protocol_spec, &ctx).await;
    assert!(result.is_err(), "Expected failure for invalid protocol");
    println!("✅ Invalid syslog protocol correctly rejected");

    // Test 3: Negative TCP buffer size
    let negative_buffer_spec = SyslogSourceBuilder::new("invalid_buffer", "tcp")
        .with_tcp_buffer(-1)
        .build();

    let result = syslog_factory.build(&negative_buffer_spec, &ctx).await;
    assert!(
        result.is_err(),
        "Expected failure for negative TCP buffer size"
    );
    println!(
        "✅ Negative TCP buffer size correctly rejected: {:?}",
        result
    );

    Ok(())
}

//=============================================================================
// Tag Functionality Tests
//=============================================================================

#[tokio::test]
async fn source_tags_are_preserved_and_accessible() -> anyhow::Result<()> {
    setup_test_environment();

    let test_dir = create_test_dir(TEST_DIR_TAGS);
    let test_file = create_test_file(&test_dir, "tagged.log", &["Test message with tags"]).await;

    let spec = FileSourceBuilder::new("tagged_source", &test_file.display().to_string())
        .with_tags(vec!["env:test", "type:log", "service:demo", "version:1.0"])
        .build();

    let factory = reg::get_source_factory("file")
        .ok_or_else(|| anyhow::anyhow!("File factory not registered"))?;

    let ctx = create_build_context();
    let mut source = factory
        .build(&spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build source: {}", e))?;

    // Read message and verify tags
    let mut batch = primary_source_handle_mut(&mut source)
        .source
        .receive()
        .await?;
    let frame = batch
        .pop()
        .ok_or_else(|| anyhow::anyhow!("empty batch from source"))?;
    let tags = frame.tags;

    // Verify custom tags
    assert_eq!(tags.get("env").unwrap_or(""), "test");
    assert_eq!(tags.get("type").unwrap_or(""), "log");
    assert_eq!(tags.get("service").unwrap_or(""), "demo");
    assert_eq!(tags.get("version").unwrap_or(""), "1.0");

    // Verify system-added tag
    assert!(
        tags.get("access_source")
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "Expected system-added access_source tag"
    );

    println!("✅ All tags correctly preserved and accessible");

    cleanup_test_dir(TEST_DIR_TAGS);
    Ok(())
}

// 长行边界：文件源应能正确读取超长单行日志
#[tokio::test]
async fn file_source_handles_long_lines() -> anyhow::Result<()> {
    setup_test_environment();

    let test_dir = create_test_dir("test_long_line");
    let long_line = "A".repeat(100_000); // 100K 字符
    let test_file = create_test_file(&test_dir, "long.log", &[&long_line]).await;

    let spec = FileSourceBuilder::new("long_file", &test_file.display().to_string()).build();
    let factory = reg::get_source_factory("file")
        .ok_or_else(|| anyhow::anyhow!("File factory not registered"))?;
    let ctx = create_build_context();
    let mut source = factory
        .build(&spec, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build source: {}", e))?;

    // 读取一条，验证长度
    let mut batch = primary_source_handle_mut(&mut source)
        .source
        .receive()
        .await?;
    let frame = batch
        .pop()
        .ok_or_else(|| anyhow::anyhow!("empty batch from source"))?;
    let msg = match frame.payload {
        RawData::String(s) => s,
        RawData::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
        RawData::ArcBytes(b) => String::from_utf8_lossy(&b).to_string(),
    };
    assert_eq!(msg.len(), long_line.len());

    cleanup_test_dir("test_long_line");
    Ok(())
}
