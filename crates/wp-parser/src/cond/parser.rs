use crate::cond::{CmpParser, ConditionParser, SymbolFrom};
use crate::symbol::{CmpSymbol, LogicSymbol, ctx_desc, symbol_bracket_beg};
use orion_exp::operator::symbols::SymbolProvider;
use orion_exp::{CmpOperator, RustSymbol, SQLSymbol, operator::LogicOperator};
use orion_exp::{Expression, LogicalExpress, LogicalTrait};
use orion_exp::{ExpressionBuilder, LogicalBuilder};
use winnow::ascii::multispace0;
use winnow::combinator::{fail, peek};
use winnow::error::{StrContext, StrContextValue};
use winnow::token::literal;
use winnow::{ModalResult as WResult, Parser};

use super::LogicSymbolProvider;

impl SymbolFrom<LogicSymbol> for LogicOperator {
    fn op_from(value: LogicSymbol) -> Self {
        match value {
            LogicSymbol::And => LogicOperator::And,
            LogicSymbol::Or => LogicOperator::Or,
            LogicSymbol::Not => LogicOperator::Not,
        }
    }
}

impl SymbolFrom<CmpSymbol> for CmpOperator {
    fn op_from(value: CmpSymbol) -> Self {
        match value {
            CmpSymbol::We => CmpOperator::We,
            CmpSymbol::Eq => CmpOperator::Eq,
            CmpSymbol::Ne => CmpOperator::Ne,
            CmpSymbol::Gt => CmpOperator::Gt,
            CmpSymbol::Ge => CmpOperator::Ge,
            CmpSymbol::Lt => CmpOperator::Lt,
            CmpSymbol::Le => CmpOperator::Le,
        }
    }
}
impl<T, H, S> ConditionParser<T, H, S>
where
    H: CmpParser<T, S>,
    S: LogicSymbolProvider + SymbolProvider,
{
    pub fn lev2_exp(data: &mut &str, stop: Option<&str>) -> WResult<Expression<T, S>> {
        let mut left: Option<Expression<T, S>> = None;
        loop {
            multispace0.parse_next(data)?;
            if data.is_empty() {
                break;
            }
            if let Some(stop) = stop
                && peek_str(stop, data).is_ok()
            {
                literal(stop).parse_next(data)?;
                break;
            }
            if peek_str("(", data).is_ok() {
                let group = Self::group_exp.parse_next(data)?;
                left = Some(group);
                continue;
            } else if peek_str(S::symbol_not(), data).is_ok() {
                S::not_symbol.parse_next(data)?;
                let right = Self::lev0_exp(data, stop)?;
                left = Some(LogicalBuilder::not(right).build());
                continue;
            } else if peek_str(S::symbol_and(), data).is_ok() {
                S::and_symbol.parse_next(data)?;
                let right = Self::lev1_exp(data, stop)?;
                left = Some(Expression::Logic(LogicalExpress::new(
                    LogicOperator::And,
                    left,
                    right,
                )));
                continue;
            } else if peek_str(S::symbol_or(), data).is_ok() {
                S::or_symbol.parse_next(data)?;
                let right = Self::lev2_exp(data, stop)?;
                left = Some(Expression::Logic(LogicalExpress::new(
                    LogicOperator::Or,
                    left,
                    right,
                )));
                continue;
            } else {
                let compare = H::cmp_exp.parse_next(data)?;
                left = Some(Expression::Compare(compare));
                continue;
            }
        }
        match left {
            Some(o) => Ok(o),
            None => fail.context(ctx_desc("left is empty")).parse_next(data),
        }
    }

    #[allow(clippy::never_loop)]
    fn lev0_exp(data: &mut &str, stop: Option<&str>) -> WResult<Expression<T, S>> {
        let mut left: Option<Expression<T, S>> = None;
        loop {
            multispace0.parse_next(data)?;
            if data.is_empty() {
                break;
            }
            if let Some(stop) = stop
                && peek_str(stop, data).is_ok()
            {
                literal(stop).parse_next(data)?;
                break;
            }
            //only one segment;
            if peek_str("(", data).is_ok() {
                let group = Self::group_exp.parse_next(data)?;
                left = Some(group);
                break;
            } else {
                let compare = H::cmp_exp.parse_next(data)?;
                left = Some(Expression::Compare(compare));
                break;
            }
        }
        match left {
            Some(o) => Ok(o),
            None => fail.context(ctx_desc("left is empty")).parse_next(data),
        }
    }

    fn lev1_exp(data: &mut &str, stop: Option<&str>) -> WResult<Expression<T, S>> {
        let mut left: Option<Expression<T, S>> = None;
        loop {
            multispace0.parse_next(data)?;
            if data.is_empty() {
                break;
            }
            if let Some(stop) = stop
                && peek_str(stop, data).is_ok()
            {
                literal(stop).parse_next(data)?;
                break;
            }
            if peek_str("(", data).is_ok() {
                let group = Self::group_exp.parse_next(data)?;
                left = Some(group);
                continue;
            } else if peek_str(S::symbol_not(), data).is_ok() {
                S::not_symbol.parse_next(data)?;
                let right = Self::lev0_exp(data, stop)?;
                left = Some(LogicalBuilder::not(right).build());
                continue;
            } else if peek_str(S::symbol_and(), data).is_ok() {
                S::and_symbol.parse_next(data)?;
                let right = Self::lev1_exp(data, stop)?;
                left = Some(Expression::Logic(LogicalExpress::new(
                    LogicOperator::And,
                    left,
                    right,
                )));
                continue;
            } else if peek_str("||", data).is_ok() {
                break;
            } else {
                let compare = H::cmp_exp.parse_next(data)?;
                left = Some(Expression::Compare(compare));
                continue;
            }
        }
        match left {
            Some(o) => Ok(o),
            None => fail.context(ctx_desc("left is empty")).parse_next(data),
        }
    }

    fn group_exp(data: &mut &str) -> WResult<Expression<T, S>> {
        multispace0.parse_next(data)?;
        symbol_bracket_beg.parse_next(data)?;
        Self::lev2_exp(data, Some(")"))
    }
}
fn peek_str(what: &str, input: &mut &str) -> WResult<()> {
    peek(what).parse_next(input)?;
    Ok(())
}

impl LogicSymbolProvider for RustSymbol {
    fn and_symbol(data: &mut &str) -> WResult<LogicSymbol> {
        let _ = multispace0.parse_next(data)?;
        literal("&&")
            .context(StrContext::Label("symbol"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need '&&'",
            )))
            .parse_next(data)?;
        Ok(LogicSymbol::And)
    }

    fn or_symbol(data: &mut &str) -> WResult<LogicSymbol> {
        let _ = multispace0.parse_next(data)?;
        literal("||")
            .context(StrContext::Label("symbol"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need '||'",
            )))
            .parse_next(data)?;
        Ok(LogicSymbol::Or)
    }

    fn not_symbol(data: &mut &str) -> WResult<LogicSymbol> {
        let _ = multispace0.parse_next(data)?;
        literal("!")
            .context(StrContext::Label("symbol"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need '!'",
            )))
            .parse_next(data)?;
        Ok(LogicSymbol::Not)
    }
}

impl LogicSymbolProvider for SQLSymbol {
    fn and_symbol(data: &mut &str) -> WResult<LogicSymbol> {
        let _ = multispace0.parse_next(data)?;
        literal("and")
            .context(StrContext::Label("symbol"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need 'and'",
            )))
            .parse_next(data)?;
        Ok(LogicSymbol::And)
    }

    fn or_symbol(data: &mut &str) -> WResult<LogicSymbol> {
        let _ = multispace0.parse_next(data)?;
        literal("or")
            .context(StrContext::Label("symbol"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need 'or'",
            )))
            .parse_next(data)?;
        Ok(LogicSymbol::Or)
    }

    fn not_symbol(data: &mut &str) -> WResult<LogicSymbol> {
        let _ = multispace0.parse_next(data)?;
        literal("not")
            .context(StrContext::Label("symbol"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need 'not'",
            )))
            .parse_next(data)?;
        Ok(LogicSymbol::Not)
    }
}
