use crate::core::prelude::*;
use crate::language::RecordOperation;
use wp_model_core::model::{DataField, DataRecord};

use crate::core::FieldExtractor;

impl FieldExtractor for RecordOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self.dat_get.extract_one(target, src, dst) {
            Some(x) => Some(x),
            None => {
                if let Some(default_acq) = &self.default_val {
                    let name = target.name().clone().unwrap_or("_".to_string());
                    let value = default_acq.extract_one(target, src, dst);
                    return name_default_tdo(&value, &name);
                }
                None
            }
        }
    }
}
pub(crate) fn name_default_tdo(dft_val: &Option<DataField>, name: &str) -> Option<DataField> {
    let dft_tdo = dft_val.clone();
    match dft_tdo {
        None => None,
        Some(mut tdo) => {
            tdo.set_name(name.to_string());
            Some(tdo)
        }
    }
}
