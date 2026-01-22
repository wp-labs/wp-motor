pub const FUN_NOW_TIME: &str = "Now::time";
pub const FUN_NOW_DATE: &str = "Now::date";
pub const FUN_NOW_HOUR: &str = "Now::hour";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NowTime {}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NowDate {}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NowHour {}
