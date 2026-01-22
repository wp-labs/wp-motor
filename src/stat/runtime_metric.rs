use std::mem;
use wp_stat::StatReq;

use crate::stat::metric_set::MetricSet;

#[derive(Clone, Default)]
pub struct RuntimeMetrics {
    pub slice: MetricSet,
    pub total: MetricSet,
}

impl RuntimeMetrics {
    pub fn sum_up(&mut self) {
        let mut slice_stat = MetricSet::default();
        mem::swap(&mut self.slice, &mut slice_stat);
        self.total.merge(slice_stat);
    }
    pub fn registry(&mut self, reqs: Vec<StatReq>) {
        self.slice.registry(reqs.clone());
        self.total.registry(reqs);
    }
}

#[cfg(test)]
mod test {

    use wp_stat::ReportVariant;

    use crate::stat::metric_collect::MetricCollectors;

    use crate::stat::runtime_metric::RuntimeMetrics;
    use crate::types::AnyResult;
    use wp_model_core::model::{DataField, DataRecord};
    use wp_stat::StatRecorder;
    use wp_stat::StatReq;
    use wp_stat::StatTarget;

    #[test]
    fn test_top_n_stat() -> AnyResult<()> {
        let mut main_stat = RuntimeMetrics::default();
        let p = StatReq::simple_test(StatTarget::All, vec!["value".to_string()], 10);
        let mut stat = MetricCollectors::new("/".to_string(), vec![p.clone()]);
        main_stat.registry(vec![p]);
        let x = DataRecord::from(vec![DataField::from_chars("value", "a")]);
        stat.record_task("/", Some(&x));
        //let snap = stat.swap_snap();
        for mut item in stat.items {
            main_stat
                .slice
                .merge_slice(ReportVariant::Stat(item.collect_stat()));
        }
        let tdc = main_stat.slice.conv_to_tdc();
        for i in &tdc {
            println!("{}", i);
        }
        let result = &tdc[0];
        assert_eq!(
            result.field("value"),
            Some(&DataField::from_chars("value", "a"))
        );
        Ok(())
    }

    /*
    #[test]
    fn test_stat() -> AnyResult<()> {
        let mut stat = wparseMainStat::default();
        let mut x = ParseSlices::new(StatStage::Parse, "!!sys".to_string());
        x.stat.rec_in();
        x.stat.rec_in();
        x.stat.rec_suc();
        stat.slice.merge_slice(StatSlices::Parse(x));

        let code = "$name == chars(!!sys) && $hav_rate > float(90.0)";
        let (_, cond) = overall_express(code).with_origin(code)?;
        let action = Action::new(
            cond,
            NotifyLevel::Error,
            "prompt test".to_string(),
            vec![UseCase::ParsEndSum],
        );
        let mut tdc = stat.slice.conv_to_tdc();
        let real = &mut tdc[1];
        println!("{}", real);
        assert_eq!(
            action.alarm_proc(UseCase::ParsEndSum, real).level(),
            NotifyLevel::Error
        );
        Ok(())
    }
     */
}
