use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use orion_conf::EnvTomlLoad;
use serde::Deserialize;
use wp_log::info_ctrl;

use crate::mem::memdb::MemDB;
use orion_error::{ContextRecord, ErrorOwe, OperationContext, ToStructError, UvsConfFrom};
use orion_variate::EnvDict;
use rusqlite::OpenFlags;
use wp_error::{KnowledgeReason, KnowledgeResult};

/// V2 KnowDB 配置：目录式 + 外置 SQL。仅支持单一数据文件：`<table_dir>/data.csv`，
/// 或通过 `tables[n].data_file` 相对 `<table_dir>` 指定。
#[derive(Debug, Deserialize)]
pub struct KnowDbConf {
    pub version: u32,
    #[serde(default = "default_dot")]
    pub base_dir: String,
    #[serde(default)]
    pub default: OptLoadSpec,
    #[serde(default)]
    pub csv: CsvSpec,
    pub tables: Vec<TableSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptLoadSpec {
    #[serde(default = "default_true")]
    pub transaction: bool,
    #[serde(default = "default_batch")]
    pub batch_size: usize,
    #[serde(default = "default_on_error")]
    pub on_error: OnError,
}
impl Default for OptLoadSpec {
    fn default() -> Self {
        Self {
            transaction: true,
            batch_size: default_batch(),
            on_error: default_on_error(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    #[default]
    Fail,
    Skip,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CsvSpec {
    #[serde(default = "default_true")]
    pub has_header: bool,
    #[serde(default = "default_comma")]
    pub delimiter: String,
    #[serde(default = "default_utf8")]
    pub encoding: String,
    #[serde(default = "default_true")]
    pub trim: bool,
}
impl Default for CsvSpec {
    fn default() -> Self {
        CsvSpec {
            has_header: true,
            delimiter: ",".into(),
            encoding: "utf-8".into(),
            trim: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableSpec {
    pub name: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub data_file: Option<String>,
    pub columns: ColumnsSpec,
    #[serde(default)]
    pub expected_rows: RowExpect,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColumnsSpec {
    #[serde(default)]
    pub by_header: Vec<String>,
    #[serde(default)]
    pub by_index: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RowExpect {
    pub min: Option<usize>,
    pub max: Option<usize>,
}

const fn default_true() -> bool {
    true
}
const fn default_batch() -> usize {
    2000
}
fn default_comma() -> String {
    ",".to_string()
}
fn default_utf8() -> String {
    "utf-8".to_string()
}
fn default_on_error() -> OnError {
    OnError::Fail
}
fn default_dot() -> String {
    ".".to_string()
}

/// 读取文本文件，返回字符串
fn read_to_string(path: &Path) -> KnowledgeResult<String> {
    let mut f = fs::File::open(path).owe_res()?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).owe_res()?;
    Ok(buf)
}

fn replace_table(sql: &str, table: &str) -> String {
    sql.replace("{table}", table)
}

fn join_rel(base: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

pub fn build_authority_from_knowdb(
    root: &Path,
    conf_path: &Path,
    authority_uri: &str,
    dict: &EnvDict,
) -> KnowledgeResult<Vec<String>> {
    let mut opx = OperationContext::want("build authority from knowdb").with_auto_log();
    // 1) 解析配置与 base_dir
    let (conf, conf_abs, base_dir) = parse_knowdb_conf(root, conf_path, dict)?;
    opx.record("conf", &conf_abs);
    opx.record("base_dir", &base_dir);
    // 2) 打开权威库
    let db = open_authority(authority_uri)?;
    // 3) 逐表加载（按配置顺序）；不再处理显式依赖
    let mut loaded_names = Vec::new();
    for t in &conf.tables {
        if !t.enabled {
            continue;
        }
        load_one_table(&db, &base_dir, t, &conf.csv, &conf.default)?;
        info_ctrl!("load table {} suc!", base_dir.display(),);
        loaded_names.push(t.name.clone());
    }
    opx.mark_suc();
    Ok(loaded_names)
}

fn parse_knowdb_conf(
    root: &Path,
    conf_path: &Path,
    dict: &EnvDict,
) -> KnowledgeResult<(KnowDbConf, PathBuf, PathBuf)> {
    let conf_abs = if conf_path.is_absolute() {
        conf_path.to_path_buf()
    } else {
        root.join(conf_path)
    };
    let conf_txt = read_to_string(&conf_abs)?;
    let conf: KnowDbConf =
        <KnowDbConf as EnvTomlLoad<KnowDbConf>>::env_parse_toml(&conf_txt, dict).owe_conf()?;
    if conf.version != 2 {
        return KnowledgeReason::from_conf("unsupported knowdb.version").err_result();
    }
    let conf_dir = conf_abs.parent().unwrap_or_else(|| Path::new("."));
    let base_dir = join_rel(conf_dir, &conf.base_dir);
    Ok((conf, conf_abs, base_dir))
}

fn open_authority(authority_uri: &str) -> KnowledgeResult<MemDB> {
    ensure_parent_dir_for_file_uri(authority_uri);
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;
    let db = MemDB::new_file(authority_uri, 1, flags)?;
    // 预注册内置 UDF 至权威库连接（注意：连接池可能返回不同连接，导入时也会再次注册）
    let _ = db.with_conn(|conn| {
        let _ = crate::sqlite_ext::register_builtin(conn);
        Ok::<(), anyhow::Error>(())
    });
    Ok(db)
}

/// Kahn 拓扑排序：返回按依赖顺序的表索引列表。
/// no topo_sort_tables: V2 简化版按配置顺序加载
fn ensure_parent_dir_for_file_uri(uri: &str) {
    if let Some(rest) = uri.strip_prefix("file:") {
        let path_part = rest.split('?').next().unwrap_or(rest);
        let p = Path::new(path_part);
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
    }
}

fn load_one_table(
    db: &MemDB,
    base_dir: &Path,
    t: &TableSpec,
    csvd: &CsvSpec,
    load: &OptLoadSpec,
) -> KnowledgeResult<()> {
    // 目录与必须文件
    let mut opx = OperationContext::want("load table to kdb")
        .with_auto_log()
        .with_mod_path("ctrl");
    let dir_name: &str = t.dir.as_deref().unwrap_or(&t.name);
    let table_dir = base_dir.join(dir_name);
    opx.record("table_dir", &table_dir);
    let create_sql = replace_table(&read_to_string(&table_dir.join("create.sql"))?, &t.name);
    let insert_sql = replace_table(&read_to_string(&table_dir.join("insert.sql"))?, &t.name);
    let clean_path = table_dir.join("clean.sql");
    let clean_sql = if clean_path.exists() {
        replace_table(&read_to_string(&clean_path)?, &t.name)
    } else {
        format!("DELETE FROM {}", &t.name)
    };

    // 建表与清理
    db.with_conn(|conn| {
        // 注册内置 UDF（导入连接）
        let _ = crate::sqlite_ext::register_builtin(conn);
        conn.execute_batch(&create_sql)?;
        conn.execute_batch(&clean_sql)?;
        Ok::<(), anyhow::Error>(())
    })
    .owe_res()?;

    // 数据源
    let data_path = match &t.data_file {
        Some(rel) => join_rel(&table_dir, rel),
        None => table_dir.join("data.csv"),
    };
    if !data_path.exists() {
        return KnowledgeReason::from_conf("data.csv not found").err_result();
    }
    opx.record("data_path", &data_path);

    // CSV 解析器
    let mut rdr = build_csv_reader(csvd, &data_path)?;

    // 列映射
    let col_indices: Vec<usize> = if !t.columns.by_header.is_empty() {
        let headers = rdr.headers().owe_res()?;
        select_indices_by_header(headers, &t.columns.by_header)?
    } else if !t.columns.by_index.is_empty() {
        t.columns.by_index.clone()
    } else {
        return KnowledgeReason::from_conf("columns mapping required").err_result();
    };

    // 导入（分批事务）
    let mut inserted: usize = 0;
    let mut bad: usize = 0;
    let mut batch_left = load.batch_size.max(1);
    db.with_conn(|conn| {
        // 注册内置 UDF（用于 INSERT 绑定表达式）
        let _ = crate::sqlite_ext::register_builtin(conn);
        let mut tx = if load.transaction {
            Some(conn.unchecked_transaction()?)
        } else {
            None
        };
        let mut stmt = conn.prepare(&insert_sql)?;
        for rec in rdr.into_records() {
            match rec {
                Ok(record) => {
                    let refs = extract_row_refs(&record, &col_indices, &mut bad, load)?;
                    if let Some(refs) = refs {
                        stmt.execute(rusqlite::params_from_iter(refs))?;
                        inserted += 1;
                        if load.transaction {
                            batch_left -= 1;
                            if batch_left == 0 {
                                tx.take().unwrap().commit()?;
                                tx = Some(conn.unchecked_transaction()?);
                                batch_left = load.batch_size.max(1);
                            }
                        }
                    }
                }
                Err(_e) => {
                    if matches!(load.on_error, OnError::Skip) {
                        bad += 1;
                        continue;
                    } else {
                        anyhow::bail!("csv record parse error");
                    }
                }
            }
        }
        if let Some(tx) = tx {
            tx.commit()?;
        }
        Ok::<(), anyhow::Error>(())
    })
    .owe_res()?;

    // 行数校验
    if let Some(min) = t.expected_rows.min
        && inserted < min
    {
        return KnowledgeReason::from_conf("table data less").err_result();
    }
    if let Some(max) = t.expected_rows.max
        && inserted > max
    {
        wp_log::warn_kdb!(
            "table {} loaded rows {} exceed max {}",
            &t.name,
            inserted,
            max
        );
    }
    if bad > 0 {
        wp_log::warn_kdb!("table {} skipped {} bad rows (on_error=skip)", &t.name, bad);
    }
    opx.mark_suc();
    Ok(())
}

fn build_csv_reader(
    csvd: &CsvSpec,
    data_path: &Path,
) -> KnowledgeResult<csv::Reader<std::fs::File>> {
    if csvd.encoding.to_lowercase() != "utf-8" {
        return KnowledgeReason::from_conf("only utf-8 csv is supported").err_result();
    }
    let mut rdr_b = csv::ReaderBuilder::new();
    rdr_b.has_headers(csvd.has_header);
    if csvd.delimiter.len() == 1 {
        rdr_b.delimiter(csvd.delimiter.as_bytes()[0]);
    }
    if csvd.trim {
        rdr_b.trim(csv::Trim::All);
    }
    rdr_b.from_path(data_path).owe_res()
}

fn select_indices_by_header(
    headers: &csv::StringRecord,
    wanted: &[String],
) -> KnowledgeResult<Vec<usize>> {
    let mut out = Vec::with_capacity(wanted.len());
    for name in wanted {
        let pos = headers
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| KnowledgeReason::from_conf("header not found").to_err())?;
        out.push(pos);
    }
    Ok(out)
}

fn extract_row_refs<'a>(
    record: &'a csv::StringRecord,
    col_indices: &[usize],
    bad: &mut usize,
    load: &OptLoadSpec,
) -> anyhow::Result<Option<Vec<&'a str>>> {
    let mut vs: Vec<&str> = Vec::with_capacity(col_indices.len());
    for &idx in col_indices {
        if idx >= record.len() {
            if matches!(load.on_error, OnError::Skip) {
                *bad += 1;
                return Ok(None);
            } else {
                // 将错误在调用方 bail（构建 anyhow）
                anyhow::bail!("missing column at index {}", idx);
            }
        }
        vs.push(record.get(idx).unwrap_or(""));
    }
    Ok(Some(vs))
}
