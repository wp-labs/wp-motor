use std::{fmt::Display, marker::PhantomData};

use derive_getters::Getters;

use crate::{CmpSymbolProvider, LogicOperator, LogicSymbolProvider};

use super::compare::Comparison;

#[derive(Debug, PartialEq, Clone, Getters)]
pub struct LogicalExpress<T: Sized, S>
where
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    _keep1: PhantomData<S>,
    pub op: LogicOperator,
    pub left: Option<Box<Expression<T, S>>>,
    pub right: Box<Expression<T, S>>,
}

#[derive(Debug, PartialEq, Clone)]

pub enum Expression<T: Sized, S>
where
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    Logic(LogicalExpress<T, S>),
    Compare(Comparison<T, S>),
}
impl<T: Sized + Display, S> Display for Expression<T, S>
where
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Logic(v) => v.fmt(f),
            Expression::Compare(v) => v.fmt(f),
        }
    }
}

impl<T, S> LogicalExpress<T, S>
where
    S: LogicSymbolProvider + CmpSymbolProvider,
{
    pub fn new(op: LogicOperator, left: Option<Expression<T, S>>, right: Expression<T, S>) -> Self {
        Self {
            _keep1: PhantomData,
            op,
            left: left.map(Box::new),
            right: Box::new(right),
        }
    }
}
