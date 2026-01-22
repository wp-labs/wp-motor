use crate::language::prelude::*;

pub mod base64;
pub mod escape;
pub mod fmt;
pub mod net;
pub mod other;
pub mod time;
pub use base64::*;
pub use escape::*;
pub use fmt::*;
pub use net::*;
pub use other::*;
pub use time::*;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PipeFun {
    Base64Encode(Base64Encode),
    Base64Decode(Base64Decode),
    HtmlEscape(HtmlEscape),
    HtmlUnescape(HtmlUnescape),
    StrEscape(StrEscape),
    JsonEscape(JsonEscape),
    JsonUnescape(JsonUnescape),
    TimeToTs(TimeToTs),
    TimeToTsMs(TimeToTsMs),
    TimeToTsUs(TimeToTsUs),
    TimeToTsZone(TimeToTsZone),
    Nth(Nth),
    Get(Get),
    ToStr(ToStr),
    ToJson(ToJson),
    SkipEmpty(SkipEmpty),
    Dumb(Dumb),
    PathGet(PathGet),
    UrlGet(UrlGet),
    Ip4ToInt(Ip4ToInt),
}

impl Display for PipeFun {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PipeFun::Base64Encode(_) => write!(f, "{}", PIPE_BASE64_ENCODE),
            PipeFun::Base64Decode(v) => write!(f, "{}", v),
            PipeFun::HtmlEscape(_) => write!(f, "{}", PIPE_HTML_ESCAPE),
            PipeFun::StrEscape(_) => write!(f, "{}", PIPE_STR_ESCAPE),
            PipeFun::JsonEscape(_) => write!(f, "{}", PIPE_JSON_ESCAPE),
            PipeFun::JsonUnescape(_) => write!(f, "{}", PIPE_JSON_UNESCAPE),
            PipeFun::HtmlUnescape(_) => write!(f, "{}", PIPE_HTML_UNESCAPE),
            PipeFun::TimeToTs(_) => write!(f, "{}", PIPE_TIME_TO_TS),
            PipeFun::TimeToTsMs(_) => write!(f, "{}", PIPE_TIME_TO_TS_MS),
            PipeFun::TimeToTsUs(_) => write!(f, "{}", PIPE_TIME_TO_TS_US),
            PipeFun::TimeToTsZone(v) => write!(f, "{}", v),
            PipeFun::Nth(v) => write!(f, "{}", v),
            PipeFun::Get(v) => write!(f, "{}", v),
            PipeFun::ToJson(_) => write!(f, "{}", PIPE_TO_JSON),
            PipeFun::ToStr(_) => write!(f, "{}", PIPE_TO_STR),
            PipeFun::SkipEmpty(_) => write!(f, "{}", PIPE_SKIP_EMPTY),
            PipeFun::Dumb(_) => write!(f, "{}", PIPE_TO_STR),
            PipeFun::PathGet(v) => write!(f, "{}", v),
            PipeFun::UrlGet(v) => write!(f, "{}", v),
            PipeFun::Ip4ToInt(v) => write!(f, "{}", v),
        }
    }
}

impl Default for PipeFun {
    fn default() -> Self {
        PipeFun::Dumb(Dumb::default())
    }
}
