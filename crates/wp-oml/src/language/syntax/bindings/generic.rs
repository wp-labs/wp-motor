use crate::language::prelude::*;

#[derive(Builder, Debug, Clone, Getters, PartialEq)]
#[builder(setter(into))]
pub struct GenericBinding {
    target: EvaluationTarget,
    accessor: GenericAccessor,
}
impl GenericBinding {
    pub fn new(target: EvaluationTarget, acquirer: GenericAccessor) -> Self {
        Self {
            target,
            accessor: acquirer,
        }
    }
}

impl Display for GenericBinding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {} ; ", self.target, self.accessor)
    }
}
