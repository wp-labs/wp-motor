use std::slice::Iter;

use rustc_hash::FxHashMap;

use crate::core::prelude::*;
use wp_model_core::model::FieldStorage;
use wp_model_core::model::data::Field;

/// 字段数低于该阈值时直接用线性扫描（避免为小记录构建索引的固定开销）。
const INDEX_THRESHOLD: usize = 64;

pub struct RecordRef<'a, T> {
    items: Vec<&'a Field<T>>,
    /// 惰性构建的 `name → position` 索引，使用 `&str` 键 + FxHash 避免分配与慢哈希。
    /// 仅在字段数达到阈值后构建；`remove` 会使索引失效。
    index: Option<FxHashMap<&'a str, usize>>,
}
pub type DataRecordRef<'a> = RecordRef<'a, Value>;

#[allow(dead_code)]
impl<T> RecordRef<'_, T> {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn ensure_index(&mut self) {
        if self.index.is_some() || self.items.len() < INDEX_THRESHOLD {
            return;
        }
        let mut index = FxHashMap::default();
        index.reserve(self.items.len());
        for (i, item) in self.items.iter().enumerate() {
            index.entry(item.get_name()).or_insert(i);
        }
        self.index = Some(index);
    }

    /// 按名字查找位置：小记录走线性扫描，大记录走索引。
    pub fn find_pos(&mut self, key: &str) -> Option<usize> {
        if self.items.len() < INDEX_THRESHOLD {
            return self.items.iter().position(|o| o.get_name() == key);
        }
        self.ensure_index();
        self.index
            .as_ref()
            .and_then(|index| index.get(key).copied())
    }

    /// 按名字查找字段：小记录走线性扫描，大记录走索引。
    pub fn get_indexed(&mut self, key: &str) -> Option<&Field<T>> {
        if self.items.len() < INDEX_THRESHOLD {
            return self.items.iter().find(|o| o.get_name() == key).copied();
        }
        let pos = self.find_pos(key)?;
        self.items.get(pos).copied()
    }

    fn invalidate_index(&mut self) {
        self.index = None;
    }
}

impl<'a, T> From<Vec<&'a Field<T>>> for RecordRef<'a, T> {
    fn from(items: Vec<&'a Field<T>>) -> Self {
        Self { items, index: None }
    }
}
impl<'a, T> From<&'a Vec<Field<T>>> for RecordRef<'a, T> {
    fn from(value: &'a Vec<Field<T>>) -> Self {
        let items = value.iter().collect();
        Self { items, index: None }
    }
}

impl<'a> From<&'a wp_model_core::model::data::Record<Field<wp_model_core::model::Value>>>
    for RecordRef<'a, wp_model_core::model::Value>
{
    fn from(
        value: &'a wp_model_core::model::data::Record<Field<wp_model_core::model::Value>>,
    ) -> Self {
        let items = value.items.iter().collect();
        Self { items, index: None }
    }
}

// Support for Record<FieldStorage> (new FieldStorage-based DataRecord)
impl<'a> From<&'a wp_model_core::model::data::Record<FieldStorage>>
    for RecordRef<'a, wp_model_core::model::Value>
{
    fn from(value: &'a wp_model_core::model::data::Record<FieldStorage>) -> Self {
        let items = value
            .items
            .iter()
            .map(|storage| storage.as_field())
            .collect();
        Self { items, index: None }
    }
}

impl<T> RecordRef<'_, T>
where
    T: AsValueRef<Value>,
{
    pub fn get_pos(&self, key: &str) -> Option<(usize, &Field<T>)> {
        for (i, o) in self.items.iter().enumerate() {
            if o.get_name() == key {
                return Some((i, *o));
            }
        }
        None
    }
    pub fn get(&self, key: &str) -> Option<&Field<T>> {
        self.items.iter().find(|o| o.get_name() == key).copied()
    }
    pub fn iter(&self) -> Iter<'_, &Field<T>> {
        self.items.iter()
    }
    pub fn remove(&mut self, idx: usize) {
        self.items.remove(idx);
        self.invalidate_index();
    }
}

#[cfg(test)]
mod tests {
    use super::DataRecordRef;
    use wp_model_core::model::{DataField, DataRecord};

    fn sample() -> DataRecord {
        DataRecord::from(vec![
            DataField::from_chars("a", "1"),
            DataField::from_chars("b", "2"),
            DataField::from_chars("c", "3"),
        ])
    }

    #[test]
    fn linear_lookup_matches() {
        let rec = sample();
        let mut r = DataRecordRef::from(&rec);
        assert_eq!(r.find_pos("b"), Some(1));
        assert!(r.get_indexed("a").is_some());
        assert!(r.get_indexed("missing").is_none());
    }

    #[test]
    fn index_rebuilds_after_remove() {
        let fields: Vec<DataField> = (0..super::INDEX_THRESHOLD)
            .map(|i| DataField::from_chars(format!("f{}", i), "x"))
            .collect();
        let rec = DataRecord::from(fields);
        let mut r = DataRecordRef::from(&rec);
        assert_eq!(r.find_pos("f0"), Some(0)); // build index
        r.remove(0);
        assert_eq!(r.find_pos("f0"), None);
        assert_eq!(r.find_pos("f1"), Some(0));
        assert!(r.get_indexed("f2").is_some());
    }

    #[test]
    fn index_keeps_first_duplicate() {
        let rec = DataRecord::from(vec![
            DataField::from_chars("a", "first"),
            DataField::from_chars("a", "second"),
        ]);
        let mut r = DataRecordRef::from(&rec);
        assert_eq!(r.find_pos("a"), Some(0));
    }
}
