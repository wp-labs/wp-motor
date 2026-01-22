use std::fmt::{Display, Formatter};

pub use pattern::{BatchEvalExp, BatchEvalExpBuilder, BatchEvaluation};
pub use precise::{PreciseEvaluator, SingleEvalExp, SingleEvalExpBuilder};

pub mod pattern;
pub mod precise;
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum EvalExp {
    Single(SingleEvalExp),
    Batch(BatchEvalExp),
}
impl Display for EvalExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalExp::Single(x) => Display::fmt(x, f),
            EvalExp::Batch(x) => Display::fmt(x, f),
        }
    }
}
