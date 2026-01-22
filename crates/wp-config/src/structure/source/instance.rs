use crate::connectors::{ParamMap, param_map_from_table_ref};
use crate::structure::Validate;
use orion_conf::{
    ToStructError,
    error::{ConfIOReason, OrionConfResult},
};
use orion_error::UvsValidationFrom;
use serde::{Deserialize, Serialize};
use wp_conf_base::ConfParser;
use wp_connector_api::Tags;

/// Source 实例级配置（最小实现）：
/// - 扁平合入 CoreSourceSpec（name/type/params/tags）作为“单一事实来源”
/// - 预留 connector_id（运行期展示/诊断用）
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, derive_getters::Getters)]
pub struct SourceInstanceConf {
    #[serde(flatten)]
    pub core: wp_specs::CoreSourceSpec,
    #[serde(skip, default)]
    pub connector_id: Option<String>,
}

impl SourceInstanceConf {
    pub fn name(&self) -> &String {
        &self.core.name
    }
    pub fn tags(&self) -> &Vec<String> {
        &self.core.tags
    }
    pub fn set_name(&mut self, name: String) {
        self.core.name = name;
    }
    pub fn set_kind(&mut self, kind: String) {
        self.core.kind = kind;
    }
    pub fn kind(&self) -> String {
        self.core.kind.clone()
    }
    pub fn set_params(&mut self, params: toml::value::Table) {
        self.core.params = param_map_from_table_ref(&params);
    }
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.core.tags = tags;
    }
    pub fn resolved_kind_str(&self) -> String {
        self.core.kind.clone()
    }
    pub fn resolved_params_table(&self) -> ParamMap {
        self.core.params.clone()
    }
    pub fn new_type(
        name: String,
        kind: String,
        params: crate::connectors::ParamMap,
        tags: Vec<String>,
    ) -> Self {
        Self {
            core: wp_specs::CoreSourceSpec {
                name,
                kind,
                params,
                tags,
            },
            connector_id: None,
        }
    }
}

impl From<&SourceInstanceConf> for wp_specs::CoreSourceSpec {
    fn from(s: &SourceInstanceConf) -> Self {
        s.core.clone()
    }
}

impl Validate for SourceInstanceConf {
    fn validate(&self) -> OrionConfResult<()> {
        if self.core.name.trim().is_empty() {
            return ConfIOReason::from_validation("source.name must not be empty").err_result();
        }
        if let Err(e) = Tags::validate(&self.core.tags) {
            return ConfIOReason::from_validation(e).err_result();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    #[test]
    fn construct_min_ok() {
        let mut tbl = ParamMap::new();
        tbl.insert("path".to_string(), json!("in.dat".to_string()));
        let s = SourceInstanceConf::new_type("src1".into(), "file".into(), tbl.clone(), vec![]);
        assert_eq!(s.name(), &"src1".to_string());
        assert_eq!(s.resolved_kind_str(), "file".to_string());
        assert_eq!(s.resolved_params_table(), tbl);
        s.validate().expect("ok");
    }
}
