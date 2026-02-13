extern crate serde;

#[macro_use]
extern crate wp_log;

extern crate anyhow;

#[macro_use]
extern crate serde_derive;

extern crate async_trait;

extern crate winnow;

extern crate wp_knowledge as wp_know;

pub mod core;
pub mod language;
pub mod parser;

#[cfg(test)]
pub(crate) mod test_helpers;
#[cfg(test)]
mod test_utils;
pub mod types;

pub use core::{DataRecordRef, ExpEvaluator};
pub use parser::oml_parse_raw;

// 导出语义词典相关的公开 API
pub use core::evaluator::transform::pipe::semantic_dict_loader::{
    check_semantic_dict_config, generate_default_semantic_dict_config, init_semantic_dict,
    set_semantic_enabled,
};
