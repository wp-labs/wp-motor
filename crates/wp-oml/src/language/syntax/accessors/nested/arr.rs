use crate::language::prelude::*;
use std::fmt::{Display, Formatter};

#[derive(Builder, Debug, Clone, Getters, PartialEq)]
pub struct ArrOperation {
    pub dat_crate: DirectAccessor,
}

impl Display for ArrOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, " collect {}", self.dat_crate)?;
        Ok(())
    }
}
impl ArrOperation {
    pub fn new(dat_crate: DirectAccessor) -> Self {
        Self { dat_crate }
    }
}
