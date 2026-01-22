use crate::{
    connectors::ConnectorTomlFile,
    utils::{env_eval_params, env_eval_vec},
};
use getset::WithSetters;
use orion_variate::EnvEvaluable;
use serde::{Deserialize, Serialize};
use wp_connector_api::{ConnectorDef, ParamMap};

pub type SrcConnectorFileRec = ConnectorTomlFile;
pub type SourceConnector = ConnectorDef;

#[derive(Debug, Clone, Deserialize, Serialize, WithSetters)]
pub struct WpSource {
    pub key: String,
    #[set_with = "pub"]
    #[serde(default)]
    pub enable: Option<bool>,
    pub connect: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "params", alias = "params_override")]
    pub params: ParamMap,
}

impl EnvEvaluable<WpSource> for WpSource {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> WpSource {
        self.key = self.key.env_eval(dict);
        self.connect = self.connect.env_eval(dict);
        self.tags = env_eval_vec(self.tags, dict);
        self.params = env_eval_params(self.params, dict);
        self
    }
}

/// Deprecated alias: maintained for crates that still refer to `SourceItem`
pub type SourceItem = WpSource;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WpSourcesConfig {
    #[serde(default)]
    pub sources: Vec<WpSource>,
}

impl EnvEvaluable<WpSourcesConfig> for WpSourcesConfig {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> Self {
        self.sources = env_eval_vec(self.sources, dict);
        self
    }
}

/// Legacy alias for compatibility with tooling referencing `WarpSources`
pub type WarpSources = WpSourcesConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use orion_variate::{EnvDict, ValueType};
    use serde_json::json;

    #[test]
    fn env_eval_rewrites_source_fields() {
        let mut params = ParamMap::new();
        params.insert("base".into(), json!("${WORK_ROOT}/in"));
        params.insert("file".into(), json!("${FILE_NAME}"));
        let source = WpSource {
            key: "${SRC_KEY}".into(),
            enable: Some(true),
            connect: "${CONNECTOR}".into(),
            tags: vec!["env-${TAG}".into()],
            params,
        };
        let mut dict = EnvDict::new();
        dict.insert("SRC_KEY", ValueType::from("file_src"));
        dict.insert("CONNECTOR", ValueType::from("file_main"));
        dict.insert("TAG", ValueType::from("prod"));
        dict.insert("WORK_ROOT", ValueType::from("/tmp/work"));
        dict.insert("FILE_NAME", ValueType::from("input.log"));

        let evaluated = source.env_eval(&dict);
        assert_eq!(evaluated.key, "file_src");
        assert_eq!(evaluated.connect, "file_main");
        assert_eq!(evaluated.tags, vec!["env-prod".to_string()]);
        assert_eq!(
            evaluated.params.get("base").and_then(|v| v.as_str()),
            Some("/tmp/work/in")
        );
        assert_eq!(
            evaluated.params.get("file").and_then(|v| v.as_str()),
            Some("input.log")
        );
    }

    #[test]
    fn env_eval_on_sources_config_propagates_to_items() {
        let mut params = ParamMap::new();
        params.insert("path".into(), json!("${PATH}"));
        let cfg = WpSourcesConfig {
            sources: vec![WpSource {
                key: "${KEY}".into(),
                enable: None,
                connect: "${CONNECT}".into(),
                tags: vec![],
                params,
            }],
        };
        let mut dict = EnvDict::new();
        dict.insert("KEY", ValueType::from("src"));
        dict.insert("CONNECT", ValueType::from("file"));
        dict.insert("PATH", ValueType::from("/tmp/a.dat"));

        let evaluated = cfg.env_eval(&dict);
        let src = evaluated.sources.first().unwrap();
        assert_eq!(src.key, "src");
        assert_eq!(src.connect, "file");
        assert_eq!(
            src.params.get("path").and_then(|v| v.as_str()),
            Some("/tmp/a.dat")
        );
    }
}
