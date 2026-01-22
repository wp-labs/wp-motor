pub mod compare;
pub mod logic;
use std::collections::HashMap;

use crate::language::CondAccessor;
pub use compare::CompareExpress;
pub use logic::LogicalExpression;
pub trait ArgsTakeAble {
    fn args_take(&self) -> (String, HashMap<String, CondAccessor>);
}
