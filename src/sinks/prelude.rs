pub use orion_error::ErrorWith;
pub use wp_model_core::model::DataRecord;
pub use wpl::PkgID;
pub use wpl::generator::FmtFieldVec;
//pub use wp_model_core::model::DataRecord;
#[allow(unused_imports)]
pub use wp_model_core::model::Value;

// Async Sink traits: import from API crate directly，避免依赖 core 的 re-export
#[allow(unused_imports)]
pub use wp_connector_api::{
    AsyncCtrl, AsyncRawDataSink as AsyncRawdatSink, AsyncRecordSink, AsyncSink,
};
// For external sinks registry bridge
pub use crate::types::AnyResult;
