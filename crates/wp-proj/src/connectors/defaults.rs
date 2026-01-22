use std::sync::Once;
use wp_conf::connectors::{ConnectorDef, ConnectorScope};
use wp_engine::connectors::registry;

pub struct ConnectorTemplate {
    pub scope: ConnectorScope,
    pub file_name: String,
    pub connectors: Vec<ConnectorDef>,
}

pub fn registered_templates() -> Vec<ConnectorTemplate> {
    ensure_factories_registered();
    let mut out = Vec::new();
    out.extend(templates_from_defs(registry::registered_source_defs()));
    out.extend(templates_from_defs(registry::registered_sink_defs()));
    out
}

fn templates_from_defs(mut defs: Vec<ConnectorDef>) -> Vec<ConnectorTemplate> {
    fn slugify(raw: &str) -> String {
        raw.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect()
    }

    defs.sort_by(|a, b| a.id.cmp(&b.id));
    defs.into_iter()
        .enumerate()
        .map(|(idx, def)| ConnectorTemplate {
            scope: def.scope,
            file_name: format!("{:02}-{}.toml", idx, slugify(&def.id)),
            connectors: vec![def],
        })
        .collect()
}

fn ensure_factories_registered() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        wp_engine::connectors::startup::init_runtime_registries();
    });
}
