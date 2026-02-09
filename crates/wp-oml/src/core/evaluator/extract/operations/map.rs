use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;

use crate::language::MapOperation;
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage};

use crate::core::FieldExtractor;

impl FieldExtractor for MapOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        let name = target.name().clone().unwrap_or("_".to_string());
        let mut obj = ObjectValue::default();
        for sub in self.subs() {
            // Use extract_storage to preserve zero-copy for Arc variants
            let storage_opt = sub.acquirer().extract_storage(sub.target(), src, dst);
            let sub_name = sub.target().safe_name();

            if let Some(mut storage) = storage_opt {
                let needs_conversion =
                    sub.target().data_type() != storage.get_meta()
                    && sub.target().data_type() != &DataType::Auto;

                if storage.is_shared() && !needs_conversion {
                    // ✅ Zero-copy path: modify cur_name without cloning Arc
                    storage.set_name(sub_name.clone());
                    obj.insert(sub_name, storage);
                } else {
                    // Owned or needs conversion: extract field and apply transformations
                    let mut field = storage.into_owned();
                    field.set_name(sub_name.clone());

                    if needs_conversion {
                        field = omlobj_meta_conv(field, sub.target());
                    }

                    obj.insert(sub_name, FieldStorage::from_owned(field));
                }
            }
        }
        Some(DataField::from_obj(name, obj))
    }

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }
}
