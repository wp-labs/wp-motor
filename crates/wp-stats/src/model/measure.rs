use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::time::Instant;

use chrono::{Local, NaiveDateTime};

use crate::traits::SliceMetrics;
use wp_model_core::model::{DataField, DateTimeValue};

thread_local! {
    static FAST_NOW_CACHE: RefCell<FastNow> = RefCell::new(FastNow::new());
}

struct FastNow;

impl FastNow {
    fn new() -> Self {
        Self
    }

    fn get(&mut self) -> NaiveDateTime {
        Local::now().naive_local()
    }
}

fn fast_local_now() -> NaiveDateTime {
    FAST_NOW_CACHE.with(|cache| cache.borrow_mut().get())
}

#[derive(Clone, Debug)]
pub struct TimedStat {
    beg: DateTimeValue,
    end: DateTimeValue,
    last_refresh: Instant,
    initialized: bool,
}

impl Default for TimedStat {
    fn default() -> Self {
        Self {
            beg: fast_local_now(),
            end: fast_local_now(),
            last_refresh: Instant::now(),
            initialized: true,
        }
    }
}

impl TimedStat {
    pub fn new() -> Self {
        Self::now()
    }
    pub fn now() -> Self {
        Self {
            beg: fast_local_now(),
            end: fast_local_now(),
            last_refresh: Instant::now(),
            initialized: true,
        }
    }
    pub fn reset_now(&mut self) {
        self.beg = fast_local_now();
        self.end = fast_local_now();
        self.last_refresh = Instant::now();
        self.initialized = true;
    }
    pub fn over_reset_timed(&mut self, secs: usize) -> bool {
        self.over_reset_timed_millis(secs as u128 * 1000)
    }
    pub fn use_secs(&self) -> f64 {
        let use_it = self.end - self.beg;
        use_it.num_seconds() as f64
    }
    pub fn use_millis(&self) -> i64 {
        let use_it = self.end - self.beg;
        use_it.num_milliseconds()
    }
    pub fn max_merge(&mut self, other: &Self) {
        if self.beg > other.beg {
            self.beg = other.beg;
        }
        if self.end < other.end {
            self.end = other.end;
        }
    }
    pub fn beg_time(&self) -> DateTimeValue {
        self.beg
    }
    pub fn stat_end(&mut self) {
        self.end = fast_local_now();
    }
    pub fn stat_end_at(&mut self, time: DateTimeValue) {
        self.end = time;
    }
    pub fn over_reset_timed_millis(&mut self, millis: u128) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_refresh).as_millis() >= millis {
            let cur_time = fast_local_now();
            if !self.initialized {
                self.beg = cur_time;
                self.end = cur_time;
                self.initialized = true;
            } else {
                self.beg = cur_time;
                self.end = cur_time;
            }
            self.last_refresh = now;
            return true;
        }
        false
    }
    pub fn over_default_timed(&mut self) -> bool {
        self.over_reset_timed_millis(100)
    }
}

#[derive(Clone, Debug, Default, Copy, PartialEq)]
pub enum Importance {
    High = 3,
    Low = 1,
    #[default]
    Normal = 2,
}

impl From<u32> for Importance {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::Low,
            2 => Self::Normal,
            3 => Self::High,
            _ => {
                unreachable!("importance value is invalid")
            }
        }
    }
}

impl Importance {
    pub fn up(&mut self) {
        let mut v = *self as u32;
        v += 1;
        *self = Self::from(v);
    }
    pub fn down(&mut self) {
        let mut v = *self as u32;
        v -= 1;
        *self = Self::from(v);
    }
}

#[derive(Clone, Debug)]
pub struct MeasureUnit {
    pub timer: TimedStat,
    pub total: usize,
    pub success: usize,
}

impl MeasureUnit {
    pub fn new() -> Self {
        Self {
            timer: TimedStat::new(),
            total: 0,
            success: 0,
        }
    }

    pub fn suc_cnt(&self) -> usize {
        self.success
    }

    pub fn speed(&self) -> f64 {
        let use_time = self.timer.use_millis();
        //min time is 0.1s
        (self.total as f64 / (use_time + 1) as f64) * 1000.0
    }
    pub fn suc_rate(&self) -> f64 {
        let suc = self.success as f64;
        if self.total == 0 {
            return 0.0;
        }
        let total = self.total as f64;

        suc / total * 100.0
    }
    pub fn to_tdc(&self) -> Vec<DataField> {
        vec![
            DataField::from_digit("total", self.total as i64),
            DataField::from_digit("success", self.success as i64),
            DataField::from_float("suc_rate", self.suc_rate()),
            DataField::from_float("speed", self.speed() / 10000.0),
            DataField::from_time("beg_time", self.timer.beg),
            DataField::from_time("end_time", self.timer.end),
        ]
    }
}

impl From<&MeasureUnit> for Vec<DataField> {
    fn from(v: &MeasureUnit) -> Self {
        v.to_tdc()
    }
}

impl SliceMetrics for MeasureUnit {
    fn slices_key(&self) -> &str {
        todo!()
    }

    fn add(&mut self, other: &Self) {
        self.timer.max_merge(&other.timer);
        self.total += other.total;
        self.success += other.success;
    }

    fn rec_in(&mut self) {
        self.total += 1;
    }
    fn rec_suc(&mut self) {
        self.success += 1;
        //trace!(target: "stat","rulesuc! {}", self.success );
    }
    fn rec_end(&mut self) {}

    fn get_total(&self) -> u64 {
        self.total as u64
    }
}

impl Display for MeasureUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A:{:10}| Y:{:10} | {:3.1}%",
            self.total,
            self.success,
            self.suc_rate()
        )?;
        Ok(())
    }
}

impl Default for MeasureUnit {
    fn default() -> Self {
        Self::new()
    }
}

impl MeasureUnit {
    pub fn finalize_end(&mut self, ts: DateTimeValue) {
        self.timer.stat_end_at(ts);
    }
}

impl MeasureUnit {
    /// Batch increment for total and success counters.
    /// Used by higher-level collectors to record N successful tasks at once.
    pub fn rec_beg_end_n(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        self.total = self.total.saturating_add(n);
        self.success = self.success.saturating_add(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_measure_unit_new() {
        let unit = MeasureUnit::new();
        assert_eq!(unit.total, 0);
        assert_eq!(unit.success, 0);
    }

    #[test]
    fn test_rec_in() {
        let mut unit = MeasureUnit::new();
        unit.rec_in();
        unit.rec_in();

        assert_eq!(unit.total, 2);
        assert_eq!(unit.success, 0);
    }

    #[test]
    fn test_rec_suc() {
        let mut unit = MeasureUnit::new();
        unit.rec_suc();
        unit.rec_suc();
        unit.rec_suc();

        assert_eq!(unit.total, 0);
        assert_eq!(unit.success, 3);
    }

    #[test]
    fn test_rec_in_and_suc() {
        let mut unit = MeasureUnit::new();
        unit.rec_in();
        unit.rec_in();
        unit.rec_suc();

        assert_eq!(unit.total, 2);
        assert_eq!(unit.success, 1);
    }

    #[test]
    fn test_suc_rate_zero_total() {
        let unit = MeasureUnit::new();
        assert_eq!(unit.suc_rate(), 0.0);
    }

    #[test]
    fn test_suc_rate_normal() {
        let mut unit = MeasureUnit::new();
        unit.total = 10;
        unit.success = 7;

        assert_eq!(unit.suc_rate(), 70.0);
    }

    #[test]
    fn test_suc_rate_100_percent() {
        let mut unit = MeasureUnit::new();
        unit.total = 5;
        unit.success = 5;

        assert_eq!(unit.suc_rate(), 100.0);
    }

    #[test]
    fn test_speed_calculation() {
        let mut unit = MeasureUnit::new();
        unit.total = 1000;

        let speed = unit.speed();
        // Speed should be calculated based on time elapsed
        assert!(speed > 0.0);
    }

    #[test]
    fn test_add_combines_metrics() {
        let mut unit1 = MeasureUnit::new();
        unit1.total = 10;
        unit1.success = 7;

        let mut unit2 = MeasureUnit::new();
        unit2.total = 5;
        unit2.success = 3;

        unit1.add(&unit2);

        assert_eq!(unit1.total, 15);
        assert_eq!(unit1.success, 10);
    }

    #[test]
    fn test_get_total() {
        let mut unit = MeasureUnit::new();
        unit.total = 42;

        assert_eq!(unit.get_total(), 42);
    }

    #[test]
    fn test_suc_cnt() {
        let mut unit = MeasureUnit::new();
        unit.success = 25;

        assert_eq!(unit.suc_cnt(), 25);
    }

    #[test]
    fn test_to_tdc() {
        let mut unit = MeasureUnit::new();
        unit.total = 100;
        unit.success = 80;

        let fields = unit.to_tdc();
        assert_eq!(fields.len(), 6); // total, success, suc_rate, speed, beg_time, end_time
    }

    #[test]
    fn test_timed_stat_new() {
        let timed = TimedStat::new();
        assert!(timed.use_millis() >= 0);
    }

    #[test]
    fn test_timed_stat_use_secs() {
        let mut timed = TimedStat::new();
        sleep(Duration::from_millis(100));
        timed.stat_end();

        let secs = timed.use_secs();
        assert!(secs >= 0.0);
    }

    #[test]
    fn test_timed_stat_max_merge() {
        let mut timed1 = TimedStat::new();
        sleep(Duration::from_millis(50));

        let mut timed2 = TimedStat::new();
        sleep(Duration::from_millis(50));
        timed2.stat_end();

        timed1.max_merge(&timed2);

        // After merge, beg should be the earliest and end should be the latest
        assert!(timed1.beg_time() <= timed2.beg_time());
    }

    #[test]
    fn test_timed_stat_reset_now() {
        let mut timed = TimedStat::new();
        let first_beg = timed.beg_time();

        sleep(Duration::from_millis(10));
        timed.reset_now();

        assert!(timed.beg_time() >= first_beg);
    }

    #[test]
    fn test_importance_default() {
        let imp: Importance = Default::default();
        assert_eq!(imp, Importance::Normal);
    }

    #[test]
    fn test_importance_from_u32() {
        assert_eq!(Importance::from(1), Importance::Low);
        assert_eq!(Importance::from(2), Importance::Normal);
        assert_eq!(Importance::from(3), Importance::High);
    }

    #[test]
    fn test_importance_up() {
        let mut imp = Importance::Low;
        imp.up();
        assert_eq!(imp, Importance::Normal);

        imp.up();
        assert_eq!(imp, Importance::High);
    }

    #[test]
    fn test_importance_down() {
        let mut imp = Importance::High;
        imp.down();
        assert_eq!(imp, Importance::Normal);

        imp.down();
        assert_eq!(imp, Importance::Low);
    }

    #[test]
    fn test_measure_unit_display() {
        let mut unit = MeasureUnit::new();
        unit.total = 100;
        unit.success = 75;

        let display = format!("{}", unit);
        assert!(display.contains("100"));
        assert!(display.contains("75"));
        assert!(display.contains("75.0%"));
    }

    #[test]
    fn test_over_reset_timed() {
        let mut timed = TimedStat::new();

        // Should not reset immediately
        assert!(!timed.over_reset_timed(10));

        // Wait and check again
        sleep(Duration::from_secs(1));
        // Still might not reset if less than threshold
        timed.over_reset_timed_millis(100);
    }
}
