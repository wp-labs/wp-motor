pub mod diagnostics;
mod error;
mod evaluator;
mod model;
mod prelude;
pub use error::OMLRunError;
pub use error::OMLRunReason;
pub use error::OMLRunResult;
pub use model::DataRecordRef;

use crate::language::EvaluationTarget;
use crate::language::PreciseEvaluator;
pub use evaluator::ConfADMExt;
pub use evaluator::DataTransformer;
pub use evaluator::traits::BatchFetcher;
pub use evaluator::traits::ExpEvaluator;
pub use evaluator::traits::FieldCollector;
pub use evaluator::traits::ValueProcessor;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};

pub trait FieldExtractor {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField>;

    #[allow(unused_variables)]
    fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }
    fn support_batch(&self) -> bool {
        false
    }
}
impl FieldExtractor for PreciseEvaluator {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            //PreciseEvaluator::Query(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Sql(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Match(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Obj(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Tdc(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Map(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Pipe(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Fun(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Fmt(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Collect(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Val(o) => o.extract_one(target, src, dst),
        }
    }

    fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            PreciseEvaluator::Sql(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Match(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Obj(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Tdc(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Map(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Pipe(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Fun(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Fmt(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Collect(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Val(o) => o.extract_more(src, dst, cache),
        }
    }

    fn support_batch(&self) -> bool {
        match self {
            PreciseEvaluator::Sql(o) => o.support_batch(),
            PreciseEvaluator::Match(o) => o.support_batch(),
            PreciseEvaluator::Obj(o) => o.support_batch(),
            PreciseEvaluator::Tdc(o) => o.support_batch(),
            PreciseEvaluator::Map(o) => o.support_batch(),
            PreciseEvaluator::Pipe(o) => o.support_batch(),
            PreciseEvaluator::Fun(o) => o.support_batch(),
            PreciseEvaluator::Fmt(o) => o.support_batch(),
            PreciseEvaluator::Collect(o) => o.support_batch(),
            PreciseEvaluator::Val(o) => o.support_batch(),
        }
    }
}
