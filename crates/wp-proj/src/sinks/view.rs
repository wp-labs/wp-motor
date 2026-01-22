use comfy_table::{Cell as TCell, Table};
use oml::core::ConfADMExt;
use oml::language::ObjModel;
use orion_error::{ToStructError, UvsConfFrom};
use orion_variate::EnvDict;
use serde_json::json;
use wildmatch::WildMatch;
use wp_conf::utils::find_conf_files;
use wp_engine::facade::config::WPARSE_OML_FILE;
use wp_error::run_error::{RunReason, RunResult};

use crate::utils::config_path::ConfigPathResolver;

#[derive(Clone, Copy)]
pub enum DisplayFormat {
    Json,
    Table,
}

impl DisplayFormat {
    pub fn from_bool(json: bool) -> Self {
        if json {
            DisplayFormat::Json
        } else {
            DisplayFormat::Table
        }
    }
}

pub fn render_sink_list(
    rows: &[wp_cli_core::business::connectors::sinks::RouteRow],
    fmt: DisplayFormat,
) {
    match fmt {
        DisplayFormat::Json => {
            let items: Vec<_> = rows
                .iter()
                .map(|r| {
                    json!({
                        "scope": r.scope,
                        "full_name": r.full_name,
                        "connector": r.connector,
                        "target": r.target,
                        "fmt": r.fmt,
                        "detail": r.detail,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&items).unwrap());
        }
        DisplayFormat::Table => {
            let mut table = Table::new();
            table.load_preset(comfy_table::presets::UTF8_FULL);
            table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
            table.set_width(140);
            table.set_header(vec![
                TCell::new("scope"),
                TCell::new("full_name"),
                TCell::new("connector"),
                TCell::new("target"),
                TCell::new("fmt"),
                TCell::new("detail"),
            ]);
            for r in rows {
                table.add_row(vec![
                    TCell::new(&r.scope),
                    TCell::new(&r.full_name),
                    TCell::new(&r.connector),
                    TCell::new(&r.target),
                    TCell::new(&r.fmt),
                    TCell::new(&r.detail),
                ]);
            }
            println!("{}", table);
        }
    }
}

pub fn render_route_rows(items: &[PipelineRow], fmt: DisplayFormat) {
    match fmt {
        DisplayFormat::Json => {
            let data: Vec<_> = items
                .iter()
                .map(|r| {
                    json!({
                        "sink": r.sink,
                        "group": r.group,
                        "oml": r.oml,
                        "rule": r.rule,
                        "connector": r.connector,
                        "target": r.target,
                        "scope": r.scope,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&data).unwrap());
        }
        DisplayFormat::Table => {
            let mut table = Table::new();
            table.load_preset(comfy_table::presets::UTF8_FULL);
            table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
            table.set_width(160);
            table.set_header(vec![
                TCell::new("sink"),
                TCell::new("oml"),
                TCell::new("rule"),
                TCell::new("connector"),
                TCell::new("target"),
            ]);
            for item in items {
                table.add_row(vec![
                    TCell::new(&item.sink),
                    TCell::new(&item.oml),
                    TCell::new(&item.rule),
                    TCell::new(&item.connector),
                    TCell::new(&item.target),
                ]);
            }
            println!("{}", table);
        }
    }
}

pub fn expand_route_rows(
    rows: &[wp_cli_core::business::connectors::sinks::RouteRow],
    oml_map: &[OmlRule],
) -> Vec<PipelineRow> {
    let mut out = Vec::new();
    for r in rows {
        let sink_label = r.full_name.clone();
        if !r.rules.is_empty() {
            for rule in &r.rules {
                out.push(PipelineRow {
                    sink: sink_label.clone(),
                    group: r.group.clone(),
                    oml: "-".into(),
                    rule: rule.clone(),
                    connector: r.connector.clone(),
                    target: r.target.clone(),
                    scope: r.scope.clone(),
                });
            }
        }
        if !r.oml.is_empty() {
            for pat in &r.oml {
                let matcher = WildMatch::new(pat);
                let mut matched = false;
                for mdl in oml_map.iter() {
                    if matcher.matches(mdl.name.as_str()) {
                        matched = true;
                        out.push(PipelineRow {
                            sink: sink_label.clone(),
                            group: r.group.clone(),
                            oml: mdl.name.clone(),
                            rule: mdl.rule_text(),
                            connector: r.connector.clone(),
                            target: r.target.clone(),
                            scope: r.scope.clone(),
                        });
                    }
                }
                if !matched {
                    out.push(PipelineRow {
                        sink: sink_label.clone(),
                        group: r.group.clone(),
                        oml: pat.clone(),
                        rule: "-".into(),
                        connector: r.connector.clone(),
                        target: r.target.clone(),
                        scope: r.scope.clone(),
                    });
                }
            }
        }
        if r.rules.is_empty() && r.oml.is_empty() {
            out.push(PipelineRow {
                sink: sink_label,
                group: r.group.clone(),
                oml: "-".into(),
                rule: "-".into(),
                connector: r.connector.clone(),
                target: r.target.clone(),
                scope: r.scope.clone(),
            });
        }
    }
    out
}

pub fn collect_oml_models(work_root: &str, dict: &EnvDict) -> RunResult<Vec<OmlRule>> {
    let oml_root = ConfigPathResolver::resolve_model_path(work_root, "oml", dict)?;
    let root_str = oml_root
        .to_str()
        .ok_or_else(|| RunReason::from_conf("invalid oml root path").to_err())?;
    let files = find_conf_files(root_str, WPARSE_OML_FILE)
        .map_err(|e| RunReason::from_conf(format!("list OML failed: {}", e)).to_err())?;
    let mut items = Vec::new();
    for path in files {
        let path_str = path.to_string_lossy().to_string();
        let model = ObjModel::load(path_str.as_str()).map_err(|e| {
            RunReason::from_conf(format!("parse OML {} failed: {}", path_str, e)).to_err()
        })?;
        let rules: Vec<String> = model
            .rules()
            .as_ref()
            .iter()
            .map(|r| r.to_string())
            .collect();
        let name_in_model = model.name().trim();
        let name = if name_in_model.is_empty() {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string()
        } else {
            name_in_model.to_string()
        };
        items.push(OmlRule { name, rules });
    }
    Ok(items)
}

#[derive(Clone)]
pub struct PipelineRow {
    pub sink: String,
    pub group: String,
    pub oml: String,
    pub rule: String,
    pub connector: String,
    pub target: String,
    pub scope: String,
}

#[derive(Clone)]
pub struct OmlRule {
    pub name: String,
    pub rules: Vec<String>,
}

impl OmlRule {
    pub fn rule_text(&self) -> String {
        format_rules(&self.rules)
    }
}

fn format_rules(rules: &[String]) -> String {
    if rules.is_empty() {
        "-".into()
    } else {
        rules.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_route_row() -> wp_cli_core::business::connectors::sinks::RouteRow {
        wp_cli_core::business::connectors::sinks::RouteRow {
            scope: "biz".into(),
            group: "default".into(),
            full_name: "biz.demo".into(),
            name: "demo".into(),
            connector: "file".into(),
            target: "file".into(),
            fmt: "text".into(),
            detail: "-".into(),
            rules: vec!["rule_a".into(), "rule_b".into()],
            oml: vec!["nginx_*".into()],
        }
    }

    #[test]
    fn expand_route_rows_emits_entries_for_rules_and_oml_matches() {
        let row = sample_route_row();
        let oml = vec![OmlRule {
            name: "nginx_access".into(),
            rules: vec!["r1".into()],
        }];
        let rows = expand_route_rows(&[row], &oml);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().any(|r| r.rule == "rule_a"));
        assert!(rows.iter().any(|r| r.oml == "nginx_access"));
    }

    #[test]
    fn format_rules_handles_empty_list() {
        assert_eq!(format_rules(&[]), "-");
        assert_eq!(format_rules(&["a".into(), "b".into()]), "a,b");
    }

    #[test]
    fn display_format_switches_on_boolean() {
        assert!(matches!(
            DisplayFormat::from_bool(true),
            DisplayFormat::Json
        ));
        assert!(matches!(
            DisplayFormat::from_bool(false),
            DisplayFormat::Table
        ));
    }
}
