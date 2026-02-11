use crate::compare::compare_datafield;
use orion_exp::evaluator::{
    evaluate_expression_with_getter as ox_evaluate_expression_with,
    evaluate_with_getter as ox_evaluate_with,
};
use orion_exp::{CmpOperator, ValueGetter, core::logic::LogicalExpress};
use wp_model_core::model::{DataField, DataRecord};

// Local adapter: implement external trait for our local wrapper (allowed by orphan rules)
pub struct RecordGetter<'a>(pub &'a DataRecord);

impl<'a> ValueGetter<DataField> for RecordGetter<'a> {
    fn get_value(&self, var: &str) -> Option<&DataField> {
        self.0.field(var).map(|storage| storage.as_field())
    }
}

/// Evaluate a LogicalExpress against a DataRecord
pub fn evaluate_expr(
    expr: &LogicalExpress<DataField, orion_exp::RustSymbol>,
    data: &DataRecord,
) -> bool {
    // Use function-first evaluation to avoid trait impls on external types
    let getter = RecordGetter(data);
    let cmp = |l: &DataField, r: &DataField, op: CmpOperator| compare_datafield(l, r, op);
    ox_evaluate_with(expr, &getter, &cmp)
}

/// Evaluate a general Expression tree against a DataRecord
pub fn evaluate_expression(
    expr: &orion_exp::Expression<DataField, orion_exp::RustSymbol>,
    data: &DataRecord,
) -> bool {
    let getter = RecordGetter(data);
    let cmp = |l: &DataField, r: &DataField, op: CmpOperator| compare_datafield(l, r, op);
    ox_evaluate_expression_with(expr, &getter, &cmp)
}
