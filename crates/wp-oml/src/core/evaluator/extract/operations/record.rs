use crate::core::prelude::*;
use crate::language::RecordOperation;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

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
                    // Use extract_storage to preserve zero-copy for Arc variants
                    let storage = default_acq.extract_storage(target, src, dst);
                    return storage.map(|s| {
                        let mut field = s.into_owned();
                        field.set_name(name);
                        field
                    });
                }
                None
            }
        }
    }

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        // Try primary extraction first
        if let Some(storage) = self.dat_get.extract_storage(target, src, dst) {
            return Some(storage);
        }

        // Fall back to default value with zero-copy support
        if let Some(default_acq) = &self.default_val {
            let name = target.name().clone().unwrap_or("_".to_string());
            let storage = default_acq.extract_storage(target, src, dst);
            return storage.map(|mut s| {
                if s.is_shared() {
                    // ✅ Zero-copy: modify cur_name without cloning Arc
                    s.set_name(name);
                    s
                } else {
                    // Owned: extract and modify underlying field
                    let mut field = s.into_owned();
                    field.set_name(name);
                    FieldStorage::from_owned(field)
                }
            });
        }

        None
    }
}
