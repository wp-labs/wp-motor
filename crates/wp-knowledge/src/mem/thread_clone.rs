use std::cell::RefCell;
use std::time::Duration;

use crate::DBQuery;
use crate::mem::RowData;
use orion_error::{ErrorOwe, ErrorWith};
use rusqlite::backup::Backup;
use rusqlite::{Connection, Params};
use wp_error::KnowledgeResult;
use wp_model_core::model::DataField;

thread_local! {
    // clippy: use const init for thread_local value
    static TLS_DB: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

/// Thread-cloned read-only in-memory DB built from an authority file DB via SQLite backup API.
/// Each thread lazily creates its own in-memory Connection (no cross-thread sharing).
#[derive(Clone)]
pub struct ThreadClonedMDB {
    authority_path: String,
}

impl ThreadClonedMDB {
    pub fn from_authority(path: &str) -> Self {
        Self {
            authority_path: path.to_string(),
        }
    }

    pub fn with_tls_conn<T, F: FnOnce(&Connection) -> KnowledgeResult<T>>(
        &self,
        f: F,
    ) -> KnowledgeResult<T> {
        let path = self.authority_path.clone();
        TLS_DB.with(|cell| {
            // make sure a thread-local in-memory db exists
            if cell.borrow().is_none() {
                // source: authority file; dest: in-memory
                let src = Connection::open_with_flags(
                    &path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                        | rusqlite::OpenFlags::SQLITE_OPEN_URI,
                )
                .owe_res()
                .want("connect db")?;
                let mut dst = Connection::open_in_memory().owe_res().want("oepn conn")?;
                {
                    let bk = Backup::new(&src, &mut dst).owe_conf().want("backup")?;
                    // Copy all pages with small sleep to yield
                    bk.run_to_completion(50, Duration::from_millis(0), None)
                        .owe_res()
                        .want("backup run")?;
                }
                // 为查询连接注册内置 UDF（只读场景也可用在 SQL/OML 查询中）
                let _ = crate::sqlite_ext::register_builtin(&dst);
                *cell.borrow_mut() = Some(dst);
            }
            // safe to unwrap since ensured above
            let conn = cell.borrow();
            f(conn.as_ref().unwrap())
        })
    }
}

impl DBQuery for ThreadClonedMDB {
    fn query(&self, sql: &str) -> KnowledgeResult<Vec<RowData>> {
        self.with_tls_conn(|conn| super::query_util::query(conn, sql, []))
    }
    fn query_row(&self, sql: &str) -> KnowledgeResult<RowData> {
        self.with_tls_conn(|conn| super::query_util::query_first_row(conn, sql, []))
    }

    fn query_row_params<P: Params>(&self, sql: &str, params: P) -> KnowledgeResult<RowData> {
        self.with_tls_conn(|conn| super::query_util::query_first_row(conn, sql, params))
    }

    fn query_row_tdos<P: Params>(
        &self,
        _sql: &str,
        _params: &[DataField; 2],
    ) -> KnowledgeResult<RowData> {
        // not used in current benchmarks
        Ok(vec![])
    }

    fn query_cipher(&self, table: &str) -> KnowledgeResult<Vec<String>> {
        self.with_tls_conn(|conn| {
            let sql = format!("select value from {}", table);
            let mut stmt = conn.prepare(&sql).owe_rule()?;
            let mut rows = stmt.query([]).owe_rule()?;
            let mut result = Vec::new();
            while let Some(row) = rows.next().owe_rule()? {
                let x = row.get_ref(0).owe_rule()?;
                if let rusqlite::types::ValueRef::Text(val) = x {
                    result.push(String::from_utf8(val.to_vec()).owe_rule()?);
                }
            }
            Ok(result)
        })
    }
}
