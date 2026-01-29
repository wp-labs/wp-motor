use crate::core::parser::SpaceIndex;
use crate::resources::SinkID;
use wp_conf::structure::SinkRouteConf;

use super::normalize_rule_path;
use super::res_manager::ResManager;

impl ResManager {
    pub(crate) fn update_sink_mdl_index(&mut self, conf: &SinkRouteConf) {
        let group_conf = &conf.sink_group;

        for mdl_matcher in group_conf.oml().as_ref() {
            for mdl_name in self.name_mdl_res.keys() {
                info_ctrl!("update_sink_mdl_index, match oml {}", mdl_name.0.as_str());
                if mdl_matcher.matches(mdl_name.0.as_str()) {
                    let find_sink_name = SinkID::from(conf.sink_group.name());
                    if let Some((old_sink_name, matcher)) = self.mdl_sink_map.get_mut(mdl_name) {
                        if mdl_matcher.to_string().len() < matcher.len() {
                            continue;
                        } else {
                            self.sink_mdl_relation
                                .disassociate_mdl(old_sink_name, mdl_name);
                            self.sink_mdl_relation
                                .associate_model(&find_sink_name, mdl_name.clone());
                        }
                    } else {
                        self.mdl_sink_map.insert(
                            mdl_name.clone(),
                            (find_sink_name.clone(), mdl_matcher.to_string()),
                        );
                        self.sink_mdl_relation
                            .associate_model(&find_sink_name, mdl_name.clone());
                    }
                }
            }
        }
    }

    pub(crate) fn update_sink_rule_index(
        &mut self,
        wpl_index: &SpaceIndex,
        conf: &mut SinkRouteConf,
    ) {
        let sink_name = SinkID::from(conf.sink_group.name());
        let name_mdl_res = self.name_mdl_res().clone();
        let group_conf = &conf.sink_group;

        for wpl_matcher in group_conf.rule.as_ref() {
            for rule_key in wpl_index.rule_key() {
                let normalized_rule = normalize_rule_path(rule_key);
                if wpl_matcher.matches(normalized_rule.as_str()) {
                    self.rule_sink_db.update(
                        &sink_name,
                        normalized_rule.as_str(),
                        wpl_matcher.to_string().as_str(),
                    );
                }
            }
        }
        for mdl_matcher in group_conf.oml().as_ref() {
            for rule_key in wpl_index.rule_key() {
                let normalized_rule = normalize_rule_path(rule_key);
                for (name, mdl) in &name_mdl_res {
                    if mdl_matcher.matches(name.0.as_str())
                        && mdl.is_match(normalized_rule.as_str())
                    {
                        debug_ctrl!("{} match oml {}", rule_key, name.0.as_str());
                        self.rule_sink_db.update(
                            &sink_name,
                            normalized_rule.as_str(),
                            mdl_matcher.to_string().as_str(),
                        )
                    }
                }
            }
        }
    }
}
