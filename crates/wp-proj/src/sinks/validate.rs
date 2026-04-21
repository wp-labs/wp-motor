use std::path::Path;

use super::stat::{SinkStatFilters, build_ctx, ensure_sink_dirs};
use orion_conf::ErrorWith;
use orion_error::{ErrorWrapAs, UvsFrom};
use orion_variate::EnvDict;
use wp_cli_core::{
    self as wlib,
    utils::stats::{StatsFile, load_stats_file},
};
use wp_engine::facade::config;
use wp_error::RunReason;
use wp_error::run_error::RunResult;

pub struct ValidateContext {
    pub groups: Vec<wlib::GroupAccum>,
    pub stats: Option<StatsFile>,
    pub input_from_sources: Option<u64>,
}

pub fn prepare_validate_context(
    filters: &SinkStatFilters<'_>,
    stats_file: Option<&str>,
    dict: &EnvDict,
) -> RunResult<ValidateContext> {
    let (cm, main) = config::load_warp_engine_confs(filters.work_root, dict)?;
    let ctx = build_ctx(&cm.work_root_path(), filters);
    let sink_root = Path::new(&cm.work_root_path()).join(main.sink_root());
    ensure_sink_dirs(&sink_root, main.sink_root())
        .with(&sink_root)
        .doing("validate sink directory layout")?;
    let (_rows, groups, _total) =
        wp_cli_core::business::observability::build_groups_v2(&sink_root, &ctx, dict)
            .wrap_as(RunReason::from_conf(), "build sink group stats failed")
            .with(&sink_root)
            .doing("build sink group stats")?;
    let stats = stats_file.and_then(|p| load_stats_file(Path::new(p)));
    let input_from_sources =
        wp_cli_core::total_input_from_wpsrc(Path::new(&cm.work_root_path()), &main, &ctx, dict)
            .wrap_as(
                RunReason::from_conf(),
                "count total input from file sources failed",
            )
            .with(cm.work_root_path())
            .doing("count total input from file sources")?;
    Ok(ValidateContext {
        groups,
        stats,
        input_from_sources,
    })
}
