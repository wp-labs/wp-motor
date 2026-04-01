use crate::stat::MonSend;
use crate::stat::runtime_counters;
use wp_stat::DataDim;
use wp_stat::StatRecorder;
use wp_stat::StatReq;

use std::collections::HashMap;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::error::TrySendError;
use wp_data_fmt::{Raw, RecordFormatter};
use wp_model_core::model::DataRecord;
use wp_stat::ReportVariant;
use wp_stat::StatCollector;

#[derive(Clone)]
struct DataCollectorGroup {
    collect: Vec<String>,
    indices: Vec<usize>,
}

#[derive(Clone)]
pub struct MetricCollectors {
    pub(crate) items: Vec<StatCollector>,
    unit_collectors_end: usize,
    has_data_dims: bool,
    data_groups: Vec<DataCollectorGroup>,
}

pub fn extract_metric_dimensions(tdo: &DataRecord, collects: &[String]) -> DataDim {
    extract_metric_dimensions_with_target(tdo, collects)
}

fn extract_metric_dimensions_with_target(tdo: &DataRecord, collects: &[String]) -> DataDim {
    let mut data = DataDim::with_capacity(collects.len());
    let formatter = Raw;
    let parsed_target = target.rsplit_once('/');
    for key in collects {
        match key.as_str() {
            "wp_source_type" => {
                let value = tdo
                    .field("wp_source_type")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_access_ip" => {
                let value = tdo
                    .field("wp_access_ip")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_package_name" => {
                let value = tdo
                    .field("wp_package_name")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value.or_else(|| parsed_target.map(|(pkg, _)| pkg.to_string())));
            }
            "wp_rule_name" => {
                let value = tdo
                    .field("wp_rule_name")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value.or_else(|| parsed_target.map(|(_, rule)| rule.to_string())));
            }
            "wp_sink_group" => {
                let value = tdo
                    .field("wp_sink_group")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value.or_else(|| parsed_target.map(|(group, _)| group.to_string())));
            }
            "wp_sink_name" => {
                let value = tdo
                    .field("wp_sink_name")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value.or_else(|| parsed_target.map(|(_, name)| name.to_string())));
            }
            _ => {
                let value = tdo.field(key.as_str());
                data.push(value.map(|x| formatter.fmt_field(x).to_string()));
            }
        }
    }
    data
}

fn extract_metric_dimensions_from_target_only(collects: &[String], target: &str) -> DataDim {
    let mut data = DataDim::with_capacity(collects.len());
    let parsed_target = target.rsplit_once('/');
    for key in collects {
        match key.as_str() {
            "wp_source_type" => {
                let value = tdo
                    .field("wp_source_type")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_access_ip" => {
                let value = tdo
                    .field("wp_access_ip")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_package_name" => {
                let value = tdo
                    .field("wp_package_name")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_rule_name" => {
                let value = tdo
                    .field("wp_rule_name")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_sink_group" => {
                let value = tdo
                    .field("wp_sink_group")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            "wp_sink_name" => {
                let value = tdo
                    .field("wp_sink_name")
                    .map(|x| formatter.fmt_field(x).to_string());
                data.push(value);
            }
            _ => {
                let value = tdo.field(key.as_str());
                data.push(value.map(|x| formatter.fmt_field(x).to_string()));
            }
        }
    }
    data
}

fn extract_metric_dimensions_from_target_only(collects: &[String], _target: &str) -> DataDim {
    let mut data = DataDim::with_capacity(collects.len());
    for key in collects {
        match key.as_str() {
            "wp_package_name" | "wp_rule_name" | "wp_sink_group" | "wp_sink_name" => {
                data.push(None)
            }
            _ => data.push(None),
        }
    }
    data
}

fn is_target_only_collect(collects: &[String]) -> bool {
    collects.iter().all(|k| {
        matches!(
            k.as_str(),
            "wp_package_name" | "wp_rule_name" | "wp_sink_group" | "wp_sink_name"
        )
    })
}

impl StatRecorder<Option<&DataRecord>> for MetricCollectors {
    fn record_begin(&mut self, target: &str, dat: Option<&DataRecord>) {
        for idx in 0..self.unit_collectors_end {
            self.items[idx].record_begin(target, ());
        }

        match dat {
            Some(tdo) => {
                let (items, groups) = (&mut self.items, &self.data_groups);
                for group in groups.iter() {
                    if let Some((last, rest)) = group.indices.split_last() {
                        let data =
                            extract_metric_dimensions_with_target(tdo, group.collect.as_slice());
                        for idx in rest {
                            items[*idx].record_begin(target, data.clone());
                        }
                        items[*last].record_begin(target, data);
                    }
                }
            }
            None => {
                let (items, groups) = (&mut self.items, &self.data_groups);
                for group in groups.iter() {
                    if let Some((last, rest)) = group.indices.split_last() {
                        if is_target_only_collect(group.collect.as_slice()) {
                            let data = extract_metric_dimensions_from_target_only(
                                group.collect.as_slice(),
                                target,
                            );
                            for idx in rest {
                                items[*idx].record_begin(target, data.clone());
                            }
                            items[*last].record_begin(target, data);
                        } else {
                            for idx in group.indices.iter() {
                                items[*idx].record_begin(target, ());
                            }
                        }
                    }
                }
            }
        }
    }

    fn record_end(&mut self, rule: &str, dat: Option<&DataRecord>) {
        for idx in 0..self.unit_collectors_end {
            self.items[idx].record_end(rule, ());
        }

        match dat {
            Some(tdo) => {
                let (items, groups) = (&mut self.items, &self.data_groups);
                for group in groups.iter() {
                    if let Some((last, rest)) = group.indices.split_last() {
                        let data =
                            extract_metric_dimensions_with_target(tdo, group.collect.as_slice());
                        for idx in rest {
                            items[*idx].record_end(rule, data.clone());
                        }
                        items[*last].record_end(rule, data);
                    }
                }
            }
            None => {
                for idx in self.unit_collectors_end..self.items.len() {
                    self.items[idx].record_end(rule, ());
                }
            }
        }
    }

    fn record_task(&mut self, rule: &str, dat: Option<&DataRecord>) {
        for idx in 0..self.unit_collectors_end {
            self.items[idx].record_task(rule, ());
        }

        match dat {
            Some(tdo) => {
                let (items, groups) = (&mut self.items, &self.data_groups);
                for group in groups.iter() {
                    if let Some((last, rest)) = group.indices.split_last() {
                        let data =
                            extract_metric_dimensions_with_target(tdo, group.collect.as_slice());
                        for idx in rest {
                            items[*idx].record_task(rule, data.clone());
                        }
                        items[*last].record_task(rule, data);
                    }
                }
            }
            None => {
                for idx in self.unit_collectors_end..self.items.len() {
                    self.items[idx].record_task(rule, ());
                }
            }
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
    pub fn has_pending_data(&self) -> bool {
        self.items.iter().any(|c| !c.get_cache().is_empty())
    }

    pub fn touch_task_unit(&mut self, target: &str) {
        for c in self.items.iter_mut() {
            c.record_task_n_unit(target, 0);
        }
    }

    pub fn touch_task_dim(&mut self, target: &str, dim: DataDim) {
        for c in self.items.iter_mut() {
            c.record_task_n(target, dim.clone(), 0);
        }
    }

    pub fn touch_task_record(&mut self, target: &str, tdo: &DataRecord) {
        for idx in 0..self.unit_collectors_end {
            self.items[idx].record_task_n_unit(target, 0);
        }
        let (items, groups) = (&mut self.items, &self.data_groups);
        for group in groups.iter() {
            let data = extract_metric_dimensions_with_target(tdo, group.collect.as_slice());
            for idx in group.indices.iter() {
                items[*idx].record_task_n(target, data.clone(), 0);
            }
        }
    }

    /// Batch record helper for unit `()` dat_key: add `count` occurrences at once.
    pub fn record_task_batch(&mut self, target: &str, count: usize) {
        if count == 0 {
            return;
        }
        for c in self.items.iter_mut() {
            c.record_task_n_unit(target, count);
        }
    }

    /// Batch record helper for source metadata key counts.
    ///
    /// 当事件批次中存在 `wp_access_ip` 等字符串键时，按键聚合计数并写入统计维度；
    /// 对于缺少键的事件，回退到 unit 计数，保证总量不丢失。
    pub fn record_task_batch_by_str_key(
        &mut self,
        target: &str,
        key_counts: &HashMap<String, usize>,
        total_count: usize,
    ) {
        if total_count == 0 {
            return;
        }
        if key_counts.is_empty() {
            self.record_task_batch(target, total_count);
            return;
        }

        for collector in self.items.iter_mut() {
            let mut tagged_total = 0usize;
            for (key, cnt) in key_counts {
                if *cnt == 0 {
                    continue;
                }
                collector.record_task_n_str(target, key.as_str(), *cnt);
                tagged_total += *cnt;
            }
            if tagged_total < total_count {
                collector.record_task_n_unit(target, total_count - tagged_total);
            }
        }
    }

    /// Batch record helper for `(wp_source_type, wp_access_ip)` dimensions.
    ///
    /// 对于每个维度二元组做聚合计数；若存在缺失维度的事件，则回退到 unit 计数。
    pub fn record_task_batch_by_source_ip(
        &mut self,
        target: &str,
        pair_counts: &HashMap<(String, String), usize>,
        total_count: usize,
    ) {
        if total_count == 0 {
            return;
        }
        if pair_counts.is_empty() {
            self.record_task_batch(target, total_count);
            return;
        }

        for collector in self.items.iter_mut() {
            let mut tagged_total = 0usize;
            for ((source_type, access_ip), cnt) in pair_counts {
                if *cnt == 0 {
                    continue;
                }
                let mut dim = DataDim::with_capacity(2);
                dim.push(Some(source_type.clone()));
                dim.push(Some(access_ip.clone()));
                collector.record_task_n(target, dim, *cnt);
                tagged_total += *cnt;
            }
            if tagged_total < total_count {
                collector.record_task_n_unit(target, total_count - tagged_total);
            }
        }
    }

    pub fn supports_unit_batch(&self) -> bool {
        !self.has_data_dims
    }

    pub fn record_begin_batch_unit(&mut self, target: &str, count: usize) {
        if count == 0 {
            return;
        }
        debug_assert!(
            self.supports_unit_batch(),
            "record_begin_batch_unit requires collectors without dimensions"
        );
        for c in self.items.iter_mut() {
            c.record_begin_n_unit(target, count);
        }
    }

    pub fn record_end_batch_unit(&mut self, target: &str, count: usize) {
        if count == 0 {
            return;
        }
        debug_assert!(
            self.supports_unit_batch(),
            "record_end_batch_unit requires collectors without dimensions"
        );
        for c in self.items.iter_mut() {
            c.record_end_n_unit(target, count);
        }
    }
}

impl MetricCollectors {
    pub fn new(target: String, stat_reqs: Vec<StatReq>) -> Self {
        let mut unit_items = Vec::new();
        let mut data_items = Vec::new();

        for req in stat_reqs {
            if !req.match_target(target.as_str()) {
                continue;
            }
            if req.collect.is_empty() {
                unit_items.push(StatCollector::new(target.clone(), req));
            } else {
                data_items.push(StatCollector::new(target.clone(), req));
            }
        }

        let mut data_groups: Vec<DataCollectorGroup> = Vec::new();
        let mut collect_group_map: HashMap<Vec<String>, usize> = HashMap::new();
        for (rel_idx, collector) in data_items.iter().enumerate() {
            let collect = collector.get_req().collect.clone();
            if let Some(group_idx) = collect_group_map.get(&collect).copied() {
                data_groups[group_idx].indices.push(rel_idx);
            } else {
                let group_idx = data_groups.len();
                collect_group_map.insert(collect.clone(), group_idx);
                data_groups.push(DataCollectorGroup {
                    collect,
                    indices: vec![rel_idx],
                });
            }
        }

        let unit_collectors_end = unit_items.len();
        let has_data_dims = !data_items.is_empty();
        for group in data_groups.iter_mut() {
            for idx in group.indices.iter_mut() {
                *idx += unit_collectors_end;
            }
        }
        unit_items.extend(data_items);

        Self {
            items: unit_items,
            unit_collectors_end,
            has_data_dims,
            data_groups,
        }
    }
    pub fn up_target(&mut self, target: String) {
        for item in self.items.iter_mut() {
            item.up_target(target.clone());
        }
    }
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> Result<(), SendError<ReportVariant>> {
        let batch_time = fast_now();
        for requ in self.items.iter_mut() {
            let slices = requ.collect_stat_with_time(batch_time);
            let r = mon_send.try_send(ReportVariant::Stat(slices));
            // 在 debug 构建下，监控通道关闭被视为严重错误，直接中断，利于尽快发现监控消费端未启动的问题。
            #[cfg(debug_assertions)]
            {
                if let Err(TrySendError::Closed(e)) = &r {
                    let _ = e;
                    debug_assert!(false, "monitor channel closed when sending stat");
                }
            }
            match r {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    runtime_counters::rec_monitor_send_drop_full();
                }
                Err(TrySendError::Closed(s)) => {
                    runtime_counters::rec_monitor_send_drop_closed();
                    return Err(SendError(s));
                }
            }
        }
        Ok(())
    }
}

fn fast_now() -> chrono::NaiveDateTime {
    use chrono::Local;
    Local::now().naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wp_model_core::model::{DataField, DataRecord};
    use wp_stat::{StatRecorder, StatTarget};

    #[test]
    fn none_input_still_records_when_collect_fields_present() {
        let mut collectors = MetricCollectors::new(
            "ruleA".to_string(),
            vec![StatReq::simple_test(
                StatTarget::All,
                vec!["k".to_string()],
                10,
            )],
        );

        collectors.record_begin("ruleA", None);
        collectors.record_end("ruleA", None);

        let report = collectors.items[0].collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 1);
        assert_eq!(report.get_data()[0].stat.success, 1);
    }

    #[test]
    fn sink_dims_should_use_record_fields_only() {
        let mut collectors = MetricCollectors::new(
            "monitor/victoriametrics".to_string(),
            vec![StatReq::simple_test(
                StatTarget::All,
                vec!["wp_sink_group".to_string(), "wp_sink_name".to_string()],
                10,
            )],
        );
        let record = DataRecord::from(vec![
            DataField::from_chars("wp_sink_group", "residue"),
            DataField::from_chars("wp_sink_name", "residue"),
        ]);

        collectors.record_begin("monitor/victoriametrics", Some(&record));
        collectors.record_end("monitor/victoriametrics", Some(&record));

        let report = collectors.items[0].collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(
            report.get_data()[0].get_value().to_string(),
            "residue,residue"
        );
        assert_eq!(report.get_data()[0].stat.total, 1);
        assert_eq!(report.get_data()[0].stat.success, 1);
    }

    #[test]
    fn parse_dims_should_use_record_fields_only() {
        let mut collectors = MetricCollectors::new(
            "pkg_a/rule_a".to_string(),
            vec![StatReq::simple_test(
                StatTarget::All,
                vec!["wp_package_name".to_string(), "wp_rule_name".to_string()],
                10,
            )],
        );
        let record = DataRecord::from(vec![
            DataField::from_chars("wp_package_name", "pkg_b"),
            DataField::from_chars("wp_rule_name", "rule_b"),
        ]);

        collectors.record_begin("pkg_a/rule_a", Some(&record));
        collectors.record_end("pkg_a/rule_a", Some(&record));

        let report = collectors.items[0].collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].get_value().to_string(), "pkg_b,rule_b");
        assert_eq!(report.get_data()[0].stat.total, 1);
        assert_eq!(report.get_data()[0].stat.success, 1);
    }
}
