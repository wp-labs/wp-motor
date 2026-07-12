use crate::connectors::ConnectorTomlFile;
use crate::structure::GroupExpectSpec;
use crate::structure::SinkExpectOverride;
use crate::utils::env_eval_params;
use crate::utils::env_eval_vec;
use orion_variate::EnvEvaluable;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use wp_connector_api::ConnectorDef;
use wp_connector_api::ParamMap;

pub type ConnectorFile = ConnectorTomlFile;
pub type ConnectorRec = ConnectorDef;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RouteFile {
    #[serde(default)]
    pub version: Option<String>,
    pub sink_group: RouteGroup,
    /// 原始文件路径（IO 层注入；用于错误上下文）
    #[serde(skip)]
    pub origin: Option<PathBuf>,
}
impl EnvEvaluable<RouteFile> for RouteFile {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> RouteFile {
        self.sink_group = self.sink_group.env_eval(dict);
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RouteGroup {
    pub name: String,
    #[serde(default)]
    pub parallel: Option<usize>,
    #[serde(default)]
    pub oml: Option<StringOrArray>,
    #[serde(default)]
    pub rule: Option<StringOrArray>,
    /// 组级标签
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Group-level metadata payload fields disabled for all sinks in this group.
    #[serde(default)]
    pub wp_meta_disable: Option<Vec<String>>,
    #[serde(default)]
    pub expect: Option<GroupExpectSpec>,
    /// 批量超时时间，单位：毫秒，默认 300ms
    #[serde(default)]
    pub batch_timeout_ms: Option<u64>,
    /// 批量缓冲大小，默认 1024 条记录
    #[serde(default)]
    pub batch_size: Option<usize>,
    #[serde(default)]
    pub sinks: Vec<RouteSink>,
}

impl EnvEvaluable<RouteGroup> for RouteGroup {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> Self {
        self.name = self.name.env_eval(dict);
        if let Some(tags) = self.tags {
            self.tags = Some(env_eval_vec(tags, dict));
        }
        if let Some(wp_meta_disable) = self.wp_meta_disable {
            self.wp_meta_disable = Some(env_eval_vec(wp_meta_disable, dict));
        }
        self.sinks = env_eval_vec(self.sinks, dict);
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RouteSink {
    #[serde(rename = "use", alias = "connect", alias = "connector")]
    connect: String,
    /// 同一 sink_group 内唯一名称（配置字段仍为 `name`）
    #[serde(default, rename = "name")]
    inner_name: Option<String>,
    #[serde(default)]
    params: ParamMap,
    /// sink 级标签
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    expect: Option<SinkExpectOverride>,
    #[serde(default)]
    filter: Option<String>,
    /// 当 cond 结果等于该值时投递；默认为 true
    #[serde(default = "crate_default_true")]
    filter_expect: bool,
}

impl EnvEvaluable<RouteSink> for RouteSink {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> RouteSink {
        self.connect = self.connect.env_eval(dict);
        self.inner_name = self.inner_name.env_eval(dict);
        self.params = env_eval_params(self.params, dict);
        if let Some(tags) = self.tags {
            self.tags = Some(env_eval_vec(tags, dict));
        }
        self.filter = self.filter.env_eval(dict);
        self
    }
}

impl RouteSink {
    pub fn use_id(&self) -> &str {
        self.connect.as_str()
    }
    pub fn inner_name(&self) -> Option<&str> {
        self.inner_name.as_deref()
    }
    pub fn params(&self) -> &ParamMap {
        &self.params
    }
    pub fn expect(&self) -> Option<&SinkExpectOverride> {
        self.expect.as_ref()
    }
    pub fn filter_path(&self) -> Option<&str> {
        self.filter.as_deref()
    }
    pub fn tags(&self) -> Option<&Vec<String>> {
        self.tags.as_ref()
    }
    pub fn filter_expect(&self) -> bool {
        self.filter_expect
    }
}

fn crate_default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum StringOrArray {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrArray {
    pub fn as_vec(&self) -> Vec<String> {
        match self {
            StringOrArray::Single(s) => vec![s.clone()],
            StringOrArray::Multiple(v) => v.clone(),
        }
    }
}

// 为了向后兼容，保留旧名称的别名
#[deprecated(note = "Use StringOrArray instead")]
pub type StrOrVec = StringOrArray;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DefaultsBody {
    #[serde(default)]
    pub tags: Option<Vec<String>>, // 每层 <=4；留给上层合并
    pub expect: GroupExpectSpec,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultsFile {
    pub defaults: DefaultsBody,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_file_rejects_unknown_top_level_field() {
        let err = toml::from_str::<RouteFile>(
            r#"
            versoin = "1.0"

            [sink_group]
            name = "demo"
            "#,
        )
        .expect_err("unknown top-level route fields should fail")
        .to_string();
        assert!(err.contains("unknown field"));
        assert!(err.contains("versoin"));
    }

    #[test]
    fn route_sink_rejects_unknown_field_but_keeps_params_open() {
        let ok = toml::from_str::<RouteFile>(
            r#"
            [sink_group]
            name = "demo"

            [[sink_group.sinks]]
            use = "custom_sink"
            [sink_group.sinks.params]
            arbitrary_plugin_key = "kept"
            nested = { value = 1 }
            "#,
        )
        .expect("plugin params should remain open");
        assert_eq!(ok.sink_group.sinks.len(), 1);

        let err = toml::from_str::<RouteFile>(
            r#"
            [sink_group]
            name = "demo"

            [[sink_group.sinks]]
            use = "custom_sink"
            param = { arbitrary_plugin_key = "typo" }
            "#,
        )
        .expect_err("unknown route sink fields should fail")
        .to_string();
        assert!(err.contains("unknown field"));
        assert!(err.contains("param"));
    }

    #[test]
    fn defaults_file_rejects_unknown_fields() {
        let err = toml::from_str::<DefaultsFile>(
            r#"
            [defaults]
            tag = ["typo"]

            [defaults.expect]
            basis = "group_input"
            "#,
        )
        .expect_err("unknown defaults fields should fail")
        .to_string();
        assert!(err.contains("unknown field"));
        assert!(err.contains("tag"));
    }

    #[test]
    fn route_group_wp_meta_disable_env_eval() {
        let route = toml::from_str::<RouteFile>(
            r#"
            [sink_group]
            name = "demo"
            wp_meta_disable = ["${META_FIELD}"]

            [[sink_group.sinks]]
            use = "custom_sink"
            "#,
        )
        .expect("route file");
        let mut dict = orion_variate::EnvDict::new();
        dict.insert("META_FIELD", orion_variate::ValueType::from("wp_oml_name"));

        let route = route.env_eval(&dict);

        assert_eq!(
            route
                .sink_group
                .wp_meta_disable
                .as_ref()
                .and_then(|items| items.first())
                .map(String::as_str),
            Some("wp_oml_name")
        );
    }
}
