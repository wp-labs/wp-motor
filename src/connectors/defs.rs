use wp_conf::connectors::ConnectorDef;
use wp_connector_api::SourceDefProvider;

use crate::sources::{file::FileSourceFactory, syslog::SyslogSourceFactory, tcp::TcpSourceFactory};

pub fn builtin_sink_defs() -> Vec<ConnectorDef> {
    crate::sinks::builtin_factories::builtin_sink_defs()
}

pub fn builtin_source_defs() -> Vec<ConnectorDef> {
    let mut defs = Vec::new();
    defs.append(&mut FileSourceFactory.source_defs());
    defs.append(&mut SyslogSourceFactory::default().source_defs());
    defs.append(&mut TcpSourceFactory.source_defs());
    defs
}
