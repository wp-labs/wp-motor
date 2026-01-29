use crate::core::parser::ResourceIndexer;
use crate::resources::core::allocator::ParserResAlloc;
use crate::resources::sinks::sink_resources::SinkResUnit;
use crate::resources::utils::multi_code_ins_parse_units;
use crate::resources::{RuleKey, SinkID};
use crate::sinks::{SinkGroupAgent, SinkRouteAgent};
use orion_error::UvsLogicFrom;
use wp_error::RunReason;
use wp_error::run_error::RunResult;
use wp_log::info_ctrl;
use wp_stat::StatReq;

use super::normalize_rule_path;
use super::res_manager::ResManager;

impl ParserResAlloc for ResManager {
    fn alloc_parse_res(&self, rule_key: &RuleKey) -> RunResult<Vec<SinkGroupAgent>> {
        info_ctrl!("alloc parse res : wpl rule {}", rule_key);
        let mut res = Vec::new();
        let route_agent = self
            .route_agent
            .clone()
            .ok_or(RunReason::from_logic("not init route agent"))?;

        let normalized_key = normalize_rule_path(rule_key.0.as_str());
        let lookup_key = RuleKey::from(normalized_key.as_str());

        if let Some((idx, _)) = self.rule_sink_db.rule_sink_idx().get(&lookup_key) {
            for item in &route_agent.items {
                if *idx == SinkID::from(item.conf().name()) {
                    info_ctrl!(
                        "alloc parse res : rule {}  sink agent: {}",
                        rule_key,
                        item.conf().name()
                    );
                    res.push(item.clone())
                }
            }
        }
        if res.is_empty() {
            //use default sink
            res.push(
                self.infra_agent
                    .clone()
                    .ok_or(RunReason::from_logic("not init infra agent"))?
                    .default()
                    .clone(),
            );
        }
        Ok(res)
    }
}

impl ResManager {
    pub async fn alloc_sink_res(&self, sink_name: &SinkID) -> RunResult<SinkResUnit> {
        info_ctrl!("alloc sink res : sink {} ", sink_name);
        let sink_mdls_opt = self.sink_mdl_relation.get(sink_name);
        let mut sink_res = SinkResUnit::default();
        if let Some(sink_mdls) = sink_mdls_opt {
            for mdl_name in sink_mdls {
                debug_ctrl!("will match model name {} ", mdl_name);
                if let Some(mdl_obj) = self.name_mdl_res.get(mdl_name) {
                    info_ctrl!(
                        "alloc sink res : sink {} oml model: {}",
                        sink_name,
                        mdl_name
                    );
                    sink_res.push_model(mdl_obj.clone())
                }
            }
            // PUBLIC_ADM additions 已废弃：不再注入全局 additions 模型
        }
        Ok(sink_res)
    }
}

impl ResManager {
    pub fn ins_engine_res(
        &mut self,
        agent: SinkRouteAgent,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<()> {
        self.route_agent = Some(agent);
        let wpl_space = self
            .wpl_space
            .clone()
            .ok_or(RunReason::from_logic("not init wpl space "))?;
        let mut idx_keeper = ResourceIndexer::default();
        for wpl_pkg in wpl_space.packages.iter() {
            let mut parsers =
                multi_code_ins_parse_units(self, wpl_pkg, &mut idx_keeper, stat_reqs.clone())?;
            self.parse_units.append(&mut parsers);
        }
        Ok(())
    }
}
