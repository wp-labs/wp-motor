use crate::core::AsyncFieldExtractor;
use crate::core::prelude::*;
use crate::language::EvaluationTarget;
use crate::language::FieldTake;
use async_trait::async_trait;
use wildmatch::WildMatch;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

#[allow(dead_code)]
impl FieldTake {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let target_name = target.safe_name();
        let key_string = self.get().clone().unwrap_or(target_name.clone());
        let key = key_string.as_str();

        let dst_val = find_move_tdc(dst, key, false);
        let src_val = find_move_tdo(target, src, key, false);
        // 优先返回 dst 中的值，如果 dst 中没有，再返回 src 中的值
        if dst_val.is_some() || src_val.is_some() {
            return dst_val.or(src_val);
        }
        for option in self.option() {
            let dst_val = find_move_tdc(dst, option, true);
            let src_val = find_move_tdo(target, src, option, true);
            if let Some(value) = dst_val.or(src_val) {
                return Some(value);
            }
        }
        None
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
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for FieldTake {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one(target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_storage(target, src, dst)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
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

fn find_move_tdc(dst: &mut DataRecord, key: &str, option: bool) -> Option<DataField> {
    let idx = dst
        .items
        .iter()
        .position(|item| item.get_name() == key && !(option && item.as_field().value.is_empty()))?;
    Some(dst.items.remove(idx).into_owned())
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
