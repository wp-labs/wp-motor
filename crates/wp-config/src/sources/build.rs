use super::types::WpSourcesConfig;
use crate::connectors::json_type_label;
use crate::loader::traits::ConfigLoader;
use crate::sources::load_connectors_for;
use crate::sources::types::SourceConnector;
use crate::structure::{SourceInstanceConf, Validate};
use orion_conf::EnvTomlLoad;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::conversion::{ErrorWith, SourceRawErr, ToStructError};
use orion_variate::{EnvDict, EnvEvaluable};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use wp_connector_api::ParamMap;

/// 仅解析并执行最小校验（不进行实际构建，不触发 I/O）
pub fn parse_and_validate_only(
    config_str: &str,
    dict: &EnvDict,
) -> OrionConfResult<Vec<wp_specs::CoreSourceSpec>> {
    let wrapper: WpSourcesConfig = WpSourcesConfig::env_parse_toml(config_str, dict)
        .map_err(|e| e.doing("parse sources v2"))?;
    let mut out: Vec<wp_specs::CoreSourceSpec> = Vec::new();
    for s in wrapper.sources.into_iter() {
        if !s.enable.unwrap_or(true) {
            continue;
        }
        out.push(wp_specs::CoreSourceSpec {
            name: s.key,
            kind: String::new(),
            params: ParamMap::new(),
            tags: s.tags,
        });
    }
    Ok(out)
}

/// whitelist + 合并参数，返回 a merged table
fn is_nested_field_blacklisted(k: &str) -> bool {
    matches!(k, "params" | "params_override")
}

fn merge_source_params(
    base: &ParamMap,
    override_tbl: &ParamMap,
    allow: &[String],
) -> OrionConfResult<ParamMap> {
    let mut out = base.clone();
    for (k, v) in override_tbl.iter() {
        if is_nested_field_blacklisted(k) {
            return Err(
                ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "invalid nested table '{}' in params override; please flatten and set keys [{}] directly under 'params'/'params_override'",
                        k,
                        allow.join(", ")
                    )),
            );
        }
        if !allow.iter().any(|x| x == k) {
            return Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail(format!(
                    "override not allowed: parameter '{}'; allowed keys: [{}]",
                    k,
                    allow.join(", ")
                ))
                .with_context(allow.join(",")));
        }
        // Type check: if the key exists in base, verify type compatibility
        if let Some(default_val) = out.get(k) {
            let expected = json_type_label(default_val);
            let provided = json_type_label(v);
            if expected != provided {
                return Err(ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "parameter '{}' type mismatch: expected {} (from default {:?}), got {} ({:?})",
                        k, expected, default_val, provided, v
                    )));
            }
        }
        out.insert(k.clone(), v.clone());
    }
    Ok(out)
}

/// 解析字符串并结合 connectors（通过 `connect` 字段）构建 CoreSourceSpec + connector_id 列表
pub fn load_source_instances_from_str(
    config_str: &str,
    start: &Path,
    dict: &EnvDict,
) -> OrionConfResult<Vec<SourceInstanceConf>> {
    let src_conf: WpSourcesConfig = WpSourcesConfig::env_parse_toml(config_str, dict)
        .map_err(|e| e.doing("parse sources"))?
        .env_eval(dict);
    let cnn_dict = load_connectors_for(start, dict)?;
    build_source_instances(src_conf, &cnn_dict)
}

/// 解析文件并结合 connectors 构建 CoreSourceSpec + connector_id 列表
///
/// - 如果 path 目录下存在 `wpsrc.toml` → 老格式：单文件 `[[sources]]` 数组
/// - 如果 path 是目录但无 `wpsrc.toml` → 新格式：扫目录下每个 `.toml`，每个文件一个 source
/// - 如果 path 是文件 → 直接解析（兼容单文件传参）
pub fn load_source_instances_from_file(
    path: &Path,
    dict: &EnvDict,
) -> OrionConfResult<Vec<SourceInstanceConf>> {
    // 目录模式：有 wpsrc.toml → 老格式，否则 → 新格式（扫 *.toml）
    if path.is_dir() {
        let wpsrc_file = path.join("wpsrc.toml");
        if wpsrc_file.exists() {
            return load_source_instances_from_file(&wpsrc_file, dict);
        }
        return load_source_instances_from_dir(path, dict);
    }
    // 文件不存在时：检查父目录是否是新格式目录（兼容旧调用者传 wpsrc.toml 文件路径）
    if !path.exists() {
        if let Some(parent) = path.parent() {
            if parent.is_dir() && !parent.join("wpsrc.toml").exists() {
                return load_source_instances_from_dir(parent, dict);
            }
        }
    }
    // 单文件模式：保留原有逻辑
    let content = std::fs::read_to_string(path)
        .source_raw_err(ConfIOReason::core_conf(), "source error")
        .doing("load sources config")
        .with_context(path)?;
    let start = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    load_source_instances_from_str(&content, &start, dict)
}

/// 扫目录下所有 `*.toml` 文件，每个文件解析为一个 source（新格式：少一层 [[sources]] 包裹）
pub fn load_source_instances_from_dir(
    dir: &Path,
    dict: &EnvDict,
) -> OrionConfResult<Vec<SourceInstanceConf>> {
    use crate::sources::types::WpSource;
    let conn_dict = load_connectors_for(dir, dict)?;
    let mut instances = Vec::new();

    let pattern = format!("{}/**/*.toml", dir.display());
    let paths: Vec<std::path::PathBuf> = glob::glob(&pattern)
        .map_err(|e| {
            ConfIOReason::core_conf()
                .to_err()
                .with_detail(format!("glob pattern failed: {}", e))
        })?
        .filter_map(|e| e.ok())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n != "wpsrc.toml")
        })
        .collect();

    for path in paths {
        let content = std::fs::read_to_string(&path)
            .source_raw_err(ConfIOReason::core_conf(), "source error")
            .doing("read source config")
            .with_context(&path)?;
        // 新格式：每个 .toml 直接是一个 WpSource，无 [[sources]] 外层
        let source: WpSource = WpSource::env_parse_toml(&content, dict)
            .map_err(|e| {
                ConfIOReason::core_conf()
                    .to_err()
                    .with_detail(format!("parse source config: {}", e))
            })?
            .env_eval(dict);
        if !source.enable.unwrap_or(true) {
            continue;
        }
        instances.push(resolve_source_instance(&source, &conn_dict)?);
    }

    if instances.is_empty() {
        return Err(ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!(
                "no enabled sources found under {}; place .toml files (one per source) or a wpsrc.toml with [[sources]]",
                dir.display()
            )));
    }
    Ok(instances)
}

/// 从 WarpSources + 连接器字典 构建 SourceInstanceConf（包含 Core + connector_id）列表
pub fn build_source_instances(
    source_conf: WpSourcesConfig,
    cnn_dict: &BTreeMap<String, SourceConnector>,
) -> OrionConfResult<Vec<SourceInstanceConf>> {
    let mut srcins_confs: Vec<SourceInstanceConf> = Vec::new();
    for s in source_conf.sources.into_iter() {
        if !s.enable.unwrap_or(true) {
            continue;
        }
        srcins_confs.push(resolve_source_instance(&s, cnn_dict)?);
    }
    Ok(srcins_confs)
}

/// Resolve one source item with the same connector lookup and parameter merge
/// semantics used by the runtime loader. Intended for observability views that
/// need to report per-item errors without aborting the whole listing.
pub fn resolve_source_instance(
    source: &super::types::WpSource,
    cnn_dict: &BTreeMap<String, SourceConnector>,
) -> OrionConfResult<SourceInstanceConf> {
    let conn = cnn_dict.get(&source.connect).ok_or_else(|| {
        ConfIOReason::validation_error()
            .to_err()
            .with_detail(format!(
                "connector not found: '{}' (looked up under connectors/source.d)",
                source.connect
            ))
    })?;
    let merged = merge_source_params(&conn.default_params, &source.params, &conn.allow_override)?;
    let mut inst = SourceInstanceConf::new_type(
        source.key.clone(),
        conn.kind.clone(),
        merged,
        source.tags.clone(),
    );
    inst.connector_id = Some(conn.id.clone());
    Ok(inst)
}

/// 使用插件 Factory 执行“类型特有校验”（不触发 I/O）。
pub trait SourceFactoryRegistry {
    fn get_factory(&self, kind: &str)
    -> Option<Arc<dyn wp_connector_api::SourceFactory + 'static>>;
}

pub fn validate_specs_with_factory(
    specs: &[SourceInstanceConf],
    reg: &dyn SourceFactoryRegistry,
) -> OrionConfResult<()> {
    for item in specs.iter() {
        let core: wp_specs::CoreSourceSpec = item.into();
        if let Some(factory) = reg.get_factory(&core.kind) {
            let resolved = crate::sources::resolved::core_to_resolved_with(
                &core,
                item.connector_id.clone().unwrap_or_default(),
            );
            factory.validate_spec(&resolved).map_err(|e| {
                ConfIOReason::validation_error()
                    .to_err()
                    .with_detail(format!(
                        "plugin validate failed for source '{}' of kind '{}'",
                        core.name, core.kind
                    ))
                    .with_source(e)
            })?;
        }
    }
    Ok(())
}

// ============================================================================
// ConfigLoader trait implementation for unified loading interface
// ============================================================================

impl ConfigLoader for Vec<SourceInstanceConf> {
    fn config_type_name() -> &'static str {
        "Sources"
    }

    fn load_from_str(content: &str, base: &Path, dict: &EnvDict) -> OrionConfResult<Self> {
        // 解析 TOML 并进行环境变量替换
        let src_conf: WpSourcesConfig = WpSourcesConfig::env_parse_toml(content, dict)
            .map_err(|e| e.doing("parse sources"))?
            .env_eval(dict);

        // 加载 connectors
        let cnn_dict = load_connectors_for(base, dict)?;

        // 构建 SourceInstanceConf 列表
        build_source_instances(src_conf, &cnn_dict)
    }

    fn validate(&self) -> OrionConfResult<()> {
        for source in self.iter() {
            source.validate()?;
        }
        Ok(())
    }
}

// 保留原有函数作为兼容层
#[deprecated(
    since = "1.8.0",
    note = "请使用 Vec::<SourceInstanceConf>::load_from_str()"
)]
pub fn load_sources_from_str_deprecated(
    config_str: &str,
    start: &Path,
    dict: &EnvDict,
) -> OrionConfResult<Vec<SourceInstanceConf>> {
    Vec::<SourceInstanceConf>::load_from_str(config_str, start, dict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{io, types};
    use crate::test_support::ForTest;
    use orion_variate::EnvDict;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wp_connector_api::{ConnectorScope, SourceReason, SourceResult, SourceSvcIns};

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

    #[test]
    fn parse_minimal_ok() {
        let raw = r#"[[sources]]
key = "s1"
connect = "conn1"
"#;
        // 最小解析：不校验 connectors（仅返回 name/tags）
        let _ = parse_and_validate_only(raw, &EnvDict::test_default()).expect("parse");
    }

    #[test]
    fn parse_rejects_unknown_top_level_connectors_table() {
        let raw = r#"[[connectors]]
key = "s1"
enable = true
connect = "conn1"
[connectors.params]
addr = "127.0.0.1"
"#;
        let err = parse_and_validate_only(raw, &EnvDict::test_default())
            .expect_err("unknown top-level connectors table should fail")
            .to_string();
        assert!(!err.is_empty());
    }

    #[test]
    fn parse_rejects_unknown_source_field() {
        let raw = r#"[[sources]]
key = "s1"
enable = true
connect = "conn1"
connector = "typo"
"#;
        let err = parse_and_validate_only(raw, &EnvDict::test_default())
            .expect_err("unknown source field should fail")
            .to_string();
        assert!(!err.is_empty());
    }

    #[test]
    fn merge_params_whitelist_ok_and_err() {
        let mut base = ParamMap::new();
        base.insert("endpoint".into(), json!("127.0.0.1"));
        let allow = vec!["path".to_string(), "fmt".to_string()];

        // ok: allowed key
        let mut over = ParamMap::new();
        over.insert("path".into(), json!("/a"));
        let ok = merge_source_params(&base, &over, &allow).expect("ok");
        assert_eq!(ok.get("path").and_then(|v| v.as_str()), Some("/a"));

        // err: disallowed key
        let mut bad = ParamMap::new();
        bad.insert("badkey".into(), json!("v"));
        let e = merge_source_params(&base, &bad, &allow)
            .expect_err("err")
            .to_string();
        assert!(e.contains("override not allowed"));

        // err: nested blacklisted field
        let mut nested = ParamMap::new();
        nested.insert("params".into(), json!("x"));
        let e2 = merge_source_params(&base, &nested, &allow)
            .expect_err("err")
            .to_string();
        assert!(e2.contains("invalid nested table"));
    }

    #[test]
    fn specs_from_wrapper_filters_disabled() {
        let cmap = {
            let mut m = BTreeMap::new();
            m.insert(
                "c1".to_string(),
                SourceConnector {
                    id: "c1".into(),
                    kind: "dummy".into(),
                    scope: ConnectorScope::Source,
                    allow_override: vec!["a".into()],
                    default_params: ParamMap::new(),
                    origin: None,
                },
            );
            m
        };
        let w = WpSourcesConfig {
            sources: vec![
                types::WpSource {
                    key: "s1".into(),
                    enable: Some(false),
                    connect: "c1".into(),
                    tags: vec![],
                    params: ParamMap::new(),
                },
                types::WpSource {
                    key: "s2".into(),
                    enable: Some(true),
                    connect: "c1".into(),
                    tags: vec![],
                    params: ParamMap::new(),
                },
            ],
        };
        let specs = build_source_instances(w, &cmap).expect("specs");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name(), &"s2".to_string());
    }

    #[test]
    fn connectors_dedup_detected() {
        let base = tmp_dir("src_conn");
        let cdir = base.join("connectors").join("source.d");
        fs::create_dir_all(&cdir).unwrap();
        // write two files with same id
        fs::write(
            cdir.join("a.toml"),
            r#"[[connectors]]
id = "c1"
type = "dummy"
[connectors.params]
"#,
        )
        .unwrap();
        fs::write(
            cdir.join("b.toml"),
            r#"[[connectors]]
id = "c1"
type = "dummy"
[connectors.params]
"#,
        )
        .unwrap();
        let e = io::load_connectors_for(&base, &EnvDict::test_default())
            .expect_err("dup err")
            .to_string();
        assert!(e.contains("duplicate connector id"));
    }

    use crate::connectors::ConnectorDef;
    use crate::connectors::ParamMap;
    use wp_connector_api::SourceFactory;

    struct DummyFactory;
    #[allow(clippy::needless_lifetimes)]
    #[async_trait::async_trait]
    impl wp_connector_api::SourceFactory for DummyFactory {
        fn kind(&self) -> &'static str {
            "dummy"
        }
        fn validate_spec(&self, spec: &wp_connector_api::SourceSpec) -> SourceResult<()> {
            // require key 'a' in params
            if !spec.params.contains_key("a") {
                return Err(SourceReason::core_conf().to_err());
            }
            Ok(())
        }
        async fn build(
            &self,
            _spec: &wp_connector_api::SourceSpec,
            _ctx: &wp_connector_api::SourceBuildCtx,
        ) -> SourceResult<SourceSvcIns> {
            Err(SourceReason::core_conf().to_err())
        }
    }

    impl wp_connector_api::SourceDefProvider for DummyFactory {
        fn source_def(&self) -> ConnectorDef {
            ConnectorDef {
                id: "dummy".into(),
                kind: self.kind().into(),
                scope: ConnectorScope::Source,
                allow_override: vec!["a".into()],
                default_params: ParamMap::new(),
                origin: Some("test:dummy".into()),
            }
        }
    }

    struct DummyReg;
    impl SourceFactoryRegistry for DummyReg {
        fn get_factory(
            &self,
            kind: &str,
        ) -> Option<Arc<dyn wp_connector_api::SourceFactory + 'static>> {
            if kind == "dummy" {
                Some(Arc::new(DummyFactory))
            } else {
                None
            }
        }
    }

    #[test]
    fn plugin_validate_fails_without_param() {
        // prepare one spec without 'a'
        let mut inst =
            SourceInstanceConf::new_type("s1".into(), "dummy".into(), ParamMap::new(), vec![]);
        inst.connector_id = Some("c1".into());
        let reg = DummyReg;
        let err = validate_specs_with_factory(&[inst], &reg)
            .expect_err("error")
            .to_string();
        assert!(err.contains("plugin validate failed"));
    }

    // ========================================================================
    // ConfigLoader trait tests
    // ========================================================================

    #[test]
    fn config_loader_load_from_str_works() {
        use crate::loader::traits::ConfigLoader;

        let base = tmp_dir("src_cfg_loader");
        let cdir = base.join("connectors").join("source.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建一个 connector 配置
        fs::write(
            cdir.join("dummy.toml"),
            r#"[[connectors]]
id = "dummy_conn"
type = "dummy"
allow_override = ["a", "b"]
[connectors.params]
a = "default_a"
"#,
        )
        .unwrap();

        // 使用 ConfigLoader trait 加载 sources
        let sources_toml = r#"
[[sources]]
key = "test_source"
connect = "dummy_conn"
[sources.params]
a = "custom_a"
"#;

        let result =
            Vec::<SourceInstanceConf>::load_from_str(sources_toml, &base, &EnvDict::test_default());

        assert!(result.is_ok(), "应该成功加载");
        let sources = result.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name(), &"test_source".to_string());
    }

    #[test]
    fn config_loader_load_from_path_works() {
        use crate::loader::traits::ConfigLoader;

        let base = tmp_dir("src_cfg_path");
        let cdir = base.join("connectors").join("source.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建 connector 配置
        fs::write(
            cdir.join("conn.toml"),
            r#"[[connectors]]
id = "conn1"
type = "dummy"
[connectors.params]
"#,
        )
        .unwrap();

        // 创建 sources 配置文件
        let sources_file = base.join("sources.toml");
        fs::write(
            &sources_file,
            r#"
[[sources]]
key = "src1"
connect = "conn1"
"#,
        )
        .unwrap();

        // 使用 load_from_path
        let result =
            Vec::<SourceInstanceConf>::load_from_path(&sources_file, &EnvDict::test_default());

        assert!(result.is_ok(), "load_from_path 应该成功");
        let sources = result.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name(), &"src1".to_string());
    }

    #[test]
    fn config_loader_validation_called() {
        use crate::loader::traits::ConfigLoader;

        let base = tmp_dir("src_cfg_validate");
        let cdir = base.join("connectors").join("source.d");
        fs::create_dir_all(&cdir).unwrap();

        fs::write(
            cdir.join("conn.toml"),
            r#"[[connectors]]
id = "conn1"
type = "dummy"
[connectors.params]
"#,
        )
        .unwrap();

        // 创建一个无效的 source（空 name）
        let invalid_file = base.join("invalid.toml");
        fs::write(
            &invalid_file,
            r#"
[[sources]]
key = ""
connect = "conn1"
"#,
        )
        .unwrap();

        // 使用 load_from_path（会自动调用验证）
        let result =
            Vec::<SourceInstanceConf>::load_from_path(&invalid_file, &EnvDict::test_default());

        // 应该验证失败
        assert!(result.is_err(), "空 name 应该验证失败");
    }

    #[test]
    fn load_from_dir_without_wpsrc_uses_new_format() {
        let base = tmp_dir("src_dir_new");
        let cdir = base.join("connectors").join("source.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建 connector 配置
        fs::write(
            cdir.join("conn.toml"),
            r#"[[connectors]]
id = "file_src"
type = "file"
allow_override = ["file","encode"]
[connectors.params]
encode = "text"
file = "gen.dat"
"#,
        )
        .unwrap();

        // 新格式：每个 .toml 一个 source
        let src_dir = base.join("topology").join("sources");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("file_1.toml"),
            r#"key = "file_1"
connect = "file_src"
enable = true
[params]
encode = "text"
file = "gen.dat"
"#,
        )
        .unwrap();
        fs::write(
            src_dir.join("file_2.toml"),
            r#"key = "file_2"
enable = false
connect = "file_src"
[params]
encode = "text"
file = "gen2.dat"
"#,
        )
        .unwrap();

        // 传目录路径
        let result = load_source_instances_from_file(&src_dir, &EnvDict::test_default());
        assert!(
            result.is_ok(),
            "dir load should succeed: {:?}",
            result.err()
        );
        let instances = result.unwrap();
        // file_2 has enable=false, should be skipped
        assert_eq!(instances.len(), 1, "only enabled sources should be loaded");
        assert_eq!(instances[0].name(), "file_1");
    }

    #[test]
    fn load_from_non_existent_file_falls_back_to_parent_dir() {
        let base = tmp_dir("src_fallback");
        let cdir = base.join("connectors").join("source.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建 connector 配置
        fs::write(
            cdir.join("conn.toml"),
            r#"[[connectors]]
id = "file_src"
type = "file"
allow_override = ["file","encode"]
[connectors.params]
encode = "text"
file = "gen.dat"
"#,
        )
        .unwrap();

        // 新格式目录（有 .toml 文件，无 wpsrc.toml）
        let src_dir = base.join("topology").join("sources");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("tcp_1.toml"),
            r#"key = "tcp_1"
connect = "file_src"
[params]
encode = "text"
file = "gen.dat"
"#,
        )
        .unwrap();

        // 传不存在的 wpsrc.toml 文件路径 — 应回退到父目录扫描
        let fake_wpsrc = src_dir.join("wpsrc.toml");
        assert!(!fake_wpsrc.exists());
        let result = load_source_instances_from_file(&fake_wpsrc, &EnvDict::test_default());
        assert!(
            result.is_ok(),
            "fallback should succeed: {:?}",
            result.err()
        );
        let instances = result.unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].name(), "tcp_1");
    }

    #[test]
    fn load_from_non_existent_file_without_parent_dir_fails() {
        // 不存在的路径，父目录也不存在 → 应报错
        let phantom = std::path::PathBuf::from("/nonexistent_xyz_test/sources/wpsrc.toml");
        let result = load_source_instances_from_file(&phantom, &EnvDict::test_default());
        assert!(result.is_err(), "truly missing path should fail");
    }
}
