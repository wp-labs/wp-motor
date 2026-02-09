use std::slice::Iter;

use crate::core::prelude::*;
use wp_model_core::model::data::Field;
use wp_model_core::model::FieldStorage;
pub struct RecordRef<'a, T> {
    items: Vec<&'a Field<T>>,
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
}

impl<'a, T> From<Vec<&'a Field<T>>> for RecordRef<'a, T> {
    fn from(items: Vec<&'a Field<T>>) -> Self {
        Self { items }
    }
}
impl<'a, T> From<&'a Vec<Field<T>>> for RecordRef<'a, T> {
    fn from(value: &'a Vec<Field<T>>) -> Self {
        let items = value.iter().collect();
        Self { items }
    }
}

impl<'a> From<&'a wp_model_core::model::data::Record<Field<wp_model_core::model::Value>>>
    for RecordRef<'a, wp_model_core::model::Value>
{
    fn from(
        value: &'a wp_model_core::model::data::Record<Field<wp_model_core::model::Value>>,
    ) -> Self {
        let items = value.items.iter().collect();
        Self { items }
    }
}

// Support for Record<FieldStorage> (new FieldStorage-based DataRecord)
impl<'a> From<&'a wp_model_core::model::data::Record<FieldStorage>>
    for RecordRef<'a, wp_model_core::model::Value>
{
    fn from(value: &'a wp_model_core::model::data::Record<FieldStorage>) -> Self {
        let items = value.items.iter().map(|storage| storage.as_field()).collect();
        Self { items }
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
    }
}
