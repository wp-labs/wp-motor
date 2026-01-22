use crate::{
    resources::{RuleKey, core::allocator::ParserResAlloc},
    sinks::SinkGroupAgent,
};
use wp_error::run_error::RunResult;

#[derive(Default)]
pub struct NullResCenter {}

impl ParserResAlloc for NullResCenter {
    fn alloc_parse_res(&self, _: &RuleKey) -> RunResult<Vec<SinkGroupAgent>> {
        Ok(vec![SinkGroupAgent::null()])
    }
}

pub struct AssignRes {
    assign: SinkGroupAgent,
}

impl AssignRes {
    pub fn use_it(assign: SinkGroupAgent) -> Self {
        Self { assign }
    }
}

impl ParserResAlloc for AssignRes {
    fn alloc_parse_res(&self, _: &RuleKey) -> RunResult<Vec<SinkGroupAgent>> {
        Ok(vec![self.assign.clone()])
    }
}
