pub mod cmp_pri;
use super::{
    CmpOperator, CmpSymbolProvider, ConditionEvaluator, LogicOperator, LogicSymbolProvider,
    ValueComparator, ValueGetter, WildcardMatcher,
    core::{
        compare::Comparison,
        logic::{Expression, LogicalExpress},
    },
};

impl<T> ValueComparator for T
where
    T: PartialOrd + WildcardMatcher,
{
    fn is_support(&self, _op: CmpOperator) -> bool {
        true
    }
    fn compare_with(&self, other: &Self, op: &CmpOperator) -> bool {
        match op {
            // Wildcard: right side is the pattern, left is the candidate value
            CmpOperator::We => other.matches(self),
            CmpOperator::Eq => *self == *other,
            CmpOperator::Ne => *self != *other,
            CmpOperator::Gt => *self > *other,
            CmpOperator::Ge => *self >= *other,
            CmpOperator::Lt => *self < *other,
            CmpOperator::Le => *self <= *other,
        }
    }
}

impl<T, V, S> ConditionEvaluator<V> for Expression<T, S>
where
    V: ValueGetter<T>,
    LogicalExpress<T, S>: ConditionEvaluator<V>,
    Comparison<T, S>: ConditionEvaluator<V>,
    T: Sized,
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    fn evaluate(&self, data: &V) -> bool {
        match self {
            Expression::Logic(logic) => logic.evaluate(data),
            Expression::Compare(compare) => compare.evaluate(data),
        }
    }
}

// Blanket impl: any LogicalExpress<T,S> can be evaluated against a ValueGetter<T>
impl<T, V, S> ConditionEvaluator<V> for LogicalExpress<T, S>
where
    V: ValueGetter<T>,
    T: Sized + ValueComparator,
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    fn evaluate(&self, data: &V) -> bool {
        cmp_is_true(&self.op, self.left.as_ref(), &self.right, data)
    }
}

/// Convenience: evaluate a logical expression using any value getter
pub fn evaluate_expr<T, V, S>(expr: &LogicalExpress<T, S>, data: &V) -> bool
where
    V: ValueGetter<T>,
    T: Sized + ValueComparator,
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    expr.evaluate(data)
}

// ----------------------------------------------------------------------------
// New function-first evaluation API (decoupled). Prefer these over trait-based.
// ----------------------------------------------------------------------------

/// Compare two values using the default semantics (PartialOrd + WildcardMatcher)
pub fn default_compare<T: PartialOrd + WildcardMatcher>(
    left: &T,
    right: &T,
    op: CmpOperator,
) -> bool {
    match op {
        CmpOperator::We => right.matches(left),
        CmpOperator::Eq => left == right,
        CmpOperator::Ne => left != right,
        CmpOperator::Gt => left > right,
        CmpOperator::Ge => left >= right,
        CmpOperator::Lt => left < right,
        CmpOperator::Le => left <= right,
    }
}

/// Adapter: wrap a closure as a `ValueGetter<T>` for compatibility paths
pub struct Getter<'a, T, F: Fn(&str) -> Option<&'a T>>(pub F, std::marker::PhantomData<&'a T>);

impl<'a, T, F> ValueGetter<T> for Getter<'a, T, F>
where
    F: Fn(&str) -> Option<&'a T>,
{
    fn get_value(&self, var: &str) -> Option<&T> {
        (self.0)(var)
    }
}

pub fn value_getter<'a, T, F: Fn(&str) -> Option<&'a T>>(f: F) -> Getter<'a, T, F> {
    Getter(f, std::marker::PhantomData)
}

/// Evaluate a Comparison node using function-style var-getter and comparator
pub fn evaluate_cmp_with<T, S, G, C>(cmp: &Comparison<T, S>, get: &G, compare: &C) -> bool
where
    S: CmpSymbolProvider,
    G: for<'a> Fn(&'a str) -> Option<&'a T>,
    C: Fn(&T, &T, CmpOperator) -> bool,
{
    if let Some(var) = get(cmp.var_name()) {
        return compare(var, cmp.right_const(), *cmp.compare_op());
    }
    false
}

/// Evaluate an Expression tree using function-style var-getter and comparator
pub fn evaluate_expression_with<T, S, G, C>(expr: &Expression<T, S>, get: &G, compare: &C) -> bool
where
    S: LogicSymbolProvider + CmpSymbolProvider,
    G: for<'a> Fn(&'a str) -> Option<&'a T>,
    C: Fn(&T, &T, CmpOperator) -> bool,
{
    match expr {
        Expression::Compare(c) => evaluate_cmp_with(c, get, compare),
        Expression::Logic(l) => evaluate_with(l, get, compare),
    }
}

/// Evaluate a LogicalExpress tree using function-style var-getter and comparator
pub fn evaluate_with<T, S, G, C>(lexpr: &LogicalExpress<T, S>, get: &G, compare: &C) -> bool
where
    S: LogicSymbolProvider + CmpSymbolProvider,
    G: for<'a> Fn(&'a str) -> Option<&'a T>,
    C: Fn(&T, &T, CmpOperator) -> bool,
{
    match lexpr.op() {
        LogicOperator::And => {
            let r = evaluate_expression_with(lexpr.right(), get, compare);
            if let Some(l) = lexpr.left().as_ref() {
                evaluate_expression_with(l, get, compare) && r
            } else {
                r
            }
        }
        LogicOperator::Or => {
            let r = evaluate_expression_with(lexpr.right(), get, compare);
            if let Some(l) = lexpr.left().as_ref() {
                evaluate_expression_with(l, get, compare) || r
            } else {
                r
            }
        }
        LogicOperator::Not => !evaluate_expression_with(lexpr.right(), get, compare),
    }
}

/// Evaluate using a ValueGetter (avoids HRTB/lifetime constraints for closures)
pub fn evaluate_cmp_with_getter<T, S, V, C>(cmp: &Comparison<T, S>, get: &V, compare: &C) -> bool
where
    S: CmpSymbolProvider,
    V: ValueGetter<T>,
    C: Fn(&T, &T, CmpOperator) -> bool,
{
    if let Some(var) = get.get_value(cmp.var_name()) {
        return compare(var, cmp.right_const(), *cmp.compare_op());
    }
    false
}

pub fn evaluate_expression_with_getter<T, S, V, C>(
    expr: &Expression<T, S>,
    get: &V,
    compare: &C,
) -> bool
where
    S: LogicSymbolProvider + CmpSymbolProvider,
    V: ValueGetter<T>,
    C: Fn(&T, &T, CmpOperator) -> bool,
{
    match expr {
        Expression::Compare(c) => evaluate_cmp_with_getter(c, get, compare),
        Expression::Logic(l) => evaluate_with_getter(l, get, compare),
    }
}

pub fn evaluate_with_getter<T, S, V, C>(lexpr: &LogicalExpress<T, S>, get: &V, compare: &C) -> bool
where
    S: LogicSymbolProvider + CmpSymbolProvider,
    V: ValueGetter<T>,
    C: Fn(&T, &T, CmpOperator) -> bool,
{
    match lexpr.op() {
        LogicOperator::And => {
            let r = evaluate_expression_with_getter(lexpr.right(), get, compare);
            if let Some(l) = lexpr.left().as_ref() {
                evaluate_expression_with_getter(l, get, compare) && r
            } else {
                r
            }
        }
        LogicOperator::Or => {
            let r = evaluate_expression_with_getter(lexpr.right(), get, compare);
            if let Some(l) = lexpr.left().as_ref() {
                evaluate_expression_with_getter(l, get, compare) || r
            } else {
                r
            }
        }
        LogicOperator::Not => !evaluate_expression_with_getter(lexpr.right(), get, compare),
    }
}

#[allow(clippy::borrowed_box)]
pub fn cmp_is_true<V, T: ConditionEvaluator<V>>(
    op: &LogicOperator,
    left: Option<&Box<T>>,
    right: &Box<T>,
    data: &V,
) -> bool {
    match op {
        LogicOperator::And => {
            if let Some(left) = left {
                left.evaluate(data) && right.evaluate(data)
            } else {
                right.evaluate(data)
            }
        }
        LogicOperator::Or => {
            if let Some(left) = left {
                left.evaluate(data) || right.evaluate(data)
            } else {
                right.evaluate(data)
            }
        }
        LogicOperator::Not => !right.evaluate(data),
    }
}

//不能进行泛化实现,会造成递归展开失败!!!!
/*
impl<T, V> Condition<V> for LogicExpress<T>
where
    V: ValueGet<T>,
    T: CompareAble + Sized,
    LogicalExpression<T>: Condition<V>,
{
    fn is_true(&self, data: &V) -> bool {
        match self.op {
            LogicOP::And => {
                if let Some(left) = &self.left {
                    left.is_true(data) && self.right.is_true(data)
                } else {
                    self.right.is_true(data)
                }
            }
            LogicOP::Or => {
                if let Some(left) = &self.left {
                    left.is_true(data) || self.right.is_true(data)
                } else {
                    self.right.is_true(data)
                }
            }
            LogicOP::Not => !self.right.is_true(data),
        }
    }
}
*/

#[cfg(test)]
mod tests {
    use crate::{
        CmpOperator, Comparison, ConditionEvaluator, Expression, LogicOperator, LogicalExpress,
        RustSymbol, ValueGetter,
    };
    use std::collections::HashMap;

    type CompareExpress<T> = crate::Comparison<T, RustSymbol>;

    impl<T> ValueGetter<T> for HashMap<&'static str, T> {
        fn get_value(&self, var: &str) -> Option<&T> {
            self.get(var)
        }
    }
    /*
    impl ConditionEvaluator<XMap<u32>> for LogicalExpress<u32, RustSymbol> {
        fn evaluate(&self, data: &XMap<u32>) -> bool {
            cmp_is_true(&self.op, self.left.as_ref(), &self.right, data)
        }
    }
    */

    #[test]
    pub fn test_eq_u32() {
        let express = CompareExpress::new(CmpOperator::Eq, "a", 1u32);
        let data = HashMap::from([("a", 1)]);
        assert!(express.evaluate(&data));
        let data = HashMap::from([("a", 2)]);
        assert!(!express.evaluate(&data));
        let data = HashMap::from([("b", 1)]);
        assert!(!express.evaluate(&data));

        let express2 = Comparison::new(CmpOperator::Eq, "a", 1u32);
        let express3 = LogicalExpress::<u32, RustSymbol>::new(
            LogicOperator::And,
            Some(Expression::Compare(express)),
            Expression::Compare(express2),
        );
        let data = HashMap::from([("a", 1)]);
        assert!(express3.evaluate(&data))
    }
    #[test]
    pub fn test_eq_f64() {
        let express = CompareExpress::new(CmpOperator::Eq, "a", 1.1f64);
        let data = HashMap::from([("a", 1.1f64)]);
        assert!(express.evaluate(&data));
        let data = HashMap::from([("a", 1.10000000001f64)]);
        assert!(!express.evaluate(&data));
    }

    #[test]
    pub fn test_compare_we() {
        let express = CompareExpress::new(CmpOperator::We, "a", "*ll*".to_string());
        let data = HashMap::from([("a", "hello".to_string())]);
        assert!(express.evaluate(&data));
        let data = HashMap::from([("a", "ell".to_string())]);
        assert!(express.evaluate(&data));
        let data = HashMap::from([("a", "heloo".to_string())]);
        assert!(!express.evaluate(&data));
    }
    #[test]
    pub fn test_compare_ge() {
        let express = CompareExpress::new(CmpOperator::Le, "a", 2u32);
        let data = HashMap::from([("a", 1)]);
        assert!(express.evaluate(&data));
        let data = HashMap::from([("a", 2)]);
        assert!(express.evaluate(&data));
        let data = HashMap::from([("a", 3)]);
        assert!(!express.evaluate(&data));
        let data = HashMap::from([("b", 1)]);
        assert!(!express.evaluate(&data));
    }

    #[test]
    pub fn test_rel_ops_u32() {
        // a > 5
        let gt = CompareExpress::new(CmpOperator::Gt, "a", 5u32);
        assert!(gt.evaluate(&HashMap::from([("a", 6u32)])));
        assert!(!gt.evaluate(&HashMap::from([("a", 5u32)])));
        assert!(!gt.evaluate(&HashMap::from([("a", 4u32)])));

        // a >= 5
        let ge = CompareExpress::new(CmpOperator::Ge, "a", 5u32);
        assert!(ge.evaluate(&HashMap::from([("a", 6u32)])));
        assert!(ge.evaluate(&HashMap::from([("a", 5u32)])));
        assert!(!ge.evaluate(&HashMap::from([("a", 4u32)])));

        // a < 5
        let lt = CompareExpress::new(CmpOperator::Lt, "a", 5u32);
        assert!(lt.evaluate(&HashMap::from([("a", 4u32)])));
        assert!(!lt.evaluate(&HashMap::from([("a", 5u32)])));
        assert!(!lt.evaluate(&HashMap::from([("a", 6u32)])));

        // a <= 5
        let le = CompareExpress::new(CmpOperator::Le, "a", 5u32);
        assert!(le.evaluate(&HashMap::from([("a", 5u32)])));
        assert!(le.evaluate(&HashMap::from([("a", 4u32)])));
        assert!(!le.evaluate(&HashMap::from([("a", 6u32)])));
    }

    #[test]
    pub fn test_rel_ops_f64() {
        type CE = CompareExpress<f64>;
        // a > 1.5
        let gt = CE::new(CmpOperator::Gt, "a", 1.5f64);
        assert!(gt.evaluate(&HashMap::from([("a", 1.6f64)])));
        assert!(!gt.evaluate(&HashMap::from([("a", 1.5f64)])));
        assert!(!gt.evaluate(&HashMap::from([("a", 1.4f64)])));

        // a >= 1.5
        let ge = CE::new(CmpOperator::Ge, "a", 1.5f64);
        assert!(ge.evaluate(&HashMap::from([("a", 1.6f64)])));
        assert!(ge.evaluate(&HashMap::from([("a", 1.5f64)])));
        assert!(!ge.evaluate(&HashMap::from([("a", 1.4f64)])));

        // a < 1.5
        let lt = CE::new(CmpOperator::Lt, "a", 1.5f64);
        assert!(lt.evaluate(&HashMap::from([("a", 1.4f64)])));
        assert!(!lt.evaluate(&HashMap::from([("a", 1.5f64)])));
        assert!(!lt.evaluate(&HashMap::from([("a", 1.6f64)])));

        // a <= 1.5
        let le = CE::new(CmpOperator::Le, "a", 1.5f64);
        assert!(le.evaluate(&HashMap::from([("a", 1.5f64)])));
        assert!(le.evaluate(&HashMap::from([("a", 1.4f64)])));
        assert!(!le.evaluate(&HashMap::from([("a", 1.6f64)])));
    }

    #[test]
    pub fn test_rel_ops_string() {
        type Ces = CompareExpress<String>;
        // lexicographic order
        let gt = Ces::new(CmpOperator::Gt, "a", "b".to_string());
        assert!(gt.evaluate(&HashMap::from([("a", "c".to_string())])));
        assert!(!gt.evaluate(&HashMap::from([("a", "b".to_string())])));
        assert!(!gt.evaluate(&HashMap::from([("a", "a".to_string())])));

        let lt = Ces::new(CmpOperator::Lt, "a", "b".to_string());
        assert!(lt.evaluate(&HashMap::from([("a", "a".to_string())])));
        assert!(!lt.evaluate(&HashMap::from([("a", "b".to_string())])));
        assert!(!lt.evaluate(&HashMap::from([("a", "c".to_string())])));
    }

    #[test]
    pub fn test_logic_not_cases() {
        // NOT with left=None
        let t = CompareExpress::new(CmpOperator::Eq, "a", 1u32);
        let not_true = LogicalExpress::<u32, RustSymbol>::new(
            LogicOperator::Not,
            None,
            Expression::Compare(t.clone()),
        );
        assert!(!not_true.evaluate(&HashMap::from([("a", 1u32)])));
        assert!(not_true.evaluate(&HashMap::from([("a", 2u32)])));

        // NOT with left=Some(...) should ignore left
        let dummy_left = Expression::Compare(Comparison::new(CmpOperator::Eq, "x", 0u32));
        let not_with_left = LogicalExpress::<u32, RustSymbol>::new(
            LogicOperator::Not,
            Some(dummy_left),
            Expression::Compare(t),
        );
        assert!(!not_with_left.evaluate(&HashMap::from([("a", 1u32)])));
        assert!(not_with_left.evaluate(&HashMap::from([("a", 2u32)])));
    }

    #[test]
    pub fn test_string_we_patterns() {
        type Ces = CompareExpress<String>;
        // patterns: prefix, suffix, middle, exact, not-match
        let prefix = Ces::new(CmpOperator::We, "a", "he*".to_string());
        assert!(prefix.evaluate(&HashMap::from([("a", "hello".to_string())])));
        assert!(!prefix.evaluate(&HashMap::from([("a", "xhello".to_string())])));

        let suffix = Ces::new(CmpOperator::We, "a", "*lo".to_string());
        assert!(suffix.evaluate(&HashMap::from([("a", "hello".to_string())])));
        assert!(!suffix.evaluate(&HashMap::from([("a", "hellx".to_string())])));

        let middle = Ces::new(CmpOperator::We, "a", "h*o".to_string());
        assert!(middle.evaluate(&HashMap::from([("a", "hello".to_string())])));
        assert!(!middle.evaluate(&HashMap::from([("a", "hezz".to_string())])));

        let exact = Ces::new(CmpOperator::We, "a", "hello".to_string());
        assert!(exact.evaluate(&HashMap::from([("a", "hello".to_string())])));
        assert!(!exact.evaluate(&HashMap::from([("a", "Hello".to_string())])));
    }
}
