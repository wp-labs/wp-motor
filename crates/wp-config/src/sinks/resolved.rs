// no local type imports needed
use wp_specs::CoreSinkSpec;

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
