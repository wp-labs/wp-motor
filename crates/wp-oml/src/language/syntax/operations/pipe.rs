use crate::language::{PipeFun, prelude::*};
#[derive(Builder, Debug, Clone, Getters)]
pub struct PiPeOperation {
    from: DirectAccessor,
    items: Vec<PipeFun>,
}

impl PiPeOperation {
    pub fn new(from: DirectAccessor, items: Vec<PipeFun>) -> Self {
        Self { from, items }
    }
}

impl Display for PiPeOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "pipe {}", &self.from)?;
        for i in &self.items {
            write!(f, "| {}", i)?;
        }
        write!(f, " ")
    }
}
