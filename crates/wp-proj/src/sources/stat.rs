use orion_conf::ErrorWith;
use orion_error::{ErrorWrapAs, UvsFrom};
use orion_variate::EnvDict;
use std::path::Path;
use wp_cli_core::Ctx;
use wp_engine::facade::config;
use wp_error::RunReason;
use wp_error::run_error::RunResult;

/// Result structure for file source statistics
///
/// This struct contains the results of analyzing file-based data sources,
/// including the work root directory path and a detailed line count report.
pub struct SourceStatResult {
    /// The resolved work root directory path
    pub work_root: String,
    /// Optional report containing line count statistics for each file source
    pub report: Option<wp_cli_core::SrcLineReport>,
}

/// Statistics module for file-based sources
///
/// This module provides functionality to analyze and gather statistics
/// from file-based data sources configured in the project.
///
pub fn stat_file_sources(work_root: &str, dict: &EnvDict) -> RunResult<SourceStatResult> {
    // Load engine configuration to get source settings
    let (cm, main) = config::load_warp_engine_confs(work_root, dict)
        .wrap_as(
            RunReason::from_conf(),
            "load engine config for source stats failed",
        )
        .with(work_root)
        .doing("load engine config for source stats")?;

    // Resolve the actual work root path
    let resolved = cm.work_root_path();

    // Create context for source statistics collection
    let ctx = Ctx::new(resolved.clone());

    // Gather statistics from file sources using business layer
    let report = wp_cli_core::list_file_sources_with_lines(Path::new(&resolved), &main, &ctx, dict)
        .wrap_as(RunReason::from_conf(), "collect source file stats failed")
        .with(resolved.as_str())
        .doing("collect source file stats")?;

    // Return the statistics result
    Ok(SourceStatResult {
        work_root: resolved,
        report,
    })
}
