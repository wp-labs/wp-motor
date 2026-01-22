use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};

use comfy_table::{Cell, Table};
use wp_model_core::model::DataRecord;
use wp_stat::StatReport;
use wp_stat::{Mergeable, SliceMetrics};

use crate::stat::{MetricsCalculator, ReportGenerator, TableRowRenderer};

#[derive(Clone, Default, Debug)]
pub struct MetricAggregate<T>
where
    T: Clone + Default + Debug,
{
    pub items: BTreeMap<String, T>,
    pub sum: T,
}

/*
impl<T> ItemStat<T>
where
    T: Clone + Default + Debug,
{
    fn new() -> Self {
        Self {
            items: BTreeMap::new(),
            sum: T::default(),
        }
    }
}

 */

impl<T> From<MetricAggregate<T>> for Vec<DataRecord>
where
    T: Clone + Default + Debug,
    DataRecord: From<T>,
{
    fn from(value: MetricAggregate<T>) -> Self {
        let mut tdc = Vec::new();
        for item in value.items.values() {
            tdc.push(DataRecord::from(item.clone()));
        }
        tdc.push(DataRecord::from(value.sum));
        tdc
    }
}

impl<T> Mergeable<T> for MetricAggregate<T>
where
    T: Clone + Default + Debug + Mergeable<T> + SliceMetrics,
{
    fn merge(&mut self, other: T) {
        self.sum.merge(other.clone());
        if let Some(item) = self.items.get_mut(other.slices_key()) {
            item.merge(other);
        } else {
            self.items.insert(other.slices_key().to_string(), other);
        }
    }
}

impl<T> Mergeable<Self> for MetricAggregate<T>
where
    T: Clone + Default + Debug + Mergeable<T> + SliceMetrics,
{
    fn merge(&mut self, other: Self) {
        self.sum.merge(other.sum);
        for (name, item) in other.items {
            if let Some(self_item) = self.items.get_mut(&name) {
                self_item.merge(item);
            } else {
                self.items.insert(name, item);
            }
        }
    }
}

impl<T> Display for MetricAggregate<T>
where
    T: Clone + Default + Debug + Mergeable<T> + Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "\n------------------------------ Sink Stat -------------------------- ",
        )?;
        for v in self.items.values() {
            write!(f, "{}", v)?;
        }
        Ok(())
    }
}

impl<T> ReportGenerator for MetricAggregate<T>
where
    T: Clone + Default + Debug + Mergeable<T> + TableRowRenderer,
{
    fn generate_report(&self, fmt_table: &mut Table, op: &mut impl MetricsCalculator) {
        for v in self.items.values() {
            v.render_row(fmt_table, op);
        }
    }
}

impl ReportGenerator for StatReport {
    fn generate_report(&self, fmt_table: &mut Table, op: &mut impl MetricsCalculator) {
        let mut sort_data = Vec::new();
        for v in self.get_data().iter() {
            sort_data.push(v);
        }
        sort_data.sort();
        for v in sort_data {
            let mut data = vec![];
            data.append(&mut vec![Cell::new(v.stage.to_string())]);
            data.append(&mut vec![Cell::new(self.get_name())]);
            data.append(&mut vec![Cell::new(self.target_display())]);
            data.append(&mut vec![Cell::new(v.get_value())]);
            data.append(&mut op.calculate(&v.stat));
            fmt_table.add_row(data.to_vec());
            //v.row_write(fmt_table, op, v);
        }
    }
}
