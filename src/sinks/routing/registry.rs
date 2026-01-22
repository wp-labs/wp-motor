use crate::facade::test_helpers::SinkTerminal;
use crate::sinks::{SinkEndpoint, SinkRouteAble};
use orion_overload::append::Appendable2;
use wp_conf::structure::{FixedGroup, SinkGroupConf};

use super::agent::{InfraSinkAgent, SinkGroupAgent};
use super::sink_grp::SyncSinkGroup;

#[derive(Clone)]
pub struct SinkRouteAgent {
    pub(crate) items: Vec<SinkGroupAgent>,
    //pub(crate) infra : InfraSinkAgent,
}

/*
impl SinkRouteAble for SinkRouteAgent {
    fn route(&self, target_rule: &str) -> Option<SinkGroupAgent> {
        rout_impl(&self.items, target_rule)
    }
}

 */

impl SinkRouteAgent {
    pub fn null() -> Self {
        Self { items: Vec::new() }
    }
    pub fn get_sink_agent(&self, sink_name: &str) -> Option<(SinkTerminal, String)> {
        for i in self.items.iter() {
            for sink in i.conf().sinks() {
                if sink.name().eq(sink_name) {
                    // 返回运行期终端与 sink 的 kind 字符串，避免上层直接依赖 SinksEnum
                    return Some((i.end().clone(), sink.resolved_kind_str()));
                }
            }
        }
        None
    }
}

#[derive(Clone)]
pub struct SinkRegistry {
    items: Vec<SyncSinkGroup>,
    pub miss_send: SinkEndpoint,
    pub err_send: SinkEndpoint,
    pub residue_send: SinkEndpoint,
}
impl Default for SinkRegistry {
    fn default() -> Self {
        Self {
            items: vec![],
            miss_send: SinkEndpoint::Null,
            err_send: SinkEndpoint::Null,
            residue_send: SinkEndpoint::Null,
        }
    }
}
impl SinkRegistry {
    pub fn for_test(sink: SinkEndpoint) -> Self {
        Self::new_sample(sink, SinkEndpoint::Null)
    }
    pub fn new_sample(sink: SinkEndpoint, miss_residue: SinkEndpoint) -> Self {
        let item = SinkGroupConf::Fixed(FixedGroup::default_ins());
        let ins = SyncSinkGroup::new(item, sink);
        Self {
            items: vec![ins],
            miss_send: miss_residue.clone(),
            err_send: miss_residue.clone(),
            residue_send: miss_residue.clone(),
        }
    }

    pub fn infra_agent(&self) -> InfraSinkAgent {
        let default = self
            .items
            .first()
            .map(|group| {
                SinkGroupAgent::new(
                    group.conf().clone(),
                    SinkTerminal::from(group.end().clone()),
                )
            })
            .unwrap_or_else(SinkGroupAgent::null);

        InfraSinkAgent {
            default,
            miss: Self::wrap_endpoint(&self.miss_send),
            residue: Self::wrap_endpoint(&self.residue_send),
            // sample registry 暂未提供监控通道，保持空实现
            moni: SinkGroupAgent::null(),
            error: Self::wrap_endpoint(&self.err_send),
        }
    }

    fn wrap_endpoint(ep: &SinkEndpoint) -> SinkGroupAgent {
        match ep {
            SinkEndpoint::Null => SinkGroupAgent::null(),
            _ => SinkGroupAgent::new(
                SinkGroupConf::Fixed(FixedGroup::default_ins()),
                SinkTerminal::from(ep.clone()),
            ),
        }
    }
}

pub trait RouteConfAble {
    fn conf(&self) -> &SinkGroupConf;
    fn clone_ap(&self) -> SinkTerminal;
    fn is_match(&self, rule: &str) -> Option<usize>;
}

fn rout_impl<T>(items: &Vec<T>, target_rule: &str) -> Option<SinkGroupAgent>
where
    T: RouteConfAble,
{
    let mut found: Option<SinkGroupAgent> = None;
    let mut max_match = 0;
    for ins in items {
        if let Some(match_len) = ins.is_match(target_rule)
            && match_len > max_match
        {
            max_match = match_len;
            found = Some(SinkGroupAgent::new(ins.conf().clone(), ins.clone_ap()));
        }
    }
    found
}

impl Appendable2<SinkGroupConf, SinkEndpoint> for SinkRegistry {
    fn append(&mut self, first: SinkGroupConf, second: SinkEndpoint) {
        let ins = SyncSinkGroup::new(first, second);
        self.items.push(ins);
    }
}

impl SinkRouteAble for SinkRegistry {
    fn route(&self, target_rule: &str) -> Option<SinkGroupAgent> {
        rout_impl(&self.items, target_rule)
    }
}

#[cfg(test)]
mod tests {
    use orion_overload::append::Appendable2;
    use wp_conf::structure::{FlexGroup, SinkGroupConf};

    use crate::sinks::{SinkEndpoint, SinkRegistry, SinkRouteAble};

    #[test]
    pub fn test_route() {
        let sinks = SinkRegistry::for_test(SinkEndpoint::Null);
        let route = sinks.route("test");
        assert!(route.is_none());

        let mut sinks = SinkRegistry::for_test(SinkEndpoint::Null);
        sinks.append(
            SinkGroupConf::Flexi(FlexGroup::test_new("default", "*")),
            SinkEndpoint::Null,
        );
        sinks.append(
            SinkGroupConf::Flexi(FlexGroup::test_new("t1", "/china/city/changsha")),
            SinkEndpoint::Null,
        );
        sinks.append(
            SinkGroupConf::Flexi(FlexGroup::test_new("t2", "/china/city/beijing")),
            SinkEndpoint::Null,
        );
        let route = sinks.route("/china/city/changsha").expect("not default");
        assert!(route.conf().name().eq("t1"));
        let route = sinks.route("/china/city/chang").expect("not default");
        assert!(route.conf().name().eq("default"));
        sinks.append(
            SinkGroupConf::Flexi(FlexGroup::test_new("t3", "/china/*")),
            SinkEndpoint::Null,
        );
        let route = sinks.route("/china/city/chang").expect("not default");
        assert_eq!(route.conf().name().as_str(), "t3");
        sinks.append(
            SinkGroupConf::Flexi(FlexGroup::test_new("t4", "/china/city/*")),
            SinkEndpoint::Null,
        );
        let route = sinks.route("/china/city/chang").expect("not default");
        assert_eq!(route.conf().name().as_str(), "t4");
    }
}
