use orion_error::UnifiedReason;
use orion_error::conversion::ToStructError;
use wp_connector_api::{SinkError, SinkReason};
use wp_connector_api::{SourceError, SourceReason};
use wp_error::error_handling::ErrorHandlingStrategy;
use wp_error::error_handling::RobustnessMode;
use wp_error::{RunError, RunReason, SourceFocus};
use wpl::{WparseError, WparseReason};

// 运行时错误策略映射：统一在 runtime/errors 下维护
// - sink 写入错误 → 重试/容错/终止策略
// - 解析错误 → 忽略/容错/终止策略
// - 源派发错误 → 容忍/重试/终止/抛出

pub fn err4_send_to_sink(err: &SinkError, mode: &RobustnessMode) -> ErrorHandlingStrategy {
    match err.reason() {
        SinkReason::Sink => {
            warn_data!(
                "sink error: {}",
                err.detail().as_deref().unwrap_or("sink error")
            );
            ErrorHandlingStrategy::FixRetry
        }
        SinkReason::Mock => {
            info_data!("mock ");
            ErrorHandlingStrategy::FixRetry
        }
        SinkReason::StgCtrl => {
            info_data!("stg ctrl");
            ErrorHandlingStrategy::FixRetry
        }
        SinkReason::Uvs(e) => universal_proc_stg(mode, e),
    }
}

pub fn err4_engine_parse_data(err: &WparseError, mode: &RobustnessMode) -> ErrorHandlingStrategy {
    match err.reason() {
        WparseReason::Plugin(_) => ErrorHandlingStrategy::Ignore,
        WparseReason::LineProc(_) => ErrorHandlingStrategy::Ignore,
        WparseReason::NotMatch => ErrorHandlingStrategy::Ignore,
        WparseReason::Uvs(e) => universal_proc_stg(mode, e),
    }
}

pub fn err4_dispatch_data(err: &SourceError, mode: &RobustnessMode) -> ErrorHandlingStrategy {
    match err.reason() {
        SourceReason::SupplierError => {
            warn_data!(
                "{}",
                err.detail().as_deref().unwrap_or("source supplier error")
            );
            ErrorHandlingStrategy::Throw
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
        SourceReason::Uvs(e) => universal_proc_stg(mode, e),
    }
}

pub fn source_error_to_run_error(err: SourceError) -> RunError {
    let detail = err.detail().clone();
    let reason = match err.reason() {
        SourceReason::NotData => RunReason::Source(SourceFocus::NoData),
        SourceReason::EOF => RunReason::Source(SourceFocus::Eof),
        SourceReason::SupplierError => RunReason::Source(SourceFocus::SupplierError(
            detail.clone().unwrap_or_default(),
        )),
        SourceReason::Disconnect => {
            RunReason::Source(SourceFocus::Disconnect(detail.clone().unwrap_or_default()))
        }
        SourceReason::Other => {
            RunReason::Source(SourceFocus::Other(detail.clone().unwrap_or_default()))
        }
        SourceReason::Uvs(reason) => RunReason::Uvs(reason.clone()),
    };
    let mut run_error = reason.to_err();
    if let Some(detail) = detail {
        run_error = run_error.with_detail(detail);
    }
    run_error
}

fn universal_proc_stg(mode: &RobustnessMode, e: &UnifiedReason) -> ErrorHandlingStrategy {
    match e {
        UnifiedReason::ValidationError => {
            error_data!("validation error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::LogicError => match mode {
            RobustnessMode::Strict => {
                error_data!("logic error");
                ErrorHandlingStrategy::Tolerant
            }
            _ => {
                error_data!("logic error");
                ErrorHandlingStrategy::Throw
            }
        },
        UnifiedReason::DataError => ErrorHandlingStrategy::Tolerant,
        UnifiedReason::SystemError => {
            warn_data!("system error");
            ErrorHandlingStrategy::Tolerant
        }
        UnifiedReason::BusinessError => {
            warn_data!("biz error");
            ErrorHandlingStrategy::Tolerant
        }
        UnifiedReason::RunRuleError => {
            warn_data!("run rule error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::NotFoundError => {
            error_data!("not found error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::PermissionError => {
            error_data!("permission error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::NetworkError => {
            warn_data!("network error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::ResourceError => {
            error_data!("resource error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::TimeoutError => {
            warn_data!("timeout error");
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::ConfigError(e) => {
            error_data!("conf error: {}", e);
            ErrorHandlingStrategy::Throw
        }
        UnifiedReason::ExternalError => {
            error_data!("external error");
            ErrorHandlingStrategy::Throw
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_error_does_not_panic_and_throws() {
        let err = SourceError::from(SourceReason::Uvs(UnifiedReason::ResourceError));
        let stg = err4_dispatch_data(&err, &RobustnessMode::Debug);
        assert!(matches!(stg, ErrorHandlingStrategy::Throw));
    }
}
