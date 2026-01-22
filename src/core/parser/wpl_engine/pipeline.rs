use crate::core::prelude::*;
use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::facade::test_helpers::SinkTerminal;
use crate::sinks::SinkGroupAgent;
use crate::stat::MonSend;
use crate::stat::metric_collect::MetricCollectors;
use std::cmp::Ordering;
use wp_parse_api::{DataResult, RawData};
use wp_stat::StatRecorder;
use wp_stat::StatReq;
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
    wpl_key: String,
    s_name: String,
    stat_ext: MetricCollectors,
}

impl WplPipeline {
    pub fn new(
        index: usize,
        wpl_key: String,
        fun_vec: Vec<AnnotationType>,
        parser: WplEvaluator,
        output: Vec<SinkGroupAgent>,
        stat_reqs: Vec<StatReq>,
    ) -> Self {
        //let s_name = name.split('/').last().unwrap_or(&name);
        let s_name = wpl_key.clone();
        let stat_ext = MetricCollectors::new(wpl_key.clone(), stat_reqs);

        Self {
            parser,
            fun_vec,
            index,
            wpl_key,
            output,
            hit_cnt: 0,
            access_cnt: 0,
            s_name,
            stat_ext,
        }
    }

    pub fn short_name(&self) -> &str {
        self.s_name.as_str()
    }
    pub fn get_rolled_end(&self) -> &SinkTerminal {
        let idx = self.hit_cnt % self.output().len();
        self.output[idx].end_point()
    }
    pub fn proc(&mut self, data: &SourceEvent, oth_suc_len: usize) -> DataResult {
        self.access_cnt += 1;
        match self.parser.proc(data.payload.clone(), oth_suc_len) {
            Ok((mut record, left)) => {
                self.stat_ext.record_begin(self.wpl_key.as_str(), None);
                for func in self.fun_vec.iter() {
                    func.proc(data, &mut record)?;
                }
                self.stat_ext
                    .record_end(self.wpl_key.as_str(), Some(&record));
                Ok((record, RawData::from_string(left)))
            }
            Err(e) => Err(e),
        }
    }
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> WparseResult<()> {
        self.stat_ext.send_stat(mon_send).await.owe_sys()?;
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
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
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
