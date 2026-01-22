use crate::core::ValueProcessor;
use crate::language::prelude::*;
use strum_macros::EnumString;

use wp_parser::fun::fun_trait::Fun1Builder;

pub const PIPE_TO_STR: &str = "to_str";
#[derive(Default, Builder, Debug, Clone, Getters, Serialize, Deserialize)]
pub struct ToStr {}

pub const PIPE_NTH: &str = "nth";
#[derive(Clone, Debug, Default, Builder)]
pub struct Nth {
    pub(crate) index: usize,
}
impl Display for Nth {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", Self::fun_name(), self.index)
    }
}

pub const PIPE_SKIP_EMPTY: &str = "skip_empty";
#[derive(Clone, Debug, Default)]
pub struct SkipEmpty {}

pub const PIPE_GET: &str = "get";
#[derive(Clone, Debug, Default)]
pub struct Get {
    pub(crate) name: String,
}

impl Display for Get {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", Self::fun_name(), self.name)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, EnumString, strum_macros::Display)]
pub enum PathType {
    #[default]
    Default,
    #[strum(serialize = "name")]
    FileName,
    #[strum(serialize = "path")]
    Path,
}
pub const PIPE_PATH: &str = "path";
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PathGet {
    pub key: PathType,
}

impl Display for PathGet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", PIPE_PATH, self.key)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, EnumString, strum_macros::Display)]
pub enum UrlType {
    #[default]
    Default,
    /// 获取域名部分
    #[strum(serialize = "domain")]
    Domain,
    /// 获取完整的 HTTP 请求主机（包含端口）
    #[strum(serialize = "host")]
    HttpReqHost,
    /// 获取 HTTP 请求 URI（包含路径和查询参数）
    #[strum(serialize = "uri")]
    HttpReqUri,
    /// 获取 HTTP 请求路径
    #[strum(serialize = "path")]
    HttpReqPath,
    /// 获取 HTTP 请求查询参数
    #[strum(serialize = "params")]
    HttpReqParams,
}

pub const PIPE_URL: &str = "url";
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct UrlGet {
    pub key: UrlType,
}

impl Display for UrlGet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", PIPE_URL, self.key)
    }
}

#[derive(Default, Builder, Debug, Clone, Getters, Serialize, Deserialize)]
pub struct Dumb {}
impl Display for Dumb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PIPE_TO_STR)
    }
}
impl ValueProcessor for Dumb {
    fn value_cacu(&self, _in_val: DataField) -> DataField {
        todo!()
    }
}
