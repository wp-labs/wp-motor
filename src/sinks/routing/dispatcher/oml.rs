use std::sync::Arc;

use super::SinkDispatcher;
// trait for .send_to_sink on SinkTerminal
use crate::sinks::InfraSinkAgent;
use crate::sinks::prelude::*;
// PkgID, info/debug macros
use crate::sinks::ProcMeta;
use crate::sinks::SinkRecUnit;
use crate::sinks::SinkRuntime;
use oml::core::DataTransformer;
use oml::language::{DataModel, ObjModel};
// std::collections used to be required for HashMap-based fanout; kept minimal now
use wp_connector_api::SinkResult;
use wp_data_model::cache::FieldQueryCache;
use wp_data_model::conditions::evaluate_expression;
use wp_model_core::model::{DataField, DataRecord};

// 说明：原实现通过构建 HashMap<name, DataRecord> 聚合每个 sink 的待投递数据，
// 这会导致对同一条记录进行 N 次 clone（每命中一个 sink 就 clone 一次），
// 还伴随一次 HashMap 的构建/查找开销。为降低开销，改为：
// 1) 仅执行一次 OML 变换，得到“基准记录” base；
// 2) 根据 cond 计算命中的 sink 列表；
// 3) 对除最后一个命中的 sink 之外的目标执行 `base.clone()`，最后一个直接 move `base`，
//    从而把 clone 次数从 N 降到 N-1；
// 4) 取消中间 HashMap 构造，直接生成目标下发列表。
//
#[cfg_attr(not(test), allow(dead_code))]
enum OmlOutcome {
    Success(DataRecord),
    Failure(DataRecord),
}

struct TransformedRecUnit {
    pkg_id: PkgID,
    meta: ProcMeta,
    record: DataRecord,
}

impl TransformedRecUnit {
    fn new(pkg_id: PkgID, meta: ProcMeta, record: DataRecord) -> Self {
        Self {
            pkg_id,
            meta,
            record,
        }
    }

    fn into_parts(self) -> (PkgID, ProcMeta, DataRecord) {
        (self.pkg_id, self.meta, self.record)
    }
}

impl SinkDispatcher {
    fn has_conditions(&self) -> bool {
        self.sinks.iter().any(|sink| sink.get_cond().is_some())
    }
    // OML model selection by rule
    fn get_match_oml(&self, rule: &ProcMeta) -> Option<&ObjModel> {
        for mdl in self.res.aggregate_mdl() {
            if let (DataModel::Object(om), ProcMeta::Rule(r)) = (mdl, rule) {
                for w_rule in om.rules().as_ref() {
                    if w_rule.matches(r.as_str()) {
                        return Some(om);
                    }
                }
            }
        }
        None
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn run_oml_pipeline(
        &self,
        rule: &ProcMeta,
        input: DataRecord,
        cache: &mut FieldQueryCache,
    ) -> SinkResult<OmlOutcome> {
        let Some(om_ins) = self.get_match_oml(rule) else {
            return Ok(OmlOutcome::Success(input));
        };

        let original_len = input.items.len();
        let output = om_ins.transform(input, cache);
        if output.items.is_empty() {
            let mut failed = output.clone();
            Self::annotate_err(
                &mut failed,
                "oml_transform_empty",
                rule,
                self.conf.name(),
                om_ins.name(),
                original_len,
                output.items.len(),
            );
            return Ok(OmlOutcome::Failure(failed));
        }
        Ok(OmlOutcome::Success(output))
    }

    fn run_oml_pipeline_vec(
        &self,
        wpl_meta: &ProcMeta,
        input: Vec<SinkRecUnit>,
        cache: &mut FieldQueryCache,
    ) -> SinkResult<(Vec<TransformedRecUnit>, Vec<SinkRecUnit>)> {
        let Some(om_ins) = self.get_match_oml(wpl_meta) else {
            let passthrough = input
                .into_iter()
                .map(|unit| {
                    let (pkg_id, meta, record_arc) = unit.into_parts();
                    let record =
                        Arc::try_unwrap(record_arc).unwrap_or_else(|arc| arc.as_ref().clone());
                    TransformedRecUnit::new(pkg_id, meta, record)
                })
                .collect();
            return Ok((passthrough, Vec::new()));
        };

        let mut successes = Vec::with_capacity(input.len());
        let mut failures = Vec::new();
        for unit in input {
            let (pkg_id, meta, record_arc) = unit.into_parts();
            let original_len = record_arc.items.len();
            let output = om_ins.transform_ref(record_arc.as_ref(), cache);
            if output.items.is_empty() {
                let mut failed = output.clone();
                Self::annotate_err(
                    &mut failed,
                    "oml_transform_empty",
                    wpl_meta,
                    self.conf.name(),
                    om_ins.name(),
                    original_len,
                    output.items.len(),
                );
                failures.push(SinkRecUnit::with_record(
                    pkg_id,
                    meta.clone(),
                    Arc::new(failed),
                ));
            } else {
                successes.push(TransformedRecUnit::new(pkg_id, meta, output));
            }
        }
        Ok((successes, failures))
    }

    // 为错误记录添加标准诊断字段
    fn annotate_err(
        rec: &mut DataRecord,
        kind: &str,
        rule: &ProcMeta,
        group: &str,
        mdl_name: &str,
        src_cnt: usize,
        out_cnt: usize,
    ) {
        use wp_model_core::model::DataField;
        rec.append(DataField::from_chars("__err_kind", kind));
        if let ProcMeta::Rule(r) = rule {
            rec.append(DataField::from_chars("__wpl_rule".to_string(), r.clone()));
        }
        rec.append(DataField::from_chars("__sink_group", group));
        if !mdl_name.is_empty() {
            rec.append(DataField::from_chars("__oml_model", mdl_name));
        }
        rec.append(DataField::from_digit("__src_field_count", src_cnt as i64));
        rec.append(DataField::from_digit("__out_field_count", out_cnt as i64));
        // 诊断详情（如已启用）
        if let Some(diag) = oml::core::diagnostics::take_summary() {
            rec.append(DataField::from_chars("__diag".to_string(), diag));
        }
        // 建议
        let hint = match kind {
            "oml_transform_empty" => {
                "OML 输出为空；请检查模型 rules 是否匹配当前 WPL 路径，以及 read/take 字段名是否存在"
            }
            "oml_transform_nochange" => {
                "OML 输出与输入一致；可能规则未生效或字段映射缺失，请核对字段与类型转换"
            }
            _ => "OML 转换失败",
        };
        rec.append(DataField::from_chars("__hint", hint));
    }

    // 核心 OML 流水线：返回每个 sink 的待发送记录
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn oml_proc(
        &mut self,
        pkg_id: PkgID,
        infra: &InfraSinkAgent,
        cache: &mut FieldQueryCache,
        rule: &ProcMeta,
        fds: Arc<DataRecord>,
    ) -> SinkResult<Vec<(&mut SinkRuntime, Arc<DataRecord>)>> {
        let has_oml = self.get_match_oml(rule).is_some();
        if !has_oml && !self.has_conditions() {
            return Ok(self.emit_without_transform(fds));
        }
        self.route_with_transform(pkg_id, infra, cache, rule, fds)
    }

    pub(super) fn oml_proc_batch(
        &mut self,
        batch: Vec<SinkRecUnit>,
        infra: &InfraSinkAgent,
        cache: &mut FieldQueryCache,
        rule: &ProcMeta,
    ) -> SinkResult<Vec<Vec<SinkRecUnit>>> {
        if batch.is_empty() {
            return Ok(vec![Vec::new(); self.sinks.len()]);
        }
        let has_oml = self.get_match_oml(rule).is_some();
        if !has_oml && !self.has_conditions() {
            return Ok(self.emit_without_transform_batch(batch));
        }

        let (successes, failures) = self.run_oml_pipeline_vec(rule, batch, cache)?;
        for bad in failures {
            let (pkg_id, _, bad_arc) = bad.into_parts();
            let record = Arc::try_unwrap(bad_arc).unwrap_or_else(|arc| arc.as_ref().clone());
            self.emit_oml_failure(pkg_id, infra, rule, record)?;
        }
        Ok(self.fanout_transformed_batch(successes))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn emit_without_transform(
        &mut self,
        record: Arc<DataRecord>,
    ) -> Vec<(&mut SinkRuntime, Arc<DataRecord>)> {
        let mut outputs = Vec::with_capacity(self.sinks.len());
        for sink in self.sinks.iter_mut() {
            if sink.pre_tags().is_empty() {
                outputs.push((sink, Arc::clone(&record)));
            } else {
                let mut enriched = (*record).clone();
                Self::append_pre_tags(&mut enriched, sink.pre_tags());
                outputs.push((sink, Arc::new(enriched)));
            }
        }
        outputs
    }

    fn emit_without_transform_batch(&mut self, entries: Vec<SinkRecUnit>) -> Vec<Vec<SinkRecUnit>> {
        let mut per_sink: Vec<Vec<SinkRecUnit>> = (0..self.sinks.len())
            .map(|_| self.unit_pool.take())
            .collect();
        for entry in entries {
            let (pkg_id, meta, base_arc) = entry.into_parts();
            for (idx, sink) in self.sinks.iter().enumerate() {
                let rec = if sink.pre_tags().is_empty() {
                    Arc::clone(&base_arc)
                } else {
                    let mut enriched = (*base_arc).clone();
                    Self::append_pre_tags(&mut enriched, sink.pre_tags());
                    Arc::new(enriched)
                };
                per_sink[idx].push(SinkRecUnit::with_record(pkg_id, meta.clone(), rec));
            }
        }
        per_sink
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn route_with_transform(
        &mut self,
        pkg_id: PkgID,
        infra: &InfraSinkAgent,
        cache: &mut FieldQueryCache,
        rule: &ProcMeta,
        fds: Arc<DataRecord>,
    ) -> SinkResult<Vec<(&mut SinkRuntime, Arc<DataRecord>)>> {
        let base = match self.run_oml_pipeline(rule, (*fds).clone(), cache)? {
            OmlOutcome::Success(base) => base,
            OmlOutcome::Failure(bad) => {
                self.emit_oml_failure(pkg_id, infra, rule, bad)?;
                return Ok(Vec::new());
            }
        };
        let matches = self.evaluate_sink_matches(&base);
        let mut remaining = matches.iter().filter(|&&m| m).count();
        if remaining == 0 {
            return Ok(Vec::new());
        }

        let mut base_slot = Some(base);
        let mut outputs = Vec::with_capacity(remaining);
        for (sink_rt, matched) in self.sinks.iter_mut().zip(matches.into_iter()) {
            if !matched {
                continue;
            }
            let mut record = Self::acquire_record_for_target(&mut base_slot, remaining);
            Self::append_pre_tags(&mut record, sink_rt.pre_tags());
            outputs.push((sink_rt, Arc::new(record)));
            remaining -= 1;
        }
        Ok(outputs)
    }

    fn fanout_transformed_batch(
        &mut self,
        entries: Vec<TransformedRecUnit>,
    ) -> Vec<Vec<SinkRecUnit>> {
        let mut per_sink: Vec<Vec<SinkRecUnit>> = (0..self.sinks.len())
            .map(|_| self.unit_pool.take())
            .collect();
        for entry in entries {
            let (pkg_id, meta, record) = entry.into_parts();
            self.push_transformed_record(pkg_id, meta, record, &mut per_sink);
        }
        per_sink
    }

    fn push_transformed_record(
        &self,
        pkg_id: PkgID,
        meta: ProcMeta,
        base: DataRecord,
        per_sink: &mut [Vec<SinkRecUnit>],
    ) {
        if per_sink.is_empty() {
            return;
        }
        let matches = self.evaluate_sink_matches(&base);
        let mut remaining = matches.iter().filter(|&&m| m).count();
        if remaining == 0 {
            return;
        }

        let mut base_slot = Some(base);
        for (idx, matched) in matches.into_iter().enumerate() {
            if !matched {
                continue;
            }
            let mut record = Self::acquire_record_for_target(&mut base_slot, remaining);
            if let Some(sink) = self.sinks.get(idx) {
                Self::append_pre_tags(&mut record, sink.pre_tags());
            }
            let unit = SinkRecUnit::with_record(pkg_id, meta.clone(), Arc::new(record));
            if let Some(slot) = per_sink.get_mut(idx) {
                slot.push(unit);
            }
            remaining -= 1;
        }
    }

    fn evaluate_sink_matches(&self, base: &DataRecord) -> Vec<bool> {
        self.sinks
            .iter()
            .map(|sink| {
                if let Some(cond) = sink.get_cond() {
                    let expected = *sink.conf().filter_expect();
                    evaluate_expression(cond, base) == expected
                } else {
                    true
                }
            })
            .collect()
    }

    fn acquire_record_for_target(
        base_slot: &mut Option<DataRecord>,
        remaining_targets: usize,
    ) -> DataRecord {
        if remaining_targets == 1 {
            base_slot.take().expect("base record already moved")
        } else {
            base_slot.as_ref().expect("base record missing").clone()
        }
    }

    fn append_pre_tags(record: &mut DataRecord, tags: &[DataField]) {
        if tags.is_empty() {
            return;
        }
        for tag in tags.iter().cloned() {
            record.append(tag);
        }
    }

    fn emit_oml_failure(
        &self,
        pkg_id: PkgID,
        infra: &InfraSinkAgent,
        rule: &ProcMeta,
        record: DataRecord,
    ) -> SinkResult<()> {
        warn_data!("pkg_id: {}, oml convert failed", pkg_id);
        infra
            .error()
            .end()
            .send_record(pkg_id, rule.clone(), Arc::new(record))
    }
}
