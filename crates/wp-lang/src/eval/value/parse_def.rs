use std::sync::Arc;

use crate::generator::FieldGenConf;
use crate::generator::{FmtField, GenChannel};
use crate::types::AnyResult;
use wp_model_core::model::DataField;
use wp_model_core::model::FNameStr;
use wp_parser::WResult as ModalResult;

use crate::ast::{WplField, WplSep};
use crate::eval::runtime::field::FieldEvalUnit;

pub trait FieldParser {
    fn parse(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        f_name: Option<FNameStr>,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()>;

    fn generate(
        &self,
        gnc: &mut GenChannel,
        ups_sep: &WplSep,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<FmtField>;
}

pub trait PatternParser {
    fn pattern_parse(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()>;

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField>;
}

// Parser holders must be usable across threads to allow multi-threaded execution.
// Use Arc and require trait objects to be Send + Sync.
pub type ParserHold = Arc<dyn FieldParser + Send + Sync>;
pub type Hold<T> = Arc<T>;

/*
#[derive(Clone)]
pub struct ParseArgs {
    pub log_debug: bool,
}

impl Default for ParseArgs {
    fn default() -> Self {
        Self { log_debug: true }
    }
}

impl ParseArgs {
    pub fn new() -> Self {
        Self { log_debug: true }
    }
}
*/
