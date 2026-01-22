use crate::language::prelude::*;
use crate::language::syntax::accessors::nested::arr::ArrOperation;
use crate::language::syntax::functions::FunOperation;
use crate::language::syntax::operations::fmt::FmtOperation;
use crate::language::syntax::operations::map::MapOperation;
use crate::language::syntax::operations::matchs::MatchOperation;
use crate::language::syntax::operations::pipe::PiPeOperation;
use crate::language::syntax::operations::record::RecordOperation;
use crate::language::syntax::operations::sql::SqlQuery;

#[derive(Default, Builder, Clone, Getters, Debug)]
#[builder(setter(into))]
pub struct SingleEvalExp {
    target: Vec<EvaluationTarget>,
    eval_way: PreciseEvaluator,
}

impl Display for SingleEvalExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first_pos = true;
        for i in self.target() {
            if first_pos {
                write!(f, "{} ", i)?;
            } else {
                write!(f, ", {} ", i)?;
            }
            first_pos = false;
        }
        write!(f, " = {} ;  ", self.eval_way)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PreciseEvaluator {
    //Query(LookupQuery),
    Sql(SqlQuery),
    Match(MatchOperation),
    Obj(DataField),
    Tdc(RecordOperation),
    Map(MapOperation),
    Pipe(PiPeOperation),
    Fun(FunOperation),
    Fmt(FmtOperation),
    Collect(ArrOperation),
    Val(Value),
}

impl Default for PreciseEvaluator {
    fn default() -> Self {
        PreciseEvaluator::Tdc(RecordOperation::default())
    }
}
impl Display for PreciseEvaluator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            //PreciseEvaluator::Query(x) => Display::fmt(x, f),
            PreciseEvaluator::Match(x) => Display::fmt(x, f),
            PreciseEvaluator::Sql(x) => Display::fmt(x, f),
            PreciseEvaluator::Obj(x) => Display::fmt(x, f),
            PreciseEvaluator::Tdc(x) => Display::fmt(x, f),
            PreciseEvaluator::Map(x) => Display::fmt(x, f),
            PreciseEvaluator::Pipe(x) => Display::fmt(x, f),
            PreciseEvaluator::Fun(x) => Display::fmt(x, f),
            PreciseEvaluator::Fmt(x) => Display::fmt(x, f),
            PreciseEvaluator::Collect(x) => Display::fmt(x, f),
            PreciseEvaluator::Val(x) => Display::fmt(x, f),
        }
    }
}

impl FieldExtractor for DataField {
    fn extract_one(
        &self,
        _target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
    ) -> Option<DataField> {
        let obj = self.clone();
        //obj.set_name(target.safe_name());
        Some(obj)
    }
}
