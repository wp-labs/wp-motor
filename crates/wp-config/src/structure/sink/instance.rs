use super::expect::SinkExpectOverride;
use crate::types::AnyResult;
use crate::utils::{env_eval_params, env_eval_vec};
use crate::{cond::WarpConditionParser, structure::Validate};
use derive_getters::Getters;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{ErrorOwe, ErrorWith, ToStructError};
use orion_error::{ContextRecord, OperationContext, UvsValidationFrom};
use orion_variate::EnvEvaluable;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use wp_conf_base::ConfParser;
use wp_connector_api::{ParamMap, Tags};
use wp_log::{debug_data, info_data};
use wp_model_core::model::fmt_def::TextFmt;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Getters)]
pub struct SinkInstanceConf {
    /// 组合核心规格：name/type/params/filter/tags 由 CoreSinkSpec 承担，扁平合入
    #[serde(flatten)]
    pub core: wp_specs::CoreSinkSpec,
    #[serde(default)]
    pub fmt: TextFmt,
    #[serde(default)]
    pub expect: Option<SinkExpectOverride>,
    /// 当 cond 结果等于该值时投递；默认为 true
    #[serde(default = "default_true")]
    filter_expect: bool,
    #[serde(skip, default)]
    pub connector_id: Option<String>,
    /// 运行期上下文：所属组名（仅在路由装配阶段注入；不参与序列化）
    #[serde(skip, default)]
    pub group_name: Option<String>,
}

impl EnvEvaluable<SinkInstanceConf> for SinkInstanceConf {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> SinkInstanceConf {
        self.core.name = self.core.name.env_eval(dict);
        self.core.kind = self.core.kind.env_eval(dict);
        self.core.params = env_eval_params(self.core.params, dict);
        self.core.tags = env_eval_vec(self.core.tags, dict);
        self.core.filter = self.core.filter.env_eval(dict);
        self.connector_id = self.connector_id.env_eval(dict);
        self
    }
}

// derive(Deserialize) via flatten core (CoreSinkSpec)

impl SinkInstanceConf {
    pub fn name(&self) -> &String {
        &self.core.name
    }
    pub fn filter(&self) -> &Option<String> {
        &self.core.filter
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
    pub fn set_params(&mut self, params: ParamMap) {
        self.core.params = params;
    }
    pub fn set_filter(&mut self, filter: Option<String>) {
        self.core.filter = filter;
    }
    pub fn set_filter_expect(&mut self, v: bool) {
        self.filter_expect = v;
    }
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.core.tags = tags;
    }
    pub fn resolve_file_path(&self) -> Option<String> {
        if self.core.kind == "file" || self.core.kind == "test_rescue" {
            if self.core.params.contains_key("base") || self.core.params.contains_key("file") {
                let base = self
                    .core
                    .params
                    .get("base")
                    .and_then(|v| v.as_str())
                    .unwrap_or("./data/out_dat");
                let file = self
                    .core
                    .params
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("out.dat");
                return Some(format!("{}/{}", base, file));
            }
            if let Some(p) = self.core.params.get("path").and_then(|v| v.as_str()) {
                return Some(p.to_string());
            }
        }
        None
    }

    pub fn new_type(
        name: String,
        fmt: TextFmt,
        kind: String,
        params: ParamMap,
        filter: Option<String>,
    ) -> Self {
        Self {
            core: wp_specs::CoreSinkSpec {
                name,
                kind,
                params,
                filter,
                tags: Vec::new(),
            },
            fmt,
            expect: None,
            connector_id: None,
            group_name: None,
            filter_expect: true,
        }
    }

    pub fn read_filter_content(&self) -> Option<String> {
        if let Some(path) = &self.core.filter {
            debug_data!("filter path: {}", path);
            if Path::new(path.as_str()).exists()
                && let Ok(conf) = fs::read_to_string(path.as_str())
                && !conf.is_empty()
            {
                info_data!("found path : {}", path);
                return Some(conf);
            }
            info_data!("not found filter : {}", path);
        }
        None
    }

    pub fn file_new<P: AsRef<Path>>(
        name: String,
        txt_fmt: TextFmt,
        path: P,
        filter: Option<String>,
    ) -> Self {
        let mut params = ParamMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String(path.as_ref().display().to_string()),
        );
        Self::new_type(name, txt_fmt, "file".to_string(), params, filter)
    }

    pub fn null_new(name: String, fmt: TextFmt, filter: Option<String>) -> Self {
        Self::new_type(name, fmt, "null".to_string(), ParamMap::default(), filter)
    }
    pub fn clean_sink_file(&self) -> AnyResult<()> {
        if let Some(path) = self.resolve_file_path() {
            if std::path::Path::new(path.as_str()).exists() {
                std::fs::remove_file(path.as_str())?;
                info_data!("clean file: {}", path)
            }
        } else {
            info_data!("skip clean sink (non-file): {}", self.core.name);
        }
        Ok(())
    }

    /// 返回全名：当注入了组名时为 "<group>/<name>"，否则仅为 `name`
    pub fn full_name(&self) -> String {
        match &self.group_name {
            Some(g) if !g.is_empty() => format!("{}/{}", g, self.core.name),
            _ => self.core.name.clone(),
        }
    }
}

fn default_true() -> bool {
    true
}

// 统一 Core 转换入口：从 SinkInstanceConf 提取 CoreSinkSpec（便于插件/桥接层使用）
impl From<&SinkInstanceConf> for wp_specs::CoreSinkSpec {
    fn from(s: &SinkInstanceConf) -> Self {
        Self {
            name: s.name().clone(),
            kind: s.resolved_kind_str(),
            params: s.resolved_params_table(),
            filter: s.filter().clone(),
            tags: s.tags().clone(),
        }
    }
}

impl SinkInstanceConf {
    pub fn resolved_kind_str(&self) -> String {
        self.core.kind.clone()
    }
    pub fn resolved_params_table(&self) -> ParamMap {
        self.core.params.clone()
    }
}

impl Validate for SinkInstanceConf {
    fn validate(&self) -> OrionConfResult<()> {
        let mut opx = OperationContext::want("validate sink conf")
            .with_auto_log()
            .with_mod_path("ctrl");
        opx.record("name", self.full_name().as_str());
        opx.record("kind", self.core().kind.as_str());
        if self.core.name.trim().is_empty() {
            return ConfIOReason::from_validation("sink.name must not be empty").err_result();
        }
        let kind = self.resolved_kind_str();
        let p = &self.core.params;
        match kind.as_str() {
            "file" | "test_rescue" => {
                let has_base_file = p
                    .get("base")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
                    || p.get("file")
                        .and_then(|v| v.as_str())
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false);
                if !(has_base_file) {
                    return ConfIOReason::from_validation(
                        "file sink requires 'path' or 'base'+'file'",
                    )
                    .err_result();
                }
            }
            _ => {}
        }
        if let Some(path) = &self.core.filter {
            if Path::new(path).exists() {
                if let Ok(content) = std::fs::read_to_string(path)
                    && !content.trim().is_empty()
                {
                    let mut data = content.as_str();
                    WarpConditionParser::exp(&mut data)
                        .owe_conf()
                        .want("invalid filter expression syntax")
                        .with(path.as_str())?;
                }
            } else {
                return ConfIOReason::from_validation("filter file not found")
                    .err_result()
                    .with(path.as_str());
            }
        }
        if let Some(exp) = &self.expect {
            exp.validate().owe_conf().want("sink.expect validate")?;
        }
        Tags::validate(&self.core.tags)
            .owe_conf()
            .want("tags validate")?;

        opx.mark_suc();
        opx.warn("validate suc!");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ForTest;
    use orion_conf::EnvTomlLoad;
    use orion_variate::{EnvDict, ValueType};
    use serde_json::json;

    fn tbl(k: &str, v: &str) -> ParamMap {
        let mut t = ParamMap::new();
        t.insert(k.to_string(), json!(v.to_string()));
        t
    }

    #[test]
    fn construct_syncs_core() {
        let params = tbl("path", "out.dat");
        let s = SinkInstanceConf::new_type(
            "s1".to_string(),
            TextFmt::Json,
            "file".to_string(),
            params.clone(),
            Some("filter.wpl".to_string()),
        );
        assert_eq!(s.name(), &s.core.name);
        assert_eq!(s.resolved_kind_str(), s.core.kind);
        assert_eq!(s.resolved_params_table(), s.core.params);
        assert_eq!(s.filter(), &s.core.filter);
        assert_eq!(s.tags(), &s.core.tags);
    }

    #[test]
    fn deserialize_syncs_core() {
        let raw = r#"
name = "s2"
type = "file"
fmt = "json"
filter = "f.wpl"
tags = ["env:test"]

[params]
path = "p2.dat"
"#;
        let dict = EnvDict::test_default();
        let s: SinkInstanceConf =
            SinkInstanceConf::env_parse_toml(raw, &dict).expect("deserialize");
        assert_eq!(s.name(), &s.core.name);
        assert_eq!(s.resolved_kind_str(), s.core.kind);
        assert_eq!(s.resolved_params_table(), s.core.params);
        assert_eq!(s.filter(), &s.core.filter);
        assert_eq!(s.tags(), &s.core.tags);
    }

    #[test]
    fn setters_keep_core_in_sync() {
        let mut s = SinkInstanceConf::null_new("s3".to_string(), TextFmt::Json, None);
        s.set_kind("kafka".to_string());
        assert_eq!(s.resolved_kind_str(), "kafka".to_string());
        assert_eq!(s.core.kind, "kafka".to_string());

        let mut p = ParamMap::new();
        p.insert("brokers".to_string(), json!("127.0.0.1:9092".to_string()));
        p.insert("topic".to_string(), json!("t".to_string()));
        s.set_params(p.clone());
        assert_eq!(s.resolved_params_table(), p);
        assert_eq!(s.core.params, p);

        s.set_tags(vec!["a:b".to_string(), "c".to_string()]);
        assert_eq!(&s.core.tags, s.tags());

        s.set_filter(Some("ff".to_string()));
        assert_eq!(s.core.filter, s.filter().clone());
    }

    #[test]
    fn env_eval_rewrites_all_fields() {
        let mut params = ParamMap::new();
        params.insert("base".into(), json!("${WORK_ROOT}/out"));
        params.insert("file".into(), json!("${FILE_NAME}"));
        let mut sink = SinkInstanceConf::new_type(
            "${SINK_NAME}".to_string(),
            TextFmt::Json,
            "${SINK_KIND}".to_string(),
            params,
            None,
        );
        sink.set_tags(vec!["${TAG_ONE}".to_string(), "env-${TAG_TWO}".to_string()]);
        sink.connector_id = Some("${CONNECTOR}".to_string());

        let mut dict = EnvDict::new();
        dict.insert("SINK_NAME", ValueType::from("file_sink"));
        dict.insert("SINK_KIND", ValueType::from("file"));
        dict.insert("WORK_ROOT", ValueType::from("/tmp/work"));
        dict.insert("FILE_NAME", ValueType::from("test.dat"));
        dict.insert("TAG_ONE", ValueType::from("alpha"));
        dict.insert("TAG_TWO", ValueType::from("beta"));
        dict.insert("CONNECTOR", ValueType::from("file_raw_sink"));

        let evaluated = sink.env_eval(&dict);
        assert_eq!(evaluated.name(), "file_sink");
        assert_eq!(evaluated.resolved_kind_str(), "file");
        let params = evaluated.resolved_params_table();
        assert_eq!(
            params.get("base").and_then(|v| v.as_str()),
            Some("/tmp/work/out")
        );
        assert_eq!(
            params.get("file").and_then(|v| v.as_str()),
            Some("test.dat")
        );
        assert_eq!(
            evaluated.tags(),
            &vec!["alpha".to_string(), "env-beta".to_string()]
        );
        assert_eq!(evaluated.connector_id.as_deref(), Some("file_raw_sink"));
    }

    // manual_mutation_can_resync_core: 不再支持直接字段突变
}
