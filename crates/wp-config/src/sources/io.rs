use crate::common::io_locate::find_connectors_base_dir as resolve_base;
use crate::connectors::load_connector_defs_from_dir;
use orion_conf::error::OrionConfResult;
use orion_variate::EnvDict;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wp_connector_api::ConnectorScope;

use super::types::SourceConnector;

/// 自任意起点向上寻找 `connectors/source.d` 并返回其绝对路径（不再支持旧布局）
pub fn find_connectors_dir(start: &Path) -> Option<PathBuf> {
    resolve_base(start, "source.d")
}

/// Legacy alias retained for CLI compatibility
pub fn resolve_connectors_base_dir(start: &Path) -> Option<PathBuf> {
    find_connectors_dir(start)
}

/// 加载 `connectors/source.d` 下的全部连接器（去重校验 id）
pub fn load_connectors_for(
    start: &Path,
    dict: &EnvDict,
) -> OrionConfResult<BTreeMap<String, SourceConnector>> {
    let mut map = BTreeMap::new();
    if let Some(dir) = find_connectors_dir(start) {
        for def in load_connector_defs_from_dir(&dir, ConnectorScope::Source, dict)? {
            map.insert(def.id.clone(), def);
        }
    }
    Ok(map)
}
