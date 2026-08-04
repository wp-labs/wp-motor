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
        // 任意精度整数：BigUint 不支持通配符（default_compare 要求 WildcardMatcher），手动比较
        (Value::BigUint(v1), Value::BigUint(v2)) => match op {
            CmpOperator::Eq => v1 == v2,
            CmpOperator::Ne => v1 != v2,
            CmpOperator::Gt => v1 > v2,
            CmpOperator::Ge => v1 >= v2,
            CmpOperator::Lt => v1 < v2,
            CmpOperator::Le => v1 <= v2,
            CmpOperator::We => false,
        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use std::str::FromStr;
    use wp_model_core::model::{DataType, Field, Value};

    fn biguint_field(s: &str) -> DataField {
        Field::new(
            DataType::BigInt,
            "x",
            Value::BigUint(BigUint::from_str(s).unwrap()),
        )
    }

    #[test]
    fn test_compare_biguint_interval() {
        // 区间判断：ipv6_min <= x <= ipv6_max
        let x = biguint_field("382824323044708348099391746388336347272");
        let beg = biguint_field("340282366920938463463374607431768211456"); // 2^128
        let end = biguint_field("680564733841876926926749214863536422911"); // 2^129-1

        assert!(compare_datafield(&x, &beg, CmpOperator::Ge));
        assert!(compare_datafield(&x, &end, CmpOperator::Le));
        assert!(compare_datafield(&beg, &x, CmpOperator::Lt));
        assert!(compare_datafield(&end, &x, CmpOperator::Gt));
        assert!(compare_datafield(&x, &x, CmpOperator::Eq));
        assert!(!compare_datafield(&x, &beg, CmpOperator::Eq));
        assert!(!compare_datafield(&x, &beg, CmpOperator::We));
    }

    #[test]
    fn test_compare_biguint_type_mismatch_is_false() {
        // 类型不一致（BigUint vs Digit）不匹配
        let left = biguint_field("134744072");
        let right = Field::from_digit("x", 134744072);
        assert!(!compare_datafield(&left, &right, CmpOperator::Eq));
    }
}
