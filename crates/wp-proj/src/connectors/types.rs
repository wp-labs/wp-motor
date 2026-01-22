#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize)]
pub enum LintSeverity {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub enum Side {
    Sources,
    Sinks,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::Sources => "sources",
            Side::Sinks => "sinks",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SilentErrKind {
    BadIdChars,
    SourcesIdMustEndSrc,
    SinksIdMustEndSink,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LintRow {
    pub scope: &'static str,
    pub file: String,
    pub id: String,
    pub kind: String,
    pub sev: LintSeverity,
    pub msg: String,
    #[serde(skip_serializing)]
    pub silent_err: Option<SilentErrKind>,
}
