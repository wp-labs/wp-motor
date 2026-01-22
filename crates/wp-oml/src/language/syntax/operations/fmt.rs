use crate::language::prelude::*;

use super::record::RecordOperation;

#[derive(Default, Builder, Debug, Clone, Getters)]
pub struct FmtOperation {
    fmt_str: String,
    subs: Vec<RecordOperation>,
}

impl Display for FmtOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "fmt(\"{}\"", self.fmt_str)?;
        for i in self.subs() {
            write!(f, ", {}", i)?;
        }
        write!(f, ") ")
    }
}

impl FmtOperation {
    pub fn new(fmt_str: String, subs: Vec<RecordOperation>) -> Self {
        Self { fmt_str, subs }
    }
}
