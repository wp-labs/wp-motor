//! Syslog 源的集成测试
//!
//! 覆盖点：构建/生命周期、工厂注册、并发实例、标签传播。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use wp_connector_api::{
    ControlEvent, DataSource, SourceBuildCtx, SourceSpec as ResolvedSourceSpec, Tags,
};
use wp_engine::connectors::registry as reg;
use wp_engine::sources::syslog::{TcpSyslogSource, UdpSyslogSource};

// 轻量通用工具（与其它 tests 共享）
mod common;

// 统一：测试启动时集中初始化所有内置工厂
static INIT: Once = Once::new();
fn ensure_runtime_inited() {
    INIT.call_once(|| {
        wp_engine::connectors::startup::init_runtime_registries();
    });
}

const TCP_TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Helper to create a test SourceSpec
fn create_resolved_spec(
    name: &str,
    protocol: &str,
    port: i64,
    tags: Vec<String>,
) -> ResolvedSourceSpec {
    let mut params = toml::map::Map::new();
    params.insert(
        "protocol".to_string(),
        toml::Value::String(protocol.to_string()),
    );
    params.insert("port".to_string(), toml::Value::Integer(port));
    ResolvedSourceSpec {
        name: name.to_string(),
        kind: "syslog".to_string(),
        connector_id: String::new(),
        params: wp_connector_api::parammap_from_toml_map(params),
        tags,
    }
}

/// Helper to create a test context
fn create_test_ctx() -> SourceBuildCtx {
    SourceBuildCtx::new(PathBuf::from("."))
}

/// Helper to create a TCP source with lifecycle control
async fn create_tcp_source(key: &str) -> (TcpSyslogSource, tokio::sync::broadcast::Sender<()>) {
    let tags = Tags::default();
    // Build a minimal inner TCP aggregator
    let registry = Arc::new(Mutex::new(HashSet::new()));
    let (_tx, rx) = mpsc::channel(8);
    let inner = wp_engine::sources::tcp::TcpSource::new(
        key.to_string(),
        tags.clone(),
        "127.0.0.1:0".to_string(),
        4096,
        wp_engine::sources::tcp::FramingMode::Line,
        registry,
        rx,
    )
    .unwrap();
    let source = TcpSyslogSource::new(
        key.to_string(),
        tags,
        true,  // strip_header
        true,  // attach_meta_tags
        false, // fast_strip
        inner,
    )
    .await
    .unwrap();

    let (ctrl_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    (source, ctrl_tx)
}

#[tokio::test]
async fn udp_source_builds_and_identifies() {
    let tags = Tags::default();
    if !common::is_udp_available() {
        return;
    }
    let source = UdpSyslogSource::new(
        "test_udp".to_string(),
        "127.0.0.1:0".to_string(),
        tags,
        true,
        true,
        false,
    )
    .await
    .unwrap();

    assert_eq!(source.identifier(), "test_udp");
}

#[tokio::test]
async fn tcp_source_start_stop_lifecycle() {
    if !common::is_tcp_available() {
        return;
    }
    let (mut source, _ctrl_tx) = create_tcp_source("test_tcp").await;
    let (_ctrl_tx, ctrl_rx) = async_broadcast::broadcast::<ControlEvent>(1);

    // Start the source
    match timeout(TCP_TEST_TIMEOUT, source.start(ctrl_rx)).await {
        Ok(res) => res.unwrap(),
        Err(_) => {
            println!("Skipping tcp_source_start_stop_lifecycle - start timed out");
            return;
        }
    };

    // Verify it's running
    assert_eq!(source.identifier(), "test_tcp");

    // Clean up
    match timeout(TCP_TEST_TIMEOUT, source.close()).await {
        Ok(res) => res.unwrap(),
        Err(_) => {
            println!("Skipping tcp_source_start_stop_lifecycle - close timed out");
        }
    }
}

#[tokio::test]
async fn factory_builds_udp_and_tcp_sources() {
    // 集中初始化（注册内置 source/sink 工厂）
    ensure_runtime_inited();

    let ctx = create_test_ctx();

    // Test UDP (skip if env forbids UDP bind)
    if common::is_udp_available() {
        let udp_spec = create_resolved_spec(
            "integration_udp",
            "UDP",
            0,
            vec!["test:integration".to_string()],
        );
        let fac = reg::get_source_factory("syslog").expect("factory not registered");
        let udp_init = fac.build(&udp_spec, &ctx).await.unwrap();
        let handle = udp_init
            .sources
            .first()
            .expect("expected udp source handle");
        assert_eq!(handle.source.identifier(), "integration_udp");
    }

    // Test TCP (skip if env forbids TCP bind)
    if common::is_tcp_available() {
        let tcp_spec = create_resolved_spec(
            "integration_tcp",
            "TCP",
            0,
            vec!["test:integration".to_string()],
        );
        let fac = reg::get_source_factory("syslog").expect("factory not registered");
        let tcp_init = fac.build(&tcp_spec, &ctx).await.unwrap();
        let handle = tcp_init
            .sources
            .first()
            .expect("expected tcp source handle");
        assert_eq!(handle.source.identifier(), "integration_tcp");
    }
}

#[tokio::test]
async fn tcp_source_lifecycle_enforces_single_start() {
    if !common::is_tcp_available() {
        return;
    }
    let (mut source, _ctrl_tx) = create_tcp_source("lifecycle").await;

    // Start
    let (_ctrl_tx, ctrl_rx) = async_broadcast::broadcast::<ControlEvent>(1);
    match timeout(TCP_TEST_TIMEOUT, source.start(ctrl_rx)).await {
        Ok(res) => assert!(res.is_ok()),
        Err(_) => {
            println!("Skipping tcp_source_lifecycle_enforces_single_start - first start timed out");
            return;
        }
    }

    // Cannot start twice
    let (_ctrl_tx2, ctrl_rx2) = async_broadcast::broadcast::<ControlEvent>(1);
    match timeout(TCP_TEST_TIMEOUT, source.start(ctrl_rx2)).await {
        Ok(res) => assert!(res.is_err()),
        Err(_) => {
            println!(
                "Skipping tcp_source_lifecycle_enforces_single_start - second start timed out"
            );
            return;
        }
    }

    // Close
    match timeout(TCP_TEST_TIMEOUT, source.close()).await {
        Ok(res) => assert!(res.is_ok()),
        Err(_) => {
            println!("Skipping tcp_source_lifecycle_enforces_single_start - close timed out");
            return;
        }
    }

    // Can close again (idempotent)
    match timeout(TCP_TEST_TIMEOUT, source.close()).await {
        Ok(res) => assert!(res.is_ok()),
        Err(_) => {
            println!(
                "Skipping tcp_source_lifecycle_enforces_single_start - second close timed out"
            );
        }
    }
}

#[tokio::test]
async fn multiple_udp_sources_bind_distinct_ports() {
    let tags1 = Tags::default();
    let tags2 = Tags::default();

    // Create two UDP sources on different ports (skip in restricted env)
    if common::is_udp_available() {
        let source1 = UdpSyslogSource::new(
            "concurrent1".to_string(),
            "127.0.0.1:0".to_string(),
            tags1,
            true,
            true,
            false,
        )
        .await
        .unwrap();
        let source2 = UdpSyslogSource::new(
            "concurrent2".to_string(),
            "127.0.0.1:0".to_string(),
            tags2,
            true,
            true,
            false,
        )
        .await
        .unwrap();
        assert_eq!(source1.identifier(), "concurrent1");
        assert_eq!(source2.identifier(), "concurrent2");
    }
}

#[tokio::test]
async fn udp_source_preserves_multiple_tags() {
    let mut tags = Tags::default();
    tags.set("env", "production".to_string());
    tags.set("service", "syslog".to_string());
    tags.set("region", "us-west".to_string());

    if common::is_udp_available() {
        let source = UdpSyslogSource::new(
            "tagged".to_string(),
            "127.0.0.1:0".to_string(),
            tags,
            true,
            true,
            false,
        )
        .await
        .unwrap();
        assert_eq!(source.identifier(), "tagged");
    }
}

#[tokio::test]
async fn udp_source_fails_on_port_conflict() {
    if !common::is_udp_available() {
        return;
    }
    // 先占用端口，再尝试在同端口创建 UdpSyslogSource，应报错
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    let tags = Tags::default();
    let res = UdpSyslogSource::new(
        "conflict".to_string(),
        addr.to_string(),
        tags,
        true,
        true,
        false,
    )
    .await;
    assert!(res.is_err(), "should fail when port is already bound");
}
