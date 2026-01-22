use wp_model_core::model::DataField;

use crate::{core::ValueProcessor, language::PipeFun};

mod base64;
mod escape;
mod net;
pub mod other;
mod time;

impl ValueProcessor for PipeFun {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match self {
            PipeFun::Base64Encode(o) => o.value_cacu(in_val),
            PipeFun::Base64Decode(o) => o.value_cacu(in_val),
            PipeFun::HtmlEscape(o) => o.value_cacu(in_val),
            PipeFun::HtmlUnescape(o) => o.value_cacu(in_val),
            PipeFun::StrEscape(o) => o.value_cacu(in_val),
            PipeFun::JsonEscape(o) => o.value_cacu(in_val),
            PipeFun::JsonUnescape(o) => o.value_cacu(in_val),
            PipeFun::TimeToTs(o) => o.value_cacu(in_val),
            PipeFun::TimeToTsMs(o) => o.value_cacu(in_val),
            PipeFun::TimeToTsUs(o) => o.value_cacu(in_val),
            PipeFun::TimeToTsZone(o) => o.value_cacu(in_val),
            PipeFun::Nth(o) => o.value_cacu(in_val),
            PipeFun::Get(o) => o.value_cacu(in_val),
            PipeFun::ToStr(o) => o.value_cacu(in_val),
            PipeFun::ToJson(o) => o.value_cacu(in_val),
            PipeFun::SkipEmpty(o) => o.value_cacu(in_val),
            PipeFun::Dumb(o) => o.value_cacu(in_val),
            PipeFun::PathGet(o) => o.value_cacu(in_val),
            PipeFun::UrlGet(o) => o.value_cacu(in_val),
            PipeFun::Ip4ToInt(o) => o.value_cacu(in_val),
        }
    }
}
