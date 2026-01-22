use wp_conf::conf::group::FlexiGroupConf;
use wp_conf::conf::io::OutFile;
use wp_conf::conf::sink::SinkUseConf;
use wp_model_core::model::fmt_def::TextFmt;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct SinkMissConf {
    pub sinks: Vec<SinkUseConf>,
}

impl SinkMissConf {
    pub fn new_file(fmt: TextFmt, path: String) -> Self {
        Self { sinks: vec![SinkUseConf::file_new("miss_sink".to_string(), fmt, path, None, None)] }
    }
    pub fn to_group_conf(self) -> FlexiGroupConf {
        FlexiGroupConf::build_conf("miss", self.sinks)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct SinkResidueConf {
    pub sinks: Vec<SinkUseConf>,
}

impl Default for SinkResidueConf {
    fn default() -> Self {
        Self {
            sinks: vec![SinkUseConf::file_new(
                "residue_sink".to_string(),
                TextFmt::Raw,
                "./out/residue.dat".to_string(),
                None,
            )],
        }
    }
}
impl SinkResidueConf {
    pub fn new_file(fmt: TextFmt, path: String) -> Self {
        Self { sinks: vec![SinkUseConf::file_new("residue_sink".to_string(), fmt, path, None, None)] }
    }
    pub fn to_group_conf(self) -> FlexiGroupConf {
        FlexiGroupConf::build_conf("residue", self.sinks)
    }
}
