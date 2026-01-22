use crate::structure::Protocol;
use educe::Educe;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ToStructError, UvsValidationFrom};
use orion_variate::EnvDict;
use std::path::Path;

#[derive(Educe, Serialize, Deserialize, PartialEq, Clone)]
#[educe(Debug, Default)]
pub struct SyslogSinkConf {
    #[educe(Default = "127.0.0.1")]
    pub(crate) addr: String,
    #[educe(Default = 514)]
    pub(crate) port: usize,
    pub protocol: Protocol,
    #[serde(default)]
    pub app_name: Option<String>,
}

impl SyslogSinkConf {
    pub fn addr_str(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }

    pub fn resolved_app_name(&self, fallback: &str) -> String {
        self.app_name
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| fallback.to_string())
    }
}

// ---------------- Syslog Source Config ----------------

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct SyslogSourceConf {
    pub key: String,
    pub addr: String,
    pub port: u16,
    pub protocol: Protocol,
    #[serde(default = "SyslogSourceConf::tcp_read_bytes_default")]
    pub tcp_recv_bytes: usize,
    pub enable: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for SyslogSourceConf {
    fn default() -> Self {
        Self {
            key: "syslog_1".to_string(),
            addr: "0.0.0.0".to_string(),
            port: 514,
            protocol: Protocol::UDP,
            tcp_recv_bytes: 10_485_760, // 10 MiB default read bytes per cycle
            enable: false,
            tags: Vec::new(),
        }
    }
}

impl SyslogSourceConf {
    pub fn addr_str(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }
    fn tcp_read_bytes_default() -> usize {
        10_485_760
    }
}

impl crate::structure::Validate for SyslogSourceConf {
    fn validate(&self) -> OrionConfResult<()> {
        if self.addr.trim().is_empty() {
            return ConfIOReason::from_validation("syslog.addr must not be empty").err_result();
        }
        if self.port == 0 {
            return ConfIOReason::from_validation("syslog.port must be in 1..=65535").err_result();
        }
        if matches!(self.protocol, Protocol::TCP) && self.tcp_recv_bytes == 0 {
            return ConfIOReason::from_validation("syslog.tcp_recv_bytes must be > 0 for TCP")
                .err_result();
        }
        Ok(())
    }
}

// ============================================================================
// ConfigLoader trait implementation for unified loading interface
// ============================================================================

impl crate::loader::traits::ConfigLoader for SyslogSinkConf {
    fn config_type_name() -> &'static str {
        "Syslog Sink"
    }

    fn load_from_str(content: &str, _base: &Path, _dict: &EnvDict) -> OrionConfResult<Self> {
        let conf: SyslogSinkConf = toml::from_str(content)
            .map_err(|e| ConfIOReason::from_validation(format!("TOML 解析失败: {}", e)).to_err())?;

        Ok(conf)
    }

    // SyslogSinkConf 没有实现 Validate trait，所以使用默认的空验证
}

impl crate::loader::traits::ConfigLoader for SyslogSourceConf {
    fn config_type_name() -> &'static str {
        "Syslog Source"
    }

    fn load_from_str(content: &str, _base: &Path, _dict: &EnvDict) -> OrionConfResult<Self> {
        let conf: SyslogSourceConf = toml::from_str(content)
            .map_err(|e| ConfIOReason::from_validation(format!("TOML 解析失败: {}", e)).to_err())?;

        Ok(conf)
    }

    fn validate(&self) -> OrionConfResult<()> {
        <Self as crate::structure::Validate>::validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::traits::ConfigLoader;
    use crate::test_support::ForTest;

    #[test]
    fn config_loader_syslog_sink_from_str() {
        let toml = r#"
addr = "192.168.1.1"
port = 514
protocol = "udp"
"#;
        let result = SyslogSinkConf::load_from_str(toml, Path::new("/"), &EnvDict::test_default());

        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let conf = result.unwrap();
        assert_eq!(conf.addr, "192.168.1.1");
        assert_eq!(conf.port, 514);
    }

    #[test]
    fn config_loader_syslog_sink_from_path() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
addr = "10.0.0.1"
port = 1514
protocol = "tcp"
app_name = "test_app"
"#
        )
        .unwrap();

        let result = SyslogSinkConf::load_from_path(file.path(), &EnvDict::test_default());

        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let conf = result.unwrap();
        assert_eq!(conf.addr, "10.0.0.1");
        assert_eq!(conf.port, 1514);
        assert_eq!(conf.app_name, Some("test_app".to_string()));
    }

    #[test]
    fn config_loader_syslog_source_from_str() {
        let toml = r#"
key = "syslog_test"
addr = "0.0.0.0"
port = 514
protocol = "udp"
enable = true
"#;
        let result =
            SyslogSourceConf::load_from_str(toml, Path::new("/"), &EnvDict::test_default());

        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let conf = result.unwrap();
        assert_eq!(conf.key, "syslog_test");
        assert_eq!(conf.port, 514);
        assert!(conf.enable);
    }

    #[test]
    fn config_loader_syslog_source_validation() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
key = "test"
addr = ""
port = 0
protocol = "udp"
enable = true
"#
        )
        .unwrap();

        let result = SyslogSourceConf::load_from_path(file.path(), &EnvDict::test_default());

        assert!(result.is_err(), "无效配置应该验证失败");
    }
}
