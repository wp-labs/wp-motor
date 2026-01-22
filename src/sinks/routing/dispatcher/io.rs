use std::sync::Arc;

use super::SinkDispatcher;
use crate::sinks::{ASinkSender, ProcMeta, SinkDataEnum};
use crate::stat::MonSend;
use wp_connector_api::SinkResult;
use wp_model_core::model::DataRecord;
use wpl::PkgID;

impl SinkDispatcher {
    // 事件驱动：处理一条已到达的数据（无需从通道再拉取）
    pub(crate) async fn group_sink_one(
        &mut self,
        pkg_id: PkgID,
        data: SinkDataEnum,
        bad_s: &ASinkSender,
        mon: Option<&MonSend>,
    ) -> SinkResult<usize> {
        match data {
            SinkDataEnum::Rec(rule, fds) => {
                self.dispatch_one_per_name_tdc(pkg_id, (rule, fds), Some(bad_s), mon)
                    .await?;
                return Ok(1);
            }
            SinkDataEnum::Raw(body) => {
                self.dispatch_one_per_name_raw(pkg_id, body, Some(bad_s), mon)
                    .await?;
                return Ok(1);
            }
            _ => {}
        }
        Ok(0)
    }
    #[allow(dead_code)]
    pub(crate) async fn group_sink_direct(
        &mut self,
        bad_s: &ASinkSender,
        mon: Option<&MonSend>,
    ) -> SinkResult<usize> {
        let package_opt = self.dat_r.try_recv();
        if let Ok(package) = package_opt {
            if package.is_empty() {
                return Ok(0);
            }
            let unit = &package[0];
            let pkg_id = *unit.id();
            // Now SinkRecUnit contains the meta and data directly
            match unit.meta() {
                // 为与常规路由组行为对齐：并行>1 时，不再对同名 sink 广播，而是按 pkg_id 在同名副本间一致性分配，仅投递一次
                ProcMeta::Rule(rule) => {
                    self.dispatch_one_per_name_tdc(
                        pkg_id,
                        (ProcMeta::Rule(rule.clone()), unit.data().clone()),
                        Some(bad_s),
                        mon,
                    )
                    .await?;
                    debug_data!(
                        "pkg_id: {}, sink group {} hash-route tdc",
                        pkg_id,
                        self.conf.name()
                    );
                    return Ok(1);
                }
                ProcMeta::Null => {
                    // For Null meta, we can't route, just skip
                    return Ok(0);
                }
            }
        }
        Ok(0)
    }

    pub async fn sink_tdc_direct(
        &mut self,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
        pkg: (ProcMeta, Arc<DataRecord>),
    ) -> SinkResult<()> {
        for sink_rt in self.sinks.iter_mut() {
            if sink_rt.is_ready() {
                sink_rt
                    .send_to_sink(SinkDataEnum::Rec(pkg.0.clone(), pkg.1.clone()), bad_s, mon)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn sink_raw(
        &mut self,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
        dat: String,
    ) -> SinkResult<()> {
        // 快路径：仅一个就绪副本时，避免不必要的 clone
        let ready_cnt = self.sinks.iter().filter(|rt| rt.is_ready()).count();
        if ready_cnt == 0 {
            return Ok(());
        }
        if ready_cnt == 1 {
            for sink_rt in self.sinks.iter_mut() {
                if sink_rt.is_ready() {
                    sink_rt
                        .send_to_sink(SinkDataEnum::from(dat), bad_s, mon)
                        .await?;
                    break;
                }
            }
            return Ok(());
        }
        // 多副本时按副本广播（保留原语义）
        for sink_rt in self.sinks.iter_mut() {
            if sink_rt.is_ready() {
                sink_rt
                    .send_to_sink(SinkDataEnum::from(dat.clone()), bad_s, mon)
                    .await?;
            }
        }
        Ok(())
    }

    // ============ hash-route helpers (one replica per sink name) ============
    async fn dispatch_one_per_name_tdc(
        &mut self,
        pkg_id: PkgID,
        pkg: (ProcMeta, Arc<DataRecord>),
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        use std::collections::HashMap;
        // 先统计每个 sink 名称下就绪副本数（按 String 键，避免借用冲突）
        let mut totals: HashMap<String, usize> = HashMap::new();
        for rt in self.sinks.iter() {
            if rt.is_ready() {
                *totals.entry(rt.name.clone()).or_default() += 1;
            }
        }
        if totals.is_empty() {
            return Ok(());
        }
        // 二次遍历，按一致性哈希仅向命中的副本投递
        let mut ordinals: HashMap<String, usize> = HashMap::new();
        for rt in self.sinks.iter_mut() {
            if !rt.is_ready() {
                continue;
            }
            let name = rt.name.clone();
            let total = *totals.get(name.as_str()).unwrap_or(&1);
            let idx = (pkg_id as usize) % total;
            let ord = ordinals.entry(name.clone()).or_default();
            let this = *ord;
            *ord += 1;
            if this == idx {
                rt.send_to_sink(SinkDataEnum::Rec(pkg.0.clone(), pkg.1.clone()), bad_s, mon)
                    .await?;
            }
        }
        Ok(())
    }

    async fn dispatch_one_per_name_raw(
        &mut self,
        pkg_id: PkgID,
        body: String,
        bad_s: Option<&ASinkSender>,
        mon: Option<&MonSend>,
    ) -> SinkResult<()> {
        use std::collections::HashMap;
        let mut totals: HashMap<String, usize> = HashMap::new();
        for rt in self.sinks.iter() {
            if rt.is_ready() {
                *totals.entry(rt.name.clone()).or_default() += 1;
            }
        }
        if totals.is_empty() {
            return Ok(());
        }
        let mut ordinals: HashMap<String, usize> = HashMap::new();
        for rt in self.sinks.iter_mut() {
            if !rt.is_ready() {
                continue;
            }
            let name = rt.name.clone();
            let total = *totals.get(name.as_str()).unwrap_or(&1);
            let idx = (pkg_id as usize) % total;
            let ord = ordinals.entry(name.clone()).or_default();
            let this = *ord;
            *ord += 1;
            if this == idx {
                rt.send_to_sink(SinkDataEnum::from(body.clone()), bad_s, mon)
                    .await?;
            }
        }
        Ok(())
    }
}
