pub mod field;

pub mod ann_func;
mod code;
pub mod debug;
pub mod fld_fmt;
pub mod group;
pub mod package;
pub mod processor;
pub mod rule;
mod syntax;

pub struct WplFmt<T>(pub T);
pub struct GenFmt<T>(pub T);

pub use code::WplCode;
pub use field::types::WplField;
pub use field::types::{DEFAULT_FIELD_KEY, DEFAULT_META_NAME, WplFieldSet};
pub use fld_fmt::WplFieldFmt;
pub use package::WplPackage;
pub use package::WplPkgMeta;
pub use processor::WplFun;
pub use processor::WplPipe;
pub use rule::meta::WplRuleMeta;
pub use rule::meta::WplTag;
pub use rule::types::{WplExpress, WplRule, WplStatementType};
pub use syntax::sep_pattern::{SepPattern, build_pattern};
pub use syntax::tag::{AnnEnum, AnnFun, TagKvs};
pub use syntax::wpl_sep::{DefaultSep, WplSep, WplSepT};
