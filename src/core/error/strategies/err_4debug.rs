use orion_error::{ErrStrategy, UvsReason};
use wp_connector_api::{SinkError, SinkReason};
use wp_connector_api::{SourceError, SourceReason};
use wp_error::error_handling::ErrorHandlingStrategy;
use wp_error::parse_error::{OMLCodeError, OMLCodeReason};
use wpl::parser::error::{WplCodeError, WplCodeReason};
use wpl::{WparseError, WparseReason};

use super::ErrorHandlingPolicy;

#[derive(Default)]
pub struct Err4Debug {}

impl Err4Debug {
    pub(crate) const fn init() -> Self {
        Self {}
    }
    fn err_4universal(&self, reason: &UvsReason) -> ErrStrategy {
        match reason {
            //UniversalReason::SysError(_) => ErrorStg::Interrupt,
            //UniversalReason::LogicError(_) => ErrorStg::Interrupt,
            //_ => ErrorStg::Tolerant,
            //UniversalReason::LogicError(_) => {}
            //UniversalReason::BizError(_) => {}
            UvsReason::DataError(_, _) => ErrStrategy::Ignore,
            //UniversalReason::SysError(_) => {}
            //UniversalReason::ResError(_) => {}
            //UniversalReason::ConfError(_) => {}
            //UniversalReason::RuleError(_) => {}
            _ => ErrStrategy::Throw,
        }
    }
}
impl ErrorHandlingPolicy for Err4Debug {
    fn err4_send_to_sink(&self, err: &SinkError) -> ErrorHandlingStrategy {
        match err.reason() {
            SinkReason::Sink(e) => {
                warn_data!("sink error: {}", e);
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
            OMLCodeReason::Syntax(_) => ErrStrategy::Throw,
            OMLCodeReason::NotFound(_) => ErrStrategy::Ignore,
            OMLCodeReason::Uvs(e) => self.err_4universal(e),
        }
    }
    fn err4_load_wpl(&self, err: &WplCodeError) -> ErrStrategy {
        match err.reason() {
            WplCodeReason::Plugin(_) => ErrStrategy::Throw,
            WplCodeReason::Syntax(_) => ErrStrategy::Ignore,
            WplCodeReason::Empty(_) => ErrStrategy::Ignore,
            WplCodeReason::UnSupport(_) => ErrStrategy::Ignore,
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
            SourceReason::SupplierError(e) => {
                warn_data!("{}", e);
                ErrorHandlingStrategy::FixRetry
            }
            SourceReason::NotData => ErrorHandlingStrategy::Tolerant,
            SourceReason::EOF => ErrorHandlingStrategy::Terminate,
            SourceReason::Disconnect(e) => {
                warn_data!("rule error: {}", e);
                ErrorHandlingStrategy::FixRetry
            }
            SourceReason::Other(e) => {
                error_data!("other error: {}", e);
                ErrorHandlingStrategy::Throw
            }
            SourceReason::Uvs(e) => ErrorHandlingStrategy::from(self.err_4universal(e)),
        }
    }
}
