use super::blackhole::BlackHoleSink;
use async_trait::async_trait;
use serde_json::json;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, SinkDefProvider};
use wp_connector_api::{ParamMap, SinkFactory, SinkResult};

pub struct BlackHoleFactory;

struct BlackHoleSpec {
    sleep_ms: u64,
}

impl BlackHoleSpec {
    fn from_params(params: &ParamMap) -> Self {
        let sleep_ms = params.get("sleep_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        Self { sleep_ms }
    }
}

#[async_trait]
impl SinkFactory for BlackHoleFactory {
    fn kind(&self) -> &'static str {
        "blackhole"
    }
    fn validate_spec(&self, _spec: &wp_connector_api::SinkSpec) -> SinkResult<()> {
        Ok(())
    }
    async fn build(
        &self,
        spec: &wp_connector_api::SinkSpec,
        _ctx: &wp_connector_api::SinkBuildCtx,
    ) -> SinkResult<wp_connector_api::SinkHandle> {
        let resolved = BlackHoleSpec::from_params(&spec.params);
        Ok(wp_connector_api::SinkHandle::new(Box::new(
            BlackHoleSink::new(resolved.sleep_ms),
        )))
    }
}

impl SinkDefProvider for BlackHoleFactory {
    fn sink_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("sleep_ms".into(), json!(0));
        ConnectorDef {
            id: "blackhole_sink".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Sink,
            allow_override: Vec::new(),
            default_params: params,
            origin: Some("builtin:blackhole".into()),
        }
    }
}
