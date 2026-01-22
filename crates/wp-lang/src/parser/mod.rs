use winnow::combinator::empty;
use wp_parser::Parser;
use wp_parser::WResult;

use crate::ast::AnnFun;

pub mod constants;
pub mod datatype;
mod err_report;
pub mod error;
pub mod parse_code;
pub mod string;
pub mod utils;
pub mod wpl_anno;
pub mod wpl_field;
pub mod wpl_fun;
pub mod wpl_group;
pub mod wpl_pkg;
pub mod wpl_rule;

#[inline]
pub fn peek_input(input: &mut &str) -> WResult<()> {
    empty.parse_next(input)
}

pub trait MergeTags {
    fn merge_tags(&mut self, tags: &Option<AnnFun>);
}
