use crate::language::{NestedBinding, prelude::*};

#[derive(Default, Builder, Debug, Clone, Getters)]
pub struct MapOperation {
    //target: AgaTarget,
    subs: Vec<NestedBinding>,
}

impl Display for MapOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, " object {{")?;
        for sub in &self.subs {
            writeln!(f, "{}", sub)?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}
impl MapOperation {
    pub fn new() -> Self {
        Self {
            subs: Vec::with_capacity(5),
        }
    }
    pub fn append(&mut self, mut subs: Vec<NestedBinding>) {
        self.subs.append(&mut subs)
    }
}
