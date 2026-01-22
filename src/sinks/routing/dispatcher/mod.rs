use super::agent::InfraSinkAgent;
use std::collections::HashMap;
use wp_conf::limits::sink_channel_cap;

use crate::resources::SinkResUnit;
use crate::sinks::SinkRuntime;
use crate::sinks::{ASinkSender, SinkDatYReceiver, SinkDatYSender, SinkPackage, SinkRecUnit};
use crate::stat::MonSend;
use derive_getters::Getters;
use orion_overload::append::Appendable;
use wp_conf::structure::SinkGroupConf;
use wp_connector_api::SinkResult;
use wp_data_model::cache::FieldQueryCache;

// split internal helpers

mod io; // 直发/原始数据下发
mod oml; // OML/条件路由
#[cfg(any(test, feature = "perf-ci"))]
pub mod perf; // 性能基准工具
mod recovery; // 故障恢复与收尾
type GroupedRecords = HashMap<String, Vec<SinkRecUnit>>;

struct SinkRecUnitPool {
    inner: Vec<Vec<SinkRecUnit>>,
}

impl SinkRecUnitPool {
    const POOL_MAX: usize = 128;
    const UNIT_MAX_CAP: usize = 4096;

    fn new() -> Self {
        Self { inner: Vec::new() }
    }

    fn take(&mut self) -> Vec<SinkRecUnit> {
        self.inner.pop().unwrap_or_else(|| Vec::with_capacity(64))
    }

    fn recycle(&mut self, mut vec: Vec<SinkRecUnit>) {
        if vec.capacity() > Self::UNIT_MAX_CAP {
            vec.shrink_to(Self::UNIT_MAX_CAP);
        }
        vec.clear();
        if self.inner.len() < Self::POOL_MAX {
            self.inner.push(vec);
        }
    }
}

#[derive(Getters)]
pub struct SinkDispatcher {
    conf: SinkGroupConf,
    sinks: Vec<SinkRuntime>,
    dat_s: SinkDatYSender,
    dat_r: SinkDatYReceiver,
    res: SinkResUnit,
    unit_pool: SinkRecUnitPool,
}

impl SinkDispatcher {
    pub fn new(conf: SinkGroupConf, res: SinkResUnit) -> Self {
        // 改用 tokio::mpsc 事件化通道，便于与 runtime 协作
        let (dat_s, dat_r) = tokio::sync::mpsc::channel(sink_channel_cap());
        Self {
            conf,
            sinks: Vec::new(),
            dat_s,
            dat_r,
            res,
            unit_pool: SinkRecUnitPool::new(),
        }
    }
    pub fn get_dat_r_mut(&mut self) -> &mut SinkDatYReceiver {
        &mut self.dat_r
    }
    pub fn close_channel(&mut self) {
        self.dat_r.close();
    }
    pub fn get_name(&self) -> &str {
        self.conf.name().as_str()
    }
    pub fn freeze_all(&mut self) {
        info_data!("{} sink group freeze all", self.conf.name());
        for sink_rt in self.sinks.iter_mut() {
            sink_rt.freeze();
        }
    }
    pub fn active_all(&mut self) {
        for sink_rt in self.sinks.iter_mut() {
            sink_rt.ready();
        }
    }

    pub fn active_one(&mut self, name: &str) {
        for sink_rt in self.sinks.iter_mut() {
            if sink_rt.name == name {
                info_data!("{} sink group active one", self.conf.name());
                sink_rt.ready();
                break;
            }
        }
    }

    /// 批量处理数据包（支持批量优化）
    pub(crate) async fn group_sink_package(
        &mut self,
        package: SinkPackage,
        infra: &InfraSinkAgent,
        bad_s: &ASinkSender,
        mon: Option<&MonSend>,
        cache: &mut FieldQueryCache,
    ) -> SinkResult<usize> {
        let mut processed_count = 0;

        // 先按规则分组，同一规则共享一次 OML 批处理
        let mut records_by_rule: GroupedRecords = HashMap::new();
        for unit in package.into_iter() {
            let key = unit.meta().abstract_info();
            records_by_rule
                .entry(key)
                .or_insert_with(|| self.unit_pool.take())
                .push(unit);
            processed_count += 1;
        }

        // 批量处理同一规则下的记录
        for (_rule_str, units) in records_by_rule {
            if units.is_empty() {
                continue;
            }
            let Some(meta) = units.first().map(|unit| unit.meta().clone()) else {
                continue;
            };
            let mut per_sink_units = self.oml_proc_batch(units, infra, cache, &meta)?;
            for (idx, sink_rt) in self.sinks.iter_mut().enumerate() {
                let payload = {
                    if !sink_rt.is_ready() {
                        let unused = std::mem::take(&mut per_sink_units[idx]);
                        self.unit_pool.recycle(unused);
                        None
                    } else {
                        let units = std::mem::take(&mut per_sink_units[idx]);
                        if units.is_empty() {
                            self.unit_pool.recycle(units);
                            None
                        } else {
                            let pkg = SinkPackage::from_units(units.into_iter());
                            let name_snapshot = sink_rt.name.clone();
                            sink_rt.send_package_to_sink(&pkg, Some(bad_s), mon).await?;
                            let vec_back = pkg.into_inner();
                            Some((name_snapshot, vec_back))
                        }
                    }
                };
                if let Some((name, vec_back)) = payload {
                    self.unit_pool.recycle(vec_back);
                    debug_data!("sink {} send batch rec suc!", name);
                }
            }
            for leftover in per_sink_units.into_iter() {
                self.unit_pool.recycle(leftover);
            }
        }

        Ok(processed_count)
    }

    // heavy OML pipeline helpers are moved to dispatcher::oml

    // 直发与原始数据下发在 dispatcher::io

    // 恢复与收尾在 dispatcher::recovery

    pub fn get_data_sender(&self) -> SinkDatYSender {
        self.dat_s.clone()
    }
}

impl Appendable<SinkRuntime> for SinkDispatcher {
    fn append(&mut self, first: SinkRuntime) {
        self.sinks.push(first);
    }
}

// tests moved into a dedicated file for readability
#[cfg(test)]
mod tests;
