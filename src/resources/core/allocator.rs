use wp_error::RunResult;

use crate::sinks::SinkGroupAgent;

use super::types::RuleKey;

pub trait ParserResAlloc {
    fn alloc_parse_res(&self, rule_key: &RuleKey) -> RunResult<Vec<SinkGroupAgent>>;
}
