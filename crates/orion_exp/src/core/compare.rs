use std::marker::PhantomData;

use derive_getters::Getters;

use crate::{CmpOperator, CmpSymbolProvider, ConditionEvaluator, ValueComparator, ValueGetter};
#[derive(Debug, PartialEq, Clone, Getters)]
pub struct Comparison<T: Sized, S: CmpSymbolProvider> {
    _keep: PhantomData<S>,
    var_name: String,
    compare_op: CmpOperator,
    right_const: T,
}

impl<T, CS> Comparison<T, CS>
where
    CS: CmpSymbolProvider,
{
    pub fn new<S: Into<String>>(op: CmpOperator, name: S, target: T) -> Self {
        Self {
            _keep: PhantomData,
            var_name: name.into(),
            compare_op: op,
            right_const: target,
        }
    }
}

impl<T, V, S> ConditionEvaluator<V> for Comparison<T, S>
where
    V: ValueGetter<T>,
    T: ValueComparator + Sized,
    S: CmpSymbolProvider,
{
    fn evaluate(&self, data: &V) -> bool {
        // Compare in the intuitive order: var op const
        // Note: For wildcard (We), the const on the right is treated as the pattern
        // and the variable value on the left as the candidate.
        if let Some(var_obj) = data.get_value(self.var_name()) {
            return var_obj.compare_with(&self.right_const, self.compare_op());
        }
        false
    }
}
