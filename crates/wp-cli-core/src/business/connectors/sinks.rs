use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use orion_conf::ToStructError;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ErrorOwe, ErrorWith, UvsValidationFrom};
use orion_variate::EnvDict;

use wp_conf::connectors::{
    ConnectorScope, ParamMap, load_connector_defs_from_dir, param_map_to_table,
};
use wp_conf::sinks::build_route_conf_from;
use wp_conf::sinks::io::find_connectors_base_dir;
use wp_conf::sinks::{ConnectorRec, RouteFile};
use wp_conf::sinks::{load_route_files_from, load_sink_defaults};
use wp_conf::structure::SinkInstanceConf;

/// List immediate child directories of `p`, sorted.
fn read_dirs_sorted(p: &Path) -> OrionConfResult<Vec<PathBuf>> {
    let mut v: Vec<_> = std::fs::read_dir(p)
        .owe_conf()
        .with(p)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    Ok(v)
}

#[derive(Debug, Clone)]
/// Route file plus its origin path (used for grouping and diagnostics).
struct RouteFileWithPath {
    inner: RouteFile,
    path: PathBuf,
}
impl std::ops::Deref for RouteFileWithPath {
    type Target = RouteFile;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl RouteFileWithPath {
    fn path_str(&self) -> String {
        self.path.display().to_string()
    }
}

/// Recursively discover sink route files under `usecase/*/(sink/business.d|sink/infra.d)`.
fn load_routes(work_root: &Path, env_dict: &EnvDict) -> OrionConfResult<Vec<RouteFileWithPath>> {
    let mut out: Vec<RouteFileWithPath> = Vec::new();
    // 1) usecase/*/<case>/sink/{business.d,infra.d}
    let uc = work_root.join("usecase");
    if uc.exists() {
        for dom in read_dirs_sorted(&uc)? {
            for case in read_dirs_sorted(&dom)? {
                let r1 = case.join("sink/business.d");
                let r2 = case.join("sink/infra.d");
                for rd in [r1, r2] {
                    if rd.exists() {
                        let rfs = load_route_files_from(&rd, env_dict)?;
                        for rf in rfs.into_iter() {
                            let path = rf.origin.clone().unwrap_or_else(|| rd.clone());
                            out.push(RouteFileWithPath { inner: rf, path });
                        }
                    }
                }
            }
        }
    }

    // 2) models/sinks/{business.d,infra.d} (project-local)
    let models = work_root.join("models").join("sinks");
    for rd in [models.join("business.d"), models.join("infra.d")] {
        if rd.exists() {
            let rfs = load_route_files_from(&rd, env_dict)?;
            for rf in rfs.into_iter() {
                let path = rf.origin.clone().unwrap_or_else(|| rd.clone());
                out.push(RouteFileWithPath { inner: rf, path });
            }
        }
    }
    Ok(out)
}

/// Validate that each discovered route file can be built with local defaults and connectors.
pub fn validate_routes(work_root: &str, env_dict: &EnvDict) -> OrionConfResult<()> {
    let wr = PathBuf::from(work_root);
    let routes = load_routes(&wr, env_dict)?;
    for rf in &routes {
        let sink_root = rf
            .path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| rf.path.parent().unwrap_or(&wr))
            .to_path_buf();
        let conn_map = load_sink_connectors(&sink_root, env_dict)?;
        let defaults = load_sink_defaults(sink_root.to_string_lossy().as_ref(), env_dict)?;
        let conf = build_route_conf_from(&rf.inner, defaults.as_ref(), &conn_map)?;

        // 验证 FlexGroup 配置：OML 和 RULE 只能有一个
        let flex_group = &conf.sink_group;
        let has_oml = !flex_group.oml().as_ref().is_empty();
        let has_rule = !flex_group.rule().as_ref().is_empty();

        // 检查 OML 和 RULE 是否同时存在
        if has_oml && has_rule {
            let file_display = rf.path.to_string_lossy();

            return Err(
                ConfIOReason::from_validation(format!(
                    "FlexGroup configuration validation failed in file '{}': OML and RULE cannot be used together",
                    file_display
                ))
                .err_result()?
            );
        }

        // 验证规则格式：检查是否以 '/' 开头以及是否为空
        for rule_pattern in flex_group.rule().as_ref() {
            let rule_str = rule_pattern.to_string();

            if rule_str.is_empty() {
                let file_display = rf.path.to_string_lossy();
                return Err(
                    ConfIOReason::from_validation(format!(
                        "FlexGroup configuration validation failed in file '{}': Empty rule pattern found",
                        file_display
                    ))
                    .err_result()?
                );
            }

            if !rule_str.starts_with('/') {
                let file_display = rf.path.to_string_lossy();
                return Err(
                    ConfIOReason::from_validation(format!(
                        "FlexGroup configuration validation failed in file '{}': rule pattern '{}' should start with '/'",
                        file_display, rule_str
                    ))
                    .err_result()?
                );
            }
        }

        // 验证 OML 模式：检查是否为空
        for oml_pattern in flex_group.oml().as_ref() {
            let oml_str = oml_pattern.to_string();

            if oml_str.is_empty() {
                let file_display = rf.path.to_string_lossy();
                return Err(
                    ConfIOReason::from_validation(format!(
                        "FlexGroup configuration validation failed in file '{}': Empty OML pattern found",
                        file_display
                    ))
                    .err_result()?
                );
            }
        }
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
pub fn list_connectors_usage(
    work_root: &str,
    env_dict: &EnvDict,
) -> OrionConfResult<(
    BTreeMap<String, ConnectorRec>,
    Vec<(String, String, String)>,
)> {
    let wr = PathBuf::from(work_root);
    let conn_map = load_sink_connectors(Path::new(work_root), env_dict)?;
    let routes = load_routes(&wr, env_dict)?;
    let mut usage: Vec<(String, String, String)> = Vec::new();
    for rf in &routes {
        let sink_root = rf
            .path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| rf.path.parent().unwrap_or(&wr))
            .to_path_buf();
        let conn_map_local = load_sink_connectors(&sink_root, env_dict)?;
        let defaults = load_sink_defaults(sink_root.to_string_lossy().as_ref(), env_dict)?;
        let conf = build_route_conf_from(&rf.inner, defaults.as_ref(), &conn_map_local)?;
        let g = conf.sink_group;
        for s in g.sinks.iter() {
            let cid = s.connector_id.clone().unwrap_or_else(|| "-".to_string());
            usage.push((cid, rf.path_str(), g.name().to_string()));
        }
    }
    Ok((conn_map, usage))
}

#[derive(Debug, Clone)]
pub struct RouteRow {
    pub scope: String,
    pub group: String,
    pub full_name: String,
    pub name: String,
    pub connector: String,
    pub target: String,
    pub fmt: String,
    pub detail: String,
    pub rules: Vec<String>,
    pub oml: Vec<String>,
}

/// Generate a flattened route table for sinks with optional group/sink filters.
pub fn route_table(
    work_root: &str,
    group_filters: &[String],
    sink_filters: &[String],
    env_dict: &EnvDict,
) -> OrionConfResult<Vec<RouteRow>> {
    fn matched(name: &str, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        filters.iter().any(|f| name.contains(f))
    }
    let wr = PathBuf::from(work_root);
    let mut out = Vec::new();
    let rows = load_routes(&wr, env_dict)?;
    for rf in rows {
        let sink_root = rf
            .path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| rf.path.parent().unwrap_or(&wr))
            .to_path_buf();
        let conn_map_local = load_sink_connectors(&sink_root, env_dict)?;
        let defaults = load_sink_defaults(sink_root.to_string_lossy().as_ref(), env_dict)?;
        let conf = build_route_conf_from(&rf.inner, defaults.as_ref(), &conn_map_local)?;
        let g = conf.sink_group;
        let scope = if rf.path.to_string_lossy().contains("/infra.d/")
            || rf.path.to_string_lossy().ends_with("/infra.d")
        {
            "infra"
        } else {
            "biz"
        };
        let group_name = g.name();
        let rules: Vec<String> = g.rule.as_ref().iter().map(|m| m.to_string()).collect();
        let oml_patterns: Vec<String> = g.oml().as_ref().iter().map(|m| m.to_string()).collect();
        if !matched(group_name, group_filters) {
            continue;
        }
        for s in g.sinks.iter() {
            let name = s.name();
            if !matched(name, sink_filters) {
                continue;
            }
            let (target, detail) = target_detail_of(s)?;
            let row = RouteRow {
                scope: scope.to_string(),
                group: group_name.to_string(),
                full_name: s.full_name(),
                name: name.to_string(),
                connector: s.connector_id.clone().unwrap_or_else(|| "-".into()),
                target,
                fmt: s.fmt().to_string(),
                detail,
                rules: rules.clone(),
                oml: oml_patterns.clone(),
            };
            out.push(row);
        }
    }
    out.sort_by(|a, b| a.scope.cmp(&b.scope).then(a.full_name.cmp(&b.full_name)));
    Ok(out)
}

/// Render params as single-line TOML for display; avoid guessing semantics.
fn params_one_line(params: &ParamMap) -> String {
    let table = param_map_to_table(params);
    match toml::to_string(&table) {
        Ok(s) => s.replace(['\n', '\t'], " ").trim().to_string(),
        Err(_) => format!("{:?}", params),
    }
}

/// Best-effort target and detail visualization from a resolved sink instance.
/// - target: concise identifier (e.g., kind, or `syslog/<proto>`)
/// - detail: full params rendered as single-line TOML
fn target_detail_of(s: &SinkInstanceConf) -> OrionConfResult<(String, String)> {
    let kind = s.resolved_kind_str();
    let p = s.resolved_params_table();
    let target = if kind == "syslog" {
        let proto = p.get("protocol").and_then(|v| v.as_str()).unwrap_or("udp");
        format!("syslog/{}", proto)
    } else {
        kind.clone()
    };
    let detail = params_one_line(&p);
    Ok((target, detail))
}

/// Load sink connectors (id -> connector) beneath the given work root.
pub fn load_connectors_map(
    work_root: &str,
    dict: &orion_variate::EnvDict,
) -> OrionConfResult<BTreeMap<String, ConnectorRec>> {
    load_sink_connectors(Path::new(work_root), dict)
}

fn load_sink_connectors(
    start: &Path,
    dict: &orion_variate::EnvDict,
) -> OrionConfResult<BTreeMap<String, ConnectorRec>> {
    if let Some(dir) = find_connectors_base_dir(start) {
        let defs = load_connector_defs_from_dir(&dir, ConnectorScope::Sink, dict)?;
        Ok(defs.into_iter().map(|def| (def.id.clone(), def)).collect())
    } else {
        Ok(BTreeMap::new())
    }
}

#[cfg(test)]
mod tests {
    use wp_conf::test_support::ForTest;

    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}", prefix, nanos));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_demo_connectors(base: &std::path::Path) {
        let cdir = base.join("connectors").join("sink.d");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(
            cdir.join("file.toml"),
            r#"[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["file","path","fmt"]
"#,
        )
        .unwrap();
    }

    fn write_demo_route_business(sink_root: &std::path::Path) -> std::path::PathBuf {
        let biz = sink_root.join("business.d");
        fs::create_dir_all(&biz).unwrap();
        let fp = biz.join("demo.toml");
        fs::write(
            &fp,
            r#"version = "2.0"

[sink_group]
name = "demo"
oml  = []
tags = ["biz:demo"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "demo.json" }
tags = ["sink:json"]
"#,
        )
        .unwrap();
        fp
    }

    #[test]
    fn route_table_basic() {
        let root = tmp_dir("wpcore_sink_rt");
        write_demo_connectors(&root);
        let case_sink_root = root.join("usecase").join("d").join("c").join("sink");
        let rf = write_demo_route_business(&case_sink_root);

        // validate
        validate_routes(root.to_string_lossy().as_ref(), &EnvDict::test_default())
            .expect("validate");

        // route table
        let rows = route_table(
            root.to_string_lossy().as_ref(),
            &[],
            &[],
            &EnvDict::test_default(),
        )
        .expect("rt");
        assert!(!rows.is_empty());
        let r = rows.iter().find(|r| r.full_name.ends_with("json")).unwrap();
        assert_eq!(r.scope, "biz");
        assert_eq!(r.connector, "file_json_sink");
        assert_eq!(r.target, "file");
        assert!(r.detail.contains("demo.json"));

        // usage includes our group
        let (_map, usage) =
            list_connectors_usage(root.to_string_lossy().as_ref(), &EnvDict::test_default())
                .expect("usage");
        let rf_can = std::fs::canonicalize(&rf)
            .unwrap_or(rf.clone())
            .display()
            .to_string();
        assert!(
            usage
                .iter()
                .any(|(cid, path, g)| cid == "file_json_sink" && path == &rf_can && g == "demo")
        );
    }

    #[test]
    fn test_validate_flexgroup_oml_rule_mutually_exclusive() {
        // 测试 FlexGroup 中 OML 和 RULE 的互斥验证
        let temp = tmp_dir("flexgroup_mutual_test");
        write_demo_connectors(&temp);

        // 创建同时有 OML 和 RULE 的 FlexGroup 配置
        let sinks_dir = temp.join("models/sinks/business.d");
        fs::create_dir_all(&sinks_dir).unwrap();

        let invalid_config = r#"
version = "2.0"

[sink_group]
name = "invalid_group"
oml = ["model1", "model2"]
rule = ["/api/*", "/test/*"]

[[sink_group.sinks]]
name = "sink1"
connect = "file_json_sink"
params = { file = "output.txt" }
"#;

        let config_file = sinks_dir.join("invalid.toml");
        fs::write(&config_file, invalid_config).unwrap();

        // 验证应该失败
        let result = validate_routes(temp.to_string_lossy().as_ref(), &EnvDict::test_default());
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("OML and RULE cannot be used together"));
        assert!(error_msg.contains("FlexGroup configuration validation failed"));
    }

    #[test]
    fn test_validate_flexgroup_rule_only_valid() {
        // 测试只有 RULE 的 FlexGroup 配置（应该有效）
        let temp = tmp_dir("flexgroup_rule_only_test");
        write_demo_connectors(&temp);

        let sinks_dir = temp.join("models/sinks/business.d");
        fs::create_dir_all(&sinks_dir).unwrap();

        let valid_config = r#"
version = "2.0"

[sink_group]
name = "rule_only_group"
rule = ["/api/*", "/test/*"]

[[sink_group.sinks]]
name = "sink1"
connect = "file_json_sink"
params = { file = "output.txt" }
"#;

        let config_file = sinks_dir.join("valid.toml");
        fs::write(&config_file, valid_config).unwrap();

        // 验证应该成功
        let result = validate_routes(temp.to_string_lossy().as_ref(), &EnvDict::test_default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_flexgroup_oml_only_valid() {
        // 测试只有 OML 的 FlexGroup 配置（应该有效）
        let temp = tmp_dir("flexgroup_oml_only_test");
        write_demo_connectors(&temp);

        let sinks_dir = temp.join("models/sinks/business.d");
        fs::create_dir_all(&sinks_dir).unwrap();

        let valid_config = r#"
version = "2.0"

[sink_group]
name = "oml_only_group"
oml = ["model1", "model2"]

[[sink_group.sinks]]
name = "sink1"
connect = "file_json_sink"
params = { file = "output.txt" }
"#;

        let config_file = sinks_dir.join("valid.toml");
        fs::write(&config_file, valid_config).unwrap();

        // 验证应该成功
        let result = validate_routes(temp.to_string_lossy().as_ref(), &EnvDict::test_default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_flexgroup_rule_pattern_validation() {
        // 测试 RULE 模式格式验证
        let temp = tmp_dir("flexgroup_rule_pattern_test");
        write_demo_connectors(&temp);

        let sinks_dir = temp.join("models/sinks/business.d");
        fs::create_dir_all(&sinks_dir).unwrap();

        // 创建无效的规则模式（不以 / 开头）
        let invalid_config = r#"
version = "2.0"

[sink_group]
name = "invalid_rule_group"
# 缺少 / 前缀的规则模式
rule = ["api/*", "test/*"]

[[sink_group.sinks]]
name = "sink1"
connect = "file_json_sink"
params = { file = "output.txt" }
"#;

        let config_file = sinks_dir.join("invalid_rule.toml");
        fs::write(&config_file, invalid_config).unwrap();

        // 验证应该失败
        let result = validate_routes(temp.to_string_lossy().as_ref(), &EnvDict::test_default());
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("should start with '/'"));
    }

    #[test]
    fn test_validate_flexgroup_empty_patterns() {
        // 测试空的 OML 或 RULE 模式
        let temp = tmp_dir("flexgroup_empty_patterns_test");
        write_demo_connectors(&temp);

        let sinks_dir = temp.join("models/sinks/business.d");
        fs::create_dir_all(&sinks_dir).unwrap();

        // 创建包含空规则的配置
        let invalid_config = r#"
version = "2.0"

[sink_group]
name = "empty_patterns_group"
rule = ["", "/valid/*"]  # 包含空字符串

[[sink_group.sinks]]
name = "sink1"
connect = "file_json_sink"
params = { file = "output.txt" }
"#;

        let config_file = sinks_dir.join("empty.toml");
        fs::write(&config_file, invalid_config).unwrap();

        // 验证应该失败
        let result = validate_routes(temp.to_string_lossy().as_ref(), &EnvDict::test_default());
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Empty rule pattern found"));
    }
}
