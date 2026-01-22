use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum DataErrKind {
    #[error("format error : {0}\n{1:?} ")]
    FormatError(String, Option<String>),
    #[error("not complete")]
    NotComplete,
    #[error("no parse data: {0}")]
    ParseError(String),

    #[error("less data")]
    LessData,
    #[error("empty data")]
    EmptyData,
    #[error("missing struct field : {0}")]
    MissingStructField(String),
    #[error("missing configuration : {0}")]
    MissingConfiguration(String),
}

impl From<String> for DataErrKind {
    fn from(value: String) -> Self {
        DataErrKind::FormatError(value, None)
    }
}
