// no local type imports needed
use wp_specs::CoreSinkSpec;

const CONNECTOR_HIDDEN_RUNTIME_PARAMS: &[&str] = &["wp_meta_disable"];

fn connector_params(core: &CoreSinkSpec) -> wp_connector_api::ParamMap {
    let mut params = core.params.clone();
    for key in CONNECTOR_HIDDEN_RUNTIME_PARAMS {
        params.remove(*key);
    }
    params
}

/// Bridge CoreSinkSpec to ResolvedSinkSpec (flattened params, empty group/connector)
pub fn core_to_resolved(core: &CoreSinkSpec) -> wp_connector_api::SinkSpec {
    wp_connector_api::SinkSpec {
        group: String::new(),
        name: core.name.clone(),
        kind: core.kind.clone(),
        connector_id: String::new(),
        params: core.params.clone(),
        filter: core.filter.clone(),
    }
}

/// Bridge CoreSinkSpec to connector-facing ResolvedSinkSpec.
///
/// Runtime-only metadata params stay on SinkInstanceConf for the engine/runtime,
/// but are hidden from connector factories so strict connectors only receive
/// their own business params.
pub fn core_to_connector_resolved(core: &CoreSinkSpec) -> wp_connector_api::SinkSpec {
    wp_connector_api::SinkSpec {
        group: String::new(),
        name: core.name.clone(),
        kind: core.kind.clone(),
        connector_id: String::new(),
        params: connector_params(core),
        filter: core.filter.clone(),
    }
}

/// Bridge CoreSinkSpec to ResolvedSinkSpec with given group and connector id
pub fn core_to_resolved_with(
    core: &CoreSinkSpec,
    group: impl Into<String>,
    connector_id: impl Into<String>,
) -> wp_connector_api::SinkSpec {
    let g = group.into();
    let cid = connector_id.into();
    debug_assert!(
        !cid.is_empty(),
        "connector_id should be non-empty when resolving with connectors (group='{}', name='{}')",
        g,
        core.name
    );
    wp_connector_api::SinkSpec {
        group: g,
        name: core.name.clone(),
        kind: core.kind.clone(),
        connector_id: cid,
        params: core.params.clone(),
        filter: core.filter.clone(),
    }
}

/// Bridge CoreSinkSpec to connector-facing ResolvedSinkSpec with group and connector id.
pub fn core_to_connector_resolved_with(
    core: &CoreSinkSpec,
    group: impl Into<String>,
    connector_id: impl Into<String>,
) -> wp_connector_api::SinkSpec {
    let g = group.into();
    let cid = connector_id.into();
    debug_assert!(
        !cid.is_empty(),
        "connector_id should be non-empty when resolving with connectors (group='{}', name='{}')",
        g,
        core.name
    );
    wp_connector_api::SinkSpec {
        group: g,
        name: core.name.clone(),
        kind: core.kind.clone(),
        connector_id: cid,
        params: connector_params(core),
        filter: core.filter.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn connector_resolved_hides_runtime_metadata_params() {
        let mut core = CoreSinkSpec {
            name: "s".into(),
            kind: "file".into(),
            params: Default::default(),
            filter: None,
            tags: Vec::new(),
        };
        core.params.insert("file".into(), json!("out.json"));
        core.params
            .insert("wp_meta_disable".into(), json!(["wp_event_id"]));

        let runtime = core_to_resolved(&core);
        assert!(runtime.params.contains_key("wp_meta_disable"));

        let connector = core_to_connector_resolved(&core);
        assert_eq!(
            connector.params.get("file").and_then(|v| v.as_str()),
            Some("out.json")
        );
        assert!(!connector.params.contains_key("wp_meta_disable"));
    }
}
