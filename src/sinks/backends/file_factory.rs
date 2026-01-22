use super::file::FileSinkSpec;
use crate::sinks::sink_build::build_file_sink;
use async_trait::async_trait;
use orion_error::ErrorOwe;
use serde_json::json;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, SinkDefProvider};
use wp_connector_api::{ParamMap, SinkBuildCtx, SinkFactory, SinkResult, SinkSpec};

pub struct FileFactory;

#[async_trait]
impl SinkFactory for FileFactory {
    fn kind(&self) -> &'static str {
        "file"
    }
    fn validate_spec(&self, spec: &SinkSpec) -> SinkResult<()> {
        FileSinkSpec::from_resolved("file", spec).owe_conf()?;
        Ok(())
    }
    async fn build(
        &self,
        spec: &SinkSpec,
        ctx: &SinkBuildCtx,
    ) -> SinkResult<wp_connector_api::SinkHandle> {
        let resolved = FileSinkSpec::from_resolved("file", spec).owe_conf()?;
        let path = resolved.resolve_path(ctx);
        let fmt = resolved.text_fmt();
        let dummy = wp_conf::structure::SinkInstanceConf::null_new(spec.name.clone(), fmt, None);
        let f = build_file_sink(&dummy, &path).await.owe_res()?;
        Ok(wp_connector_api::SinkHandle::new(Box::new(f)))
    }
}

impl SinkDefProvider for FileFactory {
    fn sink_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("fmt".into(), json!("json"));
        params.insert("base".into(), json!("./data/out_dat"));
        params.insert("file".into(), json!("default.json"));
        ConnectorDef {
            id: "file_json_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["base".into(), "file".into()],
            default_params: params,
            origin: Some("builtin:file".into()),
        }
    }
    fn sink_defs(&self) -> Vec<ConnectorDef> {
        let mut defs = Vec::new();
        let mut params = ParamMap::new();
        params.insert("fmt".into(), json!("json"));
        params.insert("base".into(), json!("./data/out_dat"));
        params.insert("file".into(), json!("default.json"));
        defs.push(ConnectorDef {
            id: "file_json_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["base".into(), "file".into()],
            default_params: params,
            origin: Some("builtin:file".into()),
        });

        let mut params = ParamMap::new();
        params.insert("fmt".into(), json!("proto"));
        params.insert("base".into(), json!("./data/out_dat"));
        params.insert("file".into(), json!("default.proto"));
        defs.push(ConnectorDef {
            id: "file_proto_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["base".into(), "file".into()],
            default_params: params,
            origin: Some("builtin:file".into()),
        });

        let mut params = ParamMap::new();
        params.insert("fmt".into(), json!("kv"));
        params.insert("base".into(), json!("./data/out_dat"));
        params.insert("file".into(), json!("default.kv"));
        defs.push(ConnectorDef {
            id: "file_kv_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["base".into(), "file".into()],
            default_params: params,
            origin: Some("builtin:file".into()),
        });

        defs
    }
}
