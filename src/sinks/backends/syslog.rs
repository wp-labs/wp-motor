use async_trait::async_trait;
use orion_conf::{EnvTomlLoad, ErrorOwe};
use orion_variate::EnvDict;
use serde_json::json;
use std::str::FromStr;
use wp_conf::SafeDefault;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, SinkDefProvider, param_map_to_table};
use wp_conf::structure::Protocol as ConfProtocol;
use wp_conf::structure::SyslogSinkConf;
use wp_connector_api::SinkResult;
use wp_connector_api::{
    AsyncCtrl, AsyncRawDataSink, AsyncRecordSink, SinkBuildCtx, SinkFactory, SinkHandle,
    SinkSpec as ResolvedSinkSpec,
};
use wp_data_fmt::DataFormat; // for format_record
// no extra orion-error/conf helpers needed after route-builder removal

type AnyResult<T> = anyhow::Result<T>;
use crate::protocol::syslog::{EmitMessage, SyslogEncoder};
use crate::sinks::net::transport::{
    BackoffMode, NetSendPolicy, NetWriter, Transport, net_backoff_adaptive,
};

fn syslog_conf_from_spec(spec: &ResolvedSinkSpec) -> AnyResult<SyslogSinkConf> {
    let addr = spec
        .params
        .get("addr")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("syslog.addr must be a string"))?;
    if let Some(i) = spec.params.get("port").and_then(|v| v.as_i64())
        && !(1..=65535).contains(&i)
    {
        anyhow::bail!("syslog.port must be in 1..=65535");
    }
    if let Some(p) = spec.params.get("protocol").and_then(|v| v.as_str()) {
        let v = p.to_ascii_lowercase();
        if v != "udp" && v != "tcp" {
            anyhow::bail!("syslog.protocol must be 'udp' or 'tcp'");
        }
    }
    if let Some(v) = spec.params.get("app_name")
        && v.as_str().is_none()
    {
        anyhow::bail!("syslog.app_name must be a string");
    }
    let port = spec
        .params
        .get("port")
        .and_then(|v| v.as_i64())
        .unwrap_or(514);
    let protocol = spec
        .params
        .get("protocol")
        .and_then(|v| v.as_str())
        .unwrap_or("udp");
    let proto = ConfProtocol::from_str(&protocol.to_ascii_lowercase()).unwrap_or(ConfProtocol::UDP);
    let mut params = wp_connector_api::ParamMap::new();
    params.insert("addr".to_string(), json!(addr));
    params.insert("port".to_string(), json!(port));
    params.insert("protocol".to_string(), json!(proto.to_string()));
    if let Some(app) = spec.params.get("app_name").and_then(|v| v.as_str()) {
        params.insert("app_name".to_string(), json!(app));
    }
    let toml_str = toml::to_string(&param_map_to_table(&params))?;
    let dict = EnvDict::safe_default();
    let conf: SyslogSinkConf = SyslogSinkConf::env_parse_toml(&toml_str, &dict)?;
    Ok(conf)
}

pub struct SyslogSink {
    // Underlying transport writer (UDP/TCP)
    writer: NetWriter,
    // Simple counter to emit first-send debug without spamming logs
    sent_cnt: u64,
    encoder: SyslogEncoder,
    hostname: String,
    app_name: String,
}

impl SyslogSink {
    async fn udp(addr: &str, app_name: Option<String>) -> AnyResult<Self> {
        let writer = NetWriter::connect_udp(addr).await?;
        // Log effective endpoints once (target/local)
        if let Transport::Udp(sock) = &writer.transport {
            if let Ok(local_addr) = sock.local_addr() {
                log::info!(
                    "syslog udp sink connected: target={} local={}",
                    addr,
                    local_addr
                );
            } else {
                log::info!("syslog udp sink connected: target={}", addr);
            }
        } else {
            log::info!("syslog udp sink connected: target={}", addr);
        }
        Ok(Self::with_writer(writer, app_name))
    }
    async fn tcp(addr: &str, app_name: Option<String>, rate_limit_rps: usize) -> AnyResult<Self> {
        // Align to TcpSink: enable backpressure when unlimited
        let mode = if rate_limit_rps == 0 {
            BackoffMode::ForceOn
        } else {
            BackoffMode::ForceOff
        };
        let writer = NetWriter::connect_tcp_with_policy(
            addr,
            NetSendPolicy {
                rate_limit_rps,
                backoff_mode: mode,
                adaptive: net_backoff_adaptive(),
            },
        )
        .await?;
        log::info!("syslog tcp sink connected: target={}", addr);
        Ok(Self::with_writer(writer, app_name))
    }

    fn current_process_name() -> String {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "wp-engine".to_string())
    }

    fn with_writer(writer: NetWriter, app_name: Option<String>) -> Self {
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "localhost".to_string());
        Self {
            writer,
            sent_cnt: 0,
            encoder: SyslogEncoder::new(),
            hostname,
            app_name: app_name.unwrap_or_else(Self::current_process_name),
        }
    }
}

#[async_trait]
impl AsyncCtrl for SyslogSink {
    async fn stop(&mut self) -> SinkResult<()> {
        // For TCP, try graceful shutdown and drain
        if let Transport::Tcp(_) = &self.writer.transport {
            self.writer.shutdown().await?;
            self.writer
                .drain_until_empty(std::time::Duration::from_secs(10))
                .await;
        }
        Ok(())
    }
    async fn reconnect(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

#[async_trait]
impl AsyncRecordSink for SyslogSink {
    async fn sink_record(&mut self, data: &wp_model_core::model::DataRecord) -> SinkResult<()> {
        // Format record to raw text then reuse raw path
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
impl AsyncRawDataSink for SyslogSink {
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        // Format as RFC3164 syslog message
        let mut emit = EmitMessage::new(data);
        emit.priority = 13;
        emit.hostname = Some(self.hostname.as_str());
        emit.app_name = Some(self.app_name.as_str());
        emit.append_newline = matches!(self.writer.transport, Transport::Tcp(_));

        let syslog_msg = self.encoder.encode_rfc3164(&emit);
        let payload = syslog_msg.as_ref();
        if self.sent_cnt == 0 {
            let tag = match self.writer.transport {
                Transport::Udp(_) => "udp",
                Transport::Tcp(_) => "tcp",
                #[cfg(test)]
                Transport::Null => "null",
            };
            log::info!(
                "syslog {} sink first-send: msg_len={} preview='{}'",
                tag,
                payload.len(),
                String::from_utf8_lossy(&payload[..payload.len().min(64)])
            );
        }
        trace_data!(
            "syslog {} sink send seq={} bytes={}",
            match self.writer.transport {
                Transport::Udp(_) => "udp",
                Transport::Tcp(_) => "tcp",
                #[cfg(test)]
                Transport::Null => "null",
            },
            self.sent_cnt + 1,
            payload.len()
        );
        self.writer.write(payload).await?;
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
        let is_tcp = matches!(self.writer.transport, Transport::Tcp(_));
        let mut total = 0usize;
        for s in &data {
            total = total.saturating_add(s.len() + 64);
        }
        let mut buf: Vec<u8> = Vec::with_capacity(total);
        for str_data in data.iter() {
            let mut emit = EmitMessage::new(str_data);
            emit.priority = 13;
            emit.hostname = Some(self.hostname.as_str());
            emit.app_name = Some(self.app_name.as_str());
            emit.append_newline = is_tcp;
            let msg = self.encoder.encode_rfc3164(&emit);
            buf.extend_from_slice(msg.as_ref());
        }
        let record_cnt = data.len();
        trace_data!(
            "syslog {} sink send-batch seq={} records={} bytes={}",
            if is_tcp { "tcp" } else { "udp" },
            self.sent_cnt + 1,
            record_cnt,
            buf.len()
        );
        self.writer.write(&buf).await?;
        self.sent_cnt = self.sent_cnt.saturating_add(1);
        Ok(())
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        for bytes_data in data {
            self.sink_bytes(bytes_data).await?;
        }
        Ok(())
    }
}

pub fn register_factory_syslog() {
    crate::connectors::registry::register_sink_factory(SyslogFactory);
}

// ---- Runtime factory from resolved route (for future decoupling) ----

pub struct SyslogFactory;

#[async_trait]
impl SinkFactory for SyslogFactory {
    fn kind(&self) -> &'static str {
        "syslog"
    }
    fn validate_spec(&self, spec: &ResolvedSinkSpec) -> SinkResult<()> {
        syslog_conf_from_spec(spec).owe_conf()?;
        Ok(())
    }
    async fn build(&self, spec: &ResolvedSinkSpec, _ctx: &SinkBuildCtx) -> SinkResult<SinkHandle> {
        let conf = syslog_conf_from_spec(spec).owe_conf()?;
        let proto = conf.protocol.clone();
        let target = conf.addr_str();
        // Log resolved target to aid diagnosing mismatched params
        log::info!("syslog sink build: target={} protocol={}", target, proto);
        let app_name = conf.resolved_app_name(&spec.name);

        // Build runtime sink directly; pass rate_limit_rps to TCP writer
        let runtime = match proto {
            ConfProtocol::UDP => SyslogSink::udp(target.as_str(), Some(app_name.clone()))
                .await
                .owe_res()?,
            ConfProtocol::TCP => {
                SyslogSink::tcp(target.as_str(), Some(app_name.clone()), _ctx.rate_limit_rps)
                    .await
                    .owe_res()?
            }
        };
        Ok(SinkHandle::new(Box::new(runtime)))
    }
}

impl SinkDefProvider for SyslogFactory {
    fn sink_def(&self) -> ConnectorDef {
        let mut params = wp_connector_api::ParamMap::new();
        params.insert("addr".into(), json!("127.0.0.1"));
        params.insert("port".into(), json!(1514));
        params.insert("protocol".into(), json!("udp"));
        params.insert("strip_header".into(), json!(true));
        params.insert("attach_meta_tags".into(), json!(true));
        params.insert("tcp_recv_bytes".into(), json!(256000));
        ConnectorDef {
            id: "syslog_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec![
                "addr".into(),
                "port".into(),
                "protocol".into(),
                "app_name".into(),
            ],
            default_params: params,
            origin: Some("builtin:syslog_sink".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn syslog_sink_tcp_emits_rfc3164_message() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(lst) => lst,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("bind test listener: {}", e),
        };
        let addr = listener.local_addr().expect("addr");

        let mut sink = SyslogSink::tcp(addr.to_string().as_str(), Some("wpgen".into()), 0)
            .await
            .expect("build tcp sink");

        let accept_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buf = Vec::new();
            use tokio::io::AsyncReadExt;
            stream.read_to_end(&mut buf).await.expect("read");
            buf
        });

        sink.sink_str("syslog body").await.expect("sink str");
        sink.stop().await.expect("stop");

        let bytes = accept_task.await.expect("join");
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(
            text.starts_with("<13>"),
            "missing priority header: {}",
            text
        );
        assert!(text.contains("wpgen"), "app name missing");
        assert!(text.ends_with('\n'), "tcp syslog should end with newline");
        assert!(
            text.trim_end().ends_with("syslog body"),
            "body mismatch: {}",
            text
        );
    }
}
