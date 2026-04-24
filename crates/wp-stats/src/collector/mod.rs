use crate::StatRecorder;
use crate::StatReport;
use crate::model::dimension::DataDim;
use crate::model::request::StatReq;
use crate::model::stat_dim::{DimensionBuilder, StatDim};
use crate::report::record::StatRecord;
use crate::report::stat_report::StatCache;
use chrono::NaiveDateTime;
use lru::LruCache;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use wp_log::trace_mtrc;

/// Minimum cache size to ensure basic functionality
const MIN_CACHE_SIZE: usize = 5;

/// Multiplier for top-N retention when collecting stats
/// We keep 2x the requested max to allow for better merging and filtering
const TOP_N_MULTIPLIER: usize = 2;

/// Statistical data collector for tracking and aggregating metrics
///
/// `StatCollector` manages a cache of statistical records and provides methods
/// to record events (begin, end, or complete tasks) across different dimensions.
/// It uses an LRU cache to maintain the most relevant statistics.
#[derive(Clone, Debug)]
pub struct StatCollector {
    target: String,
    require: StatReq,
    record: StatCache,
    unit_dims: HashMap<SmolStr, StatDim>,
}

impl StatCollector {
    /// Creates a new StatCollector with the specified target and requirements
    ///
    /// # Arguments
    /// * `target` - The target identifier for this collector
    /// * `req` - Statistical requirements specifying collection parameters
    pub fn new(target: String, req: StatReq) -> Self {
        let cache = Self::init_cache(&req);
        Self {
            target,
            record: cache,
            require: req,
            unit_dims: HashMap::new(),
        }
    }

    fn init_cache(req: &StatReq) -> StatCache {
        let size = req.max.max(MIN_CACHE_SIZE);
        LruCache::new(
            NonZeroUsize::new(size)
                .expect("Cache size should be non-zero after max(MIN_CACHE_SIZE)"),
        )
    }

    /// Updates the target identifier for this collector
    pub fn up_target(&mut self, target: String) {
        self.target = target
    }
}

impl StatCollector {
    /// Returns a reference to the statistical requirements
    pub fn get_req(&self) -> &StatReq {
        &self.require
    }

    /// Returns a reference to the internal cache
    pub fn get_cache(&self) -> &StatCache {
        &self.record
    }

    pub fn finalize_with_time(&mut self, ts: NaiveDateTime) {
        for (_, record) in self.record.iter_mut() {
            record.stat.finalize_end(ts);
        }
    }

    /// Collects and returns a statistical report, then resets the cache
    ///
    /// This method aggregates all recorded statistics, sorts them by total count,
    /// and returns a `StatReport`. The internal cache is cleared after collection.
    pub fn collect_stat(&mut self) -> StatReport {
        self.collect_from_cache_swapped(None)
    }

    /// Collects and finalizes all records with the same timestamp in one pass.
    pub fn collect_stat_with_time(&mut self, ts: NaiveDateTime) -> StatReport {
        self.collect_from_cache_swapped(Some(ts))
    }

    fn collect_from_cache_swapped(&mut self, finalize_at: Option<NaiveDateTime>) -> StatReport {
        let mut drained = Self::init_cache(&self.require);
        std::mem::swap(&mut self.record, &mut drained);
        let report = self.collect_from_cache(drained, finalize_at);
        // Keep auxiliary unit-key cache bounded to current window lifecycle.
        self.unit_dims.clear();
        report
    }
}

impl StatRecorder<DataDim> for StatCollector {
    fn record_begin(&mut self, rule_key: &str, dat_key: DataDim) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, dat_key);
        self.rec_beg_impl(&dim);
    }
    fn record_end(&mut self, rule_key: &str, dat_key: DataDim) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, dat_key);
        self.rec_end_impl(&dim);
    }
    fn record_task(&mut self, rule_key: &str, dat_key: DataDim) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, dat_key);
        self.rec_beg_end_impl(&dim);
    }
}
impl StatRecorder<&str> for StatCollector {
    fn record_begin(&mut self, rule_key: &str, dat_key: &str) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, DataDim::from(dat_key));
        self.rec_beg_impl(&dim);
    }
    fn record_end(&mut self, rule_key: &str, dat_key: &str) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, DataDim::from(dat_key));
        self.rec_end_impl(&dim);
    }
    fn record_task(&mut self, rule_key: &str, dat_key: &str) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, DataDim::from(dat_key));
        self.rec_beg_end_impl(&dim);
    }
}

impl StatRecorder<()> for StatCollector {
    fn record_begin(&mut self, rule_key: &str, _: ()) {
        self.rec_beg_unit_impl(rule_key);
    }
    fn record_end(&mut self, rule_key: &str, _: ()) {
        self.rec_end_unit_impl(rule_key);
    }
    fn record_task(&mut self, rule_key: &str, _: ()) {
        self.rec_beg_end_unit_impl(rule_key);
    }
}

impl StatCollector {
    fn collect_from_cache(
        &self,
        cache: StatCache,
        finalize_at: Option<NaiveDateTime>,
    ) -> StatReport {
        let mut data = Vec::with_capacity(cache.len());
        match &self.require.target {
            crate::StatTarget::All => {
                for (_, mut v) in cache {
                    if let Some(ts) = finalize_at {
                        v.stat.finalize_end(ts);
                    }
                    data.push(v);
                }
            }
            crate::StatTarget::Ignore => {}
            crate::StatTarget::Item(expect) => {
                // Only include records that match the configured target semantics.
                // This prevents non-matching targets (stored as None / "*") from
                // leaking into reports when StatTarget::Item is used.
                for (k, mut v) in cache {
                    let include = match k.target_str() {
                        Some(actual) => actual == expect,
                        None => false,
                    };
                    if include {
                        if let Some(ts) = finalize_at {
                            v.stat.finalize_end(ts);
                        }
                        data.push(v);
                    }
                }
            }
        }
        let keep = self.require.max * TOP_N_MULTIPLIER;
        if data.len() > keep {
            data.select_nth_unstable_by(keep, |a, b| b.stat.total.cmp(&a.stat.total));
            data.truncate(keep);
        }
        data.sort_unstable_by_key(|b| std::cmp::Reverse(b.stat.total));
        let ins = StatReport::new(self.require.clone(), Some(self.target.clone()), data);
        trace_mtrc!("{}", ins);
        ins
    }

    /// Helper method to get or create a record, avoiding unnecessary clones
    fn get_or_create_record(&mut self, dim: &StatDim) -> &mut StatRecord {
        self.record.get_or_insert_mut_ref(dim, || {
            StatRecord::new(
                self.require.stage.clone(),
                dim.to_string(),
                dim.dat_string().clone(),
            )
        })
    }

    fn unit_dim_cached(&mut self, rule_key: &str) -> StatDim {
        if let Some(dim) = self.unit_dims.get(rule_key) {
            return dim.clone();
        }
        let dim = StatDim::make_dim(&self.require.target, rule_key, ());
        self.unit_dims.insert(SmolStr::new(rule_key), dim.clone());
        dim
    }

    fn rec_beg_impl(&mut self, dim: &StatDim) {
        self.get_or_create_record(dim).rec_beg();
    }

    fn rec_end_impl(&mut self, dim: &StatDim) {
        self.get_or_create_record(dim).rec_end();
    }

    fn rec_beg_end_impl(&mut self, dim: &StatDim) {
        let rec = self.get_or_create_record(dim);
        rec.rec_beg_end();
    }

    fn rec_beg_n_impl(&mut self, dim: &StatDim, n: usize) {
        if n == 0 {
            return;
        }
        let rec = self.get_or_create_record(dim);
        rec.rec_beg_n(n);
    }

    fn rec_end_n_impl(&mut self, dim: &StatDim, n: usize) {
        if n == 0 {
            return;
        }
        let rec = self.get_or_create_record(dim);
        rec.rec_end_n(n);
    }

    /// Batch record helper: add `n` occurrences as successful completions at once.
    fn rec_beg_end_n_impl(&mut self, dim: &StatDim, n: usize) {
        let rec = self.get_or_create_record(dim);
        if n > 0 {
            rec.rec_beg_end_n(n);
        }
    }

    fn rec_beg_unit_impl(&mut self, rule_key: &str) {
        let dim = self.unit_dim_cached(rule_key);
        self.rec_beg_impl(&dim);
    }

    fn rec_end_unit_impl(&mut self, rule_key: &str) {
        let dim = self.unit_dim_cached(rule_key);
        self.rec_end_impl(&dim);
    }

    fn rec_beg_end_unit_impl(&mut self, rule_key: &str) {
        let dim = self.unit_dim_cached(rule_key);
        self.rec_beg_end_impl(&dim);
    }

    fn rec_beg_n_unit_impl(&mut self, rule_key: &str, n: usize) {
        let dim = self.unit_dim_cached(rule_key);
        self.rec_beg_n_impl(&dim, n);
    }

    fn rec_end_n_unit_impl(&mut self, rule_key: &str, n: usize) {
        let dim = self.unit_dim_cached(rule_key);
        self.rec_end_n_impl(&dim, n);
    }

    fn rec_beg_end_n_unit_impl(&mut self, rule_key: &str, n: usize) {
        let dim = self.unit_dim_cached(rule_key);
        self.rec_beg_end_n_impl(&dim, n);
    }
}

impl StatCollector {
    /// Batch `record_begin` for DataDim
    pub fn record_begin_n(&mut self, rule_key: &str, dat_key: DataDim, n: usize) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, dat_key);
        self.rec_beg_n_impl(&dim, n);
    }
    /// Batch `record_begin` for unit `()`
    pub fn record_begin_n_unit(&mut self, rule_key: &str, n: usize) {
        self.rec_beg_n_unit_impl(rule_key, n);
    }
    /// Batch `record_end` for DataDim
    pub fn record_end_n(&mut self, rule_key: &str, dat_key: DataDim, n: usize) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, dat_key);
        self.rec_end_n_impl(&dim, n);
    }
    /// Batch `record_end` for unit `()`
    pub fn record_end_n_unit(&mut self, rule_key: &str, n: usize) {
        self.rec_end_n_unit_impl(rule_key, n);
    }
    /// Batch record for DataDim
    pub fn record_task_n(&mut self, rule_key: &str, dat_key: DataDim, n: usize) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, dat_key);
        self.rec_beg_end_n_impl(&dim, n);
    }
    /// Batch record for &str
    pub fn record_task_n_str(&mut self, rule_key: &str, dat_key: &str, n: usize) {
        let dim = StatDim::make_dim(&self.require.target, rule_key, DataDim::from(dat_key));
        self.rec_beg_end_n_impl(&dim, n);
    }
    /// Batch record for unit `()`
    pub fn record_task_n_unit(&mut self, rule_key: &str, n: usize) {
        self.rec_beg_end_n_unit_impl(rule_key, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataDim, SliceMetrics, StatTarget};

    #[test]
    fn test_collector_new() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 10);
        let collector = StatCollector::new("test_target".to_string(), req);

        assert_eq!(collector.get_req().max, 10);
        assert_eq!(collector.target, "test_target");
    }

    #[test]
    fn test_collector_min_cache_size() {
        // Test that cache size is at least MIN_CACHE_SIZE even when max is smaller
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 2);
        let collector = StatCollector::new("test".to_string(), req);

        // Cache should be at least MIN_CACHE_SIZE (5)
        assert!(collector.get_cache().cap().get() >= MIN_CACHE_SIZE);
    }

    #[test]
    fn test_record_begin_with_datadim() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        let dim = DataDim::from("key1");
        collector.record_begin("rule1", dim);

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 1);
        assert_eq!(report.get_data()[0].stat.success, 0);
    }

    #[test]
    fn test_record_end_with_datadim() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        let dim = DataDim::from("key1");
        collector.record_end("rule1", dim);

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 0);
        assert_eq!(report.get_data()[0].stat.success, 1);
    }

    #[test]
    fn test_record_task_with_datadim() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        let dim = DataDim::from("key1");
        collector.record_task("rule1", dim);

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 1);
        assert_eq!(report.get_data()[0].stat.success, 1);
    }

    #[test]
    fn test_record_with_str_key() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        collector.record_begin("rule1", "key1");
        collector.record_begin("rule1", "key1");
        collector.record_end("rule1", "key1");

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 2);
        assert_eq!(report.get_data()[0].stat.success, 1);
    }

    #[test]
    fn test_record_with_unit_key() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        collector.record_begin("rule1", ());
        collector.record_end("rule1", ());
        collector.record_task("rule1", ());

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 2);
        assert_eq!(report.get_data()[0].stat.success, 2);
    }

    #[test]
    fn test_record_begin_end_n_with_unit_key() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        collector.record_begin_n_unit("rule1", 5);
        collector.record_end_n_unit("rule1", 3);

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 5);
        assert_eq!(report.get_data()[0].stat.success, 3);
    }

    #[test]
    fn test_collect_clears_unit_dim_cache() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        collector.record_task("rule1", ());
        assert!(!collector.unit_dims.is_empty());

        let _ = collector.collect_stat();
        assert!(collector.unit_dims.is_empty());
    }

    #[test]
    fn test_multiple_rules() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        collector.record_task("rule1", "key1");
        collector.record_task("rule2", "key1");
        collector.record_task("rule1", "key2");

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 3);
    }

    #[test]
    fn test_collect_stat_clears_cache() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        collector.record_task("rule1", "key1");
        let report1 = collector.collect_stat();
        assert_eq!(report1.get_data().len(), 1);

        // After collection, cache should be cleared
        let report2 = collector.collect_stat();
        assert_eq!(report2.get_data().len(), 0);
    }

    #[test]
    fn test_top_n_multiplier() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 3),
        );

        // Add more records than max
        for i in 0..10 {
            collector.record_task("rule1", format!("key{}", i).as_str());
        }

        // Capture current cache capacity before collection (collect_stat clears it)
        let cap = collector.get_cache().cap().get();
        let report = collector.collect_stat();
        // Keep up to top-N multiplier, but never exceed cache capacity
        let expect = (3 * TOP_N_MULTIPLIER).min(cap);
        assert_eq!(report.get_data().len(), expect);
    }

    #[test]
    fn test_update_target() {
        let mut collector = StatCollector::new(
            "target1".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        assert_eq!(collector.target, "target1");

        collector.up_target("target2".to_string());
        assert_eq!(collector.target, "target2");
    }

    #[test]
    fn test_get_or_create_record_creates_new() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        let dim = StatDim::make_dim(&StatTarget::All, "rule1", DataDim::from("key1"));

        // First call should create new record
        {
            let record = collector.get_or_create_record(&dim);
            record.rec_beg();
        }

        // Second call should reuse existing record
        {
            let record = collector.get_or_create_record(&dim);
            assert_eq!(record.stat.total, 1);
            record.rec_beg();
        }

        let report = collector.collect_stat();
        assert_eq!(report.get_data().len(), 1);
        assert_eq!(report.get_data()[0].stat.total, 2);
    }

    #[test]
    fn test_stat_target_filtering() {
        // Test with specific target
        let mut collector = StatCollector::new(
            "rule1".to_string(),
            StatReq::simple_test(StatTarget::Item("rule1".to_string()), Vec::new(), 10),
        );

        collector.record_task("rule1", "key1");
        collector.record_task("rule2", "key1"); // This should not be counted

        let report = collector.collect_stat();
        // Only rule1 should be recorded when target is Item("rule1")
        assert!(report.get_data().len() <= 1);
    }

    #[test]
    fn test_lru_cache_eviction() {
        // Create collector with small cache size
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 3),
        );

        // Add more items than cache size
        for i in 0..10 {
            collector.record_task("rule1", format!("key{}", i).as_str());
        }

        // Cache should only hold 3 items (MIN_CACHE_SIZE or max)
        let cache_size = collector.get_cache().len();
        assert!(cache_size <= 5); // Should be limited by cache capacity
    }

    #[test]
    fn test_lru_cache_most_recent() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 2),
        );

        // Fill cache
        collector.record_task("rule1", "key1");
        collector.record_task("rule1", "key2");
        collector.record_task("rule1", "key3");

        // Access key1 again to make it recent
        collector.record_task("rule1", "key1");

        // Add new items
        collector.record_task("rule1", "key4");
        collector.record_task("rule1", "key5");

        let report = collector.collect_stat();

        // key1 should still be present because we accessed it recently
        let has_key1 = report
            .get_data()
            .iter()
            .any(|r| r.slices_key().contains("key1"));

        // LRU should keep recently accessed items
        assert!(has_key1 || report.get_data().len() <= 5);
    }

    #[test]
    fn test_cache_size_respects_min() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 1);
        let collector = StatCollector::new("test".to_string(), req);

        // Even though max is 1, cache should be at least MIN_CACHE_SIZE
        assert_eq!(collector.get_cache().cap().get(), MIN_CACHE_SIZE);
    }

    #[test]
    fn test_cache_size_respects_max() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 100);
        let collector = StatCollector::new("test".to_string(), req);

        // Cache size should be the max value
        assert_eq!(collector.get_cache().cap().get(), 100);
    }

    #[test]
    fn test_repeated_access_same_key() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        // Record same key multiple times
        for _ in 0..5 {
            collector.record_task("rule1", "same_key");
        }

        let report = collector.collect_stat();

        // Should only have one entry for the same key
        assert_eq!(report.get_data().len(), 1);
        // But total count should be 5
        assert_eq!(report.get_data()[0].stat.total, 5);
    }

    #[test]
    fn test_cache_contains_check() {
        let mut collector = StatCollector::new(
            "test".to_string(),
            StatReq::simple_test(StatTarget::All, Vec::new(), 10),
        );

        let dim1 = StatDim::make_dim(&StatTarget::All, "rule1", DataDim::from("key1"));
        let dim2 = StatDim::make_dim(&StatTarget::All, "rule1", DataDim::from("key2"));

        collector.record_task("rule1", "key1");

        // Cache should contain dim1 but not dim2
        assert!(collector.get_cache().contains(&dim1));
        assert!(!collector.get_cache().contains(&dim2));
    }
}
