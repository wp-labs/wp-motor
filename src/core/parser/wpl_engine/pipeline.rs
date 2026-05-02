use crate::core::prelude::*;
use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::facade::test_helpers::SinkTerminal;
use crate::sinks::SinkGroupAgent;
use crate::stat::MonSend;
use crate::stat::metric_collect::MetricCollectors;
use orion_error::{UvsReason, compat_prelude::ErrorOweBase};
use std::cmp::Ordering as CmpOrdering;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use wp_model_core::model::{DataField, DataRecord};
use wp_model_core::raw::RawData;
use wp_parse_api::DataResult;
use wp_stat::StatRecorder;
use wp_stat::StatReq;
use wpl::WparseReason;
use wpl::WparseResult;
use wpl::{AnnotationFunc, AnnotationType};
use wpl::{OPTIMIZE_TIMES, WplEvaluator};

#[derive(Getters, Clone)]
pub struct WplPipeline {
    parser: WplEvaluator,
    fun_vec: Vec<AnnotationType>,
    pub hit_cnt: usize,
    pub access_cnt: usize,
    pub index: usize,
    output: Vec<SinkGroupAgent>,
    send_rr: Arc<AtomicUsize>,
    wpl_key: String,
    s_name: String,
    package_name: String,
    rule_name: String,
    // 仅当统计请求显式包含 wp_package_name/wp_rule_name 时开启。
    // 这样可避免默认路径上每条数据都做额外字段构造与拷贝。
    stat_need_pkg_rule: bool,
    stat_ext: MetricCollectors,
}

impl WplPipeline {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        index: usize,
        wpl_key: String,
        package_name: String,
        rule_name: String,
        fun_vec: Vec<AnnotationType>,
        parser: WplEvaluator,
        output: Vec<SinkGroupAgent>,
        stat_reqs: Vec<StatReq>,
    ) -> Self {
        //let s_name = name.split('/').last().unwrap_or(&name);
        let s_name = wpl_key.clone();
        // 按需开启 parse 维度补齐：只有请求这两个维度时才走补齐逻辑。
        let stat_need_pkg_rule = stat_reqs.iter().any(|r| {
            r.collect
                .iter()
                .any(|f| matches!(f.as_str(), "wp_package_name" | "wp_rule_name"))
        });
        let stat_ext = MetricCollectors::new(wpl_key.clone(), stat_reqs);

        Self {
            parser,
            fun_vec,
            index,
            wpl_key,
            output,
            send_rr: Arc::new(AtomicUsize::new(0)),
            hit_cnt: 0,
            access_cnt: 0,
            s_name,
            package_name,
            rule_name,
            stat_need_pkg_rule,
            stat_ext,
        }
    }

    pub fn short_name(&self) -> &str {
        self.s_name.as_str()
    }

    fn next_output_index(&self) -> usize {
        self.send_rr.fetch_add(1, AtomicOrdering::Relaxed) % self.output().len()
    }

    pub fn get_rolled_end(&self) -> &SinkTerminal {
        let idx = self.next_output_index();
        self.output[idx].end_point()
    }
    pub fn proc(&mut self, data: &SourceEvent, oth_suc_len: usize) -> DataResult {
        self.access_cnt += 1;
        match self
            .parser
            .proc(data.event_id, data.payload.clone(), oth_suc_len)
        {
            Ok((mut record, left)) => {
                if self.stat_need_pkg_rule {
                    // begin 阶段使用稳定维度，避免因 record 尚未填充导致分组漂移。
                    let stat_begin = DataRecord::from(vec![
                        DataField::from_chars("wp_package_name", self.package_name.as_str()),
                        DataField::from_chars("wp_rule_name", self.rule_name.as_str()),
                    ]);
                    self.stat_ext
                        .record_begin(self.wpl_key.as_str(), Some(&stat_begin));
                } else {
                    self.stat_ext.record_begin(self.wpl_key.as_str(), None);
                }
                for func in self.fun_vec.iter() {
                    func.proc(data, &mut record)?;
                }
                if self.stat_need_pkg_rule {
                    // end 阶段优先保留 record 中已有字段；缺失时再回填默认值。
                    let mut stat_end = record.clone();
                    if stat_end.field("wp_package_name").is_none() {
                        stat_end.append(DataField::from_chars(
                            "wp_package_name",
                            self.package_name.as_str(),
                        ));
                    }
                    if stat_end.field("wp_rule_name").is_none() {
                        stat_end.append(DataField::from_chars(
                            "wp_rule_name",
                            self.rule_name.as_str(),
                        ));
                    }
                    self.stat_ext
                        .record_end(self.wpl_key.as_str(), Some(&stat_end));
                } else {
                    self.stat_ext
                        .record_end(self.wpl_key.as_str(), Some(&record));
                }
                Ok((record, RawData::from_string(left)))
            }
            Err(e) => Err(e),
        }
    }
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> WparseResult<()> {
        if !self.stat_ext.has_pending_data() {
            if self.stat_need_pkg_rule {
                // 空闲周期也触发维度桶，保证监控面板维度连续可见。
                let rec = DataRecord::from(vec![
                    DataField::from_chars("wp_package_name", self.package_name.as_str()),
                    DataField::from_chars("wp_rule_name", self.rule_name.as_str()),
                ]);
                self.stat_ext.touch_task_record(self.wpl_key.as_str(), &rec);
            } else {
                self.stat_ext.touch_task_unit(self.wpl_key.as_str());
            }
        }
        self.stat_ext
            .send_stat(mon_send)
            .await
            .owe(WparseReason::Uvs(UvsReason::system_error()))?;
        Ok(())
    }
    pub fn stop(&mut self) {
        for out in &mut self.output {
            out.end_mut().stop().expect("stop error");
        }
    }
}

//#[derive(Clone)]

impl Eq for WplPipeline {}

impl PartialEq<Self> for WplPipeline {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl PartialOrd<Self> for WplPipeline {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for WplPipeline {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_hit = OPTIMIZE_TIMES - self.hit_cnt;
        let other_hit = OPTIMIZE_TIMES - other.hit_cnt;
        self_hit.cmp(&other_hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wp_conf::structure::{FixedGroup, SinkGroupConf};

    fn build_pipeline(output_cnt: usize) -> WplPipeline {
        let evaluator = WplEvaluator::from_code("rule demo { ( _ ) }").expect("build wpl");
        let mut output = Vec::with_capacity(output_cnt);
        for _ in 0..output_cnt {
            output.push(SinkGroupAgent::new(
                SinkGroupConf::Fixed(FixedGroup::default_ins()),
                SinkTerminal::null(),
            ));
        }
        WplPipeline::new(
            0,
            "demo/rule".to_string(),
            "demo".to_string(),
            "rule".to_string(),
            Vec::new(),
            evaluator,
            output,
            Vec::new(),
        )
    }

    #[test]
    fn rolled_index_advances_by_one_per_send_and_ignores_hit_cnt() {
        let mut p = build_pipeline(3);
        p.hit_cnt = 100;
        let first = p.next_output_index();

        p.hit_cnt = 1;
        let second = p.next_output_index();
        let third = p.next_output_index();

        assert_eq!((first + 1) % 3, second);
        assert_eq!((second + 1) % 3, third);
    }

    #[test]
    fn rolled_index_is_shared_across_clones() {
        let p = build_pipeline(2);
        let p2 = p.clone();

        let first = p.next_output_index();
        let second = p2.next_output_index();

        assert_eq!((first + 1) % 2, second);
    }
}
