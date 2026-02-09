use crate::core::FieldExtractor;
use crate::core::prelude::*;
use crate::language::EvaluationTarget;
use crate::language::FieldTake;
use wildmatch::WildMatch;
use wp_model_core::model::{DataField, DataRecord};

impl FieldExtractor for FieldTake {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
    ) -> Option<DataField> {
        let target_name = target.safe_name();
        let key_string = self.get().clone().unwrap_or(target_name.clone());
        //let key_string = self.get().clone().unwrap_or(target.name().to_string());
        let key = key_string.as_str();
        if let Some(value) = find_move_tdo(target, src, key, false) {
            return Some(value);
        }

        for option in self.option() {
            if let Some(value) = find_move_tdo(target, src, option, true) {
                return Some(value);
            }
        }
        None
    }
}

fn find_move_tdo(
    _target: &EvaluationTarget,
    src: &mut DataRecordRef,
    key: &str,
    option: bool,
) -> Option<DataField> {
    if let Some((idx, found)) = src.get_pos(key)
        && !(option && found.value.is_empty())
    {
        let obj = (*found).clone();
        src.remove(idx);
        return Some(obj);
    }
    None
}

impl FieldCollector for FieldTake {
    fn collect_item(
        &self,
        _name: &str,
        src: &DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Vec<DataField> {
        let mut result: Vec<DataField> = Vec::with_capacity(3);
        for i in src.iter() {
            for key in &self.collect {
                if WildMatch::new(key.as_str()).matches(i.get_name().trim()) {
                    result.push((*i).clone())
                }
            }
        }
        for i in &dst.items {
            for key in &self.collect {
                if WildMatch::new(key.as_str()).matches(i.get_name().trim()) {
                    result.push(i.as_field().clone())
                }
            }
        }
        result
    }
}
