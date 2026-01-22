use derive_getters::Getters;
use wp_conf::structure::{FixedGroup, FlexGroup, SinkGroupConf};

use crate::facade::test_helpers::SinkTerminal;

#[derive(Clone, Getters)]
pub struct SinkGroupAgent {
    end: SinkTerminal,
    pub conf: SinkGroupConf,
}

impl SinkGroupAgent {
    pub fn new(cnf: SinkGroupConf, end: SinkTerminal) -> Self {
        Self { end, conf: cnf }
    }

    pub fn null() -> Self {
        Self {
            end: SinkTerminal::null(),
            conf: SinkGroupConf::Flexi(FlexGroup::default()),
        }
    }
    pub fn end_mut(&mut self) -> &mut SinkTerminal {
        &mut self.end
    }

    pub fn end_point(&self) -> &SinkTerminal {
        &self.end
    }
    pub fn is_match(&self, rule: &str) -> bool {
        if let SinkGroupConf::Flexi(full) = &self.conf {
            for ws in full.rule.as_ref() {
                if ws.matches(rule) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Getters, Clone)]
pub struct InfraSinkAgent {
    pub default: SinkGroupAgent,
    pub miss: SinkGroupAgent,
    pub residue: SinkGroupAgent,
    pub moni: SinkGroupAgent,
    pub error: SinkGroupAgent,
}

impl InfraSinkAgent {
    pub fn use_null() -> Self {
        Self {
            default: SinkGroupAgent::null(),
            miss: SinkGroupAgent::null(),
            residue: SinkGroupAgent::null(),
            moni: SinkGroupAgent::null(),
            error: SinkGroupAgent::null(),
        }
    }
    pub fn only_default(ap: SinkTerminal) -> Self {
        Self {
            default: SinkGroupAgent::new(SinkGroupConf::Fixed(FixedGroup::default_ins()), ap),
            miss: SinkGroupAgent::null(),
            residue: SinkGroupAgent::null(),
            moni: SinkGroupAgent::null(),
            error: SinkGroupAgent::null(),
        }
    }
}
