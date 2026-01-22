use async_trait::async_trait;
use orion_conf::ErrorOwe;
use serde_json::json;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, ParamMap, SinkDefProvider};
use wp_connector_api::SinkResult;
use wp_connector_api::{
    AsyncCtrl, AsyncRawDataSink, AsyncRecordSink, SinkBuildCtx, SinkFactory, SinkHandle,
    SinkSpec as ResolvedSinkSpec,
};
use wp_data_fmt::DataFormat; // for format_record

type AnyResult<T> = anyhow::Result<T>;
use crate::sinks::net::transport::{BackoffMode, NetSendPolicy, NetWriter, net_backoff_adaptive};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Framing {
    Line,
    Len,
}

#[derive(Clone, Debug)]
struct TcpSinkSpec {
    addr: String,
    port: u16,
    framing: Framing,
}

impl TcpSinkSpec {
    fn from_resolved(spec: &ResolvedSinkSpec) -> AnyResult<Self> {
        let addr = match spec.params.get("addr").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => anyhow::bail!("tcp.addr must be a string"),
        };
        let port = match spec.params.get("port").and_then(|v| v.as_i64()) {
            Some(p) if (1..=65535).contains(&p) => p as u16,
            Some(_) => anyhow::bail!("tcp.port must be in 1..=65535"),
            None => 9000,
        };
        let framing = spec
            .params
            .get("framing")
            .and_then(|v| v.as_str())
            .unwrap_or("line");
        let framing = match framing.to_ascii_lowercase().as_str() {
            "len" | "length" => Framing::Len,
            "line" => Framing::Line,
            _ => anyhow::bail!("tcp.framing must be 'line' or 'len'"),
        };
        Self::ensure_bool(spec, "max_backoff")?;
        Self::ensure_bool(spec, "sendq_backpressure")?;
        Ok(Self {
            addr,
            port,
            framing,
        })
    }

    fn ensure_bool(spec: &ResolvedSinkSpec, key: &str) -> AnyResult<()> {
        if let Some(v) = spec.params.get(key)
            && v.as_bool().is_none()
        {
            anyhow::bail!("tcp.{key} must be a boolean");
        }
        Ok(())
    }

    fn target_addr(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }
}

// Max seconds to wait for kernel TCP send-queue to drain at shutdown
const TCP_DRAIN_MAX_SECS: u64 = 10;

pub struct TcpSink {
    writer: NetWriter,
    framing: Framing,
    sent_cnt: u64,
}

impl TcpSink {
    async fn connect(spec: &TcpSinkSpec, rate_limit_rps: usize) -> AnyResult<Self> {
        let target = spec.target_addr();
        // 根据限速目标决定策略：
        // - rate_limit_rps == 0（无限速）：启用背压能力（ForceOn）——仅在水位/包型需要时退让；
        // - rate_limit_rps > 0（限速）：关闭背压能力（ForceOff），避免与源端限速叠加造成双重退让。
        let mode = if rate_limit_rps == 0 {
            BackoffMode::ForceOn
        } else {
            BackoffMode::ForceOff
        };
        let writer = NetWriter::connect_tcp_with_policy(
            &target,
            NetSendPolicy {
                rate_limit_rps,
                backoff_mode: mode,
                adaptive: net_backoff_adaptive(),
            },
        )
        .await?;
        log::info!("tcp sink connected: target={}", target);
        Ok(Self {
            writer,
            framing: spec.framing,
            sent_cnt: 0,
        })
    }
}

#[async_trait]
impl AsyncCtrl for TcpSink {
    async fn stop(&mut self) -> SinkResult<()> {
        // Gracefully shutdown write side so peer can drain
        self.writer.shutdown().await?;
        // Best-effort kernel send-queue drain, capped 10s
        self.writer
            .drain_until_empty(std::time::Duration::from_secs(TCP_DRAIN_MAX_SECS))
            .await;
        Ok(())
    }
    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncRecordSink for TcpSink {
    async fn sink_record(&mut self, data: &wp_model_core::model::DataRecord) -> SinkResult<()> {
        // 复用 Raw 格式化，随后走 raw 路径
        let raw = wp_data_fmt::Raw::new().format_record(data);
        AsyncRawDataSink::sink_str(self, raw.as_str()).await
    }

    async fn sink_records(
        &mut self,
        data: Vec<std::sync::Arc<wp_model_core::model::DataRecord>>,
    ) -> SinkResult<()> {
        for record in data {
            self.sink_record(&record).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncRawDataSink for TcpSink {
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        let payload = build_payload(data, self.framing);
        if self.sent_cnt == 0 {
            log::info!(
                "tcp sink first-send: framing={:?} msg_len={} preview='{}'",
                self.framing,
                payload.len(),
                &data.chars().take(64).collect::<String>()
            );
        }
        self.writer.write(&payload).await?;
        self.sent_cnt = self.sent_cnt.saturating_add(1);
        Ok(())
    }
    async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
        Ok(())
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        if data.is_empty() {
            return Ok(());
        }

        // 批量处理：根据 framing 模式决定如何合并数据
        match self.framing {
            Framing::Line => {
                // Line 模式：合并所有字符串，确保每个都有换行符
                let mut total_len = 0;
                for str_data in &data {
                    total_len += str_data.len();
                    if str_data.as_bytes().last().is_none_or(|&b| b != b'\n') {
                        total_len += 1;
                    }
                }

                let mut buffer = Vec::with_capacity(total_len);
                for str_data in &data {
                    buffer.extend_from_slice(str_data.as_bytes());
                    if str_data.as_bytes().last().is_none_or(|&b| b != b'\n') {
                        buffer.push(b'\n');
                    }
                }

                // 一次性发送所有数据
                self.writer.write(&buffer).await?;
                self.sent_cnt = self.sent_cnt.saturating_add(1);
            }
            Framing::Len => {
                // Length 模式：每个消息需要独立的长度前缀，不能简单合并
                // 但仍然可以批量发送
                let mut buffers = Vec::with_capacity(data.len());
                for str_data in &data {
                    buffers.push(build_payload(str_data, self.framing));
                }

                // 合并所有消息
                let total_len: usize = buffers.iter().map(|b| b.len()).sum();
                let mut combined = Vec::with_capacity(total_len);
                for buffer in buffers {
                    combined.extend_from_slice(&buffer);
                }

                self.writer.write(&combined).await?;
                self.sent_cnt = self.sent_cnt.saturating_add(data.len() as u64);
            }
        }

        Ok(())
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        if data.is_empty() {
            return Ok(());
        }

        // u8 数据的 sink_bytes 实际上什么都不做，这里保持一致
        // 如果需要实际的实现，可以根据 framing 模式处理
        for bytes_data in data {
            self.sink_bytes(bytes_data).await?;
        }
        Ok(())
    }
}

// 小工具：将 Vec<u8> 适配为 fmt::Write
fn buf_writer(buf: &mut Vec<u8>) -> impl std::fmt::Write + '_ {
    struct W<'a>(&'a mut Vec<u8>);
    impl<'a> std::fmt::Write for W<'a> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.extend_from_slice(s.as_bytes());
            Ok(())
        }
    }
    W(buf)
}

pub struct TcpFactory;

#[async_trait]
impl SinkFactory for TcpFactory {
    fn kind(&self) -> &'static str {
        "tcp"
    }
    fn validate_spec(&self, spec: &ResolvedSinkSpec) -> SinkResult<()> {
        TcpSinkSpec::from_resolved(spec).owe_conf()?;
        Ok(())
    }
    async fn build(&self, spec: &ResolvedSinkSpec, ctx: &SinkBuildCtx) -> SinkResult<SinkHandle> {
        let resolved = TcpSinkSpec::from_resolved(spec).owe_conf()?;
        // Internal defaults: no ACK; auto-drain at shutdown.
        // 限速目标：由 SinkBuildCtx 统一传入，TcpSink 内部据此构建 SendPolicy。
        let runtime = TcpSink::connect(&resolved, ctx.rate_limit_rps)
            .await
            .owe_res()?;
        Ok(SinkHandle::new(Box::new(runtime)))
    }
}

impl SinkDefProvider for TcpFactory {
    fn sink_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("addr".into(), json!("127.0.0.1"));
        params.insert("port".into(), json!(9000));
        params.insert("framing".into(), json!("line"));
        ConnectorDef {
            id: "tcp_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["addr".into(), "port".into(), "framing".into()],
            default_params: params,
            origin: Some("builtin:tcp_sink".into()),
        }
    }
}

// No external ACK mode; keep sink simple

// --- pure helper for payload framing ---
fn build_payload(data: &str, framing: Framing) -> Vec<u8> {
    match framing {
        Framing::Line => {
            if data.ends_with('\n') {
                data.as_bytes().to_vec()
            } else {
                [data.as_bytes(), b"\n"].concat()
            }
        }
        Framing::Len => {
            let mut buf = Vec::with_capacity(16 + data.len());
            let _ = std::fmt::Write::write_fmt(
                &mut buf_writer(&mut buf),
                format_args!("{} ", data.len()),
            );
            buf.extend_from_slice(data.as_bytes());
            buf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use wp_connector_api::{AsyncRawDataSink, SinkFactory};

    #[tokio::test(flavor = "multi_thread")]
    async fn tcp_sink_sends_line() -> anyhow::Result<()> {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return Ok(());
        }
        // server
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let srv = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 16];
            let n = s.read(&mut buf).await.unwrap();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        // sink
        let fac = TcpFactory;
        let mut params = toml::map::Map::new();
        params.insert("addr".into(), toml::Value::String("127.0.0.1".into()));
        params.insert("port".into(), toml::Value::Integer(port as i64));
        params.insert("framing".into(), toml::Value::String("line".into()));
        let spec = wp_connector_api::SinkSpec {
            group: String::new(),
            name: "t".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params),
            filter: None,
        };
        let ctx = wp_connector_api::SinkBuildCtx::new(std::env::current_dir().unwrap());
        let mut h = fac.build(&spec, &ctx).await?;
        AsyncRawDataSink::sink_str(h.sink.as_mut(), "abc").await?;
        let body = srv.await.unwrap();
        assert_eq!(body, "abc\n");
        Ok(())
    }

    #[test]
    fn payload_builder_line_and_len() {
        let p1 = build_payload("abc", Framing::Line);
        assert_eq!(p1, b"abc\n");
        let p2 = build_payload("hello", Framing::Len);
        assert_eq!(p2, b"5 hello");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tcp_sink_sends_len() -> anyhow::Result<()> {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return Ok(());
        }
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let srv = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 32];
            let n = s.read(&mut buf).await.unwrap();
            buf[..n].to_vec()
        });
        let fac = TcpFactory;
        let mut params = toml::map::Map::new();
        params.insert("addr".into(), toml::Value::String("127.0.0.1".into()));
        params.insert("port".into(), toml::Value::Integer(port as i64));
        params.insert("framing".into(), toml::Value::String("len".into()));
        let spec = wp_connector_api::SinkSpec {
            group: String::new(),
            name: "t".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params),
            filter: None,
        };
        let ctx = wp_connector_api::SinkBuildCtx::new(std::env::current_dir().unwrap());
        let mut h = fac.build(&spec, &ctx).await?;
        AsyncRawDataSink::sink_str(h.sink.as_mut(), "hello").await?;
        let body = srv.await.unwrap();
        assert_eq!(body, b"5 hello");
        Ok(())
    }
}
