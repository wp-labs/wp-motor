use orion_error::UvsReason;
use wp_connector_api::{SinkError, SinkReason};
use wp_connector_api::{SourceError, SourceReason};
use wp_error::error_handling::ErrorHandlingStrategy;
use wp_error::error_handling::RobustnessMode;
use wpl::{WparseError, WparseReason};

// 运行时错误策略映射：统一在 runtime/errors 下维护
// - sink 写入错误 → 重试/容错/终止策略
// - 解析错误 → 忽略/容错/终止策略
// - 源派发错误 → 容忍/重试/终止/抛出

pub fn err4_send_to_sink(err: &SinkError, mode: &RobustnessMode) -> ErrorHandlingStrategy {
    match err.reason() {
        SinkReason::Sink(e) => {
            warn_data!("sink error: {}", e);
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
        SourceReason::SupplierError(e) => {
            warn_data!("{}", e);
            ErrorHandlingStrategy::Throw
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
        SourceReason::Uvs(e) => universal_proc_stg(mode, e),
    }
}

fn universal_proc_stg(mode: &RobustnessMode, e: &UvsReason) -> ErrorHandlingStrategy {
    match e {
        UvsReason::LogicError(e) => match mode {
            RobustnessMode::Strict => {
                error_data!("logic error: {}", e);
                ErrorHandlingStrategy::Tolerant
            }
            _ => {
                error_data!("logic error: {}", e);
                ErrorHandlingStrategy::Throw
            }
        },
        UvsReason::DataError(_, _) => ErrorHandlingStrategy::Tolerant,
        UvsReason::SystemError(e) => {
            warn_data!("data error: {}", e);
            ErrorHandlingStrategy::Tolerant
        }
        UvsReason::BusinessError(e) => {
            warn_data!("biz error: {}", e);
            ErrorHandlingStrategy::Tolerant
        }
        UvsReason::ConfigError(e) => {
            error_data!("conf error: {}", e);
            ErrorHandlingStrategy::Throw
        }
        _ => {
            unimplemented!("robust error: {}", e)
        }
    }
}
