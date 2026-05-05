use super::framing::{DEFAULT_TCP_RECV_BYTES, FramingMode};
use wp_connector_api::{SourceReason, SourceResult};

#[derive(Debug, Clone)]
pub struct TcpSourceSpec {
    pub addr: String,
    pub port: u16,
    pub tcp_recv_bytes: usize,
    pub framing: FramingMode,
    pub instances: usize,
}

pub const DEFAULT_TCP_SOURCE_INSTANCES: usize = 1;
pub const MAX_TCP_SOURCE_INSTANCES: usize = 16;

impl TcpSourceSpec {
    pub fn from_params(params: &wp_connector_api::ParamMap) -> SourceResult<Self> {
        let addr = params
            .get("addr")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0.0")
            .to_string();
        let port_i64 = params.get("port").and_then(|v| v.as_i64()).unwrap_or(9000);
        if !(0..=65535).contains(&port_i64) {
            return Err(SourceReason::core_conf().err_detail(format!(
                "invalid port: {}",
                port_i64
            )));
        }
        let port = port_i64 as u16;
        let tcp_recv_bytes = params
            .get("tcp_recv_bytes")
            .and_then(|v| v.as_i64())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_TCP_RECV_BYTES as i64) as usize;
        if tcp_recv_bytes == 0 {
            return Err(SourceReason::core_conf().err_detail("tcp_recv_bytes must be > 0"));
        }
        let framing = match params
            .get("framing")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_ascii_lowercase()
            .as_str()
        {
            "line" => FramingMode::Line,
            "len" | "length" => FramingMode::Len,
            "auto" => FramingMode::Auto,
            other => {
                return Err(SourceReason::core_conf()
                    .err_detail(format!("invalid framing: {} (expect auto|line|len)", other)));
            }
        };

        let instances = params
            .get("instances")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_TCP_SOURCE_INSTANCES as i64);
        if !(1..=MAX_TCP_SOURCE_INSTANCES as i64).contains(&instances) {
            return Err(SourceReason::core_conf().err_detail(format!(
                "tcp.instances must be between 1 and {}",
                MAX_TCP_SOURCE_INSTANCES
            )));
        }
        let instances = instances as usize;

        Ok(Self {
            addr,
            port,
            tcp_recv_bytes,
            framing,
            instances,
        })
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }
}
