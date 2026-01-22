use std::fmt::Display;
pub mod symbols;
#[derive(Debug, PartialEq, Clone)]
pub enum LogicOperator {
    And,
    Or,
    Not,
}
impl Display for LogicOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogicOperator::And => write!(f, "&&"),
            LogicOperator::Or => write!(f, "||"),
            LogicOperator::Not => write!(f, "!"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CmpOperator {
    // width match =*
    We,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}
impl Display for CmpOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmpOperator::We => write!(f, "=*"),
            CmpOperator::Eq => write!(f, "=="),
            CmpOperator::Ne => write!(f, "!="),
            CmpOperator::Gt => write!(f, ">"),
            CmpOperator::Ge => write!(f, ">="),
            CmpOperator::Lt => write!(f, "<"),
            CmpOperator::Le => write!(f, "<="),
        }
    }
}
