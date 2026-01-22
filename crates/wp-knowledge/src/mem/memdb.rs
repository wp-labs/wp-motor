use crate::DBQuery;
use crate::mem::RowData;
use crate::mem::stub::StubMDB;
use csv::Reader;
use enum_dispatch::enum_dispatch;
use lazy_static::lazy_static;
use orion_error::ErrorOwe;
use orion_error::ErrorWith;
use orion_error::ToStructError;
use orion_error::UvsConfFrom;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;
use rusqlite::Params;
use rusqlite::ToSql;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::Value;
use std::path::PathBuf;
use wp_data_model::cache::CacheAble;
use wp_error::KnowledgeReason;
use wp_error::KnowledgeResult;
use wp_log::debug_kdb;
use wp_log::info_kdb;
use wp_log::warn_kdb;
use wp_model_core::model;
use wp_model_core::model::DataField;

use super::AnyResult;
use super::SqlNamedParam;

lazy_static! {
    // Important: Use a single SQLite in-memory connection so schema/data persist across calls.
    // r2d2 with `memory()` creates isolated DBs per connection; limit pool to size=1 to reuse
    // the same connection and avoid "no such table" issues when different checkouts observe
    // different ephemeral databases.
    pub static ref MEM_SQLITE_INS: r2d2::Pool<SqliteConnectionManager> =
        r2d2::Pool::builder()
            .max_size(1)
            .build(SqliteConnectionManager::memory())
            .expect("init SQLite memory pool (size=1) failed");
}

#[derive(Debug, Clone)]
pub struct MemDB {
    conn: r2d2::Pool<SqliteConnectionManager>,
}

#[derive(Debug, Clone)]
#[enum_dispatch(DBQuery)]
pub enum MDBEnum {
    Stub(StubMDB),
    Use(MemDB),
}
impl Default for MDBEnum {
    fn default() -> Self {
        MDBEnum::Stub(StubMDB {})
    }
}
impl MDBEnum {
    pub fn global() -> Self {
        MDBEnum::Use(MemDB::global())
    }
    pub fn load_test() -> AnyResult<()> {
        MemDB::load_test()?;
        Ok(())
    }
}

pub fn cache_query<const N: usize, P: Params>(
    db: &MDBEnum,
    sql: &str,
    c_params: &[DataField; N],
    q_params: P,
    cache: &mut impl CacheAble<DataField, RowData, N>,
) -> RowData {
    crate::cache_util::cache_query_impl(c_params, cache, || db.query_row_params(sql, q_params))
}
impl ToSql for SqlNamedParam {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self.0.get_value() {
            model::Value::Bool(v) => Ok(ToSqlOutput::Owned(Value::Integer(if *v { 1 } else { 0 }))),
            model::Value::Null => Ok(ToSqlOutput::Owned(Value::Null)),
            model::Value::Chars(v) => Ok(ToSqlOutput::Owned(Value::Text(v.to_string()))),
            model::Value::Symbol(v) => Ok(ToSqlOutput::Owned(Value::Text(v.to_string()))),
            model::Value::Time(v) => Ok(ToSqlOutput::Owned(Value::Text(v.to_string()))),
            model::Value::Digit(v) => Ok(ToSqlOutput::Owned(Value::Integer(*v))),
            model::Value::Hex(v) => Ok(ToSqlOutput::Owned(Value::Text(v.to_string()))),
            model::Value::Float(v) => Ok(ToSqlOutput::Owned(Value::Real(*v))),
            model::Value::IpNet(v) => Ok(ToSqlOutput::Owned(Value::Text(v.to_string()))),
            model::Value::IpAddr(v) => Ok(ToSqlOutput::Owned(Value::Text(v.to_string()))),
            model::Value::Ignore(_) => Ok(ToSqlOutput::Owned(Value::Null)),
            model::Value::Obj(v) => Ok(ToSqlOutput::Owned(Value::Text(format!("{:?}", v)))),
            model::Value::Array(v) => Ok(ToSqlOutput::Owned(Value::Text(format!("{:?}", v)))),
            model::Value::Domain(v) => Ok(ToSqlOutput::Owned(Value::Text(v.0.to_string()))),
            model::Value::Url(v) => Ok(ToSqlOutput::Owned(Value::Text(v.0.to_string()))),
            model::Value::Email(v) => Ok(ToSqlOutput::Owned(Value::Text(v.0.to_string()))),
            model::Value::IdCard(v) => Ok(ToSqlOutput::Owned(Value::Text(v.0.to_string()))),
            model::Value::MobilePhone(v) => Ok(ToSqlOutput::Owned(Value::Text(v.0.to_string()))),
        }
    }
}

impl DBQuery for MemDB {
    fn query(&self, sql: &str) -> KnowledgeResult<Vec<RowData>> {
        let conn = self.conn.get().owe_res().want("get memdb connect")?;
        let _ = crate::sqlite_ext::register_builtin(&conn);
        super::query_util::query_cached(&conn, sql, [])
    }

    fn query_row(&self, sql: &str) -> KnowledgeResult<RowData> {
        let conn = self.conn.get().owe_res().want("get memdb connect")?;
        // Ensure SQLite UDFs are available on this connection (ip4_int/cidr4_* etc.)
        let _ = crate::sqlite_ext::register_builtin(&conn);
        super::query_util::query_first_row_cached(&conn, sql, [])
    }

    fn query_row_params<P: Params>(&self, sql: &str, params: P) -> KnowledgeResult<RowData> {
        debug_kdb!("[memdb] query_row_params: {}", sql);
        let conn = self.conn.get().owe_res()?;
        // Ensure SQLite UDFs are available on this connection
        let _ = crate::sqlite_ext::register_builtin(&conn);
        super::query_util::query_first_row_cached(&conn, sql, params)
    }

    fn query_cipher(&self, table: &str) -> KnowledgeResult<Vec<String>> {
        let sql = format!("select value from {}", table);
        let conn = self.conn.get().owe_res()?;
        let mut stmt = conn.prepare(&sql).owe_rule()?;
        let mut rows = stmt.query([]).owe_rule()?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().owe_rule()? {
            let x = row.get_ref(0).owe_rule()?;
            if let rusqlite::types::ValueRef::Text(val) = x {
                result.push(String::from_utf8(val.to_vec()).owe_conf()?);
            }
        }

        Ok(result)
    }

    fn query_row_tdos<P: Params>(
        &self,
        _sql: &str,
        _params: &[DataField; 2],
    ) -> KnowledgeResult<RowData> {
        //let data: [TDOParams; 2] = [TDOParams(&params[0]), TDOParams(&params[1])];
        //params.iter().for_each(|x| data.push(TDOParams(x)));
        //self.query_row_params(sql, data)
        todo!();
    }
}
impl MemDB {
    pub fn instance() -> Self {
        // Provide a single-connection pool for a consistent in-memory DB view
        let manager = SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("init SQLite memory pool (size=1) failed");
        Self { conn: pool }
    }
    /// Experimental: shared in-memory SQLite via URI with a pool size > 1.
    /// Requires SQLite compiled with shared-cache support.
    pub fn shared_pool(max_size: u32) -> AnyResult<Self> {
        // Shared in-memory URI. Every connection to this URI shares same DB.
        // Note: this depends on platform SQLite features.
        let uri = "file:wp_knowledge_shm?mode=memory&cache=shared";
        let manager = SqliteConnectionManager::file(uri).with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI,
        );
        let pool = r2d2::Pool::builder().max_size(max_size).build(manager)?;
        Ok(Self { conn: pool })
    }

    /// Create a MemDB backed by a file path with custom flags and pool size.
    pub fn new_file(
        path: &str,
        max_size: u32,
        flags: rusqlite::OpenFlags,
    ) -> KnowledgeResult<Self> {
        let manager = r2d2_sqlite::SqliteConnectionManager::file(path).with_flags(flags);
        let pool = r2d2::Pool::builder()
            .max_size(max_size)
            .build(manager)
            .owe_res()?;
        Ok(Self { conn: pool })
    }
    // V1 init_load_by_conf removed: use loader::build_authority_from_knowdb for V2

    /// Execute a closure with a checked-out SQLite connection from the pool.
    /// Useful for one-time prepared statements or specialized operations.
    pub fn with_conn<T, F: FnOnce(&rusqlite::Connection) -> AnyResult<T>>(
        &self,
        f: F,
    ) -> AnyResult<T> {
        let pooled = self.conn.get()?;
        let conn_ref: &rusqlite::Connection = &pooled;
        f(conn_ref)
    }

    pub fn table_create(&self, sql: &str) -> anyhow::Result<()> {
        let conn = self.conn.get()?;
        conn.execute(sql, ())?;
        debug_kdb!("crate table: {} ", sql);
        Ok(())
    }
    pub fn execute(&self, sql: &str) -> anyhow::Result<()> {
        let conn = self.conn.get()?;
        conn.execute(sql, ())?;
        debug_kdb!("execute: {} ", sql);
        Ok(())
    }

    pub fn table_clean(&self, sql: &str) -> anyhow::Result<()> {
        let conn = self.conn.get()?;
        conn.execute(sql, ())?;
        debug_kdb!("clean table: {} ", sql);
        Ok(())
    }

    pub fn table_load(
        &self,
        sql: &str,
        csv_path: PathBuf,
        cols: Vec<usize>,
        max: usize,
    ) -> AnyResult<usize> {
        info_kdb!("load table data in {}", csv_path.display());
        if !csv_path.exists() {
            warn_kdb!("{} not find, load knowdb failed", csv_path.display());
            return Ok(0);
        }
        let mut rdr = Reader::from_path(&csv_path)?;
        let conn = self.conn.get()?;
        let mut load_cnt: usize = 0;
        // Prepare once outside loop for performance
        let mut stmt = conn.prepare(sql)?;
        for (idx, result) in rdr.records().enumerate() {
            if load_cnt >= max {
                break;
            }
            let record = result.map_err(|e| {
                anyhow::anyhow!("read csv record failed at line {}: {}", idx + 1, e)
            })?;

            // Basic bounds check to avoid panic on bad column indices
            if let Some(max_col) = cols.iter().max()
                && *max_col >= record.len()
            {
                return Err(anyhow::anyhow!(
                    "csv has insufficient columns at line {}: need index {}, got {} columns",
                    idx + 1,
                    *max_col,
                    record.len()
                ));
            }

            // Unified dynamic binding (strict): any missing column is an error
            let mut vec: Vec<&str> = Vec::with_capacity(cols.len());
            for &ci in &cols {
                let v = record
                    .get(ci)
                    .ok_or_else(|| anyhow::anyhow!("line {} col {} missing", idx + 1, ci))?;
                vec.push(v);
            }
            let params = rusqlite::params_from_iter(vec);
            stmt.execute(params)?;
            load_cnt += 1;
        }
        info_kdb!("from {} load data cnt: {}", csv_path.display(), load_cnt);
        Ok(load_cnt)
    }

    pub fn check_data(&self, table: &str, scope: (usize, usize)) -> KnowledgeResult<usize> {
        let conn = self.conn.get().owe_res()?;
        let count_sql = format!("select count(*) from {}", table);
        let count: usize = conn
            .query_row(count_sql.as_str(), (), |row| row.get(0))
            .owe_rule()?;
        if count >= scope.0 {
            Ok(count)
        } else {
            KnowledgeReason::from_conf("table data less")
                .err_result()
                .with(("table", table))
                .with(("count", count.to_string()))

            /*
            Err(anyhow!(
                "data less! , load data count {} <= min {}",
                count,
                scope.0,
            ))
            */
        }
    }

    pub fn global() -> Self {
        Self {
            conn: MEM_SQLITE_INS.clone(),
        }
    }
    pub fn load_test() -> AnyResult<Self> {
        let db = Self::global();
        debug_kdb!("[memdb] load_test invoked");
        db.table_create(EXAMPLE_CREATE_SQL)?;
        // 通过 crate 根目录定位测试字典，避免 cwd 影响
        let csv = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/mem/dict/example.csv");
        let _ = db.table_clean(EXAMPLE_CLEAN_SQL);
        db.table_load(EXAMPLE_INSERT_SQL, csv, vec![0, 1], 100)?;
        // quick sanity check
        if let Ok(cnt) = db.check_data("example", (1, usize::MAX)) {
            debug_kdb!("[memdb] example rows loaded = {}", cnt);
        }
        Ok(db)
    }
}
pub const EXAMPLE_CREATE_SQL: &str = r#"CREATE TABLE IF NOT EXISTS example (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    pinying TEXT NOT NULL
    )"#;
pub const EXAMPLE_CLEAN_SQL: &str = "DELETE FROM example";
pub const EXAMPLE_INSERT_SQL: &str = r#"INSERT INTO example(name,pinying) VALUES (?1, ?2 ) "#;

#[cfg(test)]
mod tests {

    use std::{fs::File, io::Read};

    use super::*;
    // V1 TableConf removed
    use crate::mem::ToSqlParams;
    use anyhow::Context;
    use orion_conf::EnvTomlLoad;
    use orion_variate::EnvDict;
    use serde::Serialize;
    use std::fs;
    use wp_data_fmt::{Csv, DataFormat};

    #[test]
    fn test_load() -> AnyResult<()> {
        let db = MemDB::instance();
        db.table_create(EXAMPLE_CREATE_SQL)?;
        let loaded = db.table_load(
            EXAMPLE_INSERT_SQL,
            PathBuf::from("src/mem/dict/example.csv"),
            vec![0, 1],
            100,
        )?;
        assert_eq!(loaded, 10);
        let fmt = Csv::default();
        let tdos = db.query_row("select * from example;")?;
        for obj in tdos {
            println!("{}", fmt.format_field(&obj));
        }
        Ok(())
    }

    #[test]
    fn test_csv_off_by_one() -> AnyResult<()> {
        let db = MemDB::instance();
        db.table_create(EXAMPLE_CREATE_SQL)?;
        // Expect only 1 row loaded when max=1 (no off-by-one)
        let loaded = db.table_load(
            EXAMPLE_INSERT_SQL,
            PathBuf::from("src/mem/dict/example.csv"),
            vec![0, 1],
            1,
        )?;
        assert_eq!(loaded, 1);
        Ok(())
    }

    #[test]
    fn test_row_null_mapping() -> AnyResult<()> {
        let db = MemDB::instance();
        db.execute("CREATE TABLE tnull (v TEXT)")?;
        db.execute("INSERT INTO tnull (v) VALUES (NULL)")?;
        let row = db.query_row("SELECT v FROM tnull")?;
        assert_eq!(row.len(), 1);
        assert_eq!(row[0].get_name(), "v");
        // Ensure NULL becomes a Value::Null rather than panic
        assert!(matches!(row[0].get_value(), model::Value::Null));
        Ok(())
    }

    #[test]
    fn test_row_blob_mapping() -> AnyResult<()> {
        let db = MemDB::instance();
        db.execute("CREATE TABLE tblob (b BLOB)")?;
        // Insert ASCII 'ABC' as blob
        db.execute("INSERT INTO tblob (b) VALUES (X'414243')")?;
        let row = db.query_row("SELECT b FROM tblob")?;
        assert_eq!(row.len(), 1);
        assert_eq!(row[0].get_name(), "b");
        // lossy utf8 decode should yield "ABC"
        assert_eq!(row[0].to_string(), "chars(ABC)");
        Ok(())
    }

    #[test]
    fn test_csv_missing_column_error() -> AnyResult<()> {
        use std::fs;
        use std::io::Write;
        let db = MemDB::instance();
        db.table_create(EXAMPLE_CREATE_SQL)?;
        // Create a temp csv with only 1 column per row
        let mut path = std::env::temp_dir();
        path.push("wp_knowledge_csv_missing_col.csv");
        {
            let mut f = fs::File::create(&path)?;
            writeln!(f, "name")?;
            writeln!(f, "only_one_col")?;
        }
        let res = db.table_load(
            EXAMPLE_INSERT_SQL,
            path.clone(),
            vec![0, 1], // request 2 columns but csv has 1
            10,
        );
        assert!(res.is_err());
        let e = format!("{}", res.err().unwrap());
        assert!(e.contains("line"));
        assert!(e.contains("insufficient columns"));
        // cleanup
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn test_global_persistence_across_handles() -> AnyResult<()> {
        // Create table via one global handle
        {
            let db1 = MemDB::global();
            db1.execute("CREATE TABLE IF NOT EXISTS gtest (v TEXT)")?;
            db1.execute("INSERT INTO gtest (v) VALUES ('ok')")?;
        }
        // Read via a new global handle; should see the same in-memory DB
        {
            let db2 = MemDB::global();
            let rows = db2.query_row("SELECT v FROM gtest")?;
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].to_string(), "chars(ok)");
        }
        Ok(())
    }

    #[test]
    fn test_init_by_conf() -> AnyResult<()> {
        let db = MemDB::global();
        db.table_create(EXAMPLE_CREATE_SQL)?;
        let _ = db.table_clean(EXAMPLE_CLEAN_SQL);
        db.table_load(
            EXAMPLE_INSERT_SQL,
            PathBuf::from("src/mem/dict/example.csv"),
            vec![0, 1],
            100,
        )?;
        Ok(())
    }

    // V1 conf serde test removed

    #[test]
    fn test_alter_level() -> anyhow::Result<()> {
        let db = MemDB::global();
        // ensure clean state across global in-memory handle
        let _ = db.execute("DROP TABLE IF EXISTS alert_cat_level");
        db.table_create(
            r#"CREATE TABLE IF NOT EXISTS alert_cat_level (
                id   INTEGER PRIMARY KEY,
                log_type TEXT NOT NULL,
                level1_code TEXT NOT NULL,
                level1_name TEXT NOT NULL,
                level2_code TEXT NOT NULL,
                level2_name TEXT NOT NULL,
                original_code TEXT NOT NULL,
                original_name TEXT NOT NULL
            )"#,
        )?;
        let _ = db.table_clean("DELETE FROM alert_cat_level");
        db.table_load(
            r#"INSERT INTO alert_cat_level (log_type, level1_code, level1_name, level2_code, level2_name, original_code, original_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            PathBuf::from("src/mem/dict/event_cat_level.csv"),
            vec![0, 1, 2, 3, 4, 5, 6],
            2000,
        )?;

        let sql = "select level1_code from alert_cat_level where log_type = :log_type  and  original_code = :code ";
        let result = db.query_row_params(
            //"select level1_code from alert_cat_level where log_type = 'jowto_server_alert_log' and original_code = '00000002'",
            sql,
            &[(":log_type", "app_log"), (":code", "00000002")],
        )?;
        assert_eq!(result, vec![DataField::from_chars("level1_code", "105")]);

        let px = [
            SqlNamedParam(DataField::from_chars(":code", "00000002")),
            SqlNamedParam(DataField::from_chars(":log_type", "app_log")),
        ];

        let p = px.to_params();
        let result = db.query_row_params(sql, &p)?;
        assert_eq!(result, vec![DataField::from_chars("level1_code", "105")]);

        Ok(())
    }

    #[test]
    fn test_tosql_bind_various_types() -> AnyResult<()> {
        use chrono::NaiveDate;
        use std::net::{IpAddr, Ipv4Addr};
        use wp_model_core::model::types::value::ObjectValue;
        use wp_model_core::model::{DateTimeValue, HexT};

        let db = MemDB::instance();
        db.execute("CREATE TABLE p (v)")?;

        // Bool -> integer 1
        {
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::from_bool(":v", true))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert!(matches!(row[0].get_value(), model::Value::Digit(1)));
        }
        // Null
        {
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::new(
                model::DataType::default(),
                ":v",
                model::Value::Null,
            ))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert!(matches!(row[0].get_value(), model::Value::Null));
        }
        // Time -> text
        {
            let dt: DateTimeValue = NaiveDate::from_ymd_opt(2023, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::from_time(":v", dt))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert!(matches!(row[0].get_value(), model::Value::Chars(_)));
        }
        // Hex -> text
        {
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::from_hex(":v", HexT(0xABCD)))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert!(matches!(row[0].get_value(), model::Value::Chars(_)));
        }
        // IpAddr -> text
        {
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::from_ip(
                ":v",
                IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            ))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert_eq!(row[0].to_string(), "chars(1.2.3.4)");
        }
        // Obj -> text (debug)
        {
            let mut obj = ObjectValue::new();
            obj.insert("k".to_string(), DataField::from_chars("", "v"));
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::from_obj(":v", obj))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert!(matches!(row[0].get_value(), model::Value::Chars(_)));
        }
        // Array -> text (debug)
        {
            let arr = vec![DataField::from_chars("", "a"), DataField::from_digit("", 1)];
            let sql = "INSERT INTO p (v) VALUES (:v)";
            let p = [SqlNamedParam(DataField::from_arr(":v", arr))];
            db.query_row_params(sql, &p.to_params())?;
            let row = db.query_row("SELECT v FROM p ORDER BY rowid DESC LIMIT 1")?;
            assert!(matches!(row[0].get_value(), model::Value::Chars(_)));
        }
        Ok(())
    }

    #[test]
    fn test_column_alias_names() -> AnyResult<()> {
        let db = MemDB::instance();
        // Create a simple one-shot table/view using alias
        db.execute("CREATE TABLE ctest (a INTEGER, b TEXT)")?;
        db.execute("INSERT INTO ctest (a,b) VALUES (42,'x')")?;
        let row = db.query_row("SELECT a AS 'the number', b AS 'the text' FROM ctest LIMIT 1")?;
        assert_eq!(row.len(), 2);
        assert_eq!(row[0].get_name(), "the number");
        assert_eq!(row[1].get_name(), "the text");
        Ok(())
    }

    #[test]
    fn test_query_cipher_basic() -> AnyResult<()> {
        let db = MemDB::instance();
        db.execute("CREATE TABLE cipher (value TEXT)")?;
        db.execute("INSERT INTO cipher (value) VALUES ('A')")?;
        db.execute("INSERT INTO cipher (value) VALUES ('B')")?;
        let vals = db.query_cipher("cipher")?;
        assert!(vals.contains(&"A".to_string()));
        assert!(vals.contains(&"B".to_string()));
        Ok(())
    }

    #[test]
    fn test_concurrent_inserts() -> AnyResult<()> {
        use std::thread;
        let db = MemDB::global();
        db.execute("CREATE TABLE IF NOT EXISTS concur (v INTEGER)")?;
        let threads: Vec<_> = (0..4)
            .map(|_| {
                thread::spawn(|| {
                    let dbt = MemDB::global();
                    for _ in 0..10 {
                        let _ = dbt.execute("INSERT INTO concur (v) VALUES (1)");
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let row = db.query_row("SELECT SUM(v) AS total FROM concur")?;
        // total should be 40
        assert_eq!(row[0].to_string(), "digit(40)");
        Ok(())
    }

    #[test]
    fn test_query_returns_all_rows() -> AnyResult<()> {
        let db = MemDB::instance();
        db.execute("CREATE TABLE multi (id INTEGER, name TEXT)")?;
        let rows = db.query("SELECT * FROM multi")?;
        assert!(rows.is_empty(), "empty table should return empty vec");
        db.execute("INSERT INTO multi (id, name) VALUES (1, 'alice')")?;
        db.execute("INSERT INTO multi (id, name) VALUES (2, 'bob')")?;
        db.execute("INSERT INTO multi (id, name) VALUES (3, 'charlie')")?;

        let rows = db.query("SELECT id, name FROM multi ORDER BY id")?;
        assert_eq!(rows.len(), 3, "should return all 3 rows");

        Ok(())
    }

    #[allow(dead_code)]
    fn load_toml_conf<T: serde::de::DeserializeOwned>(path: &str) -> AnyResult<T> {
        let mut f = File::open(path).with_context(|| format!("conf file not found: {}", path))?;
        let mut buffer = Vec::with_capacity(10240);
        f.read_to_end(&mut buffer).expect("read conf file error");
        let conf_data = String::from_utf8(buffer)?;
        let dict = EnvDict::new();
        let conf: T = T::env_parse_toml(conf_data.as_str(), &dict)?;
        Ok(conf)
    }

    #[allow(dead_code)]
    fn export_toml_local<T: Serialize>(val: &T, path: &str) -> AnyResult<()> {
        let data = toml::to_string_pretty(val)?;
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, data)?;
        Ok(())
    }
}
