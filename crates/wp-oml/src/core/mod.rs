pub mod diagnostics;
mod error;
pub mod evaluator; // 公开 evaluator 模块
mod model;
mod prelude;
pub use error::OMLRunError;
pub use error::OMLRunReason;
pub use error::OMLRunResult;
pub use model::DataRecordRef;

use crate::language::EvaluationTarget;
use crate::language::PreciseEvaluator;
use async_trait::async_trait;
pub use evaluator::traits::AsyncDataTransformer;
pub use evaluator::traits::AsyncExpEvaluator;
pub use evaluator::traits::BatchFetcher;
pub use evaluator::traits::ConfADMExt;
pub use evaluator::traits::FieldCollector;
pub use evaluator::traits::ValueProcessor;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

#[async_trait]
pub trait AsyncFieldExtractor {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField>;

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one_async(target, src, dst)
            .await
            .map(FieldStorage::from_owned)
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        let _ = (src, dst, cache);
        Vec::new()
    }

    fn support_batch_async(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for PreciseEvaluator {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            PreciseEvaluator::Sql(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Match(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Lookup(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Tdc(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Map(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Pipe(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Fun(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Collect(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Val(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Obj(o) => o.extract_one_async(target, src, dst).await,
            PreciseEvaluator::Calc(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::AccessDirect(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::Fmt(o) => o.extract_one(target, src, dst),
            PreciseEvaluator::ObjArc(arc) => arc.as_ref().extract_one_async(target, src, dst).await,
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            PreciseEvaluator::Sql(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Match(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Lookup(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Tdc(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Map(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Pipe(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Fun(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Collect(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Val(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Obj(o) => o.extract_storage_async(target, src, dst).await,
            PreciseEvaluator::Calc(o) => o.extract_storage(target, src, dst),
            PreciseEvaluator::AccessDirect(o) => o.extract_storage(target, src, dst),
            PreciseEvaluator::Fmt(o) => o.extract_storage(target, src, dst),
            PreciseEvaluator::ObjArc(arc) => {
                arc.as_ref().extract_storage_async(target, src, dst).await
            }
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            PreciseEvaluator::Sql(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Match(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Lookup(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Tdc(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Map(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Pipe(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Fun(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Collect(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Val(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Obj(o) => o.extract_more_async(src, dst, cache).await,
            PreciseEvaluator::Calc(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::AccessDirect(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::Fmt(o) => o.extract_more(src, dst, cache),
            PreciseEvaluator::ObjArc(o) => o.as_ref().extract_more_async(src, dst, cache).await,
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    fn support_batch_async(&self) -> bool {
        match self {
            PreciseEvaluator::Sql(o) => o.support_batch_async(),
            PreciseEvaluator::Match(o) => o.support_batch_async(),
            PreciseEvaluator::Lookup(o) => o.support_batch_async(),
            PreciseEvaluator::Tdc(o) => o.support_batch_async(),
            PreciseEvaluator::Map(o) => o.support_batch_async(),
            PreciseEvaluator::Pipe(o) => o.support_batch_async(),
            PreciseEvaluator::Fun(o) => o.support_batch_async(),
            PreciseEvaluator::Collect(o) => o.support_batch_async(),
            PreciseEvaluator::Val(o) => o.support_batch_async(),
            PreciseEvaluator::Obj(o) => o.support_batch_async(),
            PreciseEvaluator::Calc(o) => o.support_batch(),
            PreciseEvaluator::AccessDirect(o) => o.support_batch(),
            PreciseEvaluator::Fmt(o) => o.support_batch(),
            PreciseEvaluator::ObjArc(o) => o.as_ref().support_batch_async(),
            PreciseEvaluator::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
}
