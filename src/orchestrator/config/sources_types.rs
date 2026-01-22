//! Unified Source types for configuration
//!
//! 说明：本模块替代 legacy_sources，承载最小的源配置类型（仅 key/tag/encode 等），
//! 供 orchestrator 层在 WPL 索引与装配阶段使用。V1 旧结构已移除，不再回退。

use std::fmt::{Display, Formatter};

use educe::Educe;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Default)]
pub enum DataEncoding {
    #[serde(rename = "base64")]
    Base64,
    #[serde(rename = "hex")]
    Hex,
    #[default]
    #[serde(rename = "text")]
    Text,
}

#[derive(PartialEq, Deserialize, Serialize, Clone, Debug)]
pub struct FileSourceConf {
    pub key: String,
    pub path: String,
    pub enable: bool,
    #[serde(default)]
    pub encode: DataEncoding,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for FileSourceConf {
    fn default() -> Self {
        Self {
            key: "file_1".to_string(),
            path: "./sample.dat".to_string(),
            enable: false,
            encode: DataEncoding::Text,
            tags: vec!["dev_src_ip: 10.0.0.1".to_string()],
        }
    }
}
impl FileSourceConf {
    pub fn use_cli(&mut self, cli_path: Option<String>) {
        if let Some(cli_path) = cli_path {
            self.path = cli_path;
        }
    }
    pub fn new<S: Into<String>>(path: S) -> Self {
        Self {
            key: "file_1".to_string(),
            path: path.into(),
            enable: false,
            encode: DataEncoding::default(),
            tags: vec!["dev_src_ip : 10.0.0.1".to_string()],
        }
    }

    pub fn on_new<S: Into<String>>(path: S) -> Self {
        Self {
            key: "file_1".to_string(),
            path: path.into(),
            enable: true,
            encode: DataEncoding::default(),
            tags: vec!["dev_src_ip : 10.0.0.1".to_string()],
        }
    }
}

#[derive(Educe, PartialEq, Clone)]
#[educe(Debug, Default)]
pub enum SourceConfig {
    #[educe(Default)]
    File(FileSourceConf),
}

impl Display for SourceConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceConfig::File(_x) => write!(f, "source-file"),
        }
    }
}

impl SourceConfig {
    pub fn get_key(&self) -> String {
        match self {
            SourceConfig::File(x) => x.key.clone(),
        }
    }
}
