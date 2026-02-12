use crate::core::prelude::*;
use crate::language::MatchAble;
use crate::language::MatchOperation;
use crate::language::MatchSource;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage};

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
            MatchSource::Triple(s1, s2, s3) => {
                let k1 = s1.field_name().clone().unwrap_or(target.to_string());
                let k2 = s2.field_name().clone().unwrap_or(target.to_string());
                let k3 = s3.field_name().clone().unwrap_or(target.to_string());
                let c1 = EvaluationTarget::new(k1, DataType::Auto);
                let c2 = EvaluationTarget::new(k2, DataType::Auto);
                let c3 = EvaluationTarget::new(k3, DataType::Auto);

                let v1 = s1.extract_one(&c1, src, dst);
                let v2 = s2.extract_one(&c2, src, dst);
                let v3 = s3.extract_one(&c3, src, dst);
                if let (Some(v1), Some(v2), Some(v3)) = (v1, v2, v3) {
                    for i in self.items() {
                        if i.is_match((&v1, &v2, &v3)) {
                            return i.result().extract_one(target, src, dst);
                        }
                    }
                }
            }
            MatchSource::Quadruple(s1, s2, s3, s4) => {
                let k1 = s1.field_name().clone().unwrap_or(target.to_string());
                let k2 = s2.field_name().clone().unwrap_or(target.to_string());
                let k3 = s3.field_name().clone().unwrap_or(target.to_string());
                let k4 = s4.field_name().clone().unwrap_or(target.to_string());
                let c1 = EvaluationTarget::new(k1, DataType::Auto);
                let c2 = EvaluationTarget::new(k2, DataType::Auto);
                let c3 = EvaluationTarget::new(k3, DataType::Auto);
                let c4 = EvaluationTarget::new(k4, DataType::Auto);

                let v1 = s1.extract_one(&c1, src, dst);
                let v2 = s2.extract_one(&c2, src, dst);
                let v3 = s3.extract_one(&c3, src, dst);
                let v4 = s4.extract_one(&c4, src, dst);
                if let (Some(v1), Some(v2), Some(v3), Some(v4)) = (v1, v2, v3, v4) {
                    for i in self.items() {
                        if i.is_match((&v1, &v2, &v3, &v4)) {
                            return i.result().extract_one(target, src, dst);
                        }
                    }
                }
            }
        }
        if let Some(default) = self.default() {
            return default.result().extract_one(target, src, dst);
        }
        None
    }

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        // Use extract_storage instead of extract_one to preserve zero-copy for Arc variants
        match self.dat_crate() {
            MatchSource::Single(dat) => {
                let key = dat.field_name().clone().unwrap_or(target.to_string());
                let cur = EvaluationTarget::new(key, DataType::Auto);
                if let Some(x) = dat.extract_one(&cur, src, dst) {
                    for i in self.items() {
                        if i.is_match(&x) {
                            // Call extract_storage to enable zero-copy for FieldArc/ObjArc
                            return i.result().extract_storage(target, src, dst);
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
                            // Call extract_storage to enable zero-copy for FieldArc/ObjArc
                            return i.result().extract_storage(target, src, dst);
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
            MatchSource::Triple(s1, s2, s3) => {
                let k1 = s1.field_name().clone().unwrap_or(target.to_string());
                let k2 = s2.field_name().clone().unwrap_or(target.to_string());
                let k3 = s3.field_name().clone().unwrap_or(target.to_string());
                let c1 = EvaluationTarget::new(k1, DataType::Auto);
                let c2 = EvaluationTarget::new(k2, DataType::Auto);
                let c3 = EvaluationTarget::new(k3, DataType::Auto);

                let v1 = s1.extract_one(&c1, src, dst);
                let v2 = s2.extract_one(&c2, src, dst);
                let v3 = s3.extract_one(&c3, src, dst);
                if let (Some(v1), Some(v2), Some(v3)) = (v1, v2, v3) {
                    for i in self.items() {
                        if i.is_match((&v1, &v2, &v3)) {
                            return i.result().extract_storage(target, src, dst);
                        }
                    }
                }
            }
            MatchSource::Quadruple(s1, s2, s3, s4) => {
                let k1 = s1.field_name().clone().unwrap_or(target.to_string());
                let k2 = s2.field_name().clone().unwrap_or(target.to_string());
                let k3 = s3.field_name().clone().unwrap_or(target.to_string());
                let k4 = s4.field_name().clone().unwrap_or(target.to_string());
                let c1 = EvaluationTarget::new(k1, DataType::Auto);
                let c2 = EvaluationTarget::new(k2, DataType::Auto);
                let c3 = EvaluationTarget::new(k3, DataType::Auto);
                let c4 = EvaluationTarget::new(k4, DataType::Auto);

                let v1 = s1.extract_one(&c1, src, dst);
                let v2 = s2.extract_one(&c2, src, dst);
                let v3 = s3.extract_one(&c3, src, dst);
                let v4 = s4.extract_one(&c4, src, dst);
                if let (Some(v1), Some(v2), Some(v3), Some(v4)) = (v1, v2, v3, v4) {
                    for i in self.items() {
                        if i.is_match((&v1, &v2, &v3, &v4)) {
                            return i.result().extract_storage(target, src, dst);
                        }
                    }
                }
            }
        }
        if let Some(default) = self.default() {
            // Call extract_storage to enable zero-copy for FieldArc/ObjArc
            return default.result().extract_storage(target, src, dst);
        }
        None
    }
}
