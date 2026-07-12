use wp_specs::CoreSinkSpec;

fn connector_params(core: &CoreSinkSpec) -> wp_connector_api::ParamMap {
    core.params.clone()
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
