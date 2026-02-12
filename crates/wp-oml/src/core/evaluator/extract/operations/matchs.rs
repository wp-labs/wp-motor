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
            MatchSource::Multi(sources) => {
                let mut vals: Vec<DataField> = Vec::with_capacity(sources.len());
                for s in sources.iter() {
                    let k = s.field_name().clone().unwrap_or(target.to_string());
                    let c = EvaluationTarget::new(k, DataType::Auto);
                    if let Some(v) = s.extract_one(&c, src, dst) {
                        vals.push(v);
                    } else {
                        // If any source fails to extract, skip matching
                        if let Some(default) = self.default() {
                            return default.result().extract_one(target, src, dst);
                        }
                        return None;
                    }
                }
                let refs: Vec<&DataField> = vals.iter().collect();
                for i in self.items() {
                    if i.is_match(refs.as_slice()) {
                        return i.result().extract_one(target, src, dst);
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
            MatchSource::Multi(sources) => {
                let mut vals: Vec<DataField> = Vec::with_capacity(sources.len());
                for s in sources.iter() {
                    let k = s.field_name().clone().unwrap_or(target.to_string());
                    let c = EvaluationTarget::new(k, DataType::Auto);
                    if let Some(v) = s.extract_one(&c, src, dst) {
                        vals.push(v);
                    } else {
                        if let Some(default) = self.default() {
                            return default.result().extract_storage(target, src, dst);
                        }
                        return None;
                    }
                }
                let refs: Vec<&DataField> = vals.iter().collect();
                for i in self.items() {
                    if i.is_match(refs.as_slice()) {
                        // Call extract_storage to enable zero-copy for FieldArc/ObjArc
                        return i.result().extract_storage(target, src, dst);
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
