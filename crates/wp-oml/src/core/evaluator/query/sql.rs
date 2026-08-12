use crate::core::prelude::*;
use crate::language::EvaluationTarget;
use crate::language::{SqlKnowledgeRoute, SqlQuery};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use wp_know::mem::thread_clone::ThreadClonedMDB;
use wp_knowledge::facade as kdb;
use wp_knowledge::sql_route::first_table_name;
use wp_model_core::model::FieldStorage;
use wp_model_core::model::{DataType, Value};

use crate::core::AsyncFieldExtractor;

// SQL evaluator already places the SQL md5 into c_params[0], so a separate scope hash
// would only duplicate the same partitioning work on the local-cache hot path.
const INLINE_SQL_LOCAL_CACHE_SCOPE: u64 = 0;

#[derive(Clone)]
struct TableRouteConfig {
    sqlite: ThreadClonedMDB,
    sqlite_tables: HashSet<String>,
    provider_tables: HashSet<String>,
}

fn table_route_config() -> &'static OnceLock<Option<Arc<TableRouteConfig>>> {
    static ROUTE: OnceLock<Option<Arc<TableRouteConfig>>> = OnceLock::new();
    &ROUTE
}

pub fn set_sql_table_route(
    authority_uri: String,
    sqlite_tables: Vec<String>,
    provider_tables: Vec<String>,
) {
    let route = TableRouteConfig {
        sqlite: ThreadClonedMDB::from_authority(authority_uri.as_str()),
        sqlite_tables: sqlite_tables.into_iter().collect(),
        provider_tables: provider_tables.into_iter().collect(),
    };
    let _ = table_route_config().set(Some(Arc::new(route)));
}

pub fn clear_sql_table_route() {
    // OnceLock 不支持在运行时重置；当前路由在进程启动期初始化一次即可。
}

pub fn resolve_sql_route(sql: &str) -> SqlKnowledgeRoute {
    let Some(table) = first_table_name(sql) else {
        return SqlKnowledgeRoute::Unknown;
    };
    let Some(route) = table_route_config().get().and_then(|route| route.as_ref()) else {
        return SqlKnowledgeRoute::Provider;
    };
    if route.sqlite_tables.contains(table) {
        return SqlKnowledgeRoute::Sqlite;
    }
    if route.provider_tables.contains(table) {
        return SqlKnowledgeRoute::Provider;
    }
    SqlKnowledgeRoute::Provider
}

fn local_sqlite_for_route(route: SqlKnowledgeRoute) -> Option<ThreadClonedMDB> {
    if route != SqlKnowledgeRoute::Sqlite {
        return None;
    }
    let route = table_route_config().get()?.as_ref()?;
    Some(route.sqlite.clone())
}

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
        match bytes[idx] {
            // 跳过字符串字面量与引号/方括号标识符
            b'\'' | b'"' | b'`' => {
                let quote = bytes[idx];
                idx += 1;
                while idx < bytes.len() {
                    if bytes[idx] == quote {
                        idx += 1;
                        if idx < bytes.len() && bytes[idx] == quote {
                            idx += 1;
                            continue;
                        }
                        break;
                    }
                    idx += 1;
                }
            }
            b'[' => {
                idx += 1;
                while idx < bytes.len() && bytes[idx] != b']' {
                    idx += 1;
                }
                idx += usize::from(idx < bytes.len());
            }
            // 跳过注释，避免把注释里的 `:名` 误当参数
            b'-' if bytes.get(idx + 1) == Some(&b'-') => {
                idx += 2;
                while idx < bytes.len() && bytes[idx] != b'\n' {
                    idx += 1;
                }
            }
            b'/' if bytes.get(idx + 1) == Some(&b'*') => {
                idx += 2;
                while idx + 1 < bytes.len() && !(bytes[idx] == b'*' && bytes[idx + 1] == b'/') {
                    idx += 1;
                }
                if idx + 1 < bytes.len() {
                    idx += 2;
                } else {
                    idx = bytes.len();
                }
            }
            b':' => {
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
            _ => idx += 1,
        }
    }

    out
}

fn collect_sql_params(
    query: &SqlQuery,
    src: &mut DataRecordRef<'_>,
    dst: &mut DataRecord,
) -> (String, DataField, Vec<DataField>, bool) {
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
    let all_params_null = !params.is_empty()
        && params
            .iter()
            .all(|param| matches!(param.get_value(), Value::Null));
    (sql, md5, params, all_params_null)
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
        let (sql, md5, params, all_params_null) = collect_sql_params(self, src, dst);
        if all_params_null {
            debug_kdb!("[sql] skip query because all params are null");
            return Vec::new();
        }

        if let Some(local_sqlite) = local_sqlite_for_route(*self.route()) {
            let row = if params.is_empty() {
                local_sqlite.query_row_with_scope(&sql)
            } else {
                local_sqlite.query_named_fields_with_scope(&sql, &params)
            };
            return match row {
                Ok(row) => row,
                Err(err) => {
                    warn_kdb!("[kdb] local sqlite routed query error: {}", err);
                    Vec::new()
                }
            };
        }

        // 命名 provider 路由：`from <provider>.<schema>.<table>` 命中已安装的 provider 时，
        // 剥离前缀后派发到对应命名 provider；未命中走默认 provider。
        let routed = kdb::route_provider_sql(&sql);
        let provider = routed.as_ref().map(|(name, _)| name.as_str());
        let exec_sql = routed
            .as_ref()
            .map(|(_, stripped)| stripped.as_str())
            .unwrap_or(&sql);

        match params.len() {
            0 => {
                let c_params: [DataField; 1] = [norm_query_field(&md5)];
                let out = kdb::cache_query_fields_route(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route(
                    provider,
                    exec_sql,
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
        let (sql, md5, params, all_params_null) = collect_sql_params(self, src, dst);
        if all_params_null {
            debug_kdb!("[sql] skip async query because all params are null");
            return Vec::new();
        }

        if let Some(local_sqlite) = local_sqlite_for_route(*self.route()) {
            let row = if params.is_empty() {
                local_sqlite.query_row_with_scope(&sql)
            } else {
                local_sqlite.query_named_fields_with_scope(&sql, &params)
            };
            return match row {
                Ok(row) => row,
                Err(err) => {
                    warn_kdb!("[kdb] local sqlite routed async query error: {}", err);
                    Vec::new()
                }
            };
        }

        // 命名 provider 路由（异步）：逻辑与同步路径一致。
        let routed = kdb::route_provider_sql(&sql);
        let provider = routed.as_ref().map(|(name, _)| name.as_str());
        let exec_sql = routed
            .as_ref()
            .map(|(_, stripped)| stripped.as_str())
            .unwrap_or(&sql);

        match params.len() {
            0 => {
                let c_params: [DataField; 1] = [norm_query_field(&md5)];
                let out = kdb::cache_query_fields_route_async(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route_async(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route_async(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route_async(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route_async(
                    provider,
                    exec_sql,
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
                let out = kdb::cache_query_fields_route_async(
                    provider,
                    exec_sql,
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
    use orion_error::dev::testing::TestAssert;
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
    fn test_resolve_sql_route_prefers_sqlite_for_sqlite_tables() {
        set_sql_table_route(
            "file:/tmp/test-authority.sqlite?mode=ro&uri=true".to_string(),
            vec!["local_asset_data".to_string()],
            vec!["asset_data".to_string()],
        );

        assert!(matches!(
            resolve_sql_route("SELECT asset FROM local_asset_data WHERE ip = :ip"),
            SqlKnowledgeRoute::Sqlite
        ));
        assert!(matches!(
            resolve_sql_route("SELECT asset FROM asset_data WHERE ip = :ip"),
            SqlKnowledgeRoute::Provider
        ));
    }

    #[test]
    fn test_resolve_sql_route_reads_table_from_subquery() {
        set_sql_table_route(
            "file:/tmp/test-authority.sqlite?mode=ro&uri=true".to_string(),
            vec!["local_asset_data".to_string()],
            vec!["asset_data".to_string()],
        );

        assert!(matches!(
            resolve_sql_route(
                "SELECT asset FROM (SELECT asset FROM local_asset_data WHERE ip = :ip) AS local_hits"
            ),
            SqlKnowledgeRoute::Sqlite
        ));
        assert!(matches!(
            resolve_sql_route(
                "SELECT asset FROM (SELECT asset FROM (SELECT asset FROM asset_data) nested) hits"
            ),
            SqlKnowledgeRoute::Provider
        ));
    }

    #[test]
    fn test_resolve_sql_route_ignores_from_inside_string_literal() {
        set_sql_table_route(
            "file:/tmp/test-authority.sqlite?mode=ro&uri=true".to_string(),
            vec!["local_asset_data".to_string()],
            vec!["asset_data".to_string()],
        );

        assert!(matches!(
            resolve_sql_route("SELECT ' from asset_data ' AS marker FROM local_asset_data"),
            SqlKnowledgeRoute::Sqlite
        ));
    }

    #[test]
    fn test_resolve_sql_route_ignores_from_inside_comment() {
        set_sql_table_route(
            "file:/tmp/test-authority.sqlite?mode=ro&uri=true".to_string(),
            vec!["local_asset_data".to_string()],
            vec!["asset_data".to_string()],
        );

        // 注释里的 `from asset_data`（provider 表）不应干扰 → 真实表 local_asset_data → Sqlite
        assert!(matches!(
            resolve_sql_route("SELECT 1 /* from asset_data */ FROM local_asset_data"),
            SqlKnowledgeRoute::Sqlite
        ));
        assert!(matches!(
            resolve_sql_route("SELECT 1 -- from asset_data\nFROM local_asset_data"),
            SqlKnowledgeRoute::Sqlite
        ));
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

    #[test]
    fn test_all_null_params_skip_query() {
        ensure_provider();
        let cache = &mut FieldQueryCache::default();

        let query = create_test_query(
            "SELECT * FROM table_that_should_not_be_queried WHERE id = :id AND name = :name",
            vec![
                (
                    "id",
                    DataField::new(DataType::default(), "id".to_string(), Value::Null),
                ),
                (
                    "name",
                    DataField::new(DataType::default(), "name".to_string(), Value::Null),
                ),
            ],
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

    // 命名 provider 路由测试用的桩 executor。
    struct NamedTestProvider {
        marker: &'static str,
    }

    #[async_trait]
    impl wp_knowledge::runtime::ProviderExecutor for NamedTestProvider {
        fn query(&self, _sql: &str) -> wp_knowledge::error::KnowledgeResult<Vec<Vec<DataField>>> {
            Ok(vec![vec![DataField::from_chars(
                "country_name",
                self.marker,
            )]])
        }

        fn query_fields(
            &self,
            _sql: &str,
            _params: &[DataField],
        ) -> wp_knowledge::error::KnowledgeResult<Vec<Vec<DataField>>> {
            self.query("")
        }

        fn query_row(&self, _sql: &str) -> wp_knowledge::error::KnowledgeResult<Vec<DataField>> {
            Ok(vec![DataField::from_chars("country_name", self.marker)])
        }

        fn query_named_fields(
            &self,
            _sql: &str,
            _params: &[DataField],
        ) -> wp_knowledge::error::KnowledgeResult<Vec<DataField>> {
            self.query_row("")
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_named_provider_routing() {
        use wp_knowledge::loader::ProviderKind;
        use wp_knowledge::runtime::DatasourceId;

        wp_knowledge::runtime::runtime()
            .install_provider_named(
                "geo",
                ProviderKind::SqliteAuthority,
                DatasourceId::from_seed(ProviderKind::SqliteAuthority, "geo"),
                |_generation| {
                    Ok(std::sync::Arc::new(NamedTestProvider {
                        marker: "geo-country",
                    }))
                },
                false,
            )
            .expect("install named geo provider");

        let cache = &mut FieldQueryCache::default();
        let query = create_test_query(
            "select country_name from geo.public.ip_geo_city where ip_num = :ip",
            vec![("ip", DataField::from_digit("ip".to_string(), 1))],
        );
        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &mut DataRecord::default(),
            cache,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get_name(), "country_name");
        assert_eq!(result[0].get_value(), &Value::Chars("geo-country".into()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_named_provider_unknown_falls_back_to_default() {
        // 未安装的 provider 名应回退到默认 provider（mem 库里有 test 表）。
        ensure_provider();
        let cache = &mut FieldQueryCache::default();
        let query = create_test_query(
            "select * from test where id = :id",
            vec![("id", DataField::from_digit("id".to_string(), 1))],
        );
        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &mut DataRecord::default(),
            cache,
        );
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get_value(), &Value::Digit(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_named_provider_routing_async() {
        use wp_knowledge::loader::ProviderKind;
        use wp_knowledge::runtime::DatasourceId;

        wp_knowledge::runtime::runtime()
            .install_provider_named(
                "geo",
                ProviderKind::SqliteAuthority,
                DatasourceId::from_seed(ProviderKind::SqliteAuthority, "geo"),
                |_generation| {
                    Ok(std::sync::Arc::new(NamedTestProvider {
                        marker: "geo-country",
                    }))
                },
                false,
            )
            .expect("install named geo provider");

        let cache = &mut FieldQueryCache::default();
        let query = create_test_query(
            "select country_name from geo.public.ip_geo_city where ip_num = :ip",
            vec![("ip", DataField::from_digit("ip".to_string(), 1))],
        );
        let mut dst = DataRecord::default();
        let result = query
            .extract_more_async(
                &mut DataRecordRef::from(&DataRecord::default()),
                &mut dst,
                cache,
            )
            .await;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get_value(), &Value::Chars("geo-country".into()));
    }

    #[test]
    fn test_named_provider_routing_two_params() {
        use wp_knowledge::loader::ProviderKind;
        use wp_knowledge::runtime::DatasourceId;

        wp_knowledge::runtime::runtime()
            .install_provider_named(
                "asset",
                ProviderKind::SqliteAuthority,
                DatasourceId::from_seed(ProviderKind::SqliteAuthority, "asset"),
                |_generation| {
                    Ok(std::sync::Arc::new(NamedTestProvider {
                        marker: "asset-value",
                    }))
                },
                false,
            )
            .expect("install named asset provider");

        let cache = &mut FieldQueryCache::default();
        let query = create_test_query(
            "select name from asset.public.t where id = :id and kind = :kind",
            vec![
                ("id", DataField::from_digit("id".to_string(), 1)),
                (
                    "kind",
                    DataField::from_chars("kind".to_string(), "k1".to_string()),
                ),
            ],
        );
        let result = query.extract_more(
            &mut DataRecordRef::from(&DataRecord::default()),
            &mut DataRecord::default(),
            cache,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get_name(), "country_name");
        assert_eq!(result[0].get_value(), &Value::Chars("asset-value".into()));
    }

    #[test]
    fn ordered_sql_param_names_skips_comments_and_strings() {
        let query = create_test_query(
            "select x from t where ip = :ip",
            vec![
                ("ip", DataField::from_digit("ip".to_string(), 1)),
                (
                    "name",
                    DataField::from_chars("name".to_string(), "x".to_string()),
                ),
            ],
        );

        // 注释 / 字符串字面量里的 `:name` 不应被当作参数收集；仅 `:ip` 生效
        let names = ordered_sql_param_names(
            "select x /* :name */ from t where name = ':name' and ip = :ip",
            &query,
        );
        assert_eq!(names, vec!["ip".to_string()]);

        let names = ordered_sql_param_names("select x -- :name\nfrom t where ip = :ip", &query);
        assert_eq!(names, vec!["ip".to_string()]);
    }
}
