use crate::language::{prelude::*, syntax::accessors::CondAccessor};
use std::collections::HashMap;
use wp_data_fmt::SqlInsert;
// 已移除对具体 DB 的依赖；通过门面在运行期解析

#[derive(Builder, Debug, Clone, Getters)]
pub struct SqlQuery {
    oml_sql: String,
    sql_md5: String,
    vars: HashMap<String, CondAccessor>,
}

impl SqlQuery {
    pub fn new(sql: String, vars: HashMap<String, CondAccessor>) -> Self {
        let sql_md5 = format!("{:x}", md5::compute(sql.as_bytes()));

        Self {
            oml_sql: sql,
            vars,
            sql_md5,
        }
    }
}

impl Display for SqlQuery {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let sql_fmt = SqlInsert::new_with_json("unknow");
        let mut sql = self.oml_sql.clone();
        for (v_name, acq) in &self.vars {
            let v_str = format!(":{}", v_name);
            let a_str = acq.diy_fmt(&sql_fmt).to_string();
            sql = sql.replace(v_str.as_str(), a_str.as_str());
        }
        write!(f, "{};", sql)
    }
}

#[cfg(test)]
mod tests {
    use crate::language::FieldRead;
    use crate::language::SqlQuery;
    use crate::language::{BuiltinFunction, FunOperation, NowTime, RecordOperation};
    use crate::language::{CondAccessor, DirectAccessor};
    use std::collections::HashMap;

    #[test]
    fn test_build() {
        let acq = CondAccessor::Tdc(RecordOperation::new(DirectAccessor::Read(
            FieldRead::default(),
        )));
        let mut vars = HashMap::new();
        vars.insert("A".to_string(), acq);
        let sql_prm = SqlQuery::new("select a,b from table_1 where a = :A ".into(), vars);
        let result = format!("{}", sql_prm);
        assert_eq!(result, "select a,b from table_1 where a = read(  )   ;");

        let acq = CondAccessor::Fun(FunOperation::new(BuiltinFunction::NowTime(
            NowTime::default(),
        )));
        let mut vars = HashMap::new();
        vars.insert("A".to_string(), acq);
        let sql_prm = SqlQuery::new("select a,b from table_1 where a = :A".into(), vars);
        let result = format!("{}", sql_prm);
        assert_eq!(result, "select a,b from table_1 where a = Now::time() ;")
    }
}
