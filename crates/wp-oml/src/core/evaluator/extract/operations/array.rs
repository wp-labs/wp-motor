use crate::core::prelude::*;
use crate::language::ArrOperation;

use wp_model_core::model::{DataField, DataRecord};

use crate::core::FieldExtractor;

impl FieldExtractor for ArrOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        let target_name = target.name().clone().unwrap_or("_".to_string());
        let arr = self.dat_crate.collect_item(target_name.as_str(), src, dst);
        if !arr.is_empty() {
            Some(DataField::from_arr(target_name, arr))
        } else {
            None
        }
    }
}
