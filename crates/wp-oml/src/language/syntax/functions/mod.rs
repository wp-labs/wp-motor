pub mod pipe;
pub mod time;
use std::fmt::{Display, Formatter};

use derive_getters::Getters;
#[derive(strum_macros::Display, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BuiltinFunction {
    #[strum(to_string = "Now::time")]
    NowTime(NowTime),
    #[strum(to_string = "Now::date")]
    NowDate(NowDate),
    #[strum(to_string = "Now::hour")]
    NowHour(NowHour),
}

#[derive(Debug, Clone, Getters, Serialize, Deserialize, PartialEq)]
pub struct FunOperation {
    fun: BuiltinFunction,
}
impl FunOperation {
    pub fn new(fun: BuiltinFunction) -> Self {
        Self { fun }
    }
}
impl Display for FunOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}() ", self.fun)
    }
}

pub use pipe::{
    Base64Decode, Base64Encode, Dumb, EncodeType, Get, HtmlEscape, HtmlUnescape, Ip4ToInt,
    JsonEscape, JsonUnescape, Nth, PIPE_BASE64_DECODE, PIPE_BASE64_ENCODE, PIPE_GET,
    PIPE_HTML_ESCAPE, PIPE_HTML_UNESCAPE, PIPE_IP4_TO_INT, PIPE_JSON_ESCAPE, PIPE_JSON_UNESCAPE,
    PIPE_NTH, PIPE_PATH, PIPE_SKIP_EMPTY, PIPE_STR_ESCAPE, PIPE_TIME_TO_TS, PIPE_TIME_TO_TS_MS,
    PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON, PIPE_TO_STR, PIPE_URL, PathGet,
    PathType, PipeFun, SkipEmpty, StrEscape, TimeStampUnit, TimeToTs, TimeToTsMs, TimeToTsUs,
    TimeToTsZone, ToJson, ToStr, UrlGet, UrlType,
};
pub use time::*;
