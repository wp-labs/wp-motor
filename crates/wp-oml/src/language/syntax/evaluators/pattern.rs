use crate::language::BatchEvalTarget;
use crate::language::prelude::*;
use crate::language::syntax::operations::record::RecordOperation;

#[derive(Builder, Debug, Clone, Getters)]
#[builder(setter(into))]
pub struct BatchEvalExp {
    target: BatchEvalTarget,
    eval_way: BatchEvaluation,
}
#[derive(Debug, Clone)]
pub enum BatchEvaluation {
    Get(RecordOperation),
}

impl BatchEvalExp {
    pub fn new(name: String, meta: DataType) -> Self {
        let target = BatchEvalTarget::new(EvaluationTarget::new(name, meta));
        Self {
            target,
            eval_way: BatchEvaluation::Get(RecordOperation::default()),
        }
    }
}
impl Display for BatchEvalExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {} ; ", self.target, self.eval_way)
    }
}
impl Display for BatchEvaluation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchEvaluation::Get(x) => Display::fmt(x, f),
        }
    }
}
