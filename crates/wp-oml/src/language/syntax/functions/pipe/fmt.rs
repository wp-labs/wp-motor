use crate::language::prelude::*;
pub const PIPE_TO_JSON: &str = "to_json";
#[derive(Default, Builder, Debug, Clone, Getters, Serialize, Deserialize)]
pub struct ToJson {}
pub const PIPE_JSON_ESCAPE: &str = "json_escape";
#[derive(Clone, Debug, Default)]
pub struct JsonEscape {}

pub const PIPE_JSON_UNESCAPE: &str = "json_unescape";
#[derive(Clone, Debug, Default)]
pub struct JsonUnescape {}
