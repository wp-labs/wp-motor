use crate::core::AsyncFieldExtractor;
use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::prelude::*;
use crate::language::{CalcExpr, CalcFun, CalcNumber, CalcOp, CalcOperation};
use async_trait::async_trait;
use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

#[derive(Debug, Clone)]
enum CalcEvalError {
    DivideByZero,
    ModByZero,
    Overflow(&'static str),
    NonFinite(&'static str),
    MissingOperand(String),
    NonNumericOperand(String),
    InvalidModOperand,
    InvalidArgument(String),
}

impl CalcEvalError {
    fn detail(&self) -> String {
        match self {
            CalcEvalError::DivideByZero => "math_divide_by_zero".to_string(),
            CalcEvalError::ModByZero => "math_mod_by_zero".to_string(),
            CalcEvalError::Overflow(op) => format!("math_overflow: {}", op),
            CalcEvalError::NonFinite(op) => format!("math_non_finite: {}", op),
            CalcEvalError::MissingOperand(name) => format!("math_missing_operand: {}", name),
            CalcEvalError::NonNumericOperand(detail) => {
                format!("math_non_numeric_operand: {}", detail)
            }
            CalcEvalError::InvalidModOperand => "math_invalid_mod_operand".to_string(),
            CalcEvalError::InvalidArgument(detail) => format!("math_invalid_argument: {}", detail),
        }
    }
}

impl CalcNumber {
    fn ensure_finite(self, op: &'static str) -> Result<Self, CalcEvalError> {
        match self {
            CalcNumber::Digit(_) => Ok(self),
            CalcNumber::Float(v) if v.is_finite() => Ok(CalcNumber::Float(v)),
            CalcNumber::Float(_) => Err(CalcEvalError::NonFinite(op)),
        }
    }

    fn negate(self) -> Result<Self, CalcEvalError> {
        match self {
            CalcNumber::Digit(v) => v
                .checked_neg()
                .map(CalcNumber::Digit)
                .ok_or(CalcEvalError::Overflow("neg")),
            CalcNumber::Float(v) => CalcNumber::Float(-v).ensure_finite("neg"),
        }
    }

    fn into_field(self, name: impl Into<String>) -> DataField {
        let name = name.into();
        match self {
            CalcNumber::Digit(v) => DataField::from_digit(name, v),
            CalcNumber::Float(v) => DataField::from_float(name, v),
        }
    }
}

fn operand_target(
    accessor: &crate::language::DirectAccessor,
    fallback: &EvaluationTarget,
) -> EvaluationTarget {
    let key = accessor
        .field_name()
        .clone()
        .unwrap_or_else(|| fallback.safe_name());
    EvaluationTarget::new(key, DataType::Auto)
}

fn field_to_number(field: DataField) -> Result<CalcNumber, CalcEvalError> {
    match field.get_value() {
        Value::Digit(v) => Ok(CalcNumber::Digit(*v)),
        Value::Float(v) => CalcNumber::Float(*v).ensure_finite("operand"),
        Value::Ignore(_) => Err(CalcEvalError::MissingOperand(field.get_name().to_string())),
        _ => Err(CalcEvalError::NonNumericOperand(format!(
            "field={} type={}",
            field.get_name(),
            field.get_meta()
        ))),
    }
}

fn to_i64_checked(value: f64, fun: &str) -> Result<i64, CalcEvalError> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(CalcEvalError::InvalidArgument(format!(
            "{} result out of i64 range",
            fun
        )));
    }
    Ok(value as i64)
}

fn eval_binary(op: &CalcOp, lhs: CalcNumber, rhs: CalcNumber) -> Result<CalcNumber, CalcEvalError> {
    match op {
        CalcOp::Add => match (lhs, rhs) {
            (CalcNumber::Digit(a), CalcNumber::Digit(b)) => a
                .checked_add(b)
                .map(CalcNumber::Digit)
                .ok_or(CalcEvalError::Overflow("add")),
            (CalcNumber::Digit(a), CalcNumber::Float(b)) => {
                CalcNumber::Float(a as f64 + b).ensure_finite("add")
            }
            (CalcNumber::Float(a), CalcNumber::Digit(b)) => {
                CalcNumber::Float(a + b as f64).ensure_finite("add")
            }
            (CalcNumber::Float(a), CalcNumber::Float(b)) => {
                CalcNumber::Float(a + b).ensure_finite("add")
            }
        },
        CalcOp::Sub => match (lhs, rhs) {
            (CalcNumber::Digit(a), CalcNumber::Digit(b)) => a
                .checked_sub(b)
                .map(CalcNumber::Digit)
                .ok_or(CalcEvalError::Overflow("sub")),
            (CalcNumber::Digit(a), CalcNumber::Float(b)) => {
                CalcNumber::Float(a as f64 - b).ensure_finite("sub")
            }
            (CalcNumber::Float(a), CalcNumber::Digit(b)) => {
                CalcNumber::Float(a - b as f64).ensure_finite("sub")
            }
            (CalcNumber::Float(a), CalcNumber::Float(b)) => {
                CalcNumber::Float(a - b).ensure_finite("sub")
            }
        },
        CalcOp::Mul => match (lhs, rhs) {
            (CalcNumber::Digit(a), CalcNumber::Digit(b)) => a
                .checked_mul(b)
                .map(CalcNumber::Digit)
                .ok_or(CalcEvalError::Overflow("mul")),
            (CalcNumber::Digit(a), CalcNumber::Float(b)) => {
                CalcNumber::Float(a as f64 * b).ensure_finite("mul")
            }
            (CalcNumber::Float(a), CalcNumber::Digit(b)) => {
                CalcNumber::Float(a * b as f64).ensure_finite("mul")
            }
            (CalcNumber::Float(a), CalcNumber::Float(b)) => {
                CalcNumber::Float(a * b).ensure_finite("mul")
            }
        },
        CalcOp::Div => {
            let denom = match rhs {
                CalcNumber::Digit(v) => v as f64,
                CalcNumber::Float(v) => v,
            };
            if denom == 0.0 {
                return Err(CalcEvalError::DivideByZero);
            }
            let numer = match lhs {
                CalcNumber::Digit(v) => v as f64,
                CalcNumber::Float(v) => v,
            };
            CalcNumber::Float(numer / denom).ensure_finite("div")
        }
        CalcOp::Mod => match (lhs, rhs) {
            (CalcNumber::Digit(_), CalcNumber::Digit(0)) => Err(CalcEvalError::ModByZero),
            (CalcNumber::Digit(a), CalcNumber::Digit(b)) => a
                .checked_rem(b)
                .map(CalcNumber::Digit)
                .ok_or(CalcEvalError::Overflow("mod")),
            _ => Err(CalcEvalError::InvalidModOperand),
        },
    }
}

fn eval_fun(fun: &CalcFun, arg: CalcNumber) -> Result<CalcNumber, CalcEvalError> {
    match fun {
        CalcFun::Abs => match arg {
            CalcNumber::Digit(v) => v
                .checked_abs()
                .map(CalcNumber::Digit)
                .ok_or(CalcEvalError::Overflow("abs")),
            CalcNumber::Float(v) => CalcNumber::Float(v.abs()).ensure_finite("abs"),
        },
        CalcFun::Round => match arg {
            CalcNumber::Digit(v) => Ok(CalcNumber::Digit(v)),
            CalcNumber::Float(v) => Ok(CalcNumber::Digit(to_i64_checked(v.round(), "round")?)),
        },
        CalcFun::Floor => match arg {
            CalcNumber::Digit(v) => Ok(CalcNumber::Digit(v)),
            CalcNumber::Float(v) => Ok(CalcNumber::Digit(to_i64_checked(v.floor(), "floor")?)),
        },
        CalcFun::Ceil => match arg {
            CalcNumber::Digit(v) => Ok(CalcNumber::Digit(v)),
            CalcNumber::Float(v) => Ok(CalcNumber::Digit(to_i64_checked(v.ceil(), "ceil")?)),
        },
    }
}

fn eval_expr(
    expr: &CalcExpr,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> Result<CalcNumber, CalcEvalError> {
    match expr {
        CalcExpr::Const(v) => Ok(v.clone()),
        CalcExpr::Accessor(accessor) => {
            let tmp_target = operand_target(accessor, target);
            let field = accessor
                .extract_one(&tmp_target, src, dst)
                .ok_or_else(|| CalcEvalError::MissingOperand(tmp_target.safe_name()))?;
            field_to_number(field)
        }
        CalcExpr::UnaryNeg(inner) => eval_expr(inner, target, src, dst)?.negate(),
        CalcExpr::Binary { op, lhs, rhs } => {
            let left = eval_expr(lhs, target, src, dst)?;
            let right = eval_expr(rhs, target, src, dst)?;
            eval_binary(op, left, right)
        }
        CalcExpr::Func { fun, arg } => {
            let value = eval_expr(arg, target, src, dst)?;
            eval_fun(fun, value)
        }
    }
}

impl CalcOperation {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match eval_expr(self.expr(), target, src, dst) {
            Ok(number) => Some(number.into_field(target.safe_name())),
            Err(err) => {
                warn_data!("calc {} failed: {}", self.expr(), err.detail());
                diagnostics::push(OmlIssue::new(OmlIssueKind::MathEvalFail, err.detail()));
                Some(DataField::from_ignore(target.safe_name()))
            }
        }
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
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
impl AsyncFieldExtractor for CalcOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one(target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_storage(target, src, dst)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::dev::testing::TestAssert;
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage, Value};

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_mixed_numeric_expression() {
        let mut conf = r#"
name : test
---
risk_score : float = calc(read(cpu) * 0.7 + read(mem) * 0.3);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("cpu", 10)),
            FieldStorage::from_owned(DataField::from_digit("mem", 20)),
        ]);
        let target = model.transform_async(src, cache).await;

        let out = target
            .field("risk_score")
            .expect("risk_score field")
            .as_field();
        assert_eq!(out.get_meta(), &DataType::Float);
        assert_eq!(out.get_value(), &Value::Float(13.0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_operator_precedence() {
        let mut conf = r#"
name : test
---
result : digit = calc(1 + 2 * 3);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let target = model.transform_async(DataRecord::default(), cache).await;

        let out = target.field("result").expect("result field").as_field();
        assert_eq!(out.get_value(), &Value::Digit(7));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_functions() {
        let mut conf = r#"
name : test
---
a : digit = calc(abs(read(x)));
b : digit = calc(round(read(y)));
c : digit = calc(floor(read(z)));
d : digit = calc(ceil(read(w)));
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("x", -3)),
            FieldStorage::from_owned(DataField::from_float("y", 1.6)),
            FieldStorage::from_owned(DataField::from_float("z", 1.8)),
            FieldStorage::from_owned(DataField::from_float("w", 1.2)),
        ]);
        let target = model.transform_async(src, cache).await;

        assert_eq!(
            target.field("a").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(3))
        );
        assert_eq!(
            target.field("b").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(2))
        );
        assert_eq!(
            target.field("c").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(1))
        );
        assert_eq!(
            target.field("d").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(2))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_divide_by_zero_returns_ignore() {
        let mut conf = r#"
name : test
---
ratio : float = calc(read(a) / read(b));
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("a", 10)),
            FieldStorage::from_owned(DataField::from_digit("b", 0)),
        ]);
        let target = model.transform_async(src, cache).await;

        let out = target.field("ratio").expect("ratio field").as_field();
        assert_eq!(out.get_meta(), &DataType::Ignore);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_missing_or_non_numeric_returns_ignore() {
        let mut conf = r#"
name : test
---
missing_case = calc(read(a) + read(b));
bad_case = calc(read(status) + 1);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("a", 10)),
            FieldStorage::from_owned(DataField::from_chars("status", "ok")),
        ]);
        let target = model.transform_async(src, cache).await;

        assert_eq!(
            target
                .field("missing_case")
                .map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
        assert_eq!(
            target.field("bad_case").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_mod_success_and_float_mod_returns_ignore() {
        let mut conf = r#"
name : test
---
bucket : digit = calc(read(uid) % 16);
bad_mod = calc(read(rate) % 2);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("uid", 35)),
            FieldStorage::from_owned(DataField::from_float("rate", 3.5)),
        ]);
        let target = model.transform_async(src, cache).await;

        assert_eq!(
            target.field("bucket").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(3))
        );
        assert_eq!(
            target.field("bad_mod").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_integer_overflow_returns_ignore() {
        let mut conf = r#"
name : test
---
add_overflow = calc(9223372036854775807 + 1);
mul_overflow = calc(9223372036854775807 * 2);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let target = model.transform_async(DataRecord::default(), cache).await;

        assert_eq!(
            target
                .field("add_overflow")
                .map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
        assert_eq!(
            target
                .field("mul_overflow")
                .map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_min_value_abs_neg_and_mod_overflow_return_ignore() {
        let mut conf = r#"
name : test
---
neg_case : digit = calc(-read(min_v));
abs_case : digit = calc(abs(read(min_v)));
mod_case : digit = calc(read(min_v) % read(neg_one));
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("min_v", i64::MIN)),
            FieldStorage::from_owned(DataField::from_digit("neg_one", -1)),
        ]);
        let target = model.transform_async(src, cache).await;

        assert_eq!(
            target.field("neg_case").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
        assert_eq!(
            target.field("abs_case").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
        assert_eq!(
            target.field("mod_case").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_round_floor_ceil_preserve_large_digit_input() {
        let mut conf = r#"
name : test
---
round_case : digit = calc(round(read(big)));
floor_case : digit = calc(floor(read(big)));
ceil_case : digit = calc(ceil(read(big)));
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_digit(
            "big",
            9_007_199_254_740_993,
        ))]);
        let target = model.transform_async(src, cache).await;

        assert_eq!(
            target.field("round_case").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(9_007_199_254_740_993))
        );
        assert_eq!(
            target.field("floor_case").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(9_007_199_254_740_993))
        );
        assert_eq!(
            target.field("ceil_case").map(|s| s.as_field().get_value()),
            Some(&Value::Digit(9_007_199_254_740_993))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_non_finite_input_and_result_return_ignore() {
        let mut conf = r#"
name : test
---
bad_input = calc(read(bad) + 1);
bad_abs = calc(abs(read(nan_v)));
bad_mul = calc(read(huge1) * read(huge2));
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_float("bad", f64::INFINITY)),
            FieldStorage::from_owned(DataField::from_float("nan_v", f64::NAN)),
            FieldStorage::from_owned(DataField::from_float("huge1", 1.0e308)),
            FieldStorage::from_owned(DataField::from_float("huge2", 1.0e308)),
        ]);
        let target = model.transform_async(src, cache).await;

        assert_eq!(
            target.field("bad_input").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
        assert_eq!(
            target.field("bad_abs").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
        assert_eq!(
            target.field("bad_mul").map(|s| s.as_field().get_meta()),
            Some(&DataType::Ignore)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_calc_mixed_numeric_expression_async() {
        let mut conf = r#"
name : test
---
risk_score : float = calc(read(cpu) * 0.7 + read(mem) * 0.3);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_digit("cpu", 10)),
            FieldStorage::from_owned(DataField::from_digit("mem", 20)),
        ]);
        let target = model.transform_async(src, cache).await;

        let out = target
            .field("risk_score")
            .expect("risk_score field")
            .as_field();
        assert_eq!(out.get_meta(), &DataType::Float);
        assert_eq!(out.get_value(), &Value::Float(13.0));
    }
}
