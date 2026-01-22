use std::collections::HashMap;

use crate::language::CondAccessor;

use orion_exp::SQLSymbol;

use super::ArgsTakeAble;
use super::logic::ToSqlFmt;
pub type CompareExpress = orion_exp::Comparison<CondAccessor, SQLSymbol>;
impl ArgsTakeAble for CompareExpress {
    fn args_take(&self) -> (String, HashMap<String, CondAccessor>) {
        // Special case: right side is a SQL function expression that must be printed inline
        if let CondAccessor::SqlFn(fun) = self.right_const() {
            let (right_sql, params) = fun.to_sql_and_params();
            let sql = format!(
                "{} {} {}",
                self.var_name(),
                self.compare_op().to_sql(),
                right_sql
            );
            return (sql, params);
        }
        // Default: bind right const as a named parameter by left name
        let mut args = HashMap::new();
        args.insert(self.var_name().clone(), self.right_const().clone());
        let sql = format!(
            "{} {} :{}",
            self.var_name(),
            self.compare_op().to_sql(),
            self.var_name()
        );
        (sql, args)
    }
}
