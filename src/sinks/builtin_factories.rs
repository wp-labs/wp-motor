use crate::sinks::backends::arrow_file::ArrowFileFactory;
use crate::sinks::backends::arrow_ipc::ArrowIpcFactory;
use crate::sinks::backends::blackhole::BlackHoleSink;
use crate::sinks::backends::blackhole_factory::BlackHoleFactory;
use crate::sinks::backends::file_factory::FileFactory;
use crate::sinks::backends::syslog::SyslogFactory;
use crate::sinks::backends::tcp::TcpFactory;
use crate::sinks::backends::test_rescue::TestRescueFactory;

pub fn register_builtin_factories() {
    wp_core_connectors::registry::register_sink_factory(ArrowFileFactory);
    wp_core_connectors::registry::register_sink_factory(ArrowIpcFactory);
    wp_core_connectors::registry::register_sink_factory(BlackHoleFactory);
    wp_core_connectors::registry::register_sink_factory(FileFactory);
    wp_core_connectors::registry::register_sink_factory(SyslogFactory);
    wp_core_connectors::registry::register_sink_factory(TcpFactory);
    wp_core_connectors::registry::register_sink_factory(TestRescueFactory);
}

#[allow(dead_code)]
pub fn make_blackhole_sink() -> Box<dyn wp_connector_api::AsyncSink> {
    Box::new(BlackHoleSink::new(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::LegacyOwe;
    use orion_error::UnifiedReason;
    use wp_connector_api::{
        AsyncRawDataSink, AsyncRecordSink, SinkFactory, SinkReason, SinkResult,
    };
    use wp_model_core::model::DataRecord;

    #[tokio::test(flavor = "multi_thread")]
    async fn file_factory_supports_fmt_param() -> SinkResult<()> {
        let tmp = std::env::temp_dir().join(format!("wp_file_factory_fmt_{}.log", nano_ts()));
        let mut params = toml::value::Table::new();
        params.insert(
            "base".into(),
            toml::Value::String(tmp.parent().unwrap().to_string_lossy().into()),
        );
        params.insert(
            "file".into(),
            toml::Value::String(tmp.file_name().unwrap().to_string_lossy().into()),
        );
        params.insert("fmt".into(), toml::Value::String("json".into()));
        let spec = wp_connector_api::SinkSpec {
            group: String::new(),
            name: "t".into(),
            kind: "file".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_table(params),
            filter: None,
        };
        let ctx = wp_connector_api::SinkBuildCtx::new(std::env::current_dir().unwrap());
        let init = FileFactory.build(&spec, &ctx).await?;
        let mut sink = init.sink;
        let rec = DataRecord::default();
        AsyncRecordSink::sink_record(sink.as_mut(), &rec).await?;
        AsyncRawDataSink::sink_str(sink.as_mut(), "\n").await?;
        AsyncRawDataSink::sink_str(sink.as_mut(), "").await?;
        drop(sink);
        let body =
            std::fs::read_to_string(tmp).owe(SinkReason::Uvs(UnifiedReason::system_error()))?;
        assert!(body.trim_start().starts_with("{"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn null_factory_is_noop() -> SinkResult<()> {
        let spec = wp_connector_api::SinkSpec {
            group: String::new(),
            name: "n".into(),
            kind: "null".into(),
            connector_id: String::new(),
            params: wp_connector_api::parammap_from_toml_table(toml::value::Table::new()),
            filter: None,
        };
        let ctx = wp_connector_api::SinkBuildCtx::new(std::env::current_dir().unwrap());
        let init = BlackHoleFactory.build(&spec, &ctx).await?;
        let mut sink = init.sink;
        AsyncRawDataSink::sink_str(sink.as_mut(), "hello").await?;
        Ok(())
    }

    fn nano_ts() -> i128 {
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0).into()
    }
}
