use std::fmt::Display;

use orion_error::conversion::ToStructError;
use orion_error::reason::DomainReason;
use orion_error::{StructError, UnifiedReason};
use wp_error::run_error::{RunReason, RunResult};

pub use orion_error::conversion::ConvErr as ErrorConv;

pub trait UvsFrom: Sized + From<UnifiedReason> {
    fn from_conf() -> Self {
        Self::from(UnifiedReason::core_conf())
    }

    fn from_validation() -> Self {
        Self::from(UnifiedReason::validation_error())
    }

    fn from_rule() -> Self {
        Self::from(UnifiedReason::rule_error())
    }

    fn from_res() -> Self {
        Self::from(UnifiedReason::resource_error())
    }

    fn from_biz() -> Self {
        Self::from(UnifiedReason::business_error())
    }
}

impl<T> UvsFrom for T where T: From<UnifiedReason> {}

pub trait ErrorOweBase<T, R: DomainReason> {
    fn owe(self, reason: R) -> Result<T, StructError<R>>;
}

impl<T, E, R> ErrorOweBase<T, R> for Result<T, E>
where
    E: Display,
    R: DomainReason,
{
    fn owe(self, reason: R) -> Result<T, StructError<R>> {
        self.map_err(|err| reason.to_err().with_detail(err.to_string()))
    }
}

pub trait ErrorOweSource<T> {
    fn owe_conf_source(self) -> RunResult<T>;
}

impl<T, E> ErrorOweSource<T> for Result<T, E>
where
    E: Display,
{
    fn owe_conf_source(self) -> RunResult<T> {
        self.map_err(|err| RunReason::from_conf().to_err().with_detail(err.to_string()))
    }
}

pub trait WrapStructErrorAs<R: DomainReason> {
    fn wrap_as<R2: DomainReason>(self, reason: R2, detail: impl Into<String>) -> StructError<R2>;
}

impl<R> WrapStructErrorAs<R> for StructError<R>
where
    R: DomainReason,
{
    fn wrap_as<R2: DomainReason>(self, reason: R2, detail: impl Into<String>) -> StructError<R2> {
        reason.to_err().with_detail(detail.into()).with_source(self)
    }
}
