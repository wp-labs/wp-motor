//! Sink group processing business logic
//!
//! This module provides functions for processing sink groups and
//! collecting line count statistics.

use crate::utils::fs::{count_lines_file, is_match, resolve_path};
use crate::utils::types::{Ctx, GroupAccum, Row, SinkAccum};
use anyhow::Result;
use orion_variate::EnvDict;
use std::path::Path;
use wp_conf::sinks::{load_business_route_confs, load_infra_route_confs};

/// Process a sink group and collect line count statistics
pub fn process_group(
    group_name: &str,
    expect: Option<wp_conf::structure::GroupExpectSpec>,
    sinks: Vec<wp_conf::structure::SinkInstanceConf>,
    framework: bool,
    ctx: &Ctx,
    rows: &mut Vec<Row>,
    total: &mut u64,
) -> GroupAccum {
    let mut gacc = GroupAccum::new(group_name.to_string(), expect);
    for s in sinks.into_iter() {
        if !is_match(s.name().as_str(), &ctx.sink_filters) {
            continue;
        }
        let kind = s.resolved_kind_str();
        if !(kind.eq_ignore_ascii_case("file") || kind.eq_ignore_ascii_case("test_rescue")) {
            continue;
        }
        // Resolve V2 style path
        let params = s.resolved_params_table();
        let raw_path = if params.contains_key("base") || params.contains_key("file") {
            let base = params
                .get("base")
                .and_then(|v| v.as_str())
                .unwrap_or("./data/out_dat");
            let file = params
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("out.dat");
            std::path::Path::new(base).join(file).display().to_string()
        } else {
            params
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("./data/out_dat/out.dat")
                .to_string()
        };
        if let Some(substr) = &ctx.path_like
            && !raw_path.contains(substr)
        {
            continue;
        }
        let prefer = resolve_path(&raw_path, &ctx.work_root);
        match count_lines_file(&prefer) {
            Ok(n) => {
                *total += n;
                let sink_name = s.name().clone();
                let sink_expect = s.expect.clone();
                if !ctx.total_only {
                    rows.push(Row::ok(
                        group_name.to_string(),
                        sink_name.clone(),
                        prefer,
                        framework,
                        n,
                    ));
                }
                gacc.add_sink(SinkAccum {
                    name: sink_name,
                    lines: n,
                    expect: sink_expect,
                });
            }
            Err(_e) => {
                let sink_name = s.name().clone();
                let sink_expect = s.expect.clone();
                if !ctx.total_only {
                    rows.push(Row::err(
                        group_name.to_string(),
                        sink_name.clone(),
                        prefer,
                        framework,
                    ));
                }
                gacc.add_sink(SinkAccum {
                    name: sink_name,
                    lines: 0,
                    expect: sink_expect,
                });
            }
        }
    }
    gacc
}

/// V2: process using resolved route info (no dependency on SinkUseConf)
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ResolvedSinkLite {
    pub name: String,
    pub kind: String,
    pub params: toml::value::Table,
}

/// V2 version of process_group using resolved sink information
pub fn process_group_v2(
    group_name: &str,
    expect: Option<wp_conf::structure::GroupExpectSpec>,
    sinks: Vec<ResolvedSinkLite>,
    framework: bool,
    ctx: &Ctx,
    rows: &mut Vec<Row>,
    total: &mut u64,
) -> GroupAccum {
    let mut gacc = GroupAccum::new(group_name.to_string(), expect);
    for s in sinks.into_iter() {
        if !is_match(s.name.as_str(), &ctx.sink_filters) {
            continue;
        }
        // Only file-like sinks contribute line counts
        if s.kind.eq_ignore_ascii_case("file") {
            // Resolve path: prefer base+file; fallback to path
            let path = if s.params.contains_key("base") || s.params.contains_key("file") {
                let base = s
                    .params
                    .get("base")
                    .and_then(|v| v.as_str())
                    .unwrap_or("./data/out_dat");
                let file = s
                    .params
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("out.dat");
                std::path::Path::new(base).join(file).display().to_string()
            } else {
                s.params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("./data/out_dat/out.dat")
                    .to_string()
            };
            if let Some(substr) = &ctx.path_like
                && !path.contains(substr)
            {
                continue;
            }
            let prefer = resolve_path(&path, &ctx.work_root);
            match count_lines_file(&prefer) {
                Ok(n) => {
                    *total += n;
                    if !ctx.total_only {
                        rows.push(Row::ok(
                            group_name.to_string(),
                            s.name.clone(),
                            prefer,
                            framework,
                            n,
                        ));
                    }
                    gacc.add_sink(SinkAccum {
                        name: s.name,
                        lines: n,
                        expect: None,
                    });
                }
                Err(_e) => {
                    if !ctx.total_only {
                        rows.push(Row::err(
                            group_name.to_string(),
                            s.name.clone(),
                            prefer,
                            framework,
                        ));
                    }
                    gacc.add_sink(SinkAccum {
                        name: s.name,
                        lines: 0,
                        expect: None,
                    });
                }
            }
        }
    }
    gacc
}

/// Collect sink statistics from both business and infra configurations
///
/// This function validates that the sink directory exists, loads both business
/// and infra route configurations, and processes all matching groups.
///
/// # Arguments
/// * `sink_root` - Path to the sink root directory (should contain business.d/ and/or infra.d/)
/// * `ctx` - Processing context with filters and options
/// * `dict` - Environment dictionary for variable substitution
///
/// # Returns
/// A tuple of (rows, total) where rows contains per-sink statistics and total is the sum of all lines
pub fn collect_sink_statistics(
    sink_root: &Path,
    ctx: &Ctx,
    dict: &EnvDict,
) -> Result<(Vec<Row>, u64)> {
    // Validate that sink directories exist
    if !(sink_root.join("business.d").exists() || sink_root.join("infra.d").exists()) {
        anyhow::bail!(
            "缺少 sinks 配置目录：在 '{}' 下未发现 business.d/ 或 infra.d/",
            sink_root.display()
        );
    }

    let mut rows = Vec::new();
    let mut total = 0u64;

    // Process business route configurations
    for conf in load_business_route_confs(sink_root.to_string_lossy().as_ref(), dict)? {
        let g = conf.sink_group;
        if !is_match(g.name().as_str(), &ctx.group_filters) {
            continue;
        }
        let _ = process_group(
            g.name(),
            g.expect().clone(),
            g.sinks().clone(),
            false, // framework = false for business
            ctx,
            &mut rows,
            &mut total,
        );
    }

    // Process infra route configurations
    for conf in load_infra_route_confs(sink_root.to_string_lossy().as_ref(), dict)? {
        let g = conf.sink_group;
        if !is_match(g.name().as_str(), &ctx.group_filters) {
            continue;
        }
        let _ = process_group(
            g.name(),
            g.expect().clone(),
            g.sinks().clone(),
            true, // framework = true for infra
            ctx,
            &mut rows,
            &mut total,
        );
    }

    Ok((rows, total))
}
