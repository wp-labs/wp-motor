use std::path::Path;
use std::sync::Arc;

use std::collections::HashSet;
use std::sync::OnceLock;
use wp_data_model::cache::CacheAble;
use wp_error::{KnowledgeReason, KnowledgeResult};
use wp_log::info_ctrl;
use wp_model_core::model::DataField;

use crate::DBQuery;
use crate::mem::RowData;
use crate::mem::memdb::MemDB;
use crate::mem::thread_clone::ThreadClonedMDB;
//use anyhow::{anyhow, Result};
use orion_error::{ErrorWith, ToStructError, UvsLogicFrom};
use rusqlite::ToSql;
use rusqlite::{Connection, OpenFlags};

/// 对外统一查询门面，隐藏底层 MemDB/线程副本等实现选择。
/// 仅提供对象安全的两种查询接口：无参和命名参数。
pub trait QueryFacade: Send + Sync {
    fn query(&self, sql: &str) -> KnowledgeResult<Vec<RowData>>;
    fn query_row(&self, sql: &str) -> KnowledgeResult<RowData>;
    fn query_named<'a>(
        &self,
        sql: &str,
        params: &'a [(&'a str, &'a dyn ToSql)],
    ) -> KnowledgeResult<RowData>;
    fn query_cipher(&self, table: &str) -> KnowledgeResult<Vec<String>>;
}

impl QueryFacade for ThreadClonedMDB {
    fn query(&self, sql: &str) -> KnowledgeResult<Vec<RowData>> {
        DBQuery::query(self, sql)
    }
    fn query_row(&self, sql: &str) -> KnowledgeResult<RowData> {
        DBQuery::query_row(self, sql)
    }
    fn query_named<'a>(
        &self,
        sql: &str,
        params: &'a [(&'a str, &'a dyn ToSql)],
    ) -> KnowledgeResult<RowData> {
        DBQuery::query_row_params(self, sql, params)
    }
    fn query_cipher(&self, table: &str) -> KnowledgeResult<Vec<String>> {
        DBQuery::query_cipher(self, table)
    }
}

struct MemProvider(MemDB);
impl QueryFacade for MemProvider {
    fn query(&self, sql: &str) -> KnowledgeResult<Vec<RowData>> {
        DBQuery::query(&self.0, sql)
    }
    fn query_row(&self, sql: &str) -> KnowledgeResult<RowData> {
        DBQuery::query_row(&self.0, sql)
    }
    fn query_named<'a>(
        &self,
        sql: &str,
        params: &'a [(&'a str, &'a dyn ToSql)],
    ) -> KnowledgeResult<RowData> {
        DBQuery::query_row_params(&self.0, sql, params)
    }
    fn query_cipher(&self, table: &str) -> KnowledgeResult<Vec<String>> {
        DBQuery::query_cipher(&self.0, table)
    }
}

static PROVIDER: OnceLock<Arc<dyn QueryFacade>> = OnceLock::new();
static TABLE_WHITELIST: OnceLock<HashSet<String>> = OnceLock::new();

/// 直接使用已有的权威库 URI 初始化线程副本 provider。
pub fn init_thread_cloned_from_authority(authority_uri: &str) -> KnowledgeResult<()> {
    let tc = ThreadClonedMDB::from_authority(authority_uri);
    set_provider(Arc::new(tc))
}

/// （备选）使用内存/文件 MemDB 作为 provider。
pub fn init_mem_provider(memdb: MemDB) -> KnowledgeResult<()> {
    let res = set_provider(Arc::new(MemProvider(memdb)));
    if res.is_err() {
        eprintln!("[kdb] provider already initialized");
    } else {
        eprintln!("[kdb] provider set to MemProvider");
    }
    res
}

fn set_provider(p: Arc<dyn QueryFacade>) -> KnowledgeResult<()> {
    PROVIDER
        .set(p)
        .map_err(|_| KnowledgeReason::from_logic("knowledge provider already initialized").to_err())
}

fn get_provider() -> KnowledgeResult<&'static Arc<dyn QueryFacade>> {
    PROVIDER
        .get()
        .ok_or_else(|| KnowledgeReason::from_logic("knowledge provider not initialized").to_err())
}

pub fn query(sql: &str) -> KnowledgeResult<Vec<RowData>> {
    get_provider()?.query(sql)
}

/// 门面查询：无参
pub fn query_row(sql: &str) -> KnowledgeResult<RowData> {
    get_provider()?.query_row(sql)
}

/// 门面查询：命名参数
pub fn query_named<'a>(
    sql: &str,
    params: &'a [(&'a str, &'a dyn ToSql)],
) -> KnowledgeResult<RowData> {
    get_provider()?.query_named(sql, params)
}

/// 读取密文字典表（单列表 `value`），用于隐私脱敏加载词表
pub fn query_cipher(table: &str) -> KnowledgeResult<Vec<String>> {
    if let Some(wl) = TABLE_WHITELIST.get()
        && !wl.contains(table)
    {
        return KnowledgeReason::from_logic("table not allowed by knowdb whitelist")
            .err_result()
            .with(("table", table));
    }
    get_provider()?.query_cipher(table)
}

/// 带缓存的查询门面：
/// - `c_params` 用于上层缓存键（通常为 `[md5, :k1, :k2, ...]`）
/// - `named_params` 为 SQLite 命名参数切片（可由 `SqlNamedParam` 数组通过 `to_params()` 生成）
///   命中缓存直接返回；未命中则通过全局 provider 查询并回填缓存。
pub fn cache_query<const N: usize>(
    sql: &str,
    c_params: &[DataField; N],
    named_params: &[(&str, &dyn ToSql)],
    cache: &mut impl CacheAble<DataField, RowData, N>,
) -> RowData {
    crate::cache_util::cache_query_impl(c_params, cache, || {
        if named_params.is_empty() {
            get_provider().and_then(|p| p.query_row(sql))
        } else {
            get_provider().and_then(|p| p.query_named(sql, named_params))
        }
    })
}

fn ensure_wal(authority_uri: &str) -> KnowledgeResult<()> {
    // Try to enable WAL on authority DB (ignore if already set)
    if let Ok(conn) = Connection::open_with_flags(
        authority_uri,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI,
    ) {
        let _ = conn.execute_batch(
            "PRAGMA journal_mode=WAL;\nPRAGMA synchronous=NORMAL;\nPRAGMA temp_store=MEMORY;",
        );
    }
    Ok(())
}

/// 初始化基于文件库 + WAL + 多连接池的 Provider（已有权威库）
pub fn init_wal_pool_from_authority(authority_uri: &str, pool_size: u32) -> KnowledgeResult<()> {
    ensure_wal(authority_uri)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let mem = MemDB::new_file(authority_uri, pool_size, flags)?;
    init_mem_provider(mem)
}

/// 设置基于 V2 KnowDB 的全局 Provider：
/// - 构建权威库文件（CSV → SQLite），并启用线程克隆 Provider；
/// - 同时设置表名白名单（仅允许访问配置中声明的表）。
pub fn init_thread_cloned_from_knowdb(
    root: &Path,
    knowdb_conf: &Path,
    authority_uri: &str,
    dict: &orion_variate::EnvDict,
) -> KnowledgeResult<()> {
    let tables =
        crate::loader::build_authority_from_knowdb(root, knowdb_conf, authority_uri, dict)?;
    // 使用只读 URI 暴露给线程克隆
    let ro_uri = if let Some(rest) = authority_uri.strip_prefix("file:") {
        let path_part = rest.split('?').next().unwrap_or(rest);
        format!("file:{}?mode=ro&uri=true", path_part)
    } else {
        authority_uri.to_string()
    };
    let tc = ThreadClonedMDB::from_authority(&ro_uri);

    // Pre-load the database into memory to avoid file deletion issues
    // This ensures the database is copied immediately rather than lazily
    #[cfg(test)]
    {
        // In test mode, immediately trigger the lazy loading
        tc.with_tls_conn(|_| Ok(()))?;
    }

    let _ = TABLE_WHITELIST.set(tables.into_iter().collect::<HashSet<_>>());
    info_ctrl!("init authority knowdb success({}) ", knowdb_conf.display(),);
    set_provider(Arc::new(tc))
}
