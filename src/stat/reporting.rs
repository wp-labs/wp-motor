use crate::stat::MetricsCalculator;
use comfy_table::{Cell, Table};
use wp_stat::MeasureUnit;

#[derive(Clone, Debug, Default)]
pub struct ReportEngine {}

impl ReportEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl MetricsCalculator for ReportEngine {
    fn calculate(&self, data: &MeasureUnit) -> Vec<Cell> {
        vec![
            Cell::new(format!("{}", data.total)),
            Cell::new(format!("{}", data.success)),
            Cell::new(format!("{:3.1}%", data.suc_rate())),
            Cell::new(format!("{:3.2}", data.speed() / 10000.0)),
        ]
    }
    fn update_state(&mut self, _val_x: Option<f64>, _val_y: Option<f64>) {}
}

/*
impl From<ParseStat> for Vec<TDOEnum> {
    fn from(value: ParseStat) -> Self {
        let mut tdc = Vec::new();
        tdc.append(&mut Vec::from(value.total));
        tdc
    }
}

 */

pub fn create_report_table() -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        "stage", "name", "target", "collect", "total", "success", "suc-rate", "speed",
    ]);
    table
}
