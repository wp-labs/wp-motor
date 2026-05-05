use crate::compat::LegacyOwe;
use crate::sinks::backends::file::FileSinkSpec;
use crate::sinks::sink_build::build_file_sink;
use crate::sinks::{ASinkTestProxy, HealthController};
use async_trait::async_trait;
use orion_error::UnifiedReason;
use wp_conf::connectors::{ConnectorDef, SinkDefProvider};
use wp_connector_api::{SinkBuildCtx, SinkFactory, SinkReason, SinkResult, SinkSpec};

pub struct TestRescueFactory;

#[async_trait]
impl SinkFactory for TestRescueFactory {
    fn kind(&self) -> &'static str {
        "test_rescue"
    }
    fn validate_spec(&self, spec: &SinkSpec) -> SinkResult<()> {
        FileSinkSpec::from_resolved("test_rescue", spec)
            .owe(SinkReason::Uvs(UnifiedReason::core_conf()))?;
        Ok(())
    }
    async fn build(
        &self,
        spec: &SinkSpec,
        ctx: &SinkBuildCtx,
    ) -> SinkResult<wp_connector_api::SinkHandle> {
        let resolved = FileSinkSpec::from_resolved("test_rescue", spec)
            .owe(SinkReason::Uvs(UnifiedReason::core_conf()))?;
        let path = resolved.resolve_path(ctx);
        let fmt = resolved.text_fmt();
        let dummy = wp_conf::structure::SinkInstanceConf::null_new(spec.name.clone(), fmt, None);
        let f = build_file_sink(&dummy, &path)
            .await
            .owe(SinkReason::Uvs(UnifiedReason::resource_error()))?;
        let stg = HealthController::new();
        let proxy = ASinkTestProxy::new(f, stg);
        Ok(wp_connector_api::SinkHandle::new(Box::new(proxy)))
    }
}

impl SinkDefProvider for TestRescueFactory {
    fn sink_def(&self) -> ConnectorDef {
        wp_core_connectors::builtin::sink_def("file_rescue_sink")
            .expect("builtin sink def missing: file_rescue_sink")
    }
}
