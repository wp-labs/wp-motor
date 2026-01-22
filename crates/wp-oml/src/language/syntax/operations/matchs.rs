use crate::language::prelude::*;
use crate::language::syntax::accessors::NestedAccessor;
use crate::types::AnyResult;
use derive_getters::Getters;
use orion_exp::CmpOperator;
use std::fmt::{Display, Formatter};
use wp_data_model::compare::compare_datafield;
use wp_model_core::model::{DataField, DataType};
use wpl::DataTypeParser;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MatchCond {
    Eq(DataField),
    Neq(DataField),
    In(DataField, DataField),

    Default,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MatchCondition {
    Single(MatchCond),
    Double(MatchCond, MatchCond),
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

            MatchCond::Default => true,
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
            MatchCondition::Double(_, _) => {
                unreachable!()
            }
            MatchCondition::Default => true,
        }
    }
}

impl MatchAble<(&DataField, &DataField)> for MatchCondition {
    fn is_match(&self, value: (&DataField, &DataField)) -> bool {
        match self {
            MatchCondition::Single(_) => {
                unreachable!()
            }
            MatchCondition::Double(fst, sec) => fst.is_match(value.0) && sec.is_match(value.1),
            MatchCondition::Default => true,
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
impl MatchCase {
    pub fn new(cond: MatchCondition, value: NestedAccessor) -> Self {
        Self {
            cond,
            result: value,
        }
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
}

impl MatchOperation {
    pub fn new(dat_crate: MatchSource, items: Vec<MatchCase>, default: Option<MatchCase>) -> Self {
        Self {
            dat_crate,
            items,
            default,
        }
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
