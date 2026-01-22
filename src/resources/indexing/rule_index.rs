use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

use derive_getters::Getters;

use crate::resources::SinkID;

use super::{IndexDisplay, RuleIndexSet, RuleKey};

#[derive(Default)]
pub struct SinkRuleMapping(HashMap<SinkID, RuleIndexSet>);
#[derive(Getters, Default)]
pub struct SinkRuleRegistry {
    rule_sink_idx: HashMap<RuleKey, (SinkID, String)>,
    sink_rule_relation: SinkRuleMapping,
}

impl Display for SinkRuleMapping {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.0 {
            writeln!(f, "{:<50} : {} ", k, IndexDisplay::new(v))?;
        }
        Ok(())
    }
}

impl SinkRuleMapping {
    fn update(&mut self, sink_name: &SinkID, rule_key: &str) {
        if let Some(idx) = self.0.get_mut(sink_name) {
            idx.insert(RuleKey::from(rule_key));
        } else {
            let mut idx = RuleIndexSet::default();
            idx.insert(RuleKey::from(rule_key));
            self.0.insert(sink_name.clone(), idx);
        }
    }
    fn remove(&mut self, sink_name: &SinkID, rule_key: &str) {
        if let Some(idx) = self.0.get_mut(sink_name) {
            idx.remove(&RuleKey::from(rule_key));
        }
    }
}

impl Display for SinkRuleRegistry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "--- rule_sink( wpl_rule ,sink_name ) --- ")?;
        for (k, v) in &self.rule_sink_idx {
            writeln!(f, "{:<50} : ({:30},{}) ", k, v.0, v.1)?;
        }
        writeln!(f)?;

        writeln!(f, "--- sink_rule_relation( sink_name, rule_name) --- ")?;
        writeln!(f, "{} ", self.sink_rule_relation)?;
        Ok(())
    }
}
impl SinkRuleRegistry {
    pub fn update(&mut self, sink_name: &SinkID, rule_key: &str, matcher: &str) {
        if let Some((ori_sink_name, ori_matcher)) = self.rule_sink_idx.get(&RuleKey::from(rule_key))
        {
            if matcher.len() > ori_matcher.len() {
                self.sink_rule_relation.remove(ori_sink_name, rule_key);
                self.sink_rule_relation.update(sink_name, rule_key);
                self.rule_sink_idx.remove(&RuleKey::from(rule_key));
                self.rule_sink_idx
                    .insert(RuleKey::from(rule_key), (sink_name.clone(), matcher.into()));
            }
        } else {
            self.rule_sink_idx
                .insert(RuleKey::from(rule_key), (sink_name.clone(), matcher.into()));
            self.sink_rule_relation.update(sink_name, rule_key);
        }
    }
}
