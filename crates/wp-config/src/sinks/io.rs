use super::types::*;
use crate::connectors::load_connector_defs_from_dir;
use orion_conf::EnvTomlLoad;
use orion_conf::error::OrionConfResult;
use orion_error::ErrorWith;
use orion_variate::{EnvDict, EnvEvalable};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wp_connector_api::ConnectorScope;

// Local constants to avoid depending on application crate
const PATH_SINK_SUBDIR: &str = "sink.d";
const PATH_BUSINESS_SUBDIR: &str = "business.d";
const PATH_INFRA_SUBDIR: &str = "infra.d";
const PATH_DEFAULTS_FILE: &str = "defaults.toml";
const FILE_EXT_TOML: &str = "toml";

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
    let f: super::types::DefaultsFile = DefaultsFile::env_load_toml(&p, _dict)?;
    Ok(Some(f.defaults))
}

pub fn business_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join(PATH_BUSINESS_SUBDIR)
}
pub fn infra_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join(PATH_INFRA_SUBDIR)
}
