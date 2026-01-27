#[macro_use]
extern crate log as _;
#[macro_use]
extern crate serde;

extern crate winnow;

mod ast;
pub mod eval;
pub mod parser;
#[macro_use]
pub mod macro_def;
//mod checker;
mod extend;
pub mod generator;
mod pkg;
pub mod precompile;
mod setting;
pub mod traits;
pub mod types;
pub mod util;
//pub mod wasm;

pub use ast::WplCode;
pub use ast::WplRule;
pub use ast::WplSep;
pub use ast::WplStatementType;
pub use ast::ann_func::{AnnotationFunc, AnnotationType};
pub use ast::{WplExpress, WplPackage, WplPkgMeta};
pub use eval::DataTypeParser;
pub use eval::OPTIMIZE_TIMES;
pub use eval::PipeLineResult;
pub use eval::WplEvaluator;
pub use eval::builtins::registry::{
    create_pipe_unit as create_preorder_pipe_unit, list_pipe_units as list_preorder_pipe_units,
    register_pipe_unit as register_preorder_pipe_unit,
    register_wpl_pipe_batch as register_preorder_pipe_unit_batch,
};
// Note: DataResult is now provided by wp-parse-api for plugin development
pub use eval::{WparseError, WparseReason, WparseResult};
pub use parser::error::error_detail;
pub use parser::parse_code::wpl_express;
pub use parser::wpl_pkg::wpl_package;
pub use pkg::DEFAULT_KEY;
pub use pkg::PkgID;
pub use pkg::gen_pkg_id;
pub use setting::{PattenMode, WplSetting, check_level_or_stop};
//pub use engine::field::parser::base::DigitRange;
pub use ast::AnnFun;
pub use eval::ParserFactory;
pub use parser::error::{WplCodeError, WplCodeResult};
pub use precompile::{CompiledRule as WplCompiledRule, compile_rule as wpl_compile_rule};
// Deprecated: Use wp_parse_api::Parsable instead
// pub use traits::RawParseAble;
