use orion_conf::UvsConfFrom;
use orion_error::ToStructError;
use serde_json::json;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, ParamMap};
use wp_conf_base::ConfParser;
use wp_connector_api::{
    AcceptorHandle, SourceBuildCtx, SourceFactory, SourceHandle, SourceMeta, SourceResult,
    SourceSpec as ResolvedSourceSpec, SourceSvcIns, Tags,
};
use wp_connector_api::{SourceDefProvider, SourceReason};

use super::TcpAcceptor;
use super::config::TcpSourceSpec;
use super::source::TcpSource;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use wp_conf::limits::tcp_reader_batch_channel_cap;

// TcpSourceSpec moved to tcp/config.rs for clearer separation of concerns

pub struct TcpSourceFactory;

#[async_trait::async_trait]
impl SourceFactory for TcpSourceFactory {
    fn kind(&self) -> &'static str {
        "tcp"
    }

    fn validate_spec(&self, spec: &ResolvedSourceSpec) -> SourceResult<()> {
        let res: anyhow::Result<()> = (|| {
            if let Err(e) = Tags::validate(&spec.tags) {
                anyhow::bail!("Invalid tags: {}", e);
            }
            TcpSourceSpec::from_params(&spec.params)?;
            Ok(())
        })();
        res.map_err(|e| SourceReason::from_conf(e.to_string()).to_err())
    }

    async fn build(
        &self,
        spec: &ResolvedSourceSpec,
        _ctx: &SourceBuildCtx,
    ) -> SourceResult<SourceSvcIns> {
        let fut = async {
            let conf = TcpSourceSpec::from_params(&spec.params)?;
            let tags = Tags::from_parse(&spec.tags);

            let connection_registry = Arc::new(Mutex::new(HashSet::<u64>::new()));
            let mut instance_reg_txs = Vec::with_capacity(conf.instances);
            let mut source_handles = Vec::with_capacity(conf.instances);

            for idx in 0..conf.instances {
                let (reader_reg_tx, reader_reg_rx) = mpsc::channel(tcp_reader_batch_channel_cap());
                instance_reg_txs.push(reader_reg_tx);

                let key = if conf.instances == 1 {
                    spec.name.clone()
                } else {
                    format!("{}#{}", spec.name, idx + 1)
                };
                let source = TcpSource::new(
                    key.clone(),
                    tags.clone(),
                    conf.address(),
                    conf.tcp_recv_bytes,
                    conf.framing,
                    connection_registry.clone(),
                    reader_reg_rx,
                )?;

                let mut meta = SourceMeta::new(key.clone(), spec.kind.clone());
                for (k, v) in tags.iter() {
                    meta.tags.set(k, v);
                }
                if conf.instances > 1 {
                    meta.tags.set("instance".to_string(), (idx + 1).to_string());
                }

                source_handles.push(SourceHandle::new(Box::new(source), meta));
            }

            let acceptor = TcpAcceptor::new(
                spec.name.clone(),
                conf.address(),
                1000,
                connection_registry,
                instance_reg_txs,
            );

            let acceptor_handle = AcceptorHandle::new(spec.name.clone(), Box::new(acceptor));

            Ok(SourceSvcIns::new()
                .with_sources(source_handles)
                .with_acceptor(acceptor_handle))
        };

        fut.await
            .map_err(|e: anyhow::Error| SourceReason::from_conf(e.to_string()).to_err())
    }
}

impl SourceDefProvider for TcpSourceFactory {
    fn source_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("addr".into(), json!("0.0.0.0"));
        params.insert("port".into(), json!(9000));
        params.insert("framing".into(), json!("auto"));
        params.insert("tcp_recv_bytes".into(), json!(256_000));
        params.insert("instances".into(), json!(1));
        ConnectorDef {
            id: "tcp_src".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Source,
            allow_override: vec![
                "addr".into(),
                "port".into(),
                "framing".into(),
                "tcp_recv_bytes".into(),
                "instances".into(),
            ],
            default_params: params,
            origin: Some("builtin:tcp_source".into()),
        }
    }
}

/// 注册 TCP 源工厂（集中由引擎启动入口调用）
pub fn register_tcp_factory() {
    crate::connectors::registry::register_source_factory(TcpSourceFactory);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;
    use wp_connector_api::{SourceFactory, SourceSpec as ResolvedSourceSpec};
    use wp_parse_api::RawData;

    #[tokio::test]
    async fn factory_builds_with_ephemeral_port() {
        let fac = TcpSourceFactory;
        let spec = ResolvedSourceSpec {
            name: "tcp_test".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: {
                let mut t = toml::map::Map::new();
                t.insert("addr".into(), toml::Value::String("127.0.0.1".into()));
                t.insert("port".into(), toml::Value::Integer(0));
                wp_connector_api::parammap_from_toml_map(t)
            },
            tags: vec!["env:test".into()],
        };
        let ctx = SourceBuildCtx::new(std::path::PathBuf::from("."));
        let svc = fac.build(&spec, &ctx).await.expect("build tcp source");
        assert_eq!(svc.sources.len(), 1);
        assert_eq!(svc.sources[0].source.identifier(), "tcp_test");
    }

    #[tokio::test]
    async fn end_to_end_line_and_len() {
        // 在受限沙箱（无网络权限）环境下跳过
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }
        // pick a free port first
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let fac = TcpSourceFactory;
        // build line-framed source
        let mut params = toml::map::Map::new();
        params.insert("addr".into(), toml::Value::String("127.0.0.1".into()));
        params.insert("port".into(), toml::Value::Integer(port as i64));
        params.insert("framing".into(), toml::Value::String("line".into()));
        let spec = ResolvedSourceSpec {
            name: "tcp_e2e".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params.clone()),
            tags: vec![],
        };
        let ctx = SourceBuildCtx::new(std::env::current_dir().unwrap());
        let mut svc = fac.build(&spec, &ctx).await.unwrap();
        let mut handle = svc.sources.remove(0);
        let (_tx, rx) = async_broadcast::broadcast::<wp_connector_api::ControlEvent>(1);
        handle.source.start(rx).await.unwrap();

        // send a line
        let mut s = TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        use tokio::io::AsyncWriteExt as _;
        s.write_all(b"hello\n").await.unwrap();
        // read one event
        let mut batch = handle.source.receive().await.unwrap();
        assert_eq!(batch.len(), 1);
        let ev = batch.pop().unwrap();
        let got = match ev.payload {
            RawData::String(s) => s,
            RawData::Bytes(b) => String::from_utf8_lossy(&b).into_owned(),
            RawData::ArcBytes(b) => String::from_utf8_lossy(&b).into_owned(),
        };
        assert_eq!(got, "hello");
        handle.source.close().await.unwrap();

        // len-framed
        let mut params2 = params;
        params2.insert("framing".into(), toml::Value::String("len".into()));
        let spec2 = ResolvedSourceSpec {
            name: "tcp_e2e2".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params2),
            tags: vec![],
        };
        let mut svc2 = fac.build(&spec2, &ctx).await.unwrap();
        let mut h2 = svc2.sources.remove(0);
        let (_t2, r2) = async_broadcast::broadcast::<wp_connector_api::ControlEvent>(1);
        h2.source.start(r2).await.unwrap();
        let mut s2 = TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        s2.write_all(b"5 world").await.unwrap();
        let mut batch2 = h2.source.receive().await.unwrap();
        assert_eq!(batch2.len(), 1);
        let ev2 = batch2.pop().unwrap();
        let got2 = match ev2.payload {
            RawData::String(s) => s,
            RawData::Bytes(b) => String::from_utf8_lossy(&b).into_owned(),
            RawData::ArcBytes(b) => String::from_utf8_lossy(&b).into_owned(),
        };
        assert_eq!(got2, "world");
        h2.source.close().await.unwrap();
    }

    #[tokio::test]
    async fn factory_builds_multiple_instances() {
        let fac = TcpSourceFactory;
        let spec = ResolvedSourceSpec {
            name: "tcp_multi".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: {
                let mut t = toml::map::Map::new();
                t.insert("addr".into(), toml::Value::String("127.0.0.1".into()));
                t.insert("port".into(), toml::Value::Integer(0));
                t.insert("instances".into(), toml::Value::Integer(2));
                wp_connector_api::parammap_from_toml_map(t)
            },
            tags: vec!["env:test".into()],
        };
        let ctx = SourceBuildCtx::new(std::path::PathBuf::from("."));
        let svc = fac.build(&spec, &ctx).await.expect("multi build");
        assert_eq!(svc.sources.len(), 2);
        let mut idents: Vec<String> = svc
            .sources
            .iter()
            .map(|handle| handle.source.identifier())
            .collect();
        idents.sort();
        assert_eq!(
            idents,
            vec!["tcp_multi#1".to_string(), "tcp_multi#2".to_string()]
        );
    }

    #[tokio::test]
    async fn multi_instance_acceptor_handles_many_connections() {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }

        // 预留端口后释放，用于后续 TcpAcceptor 监听
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let fac = TcpSourceFactory;
        let mut params = toml::map::Map::new();
        params.insert("addr".into(), toml::Value::String("127.0.0.1".into()));
        params.insert("port".into(), toml::Value::Integer(port as i64));
        params.insert("framing".into(), toml::Value::String("line".into()));
        params.insert("instances".into(), toml::Value::Integer(3));
        let spec = ResolvedSourceSpec {
            name: "tcp_wide".into(),
            kind: "tcp".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_map(params),
            tags: vec![],
        };
        let ctx = SourceBuildCtx::new(std::env::current_dir().unwrap());
        let mut svc = fac.build(&spec, &ctx).await.unwrap();

        // 启动 acceptor，允许控制事件停止
        let mut acceptor_handle = svc.acceptor.take().expect("tcp acceptor present");
        let (accept_stop_tx, accept_stop_rx) =
            async_broadcast::broadcast::<wp_connector_api::ControlEvent>(8);
        let accept_task = tokio::spawn(async move {
            acceptor_handle
                .acceptor
                .accept_connection(accept_stop_rx)
                .await
                .expect("accept loop should exit cleanly");
        });

        // 启动所有 reader 实例
        let mut handles = svc.sources;
        for handle in handles.iter_mut() {
            let (_tx, rx) = async_broadcast::broadcast::<wp_connector_api::ControlEvent>(1);
            handle.source.start(rx).await.unwrap();
        }

        // 建立 5 个独立 TCP 连接（>3），确保不会因为 reader 数量受限而掉线
        let addr = format!("127.0.0.1:{port}");
        use std::io::ErrorKind;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::time::{Duration as TokioDuration, sleep};
        let mut streams = Vec::new();
        for idx in 0..5 {
            let mut stream = loop {
                match tokio::net::TcpStream::connect(&addr).await {
                    Ok(s) => break s,
                    Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
                        sleep(TokioDuration::from_millis(20)).await;
                        continue;
                    }
                    Err(e) => panic!("tcp connect failed: {e}"),
                }
            };
            stream
                .write_all(format!("msg{idx}\\n").as_bytes())
                .await
                .unwrap();
            stream.flush().await.unwrap();
            streams.push(stream);
        }
        for mut stream in streams {
            let mut buf = [0u8; 1];
            let _ = stream.read(&mut buf).await;
        }

        use std::collections::HashSet;
        use std::time::{Duration, Instant};
        let expected: HashSet<String> = (0..5).map(|i| format!("msg{i}")).collect();
        let mut received = HashSet::new();
        let timeout = Instant::now() + Duration::from_secs(5);

        while received.len() < expected.len() && Instant::now() < timeout {
            for handle in handles.iter_mut() {
                if received.len() >= expected.len() {
                    break;
                }
                if let Ok(Ok(batch)) =
                    tokio::time::timeout(Duration::from_millis(200), handle.source.receive()).await
                {
                    for frame in batch {
                        let text = match frame.payload {
                            RawData::String(s) => s,
                            RawData::Bytes(b) => String::from_utf8_lossy(&b).into_owned(),
                            RawData::ArcBytes(b) => String::from_utf8_lossy(&b).into_owned(),
                        };
                        if !text.is_empty() {
                            received.insert(text);
                        }
                    }
                }
            }
        }

        assert_eq!(received, expected, "每个连接都应被任一实例接住");

        for handle in handles.iter_mut() {
            handle.source.close().await.unwrap();
        }
        let _ = accept_stop_tx
            .broadcast(wp_connector_api::ControlEvent::Stop)
            .await;
        accept_task.await.unwrap();
    }
}
