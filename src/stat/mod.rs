use comfy_table::Table;
pub use tokio::sync::mpsc::Sender;

pub use alert_system::{AlertRule, AlertSeverity, MonitPhase, MonitorParser};
use wp_stat::MeasureUnit;
use wp_stat::ReportVariant;
//pub use wp_model_core::adm::stat::LoadStat;
pub use reporting::ReportEngine;

mod alert_system;
mod metric_aggregat;
pub mod metric_collect;
pub mod metric_set;
pub mod reporting;
pub mod runtime_metric;
//pub mod sink_stat;

pub type MonSend = tokio::sync::mpsc::Sender<ReportVariant>;
pub type MonRecv = tokio::sync::mpsc::Receiver<ReportVariant>;

pub trait ReportGenerator {
    fn generate_report(&self, fmt_table: &mut Table, op: &mut impl MetricsCalculator);
}

pub trait TableRowRenderer {
    fn render_row(&self, fmt_table: &mut Table, op: &mut impl MetricsCalculator);
}

pub trait MetricsCalculator {
    fn calculate(&self, data: &MeasureUnit) -> Vec<comfy_table::Cell>;
    fn update_state(&mut self, val_x: Option<f64>, val_y: Option<f64>);
}

pub const STAT_INTERVAL_MS: usize = 100;
