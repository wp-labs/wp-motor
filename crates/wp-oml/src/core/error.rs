use orion_error::{DomainReason, StructError};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Serialize)]
pub enum OMLRunReason {
    #[error("format conv fail{0}")]
    FmtConv(String),
}
impl DomainReason for OMLRunReason {}

pub type OMLRunError = StructError<OMLRunReason>;

pub type OMLRunResult<T> = Result<T, OMLRunError>;
