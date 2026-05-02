use crate::core::prelude::*;
use crate::language::EvaluationTarget;
use crate::language::SqlQuery;
use async_trait::async_trait;
use wp_knowledge::facade as kdb;
use wp_model_core::model::FieldStorage;
use wp_model_core::model::{DataType, Value};

use crate::core::AsyncFieldExtractor;

// SQL evaluator already places the SQL md5 into c_params[0], so a separate scope hash
// would only duplicate the same partitioning work on the local-cache hot path.
const INLINE_SQL_LOCAL_CACHE_SCOPE: u64 = 0;

fn norm_query_field(field: &DataField) -> DataField {
    DataField::new(
        DataType::default(),
        field.clone_name(),
        field.get_value().clone(),
    )
}

fn null_query_field(name: &str) -> DataField {
    DataField::new(DataType::default(), name.to_string(), Value::Null)
}

fn ordered_sql_param_names(sql: &str, query: &SqlQuery) -> Vec<String> {
    let mut out = Vec::with_capacity(query.vars().len());
    let bytes = sql.as_bytes();
    let mut idx = 0usize;

    while idx < bytes.len() {
        if bytes[idx] != b':' {
            idx += 1;
            continue;
        }

        let start = idx + 1;
        let mut end = start;
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if ch == '_' || ch.is_ascii_alphanumeric() {
                end += 1;
            } else {
                break;
            }
        }

        if end > start {
            let name = &sql[start..end];
            if query.vars().contains_key(name) {
                out.push(name.to_string());
            }
            idx = end;
        } else {
            idx += 1;
        }
    }

    out
}

fn collect_sql_params(
    query: &SqlQuery,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> (String, DataField, Vec<DataField>) {
    let mut params = Vec::with_capacity(5);
    let target = EvaluationTarget::auto_default();
    let sql = query.oml_sql().to_string();
    for v in ordered_sql_param_names(&sql, query) {
        let acq = query
            .vars()
            .get(&v)
            .expect("ordered SQL param names are collected from query vars");
        let mut tdo = if let Some(storage) = acq.extract_storage(&target, src, dst) {
            storage.into_owned()
        } else {
            null_query_field(format!(":{}", v).as_str())
        };
        tdo.set_name(format!(":{}", v));
        params.push(tdo);
    }
    debug_kdb!("pararms:{:#?}", params);
    debug_kdb!("[sql] {}", sql);
    for v in ordered_sql_param_names(&sql, query) {
        let acq = query
            .vars()
            .get(&v)
            .expect("ordered SQL param names are collected from query vars");
        let preview = acq.diy_fmt(&wp_data_fmt::SqlInsert::new_with_json("_"));
        debug_kdb!("[param] :{} = {}", v, preview);
    }
    let md5 = DataField::from_chars("sql".to_string(), query.sql_md5().clone());
    (sql, md5, params)
}

#[allow(dead_code)]
impl SqlQuery {
    #[allow(unused_variables)]
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        // 单值提取在 SQL 评估中不支持，返回 None 以避免运行期 panic
        None
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        let (sql, md5, params) = collect_sql_params(self, src, dst);

        match params.len() {
            0 => {
                let c_params: [DataField; 1] = [norm_query_field(&md5)];
                let out = kdb::cache_query_fields_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    &[],
                    cache,
                );
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }

            1 => {
                let c_params: [DataField; 2] =
                    [norm_query_field(&md5), norm_query_field(&params[0])];
                let query_params = [c_params[1].clone()];
                let out = kdb::cache_query_fields_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    &query_params,
                    cache,
                );
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            2 => {
                let c_params: [DataField; 3] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                ];
                let query_params = [c_params[1].clone(), c_params[2].clone()];
                let out = kdb::cache_query_fields_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    &query_params,
                    cache,
                );
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            3 => {
                let c_params: [DataField; 4] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                    norm_query_field(&params[2]),
                ];
                let query_params = [
                    c_params[1].clone(),
                    c_params[2].clone(),
                    c_params[3].clone(),
                ];
                let out = kdb::cache_query_fields_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    &query_params,
                    cache,
                );
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            4 => {
                // 显式构造，避免 try_into().unwrap() 带来的运行期 panic 风险
                let c_params: [DataField; 5] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                    norm_query_field(&params[2]),
                    norm_query_field(&params[3]),
                ];
                let query_params = [
                    c_params[1].clone(),
                    c_params[2].clone(),
                    c_params[3].clone(),
                    c_params[4].clone(),
                ];
                let out = kdb::cache_query_fields_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    &query_params,
                    cache,
                );
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            5 => {
                let c_params: [DataField; 6] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                    norm_query_field(&params[2]),
                    norm_query_field(&params[3]),
                    norm_query_field(&params[4]),
                ];
                let query_params = [
                    c_params[1].clone(),
                    c_params[2].clone(),
                    c_params[3].clone(),
                    c_params[4].clone(),
                    c_params[5].clone(),
                ];
                let out = kdb::cache_query_fields_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    &query_params,
                    cache,
                );
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            _ => {
                error_edata!(
                    dst.id,
                    "not support more 9 params in sql eval: {}",
                    params.len()
                );
                //unimplemented!("not support more 9 params len ")
                Vec::new()
            }
        }
    }
    pub(crate) fn support_batch(&self) -> bool {
        true
    }
}

#[async_trait]
impl AsyncFieldExtractor for SqlQuery {
    async fn extract_one_async(
        &self,
        _target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
    ) -> Option<DataField> {
        None
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        let (sql, md5, params) = collect_sql_params(self, src, dst);

        match params.len() {
            0 => {
                let c_params: [DataField; 1] = [norm_query_field(&md5)];
                let out = kdb::cache_query_fields_async_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    Vec::new,
                    cache,
                )
                .await;
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            1 => {
                let c_params: [DataField; 2] =
                    [norm_query_field(&md5), norm_query_field(&params[0])];
                let out = kdb::cache_query_fields_async_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    || vec![c_params[1].clone()],
                    cache,
                )
                .await;
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            2 => {
                let c_params: [DataField; 3] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                ];
                let out = kdb::cache_query_fields_async_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    || vec![c_params[1].clone(), c_params[2].clone()],
                    cache,
                )
                .await;
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            3 => {
                let c_params: [DataField; 4] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                    norm_query_field(&params[2]),
                ];
                let out = kdb::cache_query_fields_async_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    || {
                        vec![
                            c_params[1].clone(),
                            c_params[2].clone(),
                            c_params[3].clone(),
                        ]
                    },
                    cache,
                )
                .await;
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            4 => {
                let c_params: [DataField; 5] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                    norm_query_field(&params[2]),
                    norm_query_field(&params[3]),
                ];
                let out = kdb::cache_query_fields_async_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    || {
                        vec![
                            c_params[1].clone(),
                            c_params[2].clone(),
                            c_params[3].clone(),
                            c_params[4].clone(),
                        ]
                    },
                    cache,
                )
                .await;
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            5 => {
                let c_params: [DataField; 6] = [
                    norm_query_field(&md5),
                    norm_query_field(&params[0]),
                    norm_query_field(&params[1]),
                    norm_query_field(&params[2]),
                    norm_query_field(&params[3]),
                    norm_query_field(&params[4]),
                ];
                let out = kdb::cache_query_fields_async_with_scope(
                    sql.as_str(),
                    INLINE_SQL_LOCAL_CACHE_SCOPE,
                    &c_params,
                    || {
                        vec![
                            c_params[1].clone(),
                            c_params[2].clone(),
                            c_params[3].clone(),
                            c_params[4].clone(),
                            c_params[5].clone(),
                        ]
                    },
                    cache,
                )
                .await;
                debug_kdb!("[sql] got {} cols", out.len());
                out
            }
            _ => {
                error_edata!(
                    dst.id,
                    "not support more 9 params in sql eval: {}",
                    params.len()
                );
                Vec::new()
            }
        }
    }

    fn support_batch_async(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AsyncFieldExtractor;
    use crate::core::DataRecordRef;
    use crate::language::CondAccessor;
    use once_cell::sync::OnceCell;
    use orion_error::testcase::TestAssert;
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
            &mut DataRecord::default(),
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
            &mut DataRecord::default(),
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
            &mut DataRecord::default(),
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
            &mut DataRecord::default(),
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
            &mut DataRecord::default(),
            cache,
        );

        assert!(result.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_single_param_query_async() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let param = DataField::from_digit("id".to_string(), 1);
        let query = create_test_query("SELECT * FROM test WHERE id = :id", vec![("id", param)]);
        let mut dst = DataRecord::default();

        let result = query
            .extract_more_async(
                &mut DataRecordRef::from(&DataRecord::default()),
                &mut dst,
                cache,
            )
            .await;

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get_name(), "id");
        assert_eq!(result[0].get_value(), &Value::Digit(1));
    }
}
