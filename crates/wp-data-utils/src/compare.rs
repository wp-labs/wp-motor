use orion_exp::CmpOperator;
use orion_exp::evaluator::default_compare;
use wp_model_core::model::{DataField, Value};

/// Compare two DataField values according to the same semantics as legacy ValueComparator<Field<T>>
pub fn compare_datafield(left: &DataField, right: &DataField, op: CmpOperator) -> bool {
    // Special: RHS Ignore means existence test
    if matches!(right.get_value(), Value::Ignore(_)) {
        return true;
    }
    // Type mismatch -> false
    if std::mem::discriminant(left.get_value()) != std::mem::discriminant(right.get_value()) {
        return false;
    }
    match (left.get_value(), right.get_value()) {
        (Value::Chars(v1), Value::Chars(v2)) => default_compare(v1, v2, op),
        (Value::Symbol(v1), Value::Symbol(v2)) => default_compare(v1, v2, op),
        (Value::Time(v1), Value::Time(v2)) => default_compare(v1, v2, op),
        (Value::Bool(v1), Value::Bool(v2)) => default_compare(v1, v2, op),
        (Value::Digit(v1), Value::Digit(v2)) => default_compare(v1, v2, op),
        (Value::Hex(v1), Value::Hex(v2)) => default_compare(&v1.0, &v2.0, op),
        (Value::Float(v1), Value::Float(v2)) => default_compare(v1, v2, op),
        (Value::IpNet(v1), Value::IpNet(v2)) => match op {
            CmpOperator::Eq => v1 == v2,
            CmpOperator::Ne => v1 != v2,
            _ => false,
        },
        (Value::IpAddr(v1), Value::IpAddr(v2)) => default_compare(v1, v2, op),
        (Value::Domain(v1), Value::Domain(v2)) => default_compare(&v1.0, &v2.0, op),
        (Value::Email(v1), Value::Email(v2)) => default_compare(&v1.0, &v2.0, op),
        (Value::Url(v1), Value::Url(v2)) => default_compare(&v1.0, &v2.0, op),
        (Value::IdCard(v1), Value::IdCard(v2)) => default_compare(&v1.0, &v2.0, op),
        (Value::MobilePhone(v1), Value::MobilePhone(v2)) => default_compare(&v1.0, &v2.0, op),
        (Value::Ignore(_), Value::Ignore(_)) => true,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}
