use orion_error::{OrionError, StructError};

#[derive(Debug, Clone, PartialEq, Serialize, OrionError)]
pub enum OMLRunReason {
    #[orion_error(identity = "biz.oml_fmt_conv")]
    FmtConv,
}

pub type OMLRunError = StructError<OMLRunReason>;

pub type OMLRunResult<T> = Result<T, OMLRunError>;
