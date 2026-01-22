use crate::{Comparison, operator::symbols::SymbolProvider};

use super::{
    LogicOperator,
    core::logic::{Expression, LogicalExpress},
};
pub trait ExpressionBuilder<T, S> {
    type Output;

    fn build(self) -> Self::Output;
}

pub struct LogicalBuilder<T, S>
where
    S: SymbolProvider,
{
    operator: LogicOperator,
    left: Option<Expression<T, S>>,
    right: Expression<T, S>,
}

pub trait LogicalTrait<T> {
    fn and(left: T, right: T) -> Self;
    fn or(left: T, right: T) -> Self;
    fn not(right: T) -> Self;
}

impl<T, S> LogicalTrait<Expression<T, S>> for LogicalBuilder<T, S>
where
    S: SymbolProvider,
{
    fn and(left: Expression<T, S>, right: Expression<T, S>) -> Self {
        Self::new(LogicOperator::And, Some(left), right)
    }

    fn or(left: Expression<T, S>, right: Expression<T, S>) -> Self {
        Self::new(LogicOperator::Or, Some(left), right)
    }

    fn not(right: Expression<T, S>) -> Self {
        Self::new(LogicOperator::Not, None, right)
    }
}

impl<T, S> LogicalTrait<Comparison<T, S>> for LogicalBuilder<T, S>
where
    S: SymbolProvider,
{
    fn and(left: Comparison<T, S>, right: Comparison<T, S>) -> Self {
        Self::new(
            LogicOperator::And,
            Some(Expression::Compare(left)),
            Expression::Compare(right),
        )
    }

    fn or(left: Comparison<T, S>, right: Comparison<T, S>) -> Self {
        Self::new(
            LogicOperator::Or,
            Some(Expression::Compare(left)),
            Expression::Compare(right),
        )
    }

    fn not(right: Comparison<T, S>) -> Self {
        Self::new(LogicOperator::Not, None, Expression::Compare(right))
    }
}

impl<T, S> LogicalBuilder<T, S>
where
    S: SymbolProvider,
{
    fn new(
        operator: LogicOperator,
        left: Option<Expression<T, S>>,
        right: Expression<T, S>,
    ) -> Self {
        Self {
            operator,
            left,
            right,
        }
    }
}

impl<T, S> ExpressionBuilder<T, S> for LogicalBuilder<T, S>
where
    S: SymbolProvider,
{
    type Output = Expression<T, S>;

    fn build(self) -> Self::Output {
        Expression::Logic(LogicalExpress::new(self.operator, self.left, self.right))
    }
}
