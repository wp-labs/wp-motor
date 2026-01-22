use orion_error::ErrorOwe;
use rusqlite::Params;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wp_error::KnowledgeResult;
use wp_log::debug_kdb;
use wp_model_core::model::{self, DataField};

use lazy_static::lazy_static;

use crate::mem::RowData;

lazy_static! {
    /// Global column-name cache keyed by raw SQL text, shared by MemDB queries.
    pub static ref COLNAME_CACHE: RwLock<HashMap<String, Arc<Vec<String>>>> =
        RwLock::new(HashMap::new());
}

/// 将一行数据映射为 RowData
fn map_row(row: &rusqlite::Row<'_>, col_names: &[String]) -> KnowledgeResult<RowData> {
    let mut result = Vec::with_capacity(col_names.len());
    for (i, col_name) in col_names.iter().enumerate() {
        let value = row.get_ref(i).owe_rule()?;
        let field = match value {
            rusqlite::types::ValueRef::Null => {
                DataField::new(model::DataType::default(), col_name, model::Value::Null)
            }
            rusqlite::types::ValueRef::Integer(v) => DataField::from_digit(col_name, v),
            rusqlite::types::ValueRef::Real(v) => DataField::from_float(col_name, v),
            rusqlite::types::ValueRef::Text(v) => {
                DataField::from_chars(col_name, String::from_utf8(v.to_vec()).owe_rule()?)
            }
            rusqlite::types::ValueRef::Blob(v) => {
                DataField::from_chars(col_name, String::from_utf8_lossy(v).to_string())
            }
        };
        result.push(field);
    }
    Ok(result)
}

/// 从 statement 获取列名（普通版，带 debug 日志）
fn extract_col_names(stmt: &rusqlite::Statement<'_>) -> Vec<String> {
    let col_cnt = stmt.column_count();
    debug_kdb!("[memdb] col_cnt={}", col_cnt);
    let mut col_names = Vec::with_capacity(col_cnt);
    for i in 0..col_cnt {
        let name = stmt.column_name(i).unwrap_or("").to_string();
        debug_kdb!("[memdb] col[{}] name='{}'", i, name);
        col_names.push(name);
    }
    col_names
}

/// 从 statement 获取列名（cached 版，使用全局缓存）
fn extract_col_names_cached(
    stmt: &rusqlite::Statement<'_>,
    sql: &str,
) -> KnowledgeResult<Vec<String>> {
    if let Some(names) = COLNAME_CACHE.read().ok().and_then(|m| m.get(sql).cloned()) {
        return Ok((*names).clone());
    }
    let col_cnt = stmt.column_count();
    let mut names = Vec::with_capacity(col_cnt);
    for i in 0..col_cnt {
        names.push(stmt.column_name(i).owe_rule()?.to_string());
    }
    if let Ok(mut m) = COLNAME_CACHE.write() {
        m.insert(sql.to_string(), Arc::new(names.clone()));
    }
    Ok(names)
}

pub fn query<P: Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: P,
) -> KnowledgeResult<Vec<RowData>> {
    let mut stmt = conn.prepare_cached(sql).owe_rule()?;
    let col_names = extract_col_names(&stmt);
    let mut rows = stmt.query(params).owe_rule()?;
    let mut all_result = Vec::new();
    while let Some(row) = rows.next().owe_rule()? {
        all_result.push(map_row(row, &col_names)?);
    }
    Ok(all_result)
}

/// Query first row and map columns into RowData with column names preserved.
pub fn query_first_row<P: Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: P,
) -> KnowledgeResult<RowData> {
    let mut stmt = conn.prepare_cached(sql).owe_rule()?;
    let col_names = extract_col_names(&stmt);
    let mut rows = stmt.query(params).owe_rule()?;
    if let Some(row) = rows.next().owe_rule()? {
        map_row(row, &col_names)
    } else {
        debug_kdb!("[memdb] no row for sql");
        Ok(Vec::new())
    }
}

pub fn query_cached<P: Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: P,
) -> KnowledgeResult<Vec<RowData>> {
    let mut stmt = conn.prepare_cached(sql).owe_rule()?;
    // Column names cache (per SQL)
    let col_names = extract_col_names_cached(&stmt, sql)?;
    let mut rows = stmt.query(params).owe_rule()?;
    let mut all_result = Vec::new();
    while let Some(row) = rows.next().owe_rule()? {
        all_result.push(map_row(row, &col_names)?);
    }
    Ok(all_result)
}

/// Same as `query_first_row` but with a shared column-names cache to reduce metadata lookups.
pub fn query_first_row_cached<P: Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: P,
) -> KnowledgeResult<RowData> {
    let mut stmt = conn.prepare_cached(sql).owe_rule()?;
    let col_names = extract_col_names_cached(&stmt, sql)?;
    let mut rows = stmt.query(params).owe_rule()?;
    if let Some(row) = rows.next().owe_rule()? {
        map_row(row, &col_names)
    } else {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE test (id INTEGER, name TEXT, score REAL, data BLOB, empty)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_query_returns_all_rows() {
        let conn = setup_test_db();
        let rows = query(&conn, "SELECT * FROM test", []).unwrap();
        assert!(rows.is_empty());
        conn.execute("INSERT INTO test (id, name) VALUES (1, 'alice')", [])
            .unwrap();
        conn.execute("INSERT INTO test (id, name) VALUES (2, 'bob')", [])
            .unwrap();
        conn.execute("INSERT INTO test (id, name) VALUES (3, 'charlie')", [])
            .unwrap();

        let rows = query(&conn, "SELECT id, name FROM test ORDER BY id", []).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_query_first_row_returns_single_row() {
        let conn = setup_test_db();
        let row = query_first_row(&conn, "SELECT * FROM test", []).unwrap();
        assert!(row.is_empty());
        conn.execute("INSERT INTO test (id, name) VALUES (1, 'first')", [])
            .unwrap();
        conn.execute("INSERT INTO test (id, name) VALUES (2, 'second')", [])
            .unwrap();

        let row = query_first_row(&conn, "SELECT id, name FROM test ORDER BY id", []).unwrap();
        assert_eq!(row.len(), 2);
        assert_eq!(row[0].to_string(), "digit(1)");
        assert_eq!(row[1].to_string(), "chars(first)");
    }

    #[test]
    fn test_map_row_handles_all_types() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO test (id, name, score, data, empty) VALUES (42, 'hello', 3.14, X'414243', NULL)",
            [],
        )
        .unwrap();

        let row =
            query_first_row(&conn, "SELECT id, name, score, data, empty FROM test", []).unwrap();
        assert_eq!(row.len(), 5);
    }

    #[test]
    fn test_extract_col_names_preserves_aliases() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO test (id, name) VALUES (1, 'x')", [])
            .unwrap();

        let row = query_first_row(
            &conn,
            "SELECT id AS user_id, name AS user_name FROM test",
            [],
        )
        .unwrap();
        assert_eq!(row[0].get_name(), "user_id");
        assert_eq!(row[1].get_name(), "user_name");
    }

    #[test]
    fn test_query_cached_uses_cache() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO test (id) VALUES (1)", [])
            .unwrap();

        let sql = "SELECT id FROM test WHERE id = 1";
        // 第一次查询，填充缓存
        let _ = query_cached(&conn, sql, []).unwrap();
        // 第二次查询，应命中缓存
        let rows = query_cached(&conn, sql, []).unwrap();
        assert_eq!(rows.len(), 1);

        // 验证缓存已填充
        let cache = COLNAME_CACHE.read().unwrap();
        assert!(cache.contains_key(sql));
    }

    #[test]
    fn test_query_with_params() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO test (id, name) VALUES (1, 'alice')", [])
            .unwrap();
        conn.execute("INSERT INTO test (id, name) VALUES (2, 'bob')", [])
            .unwrap();

        let rows = query(&conn, "SELECT name FROM test WHERE id = ?1", [2]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].to_string(), "chars(bob)");
    }
}
