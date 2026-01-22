use crate::language::CondAccessor;
use orion_exp::LogicOperator;
use orion_exp::{CmpOperator, SQLSymbol};
use std::collections::HashMap;

use super::ArgsTakeAble;

pub type LogicExpress = orion_exp::LogicalExpress<CondAccessor, SQLSymbol>;
pub type LogicalExpression = orion_exp::Expression<CondAccessor, SQLSymbol>;

pub trait ToSqlFmt {
    fn to_sql(&self) -> String;
}
impl ToSqlFmt for CmpOperator {
    fn to_sql(&self) -> String {
        let sql = match self {
            CmpOperator::We => {
                unreachable!()
            }
            CmpOperator::Eq => "=",
            CmpOperator::Ne => "!=",
            CmpOperator::Gt => ">",
            CmpOperator::Ge => ">=",
            CmpOperator::Lt => "<",
            CmpOperator::Le => "<=",
        };
        sql.into()
    }
}

impl ArgsTakeAble for LogicalExpression {
    fn args_take(&self) -> (String, HashMap<String, CondAccessor>) {
        match self {
            LogicalExpression::Logic(v) => v.args_take(),
            LogicalExpression::Compare(v) => v.args_take(),
        }
    }
}

impl ArgsTakeAble for LogicExpress {
    fn args_take(&self) -> (String, HashMap<String, CondAccessor>) {
        match self.op {
            LogicOperator::And => {
                if let Some(left) = &self.left {
                    let (l_sql, l_vars) = left.args_take();
                    let (r_sql, r_vars) = self.right.args_take();
                    let sql = format!("{} and {} ", l_sql, r_sql);
                    let mut vars = HashMap::new();
                    vars.extend(l_vars);
                    vars.extend(r_vars);
                    (sql, vars)
                } else {
                    unreachable!("and express not left statement")
                }
            }
            LogicOperator::Or => {
                if let Some(left) = &self.left {
                    let (l_sql, l_vars) = left.args_take();
                    let (r_sql, r_vars) = self.right.args_take();
                    let sql = format!("{} or {} ", l_sql, r_sql);
                    let mut vars = HashMap::new();
                    vars.extend(l_vars);
                    vars.extend(r_vars);
                    (sql, vars)
                } else {
                    unreachable!("and express not left statement")
                }
            }
            LogicOperator::Not => {
                let (r_sql, r_vars) = self.right.args_take();
                let sql = format!("not {} ", r_sql);
                (sql, r_vars)
            }
        }
    }
}
