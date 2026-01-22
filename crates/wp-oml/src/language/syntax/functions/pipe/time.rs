use crate::language::prelude::*;
use wp_parser::fun::fun_trait::Fun2Builder;
pub const PIPE_TIME_TO_TS: &str = "Time::to_ts";
#[derive(Clone, Debug, Default)]
pub struct TimeToTs {}

pub const PIPE_TIME_TO_TS_MS: &str = "Time::to_ts_ms";
#[derive(Clone, Debug, Default)]
pub struct TimeToTsMs {}

#[derive(Clone, Debug, Default, Display)]
#[display(style = "snake_case")]
pub enum TimeStampUnit {
    MS,
    US,
    #[default]
    SS,
}
pub const PIPE_TIME_TO_TS_US: &str = "Time::to_ts_us";
#[derive(Clone, Debug, Default)]
pub struct TimeToTsUs {}
pub const PIPE_TIME_TO_TS_ZONE: &str = "Time::to_ts_zone";
#[derive(Clone, Debug, Default, Builder)]
pub struct TimeToTsZone {
    pub(crate) unit: TimeStampUnit,
    pub(crate) zone: i32,
}
impl Display for TimeToTsZone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({},{})", Self::fun_name(), self.zone, self.unit)
    }
}
