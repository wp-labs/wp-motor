use crate::core::prelude::*;
use crate::language::ObjArrayOperation;
use crate::language::PreciseEvaluator;
use async_trait::async_trait;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage, Value};

use crate::core::AsyncFieldExtractor;

impl ObjArrayOperation {
    /// 数组元素求值（异步）：每个元素独立求值成字段，Null/Ignore 元素跳过。
    pub(crate) async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let name = target.name().clone().unwrap_or("_".to_string());
        let item_target = EvaluationTarget::new(name.clone(), DataType::Auto);
        let mut items: Vec<DataField> = Vec::with_capacity(self.items().len());
        for item in self.items() {
            if let Some(field) = item.extract_one_async(&item_target, src, dst).await {
                // 缺失/占位元素（Null/Ignore）跳过，不进入数组
                if matches!(field.get_value(), Value::Null | Value::Ignore(_)) {
                    continue;
                }
                items.push(field);
            }
        }
        if items.is_empty() {
            return None;
        }
        Some(DataField::from_arr(name, items))
    }

    /// 同步求值：仅支持同步可求值的元素（object 字面量 / 值 / 函数 / 读取等）；
    /// 其余异步专属求值器在同步路径下跳过并告警（运行期走异步路径，不受影响）。
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let name = target.name().clone().unwrap_or("_".to_string());
        let item_target = EvaluationTarget::new(name.clone(), DataType::Auto);
        let mut items: Vec<DataField> = Vec::with_capacity(self.items().len());
        for item in self.items() {
            if let Some(field) = eval_item_sync(item, &item_target, src, dst) {
                // 缺失/占位元素（Null/Ignore）跳过，不进入数组
                if matches!(field.get_value(), Value::Null | Value::Ignore(_)) {
                    continue;
                }
                items.push(field);
            }
        }
        if items.is_empty() {
            return None;
        }
        Some(DataField::from_arr(name, items))
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

/// 同步求值单个数组元素：与 NestedAccessor 同步路径能力集保持一致。
fn eval_item_sync(
    item: &PreciseEvaluator,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> Option<DataField> {
    match item {
        PreciseEvaluator::Map(o) => o.extract_one(target, src, dst),
        PreciseEvaluator::Tdc(o) => o.extract_one(target, src, dst),
        PreciseEvaluator::Fun(o) => o.extract_one(target, src, dst),
        PreciseEvaluator::Collect(o) => o.extract_one(target, src, dst),
        PreciseEvaluator::Obj(o) => crate::language::data_field_extract_one(o, target, src, dst),
        PreciseEvaluator::ObjArc(o) => {
            crate::language::data_field_extract_one(o.as_ref(), target, src, dst)
        }
        PreciseEvaluator::Val(v) => crate::language::value_extract_one(v, target, src, dst),
        PreciseEvaluator::Calc(o) => o.extract_one(target, src, dst),
        other => {
            warn_data!("array item skip in sync path (async-only evaluator): {other}");
            None
        }
    }
}

#[async_trait]
impl AsyncFieldExtractor for ObjArrayOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one_async(target, src, dst).await
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one_async(target, src, dst)
            .await
            .map(FieldStorage::from_owned)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    fn support_batch_async(&self) -> bool {
        false
    }
}
