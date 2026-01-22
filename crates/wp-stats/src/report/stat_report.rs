use std::{cmp::Ordering, fmt::Display};

use lru::LruCache;
use wp_model_core::model::{DataField, DataRecord};

use crate::{
    Mergeable, SliceMetrics, StatReq, model::record::SliceRecord, model::stat_dim::StatDim,
};

use super::record::{StatRecord, WpStatTag};

/// Multiplier for top-N retention when merging reports
/// We keep 2x the requested max to allow for better merging and filtering
const TOP_N_MULTIPLIER: usize = 2;

pub type StatCache = LruCache<StatDim, SliceRecord<WpStatTag>>;

/// Statistical report containing aggregated metrics
///
/// `StatReport` holds collected statistics based on configured requirements,
/// providing methods for accessing, merging, and converting statistical data.
#[derive(Clone, Debug)]
pub struct StatReport {
    req: StatReq,
    cur_target: Option<String>,
    data: Vec<StatRecord>,
}
impl From<StatReq> for StatReport {
    fn from(req: StatReq) -> Self {
        Self {
            req,
            cur_target: None,
            data: Vec::new(),
        }
    }
}
impl StatReport {
    /// Creates a new StatReport with the given parameters
    pub fn new(req: StatReq, target: Option<String>, data: Vec<StatRecord>) -> Self {
        Self {
            req,
            cur_target: target,
            data,
        }
    }

    /// Returns the name of this report
    pub fn get_name(&self) -> &str {
        self.req.name.as_str()
    }

    /// Returns the data collection requirements
    pub fn data_reqs(&self) -> &Vec<String> {
        &self.req.collect
    }

    /// Checks if the given rule matches the target
    pub fn match_target(&self, rule: &str) -> bool {
        self.req.match_target(rule)
    }

    /// Returns the target display string
    pub fn target_display(&self) -> &str {
        if let Some(rule) = &self.cur_target {
            rule.as_str()
        } else {
            "*"
        }
    }

    /// Returns a reference to the statistical requirements
    pub fn get_req(&self) -> &StatReq {
        &self.req
    }
}

impl Eq for StatReport {}

impl PartialEq<Self> for StatReport {
    fn eq(&self, other: &Self) -> bool {
        self.req.stage == other.req.stage
            && self.req.name == other.req.name
            && self.cur_target == other.cur_target
    }
}

impl PartialOrd<Self> for StatReport {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StatReport {
    fn cmp(&self, other: &Self) -> Ordering {
        let order = self.req.name.cmp(&other.req.name);
        if order == Ordering::Equal {
            return self.cur_target.cmp(&other.cur_target);
        }
        order
    }
}

impl From<StatReport> for Vec<DataRecord> {
    fn from(other: StatReport) -> Self {
        let mut crates = vec![];
        for stat in other.data.iter() {
            let tdc = vec![
                DataField::from_chars("stage", other.req.stage.to_string().as_str()),
                DataField::from_chars("name", other.req.name.as_str()),
                DataField::from_chars("target", other.target_display()),
            ];

            let mut tdo = DataRecord::from(tdc);
            tdo.merge(stat.covert_tdc(&other.req.collect));
            crates.push(tdo);
        }
        crates
    }
}

impl Mergeable<StatReport> for StatReport {
    fn merge(&mut self, other: StatReport) {
        use std::collections::HashMap;

        self.cur_target = other.cur_target;

        // Build a HashMap for O(1) lookup during merge
        let mut data_map: HashMap<String, StatRecord> = HashMap::new();
        for item in self.data.drain(..) {
            data_map.insert(item.slices_key().to_string(), item);
        }

        // Merge other's data into the map
        for v in other.data {
            let key = v.slices_key().to_string();
            if let Some(existing) = data_map.get_mut(&key) {
                if existing.can_merge(&v) {
                    existing.merge(v);
                }
            } else {
                data_map.insert(key, v);
            }
        }

        // Convert back to Vec and sort
        self.data = data_map.into_values().collect();
        self.data.sort_by(|a, b| b.stat.total.cmp(&a.stat.total));
        self.data.truncate(self.req.max * TOP_N_MULTIPLIER);
    }
}

impl Display for StatReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "stage: {} ,name: {}, match: {} , rule: {}, max: {}",
            self.req.stage,
            self.req.name,
            self.req.target,
            self.target_display(),
            self.req.max
        )?;
        for stat in &self.data {
            writeln!(
                f,
                "{} {} {}",
                stat.slices_key(),
                stat.get_value(),
                stat.stat.total
            )?
        }
        Ok(())
    }
}

impl StatReport {
    /// Creates a StatReport from requirements
    pub fn req(req: StatReq) -> Self {
        Self {
            cur_target: None,
            data: Vec::new(),
            req,
        }
    }

    /// Checks if this report can be merged with another
    pub fn can_merge(&self, other: &Self) -> bool {
        self.req.stage == other.req.stage
            && self.req.name == other.req.name
            && self.cur_target == other.cur_target
    }

    /// Returns a reference to the statistical data
    pub fn get_data(&self) -> &Vec<StatRecord> {
        &self.data
    }
}
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::StatCollector;
    use crate::StatTarget;
    use crate::traits::recorder::StatRecorder;
    use collection_literals::hash;

    #[test]
    fn test_top_n() {
        let mut stat1 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 4),
        );
        stat1.record_begin("/", "a");
        stat1.record_begin("/", "b");
        stat1.record_begin("/", "c");
        stat1.record_begin("/", "d");
        stat1.record_begin("/", "a");
        stat1.record_begin("/", "b");
        stat1.record_begin("/", "d");
        stat1.record_begin("/", "a");

        let mut slices1 = stat1.collect_stat();
        println!("{}", slices1);
        let expect: HashMap<&str, usize> = hash! {
           "a" => 3,
           "b" => 2,
           "c" => 1,
           "d" => 2,
        };
        verify_top_n(&slices1, expect);

        let mut stat_2 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 4),
        );
        stat_2.record_begin("/", "b");
        stat_2.record_begin("/", "c");
        stat_2.record_begin("/", "d");
        stat_2.record_begin("/", "c");
        stat_2.record_begin("/", "a");
        stat_2.record_begin("/", "b");
        stat_2.record_begin("/", "a");

        let slices2 = stat_2.collect_stat();
        let expect: HashMap<&str, usize> = hash! {
           "a" => 2,
           "b" => 2,
           "c" => 2,
           "d" => 1,
        };

        verify_top_n(&slices2, expect);
        slices1.merge(slices2);
        let expect: HashMap<&str, usize> = hash! {
           "a" => 5,
           "b" => 4,
           "c" => 3,
           "d" => 3,
        };
        verify_top_n(&slices1, expect);
        stat1.record_begin("/", "c");
        stat1.record_begin("/", "c");
    }

    #[test]
    fn test_top_n_less() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 3);
        let mut stat1 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 3),
        );
        let mut total = StatReport::from(req);
        stat1.record_begin("/", "a");
        stat1.record_begin("/", "b");
        stat1.record_begin("/", "a");
        stat1.record_begin("/", "c");
        stat1.record_begin("/", "d");
        stat1.record_begin("/", "a");
        stat1.record_begin("/", "a");

        let expect: HashMap<&str, usize> = hash! {
           "a" => 4,
           "b" => 1,
           "d" => 1,
           //"b" => 2, maybe miss;
        };
        let stat_result1 = stat1.collect_stat();
        verify_top_n(&stat_result1, expect);
        total.merge(stat_result1.clone());

        println!("{}", total);
        let expect: HashMap<&str, usize> = hash! {
           "a" => 4,
           "b" => 1,
           "d" => 1,
        };
        verify_top_n(&total, expect);
        total.merge(stat_result1);

        println!("{}", total);
        let expect: HashMap<&str, usize> = hash! {
           "a" => 8,
           "b" => 2,
           "d" => 2,
        };
        verify_top_n(&total, expect);

        let mut stat2 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 3),
        );
        stat2.record_begin("/", "a");
        stat2.record_begin("/", "f");
        stat2.record_begin("/", "f");
        stat2.record_begin("/", "f");
        stat2.record_begin("/", "g");
        stat2.record_begin("/", "g");
        stat2.record_begin("/", "g");
        stat2.record_begin("/", "d");
        stat2.record_begin("/", "a");
        let slices2 = stat2.collect_stat();
        total.merge(slices2);
        let expect: HashMap<&str, usize> = hash! {
           "a" => 9,
           "f" => 3,
           "g" => 3,
        };
        verify_top_n(&total, expect);
    }

    fn verify_top_n(total: &StatReport, expect: HashMap<&str, usize>) {
        println!("{}", total);
        let result = total.get_data().clone();
        for item in result {
            if let Some(found) = expect.get(item.slices_key()) {
                assert_eq!(item.stat.total, *found);
            }
        }
    }

    #[test]
    fn test_merge_empty_reports() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 5);
        let mut report1 = StatReport::from(req.clone());
        let report2 = StatReport::from(req);

        report1.merge(report2);
        assert_eq!(report1.get_data().len(), 0);
    }

    #[test]
    fn test_merge_into_empty() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 5);
        let mut empty_report = StatReport::from(req.clone());

        let mut collector = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 5),
        );
        collector.record_task("/", "a");
        collector.record_task("/", "b");

        let report = collector.collect_stat();
        empty_report.merge(report);

        assert_eq!(empty_report.get_data().len(), 2);
    }

    #[test]
    fn test_merge_preserves_highest_counts() {
        let mut stat1 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );
        for _ in 0..5 {
            stat1.record_task("/", "a");
        }

        let report1 = stat1.collect_stat();

        let mut stat2 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );
        for _ in 0..3 {
            stat2.record_task("/", "a");
        }

        let mut report2 = stat2.collect_stat();
        report2.merge(report1);

        assert_eq!(report2.get_data().len(), 1);
        assert_eq!(report2.get_data()[0].stat.total, 8); // 5 + 3
    }

    #[test]
    fn test_merge_different_keys() {
        let mut stat1 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );
        stat1.record_task("/", "a");
        stat1.record_task("/", "b");

        let report1 = stat1.collect_stat();

        let mut stat2 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );
        stat2.record_task("/", "c");
        stat2.record_task("/", "d");

        let mut report2 = stat2.collect_stat();
        report2.merge(report1);

        assert_eq!(report2.get_data().len(), 4);
    }

    #[test]
    fn test_merge_respects_max_limit() {
        let mut stat1 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 2),
        );
        for i in 0..5 {
            stat1.record_task("/", format!("key{}", i).as_str());
        }

        let report1 = stat1.collect_stat();

        let mut stat2 = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 2),
        );
        for i in 5..10 {
            stat2.record_task("/", format!("key{}", i).as_str());
        }

        let mut report2 = stat2.collect_stat();
        report2.merge(report1);

        // Should be limited to max * TOP_N_MULTIPLIER = 2 * 2 = 4
        assert_eq!(report2.get_data().len(), 4);
    }

    #[test]
    fn test_can_merge() {
        let req1 = StatReq::simple_test2("test1", StatTarget::All, Vec::new(), 5);
        let req2 = StatReq::simple_test2("test2", StatTarget::All, Vec::new(), 5);

        let report1 = StatReport::new(req1.clone(), Some("target1".to_string()), Vec::new());
        let report2 = StatReport::new(req2.clone(), Some("target1".to_string()), Vec::new());
        let report3 = StatReport::new(req1.clone(), Some("target2".to_string()), Vec::new());

        // Same stage, name and target - should be mergeable
        assert!(report1.can_merge(&StatReport::new(
            req1.clone(),
            Some("target1".to_string()),
            Vec::new()
        )));

        // Different name - should not be mergeable
        assert!(!report1.can_merge(&report2));

        // Different target - should not be mergeable
        assert!(!report1.can_merge(&report3));
    }

    #[test]
    fn test_eq_implementation() {
        let req = StatReq::simple_test2("test", StatTarget::All, Vec::new(), 5);
        let report1 = StatReport::new(req.clone(), Some("target1".to_string()), Vec::new());
        let report2 = StatReport::new(req.clone(), Some("target1".to_string()), Vec::new());
        let report3 = StatReport::new(req.clone(), Some("target2".to_string()), Vec::new());

        assert_eq!(report1, report2);
        assert_ne!(report1, report3);
    }

    #[test]
    fn test_ord_implementation() {
        let req1 = StatReq::simple_test2("aaa", StatTarget::All, Vec::new(), 5);
        let req2 = StatReq::simple_test2("bbb", StatTarget::All, Vec::new(), 5);

        let report1 = StatReport::new(req1, Some("target1".to_string()), Vec::new());
        let report2 = StatReport::new(req2, Some("target1".to_string()), Vec::new());

        assert!(report1 < report2);

        let req3 = StatReq::simple_test2("aaa", StatTarget::All, Vec::new(), 5);
        let report3 = StatReport::new(req3.clone(), Some("aaa".to_string()), Vec::new());
        let report4 = StatReport::new(req3, Some("bbb".to_string()), Vec::new());

        assert!(report3 < report4);
    }

    #[test]
    fn test_data_record_conversion() {
        let mut stat = StatCollector::new(
            "/".to_string(),
            StatReq::simple_test2("test", StatTarget::All, vec!["field1".to_string()], 5),
        );
        stat.record_task("/", "value1");

        let report = stat.collect_stat();
        let records: Vec<DataRecord> = report.into();

        assert_eq!(records.len(), 1);
    }
}
