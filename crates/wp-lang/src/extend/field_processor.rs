use crate::parser::error::{WplCodeReason, WplCodeResult};
use crate::traits::FieldProcessor;
use once_cell::sync::Lazy;
use orion_error::ToStructError;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wp_model_core::model::DataField;
#[cfg(test)]
use wp_model_core::model::Value;

pub type FieldProcHold = Arc<dyn FieldProcessor>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FiledExtendType {
    MemChannel,
    InnerSource,
}

static FIELD_PROCESSORS: Lazy<RwLock<HashMap<FiledExtendType, Vec<FieldProcHold>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn field_registry() -> &'static RwLock<HashMap<FiledExtendType, Vec<FieldProcHold>>> {
    &FIELD_PROCESSORS
}

/// 注册一个 FieldProcessor；同名处理器会被后注册的实现覆盖。
pub fn register_field_processor<P>(extend_type: FiledExtendType, processor: P)
where
    P: FieldProcessor,
{
    let mut guard = field_registry()
        .write()
        .expect("field processor registry poisoned");
    let holder: FieldProcHold = Arc::new(processor);
    let bucket = guard.entry(extend_type).or_default();
    if let Some(pos) = bucket.iter().position(|fp| fp.name() == holder.name()) {
        bucket[pos] = holder;
    } else {
        bucket.push(holder);
    }
}

/// 返回指定扩展类型下已注册的 FieldProcessor 名称。
pub fn list_field_processors(extend_type: FiledExtendType) -> Vec<&'static str> {
    field_registry()
        .read()
        .expect("field processor registry poisoned")
        .get(&extend_type)
        .map(|bucket| bucket.iter().map(|fp| fp.name()).collect())
        .unwrap_or_default()
}

/// 获取指定扩展类型下最新注册的 Processor。
pub fn first_field_processor(extend_type: FiledExtendType) -> Option<FieldProcHold> {
    field_registry().read().ok().and_then(|map| {
        map.get(&extend_type)
            .and_then(|bucket| bucket.last().cloned())
    })
}

/// 查找指定扩展类型下的 Processor；返回 `Arc` 克隆副本。
pub fn get_field_processor(extend_type: FiledExtendType, name: &str) -> Option<FieldProcHold> {
    field_registry().read().ok().and_then(|map| {
        map.get(&extend_type)
            .and_then(|bucket| bucket.iter().find(|fp| fp.name() == name).cloned())
    })
}

#[allow(dead_code)]
pub(crate) fn run_field_processors(
    mut field: Option<&mut DataField>,
    extend_type: FiledExtendType,
) -> WplCodeResult<Vec<DataField>> {
    if let Some(bucket) = field_registry()
        .read()
        .expect("field processor registry poisoned")
        .get(&extend_type)
    {
        for processor in bucket {
            if let Err(msg) = processor.process(field.as_deref_mut()) {
                return Err(WplCodeReason::Plugin(format!(
                    "FieldProcessor '{}' failed: {}",
                    processor.name(),
                    msg
                ))
                .to_err());
            }
        }
    }
    Ok(Vec::new())
}

#[cfg(test)]
pub(crate) fn clear_field_processors() {
    field_registry()
        .write()
        .expect("field processor registry poisoned")
        .clear();
}

/// 一个空操作 Processor，主要用于验证管线可链接。
#[derive(Debug, Default, Clone, Copy)]
pub struct PassProcessor;

impl PassProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl FieldProcessor for PassProcessor {
    fn name(&self) -> &'static str {
        "pass"
    }

    fn process(&self, _field: Option<&mut DataField>) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TagProcessor(&'static str);
    impl FieldProcessor for TagProcessor {
        fn name(&self) -> &'static str {
            "tag"
        }
        fn process(&self, field: Option<&mut DataField>) -> Result<(), String> {
            if let Some(field) = field {
                let suffix = format!("|{}", self.0);
                if let Some(chars) = field.get_chars() {
                    let mut updated = String::from(chars);
                    updated.push_str(&suffix);
                    field.value = Value::from(updated.as_str());
                }
            }
            Ok(())
        }
    }

    #[test]
    fn pass_processor_is_noop() {
        clear_field_processors();
        register_field_processor(FiledExtendType::MemChannel, PassProcessor::new());
        let mut field = DataField::from_chars("key", "value");
        let extra =
            run_field_processors(Some(&mut field), FiledExtendType::MemChannel).expect("run");
        assert_eq!(field.get_chars(), Some("value"));
        assert!(extra.is_empty());
        clear_field_processors();
    }

    #[test]
    fn registry_replaces_same_name() {
        clear_field_processors();
        register_field_processor(FiledExtendType::MemChannel, PassProcessor::new());
        register_field_processor(FiledExtendType::MemChannel, TagProcessor("first"));
        let mut field = DataField::from_chars("key", "value");
        let extra = run_field_processors(Some(&mut field), FiledExtendType::MemChannel).unwrap();
        assert_eq!(field.get_chars(), Some("value|first"));
        assert!(extra.is_empty());

        register_field_processor(FiledExtendType::MemChannel, TagProcessor("second"));
        let mut field = DataField::from_chars("key", "value");
        let extra = run_field_processors(Some(&mut field), FiledExtendType::MemChannel).unwrap();
        assert_eq!(field.get_chars(), Some("value|second"));
        assert!(extra.is_empty());

        let names = list_field_processors(FiledExtendType::MemChannel);
        assert_eq!(names, vec!["pass", "tag"]);
        clear_field_processors();
    }

    #[test]
    fn lookup_returns_latest_clone() {
        clear_field_processors();
        register_field_processor(FiledExtendType::MemChannel, TagProcessor("one"));
        let first = get_field_processor(FiledExtendType::MemChannel, "tag").expect("first");
        assert_eq!(first.name(), "tag");

        register_field_processor(FiledExtendType::MemChannel, TagProcessor("two"));
        let second = get_field_processor(FiledExtendType::MemChannel, "tag").expect("second");
        assert_eq!(second.name(), "tag");
        drop(first);
        drop(second);
        clear_field_processors();
    }
}
