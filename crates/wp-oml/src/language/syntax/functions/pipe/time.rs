use crate::language::prelude::*;
use wp_primitives::fun::fun_trait::Fun2Builder;

pub const PIPE_TIME_TO_TS: &str = "Time::to_ts";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeToTs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeToTs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_optional_zone(f, PIPE_TIME_TO_TS, &self.zone)
    }
}

/// 可选 zone 的 Display：`Time::to_ts_ms` / `Time::to_ts_ms(8)`
fn fmt_optional_zone(f: &mut Formatter<'_>, fun: &str, zone: &Option<i32>) -> std::fmt::Result {
    match zone {
        Some(z) => write!(f, "{}({})", fun, z),
        None => write!(f, "{}", fun),
    }
}

pub const PIPE_TIME_TO_TS_MS: &str = "Time::to_ts_ms";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeToTsMs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeToTsMs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_optional_zone(f, PIPE_TIME_TO_TS_MS, &self.zone)
    }
}

#[derive(Clone, Debug, Default, Display, Serialize, Deserialize)]
#[display(style = "snake_case")]
pub enum TimeStampUnit {
    MS,
    US,
    #[default]
    SS,
}
pub const PIPE_TIME_TO_TS_US: &str = "Time::to_ts_us";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeToTsUs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeToTsUs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_optional_zone(f, PIPE_TIME_TO_TS_US, &self.zone)
    }
}
pub const PIPE_TIME_TO_TS_ZONE: &str = "Time::to_ts_zone";
#[derive(Clone, Debug, Default, Builder, Serialize, Deserialize)]
pub struct TimeToTsZone {
    pub(crate) unit: TimeStampUnit,
    pub(crate) zone: i32,
}
impl Display for TimeToTsZone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({},{})", Self::fun_name(), self.zone, self.unit)
    }
}

pub const PIPE_TIME_FROM_TS: &str = "Time::from_ts";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeFromTs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeFromTs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_optional_zone(f, PIPE_TIME_FROM_TS, &self.zone)
    }
}

pub const PIPE_TIME_FROM_TS_MS: &str = "Time::from_ts_ms";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeFromTsMs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeFromTsMs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_optional_zone(f, PIPE_TIME_FROM_TS_MS, &self.zone)
    }
}

pub const PIPE_TIME_FROM_TS_US: &str = "Time::from_ts_us";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeFromTsUs {
    pub(crate) zone: Option<i32>,
}
impl Display for TimeFromTsUs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_optional_zone(f, PIPE_TIME_FROM_TS_US, &self.zone)
    }
}
