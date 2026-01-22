use super::{CmpOperator, LogicOperator};

/*
pub trait SymbolProvider {
    fn logic_and() -> &'static str;
    fn logic_not() -> &'static str;
    fn logic_or() -> &'static str;

    fn cmp_eq() -> &'static str;
    fn cmp_wildcard() -> &'static str;
    fn cmp_ne() -> &'static str;
    fn cmp_ge() -> &'static str;
    fn cmp_gt() -> &'static str;
    fn cmp_le() -> &'static str;
    fn cmp_lt() -> &'static str;

    fn format_var(name: &str) -> String;
}
*/
pub trait SymbolProvider: LogicSymbolProvider + CmpSymbolProvider {}

pub trait LogicSymbolProvider {
    fn symbol_and() -> &'static str;
    fn symbol_not() -> &'static str;
    fn symbol_or() -> &'static str;
    fn symbol_logic(op: &LogicOperator) -> &'static str {
        match op {
            LogicOperator::And => Self::symbol_and(),
            LogicOperator::Or => Self::symbol_or(),
            LogicOperator::Not => Self::symbol_not(),
        }
    }
}

pub trait CmpSymbolProvider {
    fn symbol_eq() -> &'static str;
    fn symbol_we() -> &'static str;
    fn symbol_ne() -> &'static str;
    fn symbol_ge() -> &'static str;
    fn symbol_gt() -> &'static str;
    fn symbol_le() -> &'static str;
    fn symbol_lt() -> &'static str;

    fn symbol_var(name: &str) -> String;
    fn symbol_cmp(op: &CmpOperator) -> &'static str {
        match op {
            CmpOperator::We => Self::symbol_we(),
            CmpOperator::Eq => Self::symbol_eq(),
            CmpOperator::Ne => Self::symbol_ne(),
            CmpOperator::Gt => Self::symbol_gt(),
            CmpOperator::Ge => Self::symbol_ge(),
            CmpOperator::Lt => Self::symbol_lt(),
            CmpOperator::Le => Self::symbol_le(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SQLSymbol {}

#[derive(Debug, PartialEq, Clone)]
pub struct RustSymbol {}

impl LogicSymbolProvider for SQLSymbol {
    fn symbol_and() -> &'static str {
        "and"
    }

    fn symbol_not() -> &'static str {
        "not"
    }

    fn symbol_or() -> &'static str {
        "or"
    }
}

impl CmpSymbolProvider for SQLSymbol {
    fn symbol_eq() -> &'static str {
        "="
    }

    fn symbol_we() -> &'static str {
        "="
    }

    fn symbol_ne() -> &'static str {
        "!="
    }

    fn symbol_ge() -> &'static str {
        ">="
    }

    fn symbol_gt() -> &'static str {
        ">"
    }

    fn symbol_le() -> &'static str {
        "<="
    }

    fn symbol_lt() -> &'static str {
        "<"
    }

    fn symbol_var(name: &str) -> String {
        name.to_string()
    }
}
impl LogicSymbolProvider for RustSymbol {
    fn symbol_and() -> &'static str {
        "&&"
    }

    fn symbol_not() -> &'static str {
        "!"
    }

    fn symbol_or() -> &'static str {
        "||"
    }
}
impl CmpSymbolProvider for RustSymbol {
    fn symbol_eq() -> &'static str {
        "=="
    }

    fn symbol_we() -> &'static str {
        "=*"
    }

    fn symbol_ne() -> &'static str {
        "!="
    }

    fn symbol_ge() -> &'static str {
        ">="
    }

    fn symbol_gt() -> &'static str {
        ">"
    }

    fn symbol_le() -> &'static str {
        "<="
    }

    fn symbol_lt() -> &'static str {
        "<"
    }
    fn symbol_var(name: &str) -> String {
        format!("${}", name)
    }
}

impl SymbolProvider for RustSymbol {}
impl SymbolProvider for SQLSymbol {}
