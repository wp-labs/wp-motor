pub mod builder;
pub mod core;

mod display;
pub mod evaluator;
pub mod operator;
mod traits;
// adapters removed; prefer function-style evaluation from callers
pub use operator::symbols::{CmpSymbolProvider, LogicSymbolProvider, RustSymbol, SQLSymbol};
pub use operator::{CmpOperator, LogicOperator};
pub use traits::*;

pub use builder::{ExpressionBuilder, LogicalBuilder, LogicalTrait};
pub use core::compare::Comparison;
pub use core::logic::Expression;
pub use core::logic::LogicalExpress;
