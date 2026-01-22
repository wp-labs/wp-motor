pub mod memdb;
pub mod params;
pub mod query_util;
mod stub;
pub mod thread_clone;

use std::fmt::Display;

use crate::mem::memdb::MDBEnum;
use crate::mem::memdb::MemDB;
use crate::mem::stub::StubMDB;
use enum_dispatch::enum_dispatch;
use rusqlite::Params;
use rusqlite::ToSql;
use wp_error::KnowledgeResult;
use wp_model_core::model::DataField;

pub type AnyResult<T> = anyhow::Result<T>;

pub type RowData = Vec<DataField>;

#[enum_dispatch]
pub trait DBQuery {
    fn query(&self, sql: &str) -> KnowledgeResult<Vec<RowData>>;
    fn query_row(&self, sql: &str) -> KnowledgeResult<RowData>;
    fn query_row_params<P: Params>(&self, sql: &str, params: P) -> KnowledgeResult<RowData>;
    fn query_row_tdos<P: Params>(
        &self,
        sql: &str,
        params: &[DataField; 2],
    ) -> KnowledgeResult<RowData>;

    fn query_cipher(&self, table: &str) -> KnowledgeResult<Vec<String>>;
}

pub trait ToSqlParams<'a, X> {
    fn to_params(&'a self) -> X;
}
pub type ParamT<'a> = (&'a str, &'a dyn ToSql);
#[derive(Debug)]
pub struct SqlNamedParam(pub DataField);

impl Display for SqlNamedParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
