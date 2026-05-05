//! Source file observability functions
//!
//! This module provides business logic for analyzing source configurations
//! and counting lines in source files.

use crate::utils::fs::{count_lines_file, resolve_path};
use glob::glob;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{EnvTomlLoad, ErrorWith};
use orion_error::conversion::{SourceErr, ToStructError};
use orion_variate::{EnvDict, EnvEvaluable};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wp_conf::connectors::ParamMap;
use wp_conf::constants::WPSRC_TOML;
use wp_conf::engine::EngineConfig;
use wp_conf::sources::{WpSource, WpSourcesConfig};
use wp_conf::structure::SourceInstanceConf;

// Re-export types from wpcnt_lib for convenience
pub use crate::utils::types::{Ctx, SrcLineItem, SrcLineReport};

#[derive(Debug)]
struct WpsrcSource {
    source: WpSource,
    instance: SourceInstanceConf,
}

// 私有辅助函数
fn wpsrc_path(work_root: &Path, engine_conf: &EngineConfig) -> PathBuf {
    work_root.join(engine_conf.src_root()).join(WPSRC_TOML)
}

fn read_wpsrc_toml(path: &Path) -> OrionConfResult<Option<String>> {
    if path.exists() {
        return std::fs::read_to_string(path)
            .source_err(ConfIOReason::system_error(), "read wpsrc config")
            .with_context(path)
            .doing("read wpsrc config")
            .map(Some);
    }
    Ok(None)
}

fn load_wpsrc_sources(
    work_root: &Path,
    engine_conf: &EngineConfig,
    dict: &EnvDict,
) -> OrionConfResult<Option<Vec<WpsrcSource>>> {
    let path = wpsrc_path(work_root, engine_conf);
    let Some(content) = read_wpsrc_toml(&path)? else {
        return Ok(None);
    };

    let parsed: WpSourcesConfig = WpSourcesConfig::env_parse_toml(&content, dict)
        .with_context(&path)
        .doing("parse wpsrc config")?
        .env_eval(dict);
    let instances = wp_conf::sources::load_source_instances_from_file(&path, dict)
        .with_context(&path)
        .doing("load wpsrc source instances")?;

    let mut instances_by_name: BTreeMap<String, SourceInstanceConf> = instances
        .into_iter()
        .map(|instance| (instance.core.name.clone(), instance))
        .collect();
    let mut sources = Vec::new();
    for source in parsed.sources {
        if let Some(instance) = instances_by_name.remove(&source.key) {
            sources.push(WpsrcSource { source, instance });
        }
    }
    Ok(Some(sources))
}

fn load_wpsrc_config(
    work_root: &Path,
    engine_conf: &EngineConfig,
    dict: &EnvDict,
) -> OrionConfResult<Option<(PathBuf, WpSourcesConfig)>> {
    let path = wpsrc_path(work_root, engine_conf);
    let Some(content) = read_wpsrc_toml(&path)? else {
        return Ok(None);
    };
    let parsed: WpSourcesConfig = WpSourcesConfig::env_parse_toml(&content, dict)
        .with_context(&path)
        .doing("parse wpsrc config")?
        .env_eval(dict);
    Ok(Some((path, parsed)))
}

fn load_wpsrc_connectors_map(
    work_root: &Path,
    engine_conf: &EngineConfig,
    ctx: &Ctx,
    dict: &EnvDict,
) -> OrionConfResult<BTreeMap<String, wp_conf::sources::SourceConnector>> {
    let source_root = work_root.join(engine_conf.src_root());
    let conn_dir = wp_conf::find_connectors_base_dir(&source_root, "source.d")
        .or_else(|| wp_conf::find_connectors_base_dir(&ctx.work_root.join("sources"), "source.d"));
    match conn_dir.as_ref() {
        Some(path) => wp_conf::sources::load_connectors_for(path.as_path(), dict)
            .with_context(path)
            .doing("load source connectors"),
        None => Ok(BTreeMap::new()),
    }
}

fn has_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn configured_file_source_path(merged: &ParamMap) -> Option<String> {
    if let Some(path) = merged.get("path").and_then(|v| v.as_str()) {
        return Some(path.to_string());
    }

    merged.get("file").and_then(|v| v.as_str()).map(|file| {
        let base = merged
            .get("base")
            .and_then(|v| v.as_str())
            .unwrap_or("./data/in_dat");
        Path::new(base).join(file).display().to_string()
    })
}

fn validated_file_source_path(merged: &ParamMap) -> OrionConfResult<String> {
    if merged.contains_key("path") {
        return Err(ConfIOReason::validation_error().to_err().with_detail(
            "'path' is not supported for file source; use 'file' (with optional wildcard) and optional 'base'",
        ));
    }

    let base = merged
        .get("base")
        .and_then(|v| v.as_str())
        .unwrap_or("./data/in_dat");
    if has_glob_pattern(base) {
        return Err(ConfIOReason::validation_error()
            .to_err()
            .with_detail("'base' does not support wildcard patterns for file source"));
    }

    let file = merged.get("file").and_then(|v| v.as_str()).ok_or_else(|| {
        ConfIOReason::validation_error()
            .to_err()
            .with_detail("missing required 'file' for file source")
    })?;

    Ok(Path::new(base).join(file).display().to_string())
}

fn expand_source_paths(raw: &str, work_root: &Path) -> OrionConfResult<Vec<PathBuf>> {
    if !has_glob_pattern(raw) {
        return Ok(vec![resolve_path(raw, work_root)]);
    }

    let pattern = if Path::new(raw).is_absolute() {
        raw.to_string()
    } else {
        work_root.join(raw).display().to_string()
    };

    let mut matches = Vec::new();
    for entry in glob(&pattern).map_err(|err| {
        ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!("invalid glob pattern: {}", pattern))
            .with_source(err)
    })? {
        let path = entry.map_err(|err| {
            ConfIOReason::validation_error()
                .to_err()
                .with_detail(format!("iterate glob match: {}", pattern))
                .with_source(err)
        })?;
        if path.is_file() {
            matches.push(path);
        }
    }
    matches.sort();
    matches.dedup();
    if matches.is_empty() {
        return Err(ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!("glob matched no files: {}", pattern)));
    }
    Ok(matches)
}

/// 从 wpsrc 配置推导总输入条数（仅统计启用的文件源）
pub fn total_input_from_wpsrc(
    work_root: &Path,
    engine_conf: &EngineConfig,
    ctx: &Ctx,
    dict: &EnvDict,
) -> OrionConfResult<Option<u64>> {
    let Some(sources) = load_wpsrc_sources(work_root, engine_conf, dict)? else {
        return Ok(None);
    };
    let wpsrc_path = wpsrc_path(work_root, engine_conf);
    let mut sum = 0u64;
    let mut saw_enabled_file_source = false;

    for source in sources {
        let enabled = source.source.enable.unwrap_or(true);
        if !enabled || !source.instance.core.kind.eq_ignore_ascii_case("file") {
            continue;
        }
        saw_enabled_file_source = true;
        let key = source.source.key;
        let path = validated_file_source_path(&source.instance.core.params).map_err(|e| {
            e.with_context(&wpsrc_path)
                .doing(format!("validate source '{}' path spec", key))
        })?;
        let paths = expand_source_paths(&path, &ctx.work_root).map_err(|e| {
            e.with_context(&wpsrc_path)
                .doing(format!("expand source '{}' files", key))
        })?;
        for pathbuf in paths {
            let n = count_lines_file(&pathbuf).map_err(|e| {
                ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "count lines for source '{}' at {}: {}",
                        key,
                        pathbuf.display(),
                        e
                    ))
                    .with_context(&pathbuf)
            })?;
            sum += n;
        }
    }

    Ok(saw_enabled_file_source.then_some(sum))
}

/// 返回所有文件源（包含未启用）的行数信息；total 仅统计启用项
pub fn list_file_sources_with_lines(
    work_root: &Path,
    eng_conf: &EngineConfig,
    ctx: &Ctx,
    dict: &EnvDict,
) -> OrionConfResult<Option<SrcLineReport>> {
    let Some((wpsrc_path, source_conf)) = load_wpsrc_config(work_root, eng_conf, dict)? else {
        return Ok(None);
    };
    let conn_map = load_wpsrc_connectors_map(work_root, eng_conf, ctx, dict)?;
    let mut items = Vec::new();
    let mut total = 0u64;

    for source in source_conf.sources {
        let key = source.key.clone();
        let enabled = source.enable.unwrap_or(true);
        let instance = match wp_conf::sources::resolve_source_instance(&source, &conn_map) {
            Ok(instance) => instance,
            Err(err) => {
                let path_str = configured_file_source_path(&source.params).unwrap_or_default();
                items.push(SrcLineItem {
                    key,
                    path: path_str,
                    enabled,
                    lines: None,
                    error: Some(err.display_chain()),
                });
                continue;
            }
        };
        if !instance.core.kind.eq_ignore_ascii_case("file") {
            continue;
        }
        let path_str = configured_file_source_path(&instance.core.params).unwrap_or_default();
        if enabled {
            match validated_file_source_path(&instance.core.params) {
                Ok(path_str) => match expand_source_paths(&path_str, &ctx.work_root) {
                    Ok(paths) => {
                        let mut source_total = 0u64;
                        let mut first_err: Option<String> = None;
                        for path in &paths {
                            match count_lines_file(path) {
                                Ok(n) => source_total += n,
                                Err(e) if first_err.is_none() => {
                                    first_err = Some(e.to_string());
                                }
                                Err(_) => {}
                            }
                        }
                        if first_err.is_none() {
                            total += source_total;
                            items.push(SrcLineItem {
                                key,
                                path: path_str,
                                enabled,
                                lines: Some(source_total),
                                error: None,
                            });
                        } else {
                            items.push(SrcLineItem {
                                key,
                                path: path_str,
                                enabled,
                                lines: None,
                                error: first_err,
                            });
                        }
                    }
                    Err(err_msg) => {
                        items.push(SrcLineItem {
                            key,
                            path: path_str,
                            enabled,
                            lines: None,
                            error: Some(err_msg.to_string()),
                        });
                    }
                },
                Err(err) => {
                    items.push(SrcLineItem {
                        key,
                        path: path_str,
                        enabled,
                        lines: None,
                        error: Some(format!("{} ({})", err, wpsrc_path.display())),
                    });
                }
            }
        } else {
            items.push(SrcLineItem {
                key,
                path: path_str,
                enabled,
                lines: None,
                error: None,
            });
        }
    }
    Ok(Some(SrcLineReport {
        total_enabled_lines: total,
        items,
    }))
}

#[cfg(test)]
mod tests {
    use super::expand_source_paths;
    use std::path::Path;

    #[test]
    fn expand_source_paths_reports_invalid_glob_with_context() {
        let err = expand_source_paths("[", Path::new("/tmp"))
            .expect_err("invalid glob pattern should fail");
        let msg = format!("{:#}", err);

        assert!(msg.contains("invalid glob pattern"));
        assert!(msg.contains("["));
    }
}
