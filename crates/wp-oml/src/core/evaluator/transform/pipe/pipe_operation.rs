use crate::core::prelude::*;
use crate::language::PiPeOperation;
use wp_model_core::model::{DataField, DataRecord};

/// 管道操作 - pipe source | fn1 | fn2 | ...
///
/// 从源字段读取数据，依次通过管道函数进行转换处理
impl FieldExtractor for PiPeOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        if let Some(mut from) = self.from().extract_one(target, src, dst) {
            for pipe in self.items() {
                from = pipe.value_cacu(from);
            }
            return Some(from);
        }
        None
    }
}
