//pub mod arsenal;
pub mod builtins;
mod desc;
mod err_test;
mod error;
mod mod_test;
mod runtime;
mod value;

pub fn vof<T, E>(val: Option<T>, default: E) -> T
where
    T: From<E>,
{
    val.unwrap_or(default.into())
}

pub use builtins::PipeLineResult;
pub use runtime::vm_unit::OPTIMIZE_TIMES;
pub use runtime::vm_unit::{DataResult, WplEvaluator};
pub use value::ParserFactory;
pub use value::data_type::DataTypeParser;
pub use wp_parse_api::{WparseError, WparseReason, WparseResult};
