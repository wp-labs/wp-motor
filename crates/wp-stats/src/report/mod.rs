use stat_report::StatReport;

pub mod record;
pub mod stat_report;

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ReportVariant {
    Stat(StatReport),
}
