use derive_builder::Builder;
use derive_getters::Getters;
use std::fmt::{Display, Formatter};
use wildmatch::WildMatch;
use wp_model_core::model::{DataField, DataType};

#[derive(Default, Builder, Debug, Clone, Eq, PartialEq, Getters)]
pub struct EvaluationTarget {
    name: Option<String>,
    data_type: DataType,
}

impl EvaluationTarget {
    pub fn safe_name(&self) -> String {
        self.name.clone().unwrap_or("_".into())
    }
}

#[derive(Debug, Clone, Getters)]
pub struct BatchEvalTarget {
    origin: EvaluationTarget,
    wild: WildMatch,
}

impl BatchEvalTarget {
    pub fn new(origin: EvaluationTarget) -> Self {
        let name = origin.name().clone().unwrap_or("_".to_string());
        Self {
            wild: WildMatch::new(name.as_str()),
            origin,
        }
    }
    pub fn match_it(&self, tdo: &DataField) -> bool {
        let key = tdo.get_name().trim();
        //let k2 = key.trim();
        if self.wild().matches(key) && self.origin().data_type() == &DataType::Auto
            || tdo.get_meta() == self.origin().data_type()
        {
            return true;
        }
        false
    }
}

impl EvaluationTarget {
    pub fn new(name: String, meta: DataType) -> Self {
        Self {
            name: Some(name),
            data_type: meta,
        }
    }
    pub fn auto_default() -> Self {
        Self {
            name: None,
            data_type: DataType::Auto,
        }
    }
}
impl From<(Option<String>, DataType)> for EvaluationTarget {
    fn from(v: (Option<String>, DataType)) -> Self {
        Self {
            name: v.0,
            data_type: v.1,
        }
    }
}

impl Display for EvaluationTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = self.name.clone().unwrap_or("_".to_string());
        write!(f, "{} : {} ", name, self.data_type)
    }
}

impl Display for BatchEvalTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.origin)
    }
}
