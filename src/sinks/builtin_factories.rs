use crate::sinks::backends::blackhole::BlackHoleSink;
use crate::sinks::backends::blackhole_factory::BlackHoleFactory;
use crate::sinks::backends::file_factory::FileFactory;
use crate::sinks::backends::syslog::SyslogFactory;
use crate::sinks::backends::tcp::TcpFactory;
use crate::sinks::backends::test_rescue::TestRescueFactory;
use wp_conf::connectors::{ConnectorDef, SinkDefProvider};

pub fn register_builtin_factories() {
    crate::connectors::registry::register_sink_factory(BlackHoleFactory);
    crate::connectors::registry::register_sink_factory(FileFactory);
    crate::connectors::registry::register_sink_factory(SyslogFactory);
    crate::connectors::registry::register_sink_factory(TcpFactory);
    crate::connectors::registry::register_sink_factory(TestRescueFactory);
}

pub fn builtin_sink_defs() -> Vec<ConnectorDef> {
    let mut defs = Vec::new();
    defs.append(&mut BlackHoleFactory.sink_defs());
    defs.append(&mut FileFactory.sink_defs());
    defs.append(&mut SyslogFactory.sink_defs());
    defs.append(&mut TcpFactory.sink_defs());
    defs.append(&mut TestRescueFactory.sink_defs());
    defs
}

#[allow(dead_code)]
pub fn make_blackhole_sink() -> Box<dyn wp_connector_api::AsyncSink> {
    Box::new(BlackHoleSink::new(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wp_connector_api::{AsyncRawDataSink, AsyncRecordSink, SinkFactory};
    use wp_model_core::model::DataRecord;

    #[tokio::test(flavor = "multi_thread")]
    async fn file_factory_supports_fmt_param() -> anyhow::Result<()> {
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
        let body = std::fs::read_to_string(tmp)?;
        assert!(body.trim_start().starts_with("{"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn null_factory_is_noop() -> anyhow::Result<()> {
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
