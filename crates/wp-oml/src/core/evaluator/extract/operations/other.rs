use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;
use crate::language::GenericAccessor;
use crate::language::{GenericBinding, NestedBinding, SingleEvalExp};
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage};

use crate::core::FieldExtractor;

impl ExpEvaluator for SingleEvalExp {
    fn eval_proc<'a>(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) {
        if self.eval_way().support_batch() {
            let obj: Vec<DataField> = self.eval_way().extract_more(src, dst, cache);
            for i in 0..self.target().len() {
                if let (Some(target), Some(mut v)) = (self.target().get(i), obj.get(i).cloned()) {
                    if let Some(name) = target.name() {
                        v.set_name(name.clone());
                    }
                    dst.items
                        .push(FieldStorage::from_owned(omlobj_meta_conv(v, target)));
                }
            }
        } else if let Some(target) = self.target().first()
            && let Some(mut storage) = self.eval_way().extract_storage(target, src, dst)
        {
            // wp-model-core 0.8.4: FieldRef supports cur_name overlay
            // We can now use zero-copy for Shared variants!

            let needs_conversion = target.data_type() != storage.get_meta()
                                && target.data_type() != &DataType::Auto;

            if storage.is_shared() && !needs_conversion {
                // ✅ Shared + no conversion: Zero-copy optimization
                // set_name() only modifies cur_name, doesn't clone Arc
                storage.set_name(target.safe_name());
                dst.items.push(storage);
            } else {
                // Owned or needs conversion: Apply name to underlying field
                let mut field = storage.into_owned();
                field.set_name(target.safe_name());

                if needs_conversion {
                    field = omlobj_meta_conv(field, target);
                }

                dst.items.push(FieldStorage::from_owned(field));
            }
        }
    }
}

impl FieldExtractor for NestedBinding {
    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.acquirer().extract_storage(target, src, dst)
    }

    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        self.acquirer().extract_one(target, src, dst)
    }
}

impl FieldExtractor for GenericBinding {
    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.accessor().extract_storage(target, src, dst)
    }

    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        self.accessor().extract_one(target, src, dst)
    }
}

impl FieldExtractor for GenericAccessor {
    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            // Static symbol: return Shared variant (zero-copy)
            GenericAccessor::FieldArc(arc) => arc
                .as_ref()
                .extract_one(target, src, dst)
                .map(|_| FieldStorage::from_shared(arc.clone())),
            // Regular field: return Owned variant
            GenericAccessor::Field(x) => x.extract_one(target, src, dst).map(FieldStorage::from_owned),
            GenericAccessor::Fun(x) => x.extract_one(target, src, dst).map(FieldStorage::from_owned),
            GenericAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            GenericAccessor::Field(x) => x.extract_one(target, src, dst),
            GenericAccessor::FieldArc(x) => x.as_ref().extract_one(target, src, dst),
            GenericAccessor::Fun(x) => x.extract_one(target, src, dst),
            GenericAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
}
