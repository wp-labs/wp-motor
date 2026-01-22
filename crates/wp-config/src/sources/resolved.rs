use wp_specs::CoreSourceSpec;

/// Bridge CoreSourceSpec to ResolvedSourceSpec (flattened params, empty connector)
pub fn core_to_resolved(core: &CoreSourceSpec) -> wp_connector_api::SourceSpec {
    wp_connector_api::SourceSpec {
        name: core.name.clone(),
        kind: core.kind.clone(),
        connector_id: String::new(),
        params: core.params.clone(),
        tags: core.tags.clone(),
    }
}

/// Bridge CoreSourceSpec to ResolvedSourceSpec with given connector id
pub fn core_to_resolved_with(
    core: &CoreSourceSpec,
    connector_id: impl Into<String>,
) -> wp_connector_api::SourceSpec {
    wp_connector_api::SourceSpec {
        name: core.name.clone(),
        kind: core.kind.clone(),
        connector_id: connector_id.into(),
        params: core.params.clone(),
        tags: core.tags.clone(),
    }
}
