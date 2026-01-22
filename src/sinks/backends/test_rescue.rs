use crate::sinks::backends::file::FileSinkSpec;
use crate::sinks::sink_build::build_file_sink;
use crate::sinks::{ASinkTestProxy, HealthController};
use async_trait::async_trait;
use orion_error::ErrorOwe;
use serde_json::json;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, SinkDefProvider};
use wp_connector_api::{ParamMap, SinkBuildCtx, SinkFactory, SinkResult, SinkSpec};

pub struct TestRescueFactory;

#[async_trait]
impl SinkFactory for TestRescueFactory {
    fn kind(&self) -> &'static str {
        "test_rescue"
    }
    fn validate_spec(&self, spec: &SinkSpec) -> SinkResult<()> {
        FileSinkSpec::from_resolved("test_rescue", spec).owe_conf()?;
        Ok(())
    }
    async fn build(
        &self,
        spec: &SinkSpec,
        ctx: &SinkBuildCtx,
    ) -> SinkResult<wp_connector_api::SinkHandle> {
        let resolved = FileSinkSpec::from_resolved("test_rescue", spec).owe_conf()?;
        let path = resolved.resolve_path(ctx);
        let fmt = resolved.text_fmt();
        let dummy = wp_conf::structure::SinkInstanceConf::null_new(spec.name.clone(), fmt, None);
        let f = build_file_sink(&dummy, &path).await.owe_res()?;
        let stg = HealthController::new();
        let proxy = ASinkTestProxy::new(f, stg);
        Ok(wp_connector_api::SinkHandle::new(Box::new(proxy)))
    }
}

impl SinkDefProvider for TestRescueFactory {
    fn sink_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("fmt".into(), json!("kv"));
        params.insert("base".into(), json!("./data/out_dat"));
        params.insert("file".into(), json!("default.kv"));
        ConnectorDef {
            id: "file_rescue_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["base".into(), "file".into()],
            default_params: params,
            origin: Some("builtin:test_rescue".into()),
        }
    }
}
