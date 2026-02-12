use crate::language::prelude::*;
use crate::language::syntax::accessors::NestedAccessor;
use crate::types::AnyResult;
use derive_getters::Getters;
use orion_exp::CmpOperator;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use wp_data_model::compare::compare_datafield;
use wp_model_core::model::{DataField, DataType};
use wpl::DataTypeParser;

/// Match function wrapper for pattern matching with pipe functions
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MatchFun {
    pub name: String,
    pub args: Vec<String>,
}

impl MatchFun {
    pub fn new(name: impl Into<String>, arg: Option<impl Into<String>>) -> Self {
        Self {
            name: name.into(),
            args: arg.map(|a| vec![a.into()]).unwrap_or_default(),
        }
    }

    pub fn new_with_args(name: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }

    pub fn starts_with(prefix: impl Into<String>) -> Self {
        Self::new("starts_with", Some(prefix))
    }

    pub fn arg(&self) -> Option<&String> {
        self.args.first()
    }

    pub fn arg_at(&self, index: usize) -> Option<&String> {
        self.args.get(index)
    }
}

impl Display for MatchFun {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.args.is_empty() {
            write!(f, "{}()", self.name)
        } else if self.args.len() == 1 {
            // Check if argument is numeric (no quotes needed)
            let arg = &self.args[0];
            if arg.parse::<f64>().is_ok() {
                write!(f, "{}({})", self.name, arg)
            } else {
                // Don't escape - quot_str returns raw content with escape sequences intact
                write!(f, "{}('{}')", self.name, arg)
            }
        } else {
            write!(f, "{}(", self.name)?;
            for (i, arg) in self.args.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                // Check if argument is numeric (no quotes needed)
                if arg.parse::<f64>().is_ok() {
                    write!(f, "{}", arg)?;
                } else {
                    // Don't escape - quot_str returns raw content with escape sequences intact
                    write!(f, "'{}'", arg)?;
                }
            }
            write!(f, ")")
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MatchCond {
    Eq(DataField),
    Neq(DataField),
    In(DataField, DataField),
    /// Function-based matching - matches if function returns non-ignore field
    Fun(MatchFun),

    /// Arc-based variants for static symbols (zero-copy reference)
    /// These are created during rewrite phase to share DataField instances
    #[serde(skip)]
    EqArc(Arc<DataField>),
    #[serde(skip)]
    NeqArc(Arc<DataField>),
    #[serde(skip)]
    InArc(Arc<DataField>, Arc<DataField>),

    /// Static symbol reference - will be replaced with Arc variants during parse
    /// These variants should not exist after rewrite_static_references()
    EqSym(String),
    NeqSym(String),
    InSym(String, String),

    Default,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MatchCondition {
    Single(MatchCond),
    Double(MatchCond, MatchCond),
    Triple(MatchCond, MatchCond, MatchCond),
    Quadruple(MatchCond, MatchCond, MatchCond, MatchCond),
    Default,
}

impl Display for MatchCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchCondition::Single(x) => {
                write!(f, "{}", x)?;
            }
            MatchCondition::Double(fst, sec) => {
                write!(f, "({}, {})", fst, sec)?;
            }
            MatchCondition::Triple(a, b, c) => {
                write!(f, "({}, {}, {})", a, b, c)?;
            }
            MatchCondition::Quadruple(a, b, c, d) => {
                write!(f, "({}, {}, {}, {})", a, b, c, d)?;
            }
            MatchCondition::Default => {
                write!(f, "_")?;
            }
        }
        Ok(())
    }
}

pub trait MatchAble<T> {
    fn is_match(&self, value: T) -> bool;
}

impl MatchAble<&DataField> for MatchCond {
    fn is_match(&self, value: &DataField) -> bool {
        match self {
            MatchCond::Eq(x) => {
                if compare_datafield(value, x, CmpOperator::Eq) {
                    return true;
                }
                if x.get_meta() == value.get_meta() {
                    return false;
                }
                warn_data!(
                    "not same type data: {}({}): {}, expect: {}",
                    value.get_name(),
                    value.get_meta(),
                    value.get_value(),
                    x.get_meta()
                );
                false
            }
            MatchCond::Neq(x) => {
                if compare_datafield(value, x, CmpOperator::Ne) {
                    return true;
                }
                if x.get_meta() == value.get_meta() {
                    return false;
                }
                warn_data!(
                    "not same type data: {}({}): {}, expect: {}",
                    value.get_name(),
                    value.get_meta(),
                    value.get_value(),
                    x.get_meta()
                );
                false
            }
            MatchCond::In(beg, end) => {
                // Expect: value in [beg, end]  => (value >= beg) && (value <= end)
                if compare_datafield(value, beg, CmpOperator::Ge)
                    && compare_datafield(value, end, CmpOperator::Le)
                {
                    return true;
                }
                if beg.get_meta() == end.get_meta() && beg.get_meta() == value.get_meta() {
                    return false;
                }
                warn_data!(
                    "not same type data: {}({}): {}, expect: {}",
                    value.get_name(),
                    value.get_meta(),
                    value.get_value(),
                    beg.get_meta()
                );
                false
            }
            MatchCond::Fun(fun) => {
                // Execute the function and check if result is not ignore
                match_with_function(value, fun)
            }

            // Arc-based variants (for static symbols, zero-copy)
            MatchCond::EqArc(x) => {
                if compare_datafield(value, x.as_ref(), CmpOperator::Eq) {
                    return true;
                }
                if x.get_meta() == value.get_meta() {
                    return false;
                }
                warn_data!(
                    "not same type data: {}({}): {}, expect: {}",
                    value.get_name(),
                    value.get_meta(),
                    value.get_value(),
                    x.get_meta()
                );
                false
            }
            MatchCond::NeqArc(x) => {
                if compare_datafield(value, x.as_ref(), CmpOperator::Ne) {
                    return true;
                }
                if x.get_meta() == value.get_meta() {
                    return false;
                }
                warn_data!(
                    "not same type data: {}({}): {}, expect: {}",
                    value.get_name(),
                    value.get_meta(),
                    value.get_value(),
                    x.get_meta()
                );
                false
            }
            MatchCond::InArc(beg, end) => {
                // Expect: value in [beg, end]  => (value >= beg) && (value <= end)
                if compare_datafield(value, beg.as_ref(), CmpOperator::Ge)
                    && compare_datafield(value, end.as_ref(), CmpOperator::Le)
                {
                    return true;
                }
                if beg.get_meta() == end.get_meta() && beg.get_meta() == value.get_meta() {
                    return false;
                }
                warn_data!(
                    "not same type data: {}({}): {}, expect: {}",
                    value.get_name(),
                    value.get_meta(),
                    value.get_value(),
                    beg.get_meta()
                );
                false
            }

            // Symbol references should have been replaced during parse
            // If we see them here, it's a bug in the rewrite logic
            MatchCond::EqSym(sym) | MatchCond::NeqSym(sym) => {
                warn_data!("Unresolved static symbol '{}' in match condition", sym);
                false
            }
            MatchCond::InSym(beg_sym, end_sym) => {
                warn_data!(
                    "Unresolved static symbols '{}', '{}' in match condition",
                    beg_sym,
                    end_sym
                );
                false
            }

            MatchCond::Default => true,
        }
    }
}

/// Helper function to extract numeric value from DataField
fn extract_numeric(value: &DataField) -> Option<f64> {
    use wp_model_core::model::Value;
    match value.get_value() {
        Value::Digit(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        Value::Chars(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Helper function to parse numeric string
fn parse_numeric(s: &str) -> Option<f64> {
    s.parse::<f64>().ok()
}

/// Helper function for numeric comparison
fn numeric_compare<F>(value: &DataField, threshold_str: &str, op: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    if let Some(val) = extract_numeric(value) {
        if let Some(threshold) = parse_numeric(threshold_str) {
            return op(val, threshold);
        } else {
            warn_data!("invalid numeric threshold: {}", threshold_str);
        }
    }
    false
}

/// Helper function for numeric range check
fn numeric_in_range(value: &DataField, min_str: &str, max_str: &str) -> bool {
    if let Some(val) = extract_numeric(value) {
        if let Some(min) = parse_numeric(min_str) {
            if let Some(max) = parse_numeric(max_str) {
                return val >= min && val <= max;
            } else {
                warn_data!("invalid max value: {}", max_str);
            }
        } else {
            warn_data!("invalid min value: {}", min_str);
        }
    }
    false
}

/// Execute a match function and determine if it matches
fn match_with_function(value: &DataField, fun: &MatchFun) -> bool {
    use wp_model_core::model::Value;

    match fun.name.as_str() {
        "starts_with" => {
            if let Some(prefix) = fun.arg() {
                if let Value::Chars(s) = value.get_value() {
                    s.starts_with(prefix)
                } else {
                    false
                }
            } else {
                warn_data!("starts_with function requires a prefix argument");
                false
            }
        }
        "ends_with" => {
            if let Some(suffix) = fun.arg() {
                if let Value::Chars(s) = value.get_value() {
                    s.ends_with(suffix)
                } else {
                    false
                }
            } else {
                warn_data!("ends_with function requires a suffix argument");
                false
            }
        }
        "contains" => {
            if let Some(substring) = fun.arg() {
                if let Value::Chars(s) = value.get_value() {
                    s.contains(substring.as_str())
                } else {
                    false
                }
            } else {
                warn_data!("contains function requires a substring argument");
                false
            }
        }
        "regex_match" => {
            if let Some(pattern) = fun.arg() {
                if let Value::Chars(s) = value.get_value() {
                    match regex::Regex::new(pattern) {
                        Ok(re) => re.is_match(s),
                        Err(e) => {
                            warn_data!("invalid regex pattern '{}': {}", pattern, e);
                            false
                        }
                    }
                } else {
                    false
                }
            } else {
                warn_data!("regex_match function requires a pattern argument");
                false
            }
        }
        "is_empty" => {
            if let Value::Chars(s) = value.get_value() {
                s.is_empty()
            } else {
                // Non-string values are considered non-empty
                false
            }
        }
        "iequals" => {
            if let Some(compare_val) = fun.arg() {
                if let Value::Chars(s) = value.get_value() {
                    s.to_lowercase() == compare_val.to_lowercase()
                } else {
                    false
                }
            } else {
                warn_data!("iequals function requires a value argument");
                false
            }
        }
        // Numeric comparison functions
        "gt" => {
            if let Some(threshold) = fun.arg() {
                numeric_compare(value, threshold, |a, b| a > b)
            } else {
                warn_data!("gt function requires a numeric argument");
                false
            }
        }
        "lt" => {
            if let Some(threshold) = fun.arg() {
                numeric_compare(value, threshold, |a, b| a < b)
            } else {
                warn_data!("lt function requires a numeric argument");
                false
            }
        }
        "eq" => {
            if let Some(target) = fun.arg() {
                numeric_compare(value, target, |a, b| (a - b).abs() < 1e-10)
            } else {
                warn_data!("eq function requires a numeric argument");
                false
            }
        }
        "in_range" => {
            if fun.args.len() >= 2 {
                let min_val = &fun.args[0];
                let max_val = &fun.args[1];
                numeric_in_range(value, min_val, max_val)
            } else {
                warn_data!("in_range function requires two numeric arguments (min, max)");
                false
            }
        }
        _ => {
            warn_data!("unsupported match function: {}", fun.name);
            false
        }
    }
}

impl Display for MatchCond {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchCond::Eq(x) => {
                write!(f, " {}  ", x)?;
            }

            MatchCond::Neq(x) => {
                write!(f, " !{}  ", x)?;
            }
            MatchCond::In(a, b) => {
                write!(f, "in ( {}, {} )", a, b)?;
            }
            MatchCond::Fun(fun) => {
                write!(f, " {}  ", fun)?;
            }

            // Arc-based variants (same display as regular variants)
            MatchCond::EqArc(x) => {
                write!(f, " {}  ", x)?;
            }
            MatchCond::NeqArc(x) => {
                write!(f, " !{}  ", x)?;
            }
            MatchCond::InArc(a, b) => {
                write!(f, "in ( {}, {} )", a, b)?;
            }

            // Static symbol references (before rewrite)
            MatchCond::EqSym(sym) => {
                write!(f, " {}  ", sym)?;
            }
            MatchCond::NeqSym(sym) => {
                write!(f, " !{}  ", sym)?;
            }
            MatchCond::InSym(beg_sym, end_sym) => {
                write!(f, "in ( {}, {} )", beg_sym, end_sym)?;
            }

            MatchCond::Default => {
                write!(f, " _ ")?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Getters, PartialEq)]
pub struct MatchCase {
    cond: MatchCondition,
    result: NestedAccessor,
}
impl Display for MatchCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} => {} ", self.cond, self.result)
    }
}
impl MatchAble<&DataField> for MatchCondition {
    fn is_match(&self, value: &DataField) -> bool {
        match self {
            MatchCondition::Single(s) => s.is_match(value),
            MatchCondition::Double(_, _)
            | MatchCondition::Triple(_, _, _)
            | MatchCondition::Quadruple(_, _, _, _) => {
                unreachable!()
            }
            MatchCondition::Default => true,
        }
    }
}

impl MatchAble<(&DataField, &DataField)> for MatchCondition {
    fn is_match(&self, value: (&DataField, &DataField)) -> bool {
        match self {
            MatchCondition::Single(_)
            | MatchCondition::Triple(_, _, _)
            | MatchCondition::Quadruple(_, _, _, _) => {
                unreachable!()
            }
            MatchCondition::Double(fst, sec) => fst.is_match(value.0) && sec.is_match(value.1),
            MatchCondition::Default => true,
        }
    }
}

impl MatchAble<(&DataField, &DataField, &DataField)> for MatchCondition {
    fn is_match(&self, value: (&DataField, &DataField, &DataField)) -> bool {
        match self {
            MatchCondition::Triple(a, b, c) => {
                a.is_match(value.0) && b.is_match(value.1) && c.is_match(value.2)
            }
            MatchCondition::Default => true,
            _ => unreachable!(),
        }
    }
}

impl MatchAble<(&DataField, &DataField, &DataField, &DataField)> for MatchCondition {
    fn is_match(&self, value: (&DataField, &DataField, &DataField, &DataField)) -> bool {
        match self {
            MatchCondition::Quadruple(a, b, c, d) => {
                a.is_match(value.0)
                    && b.is_match(value.1)
                    && c.is_match(value.2)
                    && d.is_match(value.3)
            }
            MatchCondition::Default => true,
            _ => unreachable!(),
        }
    }
}

impl MatchAble<&DataField> for MatchCase {
    fn is_match(&self, value: &DataField) -> bool {
        self.cond.is_match(value)
    }
}

impl MatchAble<(&DataField, &DataField)> for MatchCase {
    fn is_match(&self, value: (&DataField, &DataField)) -> bool {
        self.cond.is_match(value)
    }
}

impl MatchAble<(&DataField, &DataField, &DataField)> for MatchCase {
    fn is_match(&self, value: (&DataField, &DataField, &DataField)) -> bool {
        self.cond.is_match(value)
    }
}

impl MatchAble<(&DataField, &DataField, &DataField, &DataField)> for MatchCase {
    fn is_match(&self, value: (&DataField, &DataField, &DataField, &DataField)) -> bool {
        self.cond.is_match(value)
    }
}
impl MatchCase {
    pub fn new(cond: MatchCondition, value: NestedAccessor) -> Self {
        Self {
            cond,
            result: value,
        }
    }
    pub fn condition_mut(&mut self) -> &mut MatchCondition {
        &mut self.cond
    }
    pub fn result_mut(&mut self) -> &mut NestedAccessor {
        &mut self.result
    }
    pub fn eq_const<S: Into<String>>(meta_str: &str, m_val: S, t_val: S) -> AnyResult<Self> {
        let meta = DataType::from(meta_str)?;
        let m_obj = DataField::from_str(meta.clone(), "".to_string(), m_val.into())?;
        let target = DataField::from_str(meta, "".to_string(), t_val.into())?;
        Ok(Self::new(
            MatchCondition::Single(MatchCond::Eq(m_obj)),
            NestedAccessor::Field(target),
        ))
    }
    /*
    pub fn eq_var<S: Into<String>>(meta_str: &str, m_val: S, t_val: S) -> AnyResult<Self> {
        let meta = Meta::from(meta_str).unwrap();
        let m_obj = TDOEnum::from_str(meta.clone(), "".to_string(), m_val.into())?;
        Ok(Self::new(MatchCond::Eq(m_obj), SubGetWay::Direct(t_val.into())))
    }
     */
    pub fn in_const<S: Into<String>>(
        meta_str: &str,
        m_beg: S,
        m_end: S,
        t_val: S,
    ) -> AnyResult<Self> {
        let meta = DataType::from(meta_str)?;
        let beg_obj = DataField::from_str(meta.clone(), "".to_string(), m_beg.into())?;
        let end_obj = DataField::from_str(meta.clone(), "".to_string(), m_end.into())?;
        let target = DataField::from_str(meta, "".to_string(), t_val.into())?;
        Ok(Self::new(
            MatchCondition::Single(MatchCond::In(beg_obj, end_obj)),
            NestedAccessor::Field(target),
        ))
    }
    pub fn eq2_const<S: Into<String>>(
        meta_str: &str,
        m_beg: S,
        m_end: S,
        t_val: S,
    ) -> AnyResult<Self> {
        let meta = DataType::from(meta_str)?;
        let beg_obj = DataField::from_str(meta.clone(), "".to_string(), m_beg.into())?;
        let end_obj = DataField::from_str(meta.clone(), "".to_string(), m_end.into())?;
        let target = DataField::from_str(meta, "".to_string(), t_val.into())?;
        Ok(Self::new(
            MatchCondition::Double(MatchCond::Eq(beg_obj), MatchCond::Eq(end_obj)),
            NestedAccessor::Field(target),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_code() {}
}

#[derive(Clone, Debug, Getters)]
pub struct MatchOperation {
    dat_crate: MatchSource,
    items: Vec<MatchCase>,
    default: Option<MatchCase>,
}

#[derive(Clone, Debug)]
pub enum MatchSource {
    Single(DirectAccessor),
    Double(DirectAccessor, DirectAccessor),
    Triple(DirectAccessor, DirectAccessor, DirectAccessor),
    Quadruple(DirectAccessor, DirectAccessor, DirectAccessor, DirectAccessor),
}

impl MatchOperation {
    pub fn new(dat_crate: MatchSource, items: Vec<MatchCase>, default: Option<MatchCase>) -> Self {
        Self {
            dat_crate,
            items,
            default,
        }
    }

    pub fn items_mut(&mut self) -> &mut Vec<MatchCase> {
        &mut self.items
    }

    pub fn default_mut(&mut self) -> Option<&mut MatchCase> {
        self.default.as_mut()
    }
}

impl Display for MatchOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.dat_crate {
            MatchSource::Single(c) => {
                writeln!(f, "match {} {{", c)?;
            }
            MatchSource::Double(fst, sec) => {
                writeln!(f, "match ({}, {}) {{", fst, sec)?;
            }
            MatchSource::Triple(a, b, c) => {
                writeln!(f, "match ({}, {}, {}) {{", a, b, c)?;
            }
            MatchSource::Quadruple(a, b, c, d) => {
                writeln!(f, "match ({}, {}, {}, {}) {{", a, b, c, d)?;
            }
        }
        for o in self.items.iter() {
            writeln!(f, "{},", o)?;
        }
        if let Some(default) = &self.default {
            writeln!(f, "{},", default)?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}
