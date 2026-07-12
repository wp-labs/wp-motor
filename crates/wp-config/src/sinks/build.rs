use super::types::RouteSink;
use super::types::{ConnectorRec, DefaultsBody, RouteFile, StringOrArray};
use crate::connectors::json_type_label;
use crate::sinks::io::business_dir;
use crate::sinks::{load_connectors_for, load_route_files_from, load_sink_defaults};
use crate::structure::{SinkInstanceConf, SinkRouteConf, Validate as ConfValidate};
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::conversion::ToStructError;
use orion_variate::EnvDict;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use wp_connector_api::ParamMap;
use wp_model_core::model::fmt_def::TextFmt;

use crate::structure::{FlexGroup, extend_matches};

const CONNECTOR_TYPE_FILE: &str = "file";
const CONNECTOR_TYPE_TEST_RESCUE: &str = "test_rescue";
const FIELD_FMT: &str = "fmt";
const DEFAULT_OUTPUT_FORMAT: &str = "json";
const FIELD_WP_META_DISABLE: &str = "wp_meta_disable";
const FIELD_STREAM_TAG_FIELD: &str = "stream_tag_field";
const SUPPORTED_WP_META_DISABLE_FIELDS: &[&str] = &["wp_oml_name"];

fn build_sink_instance(
    group_name: &str,
    index: usize,
    origin: Option<&Path>,
    conn: &ConnectorRec,
    r: &RouteSink,
) -> OrionConfResult<SinkInstanceConf> {
    let sink_name = r
        .inner_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("[{}]", index));
    let merged_params = merge_sink_params(group_name, index, origin, conn, r)?;
    let fmt = decide_fmt(conn, &merged_params);
    let mut sink = crate::structure::SinkInstanceConf::new_type(
        sink_name.clone(),
        fmt,
        conn.kind.clone(),
        merged_params,
        r.filter_path().map(|s| s.to_string()),
    );
    // filter_expect: 默认为 true；用于决定 cond 匹配的期望值
    sink.set_filter_expect(r.filter_expect());
    sink.connector_id = Some(conn.id.clone());
    sink.group_name = Some(group_name.to_string());
    sink.expect = r.expect().cloned();
    sink.set_tags(r.tags().cloned().unwrap_or_default());
    Ok(sink)
}

// Registry view for plugin validation (injected by engine or tests); returns None when
// factory not available to keep config tools free of runtime dependencies.
pub trait SinkFactoryLookup {
    fn get(&self, kind: &str) -> Option<Arc<dyn wp_connector_api::SinkFactory + 'static>>;
}

#[derive(Copy, Clone, Debug, Default)]
struct NullSinkFactoryLookup;
impl SinkFactoryLookup for NullSinkFactoryLookup {
    fn get(&self, _kind: &str) -> Option<Arc<dyn wp_connector_api::SinkFactory + 'static>> {
        None
    }
}

static NULL_FACTORY_LOOKUP: NullSinkFactoryLookup = NullSinkFactoryLookup;

fn pick_string(m: &ParamMap, key: &str) -> Option<String> {
    m.get(key).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(i) => Some(i.to_string()),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .next(),
        _ => None,
    })
}

/// 决定输出文本格式
/// - 文件类（file/test_rescue）：从合并后的参数表读取 `fmt`（允许覆写），若缺省则默认 `json`
/// - 其它类型：固定为 `json`
fn decide_fmt(conn: &ConnectorRec, params: &ParamMap) -> TextFmt {
    if conn.kind == CONNECTOR_TYPE_FILE || conn.kind == CONNECTOR_TYPE_TEST_RESCUE {
        let s = pick_string(params, FIELD_FMT).unwrap_or_else(|| DEFAULT_OUTPUT_FORMAT.to_string());
        TextFmt::from(s.as_str())
    } else {
        TextFmt::Json
    }
}

fn merge_sink_params(
    group_name: &str,
    index: usize,
    origin: Option<&Path>,
    conn: &ConnectorRec,
    r: &RouteSink,
) -> OrionConfResult<ParamMap> {
    let sink_name = r
        .inner_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("[{}]", index));
    merge_params_with_allowlist(
        &conn.default_params,
        r.params(),
        &conn.allow_override,
        group_name,
        &sink_name,
        &conn.id,
        origin,
    )
}

fn is_nested_field_blacklisted(k: &str) -> bool {
    matches!(k, "params" | "params_override")
}

fn is_source_only_runtime_field(k: &str) -> bool {
    matches!(k, FIELD_STREAM_TAG_FIELD)
}

fn is_group_only_runtime_field(k: &str) -> bool {
    matches!(k, FIELD_WP_META_DISABLE)
}

fn validate_wp_meta_disable_fields(
    fields: &[String],
    group_name: &str,
    origin: Option<&Path>,
) -> OrionConfResult<Vec<String>> {
    let origin = origin
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let mut normalized = Vec::with_capacity(fields.len());
    for field in fields {
        let field = field.trim();
        if field.is_empty() {
            return Err(ConfIOReason::validation_error().to_err().with_detail(format!(
                "sink_group.wp_meta_disable must be an array of non-empty strings (group: {}, file: {})",
                group_name, origin
            )));
        }
        if !SUPPORTED_WP_META_DISABLE_FIELDS.contains(&field) {
            return Err(ConfIOReason::validation_error().to_err().with_detail(format!(
                "unsupported sink_group.wp_meta_disable field '{}' (supported: [{}], group: {}, file: {})",
                field,
                SUPPORTED_WP_META_DISABLE_FIELDS.join(", "),
                group_name,
                origin
            )));
        }
        normalized.push(field.to_string());
    }
    Ok(normalized)
}

/// 合并 connector 默认参数与覆盖表，并执行白名单/嵌套校验（可被 CLI/工具链共用）
fn merge_params_with_allowlist(
    base: &ParamMap,
    overrides: &ParamMap,
    allow: &[String],
    group_name: &str,
    sink_name: &str,
    conn_id: &str,
    origin: Option<&Path>,
) -> OrionConfResult<ParamMap> {
    let mut m = base.clone();
    for (k, v) in overrides.iter() {
        if is_nested_field_blacklisted(k) {
            return Err(
                ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "invalid nested table '{}' in params; please flatten and set keys [{}] directly under 'params'. Example: params = {{ {} }} or [sink_group.sinks.params] ... (group: {}, sink: {}, connector: {}, file: {})",
                        k,
                        allow.join(", "),
                        allow
                            .iter()
                            .map(|kk| format!("{}=...", kk))
                            .collect::<Vec<_>>()
                            .join(", "),
                        group_name,
                        sink_name,
                        conn_id,
                        origin
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "-".to_string())
                    )),
            );
        }
        if is_source_only_runtime_field(k) {
            return Err(ConfIOReason::validation_error().to_err().with_detail(format!(
                "sink param '{}' is source-level only; set it under source params instead of sink params (group: {}, sink: {}, connector: {}, file: {})",
                k,
                group_name,
                sink_name,
                conn_id,
                origin
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
                )));
        }
        if is_group_only_runtime_field(k) {
            return Err(ConfIOReason::validation_error().to_err().with_detail(format!(
                "sink param '{}' is group-level only; set it under [sink_group] instead of sink params (group: {}, sink: {}, connector: {}, file: {})",
                k,
                group_name,
                sink_name,
                conn_id,
                origin
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            )));
        }
        if !allow.iter().any(|x| x == k) {
            return Err(
                ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "override '{}' not allowed; whitelist: [{}] (group: {}, sink: {}, connector: {}, file: {})",
                        k,
                        allow.join(", "),
                        group_name,
                        sink_name,
                        conn_id,
                        origin
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "-".to_string())
                    )),
            );
        }
        // Type check: if the key exists in base, verify type compatibility
        if let Some(default_val) = m.get(k) {
            let expected = json_type_label(default_val);
            let provided = json_type_label(v);
            if expected != provided {
                return Err(
                    ConfIOReason::validation_error()
                        .to_err()
                        .with_detail(format!(
                            "parameter '{}' type mismatch: expected {} (from default {:?}), got {} ({:?}) (group: {}, sink: {}, connector: {}, file: {})",
                            k, expected, default_val, provided, v,
                            group_name, sink_name, conn_id,
                            origin.map(|p| p.display().to_string()).unwrap_or_else(|| "-".to_string())
                        )),
                );
            }
        }
        m.insert(k.clone(), v.clone());
    }
    Ok(m)
}

fn collect_group_matchers(rf: &RouteFile) -> (Vec<String>, Vec<String>) {
    // 处理 oml 匹配器
    let oml_vec = if let Some(oml) = &rf.sink_group.oml {
        match oml {
            StringOrArray::Single(s) => vec![s.clone()],
            StringOrArray::Multiple(v) => v.clone(),
        }
    } else {
        vec![]
    };

    // 处理 rule 匹配器
    let rule_vec = if let Some(rule) = &rf.sink_group.rule {
        match rule {
            StringOrArray::Single(s) => vec![s.clone()],
            StringOrArray::Multiple(v) => v.clone(),
        }
    } else {
        vec![]
    };

    // 如果 oml 和 rule 都存在，返回它们的组合
    if !oml_vec.is_empty() || !rule_vec.is_empty() {
        return (oml_vec, rule_vec);
    }

    (vec![], vec![])
}

fn assemble_sink_tags(
    sink: &mut crate::structure::SinkInstanceConf,
    defaults_tags: Option<&Vec<String>>,
    group_tags: Option<&Vec<String>>,
) {
    let mut merged: Vec<String> = Vec::new();
    if let Some(d) = defaults_tags {
        merged.extend(d.clone());
    }
    if let Some(gt) = group_tags {
        merged.extend(gt.clone());
    }
    if !sink.tags().is_empty() {
        merged.extend(sink.tags().clone());
    }
    sink.set_tags(merged);
}

fn apply_group_metadata(
    g: &mut crate::structure::FlexGroup,
    rf: &RouteFile,
    defaults: Option<&DefaultsBody>,
) -> OrionConfResult<()> {
    if let Some(p) = rf.sink_group.parallel {
        g.set_parallel(p);
    }
    if let Some(exp) = &rf.sink_group.expect {
        g.expect = Some(exp.clone());
    } else if g.expect.is_none()
        && let Some(def) = defaults
    {
        g.expect = Some(def.expect.clone());
    }
    if let Some(gt) = rf.sink_group.tags.as_ref() {
        g.tags = gt.clone();
    }
    if let Some(disabled) = rf.sink_group.wp_meta_disable.as_ref() {
        g.wp_meta_disable = validate_wp_meta_disable_fields(
            disabled,
            rf.sink_group.name.as_str(),
            rf.origin.as_deref(),
        )?;
    }
    if let Some(timeout_ms) = rf.sink_group.batch_timeout_ms {
        g.batch_timeout_ms = timeout_ms;
    }
    if let Some(size) = rf.sink_group.batch_size {
        g.batch_size = size;
    }
    Ok(())
}

/// 从单个 RouteFile 构建标准输出 SinkRouteConf（统一事实源）
pub fn build_route_conf_from(
    rf: &RouteFile,
    defaults: Option<&DefaultsBody>,
    conn_map: &BTreeMap<String, ConnectorRec>,
) -> OrionConfResult<crate::structure::SinkRouteConf> {
    build_route_conf_from_with(rf, defaults, conn_map, &NULL_FACTORY_LOOKUP)
}

pub fn build_route_conf_from_with(
    rf: &RouteFile,
    defaults: Option<&DefaultsBody>,
    conn_map: &BTreeMap<String, ConnectorRec>,
    reg: &dyn SinkFactoryLookup,
) -> OrionConfResult<crate::structure::SinkRouteConf> {
    // 1) 解析匹配器（oml/rule）
    let (oml_vec, rule_vec) = collect_group_matchers(rf);

    // 2) 构建每个 sink 实例（合并参数、标签、校验、插件校验）
    let mut sinks: Vec<SinkInstanceConf> = Vec::with_capacity(rf.sink_group.sinks.len());
    let mut name_guard: BTreeSet<String> = BTreeSet::new();
    for (idx, s) in rf.sink_group.sinks.iter().enumerate() {
        let conn = resolve_connector(conn_map, rf, s)?;
        let mut sink = build_sink_instance(
            rf.sink_group.name.as_str(),
            idx,
            rf.origin.as_deref(),
            conn,
            s,
        )?;
        assemble_sink_tags(
            &mut sink,
            defaults.and_then(|d| d.tags.as_ref()),
            rf.sink_group.tags.as_ref(),
        );
        ensure_unique_name(&mut name_guard, sink.name(), &rf.sink_group.name)?;
        validate_sink_instance(&sink, rf, conn)?;
        plugin_validate_with(&sink, rf, conn, reg)?;
        sinks.push(sink);
    }

    // 3) 组装 FlexiGroupConf（空列表兜底）
    if sinks.is_empty() {
        return Err(ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!("group '{}' has no sinks", rf.sink_group.name)));
    }
    let mut group = FlexGroup::build_conf(&rf.sink_group.name, sinks);
    group.oml = extend_matches(oml_vec);
    group.rule = extend_matches(rule_vec);
    apply_group_metadata(&mut group, rf, defaults)?;

    Ok(SinkRouteConf {
        version: "2.0".into(),
        sink_group: group,
    })
}

// ----- small helpers kept close to callsite for readability -----

fn resolve_connector<'a>(
    conn_map: &'a BTreeMap<String, ConnectorRec>,
    rf: &RouteFile,
    s: &RouteSink,
) -> OrionConfResult<&'a ConnectorRec> {
    conn_map.get(s.use_id()).ok_or_else(|| {
        ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!(
                "connector '{}' not found (group '{}')",
                s.use_id(),
                rf.sink_group.name
            ))
    })
}

fn ensure_unique_name(
    guard: &mut BTreeSet<String>,
    name: &str,
    group: &str,
) -> OrionConfResult<()> {
    if !guard.insert(name.to_string()) {
        return Err(ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!(
                "duplicate sink name '{}' in group '{}'",
                name, group
            )));
    }
    Ok(())
}

fn validate_sink_instance(
    sink: &crate::structure::SinkInstanceConf,
    rf: &RouteFile,
    conn: &ConnectorRec,
) -> OrionConfResult<()> {
    if let Err(e) = sink.validate() {
        return Err(ConfIOReason::validation_error()
            .to_err()
            .with_detail("sink validate error")
            .with_source(e)
            .with_context(format!(
                "group={}, sink={}, connector={}, file={}",
                rf.sink_group.name,
                sink.name(),
                conn.id,
                rf.origin
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            )));
    }
    Ok(())
}

fn plugin_validate_with(
    sink: &crate::structure::SinkInstanceConf,
    rf: &RouteFile,
    conn: &ConnectorRec,
    reg: &dyn SinkFactoryLookup,
) -> OrionConfResult<()> {
    let kind = sink.resolved_kind_str();
    if let Some(f) = reg.get(&kind) {
        let core: wp_specs::CoreSinkSpec = (sink).into();
        let resolved = crate::sinks::resolved::core_to_connector_resolved_with(
            &core,
            rf.sink_group.name.clone(),
            conn.id.clone(),
        );
        if let Err(e) = f.validate_spec(&resolved) {
            return Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail("plugin validate failed")
                .with_source(e)
                .with_context(format!(
                    "group={}, sink={}, connector={}, file={}",
                    rf.sink_group.name,
                    sink.name(),
                    conn.id,
                    rf.origin
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "-".to_string())
                )));
        }
    }
    Ok(())
}

/// 加载 business.d 下所有路由文件并构建 SinkRouteConf 列表
pub fn load_business_route_confs(
    sink_root: &str,
    dict: &EnvDict,
) -> OrionConfResult<Vec<crate::structure::SinkRouteConf>> {
    load_business_route_confs_with(sink_root, &NULL_FACTORY_LOOKUP, dict)
}

pub fn load_business_route_confs_with(
    sink_root: &str,
    reg: &dyn SinkFactoryLookup,
    dict: &EnvDict,
) -> OrionConfResult<Vec<SinkRouteConf>> {
    let conn_map = load_connectors_for(sink_root, dict)?;
    let routes = load_route_files_from(&business_dir(sink_root), dict)?;
    let defaults = load_sink_defaults(sink_root, dict)?;
    let mut out = Vec::new();
    for rf in routes.iter() {
        let conf = build_route_conf_from_with(rf, defaults.as_ref(), &conn_map, reg)?;
        out.push(conf);
    }
    Ok(out)
}

/// 加载 infra.d 下所有路由文件并构建 SinkRouteConf 列表
pub fn load_infra_route_confs(
    sink_root: &str,
    dict: &EnvDict,
) -> OrionConfResult<Vec<crate::structure::SinkRouteConf>> {
    use super::io::{infra_dir, load_connectors_for, load_route_files_from, load_sink_defaults};
    let conn_map = load_connectors_for(sink_root, dict)?;
    let routes = load_route_files_from(&infra_dir(sink_root), dict)?;
    let defaults = load_sink_defaults(sink_root, dict)?;
    let mut out = Vec::new();
    for rf in routes.iter() {
        // Infra 组不支持并行与文件分片：
        // - 禁止 [sink_group].parallel（基础组只有单消费协程，并行无效，易误导）
        if rf.sink_group.parallel.is_some() {
            return Err(
                ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "infra group '{}' does not support [sink_group].parallel; remove this field and use business.d parallel for throughput",
                        rf.sink_group.name
                    )),
            );
        }
        let conf = build_route_conf_from(rf, defaults.as_ref(), &conn_map)?;
        out.push(conf);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sinks::types::{RouteFile, RouteGroup, StringOrArray};
    use async_trait::async_trait;
    use serde_json::json;
    use wp_connector_api::{
        ConnectorDef, ConnectorScope, SinkBuildCtx, SinkDefProvider, SinkFactory, SinkHandle,
        SinkReason, SinkResult, SinkSpec,
    };

    #[test]
    fn test_collect_matchers_rule_only() {
        // 测试只有 rule 的情况（修复前会失败的场景）
        let route_file = RouteFile {
            version: Some("2.0".to_string()),
            sink_group: RouteGroup {
                name: "test_group".to_string(),
                oml: None,
                rule: Some(StringOrArray::Multiple(vec![
                    "/test/*".to_string(),
                    "/api/*".to_string(),
                ])),
                tags: None,
                wp_meta_disable: None,
                expect: None,
                sinks: vec![],
                parallel: None,
                batch_timeout_ms: None,
                batch_size: None,
            },
            origin: None,
        };

        let (oml_vec, rule_vec) = collect_group_matchers(&route_file);

        assert_eq!(oml_vec.len(), 0);
        assert_eq!(rule_vec.len(), 2);
        assert!(rule_vec.contains(&"/test/*".to_string()));
        assert!(rule_vec.contains(&"/api/*".to_string()));
    }

    #[test]
    fn test_collect_matchers_oml_only() {
        // 测试只有 oml 的情况
        let route_file = RouteFile {
            version: Some("2.0".to_string()),
            sink_group: RouteGroup {
                name: "test_group".to_string(),
                oml: Some(StringOrArray::Single("test_model".to_string())),
                rule: None,
                tags: None,
                wp_meta_disable: None,
                expect: None,
                sinks: vec![],
                parallel: None,
                batch_timeout_ms: None,
                batch_size: None,
            },
            origin: None,
        };

        let (oml_vec, rule_vec) = collect_group_matchers(&route_file);

        assert_eq!(oml_vec.len(), 1);
        assert_eq!(oml_vec[0], "test_model");
        assert_eq!(rule_vec.len(), 0);
    }

    #[test]
    fn test_collect_matchers_both_oml_and_rule() {
        // 测试同时有 oml 和 rule 的情况
        let route_file = RouteFile {
            version: Some("2.0".to_string()),
            sink_group: RouteGroup {
                name: "test_group".to_string(),
                oml: Some(StringOrArray::Multiple(vec![
                    "model1".to_string(),
                    "model2".to_string(),
                ])),
                rule: Some(StringOrArray::Single("/test/*".to_string())),
                tags: None,
                wp_meta_disable: None,
                expect: None,
                sinks: vec![],
                parallel: None,
                batch_timeout_ms: None,
                batch_size: None,
            },
            origin: None,
        };

        let (oml_vec, rule_vec) = collect_group_matchers(&route_file);

        assert_eq!(oml_vec.len(), 2);
        assert_eq!(rule_vec.len(), 1);
        assert!(oml_vec.contains(&"model1".to_string()));
        assert!(oml_vec.contains(&"model2".to_string()));
        assert_eq!(rule_vec[0], "/test/*");
    }

    #[test]
    fn test_collect_matchers_neither_oml_nor_rule() {
        // 测试既没有 oml 也没有 rule 的情况
        let route_file = RouteFile {
            version: Some("2.0".to_string()),
            sink_group: RouteGroup {
                name: "test_group".to_string(),
                oml: None,
                rule: None,
                tags: None,
                wp_meta_disable: None,
                expect: None,
                sinks: vec![],
                parallel: None,
                batch_timeout_ms: None,
                batch_size: None,
            },
            origin: None,
        };

        let (oml_vec, rule_vec) = collect_group_matchers(&route_file);

        assert_eq!(oml_vec.len(), 0);
        assert_eq!(rule_vec.len(), 0);
    }

    #[test]
    fn merge_params_rejects_sink_level_stream_tag_field() {
        let mut base = ParamMap::new();
        base.insert("base".into(), json!("./data/out_dat"));
        base.insert("file".into(), json!("default.json"));
        let mut overrides = ParamMap::new();
        overrides.insert("file".into(), json!("out.json"));
        overrides.insert("stream_tag_field".into(), json!("wp_stream_tag"));
        let allow = vec![
            "base".to_string(),
            "file".to_string(),
            "stream_tag_field".to_string(),
        ];

        let err = merge_params_with_allowlist(
            &base,
            &overrides,
            &allow,
            "/sink/test",
            "json",
            "file_json_sink",
            None,
        )
        .expect_err("stream_tag_field belongs to source config")
        .to_string();

        assert!(err.contains("source-level only"), "err={}", err);
    }

    #[test]
    fn merge_params_rejects_sink_level_wp_meta_disable() {
        let base = ParamMap::new();
        let mut overrides = ParamMap::new();
        overrides.insert("wp_meta_disable".into(), json!(["wp_oml_name"]));
        let allow = vec!["wp_meta_disable".to_string()];

        let err = merge_params_with_allowlist(
            &base,
            &overrides,
            &allow,
            "/sink/test",
            "json",
            "file_json_sink",
            None,
        )
        .expect_err("wp_meta_disable belongs to sink_group")
        .to_string();

        assert!(err.contains("group-level only"), "err={}", err);
    }

    struct StrictFactory;

    impl SinkDefProvider for StrictFactory {
        fn sink_def(&self) -> ConnectorDef {
            ConnectorDef {
                id: "strict_sink".to_string(),
                kind: "strict".to_string(),
                scope: ConnectorScope::Sink,
                allow_override: vec!["file".to_string()],
                default_params: ParamMap::new(),
                origin: None,
            }
        }
    }

    #[async_trait]
    impl SinkFactory for StrictFactory {
        fn kind(&self) -> &'static str {
            "strict"
        }

        fn validate_spec(&self, spec: &SinkSpec) -> SinkResult<()> {
            for key in ["stream_tag_field"] {
                if spec.params.contains_key(key) {
                    return Err(SinkReason::core_conf()
                        .to_err()
                        .with_detail(format!("connector saw runtime param: {key}")));
                }
            }
            Ok(())
        }

        async fn build(&self, _spec: &SinkSpec, _ctx: &SinkBuildCtx) -> SinkResult<SinkHandle> {
            Err(SinkReason::core_conf()
                .to_err()
                .with_detail("strict test factory is validate-only"))
        }
    }

    struct StrictLookup;

    impl SinkFactoryLookup for StrictLookup {
        fn get(&self, kind: &str) -> Option<Arc<dyn SinkFactory + 'static>> {
            (kind == "strict").then(|| Arc::new(StrictFactory) as Arc<dyn SinkFactory>)
        }
    }

    #[test]
    fn plugin_validate_does_not_receive_source_only_runtime_params() {
        let rf: RouteFile = toml::from_str(
            r#"
version = "2.0"
[sink_group]
name = "/sink/test"
oml = ["network.netflow"]

[[sink_group.sinks]]
name = "strict"
connect = "strict_sink"
params = { file = "out.json" }
"#,
        )
        .expect("route file");
        let mut conn_map = BTreeMap::new();
        conn_map.insert(
            "strict_sink".to_string(),
            ConnectorRec {
                id: "strict_sink".to_string(),
                kind: "strict".to_string(),
                scope: ConnectorScope::Sink,
                allow_override: vec!["file".to_string()],
                default_params: ParamMap::new(),
                origin: None,
            },
        );

        let conf =
            build_route_conf_from_with(&rf, None, &conn_map, &StrictLookup).expect("route conf");
        let sink = conf.sink_group.sinks.first().expect("sink");
        assert!(!sink.core.params.contains_key("stream_tag_field"));
    }

    #[test]
    fn group_wp_meta_disable_is_group_metadata_not_sink_param() {
        let rf: RouteFile = toml::from_str(
            r#"
version = "2.0"
[sink_group]
name = "/sink/test"
oml = ["network.netflow"]
wp_meta_disable = ["wp_oml_name"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "out.json" }
"#,
        )
        .expect("route file");
        let mut conn_map = BTreeMap::new();
        conn_map.insert(
            "file_json_sink".to_string(),
            ConnectorRec {
                id: "file_json_sink".to_string(),
                kind: "file".to_string(),
                scope: ConnectorScope::Sink,
                allow_override: vec!["base".to_string(), "file".to_string()],
                default_params: ParamMap::new(),
                origin: None,
            },
        );

        let conf = build_route_conf_from(&rf, None, &conn_map).expect("route conf");
        assert_eq!(
            conf.sink_group.wp_meta_disable.as_slice(),
            &["wp_oml_name".to_string()]
        );
        let sink = conf.sink_group.sinks.first().expect("sink");
        assert!(!sink.core.params.contains_key("wp_meta_disable"));
    }

    #[test]
    fn group_wp_meta_disable_rejects_unknown_field() {
        let rf: RouteFile = toml::from_str(
            r#"
version = "2.0"
[sink_group]
name = "/sink/test"
oml = ["network.netflow"]
wp_meta_disable = ["wp_oml_nam"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "out.json" }
"#,
        )
        .expect("route file");
        let mut conn_map = BTreeMap::new();
        conn_map.insert(
            "file_json_sink".to_string(),
            ConnectorRec {
                id: "file_json_sink".to_string(),
                kind: "file".to_string(),
                scope: ConnectorScope::Sink,
                allow_override: vec!["base".to_string(), "file".to_string()],
                default_params: ParamMap::new(),
                origin: None,
            },
        );

        let err = build_route_conf_from(&rf, None, &conn_map)
            .expect_err("unknown wp_meta_disable fields should fail")
            .to_string();
        assert!(err.contains("unsupported sink_group.wp_meta_disable field"));
        assert!(err.contains("wp_oml_nam"));
        assert!(err.contains("wp_oml_name"));
    }
}
