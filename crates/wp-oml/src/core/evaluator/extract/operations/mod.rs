mod array;
mod calc;
mod access_direct;
mod lookup;
mod map;
mod matchs;
mod other;
mod record;

use crate::language::DirectAccessor;
use crate::language::EvaluationTarget;
use crate::language::VarAccess;
use wp_model_core::model::DataType;

/// 从访问器取目标字段名的临时 EvaluationTarget（calc / access_direct 共用）
pub(crate) fn operand_target(
    accessor: &DirectAccessor,
    fallback: &EvaluationTarget,
) -> EvaluationTarget {
    let key = accessor
        .field_name()
        .clone()
        .unwrap_or_else(|| fallback.safe_name());
    EvaluationTarget::new(key, DataType::Auto)
}
