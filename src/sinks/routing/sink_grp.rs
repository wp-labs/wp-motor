use derive_getters::Getters;
use wp_conf::structure::SinkGroupConf;

use super::registry::RouteConfAble;

use crate::{facade::test_helpers::SinkTerminal, sinks::SinkEndpoint};
#[derive(Clone, Getters)]
pub struct SyncSinkGroup {
    conf: SinkGroupConf,
    end: SinkEndpoint,
    //aggregate_mdl: Vec<DataModel>,
    //append_mdl: Option<DataModel>,
}
impl SyncSinkGroup {
    pub fn new(conf: SinkGroupConf, end: SinkEndpoint) -> Self {
        Self {
            conf,
            end,
            //aggregate_mdl: aga_mdl,
            //append_mdl: apd_mdl,
        }
    }
}
impl RouteConfAble for SyncSinkGroup {
    fn conf(&self) -> &SinkGroupConf {
        self.conf()
    }

    fn clone_ap(&self) -> SinkTerminal {
        SinkTerminal::from(self.end.clone())
    }

    fn is_match(&self, rule: &str) -> Option<usize> {
        match self.conf() {
            SinkGroupConf::Flexi(conf) => {
                for r_wild in conf.rule.as_ref() {
                    if r_wild.matches(rule) {
                        return Some(r_wild.to_string().len());
                    }
                }
            }
            SinkGroupConf::Fixed(_) => {}
        }
        None
    }
}
