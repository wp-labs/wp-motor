use orion_exp::{
    CmpSymbolProvider,
    core::{compare::Comparison, logic::Expression},
    operator::symbols::SymbolProvider,
};
use std::marker::PhantomData;
use winnow::ModalResult as WResult;

use crate::symbol::LogicSymbol;

mod parser;
#[cfg(test)]
mod test;

pub trait CmpParser<T, S>
where
    S: CmpSymbolProvider,
{
    fn cmp_exp(data: &mut &str) -> WResult<Comparison<T, S>>;
}

pub struct ConditionParser<T, H, S> {
    _keep1: PhantomData<T>,
    _keep2: PhantomData<H>,
    _keep3: PhantomData<S>,
}

impl<T, H, S> ConditionParser<T, H, S>
where
    H: CmpParser<T, S>,
    S: LogicSymbolProvider + SymbolProvider,
{
    pub fn end_exp(data: &mut &str, stop: &str) -> WResult<Expression<T, S>> {
        Self::lev2_exp(data, Some(stop))
    }
    pub fn exp(data: &mut &str) -> WResult<Expression<T, S>> {
        Self::lev2_exp(data, None)
    }
}

pub trait SymbolFrom<T> {
    fn op_from(value: T) -> Self;
}

pub trait LogicSymbolProvider {
    fn and_symbol(data: &mut &str) -> WResult<LogicSymbol>;
    fn or_symbol(data: &mut &str) -> WResult<LogicSymbol>;
    fn not_symbol(data: &mut &str) -> WResult<LogicSymbol>;
}
