use crate::core::AsyncFieldExtractor;
use crate::core::evaluator::traits::AsyncExpEvaluator;
use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;
use crate::language::GenericAccessor;
use crate::language::{GenericBinding, NestedBinding, SingleEvalExp};
use async_trait::async_trait;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage, Value};

fn emit_single_storage(mut storage: FieldStorage, target: &EvaluationTarget, dst: &mut DataRecord) {
    let needs_conversion =
        target.data_type() != storage.get_meta() && target.data_type() != &DataType::Auto;

    if storage.is_shared() && !needs_conversion {
        if matches!(storage.as_field().get_value(), Value::Null) {
            return;
        }
        storage.set_name(target.safe_name());
        dst.items.push(storage);
    } else {
        let mut field = storage.into_owned();
        if matches!(field.get_value(), Value::Null) {
            return;
        }
        field.set_name(target.safe_name());

        if needs_conversion {
            field = omlobj_meta_conv(field, target);
        }

        dst.items.push(FieldStorage::from_owned(field));
    }
}

#[async_trait]
impl AsyncExpEvaluator for SingleEvalExp {
    async fn eval_proc_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) {
        if self.eval_way().support_batch_async() {
            let obj: Vec<DataField> = self.eval_way().extract_more_async(src, dst, cache).await;
            for i in 0..self.target().len() {
                if let (Some(target), Some(mut v)) = (self.target().get(i), obj.get(i).cloned()) {
                    if matches!(v.get_value(), Value::Null) {
                        continue;
                    }
                    if let Some(name) = target.name() {
                        v.set_name(name.clone());
                    }
                    dst.items
                        .push(FieldStorage::from_owned(omlobj_meta_conv(v, target)));
                }
            }
        } else if *self.sync() {
            if let Some(target) = self.target().first()
                && let Some(storage) = self.eval_way().extract_storage_sync(target, src, dst)
            {
                emit_single_storage(storage, target, dst);
            }
        } else if let Some(target) = self.target().first()
            && let Some(storage) = self
                .eval_way()
                .extract_storage_async(target, src, dst)
                .await
        {
            emit_single_storage(storage, target, dst);
        }
    }
}

#[allow(dead_code)]
impl NestedBinding {
    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.acquirer().extract_storage(target, src, dst)
    }

    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.acquirer().extract_one(target, src, dst)
    }

    pub(crate) fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        self.acquirer().extract_more(src, dst, cache)
    }

    pub(crate) fn support_batch(&self) -> bool {
        self.acquirer().support_batch()
    }
}

#[async_trait]
impl AsyncFieldExtractor for NestedBinding {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.acquirer().extract_one_async(target, src, dst).await
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.acquirer()
            .extract_storage_async(target, src, dst)
            .await
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        self.acquirer().extract_more_async(src, dst, cache).await
    }

    fn support_batch_async(&self) -> bool {
        self.acquirer().support_batch_async()
    }
}

#[allow(dead_code)]
impl GenericBinding {
    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.accessor().extract_storage(target, src, dst)
    }

    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.accessor().extract_one(target, src, dst)
    }

    pub(crate) fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        self.accessor().extract_more(src, dst, cache)
    }

    pub(crate) fn support_batch(&self) -> bool {
        self.accessor().support_batch()
    }
}

#[async_trait]
impl AsyncFieldExtractor for GenericBinding {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.accessor().extract_one_async(target, src, dst).await
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.accessor()
            .extract_storage_async(target, src, dst)
            .await
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        self.accessor().extract_more_async(src, dst, cache).await
    }

    fn support_batch_async(&self) -> bool {
        self.accessor().support_batch_async()
    }
}

#[allow(dead_code)]
impl GenericAccessor {
    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            // Static symbol: return Shared variant (zero-copy)
            // Skip extract_one to avoid unnecessary clone
            GenericAccessor::FieldArc(arc) => Some(FieldStorage::from_shared(arc.clone())),
            // Regular field: return Owned variant
            GenericAccessor::Field(x) => {
                crate::language::data_field_extract_one(x, target, src, dst)
                    .map(FieldStorage::from_owned)
            }
            GenericAccessor::Fun(x) => x
                .extract_one(target, src, dst)
                .map(FieldStorage::from_owned),
            GenericAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            GenericAccessor::Field(x) => {
                crate::language::data_field_extract_one(x, target, src, dst)
            }
            GenericAccessor::FieldArc(x) => {
                crate::language::data_field_extract_one(x.as_ref(), target, src, dst)
            }
            GenericAccessor::Fun(x) => x.extract_one(target, src, dst),
            GenericAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    pub(crate) fn extract_more(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for GenericAccessor {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            GenericAccessor::Field(x) => x.extract_one_async(target, src, dst).await,
            GenericAccessor::FieldArc(x) => x.as_ref().extract_one_async(target, src, dst).await,
            GenericAccessor::Fun(x) => x.extract_one_async(target, src, dst).await,
            GenericAccessor::StaticSymbol(sym) => {
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
            GenericAccessor::Field(x) => x.extract_storage_async(target, src, dst).await,
            GenericAccessor::FieldArc(x) => {
                x.as_ref().extract_storage_async(target, src, dst).await
            }
            GenericAccessor::Fun(x) => x.extract_storage_async(target, src, dst).await,
            GenericAccessor::StaticSymbol(sym) => {
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
            GenericAccessor::Field(x) => x.extract_more_async(src, dst, cache).await,
            GenericAccessor::FieldArc(x) => x.as_ref().extract_more_async(src, dst, cache).await,
            GenericAccessor::Fun(x) => x.extract_more_async(src, dst, cache).await,
            GenericAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    fn support_batch_async(&self) -> bool {
        match self {
            GenericAccessor::Field(x) => x.support_batch_async(),
            GenericAccessor::FieldArc(x) => x.as_ref().support_batch_async(),
            GenericAccessor::Fun(x) => x.support_batch_async(),
            GenericAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
}
