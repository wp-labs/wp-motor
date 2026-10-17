use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;
use async_trait::async_trait;

use crate::language::MapOperation;
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage, Value};

use crate::core::AsyncFieldExtractor;

#[allow(dead_code)]
impl MapOperation {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let name = target.name().clone().unwrap_or("_".to_string());
        let mut obj = ObjectValue::default();
        for sub in self.subs() {
            // Use extract_storage to preserve zero-copy for Arc variants
            let storage_opt = sub.acquirer().extract_storage(sub.target(), src, dst);
            let sub_name = sub.target().safe_name();

            if let Some(mut storage) = storage_opt {
                if storage.get_meta() == &DataType::Ignore
                    && matches!(storage.as_field().get_value(), Value::Null)
                {
                    obj.remove(sub_name.as_str());
                    continue;
                }
                let needs_conversion = sub.target().data_type() != storage.get_meta()
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

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn extract_more(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for MapOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let name = target.name().clone().unwrap_or("_".to_string());
        let mut obj = ObjectValue::default();
        for sub in self.subs() {
            let storage_opt = sub
                .acquirer()
                .extract_storage_async(sub.target(), src, dst)
                .await;
            let sub_name = sub.target().safe_name();

            if let Some(mut storage) = storage_opt {
                if storage.get_meta() == &DataType::Ignore
                    && matches!(storage.as_field().get_value(), Value::Null)
                {
                    obj.remove(sub_name.as_str());
                    continue;
                }
                let needs_conversion = sub.target().data_type() != storage.get_meta()
                    && sub.target().data_type() != &DataType::Auto;

                if storage.is_shared() && !needs_conversion {
                    storage.set_name(sub_name.clone());
                    obj.insert(sub_name, storage);
                } else {
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
}
