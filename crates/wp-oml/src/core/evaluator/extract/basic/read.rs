use crate::core::prelude::*;
use wp_model_core::model::FieldStorage;
impl FieldExtractor for FieldRead {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        let key_string = self
            .get()
            .clone()
            .or(target.name().clone())
            .unwrap_or("_".to_string());
        let key = key_string.as_str();
        if let Some(value) = find_tdc_target(target, dst, key, false) {
            return Some(value);
        }
        if let Some(value) = find_tdr_target(target, src, key, false) {
            return Some(value);
        }

        for option in self.option() {
            if let Some(value) = find_tdc_target(target, dst, option, true) {
                return Some(value);
            }
            if let Some(value) = find_tdr_target(target, src, option, true) {
                return Some(value);
            }
        }
        None
    }

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        let key_string = self
            .get()
            .clone()
            .or(target.name().clone())
            .unwrap_or("_".to_string());
        let key = key_string.as_str();

        // Try to find in dst first (with FieldStorage preservation)
        if let Some(storage) = find_tdc_target_storage(dst, key, false) {
            return Some(storage);
        }
        // Try to find in src (with FieldStorage preservation)
        if let Some(storage) = find_tdr_target_storage(src, key, false) {
            return Some(storage);
        }

        // Try options
        for option in self.option() {
            if let Some(storage) = find_tdc_target_storage(dst, option, true) {
                return Some(storage);
            }
            if let Some(storage) = find_tdr_target_storage(src, option, true) {
                return Some(storage);
            }
        }
        None
    }
}

fn find_tdc_target(
    _target: &EvaluationTarget,
    src: &DataRecord,
    key: &str,
    option: bool,
) -> Option<DataField> {
    if let Some(found) = src.field(key)
        && !(option && found.as_field().value.is_empty())
    {
        return Some(found.as_field().clone());
    }
    None
}

// Zero-copy version: returns FieldStorage directly
fn find_tdc_target_storage(src: &DataRecord, key: &str, option: bool) -> Option<FieldStorage> {
    // Directly search in items to preserve FieldStorage
    for item in &src.items {
        if item.get_name() == key {
            if option && item.as_field().value.is_empty() {
                return None;
            }
            return Some(item.clone()); // ✅ Clone FieldStorage (Arc clone if Shared)
        }
    }
    None
}

fn find_tdr_target(
    _target: &EvaluationTarget,
    src: &DataRecordRef,
    key: &str,
    option: bool,
) -> Option<DataField> {
    if let Some((_, found)) = src.get_pos(key)
        && !(option && found.value.is_empty())
    {
        let obj = (*found).clone();
        return Some(obj);
    }
    None
}

// Zero-copy version: returns FieldStorage directly
fn find_tdr_target_storage(src: &DataRecordRef, key: &str, option: bool) -> Option<FieldStorage> {
    // Search in source record with FieldStorage preservation
    for item in src.iter() {
        if item.get_name() == key {
            if option && item.value.is_empty() {
                return None;
            }
            return Some(FieldStorage::from_owned((*item).clone()));
        }
    }
    None
}
impl FieldCollector for FieldRead {
    fn collect_item(
        &self,
        _name: &str,
        src: &DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Vec<DataField> {
        let mut result: Vec<DataField> = Vec::with_capacity(10);
        // 同一个字段先从dst里查找，查找不到再到src查找
        'outer: for cw in self.collect_wild() {
            for i in &dst.items {
                if cw.matches(i.get_name().trim()) {
                    result.push(i.as_field().clone());
                    continue 'outer;
                }
            }

            for i in src.iter() {
                if cw.matches(i.get_name().trim()) {
                    result.push((*i).clone());
                    continue 'outer;
                }
            }
        }
        result
    }
}
