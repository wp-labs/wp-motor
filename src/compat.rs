use std::fmt::Display;

use orion_error::conversion::ToStructError;
use orion_error::{StructError, reason::DomainReason};
use wp_error::error_handling::ErrorHandlingStrategy;

pub enum ErrStrategy {
    Retry,
    Ignore,
    Throw,
}

impl From<ErrStrategy> for ErrorHandlingStrategy {
    fn from(value: ErrStrategy) -> Self {
        match value {
            ErrStrategy::Retry => ErrorHandlingStrategy::FixRetry,
            ErrStrategy::Ignore => ErrorHandlingStrategy::Ignore,
            ErrStrategy::Throw => ErrorHandlingStrategy::Throw,
        }
    }
}

pub trait LegacyErrorArg<R: DomainReason> {
    fn into_error_with_detail<E: Display>(self, err: E) -> StructError<R>;
}

impl<R> LegacyErrorArg<R> for R
where
    R: DomainReason,
{
    fn into_error_with_detail<E: Display>(self, err: E) -> StructError<R> {
        self.to_err().with_detail(err.to_string())
    }
}

impl<R> LegacyErrorArg<R> for StructError<R>
where
    R: DomainReason,
{
    fn into_error_with_detail<E: Display>(self, err: E) -> StructError<R> {
        let err = err.to_string();
        let detail = self.detail().clone().filter(|detail| !detail.is_empty());
        match detail {
            Some(detail) => self.with_detail(format!("{}: {}", detail, err)),
            None => self.with_detail(err),
        }
    }
}

pub trait LegacyOwe<T, R: DomainReason> {
    fn owe<A>(self, reason: A) -> Result<T, StructError<R>>
    where
        A: LegacyErrorArg<R>;
}

impl<T, E, R> LegacyOwe<T, R> for Result<T, E>
where
    E: Display,
    R: DomainReason,
{
    fn owe<A>(self, reason: A) -> Result<T, StructError<R>>
    where
        A: LegacyErrorArg<R>,
    {
        self.map_err(|err| reason.into_error_with_detail(err))
    }
}
