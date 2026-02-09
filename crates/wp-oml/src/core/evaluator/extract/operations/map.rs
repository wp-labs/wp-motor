use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;

use crate::language::MapOperation;
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

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
            let one = sub.acquirer().extract_one(sub.target(), src, dst);
            let sub_name = sub.target().safe_name();
            if let Some(mut o) = one {
                o.set_name(sub_name.clone());
                obj.insert(
                    sub_name,
                    FieldStorage::from_owned(omlobj_meta_conv(o, sub.target())),
                );
            }
        }
        Some(DataField::from_obj(name, obj))
    }
}
