use super::types::*;
use crate::connectors::load_connector_defs_from_dir;
use orion_conf::EnvTomlLoad;
use orion_conf::error::OrionConfResult;
use orion_error::{ErrorWith, OperationContext};
use orion_variate::{EnvDict, EnvEvalable};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wp_connector_api::ConnectorScope;
use wp_error::diagnostic_meta::{ConfigGroup, ConfigKind, HintCode, OperationContextMetaExt};

// Local constants to avoid depending on application crate
const PATH_SINK_SUBDIR: &str = "sink.d";
const PATH_BUSINESS_SUBDIR: &str = "business.d";
const PATH_INFRA_SUBDIR: &str = "infra.d";
const PATH_DEFAULTS_FILE: &str = "defaults.toml";
const FILE_EXT_TOML: &str = "toml";

fn route_group_label(dir: &Path) -> Option<ConfigGroup> {
    let lower = dir.to_string_lossy().to_ascii_lowercase();
    if lower.contains("/infra.d") || lower.ends_with("infra.d") {
        Some(ConfigGroup::Infra)
    } else if lower.contains("/business.d") || lower.ends_with("business.d") {
        Some(ConfigGroup::Business)
    } else {
        None
    }
}

fn sink_route_context(path: &Path, group: Option<ConfigGroup>) -> OperationContext {
    let ctx = OperationContext::new()
        .with_meta_value(ConfigKind::SinkRoute)
        .with_meta_value(HintCode::SinkRouteTomlSchema)
        .with_file_path(path);
    if let Some(group) = group {
        ctx.with_meta_value(group)
    } else {
        ctx
    }
}

fn sink_defaults_context(path: &Path) -> OperationContext {
    OperationContext::want("load sink defaults")
        .with_meta_value(ConfigKind::SinkDefaults)
        .with_meta_value(HintCode::SinkDefaultsTomlSchema)
        .with_file_path(path)
}

pub fn find_connectors_base_dir(sink_root: &Path) -> Option<PathBuf> {
    // 复用公共定位逻辑，传入 sinks 的子目录名
    crate::common::io_locate::find_connectors_base_dir(sink_root, PATH_SINK_SUBDIR)
}

pub fn load_connectors_for(
    sink_root: &str,
    dict: &EnvDict,
) -> OrionConfResult<BTreeMap<String, ConnectorRec>> {
    let mut map = BTreeMap::new();
    if let Some(dir) = find_connectors_base_dir(Path::new(sink_root)) {
        for def in load_connector_defs_from_dir(&dir, ConnectorScope::Sink, dict)? {
            map.insert(def.id.clone(), def);
        }
    }
    Ok(map)
}

pub fn load_route_files_from(dir: &Path, dict: &EnvDict) -> OrionConfResult<Vec<RouteFile>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let group = route_group_label(dir);
    // 递归收集 business.d/ 或 infra.d/ 下所有 *.toml 文件，支持子目录
    // 使用 glob "<dir>/**/*.toml" 以兼容多平台路径
    let pattern = format!("{}/**/*.{}", dir.display(), FILE_EXT_TOML);
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = glob::glob(&pattern) {
        for path in entries.flatten() {
            if path.is_file() {
                files.push(path);
            }
        }
    }
    // 统一去重：以规范化（canonicalize）后的路径作为 key，避免 "./a.toml" 与 "a.toml" 视为不同
    use std::collections::BTreeSet;
    let mut uniq: BTreeSet<String> = BTreeSet::new();
    for fp in files.into_iter() {
        let key = std::fs::canonicalize(&fp)
            .unwrap_or(fp.clone())
            .display()
            .to_string();
        uniq.insert(key);
    }

    for fstr in uniq.into_iter() {
        let fp = Path::new(&fstr).to_path_buf();
        let mut rf: RouteFile = RouteFile::env_load_toml(&fp, dict)
            .with(sink_route_context(&fp, group))
            .with(&fp)?
            .env_eval(dict);
        rf.origin = Some(fp.clone());
        out.push(rf);
    }
    Ok(out)
}

pub fn load_sink_defaults<P: AsRef<Path>>(
    sink_root: P,
    _dict: &EnvDict,
) -> OrionConfResult<Option<DefaultsBody>> {
    let p = sink_root.as_ref().join(PATH_DEFAULTS_FILE);
    if !p.exists() {
        return Ok(None);
    }
    let f: super::types::DefaultsFile = DefaultsFile::env_load_toml(&p, _dict)
        .with(sink_defaults_context(&p))
        .with(&p)
        .want("load sink defaults")?;
    Ok(Some(f.defaults))
}

pub fn business_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join(PATH_BUSINESS_SUBDIR)
}
pub fn infra_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join(PATH_INFRA_SUBDIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ForTest;
    use orion_error::ErrorCode;
    use orion_variate::EnvDict;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wp_error::diagnostic_meta::{ConfigGroup, ConfigKind, MetaValue, key};

    fn tmp_dir(prefix: &str) -> PathBuf {
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
    fn route_file_parse_error_carries_sink_route_metadata() {
        let base = tmp_dir("sink_route_meta");
        let route_dir = infra_dir(&base);
        fs::create_dir_all(&route_dir).unwrap();
        let route_file = route_dir.join("miss.toml");
        fs::write(&route_file, "not = [valid").unwrap();

        let err = load_route_files_from(&route_dir, &EnvDict::test_default()).expect_err("parse");
        let report = err.report();
        let route_file_str = fs::canonicalize(&route_file)
            .unwrap_or(route_file.clone())
            .display()
            .to_string();

        assert_eq!(
            report.root_metadata.get_str(key::CONFIG_KIND),
            Some(ConfigKind::SinkRoute.as_str())
        );
        assert_eq!(
            report.root_metadata.get_str(key::CONFIG_GROUP),
            Some(ConfigGroup::Infra.as_str())
        );
        assert_eq!(
            report.root_metadata.get_str(key::FILE_PATH),
            Some(route_file_str.as_str())
        );
    }

    #[test]
    fn sink_defaults_parse_error_carries_sink_defaults_metadata() {
        let base = tmp_dir("sink_defaults_meta");
        let defaults = base.join(PATH_DEFAULTS_FILE);
        fs::write(&defaults, "version = \"2.0\"\n").unwrap();

        let err = load_sink_defaults(&base, &EnvDict::test_default()).expect_err("parse defaults");
        let report = err.report();
        let defaults_str = defaults.display().to_string();

        assert_eq!(
            report.root_metadata.get_str(key::CONFIG_KIND),
            Some(ConfigKind::SinkDefaults.as_str())
        );
        assert_eq!(
            report.root_metadata.get_str(key::FILE_PATH),
            Some(defaults_str.as_str())
        );
        assert_eq!(err.error_code(), 500);
    }
}
