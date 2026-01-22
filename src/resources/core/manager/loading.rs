use std::path::Path;

use crate::orchestrator::config::build_sinks::SinkRouteTable;
use crate::resources::ModelName;
use crate::resources::utils::{load_engine_code, load_oml_code};
use crate::sinks::SinkGroupAgent;
use oml::core::ConfADMExt;
use oml::language::{DataModel, ObjModel};
use orion_conf::{ErrorWith, UvsConfFrom};
use orion_error::{ErrorConv, ErrorOwe, OperationContext, ToStructError, UvsLogicFrom};
use orion_variate::EnvDict;
use wp_conf::engine::EngineConfig;
use wp_error::RunReason;
use wp_error::run_error::RunResult;

use crate::core::parser::{SpaceIndex, WplRepository};

use super::res_manager::ResManager;

impl ResManager {
    pub async fn load_all_wpl_code(
        &mut self,
        main_conf: &EngineConfig,
        error_sink: &SinkGroupAgent,
    ) -> RunResult<()> {
        info_ctrl!("load all wpl code beg...");
        let wpl_code = load_engine_code(main_conf).await?;
        let wpl_space =
            WplRepository::from_wpl_tolerant(wpl_code, error_sink.end_point()).owe_rule()?;
        self.wpl_index = Some(SpaceIndex::from(&wpl_space));
        self.wpl_space = Some(wpl_space);
        info_ctrl!("load all wpl code end");
        Ok(())
    }

    pub async fn load_all_ldm(&mut self, oml_root: &str) -> RunResult<()> {
        info_ctrl!("load all oml model");
        let oml_spc = load_oml_code(oml_root).await?;
        let wpl_index = self
            .wpl_index
            .clone()
            .ok_or(RunReason::from_logic("not init  wpl all rule key"))?;
        for (path, _code) in oml_spc.items {
            if std::path::Path::new(path.as_str()).exists() && path.ends_with(".oml") {
                let mdl = ObjModel::load(path.as_str())
                    .err_conv()
                    .want("load oml")
                    .with(path.as_str())?;
                info_data!("oml load success, from {} ", path);
                for w_rule in mdl.rules().as_ref() {
                    for r_path in wpl_index.rule_key().iter() {
                        if w_rule.matches(r_path.as_str()) {
                            self.rule_mdl_relation.update(
                                r_path,
                                mdl.name().as_str(),
                                w_rule.to_string().as_str(),
                            );
                        }
                    }
                }
                let key = ModelName::from(mdl.name().as_str());
                let odm = DataModel::Object(mdl);
                self.name_mdl_res.insert(key, odm);
            } else {
                warn_data!("{} not exists", path)
            }
        }
        Ok(())
    }

    pub(crate) fn load_all_sink(
        &mut self,
        sink_root: &str,
        dict: &EnvDict,
    ) -> RunResult<SinkRouteTable> {
        let mut op = OperationContext::want("load all sink").with_auto_log();
        let wpl_index = self
            .wpl_index
            .clone()
            .ok_or(RunReason::from_logic("not init  wpl all rule key"))?;

        let mut sink_route = SinkRouteTable::default();
        let busin_d = Path::new(sink_root).join("business.d");
        let infra_d = Path::new(sink_root).join("infra.d");
        if busin_d.exists() || infra_d.exists() {
            let confs = wp_conf::sinks::load_business_route_confs_with(sink_root, &Lookup, dict)
                .err_conv()?;
            for mut conf in confs {
                // 现有的方法正确处理 FlexGroup rule 和 oml 字段
                self.update_sink_rule_index(&wpl_index, &mut conf);
                self.update_sink_mdl_index(&conf);
                sink_route.add_route(conf.sink_group);
            }
            self.sink_table = Some(sink_route.clone());
            op.mark_suc();
            Ok(sink_route)
        } else {
            RunReason::from_conf("business and infra sink route not exists").err_result()
        }
    }
}
struct Lookup;
impl wp_conf::sinks::SinkFactoryLookup for Lookup {
    fn get(
        &self,
        kind: &str,
    ) -> Option<std::sync::Arc<dyn wp_connector_api::SinkFactory + 'static>> {
        crate::connectors::registry::get_sink_factory(kind)
    }
}
