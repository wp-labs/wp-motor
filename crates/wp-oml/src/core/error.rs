use derive_more::From;
use orion_error::{OrionError, StructError};

#[derive(Debug, Clone, PartialEq, Serialize, From, OrionError)]
pub enum OMLRunReason {
    #[orion_error(identity = "biz.oml_fmt_conv")]
    FmtConv(String),
}

pub type OMLRunError = StructError<OMLRunReason>;

pub type OMLRunResult<T> = Result<T, OMLRunError>;
