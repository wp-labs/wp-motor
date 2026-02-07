use crate::core::prelude::*;
use crate::language::MatchAble;
use crate::language::MatchOperation;
use crate::language::MatchSource;
use wp_model_core::model::{DataField, DataRecord, DataType};

use crate::core::FieldExtractor;

impl FieldExtractor for MatchOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self.dat_crate() {
            MatchSource::Single(dat) => {
                let key = dat.field_name().clone().unwrap_or(target.to_string());
                let cur = EvaluationTarget::new(key, DataType::Auto);
                if let Some(x) = dat.extract_one(&cur, src, dst) {
                    for i in self.items() {
                        if i.is_match(&x) {
                            return i.result().extract_one(target, src, dst);
                        }
                    }
                }
            }
            MatchSource::Double(fst, sec) => {
                let fst_key = fst.field_name().clone().unwrap_or(target.to_string());
                let fst_cur = EvaluationTarget::new(fst_key, DataType::Auto);

                let sec_key = sec.field_name().clone().unwrap_or(target.to_string());
                let sec_cur = EvaluationTarget::new(sec_key, DataType::Auto);

                let fst_val_opt = fst.extract_one(&fst_cur, src, dst);
                let sec_val_opt = sec.extract_one(&sec_cur, src, dst);
                if let (Some(fst_val), Some(sec_val)) = (fst_val_opt, sec_val_opt) {
                    for i in self.items() {
                        if i.is_match((&fst_val, &sec_val)) {
                            return i.result().extract_one(target, src, dst);
                        }
                    }
                    warn_edata!(
                        dst.id,
                        "not same type data ({}:{}, {}:{})",
                        fst_val.get_name(),
                        fst_val.get_meta(),
                        sec_val.get_name(),
                        sec_val.get_meta(),
                    );
                }
            }
        }
        if let Some(default) = self.default() {
            return default.result().extract_one(target, src, dst);
        }
        None
    }
}
