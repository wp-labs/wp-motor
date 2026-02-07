use super::EvaluationTarget;
use crate::language::prelude::*;
use accessors::NestedAccessor;
use std::fmt::{Debug, Display, Formatter};

//pub mod lib_prm;

pub mod accessors;
pub mod bindings;
pub mod conditions;
pub mod evaluators;
pub mod functions;
pub mod models;
pub mod operations;
pub mod traits;
pub use traits::VarAccess;
#[derive(Builder, Debug, Clone, Getters)]
#[builder(setter(into))]
pub struct NestedBinding {
    target: EvaluationTarget,
    acquirer: NestedAccessor,
}
impl NestedBinding {
    pub fn new(target: EvaluationTarget, get_way: NestedAccessor) -> Self {
        Self {
            target,
            acquirer: get_way,
        }
    }

    pub fn acquirer_mut(&mut self) -> &mut NestedAccessor {
        &mut self.acquirer
    }
}

impl Display for NestedBinding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {} ; ", self.target, self.acquirer)
    }
}

pub enum OmlKwGet {
    Take,
    Read,
}
