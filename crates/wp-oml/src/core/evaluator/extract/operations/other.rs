use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;
use crate::language::GenericAccessor;
use crate::language::{GenericBinding, NestedBinding, SingleEvalExp};
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};

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
                    dst.items.push(omlobj_meta_conv(v, target));
                }
            }
        } else if let Some(target) = self.target().first()
            && let Some(mut obj) = self.eval_way().extract_one(target, src, dst)
        {
            obj.set_name(target.safe_name());
            dst.items.push(omlobj_meta_conv(obj, target));
        }
    }
}

impl FieldExtractor for NestedBinding {
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
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            GenericAccessor::Field(x) => x.extract_one(target, src, dst),
            GenericAccessor::Fun(x) => x.extract_one(target, src, dst),
        }
    }
}
