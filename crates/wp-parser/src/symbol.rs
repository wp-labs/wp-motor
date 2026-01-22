use winnow::ascii::multispace0;
use winnow::combinator::alt;
use winnow::error::{StrContext, StrContextValue};
use winnow::token::literal;
use winnow::{ModalResult as WResult, Parser};

#[derive(Debug, PartialEq, Clone)]
pub enum LogicSymbol {
    And,
    Or,
    Not,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CmpSymbol {
    // width match =*
    We,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

// ============================================================================
// Macros for reducing boilerplate code
// ============================================================================

/// Macro to define simple symbol parsers that return `()`
macro_rules! define_unit_symbol {
    ($name:ident, $lit:expr, $desc:expr) => {
        #[doc = concat!("Parses the `", $lit, "` symbol.")]
        pub fn $name(data: &mut &str) -> WResult<()> {
            multispace0.parse_next(data)?;
            literal($lit)
                .context(StrContext::Label("symbol"))
                .context(StrContext::Expected(StrContextValue::Description($desc)))
                .parse_next(data)?;
            Ok(())
        }
    };
}

/// Macro to define logic symbol parsers that return `LogicSymbol`
macro_rules! define_logic_symbol {
    ($name:ident, $lit:expr, $desc:expr, $variant:expr) => {
        #[doc = concat!("Parses the `", $lit, "` logic operator.")]
        pub fn $name(data: &mut &str) -> WResult<LogicSymbol> {
            multispace0.parse_next(data)?;
            literal($lit)
                .context(StrContext::Label("symbol"))
                .context(StrContext::Expected(StrContextValue::Description($desc)))
                .parse_next(data)?;
            Ok($variant)
        }
    };
}

/// Macro to define comparison symbol parsers that return `CmpSymbol`
macro_rules! define_cmp_symbol {
    ($name:ident, $lit:expr, $label:expr, $desc:expr, $variant:expr) => {
        #[doc = concat!("Parses the `", $lit, "` comparison operator.")]
        pub fn $name(data: &mut &str) -> WResult<CmpSymbol> {
            multispace0.parse_next(data)?;
            literal($lit)
                .context(StrContext::Label($label))
                .context(StrContext::Expected(StrContextValue::Description($desc)))
                .parse_next(data)?;
            Ok($variant)
        }
    };
}

// ============================================================================
// Logic Operators
// ============================================================================

define_logic_symbol!(symbol_logic_and, "&&", "need '&&'", LogicSymbol::And);
define_logic_symbol!(symbol_logic_or, "||", "need '||'", LogicSymbol::Or);
define_logic_symbol!(symbol_logic_not, "!", "need '!'", LogicSymbol::Not);

// ============================================================================
// Punctuation and Delimiters
// ============================================================================

define_unit_symbol!(symbol_match_to, "=>", "need '=>'");
define_unit_symbol!(symbol_var, "var", "need 'var'");
define_unit_symbol!(symbol_comma, ",", "need ','");
define_unit_symbol!(symbol_bracket_beg, "(", "need '('");
define_unit_symbol!(symbol_bracket_end, ")", "need ')'");
define_unit_symbol!(symbol_brace_beg, "{", "need '{'");
define_unit_symbol!(symbol_brace_end, "}", "need '}'");
define_unit_symbol!(symbol_under_line, "_", "need '_'");
define_unit_symbol!(symbol_marvel, "!", "need '!'");
define_unit_symbol!(symbol_brackets_beg, "[", "need '['");
define_unit_symbol!(symbol_brackets_end, "]", "need ']'");
define_unit_symbol!(symbol_colon, ":", "need ':'");
define_unit_symbol!(symbol_semicolon, ";", "need ';'");
define_unit_symbol!(symbol_pipe, "|", "need '|' pipe symbol");
define_unit_symbol!(symbol_assign, "=", "need '='");
define_unit_symbol!(symbol_dollar, "$", "need '$'");

// ============================================================================
// Comparison Operators
// ============================================================================

define_cmp_symbol!(symbol_cmp_eq, "==", "symbol", "need '=='", CmpSymbol::Eq);
define_cmp_symbol!(symbol_cmp_we, "=*", "symbol", "need '=*'", CmpSymbol::We);
define_cmp_symbol!(symbol_cmp_ne, "!=", "symbol", "need '!='", CmpSymbol::Ne);
define_cmp_symbol!(symbol_cmp_ge, ">=", "symbol ge", "need '>='", CmpSymbol::Ge);
define_cmp_symbol!(symbol_cmp_gt, ">", "symbol gt", "need '>'", CmpSymbol::Gt);
define_cmp_symbol!(symbol_cmp_le, "<=", "symbol ge", "need '<='", CmpSymbol::Le);
define_cmp_symbol!(symbol_cmp_lt, "<", "symbol gt", "need '<'", CmpSymbol::Lt);

// ============================================================================
// Combined Parsers
// ============================================================================

/// Parses any comparison operator and returns the corresponding `CmpSymbol`.
///
/// Tries operators in a specific order to handle multi-character operators correctly.
/// For example, `>=` is tried before `>` to avoid parsing it as `>` followed by `=`.
pub fn symbol_cmp(data: &mut &str) -> WResult<CmpSymbol> {
    alt((
        symbol_cmp_eq,
        symbol_cmp_ne,
        symbol_cmp_we,
        symbol_cmp_le,
        symbol_cmp_ge,
        symbol_cmp_lt,
        symbol_cmp_gt,
    ))
    .parse_next(data)
}

/// Parses any logic operator and returns the corresponding `LogicSymbol`.
pub fn symbol_logic(data: &mut &str) -> WResult<LogicSymbol> {
    alt((symbol_logic_and, symbol_logic_or, symbol_logic_not)).parse_next(data)
}

// ============================================================================
// Helper Functions for Error Context
// ============================================================================

/// Creates a label context for winnow error reporting.
#[inline(always)]
pub fn ctx_label(label: &'static str) -> StrContext {
    StrContext::Label(label)
}

/// Creates a string literal context for winnow error reporting.
#[inline(always)]
pub fn ctx_literal(lit: &'static str) -> StrContext {
    StrContext::Expected(StrContextValue::StringLiteral(lit))
}

/// Creates a description context for winnow error reporting.
#[inline(always)]
pub fn ctx_desc(desc: &'static str) -> StrContext {
    StrContext::Expected(StrContextValue::Description(desc))
}
