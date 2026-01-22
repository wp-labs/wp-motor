use super::instance::SinkInstanceConf;
use crate::structure::FlexGroup;
use orion_conf::{
    ToStructError,
    error::{ConfIOReason, OrionConfResult},
};
use orion_error::UvsValidationFrom;
use serde::{Deserialize, Serialize};
use wp_model_core::model::fmt_def::TextFmt;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct SinkRouteConf {
    pub version: String,
    pub sink_group: FlexGroup,
}

impl SinkRouteConf {
    pub fn append_sink(&mut self, sink: SinkInstanceConf) {
        self.sink_group.append(sink);
    }
}

impl Default for SinkRouteConf {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            sink_group: FlexGroup::new(
                "example",
                vec!["oml/example_1*", "oml/example_2*"],
                None,
                vec![],
                SinkInstanceConf::file_new(
                    "example_sink".to_string(),
                    TextFmt::ProtoText,
                    "sink_out.dat",
                    None,
                ),
            ),
        }
    }
}

impl crate::structure::Validate for SinkRouteConf {
    fn validate(&self) -> OrionConfResult<()> {
        if self.version.trim().is_empty() {
            return ConfIOReason::from_validation("sink route version must not be empty")
                .err_result();
        }
        for s in &self.sink_group.sinks {
            if let Err(e) = s.validate() {
                return ConfIOReason::from_validation(format!("sink validate fail: {}", e))
                    .err_result();
            }
        }
        Ok(())
    }
}
