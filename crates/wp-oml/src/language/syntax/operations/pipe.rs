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

    pub fn from_mut(&mut self) -> &mut DirectAccessor {
        &mut self.from
    }

    pub fn items_mut(&mut self) -> &mut Vec<PipeFun> {
        &mut self.items
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
