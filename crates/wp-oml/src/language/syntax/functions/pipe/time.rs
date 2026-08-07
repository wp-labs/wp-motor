use crate::language::prelude::*;
use wp_primitives::fun::fun_trait::Fun2Builder;
pub const PIPE_TIME_TO_TS: &str = "Time::to_ts";
#[derive(Clone, Debug, Default)]
pub struct TimeToTs {}

pub const PIPE_TIME_TO_TS_MS: &str = "Time::to_ts_ms";
#[derive(Clone, Debug, Default)]
pub struct TimeToTsMs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeToTsMs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.zone {
            Some(z) => write!(f, "{}({})", PIPE_TIME_TO_TS_MS, z),
            None => write!(f, "{}", PIPE_TIME_TO_TS_MS),
        }
    }
}

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

pub const PIPE_TIME_FROM_TS_MS: &str = "Time::from_ts_ms";
#[derive(Clone, Debug, Default)]
pub struct TimeFromTsMs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeFromTsMs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.zone {
            Some(z) => write!(f, "{}({})", PIPE_TIME_FROM_TS_MS, z),
            None => write!(f, "{}", PIPE_TIME_FROM_TS_MS),
        }
    }
}
