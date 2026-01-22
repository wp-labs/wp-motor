use super::operator::CmpOperator;

pub trait ConditionEvaluator<V> {
    fn evaluate(&self, data: &V) -> bool;
}

pub trait ValueGetter<T> {
    fn get_value(&self, var: &str) -> Option<&T>;
}
pub trait ValueGet0<T> {
    fn get_value(&self) -> Option<&T>;
}

pub trait WildcardMatcher {
    fn matches(&self, other: &Self) -> bool;
}

pub trait ValueComparator {
    fn is_support(&self, op: CmpOperator) -> bool;
    fn compare_with(&self, other: &Self, op: &CmpOperator) -> bool;
}
