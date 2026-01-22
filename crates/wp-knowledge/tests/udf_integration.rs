use std::fs;
use std::path::{Path, PathBuf};

use orion_variate::EnvDict;
use wp_config::test_support::ForTest;
use wp_knowledge::facade as kdb;

fn uniq_tmp_dir() -> PathBuf {
    use rand::{RngCore, rng};
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let rnd: u64 = rng().next_u64();
    std::env::current_dir()
        .unwrap()
        .join(format!(".tmp_udf_{}_{}", ts, rnd))
}

#[ignore = "ci error"]
#[test]
fn load_zone_with_udf_and_query() {
    // 1) build a minimal knowdb under tmp root
    let root = uniq_tmp_dir();
    let models = root.join("models").join("knowledge");
    let zone_dir = models.join("zone");
    fs::create_dir_all(&zone_dir).unwrap();
    // minimal knowdb.toml
    let knowdb = r#"
version = 2
[csv]
has_header = false

[[tables]]
name = "zone"
columns.by_index = [0,1,2]
[tables.expected_rows]
min = 1
"#;
    fs::write(models.join("knowdb.toml"), knowdb).unwrap();
    // DDL: integer columns + use in query
    let create_sql = r#"
CREATE TABLE IF NOT EXISTS {table} (
  id           INTEGER PRIMARY KEY,
  ip_start_int INTEGER NOT NULL,
  ip_end_int   INTEGER NOT NULL,
  zone         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_{table}_start ON {table}(ip_start_int);
CREATE INDEX IF NOT EXISTS idx_{table}_end   ON {table}(ip_end_int);
"#;
    fs::write(zone_dir.join("create.sql"), create_sql).unwrap();
    // DML: call ip4_int + trim_quotes UDF at load time
    let insert_sql = "INSERT INTO {table} (ip_start_int, ip_end_int, zone) VALUES (ip4_int(?1), ip4_int(?2), trim_quotes(?3));\n";
    fs::write(zone_dir.join("insert.sql"), insert_sql).unwrap();
    // 注意：zone 字段带引号并含前后空白，验证 trim_quotes 的清洗效果
    let data = "10.0.10.1, 10.0.90.255, \"work_zone\"\n10.0.100.1, 10.0.200.255,  'core_zone' \n";
    fs::write(zone_dir.join("data.csv"), data).unwrap();

    // 2) init authority + thread-cloned provider
    let conf_path = models.join("knowdb.toml");
    let auth_uri = format!(
        "file:{}/.run/authority.sqlite?mode=rwc&uri=true",
        root.display()
    );
    // Try to initialize, but don't fail if it's already initialized
    let _ = kdb::init_thread_cloned_from_knowdb(
        Path::new(&root),
        &conf_path,
        &auth_uri,
        &EnvDict::test_default(),
    );

    // 3) query with UDF on read connection
    let rows = kdb::query_named(
        "SELECT zone FROM zone WHERE ip4_between(:ip, ip_start_int, ip_end_int)=1 LIMIT 1",
        &[(":ip", &"10.0.10.5" as &dyn rusqlite::ToSql)],
    )
    .expect("query zone by ip");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].to_string(), "chars(work_zone)");

    let rows = kdb::query_named(
        "SELECT cidr4_contains(:ip, '10.0.0.0/8') AS ok",
        &[(":ip", &"10.1.2.3" as &dyn rusqlite::ToSql)],
    )
    .expect("query cidr contains");
    assert_eq!(rows[0].to_string(), "digit(1)");

    // 4) cleanup
    let _ = std::fs::remove_dir_all(&root);
}

// 与上一个用例相同的数据构造，但查询时采用推荐的整数比较写法
#[test]
fn load_zone_with_int_compare_query() {
    // 1) build a minimal knowdb under tmp root
    let root = uniq_tmp_dir();
    let models = root.join("models").join("knowledge");
    let zone_dir = models.join("zone");
    fs::create_dir_all(&zone_dir).unwrap();
    // minimal knowdb.toml
    let knowdb = r#"
version = 2
[csv]
has_header = false

[[tables]]
name = "zone"
columns.by_index = [0,1,2]
[tables.expected_rows]
min = 1
"#;
    fs::write(models.join("knowdb.toml"), knowdb).unwrap();
    // DDL: integer columns + use in query
    let create_sql = r#"
CREATE TABLE IF NOT EXISTS {table} (
  id           INTEGER PRIMARY KEY,
  ip_start_int INTEGER NOT NULL,
  ip_end_int   INTEGER NOT NULL,
  zone         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_{table}_start ON {table}(ip_start_int);
CREATE INDEX IF NOT EXISTS idx_{table}_end   ON {table}(ip_end_int);
"#;
    fs::write(zone_dir.join("create.sql"), create_sql).unwrap();
    // DML: call ip4_int + trim_quotes UDF at load time
    let insert_sql = "INSERT INTO {table} (ip_start_int, ip_end_int, zone) VALUES (ip4_int(?1), ip4_int(?2), trim_quotes(?3));\n";
    fs::write(zone_dir.join("insert.sql"), insert_sql).unwrap();
    // 注意：zone 字段带引号并含前后空白，验证 trim_quotes 的清洗效果
    let data = "10.0.10.1, 10.0.90.255, \"work_zone\"\n10.0.100.1, 10.0.200.255,  'core_zone' \n";
    fs::write(zone_dir.join("data.csv"), data).unwrap();

    // 2) init authority + thread-cloned provider
    let conf_path = models.join("knowdb.toml");
    let auth_uri = format!(
        "file:{}/.run/authority.sqlite?mode=rwc&uri=true",
        root.display()
    );
    // Try to initialize, but don't fail if it's already initialized
    let _ = kdb::init_thread_cloned_from_knowdb(
        Path::new(&root),
        &conf_path,
        &auth_uri,
        &EnvDict::test_default(),
    );

    // 3) query with integer compare on read connection
    let rows = kdb::query_named(
        "SELECT zone FROM zone WHERE ip_start_int <= ip4_int(:ip) AND ip_end_int >= ip4_int(:ip) LIMIT 1",
        &[(":ip", &"10.0.10.5" as &dyn rusqlite::ToSql)],
    )
    .expect("query zone by ip (int compare)");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].to_string(), "chars(work_zone)");

    // 4) cleanup
    let _ = std::fs::remove_dir_all(&root);
}
