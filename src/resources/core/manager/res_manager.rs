use derive_getters::Getters;
use orion_variate::EnvDict;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::core::parser::{SpaceIndex, WplPipeline, WplRepository};
use crate::orchestrator::config::build_sinks::SinkRouteTable;
use crate::resources::{ModelName, SinkID};
use crate::resources::{SinkModelIndex, SinkRuleRegistry};
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::sinks::{InfraSinkAgent, SinkRouteAgent};
use orion_error::{ErrorOwe, ToStructError, UvsLogicFrom};
use wp_conf::engine::EngineConfig;
use wp_error::RunReason;
use wp_error::run_error::RunResult;

/// 规则到模型的最佳匹配关系：记录每个 rule_key 匹配到的模型及其匹配表达式长度
#[derive(Default)]
pub struct RuleMdlMapping(pub(crate) HashMap<crate::resources::RuleKey, (ModelName, String)>);

impl Display for RuleMdlMapping {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.0 {
            writeln!(f, "{:<50} : ({:30},{}) ", k, v.0, v.1)?;
        }
        Ok(())
    }
}
impl RuleMdlMapping {
    pub fn update(&mut self, rule_key: &str, mdl_name: &str, matcher: &str) {
        use crate::resources::{ModelName, RuleKey};
        if let Some(x) = self.0.get_mut(&RuleKey::from(rule_key)) {
            if matcher.len() > x.1.len() {
                x.0 = ModelName::from(mdl_name);
                x.1 = matcher.to_string();
            }
        } else {
            self.0.insert(
                RuleKey::from(rule_key),
                (ModelName::from(mdl_name), matcher.to_string()),
            );
        }
    }
}

#[derive(Getters, Default)]
pub struct ResManager {
    pub name_mdl_res: HashMap<ModelName, oml::language::DataModel>,
    pub(crate) mdl_sink_map: HashMap<ModelName, (SinkID, String)>,
    pub(crate) rule_mdl_relation: RuleMdlMapping,
    pub(crate) rule_sink_db: SinkRuleRegistry,
    pub(crate) sink_mdl_relation: SinkModelIndex,
    pub(crate) wpl_space: Option<WplRepository>,
    pub(crate) wpl_index: Option<SpaceIndex>,
    pub(crate) route_agent: Option<SinkRouteAgent>,
    pub(crate) infra_agent: Option<InfraSinkAgent>,
    pub(crate) parse_units: Vec<WplPipeline>,
    pub(crate) sink_table: Option<SinkRouteTable>,
}

impl ResManager {
    pub(crate) fn must_get_sink_table(&self) -> RunResult<&SinkRouteTable> {
        self.sink_table
            .as_ref()
            .ok_or(RunReason::from_logic("not init sink table").to_err())
    }
    pub fn get_parse_units(&self) -> &Vec<WplPipeline> {
        &self.parse_units
    }
}

impl ResManager {
    /// 构建：根据完整的 SourceConfig 列表初始化运行期资源
    pub async fn build(
        main_conf: &EngineConfig,
        infra_sinks: &InfraSinkService,
        dict: &EnvDict,
    ) -> RunResult<Self> {
        let mut res_center = ResManager::default();
        res_center.set_infra_agent(infra_sinks.agent());
        res_center
            .load_all_wpl_code(main_conf, infra_sinks.agent().error())
            .await?;
        res_center.load_all_ldm(main_conf.oml_root()).await?;
        res_center
            .load_all_sink(main_conf.sinks_root(), dict)
            .owe_conf()?;
        Ok(res_center)
    }

    /// 构建：仅根据源 key 列表初始化运行期资源（用于 WPL 索引建立）
    #[deprecated]
    pub async fn build_from_keys(
        main_conf: &EngineConfig,
        infra_sinks: &InfraSinkService,
        dict: &EnvDict,
    ) -> wp_error::run_error::RunResult<Self> {
        Self::build(main_conf, infra_sinks, dict).await
    }
}

impl Display for ResManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "--- name_mdl_res------")?;
        for k in self.name_mdl_res.keys() {
            writeln!(f, "{} ", k)?;
        }
        writeln!(f)?;
        writeln!(f, "--- rule_mdl_res(rule_name, model_name) --- ")?;
        writeln!(f, "{}", self.rule_mdl_relation)?;
        writeln!(f)?;
        writeln!(f, "{}", self.rule_sink_db())?;
        writeln!(f)?;

        writeln!(f, "--- sink_mdl_relation( sink_name, model_name) --- ")?;
        writeln!(f, "{}", self.sink_mdl_relation)?;
        writeln!(f)?;

        Ok(())
    }
}

impl ResManager {
    // PUBLIC_ADM additions 废弃：不再设置全局 additions；保持默认空模型
    pub fn set_infra_agent(&mut self, agent: InfraSinkAgent) {
        self.infra_agent = Some(agent);
    }
}
