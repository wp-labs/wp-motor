use crate::core::prelude::*;
use crate::language::EvaluationTarget;
use crate::language::SqlQuery;
use wp_know::mem::{SqlNamedParam, ToSqlParams};
use wp_knowledge::facade as kdb;

impl FieldExtractor for SqlQuery {
    #[allow(unused_variables)]
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        // 单值提取在 SQL 评估中不支持，返回 None 以避免运行期 panic
        None
    }

    fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        let mut params = Vec::with_capacity(5);
        let target = EvaluationTarget::auto_default();
        for (v, acq) in self.vars() {
            if let Some(mut tdo) = acq.extract_one(&target, src, dst) {
                tdo.set_name(format!(":{}", v));
                params.push(tdo);
            }
        }
        debug_kdb!("pararms:{:#?}", params);
        let sql = self.oml_sql();
        debug_kdb!("[sql] {}", sql);
        for (v, acq) in self.vars() {
            let preview = acq.diy_fmt(&wp_data_fmt::SqlInsert::new_with_json("_"));
            debug_kdb!("[param] :{} = {}", v, preview);
        }
        let md5 = DataField::from_chars("sql".to_string(), self.sql_md5().clone());
        // 规范化缓存键：仅保留 Value/名称，Meta 统一为 Auto，减少“同值不同 meta”导致的缓存碎片
        fn norm(f: &DataField) -> DataField {
            use wp_model_core::model::DataType;
            DataField::new(DataType::default(), f.clone_name(), f.get_value().clone())
        }

        match params.len() {
            0 => {
                let c_params: [DataField; 1] = [norm(&md5)];
                // 无命名参数：传递空切片
                let out = kdb::cache_query(sql, &c_params, &[], cache);
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }

            1 => {
                let c_params: [DataField; 2] = [norm(&md5), norm(&params[0])];
                let _p0 = params.remove(0);
                let q_params: [SqlNamedParam; 1] = [SqlNamedParam(c_params[1].clone())];
                let q_p = &q_params.to_params();
                let out = kdb::cache_query(sql, &c_params, q_p, cache);
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            2 => {
                let c_params: [DataField; 3] = [norm(&md5), norm(&params[1]), norm(&params[0])];
                let _ = (params.remove(1), params.remove(0));
                let q_params: [SqlNamedParam; 2] = [
                    SqlNamedParam(c_params[1].clone()),
                    SqlNamedParam(c_params[2].clone()),
                ];
                let q_p = &q_params.to_params();
                let out = kdb::cache_query(sql, &c_params, q_p, cache);
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            3 => {
                let c_params: [DataField; 4] = [
                    norm(&md5),
                    norm(&params[2]),
                    norm(&params[1]),
                    norm(&params[0]),
                ];
                let _ = (params.remove(2), params.remove(1), params.remove(0));
                let q_params: [SqlNamedParam; 3] = [
                    SqlNamedParam(c_params[1].clone()),
                    SqlNamedParam(c_params[2].clone()),
                    SqlNamedParam(c_params[3].clone()),
                ];
                let q_p = &q_params.to_params();
                let out = kdb::cache_query(sql, &c_params, q_p, cache);
                println!("[sql] got {} cols", out.len());
                out
            }
            4 => {
                // 显式构造，避免 try_into().unwrap() 带来的运行期 panic 风险
                let c_params: [DataField; 5] = [
                    norm(&md5),
                    norm(&params[3]),
                    norm(&params[2]),
                    norm(&params[1]),
                    norm(&params[0]),
                ];
                let _ = (
                    params.remove(3),
                    params.remove(2),
                    params.remove(1),
                    params.remove(0),
                );
                let q_params: [SqlNamedParam; 4] = [
                    SqlNamedParam(c_params[1].clone()),
                    SqlNamedParam(c_params[2].clone()),
                    SqlNamedParam(c_params[3].clone()),
                    SqlNamedParam(c_params[4].clone()),
                ];
                let q_p = &q_params.to_params();
                let out = kdb::cache_query(self.oml_sql(), &c_params, q_p, cache);
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            5 => {
                let c_params: [DataField; 6] = [
                    norm(&md5),
                    norm(&params[4]),
                    norm(&params[3]),
                    norm(&params[2]),
                    norm(&params[1]),
                    norm(&params[0]),
                ];
                let _ = (
                    params.remove(4),
                    params.remove(3),
                    params.remove(2),
                    params.remove(1),
                    params.remove(0),
                );
                let q_params: [SqlNamedParam; 5] = [
                    SqlNamedParam(c_params[1].clone()),
                    SqlNamedParam(c_params[2].clone()),
                    SqlNamedParam(c_params[3].clone()),
                    SqlNamedParam(c_params[4].clone()),
                    SqlNamedParam(c_params[5].clone()),
                ];
                let q_p = &q_params.to_params();
                let out = kdb::cache_query(self.oml_sql(), &c_params, q_p, cache);
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            _ => {
                error_data!("not support more 9 params in sql eval: {}", params.len());
                //unimplemented!("not support more 9 params len ")
                Vec::new()
            }
        }
    }
    fn support_batch(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DataRecordRef;
    use crate::language::CondAccessor;
    use once_cell::sync::OnceCell;
    use orion_error::TestAssert;
    use wp_know::mem::memdb::MemDB;
    use wp_knowledge::facade as kdb;
    use wp_model_core::model::{DataField, DataRecord, Value};

    // 测试初始化：一次性将 provider 绑定到全局内存库，并建表/灌入数据
    fn ensure_provider() {
        static INIT: OnceCell<()> = OnceCell::new();
        INIT.get_or_init(|| {
            let db = MemDB::global();
            db.table_create(
                "CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY, name TEXT, value INTEGER)",
            )
            .assert();
            db.execute(
                "INSERT OR REPLACE INTO test (id, name, value) VALUES (1, 'test1', 100)",
            )
            .assert();
            db.execute(
                "INSERT OR REPLACE INTO test (id, name, value) VALUES (2, 'test2', 200)",
            )
            .assert();
            let _ = kdb::init_mem_provider(db);
        });
    }

    // 创建测试用的 SqlQuery 对象
    fn create_test_query(sql: &str, vars: Vec<(&str, DataField)>) -> SqlQuery {
        SqlQuery::new(
            sql.to_string(),
            vars.into_iter()
                .map(|(name, field)| (name.to_string(), CondAccessor::Val(field.value)))
                .collect(),
        )
    }

    #[test]
    fn test_no_params_query() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let query = create_test_query("SELECT * FROM test WHERE id = 1", vec![]);
        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &DataRecord::default(),
            cache,
        );

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get_name(), "id");
        assert_eq!(result[0].get_value(), &Value::Digit(1));
    }

    #[test]
    fn test_single_param_query() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let param = DataField::from_digit("id".to_string(), 1);
        let query = create_test_query("SELECT * FROM test WHERE id = :id", vec![("id", param)]);

        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &DataRecord::default(),
            cache,
        );

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get_name(), "id");
        assert_eq!(result[0].get_value(), &Value::Digit(1));
    }

    #[test]
    fn test_multiple_params_query() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let id_param = DataField::from_digit("id".to_string(), 1);
        let name_param = DataField::from_chars("name".to_string(), "test1".to_string());

        let query = create_test_query(
            "SELECT * FROM test WHERE id = :id AND name = :name",
            vec![("id", id_param), ("name", name_param)],
        );

        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &DataRecord::default(),
            cache,
        );

        assert_eq!(result.len(), 3);
        assert_eq!(result[1].get_name(), "name");
        assert_eq!(result[1].get_value(), &Value::Chars("test1".into()));
    }

    #[test]
    fn test_max_params_query() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let params = vec![
            ("p1", DataField::from_digit("p1".to_string(), 1)),
            ("p2", DataField::from_digit("p2".to_string(), 2)),
            ("p3", DataField::from_digit("p3".to_string(), 3)),
            ("p4", DataField::from_digit("p4".to_string(), 4)),
            ("p5", DataField::from_digit("p5".to_string(), 5)),
        ];

        let query = create_test_query(
            "SELECT * FROM test WHERE id IN (:p1, :p2, :p3, :p4, :p5)",
            params,
        );

        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &DataRecord::default(),
            cache,
        );

        assert!(!result.is_empty());
    }

    #[test]
    fn test_too_many_params_query() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let params = vec![
            ("p1", DataField::from_digit("p1".to_string(), 1)),
            ("p2", DataField::from_digit("p2".to_string(), 2)),
            ("p3", DataField::from_digit("p3".to_string(), 3)),
            ("p4", DataField::from_digit("p4".to_string(), 4)),
            ("p5", DataField::from_digit("p5".to_string(), 5)),
            ("p6", DataField::from_digit("p6".to_string(), 6)),
        ];

        let query = create_test_query(
            "SELECT * FROM test WHERE id IN (:p1, :p2, :p3, :p4, :p5, :p6)",
            params,
        );

        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &DataRecord::default(),
            cache,
        );

        assert!(result.is_empty());
    }
}
