use crate::core::prelude::*;
use crate::language::{PiPeOperation, PipeFun, PipeSource};
use async_trait::async_trait;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage, Value};

use crate::core::AsyncFieldExtractor;

/// 从管道源取起始字段（同步）
fn has_remove(items: &[PipeFun]) -> bool {
    items.iter().any(|fun| matches!(fun, PipeFun::Remove(_)))
}

fn remove_marker(target: &EvaluationTarget) -> DataField {
    DataField::new(DataType::Ignore, target.safe_name(), Value::Null)
}

fn source_extract_one(
    source: &PipeSource,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> Option<DataField> {
    match source {
        PipeSource::Accessor(a) => a.extract_one(target, src, dst),
        PipeSource::AccessDirect(op) => op.extract_one(target, src, dst),
        PipeSource::Precise(op) => op
            .extract_storage_sync(target, src, dst)
            .map(|storage| storage.into_owned()),
    }
}

/// 从管道源取起始字段（storage 零拷贝）
fn source_extract_storage(
    source: &PipeSource,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> Option<FieldStorage> {
    match source {
        PipeSource::Accessor(a) => a.extract_storage(target, src, dst),
        PipeSource::AccessDirect(op) => op.extract_storage(target, src, dst),
        PipeSource::Precise(op) => op.extract_storage_sync(target, src, dst),
    }
}

/// 从管道源取起始字段（async）
async fn source_extract_one_async(
    source: &PipeSource,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> Option<DataField> {
    match source {
        PipeSource::Accessor(a) => a.extract_one_async(target, src, dst).await,
        PipeSource::AccessDirect(op) => op.extract_one_async(target, src, dst).await,
        PipeSource::Precise(op) => op.extract_one_async(target, src, dst).await,
    }
}

/// 从管道源取起始字段（async storage）
async fn source_extract_storage_async(
    source: &PipeSource,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> Option<FieldStorage> {
    match source {
        PipeSource::Accessor(a) => a.extract_storage_async(target, src, dst).await,
        PipeSource::AccessDirect(op) => op.extract_storage_async(target, src, dst).await,
        PipeSource::Precise(op) => op.extract_storage_async(target, src, dst).await,
    }
}

/// 管道操作 - pipe source | fn1 | fn2 | ...
///
/// 从源字段读取数据，依次通过管道函数进行转换处理
#[allow(dead_code)]
impl PiPeOperation {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        if let Some(mut from) = source_extract_one(self.from(), target, src, dst) {
            for pipe in self.items() {
                from = pipe.value_cacu(from);
            }
            return Some(from);
        }
        if has_remove(self.items()) {
            return Some(remove_marker(target));
        }
        None
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        // Use extract_storage to preserve zero-copy for Shared variants
        if let Some(mut from_storage) = source_extract_storage(self.from(), target, src, dst) {
            for pipe in self.items() {
                from_storage = pipe.value_cacu_storage(from_storage);
            }
            return Some(from_storage);
        }
        if has_remove(self.items()) {
            return Some(FieldStorage::from_owned(remove_marker(target)));
        }
        None
    }

    pub(crate) fn extract_more(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for PiPeOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        if let Some(mut from) = source_extract_one_async(self.from(), target, src, dst).await {
            for pipe in self.items() {
                from = pipe.value_cacu(from);
            }
            return Some(from);
        }
        if has_remove(self.items()) {
            return Some(remove_marker(target));
        }
        None
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        if let Some(mut from_storage) =
            source_extract_storage_async(self.from(), target, src, dst).await
        {
            for pipe in self.items() {
                from_storage = pipe.value_cacu_storage(from_storage);
            }
            return Some(from_storage);
        }
        if has_remove(self.items()) {
            return Some(FieldStorage::from_owned(remove_marker(target)));
        }
        None
    }
}
