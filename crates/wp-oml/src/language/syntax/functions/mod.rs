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
    Base64Decode, Base64Encode, DEFAULT_IPV4_POOL, DEFAULT_IPV6_POOL, DEFAULT_KEY_NAME,
    DEFAULT_MASK_CHAR, DEFAULT_PLACEHOLDER, DEFAULT_SALT_NAME, DEFAULT_TOKEN_PREFIX, Dumb,
    EncodeType, Encoding, ExtractMainWord, ExtractSubjectObject, Get, Hash, HashAlgo, HtmlEscape,
    HtmlUnescape, IntranetIp, IntranetReplace, Ip4ToInt, IpToBigUint, JsonEscape, JsonUnescape,
    MapTo, MapValue, Mask, MaskKind, NameStyle, Nth, OnError, OnFail, PIPE_BASE64_DECODE,
    PIPE_BASE64_ENCODE, PIPE_EXTRACT_MAIN_WORD, PIPE_EXTRACT_SUBJECT_OBJECT, PIPE_GET, PIPE_HASH,
    PIPE_HTML_ESCAPE, PIPE_HTML_UNESCAPE, PIPE_INTRANET_IP, PIPE_INTRANET_REPLACE,
    PIPE_IP_TO_BIGUINT, PIPE_IP4_TO_INT, PIPE_JSON_ESCAPE, PIPE_JSON_UNESCAPE, PIPE_MAP_TO,
    PIPE_MASK, PIPE_NTH, PIPE_ON_FAIL, PIPE_PATH, PIPE_REMOVE, PIPE_SKIP_EMPTY, PIPE_STARTS_WITH,
    PIPE_STR_ESCAPE, PIPE_TIME_FROM_TS, PIPE_TIME_FROM_TS_MS, PIPE_TIME_FROM_TS_US,
    PIPE_TIME_TO_TS, PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON,
    PIPE_TO_STR, PIPE_TOKENIZE, PIPE_URL, PathGet, PathType, PipeFun, Remove, SkipEmpty,
    StartsWith, StrEscape, TimeFromTs, TimeFromTsMs, TimeFromTsUs, TimeStampUnit, TimeToTs,
    TimeToTsMs, TimeToTsUs, TimeToTsZone, ToJson, ToStr, TokenAlgo, Tokenize, UrlGet, UrlType,
    ip_to_biguint,
};
pub use time::*;
