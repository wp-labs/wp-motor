use crate::language::{AccessDirectOperation, PipeFun, PreciseEvaluator, prelude::*};

/// 管道源：访问器（read/take）、`access_direct`，或任意求值原子
/// （`fmt`/`match`/`select`/`object` 等）。
#[derive(Clone, Debug)]
pub enum PipeSource {
    Accessor(DirectAccessor),
    AccessDirect(AccessDirectOperation),
    Precise(Box<PreciseEvaluator>),
}

impl Display for PipeSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PipeSource::Accessor(a) => write!(f, "{}", a),
            PipeSource::AccessDirect(op) => write!(f, "{}", op),
            PipeSource::Precise(op) => write!(f, "{}", op),
        }
    }
}

#[derive(Builder, Debug, Clone, Getters)]
pub struct PiPeOperation {
    from: PipeSource,
    items: Vec<PipeFun>,
}

impl PiPeOperation {
    pub fn new(from: PipeSource, items: Vec<PipeFun>) -> Self {
        Self { from, items }
    }

    pub fn from_mut(&mut self) -> &mut PipeSource {
        &mut self.from
    }

    pub fn items_mut(&mut self) -> &mut Vec<PipeFun> {
        &mut self.items
    }
}

impl Display for PiPeOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "pipe {}", self.from)?;
        for i in &self.items {
            write!(f, "| {}", i)?;
        }
        write!(f, " ")
    }
}
