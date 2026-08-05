mod options;
mod report;
mod types;

pub use options::{CheckComponent, CheckComponents, CheckOptions};
pub use types::{Cell, ConnectorCounts, Row, SourceBreakdown};

use report::{build_detail_table, component_cells};
use std::path::Path;
use std::path::PathBuf;

use super::warp::WarpProject;
use crate::compat::UvsFrom;
use crate::types::CheckStatus;
use orion_error::StructError;
use orion_error::conversion::ToStructError;
use orion_error::reason::{DomainReason, ErrorCode};
use orion_variate::EnvDict;
use wp_cli_core::business::connectors::{sinks as sink_connectors, sources as source_connectors};
use wp_conf::generator::wpgen::WpGenConfig;
use wp_conf::sinks::io::load_connectors_for;
use wp_engine::facade::config::{self as cfg_face, ENGINE_CONF_FILE};
use wp_error::run_error::{RunError, RunResult};

/// 检查工程（与 `wproj prj check` 语义一致）。
/// 执行全面的项目检查，包括所有组件。
pub fn check_with(
    project: &WarpProject,
    opts: &CheckOptions,
    comps: &CheckComponents,
    dict: &EnvDict,
) -> RunResult<()> {
    let (targets, default_root) = resolve_targets(project, opts);
    let rows = collect_rows(project, &targets, &default_root, opts, comps, dict);
    let stats = summarize_components(&rows, comps);

    render_output(&rows, &stats, opts, comps);

    if has_failures(&rows, comps) {
        let failed_targets = rows.iter().filter(|row| row.count_failures() > 0).count();
        return Err(wp_error::run_error::RunReason::from_conf()
            .to_err()
            .with_detail(format!(
                "project check failed: {} target(s) reported validation errors",
                failed_targets
            )));
    }
    Ok(())
}

fn component_stat_value(enabled: bool, count: &ComponentCount) -> serde_json::Value {
    use serde_json::json;
    if enabled {
        json!({ "passed": count.ok, "total": count.total })
    } else {
        serde_json::Value::Null
    }
}

fn resolve_targets(project: &WarpProject, opts: &CheckOptions) -> (Vec<PathBuf>, String) {
    let default_root = if opts.work_root.trim().is_empty() {
        project.work_root().to_string()
    } else {
        opts.work_root.clone()
    };

    let targets = if opts.work_root.trim().is_empty() {
        vec![project.paths().root.clone()]
    } else {
        vec![PathBuf::from(&opts.work_root)]
    };

    (targets, default_root)
}

fn collect_rows(
    project: &WarpProject,
    targets: &[PathBuf],
    default_root: &str,
    opts: &CheckOptions,
    comps: &CheckComponents,
    dict: &EnvDict,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for work in targets.iter() {
        let wrs = if work.as_os_str().is_empty() {
            default_root.to_string()
        } else {
            work.to_string_lossy().to_string()
        };
        let row = evaluate_target(project, &wrs, opts, comps, dict);
        rows.push(row);
    }
    rows
}

fn evaluate_target(
    project: &WarpProject,
    wrs: &str,
    opts: &CheckOptions,
    comps: &CheckComponents,
    dict: &EnvDict,
) -> Row {
    // Resolve to absolute early: load_warp_engine_confs changes CWD globally,
    // so subsequent relative paths would resolve against the wrong directory.
    let wrs = if Path::new(wrs).is_absolute() {
        wrs.to_string()
    } else {
        std::path::absolute(wrs)
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(wrs)
            })
            .to_string_lossy()
            .to_string()
    };
    let mut row = Row::new(wrs.clone());

    if comps.engine {
        row.conf = match cfg_face::load_warp_engine_confs(&wrs, dict) {
            Ok((cm, _)) => {
                row.conf_detail = Some(cm.config_path_string(ENGINE_CONF_FILE));
                Cell::success()
            }
            Err(e) => Cell::failure(describe_run_error(&e)),
        };
        if !row.conf.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.conf = Cell::skipped();
    }

    if comps.sources {
        let sources_check = project
            .sources_c()
            .check(dict)
            .map_err(|e| describe_run_error(&e))
            .map(|_| ());
        let check_cell = Cell::from_result(sources_check);
        // Use the unified check() for both syntax and runtime validation
        row.source_checks = Some(SourceBreakdown {
            syntax: check_cell.clone(),
            runtime: check_cell.clone(),
        });
        row.sources = check_cell;
        if !row.sources.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.sources = Cell::skipped();
        row.source_checks = None;
    }

    if comps.connectors {
        row.connectors = Cell::from_result(
            project
                .connectors()
                .check(&wrs, dict)
                .map(|_| ())
                .map_err(|e| describe_run_error(&e)),
        );
        match collect_connector_counts(&wrs, dict) {
            Ok(stats) => row.connector_counts = Some(stats),
            Err(_e) => {
                row.connector_counts = None;
            }
        }
        if !row.connectors.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.connectors = Cell::skipped();
        row.connector_counts = None;
    }

    if comps.sinks {
        row.sinks = Cell::from_result(
            project
                .sinks_c()
                .check(dict)
                .map_err(|e| describe_run_error(&e))
                .map(|_| ()),
        );
        if !row.sinks.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.sinks = Cell::skipped();
    }

    if comps.wpl {
        row.wpl = Cell::from_result(
            project
                .wpl()
                .check(dict)
                .map_err(|e| describe_run_error(&e))
                .map(|_| ()),
        );
        if !row.wpl.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.wpl = Cell::skipped();
    }

    if comps.oml {
        row.oml = match project.oml().check(dict) {
            Ok(check_status) => match check_status {
                CheckStatus::Suc => Cell::success(),
                CheckStatus::Miss => Cell::success_with_message("OML 文件缺失".to_string()),
                CheckStatus::Error => Cell::failure("OML 检查错误".to_string()),
            },
            Err(e) => Cell::failure(describe_run_error(&e)),
        };
        if !row.oml.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.oml = Cell::skipped();
    }

    if comps.semantic_dict {
        row.semantic_dict = match check_semantic_dict_config(Path::new(&wrs), dict) {
            Ok(Some(result)) => Cell::success_with_warnings(result.message, result.warnings),
            Ok(None) => Cell::success_with_message("使用内置词典".to_string()),
            Err(e) => Cell::failure(e),
        };
        if !row.semantic_dict.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.semantic_dict = Cell::skipped();
    }

    if comps.intranet_nets {
        row.intranet_nets = match check_intranet_nets_config(Path::new(&wrs), dict) {
            Ok(Some(msg)) => Cell::success_with_message(msg),
            Ok(None) => Cell::success_with_message("使用内置网段".to_string()),
            Err(e) => Cell::failure(e),
        };
        if !row.intranet_nets.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.intranet_nets = Cell::skipped();
    }

    if comps.wpgen {
        row.wpgen = check_wpgen_config(&wrs, dict);
        if !row.wpgen.ok && opts.fail_fast {
            return row;
        }
    } else {
        row.wpgen = Cell::skipped();
    }

    row
}

/// 检查语义词典配置
struct SemanticDictCheckView {
    message: String,
    warnings: Vec<String>,
}

fn check_semantic_dict_config(
    work_root: &Path,
    dict: &EnvDict,
) -> Result<Option<SemanticDictCheckView>, String> {
    let (_, main_conf) = cfg_face::load_warp_engine_confs(&work_root.to_string_lossy(), dict)
        .map_err(|e| describe_run_error(&e))?;

    let primary = PathBuf::from(main_conf.knowledge_root()).join("semantic_dict.toml");
    if primary.exists() {
        return oml::check_semantic_dict_config_detailed(Some(&primary)).map(|result| {
            result.map(|result| SemanticDictCheckView {
                message: shorten_semantic_dict_message(&result.message, work_root, &primary),
                warnings: result.warnings,
            })
        });
    }

    let fallback = work_root.join("knowledge/semantic_dict.toml");
    if fallback.exists() {
        return oml::check_semantic_dict_config_detailed(Some(&fallback)).map(|result| {
            result.map(|result| SemanticDictCheckView {
                message: shorten_semantic_dict_message(&result.message, work_root, &fallback),
                warnings: result.warnings,
            })
        });
    }

    Ok(None)
}

/// 检查内网网段配置（从项目的 knowdb.toml 解析 `[intranet_nets]` 节）
fn check_intranet_nets_config(work_root: &Path, dict: &EnvDict) -> Result<Option<String>, String> {
    let (_, main_conf) = cfg_face::load_warp_engine_confs(&work_root.to_string_lossy(), dict)
        .map_err(|e| describe_run_error(&e))?;

    let primary = PathBuf::from(main_conf.knowledge_root()).join("knowdb.toml");
    if primary.exists() {
        return wp_knowledge::intranet_nets::check_intranet_nets_config(&primary);
    }

    let fallback = work_root.join("models/knowledge/knowdb.toml");
    if fallback.exists() {
        return wp_knowledge::intranet_nets::check_intranet_nets_config(&fallback);
    }

    Ok(None)
}

fn shorten_semantic_dict_message(msg: &str, work_root: &Path, config_path: &Path) -> String {
    let short_path = config_path
        .strip_prefix(work_root)
        .ok()
        .map(|p| format!("./{}", p.to_string_lossy()))
        .unwrap_or_else(|| config_path.to_string_lossy().to_string());

    let full_path = config_path.to_string_lossy();
    let replaced = msg.replace(full_path.as_ref(), &short_path);
    replaced
        .strip_prefix("语义词典配置有效: ")
        .unwrap_or(&replaced)
        .to_string()
}

/// 已知的已移除字段 → 替代建议
const WPGEN_REMOVED_FIELDS: &[(&str, &str)] = &[
    (
        "mode",
        "\"mode\" 字段已不再使用，为了避免配置错误，请删除该字段",
    ),
    (
        "duration_secs",
        "\"duration_secs\" 字段已不再使用，为了避免理解错误，请删除该字段",
    ),
];

/// 对 WpGen 加载错误附加迁移提示
fn enhance_wpgen_error(err_msg: &str) -> String {
    let mut enhanced = err_msg.to_string();
    for (field, hint) in WPGEN_REMOVED_FIELDS {
        if err_msg.contains(&format!("unknown field `{}`", field)) {
            enhanced.push_str(&format!("\n  Hint: {}", hint));
            break;
        }
    }
    enhanced
}

/// 检查 wpgen 配置（conf/wpgen.toml）
fn check_wpgen_config(work_root: &str, dict: &EnvDict) -> Cell {
    let path = Path::new(work_root).join("conf").join("wpgen.toml");
    if !path.exists() {
        return Cell::success_with_message("wpgen.toml not found (optional)".into());
    }
    let config = match WpGenConfig::load_from_path(&path, dict) {
        Ok(c) => c,
        Err(e) => {
            let raw = format!("{:#}", e);
            return Cell::failure(enhance_wpgen_error(&raw));
        }
    };

    let mut errors: Vec<String> = Vec::new();

    // 检查 rule_root 路径存在性
    if let Some(ref rule_root) = config.generator.rule_root {
        let rule_path = Path::new(rule_root);
        let resolved = if rule_path.is_absolute() {
            rule_path.to_path_buf()
        } else {
            Path::new(work_root).join(rule_path)
        };
        if !resolved.exists() {
            errors.push(format!(
                "wpgen.generator.rule_root '{}' does not exist (resolved to '{}')",
                rule_root,
                resolved.display()
            ));
        }
    }

    // 检查 sample_pattern 是合法 glob
    if let Some(ref pattern) = config.generator.sample_pattern
        && glob::Pattern::new(pattern).is_err()
    {
        errors.push(format!(
            "wpgen.generator.sample_pattern '{}' is not a valid glob pattern",
            pattern
        ));
    }

    // 检查 output.connect 引用的 sink connector 是否存在；字段必填由 WpGenConfig::validate 统一处理。
    if let Some(ref connect) = config.output.connect
        && let Err(e) = validate_wpgen_sink_connector(work_root, connect, dict)
    {
        errors.push(e);
    }

    // 检查 logging.file_path 父目录存在性
    if let Some(ref file_path) = config.logging.file_path {
        let log_path = Path::new(file_path);
        let resolved = if log_path.is_absolute() {
            log_path.to_path_buf()
        } else {
            Path::new(work_root).join(log_path)
        };
        if let Some(parent) = resolved.parent()
            && !parent.exists()
        {
            eprintln!(
                "  ⚠ wpgen.logging.file_path parent directory '{}' does not exist (resolved from '{}')",
                parent.display(),
                file_path
            );
        }
    }

    if errors.is_empty() {
        Cell::success()
    } else {
        Cell::failure(errors.join("; "))
    }
}

/// 验证 wpgen 的 output.connect 引用的 sink connector 存在
fn validate_wpgen_sink_connector(
    work_root: &str,
    connect_id: &str,
    dict: &EnvDict,
) -> Result<(), String> {
    let (_, eng_conf) =
        cfg_face::load_warp_engine_confs(work_root, dict).map_err(|e| describe_run_error(&e))?;
    let configured_root = eng_conf.sinks_root();
    let sink_root_path = Path::new(configured_root);
    let resolved_root = if sink_root_path.is_absolute() {
        sink_root_path.to_path_buf()
    } else {
        Path::new(work_root).join(sink_root_path)
    };
    let start_root = resolved_root.to_string_lossy().to_string();

    let connectors = load_connectors_for(&start_root, dict)
        .map_err(|e| format!("failed to load sink connectors: {:#}", e))?;

    if !connectors.contains_key(connect_id) {
        let mut known: Vec<String> = connectors.keys().cloned().collect();
        known.sort();
        return Err(format!(
            "wpgen.output.connect '{}' not found in sink connectors at '{}'; available: {}",
            connect_id,
            resolved_root.display(),
            known.join(", ")
        ));
    }
    Ok(())
}

fn describe_run_error(err: &RunError) -> String {
    describe_struct_error(err)
}

fn describe_struct_error<R>(err: &StructError<R>) -> String
where
    R: DomainReason + ErrorCode + std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
{
    if let Some(detail) = err
        .detail()
        .as_ref()
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
    {
        return detail.to_string();
    }

    for frame in err.source_frames() {
        if let Some(detail) = frame
            .detail
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            return detail.to_string();
        }
        if let Some(path) = frame
            .path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            return format!("{} ({})", frame.message, path);
        }
    }

    if let Some(path) = err.target_path().filter(|p| !p.trim().is_empty()) {
        return format!("{} ({})", err.reason(), path);
    }

    err.display_chain()
}

#[derive(Default, Clone, Copy)]
struct ComponentCount {
    ok: usize,
    total: usize,
}

impl ComponentCount {
    fn record(&mut self, passed: bool) {
        self.total += 1;
        if passed {
            self.ok += 1;
        }
    }
}

#[derive(Default)]
struct SummaryCounts {
    conf: ComponentCount,
    connectors: ComponentCount,
    sources: ComponentCount,
    sinks: ComponentCount,
    wpl: ComponentCount,
    oml: ComponentCount,
    semantic_dict: ComponentCount,
    intranet_nets: ComponentCount,
    wpgen: ComponentCount,
}

fn summarize_components(rows: &[Row], comps: &CheckComponents) -> SummaryCounts {
    let mut stats = SummaryCounts::default();
    for r in rows {
        if comps.engine {
            stats.conf.record(r.conf.ok);
        }
        if comps.connectors {
            stats.connectors.record(r.connectors.ok);
        }
        if comps.sources {
            stats.sources.record(r.sources.ok);
        }
        if comps.sinks {
            stats.sinks.record(r.sinks.ok);
        }
        if comps.wpl {
            stats.wpl.record(r.wpl.ok);
        }
        if comps.oml {
            stats.oml.record(r.oml.ok);
        }
        if comps.semantic_dict {
            stats.semantic_dict.record(r.semantic_dict.ok);
        }
        if comps.intranet_nets {
            stats.intranet_nets.record(r.intranet_nets.ok);
        }
        if comps.wpgen {
            stats.wpgen.record(r.wpgen.ok);
        }
    }
    stats
}

fn render_output(
    rows: &[Row],
    stats: &SummaryCounts,
    opts: &CheckOptions,
    comps: &CheckComponents,
) {
    if opts.json {
        use serde_json::{Map, Value, json};
        let mut stat = Map::new();
        stat.insert("total".into(), Value::from(rows.len()));
        stat.insert(
            "conf".into(),
            component_stat_value(comps.engine, &stats.conf),
        );
        stat.insert(
            "connectors".into(),
            component_stat_value(comps.connectors, &stats.connectors),
        );
        stat.insert(
            "sources".into(),
            component_stat_value(comps.sources, &stats.sources),
        );
        stat.insert(
            "sinks".into(),
            component_stat_value(comps.sinks, &stats.sinks),
        );
        stat.insert("wpl".into(), component_stat_value(comps.wpl, &stats.wpl));
        stat.insert("oml".into(), component_stat_value(comps.oml, &stats.oml));
        stat.insert(
            "semantic_dict".into(),
            component_stat_value(comps.semantic_dict, &stats.semantic_dict),
        );
        stat.insert(
            "intranet_nets".into(),
            component_stat_value(comps.intranet_nets, &stats.intranet_nets),
        );
        stat.insert(
            "wpgen".into(),
            component_stat_value(comps.wpgen, &stats.wpgen),
        );

        let output = json!({
            "stat": Value::Object(stat),
            "detail": rows
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output)
                .expect("JSON serialize should not fail for Row/stat types")
        );
    } else if opts.console {
        println!();
        let table = build_detail_table(rows, comps);
        println!("{}", table);
    } else {
        print_text_summary(rows.len(), stats, comps);
        println!("\n{}", build_detail_table(rows, comps));
        output_failure_details(rows, comps);
    }
}

fn print_text_summary(total: usize, stats: &SummaryCounts, comps: &CheckComponents) {
    println!(
        "Project check completed ({} project{})",
        total,
        if total == 1 { "" } else { "s" }
    );
    if comps.engine {
        println!("Config: {}/{} passed", stats.conf.ok, stats.conf.total);
    } else {
        println!("Config: skipped");
    }
    if comps.connectors {
        println!(
            "Connectors: {}/{} passed",
            stats.connectors.ok, stats.connectors.total
        );
    } else {
        println!("Connectors: skipped");
    }
    if comps.sources {
        println!(
            "Sources: {}/{} passed",
            stats.sources.ok, stats.sources.total
        );
    } else {
        println!("Sources: skipped");
    }
    if comps.sinks {
        println!("Sinks: {}/{} passed", stats.sinks.ok, stats.sinks.total);
    } else {
        println!("Sinks: skipped");
    }
    if comps.wpl {
        println!("WPL models: {}/{} passed", stats.wpl.ok, stats.wpl.total);
    } else {
        println!("WPL models: skipped");
    }
    if comps.oml {
        println!("OML models: {}/{} passed", stats.oml.ok, stats.oml.total);
    } else {
        println!("OML models: skipped");
    }
    if comps.semantic_dict {
        println!(
            "Semantic dict: {}/{} passed",
            stats.semantic_dict.ok, stats.semantic_dict.total
        );
    } else {
        println!("Semantic dict: skipped");
    }
    if comps.wpgen {
        println!(
            "Wpgen config: {}/{} passed",
            stats.wpgen.ok, stats.wpgen.total
        );
    } else {
        println!("Wpgen config: skipped");
    }
}

fn output_failure_details(rows: &[Row], comps: &CheckComponents) {
    let failed_rows: Vec<_> = rows
        .iter()
        .filter(|r| {
            (comps.engine && !r.conf.ok)
                || (comps.connectors && !r.connectors.ok)
                || (comps.sources && !r.sources.ok)
                || (comps.sinks && !r.sinks.ok)
                || (comps.wpl && !r.wpl.ok)
                || (comps.oml && !r.oml.ok)
                || (comps.semantic_dict && !r.semantic_dict.ok)
                || (comps.wpgen && !r.wpgen.ok)
        })
        .collect();

    if failed_rows.is_empty() {
        return;
    }

    println!("Failure details:");
    for r in failed_rows {
        for (label, cell) in component_cells(r, comps) {
            if !cell.ok {
                let detail = cell.msg.as_deref().unwrap_or("no error message");
                println!("  - {} -> {}: {}", r.path, label, detail);
            }
        }
    }
}

fn has_failures(rows: &[Row], comps: &CheckComponents) -> bool {
    rows.iter().any(|r| {
        (comps.engine && !r.conf.ok)
            || (comps.connectors && !r.connectors.ok)
            || (comps.sources && !r.sources.ok)
            || (comps.sinks && !r.sinks.ok)
            || (comps.wpl && !r.wpl.ok)
            || (comps.oml && !r.oml.ok)
            || (comps.semantic_dict && !r.semantic_dict.ok)
            || (comps.wpgen && !r.wpgen.ok)
    })
}

/// 默认检查配置的便捷函数
#[allow(dead_code)]
pub fn check_with_default(
    project: &WarpProject,
    opts: &CheckOptions,
    dict: &EnvDict,
) -> RunResult<()> {
    check_with(project, opts, &CheckComponents::default(), dict)
}

fn collect_connector_counts(work_root: &str, dict: &EnvDict) -> Result<ConnectorCounts, String> {
    let (_cm, main) =
        cfg_face::load_warp_engine_confs(work_root, dict).map_err(|e| describe_run_error(&e))?;
    let src_rows = source_connectors::list_connectors(work_root, &main, dict)
        .map_err(|e| describe_struct_error(&e))?;
    let src_defs = src_rows.len();
    let src_refs: usize = src_rows.iter().map(|row| row.refs).sum();

    // Detect dead source connectors (defined but never referenced)
    let dead_src: Vec<&str> = src_rows
        .iter()
        .filter(|row| row.refs == 0)
        .map(|row| row.id.as_str())
        .collect();
    if !dead_src.is_empty() {
        eprintln!(
            "  ⚠ Dead source connectors (defined but never referenced): {}",
            dead_src.join(", ")
        );
    }

    let (sink_map, sink_usage) = sink_connectors::list_connectors_usage(work_root, dict)
        .map_err(|e| describe_struct_error(&e))?;
    let sink_defs = sink_map.len();
    let sink_routes = sink_usage.len();

    // Detect dead sink connectors (defined but never used in any route)
    let used_sink_ids: std::collections::BTreeSet<&str> =
        sink_usage.iter().map(|(cid, _, _)| cid.as_str()).collect();
    let dead_sink: Vec<&str> = sink_map
        .keys()
        .filter(|id| !used_sink_ids.contains(id.as_str()))
        .map(|id| id.as_str())
        .collect();
    if !dead_sink.is_empty() {
        eprintln!(
            "  ⚠ Dead sink connectors (defined but never used): {}",
            dead_sink.join(", ")
        );
    }

    Ok(ConnectorCounts {
        source_defs: src_defs,
        source_refs: src_refs,
        sink_defs,
        sink_routes,
    })
}

#[cfg(test)]
mod tests {
    use super::{describe_run_error, shorten_semantic_dict_message};
    use crate::compat::UvsFrom;
    use orion_error::conversion::ToStructError;
    use std::path::Path;
    use wp_error::run_error::RunReason;

    #[test]
    fn semantic_dict_message_is_shortened_for_table_output() {
        let work_root = Path::new("/tmp/demo");
        let config_path = work_root.join("models/knowledge/semantic_dict.toml");
        let raw = format!(
            "语义词典配置有效: {} | 模式: ADD（扩展内置词典） | 词汇数: 0",
            config_path.display()
        );

        let short = shorten_semantic_dict_message(&raw, work_root, &config_path);
        assert_eq!(
            short,
            "./models/knowledge/semantic_dict.toml | 模式: ADD（扩展内置词典） | 词汇数: 0"
        );
    }

    #[test]
    fn describe_run_error_prefers_detail_over_generic_reason() {
        let err = RunReason::from_conf()
            .to_err()
            .with_detail("override 'endpoint' not allowed");

        assert_eq!(describe_run_error(&err), "override 'endpoint' not allowed");
    }

    #[test]
    fn describe_run_error_uses_target_path_when_detail_missing() {
        let err = RunReason::from_conf()
            .to_err()
            .with_source(std::io::Error::other("/tmp/demo/wparse.toml"));

        let msg = describe_run_error(&err);
        assert!(msg.contains("configuration error << core config"));
        assert!(msg.contains("/tmp/demo/wparse.toml"));
    }
}
