pub mod model_index;
pub mod rule_index;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};

use super::{ModelName, RuleKey};

pub type RuleIndexSet = HashSet<RuleKey>;
pub type ModelNameSet = HashSet<ModelName>;
//#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IndexDisplay<'a, T> {
    inner: &'a HashSet<T>,
}
impl<'a, T> IndexDisplay<'a, T> {
    pub fn new(inner: &'a HashSet<T>) -> Self {
        Self { inner }
    }
}
impl<T> Display for IndexDisplay<'_, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, v) in self.inner.iter().enumerate() {
            if i == 0 {
                write!(f, "{}", v)?;
            } else {
                write!(f, ",{}", v)?;
            }
        }
        Ok(())
    }
}
