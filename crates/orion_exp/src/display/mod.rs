use crate::{CmpSymbolProvider, LogicSymbolProvider, operator::LogicOperator};
use std::fmt::Display;

use super::core::{
    compare::Comparison,
    logic::{Expression, LogicalExpress},
};

impl<T: Sized + Display, S> Display for LogicalExpress<T, S>
where
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(left) = &self.left {
            match left.as_ref() {
                Expression::Logic(v) => match v.op() {
                    LogicOperator::Not => {
                        write!(f, "{} ", v)?;
                    }
                    _ => {
                        write!(f, "({}) ", v)?;
                    }
                },
                Expression::Compare(v) => {
                    write!(f, "{} ", v)?;
                }
            }
        }
        match self.right.as_ref() {
            Expression::Logic(v) => match v.op() {
                LogicOperator::Not => {
                    write!(f, "{}{}", S::symbol_logic(self.op()), self.right)
                }
                _ => {
                    write!(f, "{} ({})", S::symbol_logic(self.op()), self.right)
                }
            },
            Expression::Compare(_v) => {
                write!(f, "{} {}", S::symbol_logic(self.op()), self.right)
            }
        }
    }
}

impl<T: Sized + std::fmt::Display, CS> Display for Comparison<T, CS>
where
    CS: CmpSymbolProvider,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}",
            CS::symbol_var(self.var_name()),
            CS::symbol_cmp(self.compare_op()),
            self.right_const()
        )
    }
}
