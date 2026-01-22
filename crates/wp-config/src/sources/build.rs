use super::types::WpSourcesConfig;
use crate::loader::traits::ConfigLoader;
use crate::sources::load_connectors_for;
use crate::sources::types::SourceConnector;
use crate::structure::{SourceInstanceConf, Validate};
use orion_conf::EnvTomlLoad;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ErrorOwe, ErrorWith, ToStructError, UvsValidationFrom};
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
        .owe_conf()
        .want("parse sources v2")?;
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
            return ConfIOReason::from_validation(format!(
                "invalid nested table '{}' in params override; please flatten and set keys [{}] directly under 'params'/'params_override'",
                k,
                allow.join(", ")
            ))
            .err_result();
        }
        if !allow.iter().any(|x| x == k) {
            return ConfIOReason::from_validation("override not allowed")
                .err_result()
                .with(allow.join(","));
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
        .owe_conf()
        .want("parse sources")?
        .env_eval(dict);
    let cnn_dict = load_connectors_for(start, dict)?;
    build_source_instances(src_conf, &cnn_dict)
}

/// 解析文件并结合 connectors 构建 CoreSourceSpec + connector_id 列表
pub fn load_source_instances_from_file(
    path: &Path,
    dict: &EnvDict,
) -> OrionConfResult<Vec<SourceInstanceConf>> {
    let content = std::fs::read_to_string(path)
        .owe_conf()
        .want("load sources config")
        .with(path)?;
    let start = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    };
    load_source_instances_from_str(&content, &start, dict)
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
        let conn = cnn_dict.get(&s.connect).ok_or_else(|| {
            ConfIOReason::from_validation(format!(
                "connector not found: '{}' (looked up under connectors/source.d)",
                s.connect
            ))
            .to_err()
        })?;
        let merged = merge_source_params(&conn.default_params, &s.params, &conn.allow_override)?;
        let mut inst = SourceInstanceConf::new_type(s.key, conn.kind.clone(), merged, s.tags);
        inst.connector_id = Some(conn.id.clone());
        srcins_confs.push(inst);
    }
    Ok(srcins_confs)
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
                ConfIOReason::from_validation(format!(
                    "plugin validate failed for source '{}' of kind '{}': {}",
                    core.name, core.kind, e
                ))
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
            .owe_conf()
            .want("parse sources")?
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
    use orion_conf::UvsConfFrom;
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
[connectors]
"#;
        // 最小解析：不校验 connectors（仅返回 name/tags）
        let _ = parse_and_validate_only(raw, &EnvDict::test_default()).expect("parse");
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
                return Err(SourceReason::from_conf("missing required param 'a'").to_err());
            }
            Ok(())
        }
        async fn build(
            &self,
            _spec: &wp_connector_api::SourceSpec,
            _ctx: &wp_connector_api::SourceBuildCtx,
        ) -> SourceResult<SourceSvcIns> {
            Err(SourceReason::from_conf("not used in validate test").to_err())
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
}
