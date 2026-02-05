use crate::language::{
    prelude::*,
    syntax::{accessors::direct::read::FieldRead, bindings::generic::GenericBinding},
};
use std::fmt::{Display, Formatter};

#[derive(Builder, Debug, Clone, Getters, PartialEq)]
pub struct RecordOperation {
    pub dat_get: DirectAccessor,
    #[builder(default)]
    pub default_val: Option<GenericBinding>,
}
impl Default for RecordOperation {
    fn default() -> Self {
        Self {
            dat_get: DirectAccessor::Read(FieldRead::default()),
            default_val: None,
        }
    }
}

impl Display for RecordOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ", self.dat_get)?;
        if let Some(d_val) = &self.default_val {
            write!(f, "{{ _ : {} }}", d_val.accessor())?;
        }
        write!(f, " ")
    }
}
impl RecordOperation {
    pub fn new(dat_get: DirectAccessor) -> Self {
        Self {
            dat_get,
            default_val: None,
        }
    }

    pub fn dat_get_mut(&mut self) -> &mut DirectAccessor {
        &mut self.dat_get
    }
}
