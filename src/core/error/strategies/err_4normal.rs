use crate::compat::ErrStrategy;
use orion_error::UnifiedReason;
use wp_connector_api::{SinkError, SinkReason};
use wp_connector_api::{SourceError, SourceReason};
use wp_error::error_handling::ErrorHandlingStrategy;
use wp_error::parse_error::{OMLCodeError, OMLCodeReason};
use wpl::parser::error::{WplCodeError, WplCodeReason};
use wpl::{WparseError, WparseReason};

use super::ErrorHandlingPolicy;

#[derive(Default)]
pub struct Err4Normal {}

impl Err4Normal {
    pub(crate) const fn init() -> Self {
        Self {}
    }
    fn err_4universal(&self, reason: &UnifiedReason) -> ErrStrategy {
        match reason {
            //UniversalReason::LogicError(_) => { }
            //UniversalReason::BizError(_) => {}
            UnifiedReason::DataError => ErrStrategy::Ignore,
            //UniversalReason::SysError(_) => { }
            //UniversalReason::ResError(_) => ErrorStg::FixRetry,
            //UniversalReason::ConfError(_) => {}
            UnifiedReason::RunRuleError => ErrStrategy::Ignore,
            _ => ErrStrategy::Throw,
        }
    }
}
impl ErrorHandlingPolicy for Err4Normal {
    fn err4_send_to_sink(&self, err: &SinkError) -> ErrorHandlingStrategy {
        match err.reason() {
            SinkReason::Sink => {
                warn_data!(
                    "sink error: {}",
                    err.detail().as_deref().unwrap_or("sink error")
                );
                ErrorHandlingStrategy::FixRetry
            }

            SinkReason::Mock => {
                info_data!("mock ",);
                ErrorHandlingStrategy::FixRetry
            }
            SinkReason::StgCtrl => {
                //for testcase
                info_data!("stg ctrl");
                ErrorHandlingStrategy::FixRetry
            }
            SinkReason::Uvs(e) => ErrorHandlingStrategy::from(self.err_4universal(e)),
        }
    }

    fn err4_load_oml(&self, err: &OMLCodeError) -> ErrStrategy {
        match err.reason() {
            OMLCodeReason::Syntax(_) => ErrStrategy::Ignore,
            OMLCodeReason::NotFound(_) => ErrStrategy::Ignore,
            OMLCodeReason::Uvs(e) => self.err_4universal(e),
        }
    }

    fn err4_load_wpl(&self, err: &WplCodeError) -> ErrStrategy {
        match err.reason() {
            WplCodeReason::Plugin => ErrStrategy::Ignore,
            WplCodeReason::Syntax => ErrStrategy::Ignore,
            WplCodeReason::Empty => ErrStrategy::Ignore,
            WplCodeReason::UnSupport => ErrStrategy::Ignore,
            WplCodeReason::Uvs(e) => self.err_4universal(e),
        }
    }
    fn err4_engine_parse_data(&self, err: &WparseError) -> ErrorHandlingStrategy {
        match err.reason() {
            WparseReason::Plugin(_) => ErrorHandlingStrategy::Ignore,
            WparseReason::LineProc(_) => ErrorHandlingStrategy::Ignore,
            WparseReason::NotMatch => ErrorHandlingStrategy::Ignore,
            WparseReason::Uvs(e) => ErrorHandlingStrategy::from(self.err_4universal(e)),
        }
    }

    fn err4_dispatch_data(&self, err: &SourceError) -> ErrorHandlingStrategy {
        match err.reason() {
            SourceReason::SupplierError => {
                warn_data!(
                    "{}",
                    err.detail().as_deref().unwrap_or("source supplier error")
                );
                ErrorHandlingStrategy::FixRetry
            }
            SourceReason::NotData => ErrorHandlingStrategy::Tolerant,
            SourceReason::EOF => ErrorHandlingStrategy::Terminate,
            SourceReason::Disconnect => {
                warn_data!(
                    "rule error: {}",
                    err.detail().as_deref().unwrap_or("source disconnect")
                );
                ErrorHandlingStrategy::FixRetry
            }
            SourceReason::Other => {
                error_data!(
                    "other error: {}",
                    err.detail().as_deref().unwrap_or("source error")
                );
                ErrorHandlingStrategy::Throw
            }
            SourceReason::Uvs(e) => ErrorHandlingStrategy::from(self.err_4universal(e)),
        }
    }
}
