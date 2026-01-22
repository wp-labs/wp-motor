use derive_getters::Getters;
use orion_conf::{
    ToStructError,
    error::{ConfIOReason, OrionConfResult},
};
use orion_error::UvsValidationFrom;
use orion_variate::EnvEvaluable;
use wp_conf_base::ConfParser;
use wp_connector_api::Tags;
use wp_model_core::model::fmt_def::TextFmt;

use crate::types::AnyResult;
use crate::{structure::SinkInstanceConf, utils::env_eval_vec};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use wildmatch::WildMatch;
use wp_specs::WildArray;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Default, Getters)]
pub struct FlexGroup {
    pub name: String,
    #[serde(default)]
    pub(crate) parallel: usize,
    #[serde(default)]
    pub rule: WildArray,
    #[serde(default)]
    pub oml: WildArray,
    /// 组级标签（仅用于路由/注入/统计），不会改变路由匹配
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub filter: Option<String>,
    /// 组级期望（仅公共参数，值域在每个 sink 下的 `sinks.expect` 覆盖）
    #[serde(default)]
    pub expect: Option<GroupExpectSpec>,
    pub sinks: Vec<SinkInstanceConf>,
}

impl EnvEvaluable<FlexGroup> for FlexGroup {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> Self {
        self.name = self.name.env_eval(dict);
        self.tags = env_eval_vec(self.tags, dict);
        self.filter = self.filter.env_eval(dict);
        self.sinks = env_eval_vec(self.sinks, dict);
        self
    }
}
/// 组级期望的公共参数
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Default, Getters)]
#[serde(deny_unknown_fields)]
pub struct GroupExpectSpec {
    /// 分母口径：统一应用于本组全部 sink
    #[serde(default)]
    pub basis: Basis,
    /// 在线窗口字符串（例如 "5m"/"1h"）；离线校验可忽略
    #[serde(default)]
    pub window: Option<String>,
    /// 最小样本，低于则跳过判定
    #[serde(default)]
    pub min_samples: Option<usize>,
    /// 违反时的处理模式
    #[serde(default)]
    pub mode: ExpectMode,
    /// 若给多个 sink 配置 ratio，可对其和做容差检查（可选）
    #[serde(default)]
    pub sum_tol: Option<f64>,
    /// 未配置期望的其余 sink 的总占比上限（可选）
    #[serde(default)]
    pub others_max: Option<f64>,
}

impl GroupExpectSpec {
    /// 轻量合法性检查（范围约束）
    pub fn validate(&self) -> AnyResult<()> {
        if let Some(x) = self.sum_tol
            && !(0.0..=1.0).contains(&x)
        {
            bail!("sum_tol must be in [0,1], got {}", x);
        }
        if let Some(x) = self.others_max
            && !(0.0..=1000.0).contains(&x)
        {
            bail!("others_max must be in [0,1000], got {}", x);
        }
        Ok(())
    }
}

/// 统一分母口径
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExpectMode {
    #[default]
    Warn,
    Error,
    Panic,
}

/// 分母口径：组内输入、全局输入或特定模型输入
#[derive(Debug, Serialize, PartialEq, Clone, Default)]
#[serde(untagged)]
pub enum Basis {
    /// "group_input"
    #[default]
    GroupInput,
    /// "total_input"
    TotalInput,
    /// "mdl:<name>"
    Model { mdl: String },
}

// 自定义字符串反序列化（支持 mdl:<name> 简写）
impl<'de> serde::de::Deserialize<'de> for Basis {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Basis;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a string basis: group_input | total_input | mdl:<name>")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let s = v.trim();
                if s.eq_ignore_ascii_case("group_input") {
                    return Ok(Basis::GroupInput);
                }
                if s.eq_ignore_ascii_case("total_input") {
                    return Ok(Basis::TotalInput);
                }
                if let Some(rest) = s.strip_prefix("mdl:") {
                    let name = rest.trim();
                    if name.is_empty() {
                        return Err(E::custom("mdl:<name> requires non-empty name"));
                    }
                    return Ok(Basis::Model {
                        mdl: name.to_string(),
                    });
                }
                Err(E::custom(format!("invalid basis: {}", s)))
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub enum SinkGroupConf {
    #[serde(rename = "flexi")]
    Flexi(FlexGroup),
    #[serde(rename = "fixed")]
    Fixed(FixedGroup),
}

impl SinkGroupConf {
    pub fn sinks(&self) -> &Vec<SinkInstanceConf> {
        match self {
            SinkGroupConf::Flexi(x) => x.sinks(),
            SinkGroupConf::Fixed(x) => x.sinks(),
        }
    }
    pub fn name(&self) -> &String {
        match self {
            SinkGroupConf::Flexi(x) => x.name(),
            SinkGroupConf::Fixed(x) => x.name(),
        }
    }
    pub fn append(&mut self, conf: SinkInstanceConf) {
        match self {
            SinkGroupConf::Flexi(x) => x.append(conf),
            SinkGroupConf::Fixed(x) => x.append(conf),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Default, Getters)]
pub struct FixedGroup {
    pub name: String,
    #[serde(default)]
    pub expect: Option<GroupExpectSpec>,
    pub sinks: Vec<SinkInstanceConf>,
    /// 并行度（用于 infra Fixed 组），默认 1；最大 10
    #[serde(default)]
    pub parallel: usize,
}
impl EnvEvaluable<FixedGroup> for FixedGroup {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> FixedGroup {
        for sink in self.sinks.iter_mut() {
            *sink = sink.clone().env_eval(dict)
        }
        self
    }
}

impl FixedGroup {
    pub fn append(&mut self, conf: SinkInstanceConf) {
        self.sinks.push(conf)
    }
    pub fn parallel_cnt(&self) -> usize {
        match self.parallel {
            0 => 1,
            1..=10 => self.parallel,
            _ => 10,
        }
    }
}

impl FlexGroup {
    /// 设置并行度（用于 V2 配置装载时注入）。
    /// 注意：validate 会约束最大值（>10 即报错）；运行时 `parallel_cnt()` 也会做上限裁剪。
    pub fn set_parallel(&mut self, p: usize) {
        self.parallel = p;
    }
    pub fn test_new(name: &str, rule: &str) -> Self {
        Self {
            name: name.to_string(),
            parallel: 1,
            oml: WildArray::default(),
            tags: Vec::new(),
            filter: None,
            rule: WildArray::new(rule),
            expect: None,
            sinks: vec![SinkInstanceConf::null_new(
                "test_sink".to_string(),
                TextFmt::Raw,
                None,
            )],
        }
    }

    pub fn build_conf(name: &str, sinks: Vec<SinkInstanceConf>) -> FlexGroup {
        FlexGroup {
            name: name.to_string(),
            parallel: 1,
            oml: WildArray::default(),
            tags: Vec::new(),
            filter: None,
            rule: WildArray::default(),
            expect: None,
            sinks,
        }
    }

    pub fn parallel_cnt(&self) -> usize {
        match self.parallel {
            0 => 1,
            1..=10 => self.parallel,
            _ => 10,
        }
    }
}

impl crate::structure::Validate for FlexGroup {
    fn validate(&self) -> OrionConfResult<()> {
        if self.name.trim().is_empty() {
            return ConfIOReason::from_validation("group.name must not be empty").err_result();
        }
        if self.parallel > 10 {
            return ConfIOReason::from_validation("group.parallel must be <= 10").err_result();
        }
        // tags 校验：统一使用 wp_model_core::tags::validate_tags
        if let Err(e) = Tags::validate(&self.tags) {
            return ConfIOReason::from_validation(e).err_result();
        }
        if let Some(g) = &self.expect
            && let Err(e) = g.validate()
        {
            return ConfIOReason::from_validation(e.to_string()).err_result();
        }
        if self.sinks.is_empty() {
            return ConfIOReason::from_validation("group.sinks must not be empty").err_result();
        }
        Ok(())
    }
}

impl crate::structure::Validate for FixedGroup {
    fn validate(&self) -> OrionConfResult<()> {
        if self.name.trim().is_empty() {
            return ConfIOReason::from_validation("group.name must not be empty").err_result();
        }
        if self.parallel > 10 {
            return ConfIOReason::from_validation("group.parallel must be <= 10").err_result();
        }
        if let Some(g) = &self.expect
            && let Err(e) = g.validate()
        {
            return ConfIOReason::from_validation(e.to_string()).err_result();
        }
        if self.sinks.is_empty() {
            return ConfIOReason::from_validation("group.sinks must not be empty").err_result();
        }
        Ok(())
    }
}

impl crate::structure::Validate for SinkGroupConf {
    fn validate(&self) -> OrionConfResult<()> {
        match self {
            SinkGroupConf::Flexi(x) => {
                if let Err(e) = x.validate() {
                    return ConfIOReason::from_validation(format!("flexi group validate: {}", e))
                        .err_result();
                }
            }
            SinkGroupConf::Fixed(x) => {
                if let Err(e) = x.validate() {
                    return ConfIOReason::from_validation(format!("fixed group validate: {}", e))
                        .err_result();
                }
            }
        }
        Ok(())
    }
}

impl FlexGroup {
    pub fn new2(name: &str, adm: Vec<&str>, filter: Option<&str>) -> Self {
        // let rule = extend_matches(rule_str);
        let adm_vec = extend_matches(adm);
        Self {
            name: name.to_string(),
            parallel: 1,
            oml: adm_vec,
            tags: Vec::new(),
            filter: filter.map(|x| x.to_string()),
            rule: WildArray::default(),
            expect: None,
            sinks: vec![],
        }
    }

    pub fn new<S: Into<String>>(
        name: S,
        adm: Vec<S>,
        filter: Option<S>,
        rule_str: Vec<S>,
        sink_conf: SinkInstanceConf,
    ) -> Self {
        let rule_matches = extend_matches(rule_str);
        let adm_matches = extend_matches(adm);
        Self {
            name: name.into(),
            parallel: 1,
            oml: adm_matches,
            tags: Vec::new(),
            filter: filter.map(|x| x.into()),
            rule: rule_matches,
            expect: None,
            sinks: vec![sink_conf],
        }
    }
    pub fn append(&mut self, conf: SinkInstanceConf) {
        self.sinks.push(conf)
    }
}

pub fn extend_matches<S: Into<String>>(rule: Vec<S>) -> WildArray {
    let mut out = Vec::new();
    for item in rule {
        let x: String = item.into();
        out.push(WildMatch::new(&x));
    }
    WildArray(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_variate::{EnvDict, ValueType};
    use serde_json::json;
    use wp_connector_api::ParamMap;

    #[test]
    fn flex_group_env_eval_updates_members_and_sinks() {
        let mut params = ParamMap::new();
        params.insert("base".into(), json!("${WORK_ROOT}/out"));
        params.insert("file".into(), json!("${FILE}"));
        let sink = SinkInstanceConf::new_type(
            "${SINK_NAME}".to_string(),
            TextFmt::Json,
            "${SINK_KIND}".to_string(),
            params,
            Some("${SINK_FILTER}".to_string()),
        );
        let flex = FlexGroup {
            name: "${GROUP_NAME}".to_string(),
            parallel: 2,
            rule: WildArray::default(),
            oml: WildArray::default(),
            tags: vec!["env-${TAG}".to_string()],
            filter: Some("${GROUP_FILTER}".to_string()),
            expect: None,
            sinks: vec![sink],
        };

        let mut dict = EnvDict::new();
        dict.insert("GROUP_NAME", ValueType::from("alpha"));
        dict.insert("TAG", ValueType::from("prod"));
        dict.insert("GROUP_FILTER", ValueType::from("grp-filter"));
        dict.insert("SINK_NAME", ValueType::from("sink-file"));
        dict.insert("SINK_KIND", ValueType::from("file"));
        dict.insert("SINK_FILTER", ValueType::from("sink-filter"));
        dict.insert("WORK_ROOT", ValueType::from("/tmp/work"));
        dict.insert("FILE", ValueType::from("data.log"));

        let evaluated = flex.env_eval(&dict);
        assert_eq!(evaluated.name(), "alpha");
        assert_eq!(evaluated.tags(), &vec!["env-prod".to_string()]);
        assert_eq!(evaluated.filter(), &Some("grp-filter".to_string()));
        let sink = evaluated.sinks().first().expect("one sink");
        assert_eq!(sink.name(), "sink-file");
        assert_eq!(sink.resolved_kind_str(), "file");
        assert_eq!(sink.filter(), &Some("sink-filter".to_string()));
        let params = sink.resolved_params_table();
        assert_eq!(
            params.get("base").and_then(|v| v.as_str()),
            Some("/tmp/work/out")
        );
        assert_eq!(
            params.get("file").and_then(|v| v.as_str()),
            Some("data.log")
        );
    }
}
