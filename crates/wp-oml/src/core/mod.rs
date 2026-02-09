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
use std::sync::Arc;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

pub trait FieldExtractor {
    /// Extract field as owned DataField
    ///
    /// This is the base method that all implementations must provide.
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField>;

    /// Extract field as FieldStorage (Shared or Owned variant)
    ///
    /// Default implementation wraps extract_one result in FieldStorage::Owned.
    /// Override this for zero-copy behavior (return Shared variant for Arc-based fields).
    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst).map(FieldStorage::Owned)
    }

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
            PreciseEvaluator::ObjArc(arc) => arc.as_ref().extract_one(target, src, dst),
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            // Static symbol reference: return Shared variant (zero-copy)
            PreciseEvaluator::ObjArc(arc) => arc
                .as_ref()
                .extract_one(target, src, dst)
                .map(|_| FieldStorage::Shared(Arc::clone(arc))),

            // Regular fields: delegate to default implementation (calls extract_one and wraps in Owned)
            _ => self.extract_one(target, src, dst).map(FieldStorage::Owned),
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
            PreciseEvaluator::ObjArc(o) => o.as_ref().extract_more(src, dst, cache),
            PreciseEvaluator::Tdc(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Map(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Pipe(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Fun(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Fmt(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Collect(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Val(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    fn support_batch(&self) -> bool {
        match self {
            PreciseEvaluator::Sql(o) => o.support_batch(),
            PreciseEvaluator::Match(o) => o.support_batch(),
            PreciseEvaluator::Obj(o) => o.support_batch(),
            PreciseEvaluator::ObjArc(o) => o.as_ref().support_batch(),
            PreciseEvaluator::Tdc(o) => o.support_batch(),
            PreciseEvaluator::Map(o) => o.support_batch(),
            PreciseEvaluator::Pipe(o) => o.support_batch(),
            PreciseEvaluator::Fun(o) => o.support_batch(),
            PreciseEvaluator::Fmt(o) => o.support_batch(),
            PreciseEvaluator::Collect(o) => o.support_batch(),
            PreciseEvaluator::Val(o) => o.support_batch(),
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
}
