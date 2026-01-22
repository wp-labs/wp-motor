use std::fmt::{Display, Formatter};

use crate::language::EvalExp;
use derive_getters::Getters;
use enum_dispatch::enum_dispatch;
use wp_specs::WildArray;

#[derive(Getters, Debug, Clone)]
pub struct ObjModel {
    name: String,
    rules: WildArray,
    pub items: Vec<EvalExp>,
}

impl ObjModel {
    pub(crate) fn bind_rules(&mut self, rules_opt: Option<Vec<String>>) {
        if let Some(rules) = rules_opt {
            self.rules = WildArray::new1(rules);
        }
    }
}

impl ObjModel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            rules: WildArray::default(),
            items: Vec::new(),
        }
    }
}
impl Display for ObjModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "name : {}", self.name)?;
        if !self.rules.is_empty() {
            writeln!(f, "rule: ")?;
            for rule in self.rules.as_ref() {
                writeln!(f, "\t{}", rule)?;
            }
        }
        writeln!(f, "---")?;
        for i in &self.items {
            writeln!(f, "{}", i)?;
        }
        Ok(())
    }
}

#[derive(Clone, Default, Getters, Debug)]
pub struct StubModel {
    rules: WildArray,
}

#[derive(Debug, Clone)]
#[enum_dispatch(DataTransformer)]
pub enum DataModel {
    Stub(StubModel),
    Object(ObjModel),
}
impl Default for DataModel {
    fn default() -> Self {
        DataModel::Stub(StubModel::default())
    }
}
impl DataModel {
    pub fn rules(&self) -> &WildArray {
        match self {
            DataModel::Stub(x) => x.rules(),
            DataModel::Object(x) => x.rules(),
        }
    }
    pub fn is_match(&self, rule_key: &str) -> bool {
        for w_rule in self.rules().as_ref() {
            if w_rule.matches(rule_key) {
                return true;
            }
        }
        false
    }
}

impl DataModel {
    pub fn use_null() -> Self {
        Self::Stub(StubModel::default())
    }
}
