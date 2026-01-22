pub mod macros;
pub mod strategies;
use wp_connector_api::SourceError;

pub trait SinkConv {
    fn from_sink_err<E>(e: E) -> Self;
    fn from_sink_err_info<E, S: Into<String>>(e: E, info: S) -> Self;
    fn from_sink<S: Into<String>>(info: S) -> Self;
}

pub trait SourceConv {
    fn from_source_err<E: Into<SourceError>>(e: E) -> Self;
    //fn from_source_err_info<E, S: Into<String>>(e: E, info: S) -> Self;
    fn from_source<S: Into<String>>(info: S) -> Self;
}
