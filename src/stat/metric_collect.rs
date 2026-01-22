use crate::stat::MonSend;
use wp_stat::DataDim;
use wp_stat::StatRecorder;
use wp_stat::StatReq;

use tokio::sync::mpsc::error::SendError;
use wp_data_fmt::{DataFormat, Raw};
use wp_model_core::model::DataRecord;
use wp_stat::ReportVariant;
use wp_stat::StatCollector;

#[derive(Clone)]
pub struct MetricCollectors {
    pub(crate) items: Vec<StatCollector>,
}

pub fn extract_metric_dimensions(tdo: &DataRecord, collects: &Vec<String>) -> DataDim {
    let mut data = DataDim::empty();
    let formatter = Raw;
    for key in collects {
        let value = tdo.field(key.as_str());
        data.push(value.map(|x| formatter.format_field(x).to_string()));
    }
    data
}
impl StatRecorder<Option<&DataRecord>> for MetricCollectors {
    fn record_begin(&mut self, target: &str, dat: Option<&DataRecord>) {
        for fixture in self.items.iter_mut() {
            if let (Some(tdo), reqs) = (dat, fixture.get_req()) {
                let data = extract_metric_dimensions(tdo, &reqs.collect);
                fixture.record_begin(target, data);
                continue;
            }
            fixture.record_begin(target, ());
        }
    }

    fn record_end(&mut self, rule: &str, dat: Option<&DataRecord>) {
        for requ in self.items.iter_mut() {
            if let (Some(tdo), reqs) = (dat, requ.get_req()) {
                let data = extract_metric_dimensions(tdo, &reqs.collect);
                requ.record_end(rule, data);
                continue;
            }
            requ.record_end(rule, ());
        }
    }
    fn record_task(&mut self, rule: &str, dat: Option<&DataRecord>) {
        for requ in self.items.iter_mut() {
            if let (Some(tdo), reqs) = (dat, requ.get_req()) {
                let data = extract_metric_dimensions(tdo, &reqs.collect);
                requ.record_task(rule, data);
                continue;
            }
            requ.record_task(rule, ());
        }
    }
}

impl StatRecorder<&str> for MetricCollectors {
    fn record_begin(&mut self, target: &str, dat_key: &str) {
        for requ in self.items.iter_mut() {
            requ.record_begin(target, DataDim::from(dat_key));
        }
    }

    fn record_end(&mut self, target: &str, dat_key: &str) {
        for requ in self.items.iter_mut() {
            requ.record_end(target, DataDim::from(dat_key));
        }
    }

    fn record_task(&mut self, target: &str, dat_key: &str) {
        for requ in self.items.iter_mut() {
            requ.record_task(target, DataDim::from(dat_key));
        }
    }
}

impl StatRecorder<()> for MetricCollectors {
    fn record_begin(&mut self, target: &str, dat_key: ()) {
        for requ in self.items.iter_mut() {
            requ.record_begin(target, dat_key);
        }
    }

    fn record_end(&mut self, target: &str, dat_key: ()) {
        for requ in self.items.iter_mut() {
            requ.record_end(target, dat_key);
        }
    }

    fn record_task(&mut self, target: &str, dat_key: ()) {
        for requ in self.items.iter_mut() {
            requ.record_task(target, dat_key);
        }
    }
}

impl MetricCollectors {
    /// Batch record helper for unit `()` dat_key: add `count` occurrences at once.
    pub fn record_task_batch(&mut self, target: &str, count: usize) {
        if count == 0 {
            return;
        }
        for c in self.items.iter_mut() {
            c.record_task_n_unit(target, count);
        }
    }
}

impl MetricCollectors {
    pub fn new(target: String, stat_reqs: Vec<StatReq>) -> Self {
        let mut items = Vec::new();
        for req in stat_reqs {
            if req.match_target(target.as_str()) {
                items.push(StatCollector::new(target.clone(), req));
            }
        }
        Self { items }
    }
    pub fn up_target(&mut self, target: String) {
        for item in self.items.iter_mut() {
            item.up_target(target.clone());
        }
    }
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> Result<(), SendError<ReportVariant>> {
        let batch_time = fast_now();
        for requ in self.items.iter_mut() {
            requ.finalize_with_time(batch_time);
            let slices = requ.collect_stat();
            let r = mon_send.send(ReportVariant::Stat(slices)).await;
            // 在 debug 构建下，监控通道关闭被视为严重错误，直接中断，利于尽快发现监控消费端未启动的问题。
            #[cfg(debug_assertions)]
            {
                if let Err(e) = &r {
                    debug_assert!(false, "monitor channel closed when sending stat: {}", e);
                }
            }
            r?;
        }
        Ok(())
    }
}

fn fast_now() -> chrono::NaiveDateTime {
    use chrono::Local;
    Local::now().naive_local()
}
