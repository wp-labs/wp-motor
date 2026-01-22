//! Syslog data collection module
//!
//! This module provides comprehensive syslog data collection capabilities with support for:
//! - UDP/TCP syslog protocols
//! - RFC 3164 and RFC 5424 message formats
//! - Automatic message framing and normalization
//! - Configurable metadata attachment
//! - Connection management for TCP sources
//!
//! # Example
//!
//! ```toml
//! [[sources]]
//! key = "syslog_input"
//! connect = "syslog_main"
//! enable = true
//! tags = ["env:production", "type:syslog"]
//! params_override = {
//!     addr = "0.0.0.0",
//!     port = 514,
//!     protocol = "udp",          # or "tcp"
//!     # 头部处理模式：keep(保留头)、strip(去掉头)、parse(解析并注入标签，且去掉头)
//!     header_mode = "parse",
//!     tcp_recv_bytes = 10485760  # TCP receive buffer size (bytes)
//! }
//! ```

pub mod config;
pub mod constants;
pub mod factory;
pub mod normalize;
pub mod tcp_source;
pub mod udp_source;

// Re-export public API
pub use config::{Protocol, SyslogSourceSpec};
pub use factory::SyslogSourceFactory;
pub use tcp_source::TcpSyslogSource;
pub use udp_source::UdpSyslogSource;
mod tcp_tests;

use crate::connectors::registry;

/// Register the syslog source factory
pub fn register_syslog_factory() {
    registry::register_source_factory(factory::SyslogSourceFactory::new());
}

// Auto-register removed: registration is centralized in connectors::startup

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use wp_connector_api::{SourceBuildCtx, SourceFactory, SourceSpec as ResolvedSourceSpec, Tags};

    fn ctx() -> SourceBuildCtx {
        SourceBuildCtx::new(PathBuf::from("."))
    }

    #[tokio::test]
    async fn test_factory_udp_minimal() {
        if std::net::UdpSocket::bind("127.0.0.1:0").is_err() {
            return;
        }
        // Arrange: minimal UDP config with ephemeral port
        let mut params = toml::map::Map::new();
        params.insert("protocol".into(), toml::Value::String("UDP".into()));
        params.insert("port".into(), toml::Value::Integer(0));
        let spec = ResolvedSourceSpec {
            name: "syslog_u1".into(),
            kind: "syslog".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params),
            tags: vec!["env:test".into()],
        };
        let fac = factory::SyslogSourceFactory::new();
        let init = fac
            .build(&spec, &ctx())
            .await
            .expect("factory build failed");
        assert_eq!(init.sources.len(), 1);
        assert_eq!(init.sources[0].source.identifier(), "syslog_u1");
    }

    #[test]
    fn test_syslog_config_defaults() {
        let params = toml::map::Map::new();
        let config =
            SyslogSourceSpec::from_params(&wp_connector_api::parammap_from_toml_map(params))
                .expect("syslog defaults");
        assert_eq!(config.addr, "0.0.0.0");
        assert_eq!(config.port, 514);
        assert_eq!(config.protocol, Protocol::Udp);
        assert_eq!(config.tcp_recv_bytes, 10_485_760);
        assert_eq!(config.address(), "0.0.0.0:514");
    }

    #[test]
    fn test_syslog_config_custom_addr() {
        let mut params = toml::map::Map::new();
        params.insert(
            "addr".to_string(),
            toml::Value::String("127.0.0.1".to_string()),
        );

        let config =
            SyslogSourceSpec::from_params(&wp_connector_api::parammap_from_toml_map(params))
                .expect("custom addr");
        assert_eq!(config.addr, "127.0.0.1");
        assert_eq!(config.address(), "127.0.0.1:514");
    }

    #[test]
    fn test_syslog_config_tcp_protocol() {
        let mut params = toml::map::Map::new();
        params.insert(
            "protocol".to_string(),
            toml::Value::String("TCP".to_string()),
        );

        let config =
            SyslogSourceSpec::from_params(&wp_connector_api::parammap_from_toml_map(params))
                .expect("tcp protocol");
        assert_eq!(config.protocol, Protocol::Tcp);
    }

    #[tokio::test]
    async fn test_udp_source_creation() {
        if std::net::UdpSocket::bind("127.0.0.1:0").is_err() {
            return;
        }
        let tags = Tags::default();
        // Use port 0 to get a random available port
        let result = udp_source::UdpSyslogSource::new(
            "test".to_string(),
            "127.0.0.1:0".to_string(),
            tags,
            true,
            true,
            false,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tcp_source_creation() {
        let tags = Tags::default();
        // Build a minimal inner tcp aggregator
        let pool = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = crate::sources::tcp::TcpSource::new(
            "test".to_string(),
            tags.clone(),
            "127.0.0.1:0".to_string(),
            4096,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let result =
            tcp_source::TcpSyslogSource::new("test".to_string(), tags, true, true, false, inner)
                .await;
        assert!(result.is_ok());
    }
}
